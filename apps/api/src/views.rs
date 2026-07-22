use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct SaveViewRequest {
    name: String,
    filters: Value,
}

#[derive(Debug, Deserialize)]
pub(super) struct PinViewRequest {
    pinned: bool,
}

pub(super) async fn list_saved_views(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::SavedViewRecord>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let views = state
        .application
        .database()
        .list_saved_views(principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(views))
}

pub(super) async fn save_view(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(request): Json<SaveViewRequest>,
) -> Result<Json<riichi_persistence::SavedViewRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.contains(['\n', '\r']) {
        return Err(ApiError::InvalidRequest);
    }
    if !request.filters.is_object() {
        return Err(ApiError::InvalidRequest);
    }
    let view = state
        .application
        .database()
        .save_view(principal.account.id, name, request.filters)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(view))
}

pub(super) async fn delete_saved_view(
    State(state): State<AppState>,
    Path(view_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let deleted = state
        .application
        .database()
        .delete_saved_view(principal.account.id, view_id)
        .await
        .map_err(ApiError::from)?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn list_project_saved_views(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::SavedViewRecord>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let views = state
        .application
        .database()
        .list_project_saved_views(project_id, principal.account.id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(views))
}

pub(super) async fn save_project_view(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<SaveViewRequest>,
) -> Result<Json<riichi_persistence::SavedViewRecord>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.contains(['\n', '\r']) {
        return Err(ApiError::InvalidRequest);
    }
    if !request.filters.is_object() {
        return Err(ApiError::InvalidRequest);
    }
    let view = state
        .application
        .database()
        .save_project_view(project_id, principal.account.id, name, request.filters)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(view))
}

pub(super) async fn delete_project_saved_view(
    State(state): State<AppState>,
    Path((project_id, view_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let can_manage = principal.can_access_project(project_id, riichi_auth::HumanRole::Admin);
    let deleted = state
        .application
        .database()
        .delete_project_saved_view(project_id, view_id, principal.account.id, can_manage)
        .await
        .map_err(ApiError::from)?;
    if !deleted {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn pin_saved_view(
    State(state): State<AppState>,
    Path(view_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<PinViewRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let changed = state
        .application
        .database()
        .set_personal_saved_view_pinned(principal.account.id, view_id, request.pinned)
        .await
        .map_err(ApiError::from)?;
    if !changed && request.pinned {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn pin_project_saved_view(
    State(state): State<AppState>,
    Path((project_id, view_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<PinViewRequest>,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let changed = state
        .application
        .database()
        .set_project_saved_view_pinned(project_id, principal.account.id, view_id, request.pinned)
        .await
        .map_err(ApiError::from)?;
    if !changed && request.pinned {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
