mod support;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderValue, Method, Request, StatusCode, Uri, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::Duration;
use futures_util::StreamExt;
use http_body_util::BodyExt;
use riichi_application::Application;
use riichi_auth::{AuthService, OidcConfig};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use tokio::net::TcpListener;
use tower::ServiceExt;
use uuid::Uuid;

async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

async fn discovery(State(issuer): State<String>) -> Json<Value> {
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "jwks_uri": format!("{issuer}/jwks"),
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

#[derive(Clone, Default)]
struct ElectricState {
    queries: Arc<Mutex<Vec<String>>>,
}

async fn electric_shape(State(state): State<ElectricState>, uri: Uri) -> Response {
    state
        .queries
        .lock()
        .unwrap()
        .push(uri.query().unwrap_or_default().to_owned());
    if uri
        .query()
        .is_some_and(|query| query.contains("cache-buster=hold"))
    {
        let body = Body::from_stream(async_stream::stream! {
            yield Ok::<_, std::convert::Infallible>("[]".to_owned());
            std::future::pending::<()>().await;
        });
        let mut response = Response::new(body);
        *response.status_mut() = StatusCode::OK;
        response.headers_mut().insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        return response;
    }
    let mut response = (StatusCode::OK, "[]").into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    response
}

#[tokio::test]
#[ignore = "starts disposable PostgreSQL and Electric-compatible HTTP containers"]
async fn electric_project_shape_closes_after_membership_revocation() {
    let database = database().await;
    let electric_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let electric_address = electric_listener.local_addr().unwrap();
    let electric_app = Router::new()
        .route("/v1/shape", get(electric_shape))
        .with_state(ElectricState::default());
    let electric_task = tokio::spawn(async move {
        axum::serve(electric_listener, electric_app).await.unwrap();
    });

    let provider_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let issuer = format!("http://{provider_address}");
    let provider = tokio::spawn({
        let issuer = issuer.clone();
        async move {
            axum::serve(
                provider_listener,
                Router::new()
                    .route("/.well-known/openid-configuration", get(discovery))
                    .route("/jwks", get(jwks))
                    .with_state(issuer),
            )
            .await
            .unwrap();
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
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

    let account_id = database
        .upsert_human_account(
            &issuer,
            "electric-revocation-user",
            Some("electric-revocation@example.test"),
            Some("Electric Revocation User"),
        )
        .await
        .unwrap();
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "Electric revocation project")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, account_id, "owner")
        .await
        .unwrap();
    let session_token = "electric-revocation-session";
    database
        .create_human_session(
            Uuid::now_v7(),
            account_id,
            &hash(session_token),
            Duration::hours(1),
        )
        .await
        .unwrap();

    let app = riichi_api::app_with_auth_and_electric_url(
        Application::new(database.database.clone()),
        auth,
        Some(format!("http://{electric_address}/ignored")),
    );
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/projects/{project_id}/sync/issues?live=true&cache-buster=hold"
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    assert_eq!(body.next().await.unwrap().unwrap(), "[]");

    let heartbeat_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/auth/me")
        .header(header::COOKIE, format!("riichi_session={session_token}"))
        .body(axum::body::Body::empty())
        .unwrap();
    let heartbeat_response = app.clone().oneshot(heartbeat_request).await.unwrap();
    assert_eq!(heartbeat_response.status(), StatusCode::OK);
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(250), body.next())
            .await
            .is_err(),
        "session last-seen refresh must not close an active shape"
    );

    database
        .revoke_project_membership(project_id, account_id)
        .await
        .unwrap();

    let end = tokio::time::timeout(std::time::Duration::from_secs(3), body.next())
        .await
        .expect("shape stream should close after permission revocation");
    assert!(end.is_none());

    electric_task.abort();
    provider.abort();
}

fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

