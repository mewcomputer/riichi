use super::*;

#[derive(Debug, Deserialize)]
pub(super) struct SaveViewRequest {
    name: String,
    filters: Value,
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
