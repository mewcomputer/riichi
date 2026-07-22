use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{ApiError, AppState, human_principal, require_member, require_viewer};

#[derive(Debug, Deserialize)]
pub(super) struct WorkflowAliasesRequest {
    aliases: Vec<WorkflowAliasInput>,
}

#[derive(Debug, Deserialize)]
struct WorkflowAliasInput {
    label: String,
    canonical_status: String,
}

pub(super) async fn list_workflow_aliases(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::WorkflowAlias>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    Ok(Json(
        state
            .application
            .database()
            .current_workflow_aliases(project_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn save_workflow_aliases(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<WorkflowAliasesRequest>,
) -> Result<Json<Vec<riichi_persistence::WorkflowAlias>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    if request.aliases.is_empty() || request.aliases.len() > 50 {
        return Err(ApiError::InvalidRequest);
    }
    let mut aliases = Vec::with_capacity(request.aliases.len());
    let mut labels = HashSet::new();
    for alias in request.aliases {
        let label = alias.label.trim().to_owned();
        if label.is_empty()
            || label.chars().count() > 80
            || ![
                "triage",
                "todo",
                "in_progress",
                "blocked",
                "done",
                "canceled",
            ]
            .contains(&alias.canonical_status.as_str())
        {
            return Err(ApiError::InvalidRequest);
        }
        if !labels.insert(label.clone()) {
            return Err(ApiError::InvalidRequest);
        }
        aliases.push((label, alias.canonical_status));
    }
    Ok(Json(
        state
            .application
            .database()
            .save_workflow_aliases(project_id, principal.account.id, &aliases)
            .await
            .map_err(ApiError::from)?,
    ))
}

#[derive(Debug, Deserialize)]
pub(super) struct TemplateRequest {
    name: String,
    snapshot: Value,
}

pub(super) async fn list_templates(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::IssueTemplate>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    Ok(Json(
        state
            .application
            .database()
            .list_issue_templates(project_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn create_template(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<TemplateRequest>,
) -> Result<Json<riichi_persistence::IssueTemplate>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 100 || !request.snapshot.is_object() {
        return Err(ApiError::InvalidRequest);
    }
    Ok(Json(
        state
            .application
            .database()
            .create_issue_template(project_id, principal.account.id, name, request.snapshot)
            .await
            .map_err(ApiError::from)?,
    ))
}

#[derive(Debug, Deserialize, Default)]
struct TemplateIssueSnapshot {
    title: Option<String>,
    body: Option<String>,
    status: Option<String>,
    agent_eligible: Option<bool>,
    spec_complete: Option<bool>,
    labels: Option<Vec<String>>,
    parent_issue_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InstantiateTemplateRequest {
    title: Option<String>,
}

pub(super) async fn instantiate_template(
    State(state): State<AppState>,
    Path((project_id, template_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<InstantiateTemplateRequest>,
) -> Result<Json<riichi_persistence::IssueRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let template = state
        .application
        .database()
        .get_issue_template(project_id, template_id)
        .await
        .map_err(ApiError::from)?;
    let snapshot: TemplateIssueSnapshot =
        serde_json::from_value(template.snapshot).map_err(|_| ApiError::InvalidRequest)?;
    let title = request
        .title
        .or(snapshot.title)
        .ok_or(ApiError::InvalidRequest)?;
    let issue = state
        .application
        .create_issue(
            project_id,
            riichi_persistence::IssueCreate {
                id: Uuid::now_v7(),
                display_key: String::new(),
                title,
                body: snapshot.body.unwrap_or_default(),
                status: snapshot.status.unwrap_or_else(|| "todo".to_owned()),
                agent_eligible: snapshot.agent_eligible.unwrap_or(false),
                spec_complete: snapshot.spec_complete.unwrap_or(false),
                rank: 0,
                labels: snapshot.labels.unwrap_or_default(),
                assignee_account_id: None,
                parent_issue_id: snapshot.parent_issue_id,
            },
            principal.account.id,
        )
        .await
        .map_err(ApiError::from)?;
    state
        .application
        .database()
        .record_template_instance(issue.id, template.id, template.version)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(issue))
}

#[derive(Debug, Deserialize)]
pub(super) struct SubscriptionRequest {
    issue_id: Option<Uuid>,
    kind: String,
    enabled: bool,
}

pub(super) async fn list_subscriptions(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::IssueSubscription>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    Ok(Json(
        state
            .application
            .database()
            .list_issue_subscriptions(principal.account.id, project_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn set_subscription(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<SubscriptionRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    if ![
        "approval",
        "lease_expiry",
        "blocked_dependency",
        "quarantine",
    ]
    .contains(&request.kind.as_str())
    {
        return Err(ApiError::InvalidRequest);
    }
    state
        .application
        .database()
        .set_issue_subscription(
            principal.account.id,
            project_id,
            request.issue_id,
            &request.kind,
            request.enabled,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
