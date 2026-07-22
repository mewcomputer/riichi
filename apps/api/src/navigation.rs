use super::*;

pub(super) async fn navigation(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<NavigationResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let rows = state
        .application
        .database()
        .human_navigation(principal.account.id)
        .await
        .map_err(ApiError::from)?;

    let mut organizations: Vec<NavigationOrganizationResponse> = Vec::new();
    for row in rows {
        let organization = match organizations.last_mut() {
            Some(organization) if organization.id == row.organization_id => organization,
            _ => {
                organizations.push(NavigationOrganizationResponse {
                    id: row.organization_id,
                    name: row.organization_name.clone(),
                    role: row.organization_role.clone(),
                    logo_url: row
                        .organization_has_logo
                        .then(|| format!("/api/v1/organizations/{}/logo", row.organization_id)),
                    teams: Vec::new(),
                });
                organizations
                    .last_mut()
                    .expect("organization was just inserted")
            }
        };
        let team = match organization.teams.last_mut() {
            Some(team) if team.id == row.team_id => team,
            _ => {
                organization.teams.push(NavigationTeamResponse {
                    id: row.team_id,
                    name: row.team_name.clone(),
                    key: row.team_key.clone(),
                    emoji: row.team_emoji.clone(),
                    projects: Vec::new(),
                    views: Vec::new(),
                });
                organization
                    .teams
                    .last_mut()
                    .expect("team was just inserted")
            }
        };
        team.projects.push(NavigationProjectResponse {
            id: row.project_id,
            name: row.project_name,
            icon: row.project_icon,
            role: row.project_role,
        });
    }

    Ok(Json(NavigationResponse { organizations }))
}

pub(super) async fn update_project(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<UpdateProjectRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let role = state
        .application
        .database()
        .human_project_role(principal.account.id, project_id)
        .await
        .map_err(ApiError::from)?;
    if !matches!(role.as_deref(), Some("owner" | "admin")) {
        return Err(ApiError::ProjectActionDenied);
    }
    state
        .application
        .database()
        .update_project_icon(project_id, request.icon.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn require_organization_admin(
    state: &AppState,
    principal: &HumanPrincipal,
    organization_id: Uuid,
) -> Result<(), ApiError> {
    let role = state
        .application
        .database()
        .organization_membership_role(principal.account.id, organization_id)
        .await
        .map_err(ApiError::from)?;
    if !matches!(role.as_deref(), Some("admin" | "owner")) {
        return Err(ApiError::ProjectActionDenied);
    }
    Ok(())
}

pub(super) async fn organization_logo(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let role = state
        .application
        .database()
        .organization_membership_role(principal.account.id, organization_id)
        .await
        .map_err(ApiError::from)?;
    if role.is_none() {
        return Err(ApiError::ProjectAccessDenied);
    }
    let Some((bytes, content_type)) = state
        .application
        .database()
        .organization_logo(organization_id)
        .await
        .map_err(ApiError::from)?
    else {
        return Err(ApiError::NotFound);
    };
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'".to_owned(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    ))
}

pub(super) async fn upload_organization_logo(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_organization_admin(&state, &principal, organization_id).await?;
    let mut content_type = None;
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::InvalidRequest)?
    {
        if field.name() != Some("logo") {
            continue;
        }
        let field_type = field
            .content_type()
            .ok_or(ApiError::InvalidRequest)?
            .to_owned();
        if !matches!(field_type.as_str(), "image/png" | "image/svg+xml") {
            return Err(ApiError::InvalidRequest);
        }
        let field_bytes = field.bytes().await.map_err(|_| ApiError::InvalidRequest)?;
        if field_bytes.len() > 2 * 1024 * 1024 {
            return Err(ApiError::InvalidRequest);
        }
        if field_type == "image/svg+xml" {
            let svg = std::str::from_utf8(&field_bytes).map_err(|_| ApiError::InvalidRequest)?;
            let normalized = svg.to_ascii_lowercase();
            if normalized.contains("<script")
                || normalized.contains("javascript:")
                || normalized.contains("onload=")
                || normalized.contains("onerror=")
            {
                return Err(ApiError::InvalidRequest);
            }
        }
        content_type = Some(field_type);
        bytes = Some(field_bytes);
        break;
    }
    let (Some(content_type), Some(bytes)) = (content_type, bytes) else {
        return Err(ApiError::InvalidRequest);
    };
    state
        .application
        .database()
        .set_organization_logo(organization_id, &content_type, &bytes)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn delete_organization_logo(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_organization_admin(&state, &principal, organization_id).await?;
    state
        .application
        .database()
        .clear_organization_logo(organization_id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn human_all_issues(
    State(state): State<AppState>,
    Query(query): Query<HumanIssueQuery>,
    jar: CookieJar,
) -> Result<Json<HumanQueueResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let issues = state
        .application
        .database()
        .human_all_issues(principal.account.id, None, query.limit.unwrap_or(200))
        .await
        .map_err(ApiError::from)?;
    Ok(Json(HumanQueueResponse { issues }))
}

pub(super) async fn human_get_issue(
    State(state): State<AppState>,
    Path(issue_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let issue = state
        .application
        .database()
        .human_get_issue(principal.account.id, issue_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

pub(super) async fn human_team_issues(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Query(query): Query<HumanIssueQuery>,
    jar: CookieJar,
) -> Result<Json<HumanQueueResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_viewer(&principal, team_id)?;
    let issues = state
        .application
        .database()
        .human_all_issues(
            principal.account.id,
            Some(team_id),
            query.limit.unwrap_or(200),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(HumanQueueResponse { issues }))
}

pub(super) async fn update_team(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<UpdateTeamRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_admin(&principal, team_id)?;
    state
        .application
        .database()
        .update_team_emoji(team_id, request.emoji.as_deref())
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn human_pending_approvals(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::GlobalApprovalRequest>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let approvals = state
        .application
        .database()
        .human_pending_approvals(principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(approvals))
}

pub(super) async fn human_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
    jar: CookieJar,
) -> Result<Json<InboxResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let database = state.application.database();
    let notifications = database
        .notifications_for_account(
            principal.account.id,
            query.unread_only.unwrap_or(false),
            query.limit.unwrap_or(50),
        )
        .await
        .map_err(ApiError::from)?;
    let unread_count = database
        .unread_notification_count(principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(InboxResponse {
        notifications,
        unread_count,
    }))
}

pub(super) async fn human_inbox_unread_count(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<UnreadCountResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let unread_count = state
        .application
        .database()
        .unread_notification_count(principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(UnreadCountResponse { unread_count }))
}

pub(super) async fn mark_inbox_notification_read(
    State(state): State<AppState>,
    Path(notification_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let found = state
        .application
        .database()
        .mark_notification_read(principal.account.id, notification_id)
        .await
        .map_err(ApiError::from)?;
    if !found {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
