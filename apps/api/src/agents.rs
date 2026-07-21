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
) -> Result<Json<CreateAgentRoleResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let role_id = state
        .application
        .create_agent_role_with_id(
            project_id,
            &request.display_name,
            request.owner_account_id.unwrap_or(principal.account.id),
            request.capabilities,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CreateAgentRoleResponse { role_id }))
}

pub(super) async fn create_agent_session(
    State(state): State<AppState>,
    Path((project_id, role_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<CreateAgentSessionRequest>,
) -> Result<Json<CreateAgentSessionResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let lifetime_seconds = request
        .lifetime_seconds
        .unwrap_or(30 * 60)
        .clamp(60, 24 * 60 * 60);
    let agent_token = format!("{}{}", Uuid::now_v7().simple(), Uuid::now_v7().simple());
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(lifetime_seconds);
    let session_id = state
        .application
        .create_agent_session(
            project_id,
            role_id,
            chrono::Duration::seconds(lifetime_seconds),
            &agent_token,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CreateAgentSessionResponse {
        session_id,
        agent_token,
        expires_at,
    }))
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
