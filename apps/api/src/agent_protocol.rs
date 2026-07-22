use super::*;

pub(super) async fn ready(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ReadyRequest>,
) -> Result<Json<ReadyResponse>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let snapshot = state
        .application
        .ready_snapshot(project_id, session_id, request.limit.unwrap_or(20))
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ReadyResponse {
        issues: snapshot.issues,
        snapshot_cursor: snapshot.snapshot_cursor,
        exclusions: snapshot.exclusions,
    }))
}

pub(super) async fn resolve(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ResolveRequest>,
) -> Result<Json<ResolveResponse>, ApiError> {
    let (project_id, _session_id) = principal(&state, &headers).await?;
    let issue_id = state
        .application
        .database()
        .resolve_issue_key(project_id, &request.display_key)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ResolveResponse { issue_id }))
}

pub(super) async fn claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClaimRequest>,
) -> Result<Json<riichi_persistence::Claim>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let claim = state
        .application
        .claim(
            project_id,
            session_id,
            request.issue_id,
            Duration::seconds(request.requested_ttl_seconds.unwrap_or(1800)),
            request.idempotency_key.trim(),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(claim))
}

pub(super) async fn renew(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RenewRequest>,
) -> Result<Json<RenewResponse>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let expires_at = state
        .application
        .renew(
            project_id,
            session_id,
            request.lease_id,
            request.fencing_token,
            Duration::seconds(request.requested_ttl_seconds.unwrap_or(1800)),
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(RenewResponse { expires_at }))
}

pub(super) async fn report(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ReportRequest>,
) -> Result<StatusCode, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let action = match request.action {
        ReportAction::Release => riichi_persistence::Action::Release,
        ReportAction::Complete => riichi_persistence::Action::Complete,
    };
    state
        .application
        .report(
            project_id,
            session_id,
            request.lease_id,
            request.fencing_token,
            Report {
                action,
                comment: request.comment,
                resolution_summary: request.resolution_summary,
            },
        )
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn report_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ReportBatchRequest>,
) -> Result<Json<riichi_persistence::ReportBatchResult>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let result = state
        .application
        .report_batch(
            project_id,
            session_id,
            request.lease_id,
            request.fencing_token,
            riichi_persistence::ReportBatch {
                idempotency_key: request.idempotency_key,
                operations: request.operations,
            },
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(result))
}

pub(super) async fn context(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ContextRequest>,
) -> Result<Json<riichi_persistence::ContextResponse>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let context = state
        .application
        .context(
            project_id,
            session_id,
            request.issue_id,
            request.max_bytes,
            request.document_frontiers,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(context))
}

pub(super) async fn context_resource(
    State(state): State<AppState>,
    Path((issue_id, resource)): Path<(Uuid, String)>,
    headers: HeaderMap,
) -> Result<Json<riichi_persistence::ContextSection>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let section = state
        .application
        .context_resource(project_id, session_id, issue_id, &resource)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(section))
}

pub(super) async fn read_document(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<AgentDocumentReadResponse>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let result = state
        .application
        .agent_document_read(project_id, session_id, document_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(AgentDocumentReadResponse {
        document: result.document,
        revision: result.version.revision,
        content: result.version.content,
        plain_text: result.version.plain_text,
        sanitized_html: result.version.sanitized_html,
        frontiers: result.frontiers,
    }))
}

pub(super) async fn apply_document_edit(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<AgentDocumentEditRequest>,
) -> Result<Json<AgentDocumentEditResponse>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let result = state
        .application
        .agent_document_apply_insert_text(riichi_application::AgentDocumentInsertText {
            project_id,
            session_id,
            document_id,
            idempotency_key: request.idempotency_key,
            previous_frontiers: request.previous_frontiers,
            node_path: request.node_path,
            offset: request.offset,
            text: request.text,
        })
        .await
        .map_err(ApiError::from)?;
    Ok(Json(AgentDocumentEditResponse {
        update_id: result.update_id,
        document_id: result.document_id,
        source: result.source,
        previous_frontiers: result.previous_frontiers,
        resulting_frontiers: result.resulting_frontiers,
        accepted_at: result.accepted_at,
        replayed: result.replayed,
    }))
}

#[derive(Debug, Deserialize)]
pub(super) struct AgentDocumentEditRequest {
    idempotency_key: String,
    previous_frontiers: Vec<riichi_application::loro_document::LoroFrontier>,
    node_path: Vec<usize>,
    offset: usize,
    text: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ResolveRequest {
    display_key: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ResolveResponse {
    issue_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub(super) struct AgentDocumentReadResponse {
    document: riichi_persistence::Document,
    revision: i64,
    content: Value,
    plain_text: String,
    sanitized_html: String,
    frontiers: Vec<riichi_application::loro_document::LoroFrontier>,
}

#[derive(Debug, Serialize)]
pub(super) struct AgentDocumentEditResponse {
    update_id: Uuid,
    document_id: Uuid,
    source: String,
    previous_frontiers: Vec<riichi_application::loro_document::LoroFrontier>,
    resulting_frontiers: Vec<riichi_application::loro_document::LoroFrontier>,
    accepted_at: chrono::DateTime<chrono::Utc>,
    replayed: bool,
}

pub(super) async fn agent_quarantined_attempts(
    State(state): State<AppState>,
    Path(issue_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<Vec<riichi_persistence::QuarantinedAttempt>>, ApiError> {
    let (project_id, session_id) = principal(&state, &headers).await?;
    let attempts = state
        .application
        .quarantined_attempts_for_agent(project_id, session_id, issue_id)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(attempts))
}
