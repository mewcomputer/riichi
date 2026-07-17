use super::*;

pub(super) async fn agent_roster(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<AgentRosterResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let (roles, sessions) = state
        .application
        .agent_roster(project_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(AgentRosterResponse { roles, sessions }))
}

pub(super) async fn team_agent_roster(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<AgentRosterResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_viewer(&principal, team_id)?;
    let (roles, sessions) = state
        .application
        .team_agent_roster(team_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(AgentRosterResponse { roles, sessions }))
}

pub(super) async fn create_agent_role(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateAgentRoleRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    state
        .application
        .create_agent_role(
            project_id,
            &request.display_name,
            request.owner_account_id.unwrap_or(principal.account.id),
            request.capabilities,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn revoke_agent_session(
    State(state): State<AppState>,
    Path((project_id, session_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    state
        .application
        .revoke_agent_session(project_id, session_id, principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn revoke_agent_role(
    State(state): State<AppState>,
    Path((project_id, role_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    state
        .application
        .revoke_agent_role(project_id, role_id, principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) fn require_viewer(principal: &HumanPrincipal, project_id: Uuid) -> Result<(), ApiError> {
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Viewer) {
        return Err(ApiError::ProjectAccessDenied);
    }
    Ok(())
}

pub(super) fn require_team_viewer(
    principal: &HumanPrincipal,
    team_id: Uuid,
) -> Result<(), ApiError> {
    if !principal.can_access_team(team_id, riichi_auth::HumanRole::Viewer) {
        return Err(ApiError::ProjectAccessDenied);
    }
    Ok(())
}

pub(super) fn require_team_member(
    principal: &HumanPrincipal,
    team_id: Uuid,
) -> Result<(), ApiError> {
    if !principal.can_access_team(team_id, riichi_auth::HumanRole::Member) {
        return Err(ApiError::ProjectActionDenied);
    }
    Ok(())
}

pub(super) fn require_team_admin(
    principal: &HumanPrincipal,
    team_id: Uuid,
) -> Result<(), ApiError> {
    if !principal.can_access_team(team_id, riichi_auth::HumanRole::Admin) {
        return Err(ApiError::ProjectActionDenied);
    }
    Ok(())
}

pub(super) fn require_member(principal: &HumanPrincipal, project_id: Uuid) -> Result<(), ApiError> {
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Member) {
        return Err(ApiError::ProjectActionDenied);
    }
    Ok(())
}

pub(super) fn require_admin(principal: &HumanPrincipal, project_id: Uuid) -> Result<(), ApiError> {
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Admin) {
        return Err(ApiError::ProjectActionDenied);
    }
    Ok(())
}
