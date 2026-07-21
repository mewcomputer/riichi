mod support;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Body,
    extract::{Form, State},
    http::{HeaderValue, Method, Request, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use chrono::{Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use http_body_util::BodyExt;
use openidconnect::core::{CoreHmacKey, CoreIdToken, CoreIdTokenClaims, CoreJwsSigningAlgorithm};
use openidconnect::{
    AccessToken, Audience, AuthorizationCode, EmptyAdditionalClaims, EndUserEmail, IssuerUrl,
    Nonce, StandardClaims, SubjectIdentifier,
};
use riichi_application::Application;
use riichi_auth::{AuthService, OidcConfig};
use riichi_storage::ObjectAttachmentStore;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use tokio::net::TcpListener;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};
use tower::ServiceExt;
use url::Url;
use uuid::Uuid;

#[derive(Clone)]
struct ProviderState {
    issuer: String,
    tokens: Arc<Mutex<HashMap<String, TokenFixture>>>,
}

#[derive(Clone)]
struct TokenFixture {
    code_verifier: String,
    id_token: String,
}

async fn discovery(State(state): State<ProviderState>) -> Json<Value> {
    Json(json!({
        "issuer": state.issuer,
        "authorization_endpoint": format!("{}/authorize", state.issuer),
        "token_endpoint": format!("{}/token", state.issuer),
        "jwks_uri": format!("{}/jwks", state.issuer),
        "response_types_supported": ["code"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["HS256"],
        "scopes_supported": ["openid", "profile", "email"]
    }))
}

async fn jwks() -> Json<Value> {
    Json(json!({
        "keys": [{
            "kty": "oct",
            "k": "c2VjcmV0",
            "alg": "HS256",
            "use": "sig"
        }]
    }))
}

async fn token(
    State(state): State<ProviderState>,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let code = form.get("code").cloned().unwrap_or_default();
    let code_verifier = form.get("code_verifier").cloned().unwrap_or_default();
    let fixture = state
        .tokens
        .lock()
        .unwrap()
        .get(&code)
        .filter(|fixture| fixture.code_verifier == code_verifier)
        .cloned();
    match fixture {
        Some(fixture) => (
            StatusCode::OK,
            Json(json!({
                "access_token": "access-token",
                "token_type": "Bearer",
                "expires_in": 300,
                "id_token": fixture.id_token
            })),
        )
            .into_response(),
        None => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response(),
    }
}

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

fn request(method: Method, uri: &str, body: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(axum::body::Body::from(body.to_owned()))
        .unwrap()
}

fn cookie_from_response(response: &axum::response::Response) -> HeaderValue {
    let set_cookie = response.headers().get(header::SET_COOKIE).unwrap();
    let cookie = set_cookie.to_str().unwrap().split(';').next().unwrap();
    HeaderValue::from_str(cookie).unwrap()
}

fn binary_loro_update(update_id: Uuid, payload: &[u8]) -> Message {
    let envelope = json!({
        "type": "update",
        "update_id": update_id,
        "idempotency_key": update_id,
    })
    .to_string();
    let mut frame = envelope.into_bytes();
    frame.push(b'\n');
    frame.extend_from_slice(payload);
    Message::Binary(frame.into())
}

fn binary_message_type(message: Message) -> String {
    let Message::Binary(bytes) = message else {
        panic!("expected binary sync message");
    };
    let separator = bytes.iter().position(|byte| *byte == b'\n').unwrap();
    serde_json::from_slice::<Value>(&bytes[..separator]).unwrap()["type"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn login_url(app: &Router) -> Url {
    let request = Request::builder()
        .method(Method::GET)
        .uri("/auth/login")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert!(response.status().is_redirection());
    Url::parse(
        response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap()
}

fn signed_id_token(issuer: &str, subject: &str, email: &str, nonce: String, code: &str) -> String {
    let access_token = AccessToken::new("access-token".to_owned());
    let authorization_code = AuthorizationCode::new(code.to_owned());
    let now = Utc::now();
    CoreIdToken::new(
        CoreIdTokenClaims::new(
            IssuerUrl::new(issuer.to_owned()).unwrap(),
            vec![Audience::new("riichi".to_owned())],
            now + Duration::minutes(5),
            now,
            StandardClaims::new(SubjectIdentifier::new(subject.to_owned()))
                .set_email(Some(EndUserEmail::new(email.to_owned())))
                .set_email_verified(Some(true)),
            EmptyAdditionalClaims {},
        )
        .set_nonce(Some(Nonce::new(nonce))),
        &CoreHmacKey::new(b"secret".to_vec()),
        CoreJwsSigningAlgorithm::HmacSha256,
        Some(&access_token),
        Some(&authorization_code),
    )
    .unwrap()
    .to_string()
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn configured_api_supports_oidc_cookie_project_and_invite_round_trip() {
    let database = database().await;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}");
    let provider_state = ProviderState {
        issuer: issuer.clone(),
        tokens: Arc::new(Mutex::new(HashMap::new())),
    };
    let provider_app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .with_state(provider_state.clone());
    let provider = tokio::spawn(async move {
        axum::serve(listener, provider_app).await.unwrap();
    });
    let auth = AuthService::discover(OidcConfig {
        issuer_url: issuer.clone(),
        client_id: "riichi".to_owned(),
        client_secret: "secret".to_owned(),
        redirect_url: format!("{issuer}/auth/callback"),
        cookie_secure: false,
        session_days: 7,
        login_state_minutes: 10,
    })
    .await
    .unwrap();
    let attachment_root =
        std::env::temp_dir().join(format!("riichi-api-attachments-{}", Uuid::now_v7()));
    let attachment_store = ObjectAttachmentStore::local(&attachment_root).unwrap();
    let app = riichi_api::app_with_auth_and_attachment_store(
        Application::new(database.database.clone()),
        auth.clone(),
        attachment_store.clone(),
    );

    let unauthenticated_me = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/auth/me")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated_me.status(), StatusCode::UNAUTHORIZED);

    let first_login = login_url(&app).await;
    let first_state = first_login
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    let (first_nonce, first_verifier): (String, String) =
        sqlx::query_as("SELECT nonce, pkce_verifier FROM oidc_login_states WHERE state_hash = $1")
            .bind(hash(&first_state))
            .fetch_one(database.pool())
            .await
            .unwrap();
    provider_state.tokens.lock().unwrap().insert(
        "first-code".to_owned(),
        TokenFixture {
            code_verifier: first_verifier,
            id_token: signed_id_token(
                &issuer,
                "subject-1",
                "first@example.test",
                first_nonce,
                "first-code",
            ),
        },
    );
    let callback = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/callback?code=first-code&state={first_state}"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(callback.status().is_redirection());
    let cookie_header = callback.headers().get(header::SET_COOKIE).unwrap();
    assert!(cookie_header.to_str().unwrap().contains("HttpOnly"));
    assert!(cookie_header.to_str().unwrap().contains("SameSite=Lax"));
    assert!(!cookie_header.to_str().unwrap().contains("Secure"));
    let first_cookie = cookie_from_response(&callback);

    let mut me = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/auth/me")
        .body(axum::body::Body::empty())
        .unwrap();
    me.headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let me_response = app.clone().oneshot(me).await.unwrap();
    assert_eq!(me_response.status(), StatusCode::OK);
    let me_body: Value =
        serde_json::from_slice(&me_response.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(me_body["email"], "first@example.test");
    assert!(me_body["memberships"].as_array().unwrap().is_empty());

    let mut create_project = request(Method::POST, "/api/v1/projects", r#"{"name":"pilot"}"#);
    create_project
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let project_response = app.clone().oneshot(create_project).await.unwrap();
    assert_eq!(project_response.status(), StatusCode::OK);
    let project_body: Value = serde_json::from_slice(
        &project_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let project_id = project_body["project_id"].as_str().unwrap();

    let mut onboarding = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/onboarding-sample"),
        "",
    );
    onboarding
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let onboarding_response = app.clone().oneshot(onboarding).await.unwrap();
    assert_eq!(onboarding_response.status(), StatusCode::OK);
    let onboarding_body: Value = serde_json::from_slice(
        &onboarding_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    for field in [
        "role_id",
        "session_id",
        "triage_issue_id",
        "agent_issue_id",
        "recovery_issue_id",
        "approval_id",
        "recovery_checklist_id",
    ] {
        assert!(onboarding_body[field].as_str().is_some(), "missing {field}");
    }
    let onboarding_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM onboarding_samples WHERE project_id = $1")
            .bind(Uuid::parse_str(project_id).unwrap())
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(onboarding_count, 1);
    let mut repeat_onboarding = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/onboarding-sample"),
        "",
    );
    repeat_onboarding
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let repeat_response = app.clone().oneshot(repeat_onboarding).await.unwrap();
    assert_eq!(repeat_response.status(), StatusCode::OK);
    let repeat_body: Value = serde_json::from_slice(
        &repeat_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(repeat_body, onboarding_body);

    let redrive_message_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO outbox_messages (id, project_id, message_type, payload,
         dead_lettered_at, last_error)
         VALUES ($1, $2, 'future_delivery', $3, now(), 'test poison message')",
    )
    .bind(redrive_message_id)
    .bind(Uuid::parse_str(project_id).unwrap())
    .bind(json!({"event": "sent"}))
    .execute(database.pool())
    .await
    .unwrap();
    let mut redrive = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/api/v1/projects/{project_id}/outbox/{redrive_message_id}/redrive"
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    redrive
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let redrive_response = app.clone().oneshot(redrive).await.unwrap();
    assert_eq!(redrive_response.status(), StatusCode::NO_CONTENT);
    let (dead_lettered, attempt_count): (Option<chrono::DateTime<Utc>>, i32) =
        sqlx::query_as("SELECT dead_lettered_at, attempt_count FROM outbox_messages WHERE id = $1")
            .bind(redrive_message_id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert!(dead_lettered.is_none());
    assert_eq!(attempt_count, 0);
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM audit_records
         WHERE project_id = $1 AND operation = 'outbox_redrive' AND target_id = $2",
    )
    .bind(Uuid::parse_str(project_id).unwrap())
    .bind(redrive_message_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(audit_count, 1);

    let mut create_invite = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/invites"),
        r#"{"role":"member","email_hint":"second@example.test"}"#,
    );
    create_invite
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let invite_response = app.clone().oneshot(create_invite).await.unwrap();
    assert_eq!(invite_response.status(), StatusCode::OK);
    let invite_body: Value = serde_json::from_slice(
        &invite_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let invite_token = invite_body["token"].as_str().unwrap().to_owned();

    let second_login = login_url(&app).await;
    let second_state = second_login
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    let (second_nonce, second_verifier): (String, String) =
        sqlx::query_as("SELECT nonce, pkce_verifier FROM oidc_login_states WHERE state_hash = $1")
            .bind(hash(&second_state))
            .fetch_one(database.pool())
            .await
            .unwrap();
    provider_state.tokens.lock().unwrap().insert(
        "second-code".to_owned(),
        TokenFixture {
            code_verifier: second_verifier,
            id_token: signed_id_token(
                &issuer,
                "subject-2",
                "second@example.test",
                second_nonce,
                "second-code",
            ),
        },
    );
    let second_callback = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!(
                    "/auth/callback?code=second-code&state={second_state}"
                ))
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let second_cookie = cookie_from_response(&second_callback);

    let mut unauthorized_invite = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/invites"),
        r#"{"role":"viewer"}"#,
    );
    unauthorized_invite
        .headers_mut()
        .insert(header::COOKIE, second_cookie.clone());
    let unauthorized_response = app.clone().oneshot(unauthorized_invite).await.unwrap();
    assert_eq!(unauthorized_response.status(), StatusCode::FORBIDDEN);

    let mut accept = request(
        Method::POST,
        "/api/v1/invites/accept",
        &json!({"token": invite_token.clone()}).to_string(),
    );
    accept
        .headers_mut()
        .insert(header::COOKIE, second_cookie.clone());
    let accepted = app.clone().oneshot(accept).await.unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);

    let mut replay = request(
        Method::POST,
        "/api/v1/invites/accept",
        &json!({"token": invite_token}).to_string(),
    );
    replay
        .headers_mut()
        .insert(header::COOKIE, second_cookie.clone());
    let replay_response = app.clone().oneshot(replay).await.unwrap();
    assert_eq!(replay_response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let mut second_me = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/auth/me")
        .body(axum::body::Body::empty())
        .unwrap();
    second_me
        .headers_mut()
        .insert(header::COOKIE, second_cookie.clone());
    let second_me_response = app.clone().oneshot(second_me).await.unwrap();
    let second_me_body: Value = serde_json::from_slice(
        &second_me_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(second_me_body["memberships"][0]["role"], "member");
    assert_eq!(second_me_body["memberships"][0]["project_id"], project_id);

    let mut create_document = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/documents"),
        &json!({
            "title": "Loro bridge",
            "schema_version": 1,
            "content": {
                "type": "doc",
                "content": [{
                    "type": "paragraph",
                    "content": [{"type": "text", "text": "snapshot me"}]
                }]
            }
        })
        .to_string(),
    );
    create_document
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let document_response = app.clone().oneshot(create_document).await.unwrap();
    assert_eq!(document_response.status(), StatusCode::OK);
    let document_body: Value = serde_json::from_slice(
        &document_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let document_id = document_body["id"].as_str().unwrap();

    let mut create_default_document = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/documents"),
        &json!({"title": "Default v2 document"}).to_string(),
    );
    create_default_document
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let default_document_response = app.clone().oneshot(create_default_document).await.unwrap();
    assert_eq!(default_document_response.status(), StatusCode::OK);
    let default_document_body: Value = serde_json::from_slice(
        &default_document_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let default_document_id = default_document_body["id"].as_str().unwrap();
    let mut default_snapshot = request(
        Method::GET,
        &format!("/api/v1/documents/{default_document_id}/loro-snapshot"),
        "",
    );
    default_snapshot
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let default_snapshot_response = app.clone().oneshot(default_snapshot).await.unwrap();
    assert_eq!(default_snapshot_response.status(), StatusCode::OK);
    assert_eq!(
        default_snapshot_response
            .headers()
            .get("x-riichi-document-schema-version")
            .unwrap(),
        "2"
    );

    let attachment_bytes = b"api attachment";
    let attachment_checksum = Sha256::digest(attachment_bytes);
    let attachment_checksum_hex = attachment_checksum
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let mut create_attachment = request(
        Method::POST,
        &format!("/api/v1/documents/{document_id}/attachments"),
        &json!({
            "filename": "api.txt",
            "media_type": "text/plain",
            "byte_size": attachment_bytes.len(),
            "checksum": attachment_checksum_hex,
            "source_block_id": "api-attachment-block"
        })
        .to_string(),
    );
    create_attachment
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let create_attachment_response = app.clone().oneshot(create_attachment).await.unwrap();
    assert_eq!(create_attachment_response.status(), StatusCode::OK);
    let upload_body: Value = serde_json::from_slice(
        &create_attachment_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let upload_id = upload_body["upload_id"].as_str().unwrap();
    let attachment_id = upload_body["attachment_id"].as_str().unwrap();

    let mut put_attachment = Request::builder()
        .method(Method::PUT)
        .uri(format!("/api/v1/attachment-uploads/{upload_id}"))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(attachment_bytes.to_vec()))
        .unwrap();
    put_attachment
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let put_attachment_response = app.clone().oneshot(put_attachment).await.unwrap();
    assert_eq!(put_attachment_response.status(), StatusCode::NO_CONTENT);

    let mut complete_attachment = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1/attachment-uploads/{upload_id}/complete"))
        .body(Body::empty())
        .unwrap();
    complete_attachment
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let complete_attachment_response = app.clone().oneshot(complete_attachment).await.unwrap();
    assert_eq!(complete_attachment_response.status(), StatusCode::OK);

    let mut download_attachment = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/attachments/{attachment_id}"))
        .body(Body::empty())
        .unwrap();
    download_attachment
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let download_attachment_response = app.clone().oneshot(download_attachment).await.unwrap();
    assert_eq!(download_attachment_response.status(), StatusCode::OK);
    assert_eq!(
        download_attachment_response
            .headers()
            .get(header::CONTENT_TYPE)
            .unwrap(),
        "text/plain"
    );
    let downloaded_attachment = download_attachment_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(downloaded_attachment.as_ref(), attachment_bytes);

    let unauthenticated_attachment = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/v1/attachments/{attachment_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        unauthenticated_attachment.status(),
        StatusCode::UNAUTHORIZED
    );
    let _ = std::fs::remove_dir_all(attachment_root);

    let mut loro_snapshot = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/v1/documents/{document_id}/loro-snapshot"))
        .body(axum::body::Body::empty())
        .unwrap();
    loro_snapshot
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let snapshot_response = app.clone().oneshot(loro_snapshot).await.unwrap();
    assert_eq!(snapshot_response.status(), StatusCode::OK);
    assert_eq!(
        snapshot_response.headers().get("content-type").unwrap(),
        "application/octet-stream"
    );
    assert_eq!(
        snapshot_response
            .headers()
            .get("x-riichi-document-revision")
            .unwrap(),
        "1"
    );
    assert_eq!(
        snapshot_response
            .headers()
            .get("x-riichi-document-schema-version")
            .unwrap(),
        "1"
    );
    let frontiers: Value = serde_json::from_str(
        snapshot_response
            .headers()
            .get("x-riichi-document-frontiers")
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    assert!(frontiers.is_array());
    let snapshot_bytes = snapshot_response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert!(!snapshot_bytes.is_empty());
    let previous_frontiers = frontiers
        .as_array()
        .unwrap()
        .iter()
        .map(|frontier| {
            json!({
                "peer_id": frontier["peer"],
                "counter": frontier["counter"],
            })
        })
        .collect::<Vec<_>>();
    let update_id = Uuid::now_v7();
    let apply_body = json!({
        "schema_version": 1,
        "update_id": update_id,
        "idempotency_key": "snapshot-bridge-1",
        "previous_frontiers": previous_frontiers.clone(),
        "payload_base64": BASE64.encode(&snapshot_bytes),
    });
    let mut apply_update = request(
        Method::POST,
        &format!("/api/v1/documents/{document_id}/loro-updates"),
        &apply_body.to_string(),
    );
    apply_update
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let apply_response = app.clone().oneshot(apply_update).await.unwrap();
    assert_eq!(apply_response.status(), StatusCode::OK);
    let apply_result: Value = serde_json::from_slice(
        &apply_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(apply_result["replayed"], false);
    let projected_revision: i64 = sqlx::query_scalar(
        "SELECT content_revision FROM document_projections WHERE document_id = $1",
    )
    .bind(Uuid::parse_str(document_id).unwrap())
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(projected_revision, 2);

    let incompatible_apply_body = json!({
        "schema_version": 2,
        "update_id": Uuid::now_v7(),
        "idempotency_key": "schema-mismatch-1",
        "previous_frontiers": previous_frontiers,
        "payload_base64": BASE64.encode(&snapshot_bytes),
    });
    let mut incompatible_apply = request(
        Method::POST,
        &format!("/api/v1/documents/{document_id}/loro-updates"),
        &incompatible_apply_body.to_string(),
    );
    incompatible_apply
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let incompatible_response = app.clone().oneshot(incompatible_apply).await.unwrap();
    assert_eq!(
        incompatible_response.status(),
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let incompatible_result: Value = serde_json::from_slice(
        &incompatible_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(incompatible_result["code"], "invalid_document");

    let mut replay_update = request(
        Method::POST,
        &format!("/api/v1/documents/{document_id}/loro-updates"),
        &apply_body.to_string(),
    );
    replay_update
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let replay_response = app.clone().oneshot(replay_update).await.unwrap();
    assert_eq!(replay_response.status(), StatusCode::OK);
    let replay_result: Value = serde_json::from_slice(
        &replay_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(replay_result["replayed"], true);

    let api_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let api_address = api_listener.local_addr().unwrap();
    let api_app = app.clone();
    let api_server = tokio::spawn(async move {
        axum::serve(api_listener, api_app).await.unwrap();
    });
    let ws_url = format!("ws://{api_address}/api/v1/documents/{document_id}/loro-sync");
    let mut first_ws_request = ws_url.clone().into_client_request().unwrap();
    first_ws_request
        .headers_mut()
        .insert("Cookie", first_cookie.to_str().unwrap().parse().unwrap());
    let (mut first_ws, _) = connect_async(first_ws_request).await.unwrap();
    first_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "101", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let first_snapshot = first_ws.next().await.unwrap().unwrap();
    assert_eq!(binary_message_type(first_snapshot), "snapshot");

    let mut second_ws_request = ws_url.clone().into_client_request().unwrap();
    second_ws_request
        .headers_mut()
        .insert("Cookie", first_cookie.to_str().unwrap().parse().unwrap());
    let (mut second_ws, _) = connect_async(second_ws_request).await.unwrap();
    second_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "102", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let second_snapshot = second_ws.next().await.unwrap().unwrap();
    assert_eq!(binary_message_type(second_snapshot), "snapshot");

    let transport_update_id = Uuid::now_v7();
    first_ws
        .send(binary_loro_update(transport_update_id, &snapshot_bytes))
        .await
        .unwrap();
    let accepted = first_ws.next().await.unwrap().unwrap();
    assert!(matches!(accepted, Message::Text(text) if text.contains("accepted")));
    let fanout = second_ws.next().await.unwrap().unwrap();
    assert_eq!(binary_message_type(fanout), "update");

    let mut missing_peer_ws_request = ws_url.clone().into_client_request().unwrap();
    missing_peer_ws_request
        .headers_mut()
        .insert("Cookie", first_cookie.to_str().unwrap().parse().unwrap());
    let (mut missing_peer_ws, _) = connect_async(missing_peer_ws_request).await.unwrap();
    missing_peer_ws
        .send(Message::Text(
            json!({"type": "hello", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let missing_peer_error = missing_peer_ws.next().await.unwrap().unwrap();
    assert!(
        matches!(missing_peer_error, Message::Text(text) if text.contains("peer ID is required"))
    );
    missing_peer_ws.close(None).await.unwrap();

    let mut duplicate_ws_request = ws_url.clone().into_client_request().unwrap();
    duplicate_ws_request
        .headers_mut()
        .insert("Cookie", first_cookie.to_str().unwrap().parse().unwrap());
    let (mut duplicate_ws, _) = connect_async(duplicate_ws_request).await.unwrap();
    duplicate_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "101", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let duplicate_error = duplicate_ws.next().await.unwrap().unwrap();
    assert!(matches!(duplicate_error, Message::Text(text) if text.contains("already active")));
    duplicate_ws.close(None).await.unwrap();

    let mut incompatible_ws_request = ws_url.clone().into_client_request().unwrap();
    incompatible_ws_request
        .headers_mut()
        .insert("Cookie", first_cookie.to_str().unwrap().parse().unwrap());
    let (mut incompatible_ws, _) = connect_async(incompatible_ws_request).await.unwrap();
    incompatible_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "103", "schema_version": 2})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let incompatible_error = incompatible_ws.next().await.unwrap().unwrap();
    assert!(matches!(
        incompatible_error,
        Message::Text(text) if text.contains("incompatible with server version 1")
    ));
    incompatible_ws.close(None).await.unwrap();

    let restarted_app = riichi_api::app_with_auth_and_attachment_store(
        Application::new(database.database.clone()),
        auth.clone(),
        attachment_store,
    );
    let restarted_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let restarted_address = restarted_listener.local_addr().unwrap();
    let restarted_server = tokio::spawn(async move {
        axum::serve(restarted_listener, restarted_app)
            .await
            .unwrap();
    });
    let restarted_ws_url =
        format!("ws://{restarted_address}/api/v1/documents/{document_id}/loro-sync");
    let mut restarted_ws_request = restarted_ws_url.into_client_request().unwrap();
    restarted_ws_request
        .headers_mut()
        .insert("Cookie", first_cookie.to_str().unwrap().parse().unwrap());
    let (mut restarted_ws, _) = connect_async(restarted_ws_request).await.unwrap();
    restarted_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "101", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let restarted_snapshot = restarted_ws.next().await.unwrap().unwrap();
    assert_eq!(binary_message_type(restarted_snapshot), "snapshot");
    restarted_ws.close(None).await.unwrap();
    restarted_server.abort();

    let mut second_invite = request(
        Method::POST,
        &format!("/api/v1/projects/{project_id}/invites"),
        r#"{"role":"viewer"}"#,
    );
    second_invite
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let second_invite_response = app.clone().oneshot(second_invite).await.unwrap();
    assert_eq!(second_invite_response.status(), StatusCode::OK);
    let second_invite_body: Value = serde_json::from_slice(
        &second_invite_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    let second_invite_id = second_invite_body["invite_id"].as_str().unwrap();
    let second_invite_token = second_invite_body["token"].as_str().unwrap();
    let mut revoke = Request::builder()
        .method(Method::POST)
        .uri(format!(
            "/api/v1/projects/{project_id}/invites/{second_invite_id}/revoke"
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    revoke
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let revoke_response = app.clone().oneshot(revoke).await.unwrap();
    assert_eq!(revoke_response.status(), StatusCode::NO_CONTENT);
    let mut accept_revoked = request(
        Method::POST,
        "/api/v1/invites/accept",
        &json!({"token": second_invite_token}).to_string(),
    );
    accept_revoked
        .headers_mut()
        .insert(header::COOKIE, second_cookie.clone());
    let revoked_response = app.clone().oneshot(accept_revoked).await.unwrap();
    assert_eq!(revoked_response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let first_account_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM human_accounts WHERE issuer = $1 AND subject = 'subject-1'",
    )
    .bind(&issuer)
    .fetch_one(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE project_memberships SET revoked_at = now()
         WHERE project_id = $1 AND account_id = $2",
    )
    .bind(Uuid::parse_str(project_id).unwrap())
    .bind(first_account_id)
    .execute(database.pool())
    .await
    .unwrap();
    let permission_error = tokio::time::timeout(std::time::Duration::from_secs(2), first_ws.next())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(matches!(permission_error, Message::Text(text) if text.contains("access was revoked")));
    first_ws.close(None).await.unwrap();
    second_ws.close(None).await.unwrap();
    api_server.abort();

    let second_account_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM human_accounts WHERE issuer = $1 AND subject = 'subject-2'",
    )
    .bind(&issuer)
    .fetch_one(database.pool())
    .await
    .unwrap();
    Application::new(database.database.clone())
        .migrate_document_to_v2(second_account_id, Uuid::parse_str(document_id).unwrap())
        .await
        .unwrap();
    let (current_schema, current_revision): (i32, i64) = sqlx::query_as(
        "SELECT schema_version, source_revision
         FROM document_loro_snapshots WHERE document_id = $1",
    )
    .bind(Uuid::parse_str(document_id).unwrap())
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(current_schema, 2);
    assert_eq!(current_revision, 4);
    let (history_schema, history_revision, history_frontiers, history_count): (
        i32,
        i64,
        Value,
        i64,
    ) = sqlx::query_as(
        "SELECT schema_version, source_revision, frontiers, count(*) OVER ()
         FROM document_loro_snapshot_history WHERE document_id = $1",
    )
    .bind(Uuid::parse_str(document_id).unwrap())
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!((history_schema, history_revision, history_count), (1, 3, 1));
    let version_frontiers: Value = sqlx::query_scalar(
        "SELECT frontiers FROM document_versions WHERE document_id = $1 AND revision = $2",
    )
    .bind(Uuid::parse_str(document_id).unwrap())
    .bind(history_revision)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(history_frontiers, version_frontiers);
    let binding_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM document_bindings WHERE document_id = $1")
            .bind(Uuid::parse_str(document_id).unwrap())
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(binding_count, 1);

    let migrated_app = app.clone();
    let migrated_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let migrated_address = migrated_listener.local_addr().unwrap();
    let migrated_server = tokio::spawn(async move {
        axum::serve(migrated_listener, migrated_app).await.unwrap();
    });
    let migrated_ws_url =
        format!("ws://{migrated_address}/api/v1/documents/{document_id}/loro-sync");
    let mut migrated_v1_request = migrated_ws_url.clone().into_client_request().unwrap();
    migrated_v1_request
        .headers_mut()
        .insert("Cookie", second_cookie.to_str().unwrap().parse().unwrap());
    let (mut migrated_v1_ws, _) = connect_async(migrated_v1_request).await.unwrap();
    migrated_v1_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "201", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let migrated_v1_error = migrated_v1_ws.next().await.unwrap().unwrap();
    assert!(matches!(
        migrated_v1_error,
        Message::Text(text) if text.contains("incompatible with server version 2")
    ));
    migrated_v1_ws.close(None).await.unwrap();

    let mut migrated_v2_request = migrated_ws_url.into_client_request().unwrap();
    migrated_v2_request
        .headers_mut()
        .insert("Cookie", second_cookie.to_str().unwrap().parse().unwrap());
    let (mut migrated_v2_ws, _) = connect_async(migrated_v2_request).await.unwrap();
    migrated_v2_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "202", "schema_version": 2})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let migrated_v2_snapshot = migrated_v2_ws.next().await.unwrap().unwrap();
    assert_eq!(binary_message_type(migrated_v2_snapshot), "snapshot");
    migrated_v2_ws.close(None).await.unwrap();

    let rolled_back = Application::new(database.database.clone())
        .rollback_document_to_v1(second_account_id, Uuid::parse_str(document_id).unwrap())
        .await
        .unwrap();
    assert_eq!(rolled_back.schema_version, 1);
    let (rollback_schema, rollback_revision): (i32, i64) = sqlx::query_as(
        "SELECT schema_version, source_revision
         FROM document_loro_snapshots WHERE document_id = $1",
    )
    .bind(Uuid::parse_str(document_id).unwrap())
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!((rollback_schema, rollback_revision), (1, 5));
    let (rollback_history_schema, rollback_reason): (i32, String) = sqlx::query_as(
        "SELECT schema_version, reason
         FROM document_loro_snapshot_history
         WHERE document_id = $1 ORDER BY archived_at DESC, id DESC LIMIT 1",
    )
    .bind(Uuid::parse_str(document_id).unwrap())
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(rollback_history_schema, 2);
    assert_eq!(rollback_reason, "schema_rollback");

    let mut rollback_v1_request =
        format!("ws://{migrated_address}/api/v1/documents/{document_id}/loro-sync")
            .into_client_request()
            .unwrap();
    rollback_v1_request
        .headers_mut()
        .insert("Cookie", second_cookie.to_str().unwrap().parse().unwrap());
    let (mut rollback_v1_ws, _) = connect_async(rollback_v1_request).await.unwrap();
    rollback_v1_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "203", "schema_version": 1})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let rollback_v1_snapshot = rollback_v1_ws.next().await.unwrap().unwrap();
    assert_eq!(binary_message_type(rollback_v1_snapshot), "snapshot");
    rollback_v1_ws.close(None).await.unwrap();

    let mut rollback_v2_request =
        format!("ws://{migrated_address}/api/v1/documents/{document_id}/loro-sync")
            .into_client_request()
            .unwrap();
    rollback_v2_request
        .headers_mut()
        .insert("Cookie", second_cookie.to_str().unwrap().parse().unwrap());
    let (mut rollback_v2_ws, _) = connect_async(rollback_v2_request).await.unwrap();
    rollback_v2_ws
        .send(Message::Text(
            json!({"type": "hello", "peer_id": "204", "schema_version": 2})
                .to_string()
                .into(),
        ))
        .await
        .unwrap();
    let rollback_v2_error = rollback_v2_ws.next().await.unwrap().unwrap();
    assert!(matches!(
        rollback_v2_error,
        Message::Text(text) if text.contains("incompatible with server version 1")
    ));
    rollback_v2_ws.close(None).await.unwrap();
    migrated_server.abort();

    let mut logout = Request::builder()
        .method(Method::POST)
        .uri("/auth/logout")
        .body(axum::body::Body::empty())
        .unwrap();
    logout
        .headers_mut()
        .insert(header::COOKIE, first_cookie.clone());
    let logout_response = app.clone().oneshot(logout).await.unwrap();
    assert_eq!(logout_response.status(), StatusCode::OK);
    let mut logged_out_me = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/auth/me")
        .body(axum::body::Body::empty())
        .unwrap();
    logged_out_me
        .headers_mut()
        .insert(header::COOKIE, first_cookie);
    let logged_out_response = app.oneshot(logged_out_me).await.unwrap();
    assert_eq!(logged_out_response.status(), StatusCode::UNAUTHORIZED);

    provider.abort();
}
