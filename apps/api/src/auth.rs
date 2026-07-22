use super::*;
use axum::response::{Html, IntoResponse, Response};

pub(super) async fn login(
    State(state): State<AppState>,
    Query(query): Query<CliLoginQuery>,
) -> Result<Redirect, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let return_to = query
        .return_to
        .as_deref()
        .filter(|value| value.starts_with('/') && !value.starts_with("//"))
        .unwrap_or("/");
    let authorization_url = auth
        .begin_login_to(&state.application.database(), return_to)
        .await
        .map_err(ApiError::from)?;
    Ok(Redirect::to(authorization_url.as_str()))
}

pub(super) async fn create_cli_login(
    State(state): State<AppState>,
) -> Result<Json<CliLoginStartResponse>, ApiError> {
    state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let token = Uuid::new_v4().to_string();
    state
        .application
        .database()
        .create_cli_login_handoff(&token, chrono::Duration::minutes(10))
        .await
        .map_err(ApiError::from)?;
    Ok(Json(CliLoginStartResponse {
        token: token.clone(),
        login_url: format!("/auth/cli-login/{token}"),
    }))
}

pub(super) async fn complete_cli_login(
    State(state): State<AppState>,
    Path(token): Path<String>,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    if jar.get(auth.cookie_name()).is_none() {
        return Ok(
            Redirect::to(&format!("/auth/login?return_to=/auth/cli-login/{token}")).into_response(),
        );
    }
    let principal = human_principal(&state, &jar).await?;
    if !state
        .application
        .database()
        .complete_cli_login_handoff(&token, principal.account.id)
        .await
        .map_err(ApiError::from)?
    {
        return Err(ApiError::NotFound);
    }
    Ok(Html("<h1>riichi login complete</h1><p>return to your terminal.</p>").into_response())
}

pub(super) async fn exchange_cli_login(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> Result<(StatusCode, Json<CliLoginExchangeResponse>), ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let Some(account_id) = state
        .application
        .database()
        .claim_cli_login_handoff(&token)
        .await
        .map_err(ApiError::from)?
    else {
        return Ok((
            StatusCode::ACCEPTED,
            Json(CliLoginExchangeResponse {
                status: "pending".to_owned(),
                session_token: None,
            }),
        ));
    };
    let result = match auth
        .issue_session(&state.application.database(), account_id)
        .await
    {
        Ok(result) => result,
        Err(error) => {
            state
                .application
                .database()
                .release_cli_login_handoff(&token)
                .await
                .map_err(ApiError::from)?;
            return Err(ApiError::from(error));
        }
    };
    Ok((
        StatusCode::OK,
        Json(CliLoginExchangeResponse {
            status: "complete".to_owned(),
            session_token: Some(result.session_token),
        }),
    ))
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
        last_completed_nux_version: principal.account.last_completed_nux_version,
        last_completed_nux_at: principal.account.last_completed_nux_at,
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

#[derive(Debug, Deserialize)]
pub(super) struct CliLoginQuery {
    return_to: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct CliLoginStartResponse {
    token: String,
    login_url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct CliLoginExchangeResponse {
    status: String,
    session_token: Option<String>,
}

pub(super) async fn complete_nux(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(request): Json<CompleteNuxRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    if request.version.trim().is_empty() || request.version.chars().count() > 100 {
        return Err(ApiError::InvalidRequest);
    }
    state
        .application
        .database()
        .complete_nux(principal.account.id, request.version.trim())
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::HumanUnauthenticated)?;
    Ok(StatusCode::NO_CONTENT)
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
