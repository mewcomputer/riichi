use super::*;
use serde_json::Value;

fn delivery_events_to_sse(
    messages: Vec<riichi_persistence::DeliveryEventRecord>,
    cursor: &mut Option<i64>,
) -> Vec<Result<Event, Infallible>> {
    messages
        .into_iter()
        .map(|message| {
            *cursor = Some(message.event_seq);
            Ok(Event::default()
                .id(message.event_seq.to_string())
                .event(message.message_type)
                .json_data(message.payload)
                .unwrap_or_else(|_| Event::default().event("error").data("event unavailable")))
        })
        .collect()
}

pub(super) async fn human_queue(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<HumanIssueQuery>,
    jar: CookieJar,
) -> Result<Json<HumanQueueResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Viewer) {
        return Err(ApiError::Auth(AuthError::InsufficientRole));
    }
    let issues = state
        .application
        .human_queue(project_id, query.limit.unwrap_or(200))
        .await
        .map_err(|error| {
            tracing::error!(project_id = %project_id, error = ?error, "human queue query failed");
            ApiError::from(error)
        })?;
    Ok(Json(HumanQueueResponse { issues }))
}

#[derive(Debug, Deserialize)]
pub(super) struct HumanIssueQuery {
    pub limit: Option<i64>,
}

pub(super) async fn project_events(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<Sse<impl futures_core::Stream<Item = Result<Event, Infallible>>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let after = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok());
    let database = state.application.database();
    let mut event_wakeups = state.event_wakeups.subscribe();
    let stream = async_stream::stream! {
        let mut cursor = after;
        match database.events_since(project_id, cursor, 100).await {
            Ok(messages) => {
                for event in delivery_events_to_sse(messages, &mut cursor) {
                    yield event;
                }
            }
            Err(_) => {
                yield Ok(Event::default().event("error").data("event stream unavailable"));
                return;
            }
        }

        loop {
            match event_wakeups.recv().await {
                Ok(super::EventWakeup::Project(wakeup_project_id)) if wakeup_project_id == project_id => {
                    match database.events_since(project_id, cursor, 100).await {
                        Ok(messages) => {
                            for event in delivery_events_to_sse(messages, &mut cursor) {
                                yield event;
                            }
                        }
                        Err(_) => {
                            yield Ok(Event::default().event("error").data("event stream unavailable"));
                            break;
                        }
                    }
                }
                Ok(_) => continue,
                Err(_) => {
                    yield Ok(Event::default().event("error").data("event stream unavailable"));
                    break;
                }
            }
        }
    };
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    ))
}

