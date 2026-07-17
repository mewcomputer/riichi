mod support;

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use chrono::Duration;
use http_body_util::BodyExt;
use riichi_application::Application;
use riichi_persistence::{Database, NewIssue};
use serde_json::Value;
use support::PostgresHarness;
use tower::ServiceExt;
use uuid::Uuid;

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

async fn fixture(database: &Database) -> (Uuid, Uuid, Uuid, String) {
    let project_id = Uuid::now_v7();
    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_project(project_id, "api test project")
        .await
        .unwrap();
    database
        .create_agent_role(role_id, project_id, "implementation")
        .await
        .unwrap();
    database
        .create_session(
            session_id,
            project_id,
            role_id,
            Duration::hours(1),
            "api-test-token",
        )
        .await
        .unwrap();
    (project_id, role_id, session_id, "api-test-token".to_owned())
}

fn request(method: Method, uri: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn ready_requires_both_development_principal_headers() {
    let database = database().await;
    let app = riichi_api::app_with_state(Application::new(database.database.clone()));

    let response = app
        .oneshot(request(Method::POST, "/api/v1/ready", r#"{"limit":20}"#))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn ready_rejects_an_invalid_agent_credential() {
    let database = database().await;
    let (project_id, _role_id, session_id, _agent_token) = fixture(&database).await;
    let mut request = request(Method::POST, "/api/v1/ready", r#"{"limit":20}"#);
    request.headers_mut().insert(
        "x-riichi-project-id",
        project_id.to_string().parse().unwrap(),
    );
    request.headers_mut().insert(
        "x-riichi-session-id",
        session_id.to_string().parse().unwrap(),
    );
    request
        .headers_mut()
        .insert("authorization", "Bearer wrong-token".parse().unwrap());

    let response = riichi_api::app_with_state(Application::new(database.database.clone()))
        .oneshot(request)
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn auth_routes_report_when_oidc_is_not_configured() {
    let database = database().await;
    let app = riichi_api::app_with_state(Application::new(database.database.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/auth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn readiness_checks_database_health() {
    let database = database().await;
    let app = riichi_api::app_with_state(Application::new(database.database.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn human_me_reports_when_oidc_is_not_configured() {
    let database = database().await;
    let app = riichi_api::app_with_state(Application::new(database.database.clone()));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/v1/auth/me")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn human_queue_reports_when_oidc_is_not_configured() {
    let database = database().await;
    let project_id = Uuid::now_v7();
    let response = riichi_api::app_with_state(Application::new(database.database.clone()))
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri(format!("/api/v1/projects/{project_id}/queue"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn ready_returns_the_persisted_issue_shape() {
    let database = database().await;
    let (project_id, _role_id, session_id, agent_token) = fixture(&database).await;
    let issue_id = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: issue_id,
            project_id,
            display_key: "RII-1".to_owned(),
            title: "serve ready work".to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 0,
        })
        .await
        .unwrap();
    let mut request = request(Method::POST, "/api/v1/ready", r#"{"limit":20}"#);
    request.headers_mut().insert(
        "x-riichi-project-id",
        project_id.to_string().parse().unwrap(),
    );
    request.headers_mut().insert(
        "x-riichi-session-id",
        session_id.to_string().parse().unwrap(),
    );
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {agent_token}").parse().unwrap(),
    );

    let response = riichi_api::app_with_state(Application::new(database.database.clone()))
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["issues"][0]["id"], issue_id.to_string());
    assert_eq!(json["issues"][0]["display_key"], "RII-1");
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn claim_rejects_an_empty_idempotency_key_without_claiming() {
    let database = database().await;
    let (project_id, _role_id, session_id, agent_token) = fixture(&database).await;
    let issue_id = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: issue_id,
            project_id,
            display_key: "RII-API-2".to_owned(),
            title: "require a retry key".to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 0,
        })
        .await
        .unwrap();
    let mut request = request(
        Method::POST,
        "/api/v1/claim",
        &format!(r#"{{"issue_id":"{issue_id}","idempotency_key":"   "}}"#),
    );
    request.headers_mut().insert(
        "x-riichi-project-id",
        project_id.to_string().parse().unwrap(),
    );
    request.headers_mut().insert(
        "x-riichi-session-id",
        session_id.to_string().parse().unwrap(),
    );
    request.headers_mut().insert(
        "authorization",
        format!("Bearer {agent_token}").parse().unwrap(),
    );

    let response = riichi_api::app_with_state(Application::new(database.database.clone()))
        .oneshot(request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

    assert_eq!(
        database
            .ready(project_id, session_id, 20)
            .await
            .unwrap()
            .len(),
        1
    );
}
