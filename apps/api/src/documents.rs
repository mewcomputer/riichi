use super::*;
use axum::body::Bytes;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use riichi_storage::{AttachmentStore, ObjectAttachmentStore, StorageError};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize)]
pub(super) struct CreateDocumentRequest {
    title: String,
    parent_document_id: Option<Uuid>,
    #[serde(default)]
    position: i64,
    #[serde(default)]
    schema_version: Option<i32>,
    content: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateDocumentContentRequest {
    expected_revision: i64,
    content: Value,
    #[serde(default)]
    references: Vec<DocumentReferenceRequest>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ReplaceDocumentReferencesRequest {
    #[serde(default)]
    references: Vec<DocumentReferenceRequest>,
}

#[derive(Debug, Deserialize)]
pub(super) struct UpdateDocumentMetadataRequest {
    title: String,
    parent_document_id: Option<Uuid>,
    #[serde(default)]
    position: i64,
}

#[derive(Debug, Deserialize)]
pub(super) struct DocumentReferenceRequest {
    source_block_id: String,
    resource_kind: String,
    resource_id: Uuid,
    reference_kind: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct CreateAttachmentUploadRequest {
    filename: String,
    media_type: String,
    byte_size: i64,
    checksum: String,
    source_block_id: String,
}

#[derive(Debug, Serialize)]
pub(super) struct AttachmentUploadResponse {
    upload_id: Uuid,
    attachment_id: Uuid,
    expires_at: chrono::DateTime<chrono::Utc>,
    upload_url: String,
}

pub(super) async fn create_organization_document(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateDocumentRequest>,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_organization_member(&state, &principal, organization_id).await?;
    create_document(
        &state,
        principal.account.id,
        organization_id,
        "standalone_page",
        request,
        None,
        None,
    )
    .await
}

pub(super) async fn create_team_document(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateDocumentRequest>,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_member(&principal, team_id)?;
    let organization_id = state
        .application
        .database()
        .team_organization_id(team_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound)?;
    create_document(
        &state,
        principal.account.id,
        organization_id,
        "team_page",
        request,
        Some(team_id),
        None,
    )
    .await
}

pub(super) async fn create_project_document(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateDocumentRequest>,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    let organization_id = state
        .application
        .database()
        .project_organization_id(project_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound)?;
    create_document(
        &state,
        principal.account.id,
        organization_id,
        "project_page",
        request,
        None,
        Some(project_id),
    )
    .await
}

pub(super) async fn list_organization_documents(
    State(state): State<AppState>,
    Path(organization_id): Path<Uuid>,
    Query(query): Query<DocumentChildrenQuery>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::Document>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_organization_member(&state, &principal, organization_id).await?;
    Ok(Json(
        state
            .application
            .list_document_children(
                principal.account.id,
                query.parent_document_id,
                organization_id,
                None,
                None,
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn list_team_documents(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    Query(query): Query<DocumentChildrenQuery>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::Document>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_viewer(&principal, team_id)?;
    let organization_id = state
        .application
        .database()
        .team_organization_id(team_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(
        state
            .application
            .list_document_children(
                principal.account.id,
                query.parent_document_id,
                organization_id,
                Some(team_id),
                None,
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn list_project_documents(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    Query(query): Query<DocumentChildrenQuery>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::Document>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let organization_id = state
        .application
        .database()
        .project_organization_id(project_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::NotFound)?;
    Ok(Json(
        state
            .application
            .list_document_children(
                principal.account.id,
                query.parent_document_id,
                organization_id,
                None,
                Some(project_id),
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

#[derive(Debug, Deserialize)]
pub(super) struct DocumentChildrenQuery {
    parent_document_id: Option<Uuid>,
}

async fn create_document(
    state: &AppState,
    account_id: Uuid,
    organization_id: Uuid,
    kind: &str,
    request: CreateDocumentRequest,
    owner_team_id: Option<Uuid>,
    owner_project_id: Option<Uuid>,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let content = request.content.unwrap_or_else(empty_tiptap_document);
    let plain_text = riichi_application::tiptap_plain_text(&content);
    let sanitized_html =
        riichi_application::tiptap_sanitized_html(&content).map_err(ApiError::from)?;
    let document = state
        .application
        .create_document(riichi_persistence::DocumentCreate {
            id: Uuid::now_v7(),
            organization_id,
            kind: kind.to_owned(),
            title: request.title,
            parent_document_id: request.parent_document_id,
            position: request.position,
            owner_team_id,
            owner_project_id,
            created_by: account_id,
            content,
            plain_text,
            sanitized_html,
            schema_version: request
                .schema_version
                .unwrap_or(riichi_application::loro_document::CURRENT_DOCUMENT_SCHEMA_VERSION),
        })
        .await
        .map_err(ApiError::from)?;
    Ok(Json(document))
}

pub(super) async fn get_document(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    Ok(Json(
        state
            .application
            .get_document(principal.account.id, document_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn get_issue_description_document(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    Ok(Json(
        state
            .application
            .get_issue_description_document(principal.account.id, project_id, issue_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn update_document_metadata(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<UpdateDocumentMetadataRequest>,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    Ok(Json(
        state
            .application
            .update_document_metadata(
                principal.account.id,
                document_id,
                request.title,
                request.parent_document_id,
                request.position,
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn delete_document(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    state
        .application
        .delete_document(principal.account.id, document_id)
        .await
        .map_err(ApiError::from)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn get_document_version(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<DocumentVersionQuery>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::DocumentVersion>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    Ok(Json(
        state
            .application
            .get_document_version(principal.account.id, document_id, query.revision)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn get_document_loro_snapshot(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    Query(query): Query<DocumentVersionQuery>,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let snapshot = state
        .application
        .get_loro_snapshot(principal.account.id, document_id, query.revision)
        .await
        .map_err(ApiError::from)?;
    let frontiers = serde_json::to_string(
        &snapshot
            .frontiers
            .iter()
            .map(|frontier| {
                json!({
                    "peer": frontier.peer_id.to_string(),
                    "counter": frontier.counter,
                })
            })
            .collect::<Vec<_>>(),
    )
    .map_err(|_| ApiError::InvalidRequest)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/octet-stream"),
    );
    for (name, value) in [
        ("x-riichi-document-revision", snapshot.revision.to_string()),
        (
            "x-riichi-document-schema-version",
            snapshot.schema_version.to_string(),
        ),
        ("x-riichi-document-frontiers", frontiers),
    ] {
        headers.insert(
            header::HeaderName::from_static(name),
            header::HeaderValue::from_str(&value).map_err(|_| ApiError::InvalidRequest)?,
        );
    }
    Ok((headers, snapshot.bytes).into_response())
}

#[derive(Debug, Deserialize)]
pub(super) struct ApplyLoroUpdateRequest {
    #[serde(default)]
    schema_version: Option<i32>,
    update_id: Uuid,
    idempotency_key: Option<String>,
    previous_frontiers: Vec<LoroFrontierRequest>,
    payload_base64: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct LoroFrontierRequest {
    peer_id: String,
    counter: i32,
}

#[derive(Debug, Serialize)]
pub(super) struct LoroFrontierResponse {
    peer_id: String,
    counter: i32,
}

#[derive(Debug, Serialize)]
pub(super) struct ApplyLoroUpdateResponse {
    update_id: Uuid,
    document_id: Uuid,
    source: String,
    previous_frontiers: Vec<LoroFrontierResponse>,
    resulting_frontiers: Vec<LoroFrontierResponse>,
    accepted_at: chrono::DateTime<chrono::Utc>,
    replayed: bool,
}

pub(super) async fn apply_document_loro_update(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<ApplyLoroUpdateRequest>,
) -> Result<Json<ApplyLoroUpdateResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let payload = BASE64
        .decode(request.payload_base64)
        .map_err(|_| ApiError::InvalidRequest)?;
    let previous_frontiers = request
        .previous_frontiers
        .into_iter()
        .map(|frontier| {
            Ok(riichi_application::loro_document::LoroFrontier {
                peer_id: frontier
                    .peer_id
                    .parse()
                    .map_err(|_| ApiError::InvalidRequest)?,
                counter: frontier.counter,
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    let result = state
        .application
        .accept_loro_update(
            principal.account.id,
            document_id,
            riichi_application::loro_document::LoroUpdateCommand {
                schema_version: request
                    .schema_version
                    .unwrap_or(riichi_application::loro_document::CURRENT_DOCUMENT_SCHEMA_VERSION),
                update_id: request.update_id,
                idempotency_key: request.idempotency_key,
                previous_frontiers,
                payload,
                source: "human".to_owned(),
            },
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(ApplyLoroUpdateResponse {
        update_id: result.update_id,
        document_id: result.document_id,
        source: result.source,
        previous_frontiers: result
            .previous_frontiers
            .into_iter()
            .map(frontier_response)
            .collect(),
        resulting_frontiers: result
            .resulting_frontiers
            .into_iter()
            .map(frontier_response)
            .collect(),
        accepted_at: result.accepted_at,
        replayed: result.replayed,
    }))
}

fn frontier_response(
    frontier: riichi_application::loro_document::LoroFrontier,
) -> LoroFrontierResponse {
    LoroFrontierResponse {
        peer_id: frontier.peer_id.to_string(),
        counter: frontier.counter,
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct DocumentVersionQuery {
    revision: Option<i64>,
}

pub(super) async fn update_document_content(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<UpdateDocumentContentRequest>,
) -> Result<Json<riichi_persistence::Document>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let plain_text = riichi_application::tiptap_plain_text(&request.content);
    let sanitized_html =
        riichi_application::tiptap_sanitized_html(&request.content).map_err(ApiError::from)?;
    let references = request
        .references
        .into_iter()
        .map(|reference| riichi_persistence::DocumentReferenceInput {
            source_block_id: reference.source_block_id,
            resource_kind: reference.resource_kind,
            resource_id: reference.resource_id,
            reference_kind: reference.reference_kind,
        })
        .collect();
    Ok(Json(
        state
            .application
            .update_document_content(
                principal.account.id,
                document_id,
                riichi_persistence::DocumentContentUpdate {
                    expected_revision: request.expected_revision,
                    content: request.content,
                    plain_text,
                    sanitized_html,
                    references,
                },
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn get_document_references(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::DocumentReference>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    Ok(Json(
        state
            .application
            .document_references(principal.account.id, document_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn replace_document_references(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<ReplaceDocumentReferencesRequest>,
) -> Result<Json<Vec<riichi_persistence::DocumentReference>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let references = request
        .references
        .into_iter()
        .map(|reference| riichi_persistence::DocumentReferenceInput {
            source_block_id: reference.source_block_id,
            resource_kind: reference.resource_kind,
            resource_id: reference.resource_id,
            reference_kind: reference.reference_kind,
        })
        .collect::<Vec<_>>();
    Ok(Json(
        state
            .application
            .replace_document_references(principal.account.id, document_id, &references)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn get_document_backlinks(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Vec<riichi_persistence::DocumentReference>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    Ok(Json(
        state
            .application
            .document_backlinks(principal.account.id, document_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn create_attachment_upload(
    State(state): State<AppState>,
    Path(document_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<CreateAttachmentUploadRequest>,
) -> Result<Json<AttachmentUploadResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let document = state
        .application
        .get_document(principal.account.id, document_id)
        .await
        .map_err(ApiError::from)?;
    if request.byte_size < 0 || request.byte_size > 50 * 1024 * 1024 {
        return Err(ApiError::InvalidRequest);
    }
    if request.filename.trim().is_empty()
        || request.filename.len() > 255
        || request.filename.contains(['/', '\\', '\0'])
        || request.media_type.trim().is_empty()
    {
        return Err(ApiError::InvalidRequest);
    }
    let checksum = decode_checksum(&request.checksum).ok_or(ApiError::InvalidRequest)?;
    let upload_id = Uuid::now_v7();
    let attachment_id = Uuid::now_v7();
    let upload = state
        .application
        .create_attachment_upload(riichi_persistence::AttachmentUploadSeed {
            id: upload_id,
            attachment_id,
            organization_id: document.organization_id,
            storage_key: format!("uploads/{upload_id}.bin"),
            filename: request.filename,
            media_type: request.media_type,
            byte_size: request.byte_size,
            checksum,
            uploaded_by: principal.account.id,
            document_id,
            source_block_id: request.source_block_id,
            lifetime: Duration::hours(1),
        })
        .await
        .map_err(ApiError::from)?;
    Ok(Json(AttachmentUploadResponse {
        upload_id,
        attachment_id,
        expires_at: upload.expires_at,
        upload_url: format!("/api/v1/attachment-uploads/{upload_id}"),
    }))
}

pub(super) async fn put_attachment_upload(
    State(state): State<AppState>,
    Path(upload_id): Path<Uuid>,
    jar: CookieJar,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    state
        .application
        .authorize_attachment_upload(principal.account.id, upload_id)
        .await
        .map_err(ApiError::from)?;
    if body.len() > 50 * 1024 * 1024 {
        return Err(ApiError::InvalidRequest);
    }
    state
        .attachment_store
        .put(&upload_storage_key(upload_id), body)
        .await
        .map_err(|_| ApiError::NotReady)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn complete_attachment_upload(
    State(state): State<AppState>,
    Path(upload_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<riichi_persistence::Attachment>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    state
        .application
        .authorize_attachment_upload(principal.account.id, upload_id)
        .await
        .map_err(ApiError::from)?;
    let bytes = state
        .attachment_store
        .get(&upload_storage_key(upload_id))
        .await
        .map_err(map_attachment_read_error)?;
    let checksum = Sha256::digest(&bytes);
    let attachment = state
        .application
        .complete_attachment_upload(
            principal.account.id,
            upload_id,
            bytes.len() as i64,
            &checksum,
        )
        .await
        .map_err(ApiError::from)?;
    Ok(Json(attachment))
}

pub(super) async fn get_attachment(
    State(state): State<AppState>,
    Path(attachment_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    let attachment = state
        .application
        .get_attachment(principal.account.id, attachment_id)
        .await
        .map_err(ApiError::from)?;
    let bytes = state
        .attachment_store
        .get(&attachment.storage_key)
        .await
        .map_err(map_attachment_read_error)?;
    Ok((
        [
            (header::CONTENT_TYPE, attachment.media_type),
            (
                header::CONTENT_DISPOSITION,
                format!(
                    "inline; filename=\"{}\"",
                    safe_download_filename(&attachment.filename)
                ),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

async fn require_organization_member(
    state: &AppState,
    principal: &HumanPrincipal,
    organization_id: Uuid,
) -> Result<(), ApiError> {
    let role = state
        .application
        .database()
        .organization_membership_role(principal.account.id, organization_id)
        .await
        .map_err(ApiError::from)?;
    if role.is_some() {
        Ok(())
    } else {
        Err(ApiError::ProjectAccessDenied)
    }
}

fn empty_tiptap_document() -> Value {
    json!({"type": "doc", "content": []})
}

fn decode_checksum(value: &str) -> Option<Vec<u8>> {
    if value.len() != 64 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

fn upload_storage_key(upload_id: Uuid) -> String {
    format!("uploads/{upload_id}.bin")
}

fn map_attachment_read_error(error: StorageError) -> ApiError {
    if ObjectAttachmentStore::is_not_found(&error) {
        ApiError::NotFound
    } else {
        ApiError::NotReady
    }
}

fn safe_download_filename(filename: &str) -> String {
    filename
        .replace(['"', '\\', '\r', '\n'], "_")
        .trim()
        .to_owned()
}
