use axum::{
    extract::{Path, State},
    response::Json,
};
use axum_extra::extract::cookie::CookieJar;
use serde::Serialize;
use uuid::Uuid;

use crate::{ApiError, AppState, human_principal, require_viewer};

#[derive(Debug, Serialize)]
pub(super) struct ProjectOverviewResponse {
    pub summary: riichi_persistence::ProjectOverviewSummary,
    pub issues: Vec<riichi_persistence::ProjectOverviewIssue>,
    pub recent_changes: Vec<riichi_persistence::ProjectOverviewChange>,
}

pub(super) async fn project_overview(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<ProjectOverviewResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let (summary, issues, recent_changes) = state
        .application
        .database()
        .project_overview(project_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ProjectOverviewResponse {
        summary,
        issues,
        recent_changes,
    }))
}
