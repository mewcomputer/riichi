use super::*;

pub(super) async fn create_project(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let principal = human_principal(&state, &jar).await?;
    let project_id = auth
        .create_project(&state.application.database(), &principal, &request.name)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CreateProjectResponse { project_id }))
}

pub(super) async fn create_invite(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateInviteRequest>,
) -> Result<Json<CreateInviteResponse>, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let principal = human_principal(&state, &jar).await?;
    let invite = auth
        .create_invite(
            &state.application.database(),
            &principal,
            project_id,
            &request.role,
            request.email_hint.as_deref(),
            Duration::seconds(request.expires_in_seconds.unwrap_or(7 * 24 * 60 * 60)),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CreateInviteResponse {
        invite_id: invite.id,
        project_id: invite.project_id,
        role: invite.role,
        email_hint: invite.email_hint,
        token: invite.token,
        expires_at: invite.expires_at,
    }))
}

pub(super) async fn revoke_invite(
    State(state): State<AppState>,
    Path((project_id, invite_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let principal = human_principal(&state, &jar).await?;
    auth.revoke_invite(
        &state.application.database(),
        &principal,
        project_id,
        invite_id,
    )
    .await
    .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn redrive_outbox(
    State(state): State<AppState>,
    Path((project_id, message_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let principal = human_principal(&state, &jar).await?;
    let redriven = auth
        .redrive_outbox(
            &state.application.database(),
            &principal,
            project_id,
            message_id,
        )
        .await
        .map_err(ApiError::from)?;
    if !redriven {
        return Err(ApiError::OutboxNotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn accept_invite(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(request): Json<AcceptInviteRequest>,
) -> Result<Json<AcceptInviteResponse>, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let principal = human_principal(&state, &jar).await?;
    let accepted = auth
        .accept_invite(&state.application.database(), &principal, &request.token)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(AcceptInviteResponse {
        project_id: accepted.project_id,
        role: accepted.role,
    }))
}
