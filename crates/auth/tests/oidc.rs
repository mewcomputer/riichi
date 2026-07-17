use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    routing::{get, post},
};
use chrono::{Duration, Utc};
use openidconnect::core::{CoreHmacKey, CoreIdToken, CoreIdTokenClaims, CoreJwsSigningAlgorithm};
use openidconnect::{
    AccessToken, Audience, AuthorizationCode, EmptyAdditionalClaims, EndUserEmail, IssuerUrl,
    Nonce, StandardClaims, SubjectIdentifier,
};
use riichi_auth::{AuthError, AuthService, HumanRole, OidcConfig};
use riichi_persistence::Database;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};
use tokio::net::TcpListener;

#[derive(Clone)]
struct ProviderState {
    issuer: String,
    id_token: Arc<Mutex<String>>,
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

async fn token(State(state): State<ProviderState>) -> Json<Value> {
    let id_token = state.id_token.lock().unwrap().clone();
    Json(json!({
        "access_token": "access-token",
        "token_type": "Bearer",
        "expires_in": 300,
        "id_token": id_token
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

struct PostgresHarness {
    _container: ContainerAsync<Postgres>,
    database: Database,
}

async fn database() -> PostgresHarness {
    let container = Postgres::default()
        .with_host_auth()
        .start()
        .await
        .expect("Docker must be available for OIDC integration tests");
    let host = container
        .get_host()
        .await
        .expect("the PostgreSQL container should expose a host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("the PostgreSQL container should expose a port");
    let database = Database::connect(&format!("postgres://postgres@{host}:{port}/postgres"), 5)
        .await
        .unwrap();
    database.migrate().await.unwrap();
    PostgresHarness {
        _container: container,
        database,
    }
}

fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn oidc_code_flow_issues_a_session_and_rejects_callback_replay() {
    let harness = database().await;
    let database = &harness.database;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let address = listener.local_addr().unwrap();
    let issuer = format!("http://{address}");
    let provider_state = ProviderState {
        issuer: issuer.clone(),
        id_token: Arc::new(Mutex::new(String::new())),
    };
    let app = Router::new()
        .route("/.well-known/openid-configuration", get(discovery))
        .route("/jwks", get(jwks))
        .route("/token", post(token))
        .with_state(provider_state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
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

    let authorization_url = auth.begin_login(database).await.unwrap();
    let state = authorization_url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    let code_challenge = authorization_url
        .query_pairs()
        .find(|(key, _)| key == "code_challenge")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    assert!(!code_challenge.is_empty());
    assert!(
        authorization_url
            .query_pairs()
            .find(|(key, value)| key == "scope" && value.contains("openid"))
            .is_some()
    );

    let nonce: String =
        sqlx::query_scalar("SELECT nonce FROM oidc_login_states WHERE state_hash = $1")
            .bind(hash(&state))
            .fetch_one(database.pool())
            .await
            .unwrap();
    let access_token = AccessToken::new("access-token".to_owned());
    let authorization_code = AuthorizationCode::new("code".to_owned());
    let now = Utc::now();
    let id_token = CoreIdToken::new(
        CoreIdTokenClaims::new(
            IssuerUrl::new(issuer.clone()).unwrap(),
            vec![Audience::new("riichi".to_owned())],
            now + Duration::minutes(5),
            now,
            StandardClaims::new(SubjectIdentifier::new("subject-1".to_owned()))
                .set_email(Some(EndUserEmail::new("person@example.test".to_owned())))
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
    .to_string();
    *provider_state.id_token.lock().unwrap() = id_token;

    let result = auth.finish_login(database, "code", &state).await.unwrap();
    assert_eq!(result.return_to, "/");
    assert!(!result.session_token.is_empty());
    assert!(
        database
            .active_human_session(&hash(&result.session_token))
            .await
            .unwrap()
            .is_some()
    );
    let session = database
        .active_human_session(&hash(&result.session_token))
        .await
        .unwrap()
        .unwrap();
    let project_id = uuid::Uuid::now_v7();
    database
        .create_project(project_id, "human principal project")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, session.account_id, "admin")
        .await
        .unwrap();
    let principal = auth
        .authenticate(database, &result.session_token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(principal.account.subject, "subject-1");
    assert!(principal.can_access_project(project_id, HumanRole::Member));

    let team_project_id = uuid::Uuid::now_v7();
    database
        .create_project(team_project_id, "team-derived project")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO organization_memberships (organization_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000001', $1, 'member')
         ON CONFLICT (organization_id, account_id) DO UPDATE SET revoked_at = NULL",
    )
    .bind(session.account_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO team_memberships (team_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000002', $1, 'member')
         ON CONFLICT (team_id, account_id) DO UPDATE SET revoked_at = NULL",
    )
    .bind(session.account_id)
    .execute(database.pool())
    .await
    .unwrap();
    let team_principal = auth
        .authenticate(database, &result.session_token)
        .await
        .unwrap()
        .unwrap();
    assert!(team_principal.can_access_project(team_project_id, HumanRole::Member));
    assert!(!team_principal.can_access_project(team_project_id, HumanRole::Admin));
    assert!(
        auth.authenticate(database, "wrong-session-token")
            .await
            .unwrap()
            .is_none()
    );

    let replay = auth
        .finish_login(database, "code", &state)
        .await
        .unwrap_err();
    assert!(matches!(replay, AuthError::InvalidState));

    let canceled_url = auth.begin_login(database).await.unwrap();
    let canceled_state = canceled_url
        .query_pairs()
        .find(|(key, _)| key == "state")
        .map(|(_, value)| value.into_owned())
        .unwrap();
    auth.cancel_login(database, &canceled_state).await.unwrap();
    let canceled = auth
        .finish_login(database, "code", &canceled_state)
        .await
        .unwrap_err();
    assert!(matches!(canceled, AuthError::InvalidState));

    server.abort();
}
