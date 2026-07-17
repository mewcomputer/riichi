mod support;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderValue, Method, Request, StatusCode, header},
    routing::{get, post},
};
use chrono::Duration;
use http_body_util::BodyExt;
use openidconnect::core::{CoreHmacKey, CoreIdToken, CoreIdTokenClaims, CoreJwsSigningAlgorithm};
use openidconnect::{
    AccessToken, Audience, AuthorizationCode, EmptyAdditionalClaims, EndUserEmail, IssuerUrl,
    Nonce, StandardClaims, SubjectIdentifier,
};
use riichi_api::app_with_auth;
use riichi_application::Application;
use riichi_auth::{AuthService, OidcConfig};
use riichi_persistence::NewIssue;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
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
    axum::extract::Form(form): axum::extract::Form<HashMap<String, String>>,
) -> (StatusCode, Json<Value>) {
    let code = form.get("code").cloned().unwrap_or_default();
    let code_verifier = form.get("code_verifier").cloned().unwrap_or_default();
    match state
        .tokens
        .lock()
        .unwrap()
        .get(&code)
        .filter(|fixture| fixture.code_verifier == code_verifier)
        .cloned()
    {
        Some(fixture) => (
            StatusCode::OK,
            Json(json!({
                "access_token": "access-token",
                "token_type": "Bearer",
                "expires_in": 300,
                "id_token": fixture.id_token
            })),
        ),
        None => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        ),
    }
}

fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

fn signed_id_token(issuer: &str, nonce: String) -> String {
    let access_token = AccessToken::new("access-token".to_owned());
    let authorization_code = AuthorizationCode::new("queue-e2e-code".to_owned());
    let now = chrono::Utc::now();
    CoreIdToken::new(
        CoreIdTokenClaims::new(
            IssuerUrl::new(issuer.to_owned()).unwrap(),
            vec![Audience::new("riichi".to_owned())],
            now + Duration::minutes(5),
            now,
            StandardClaims::new(SubjectIdentifier::new("queue-e2e-user".to_owned()))
                .set_email(Some(EndUserEmail::new("queue@example.test".to_owned())))
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

fn cookie_from_response(response: &axum::response::Response) -> HeaderValue {
    let set_cookie = response.headers().get(header::SET_COOKIE).unwrap();
    let cookie = set_cookie.to_str().unwrap().split(';').next().unwrap();
    HeaderValue::from_str(cookie).unwrap()
}

fn request(method: Method, uri: &str) -> Request<axum::body::Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(axum::body::Body::empty())
        .unwrap()
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn authenticated_human_queue_reads_authoritative_issue_and_lease_state() {
    let harness = support::PostgresHarness::start().await;
    let database = harness.database.clone();
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}");
    let provider_state = ProviderState {
        issuer: issuer.clone(),
        tokens: Arc::new(Mutex::new(HashMap::new())),
    };
    let provider = tokio::spawn({
        let state = provider_state.clone();
        async move {
            let app = Router::new()
                .route("/.well-known/openid-configuration", get(discovery))
                .route("/jwks", get(jwks))
                .route("/token", post(token))
                .with_state(state);
            axum::serve(listener, app).await.unwrap();
        }
    });
    let auth = AuthService::discover(OidcConfig {
        issuer_url: issuer.clone(),
        client_id: "riichi".to_owned(),
        client_secret: "secret".to_owned(),
        redirect_url: "http://riichi.test/auth/callback".to_owned(),
        cookie_secure: false,
        session_days: 1,
        login_state_minutes: 10,
    })
    .await
    .unwrap();
    let app = app_with_auth(Application::new(database.clone()), auth);

    let login_response = app
        .clone()
        .oneshot(request(Method::GET, "/auth/login"))
        .await
        .unwrap();
    assert!(login_response.status().is_redirection());
    let login_url = Url::parse(
        login_response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap();
    let state = login_url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    let (nonce, code_verifier): (String, String) =
        sqlx::query_as("SELECT nonce, pkce_verifier FROM oidc_login_states WHERE state_hash = $1")
            .bind(hash(&state))
            .fetch_one(database.pool())
            .await
            .unwrap();
    provider_state.tokens.lock().unwrap().insert(
        "queue-e2e-code".to_owned(),
        TokenFixture {
            code_verifier,
            id_token: signed_id_token(&issuer, nonce),
        },
    );

    let callback = app
        .clone()
        .oneshot(request(
            Method::GET,
            &format!("/auth/callback?code=queue-e2e-code&state={state}"),
        ))
        .await
        .unwrap();
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
    let cookie = cookie_from_response(&callback);
    let session = database
        .active_human_session(&hash(cookie.to_str().unwrap().split('=').nth(1).unwrap()))
        .await
        .unwrap()
        .unwrap();

    let project_id = Uuid::now_v7();
    database
        .create_human_project(project_id, "queue e2e project", session.account_id)
        .await
        .unwrap();
    let issue_id = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: issue_id,
            project_id,
            display_key: "RII-E2E-1".to_owned(),
            title: "read the real queue".to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 0,
        })
        .await
        .unwrap();
    let role_id = Uuid::now_v7();
    let agent_session_id = Uuid::now_v7();
    database
        .create_agent_role(role_id, project_id, "implementation")
        .await
        .unwrap();
    database
        .create_session(
            agent_session_id,
            project_id,
            role_id,
            Duration::hours(1),
            "queue-e2e-agent-token",
        )
        .await
        .unwrap();
    let claim = database
        .claim(
            project_id,
            agent_session_id,
            issue_id,
            Duration::minutes(30),
            "queue-e2e-claim",
        )
        .await
        .unwrap();

    let mut queue_request = request(Method::GET, &format!("/api/v1/projects/{project_id}/queue"));
    queue_request
        .headers_mut()
        .insert(header::COOKIE, cookie.clone());
    let queue_response = app.clone().oneshot(queue_request).await.unwrap();
    assert_eq!(queue_response.status(), StatusCode::OK);
    let body: Value = serde_json::from_slice(
        &queue_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes(),
    )
    .unwrap();
    assert_eq!(body["issues"][0]["display_key"], "RII-1");
    assert_eq!(
        body["issues"][0]["active_lease_id"],
        claim.lease_id.to_string()
    );
    assert_eq!(body["issues"][0]["status"], "in_progress");

    let other_project_id = Uuid::now_v7();
    database
        .create_project(other_project_id, "private project")
        .await
        .unwrap();
    // Normal projects start with the default Riichi team attached. Remove it
    // here so the cross-project request exercises a project the signed-in
    // account genuinely cannot access.
    sqlx::query("DELETE FROM project_teams WHERE project_id = $1")
        .bind(other_project_id)
        .execute(database.pool())
        .await
        .unwrap();
    let mut cross_project_request = request(
        Method::GET,
        &format!("/api/v1/projects/{other_project_id}/queue"),
    );
    cross_project_request
        .headers_mut()
        .insert(header::COOKIE, cookie);
    let cross_project_response = app.oneshot(cross_project_request).await.unwrap();
    assert_eq!(cross_project_response.status(), StatusCode::FORBIDDEN);

    provider.abort();
}