pub(super) async fn create_issue(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let issue = state
        .application
        .create_issue(
            project_id,
            riichi_persistence::IssueCreate {
                id: Uuid::now_v7(),
                display_key: String::new(),
                title: request.title,
                body: request.body,
                status: request.status,
                agent_eligible: request.agent_eligible,
                spec_complete: request.spec_complete,
                rank: request.rank,
                labels: request.labels,
                assignee_account_id: request.assignee_account_id,
                parent_issue_id: request.parent_issue_id,
            },
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn create_team_issue(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateIssueRequest>,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_member(&principal, team_id)?;
    let project_id = request.project_id.ok_or(ApiError::InvalidRequest)?;
    if !state
        .application
        .database()
        .project_belongs_to_team(project_id, team_id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::NotFound);
    }
    let issue = state
        .application
        .create_issue(
            project_id,
            riichi_persistence::IssueCreate {
                id: Uuid::now_v7(),
                display_key: String::new(),
                title: request.title,
                body: request.body,
                status: request.status,
                agent_eligible: request.agent_eligible,
                spec_complete: request.spec_complete,
                rank: request.rank,
                labels: request.labels,
                assignee_account_id: request.assignee_account_id,
                parent_issue_id: request.parent_issue_id,
            },
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn get_issue(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let issue = state
        .application
        .get_issue(project_id, issue_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn create_comment(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<CreateCommentRequest>,
) -> Result<Json<riichi_persistence::Comment>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let serialized =
        serde_json::to_string(&request.content).map_err(|_| ApiError::InvalidRequest)?;
    if serialized.len() > 100_000 {
        return Err(ApiError::InvalidRequest);
    }
    let body = tiptap_plain_text(&request.content);
    if body.trim().is_empty() || body.chars().count() > 20_000 {
        return Err(ApiError::InvalidRequest);
    }
    let comment = state
        .application
        .database()
        .create_human_comment(
            project_id,
            issue_id,
            principal.account.id,
            &body,
            request.content,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(comment))
}

pub(super) async fn issue_activity(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<IssueActivityQuery>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::Activity>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let activity = state
        .application
        .database()
        .issue_activity(project_id, issue_id, query.limit.unwrap_or(200))
        .await
        .map_err(ApiError::from)?;
    Ok(Json(activity))
}

#[derive(Debug, Deserialize)]
pub(super) struct IssueActivityQuery {
    limit: Option<i64>,
}

pub(super) fn tiptap_plain_text(value: &Value) -> String {
    match value {
        Value::Object(object) => {
            let mut text = object
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            if let Some(content) = object.get("content").and_then(Value::as_array) {
                for child in content {
                    let child_text = tiptap_plain_text(child);
                    if !text.is_empty() && !child_text.is_empty() {
                        text.push('\n');
                    }
                    text.push_str(&child_text);
                }
            }
            text
        }
        Value::Array(values) => values
            .iter()
            .map(tiptap_plain_text)
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

pub(super) async fn get_issue_collaborators(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<Json<CollaboratorResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let collaborators = state
        .application
        .lease_collaborators(project_id, issue_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CollaboratorResponse { collaborators }))
}

pub(super) async fn get_quarantined_attempts(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<QuarantinedAttemptsQuery>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::QuarantinedAttempt>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let attempts = state
        .application
        .database()
        .quarantined_attempts(project_id, issue_id, query.limit.unwrap_or(100))
        .await
        .map_err(ApiError::from)?;
    Ok(Json(attempts))
}

#[derive(Debug, Deserialize)]
pub(super) struct QuarantinedAttemptsQuery {
    limit: Option<i64>,
}

pub(super) async fn grant_issue_collaborator(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<GrantCollaboratorRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    state
        .application
        .grant_lease_collaborator(
            project_id,
            issue_id,
            request.lease_id,
            request.session_id,
            &request.capability,
            &request.grant_mode,
            principal.account.id,
            request.expires_in_seconds.map(Duration::seconds),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn revoke_issue_collaborator(
    State(state): State<AppState>,
    Path((project_id, issue_id, session_id, capability)): Path<(Uuid, Uuid, Uuid, String)>,
    Query(query): Query<CollaboratorLeaseQuery>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    state
        .application
        .revoke_lease_collaborator(
            project_id,
            issue_id,
            query.lease_id,
            session_id,
            &capability,
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn update_issue(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<UpdateIssueRequest>,
) -> Result<(HeaderMap, Json<riichi_persistence::IssueRecord>), ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let result = state
        .application
        .update_issue_with_transaction(
            project_id,
            issue_id,
            riichi_persistence::IssueUpdate {
                expected_version: request.expected_version,
                title: request.title,
                status: request.status,
                importance: request.importance,
                agent_eligible: request.agent_eligible,
                spec_complete: request.spec_complete,
                rank: request.rank,
                labels: request.labels,
                assignee_account_id: request.assignee_account_id,
            },
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-riichi-transaction-id",
        HeaderValue::from_str(&result.transaction_id.to_string())
            .map_err(|_| ApiError::InvalidRequest)?,
    );
    Ok((headers, Json(result.issue)))
}

pub(super) async fn delete_issue(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<DeleteIssueQuery>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let issue = state
        .application
        .update_issue(
            project_id,
            issue_id,
            riichi_persistence::IssueUpdate {
                expected_version: query.expected_version,
                status: Some("canceled".to_owned()),
                ..Default::default()
            },
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn create_issue_edge(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateIssueEdgeRequest>,
) -> Result<Json<riichi_persistence::IssueEdge>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let edge = state
        .application
        .create_issue_edge(
            project_id,
            request.source_issue_id,
            request.target_issue_id,
            &request.edge_type,
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(edge))
}

pub(super) async fn remove_issue_edge(
    State(state): State<AppState>,
    Path((project_id, edge_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    state
        .application
        .remove_issue_edge(project_id, edge_id, principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn create_hold(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<CreateHoldRequest>,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let issue = state
        .application
        .create_hold(
            project_id,
            issue_id,
            &request.hold_type,
            &request.reason,
            principal.account.id,
            request.expires_in_seconds.map(Duration::seconds),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn release_hold(
    State(state): State<AppState>,
    Path((project_id, hold_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    state
        .application
        .release_hold(project_id, hold_id, principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn takeover_issue(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<TakeoverRequest>,
) -> Result<Json<riichi_persistence::RecoveryChecklist>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let checklist = state
        .application
        .takeover_issue(project_id, issue_id, principal.account.id, &request.reason)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(checklist))
}

pub(super) async fn complete_recovery(
    State(state): State<AppState>,
    Path((project_id, checklist_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<CompleteRecoveryRequest>,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let issue = state
        .application
        .complete_recovery(
            project_id,
            checklist_id,
            principal.account.id,
            request.expected_version,
            &request.action,
            request.resolution_summary.as_deref(),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn create_approval_request(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<CreateApprovalRequest>,
) -> Result<Json<riichi_persistence::ApprovalRequest>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    match &request.proposed_operation {
        riichi_persistence::ApprovalOperation::SetRank { .. } => {
            require_member(&principal, project_id)?;
        }
        riichi_persistence::ApprovalOperation::ReopenForDispatch { .. }
        | riichi_persistence::ApprovalOperation::CompleteWithSummary { .. } => {
            require_admin(&principal, project_id)?;
        }
    }
    let approval = state
        .application
        .create_approval_request(
            project_id,
            issue_id,
            principal.account.id,
            request.target_version,
            request.proposed_operation,
            Duration::seconds(request.expires_in_seconds.unwrap_or(24 * 60 * 60)),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(approval))
}

pub(super) async fn approve_approval_request(
    State(state): State<AppState>,
    Path((project_id, approval_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::ApprovalRequest>, ApiError> {
    decide_approval_request(state, project_id, approval_id, jar, true).await
}

pub(super) async fn reject_approval_request(
    State(state): State<AppState>,
    Path((project_id, approval_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::ApprovalRequest>, ApiError> {
    decide_approval_request(state, project_id, approval_id, jar, false).await
}

pub(super) async fn decide_approval_request(
    state: AppState,
    project_id: Uuid,
    approval_id: Uuid,
    jar: CookieJar,
    approve: bool,
) -> Result<Json<riichi_persistence::ApprovalRequest>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Admin) {
        return Err(ApiError::Auth(AuthError::InsufficientRole));
    }
    let approval = state
        .application
        .decide_approval_request(project_id, approval_id, principal.account.id, approve)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(approval))
}
