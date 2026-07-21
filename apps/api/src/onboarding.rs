use super::*;

pub(super) async fn create_onboarding_sample(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::OnboardingSampleRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    state
        .application
        .create_onboarding_sample(project_id, principal.account.id)
        .await
        .map(Json)
        .map_err(ApiError::from)
}
