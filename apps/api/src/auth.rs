use super::*;

pub(super) async fn login(State(state): State<AppState>) -> Result<Redirect, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let authorization_url = auth
        .begin_login(&state.application.database())
        .await
        .map_err(ApiError::from)?;
    Ok(Redirect::to(authorization_url.as_str()))
}

pub(super) async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<(CookieJar, Redirect), ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let state_token = query.state.ok_or(ApiError::Auth(AuthError::InvalidState))?;
    if let Some(error) = query.error {
        auth.cancel_login(&state.application.database(), &state_token)
            .await
            .map_err(ApiError::from)?;
        let message = query
            .error_description
            .map(|description| format!("{error}: {description}"))
            .unwrap_or(error);
        return Err(ApiError::Auth(AuthError::ProviderRejected(message)));
    }
    let code = match query.code {
        Some(code) => code,
        None => {
            auth.cancel_login(&state.application.database(), &state_token)
                .await
                .map_err(ApiError::from)?;
            return Err(ApiError::Auth(AuthError::InvalidState));
        }
    };
    let result = auth
        .finish_login(&state.application.database(), &code, &state_token)
        .await
        .map_err(ApiError::from)?;
    let cookie = Cookie::build((auth.cookie_name(), result.session_token))
        .path("/")
        .http_only(true)
        .secure(auth.cookie_secure())
        .same_site(SameSite::Lax)
        .build();
    Ok((
        CookieJar::new().add(cookie),
        Redirect::to(&result.return_to),
    ))
}

pub(super) async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<CookieJar, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    if let Some(session_token) = jar
        .get(auth.cookie_name())
        .map(|cookie| cookie.value().to_owned())
    {
        auth.logout(&state.application.database(), &session_token)
            .await
            .map_err(ApiError::from)?;
    }
    let removal_cookie = Cookie::build((auth.cookie_name(), ""))
        .path("/")
        .http_only(true)
        .secure(auth.cookie_secure())
        .same_site(SameSite::Lax)
        .removal()
        .build();
    Ok(jar.remove(removal_cookie))
}

pub(super) async fn human_me(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<HumanMeResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    Ok(Json(HumanMeResponse {
        account_id: principal.account.id,
        email: principal.account.email,
        display_name: principal.account.display_name,
        avatar_url: state
            .application
            .database()
            .human_avatar(principal.account.id)
            .await
            .map_err(ApiError::from)?
            .map(|_| "/api/v1/auth/me/avatar".to_owned()),
        memberships: principal
            .memberships
            .into_iter()
            .map(|membership| HumanMembershipResponse {
                project_id: membership.project_id,
                project_name: membership.project_name,
                role: membership.role,
            })
            .collect(),
        teams: principal
            .team_memberships
            .into_iter()
            .map(|membership| HumanTeamMembershipResponse {
                team_id: membership.team_id,
                team_name: membership.team_name,
                team_key: membership.team_key,
                role: membership.role,
            })
            .collect(),
    }))
}

pub(super) async fn human_avatar(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<impl IntoResponse, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let Some((bytes, content_type)) = state
        .application
        .database()
        .human_avatar(principal.account.id)
        .await
        .map_err(ApiError::from)?
    else {
        return Err(ApiError::NotFound);
    };
    Ok(([(header::CONTENT_TYPE, content_type)], bytes))
}

pub(super) async fn upload_human_avatar(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: Multipart,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let mut content_type = None;
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| ApiError::InvalidRequest)?
    {
        if field.name() != Some("avatar") {
            continue;
        }
        let field_type = field
            .content_type()
            .ok_or(ApiError::InvalidRequest)?
            .to_owned();
        if !matches!(
            field_type.as_str(),
            "image/jpeg" | "image/png" | "image/webp" | "image/gif"
        ) {
            return Err(ApiError::InvalidRequest);
        }
        let field_bytes = field.bytes().await.map_err(|_| ApiError::InvalidRequest)?;
        if field_bytes.len() > 2 * 1024 * 1024 {
            return Err(ApiError::InvalidRequest);
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
        .set_human_avatar(principal.account.id, &content_type, &bytes)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn delete_human_avatar(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    state
        .application
        .database()
        .clear_human_avatar(principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}