#[tokio::test]
#[ignore = "starts disposable PostgreSQL and Electric-compatible HTTP containers"]
async fn electric_proxy_authenticates_and_pins_project_shape() {
    let database = database().await;

    let electric_state = ElectricState::default();
    let electric_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let electric_address = electric_listener.local_addr().unwrap();
    let electric_app = Router::new()
        .route("/v1/shape", get(electric_shape))
        .with_state(electric_state.clone());
    let electric_task = tokio::spawn(async move {
        axum::serve(electric_listener, electric_app).await.unwrap();
    });

    let provider_listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let provider_address = provider_listener.local_addr().unwrap();
    let issuer = format!("http://{provider_address}");
    let provider = tokio::spawn({
        let issuer = issuer.clone();
        async move {
            axum::serve(
                provider_listener,
                Router::new()
                    .route("/.well-known/openid-configuration", get(discovery))
                    .route("/jwks", get(jwks))
                    .with_state(issuer),
            )
            .await
            .unwrap();
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(25)).await;
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

    let account_id = database
        .upsert_human_account(
            &issuer,
            "electric-proxy-user",
            Some("electric@example.test"),
            Some("Electric Proxy User"),
        )
        .await
        .unwrap();
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "Electric project")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, account_id, "owner")
        .await
        .unwrap();
    let team_id = Uuid::from_u128(2);
    sqlx::query(
        "INSERT INTO organization_memberships (organization_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000001', $1, 'owner')",
    )
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO team_memberships (team_id, account_id, role)
         VALUES ($1, $2, 'owner')",
    )
    .bind(team_id)
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    let session_token = "electric-session-token";
    database
        .create_human_session(
            Uuid::now_v7(),
            account_id,
            &hash(session_token),
            Duration::hours(1),
        )
        .await
        .unwrap();

    let app = riichi_api::app_with_auth_and_electric_url(
        Application::new(database.database.clone()),
        auth,
        Some(format!("http://{electric_address}/ignored")),
    );
    let unauthenticated_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sync/issues?table=users&where=org_id%3D1")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.clone().oneshot(unauthenticated_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let unauthorized_project_id = Uuid::now_v7();
    database
        .create_project(unauthorized_project_id, "Unauthorized Electric project")
        .await
        .unwrap();
    // `create_project` provisions the default Riichi team for a normal
    // application project. Remove that relationship here so this fixture is
    // genuinely outside the account's effective project permissions.
    sqlx::query("DELETE FROM project_teams WHERE project_id = $1")
        .bind(unauthorized_project_id)
        .execute(database.pool())
        .await
        .unwrap();
    let mut unauthorized_project_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/projects/{unauthorized_project_id}/sync/issues?table=users&where=org_id%3D1"
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    unauthorized_project_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app
        .clone()
        .oneshot(unauthorized_project_request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let mut request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/projects/{project_id}/sync/issues?table=users&where=org_id%3D1&live=true&offset=10_2"
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );

    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        "[]"
    );
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    let project_id_string = project_id.to_string();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("issue_metadata_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("project_id = $1")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(project_id_string.as_str())
    );
    assert_eq!(query.get("live").map(|value| value.as_ref()), Some("true"));
    assert_eq!(
        query.get("offset").map(|value| value.as_ref()),
        Some("10_2")
    );
    assert!(!query.contains_key("org_id"));

    let mut global_issues_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sync/issues?table=users&where=org_id%3D1&live=true")
        .body(axum::body::Body::empty())
        .unwrap();
    global_issues_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(global_issues_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    let account_id_string = account_id.to_string();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("human_issue_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("account_id = $1")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(account_id_string.as_str())
    );
    assert!(!query.contains_key("org_id"));

    let mut documents_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sync/documents?table=users&where=org_id%3D1&live=true")
        .body(axum::body::Body::empty())
        .unwrap();
    documents_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(documents_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("human_document_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("account_id = $1")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(account_id_string.as_str())
    );
    assert!(!query.contains_key("org_id"));

    let mut agents_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/teams/{team_id}/sync/agents?table=users&where=org_id%3D1&live=true"
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    agents_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(agents_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    let team_id_string = team_id.to_string();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("human_agent_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("team_id = $1 AND account_id = $2")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(team_id_string.as_str())
    );
    assert_eq!(
        query.get("params[2]").map(|value| value.as_ref()),
        Some(account_id_string.as_str())
    );
    assert!(!query.contains_key("org_id"));

    let mut inbox_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sync/inbox?table=users&where=org_id%3D1&live=true")
        .body(axum::body::Body::empty())
        .unwrap();
    inbox_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(inbox_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("notifications")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("recipient_account_id = $1")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(account_id_string.as_str())
    );
    assert!(!query.contains_key("org_id"));

    let mut approvals_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sync/approvals?table=users&where=org_id%3D1&live=true")
        .body(axum::body::Body::empty())
        .unwrap();
    approvals_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(approvals_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("approval_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("account_id = $1")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(account_id_string.as_str())
    );
    assert!(!query.contains_key("org_id"));

    let mut navigation_request = Request::builder()
        .method(Method::GET)
        .uri("/api/v1/sync/navigation?table=users&where=org_id%3D1&live=true")
        .body(axum::body::Body::empty())
        .unwrap();
    navigation_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.clone().oneshot(navigation_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("navigation_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("account_id = $1")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(account_id_string.as_str())
    );
    assert!(!query.contains_key("org_id"));

    let mut activity_request = Request::builder()
        .method(Method::GET)
        .uri(format!(
            "/api/v1/projects/{project_id}/sync/issues/{}/activity?table=users&where=org_id%3D1&live=true",
            Uuid::now_v7()
        ))
        .body(axum::body::Body::empty())
        .unwrap();
    activity_request.headers_mut().insert(
        header::COOKIE,
        format!("riichi_session={session_token}").parse().unwrap(),
    );
    let response = app.oneshot(activity_request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let query = electric_state
        .queries
        .lock()
        .unwrap()
        .last()
        .cloned()
        .unwrap();
    let query = url::form_urlencoded::parse(query.as_bytes()).collect::<HashMap<_, _>>();
    assert_eq!(
        query.get("table").map(|value| value.as_ref()),
        Some("issue_activity_sync")
    );
    assert_eq!(
        query.get("where").map(|value| value.as_ref()),
        Some("project_id = $1 AND issue_id = $2")
    );
    assert_eq!(
        query.get("params[1]").map(|value| value.as_ref()),
        Some(project_id_string.as_str())
    );
    assert_eq!(query.get("live").map(|value| value.as_ref()), Some("true"));
    assert!(!query.contains_key("org_id"));

    electric_task.abort();
    provider.abort();
}
