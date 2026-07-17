mod support;

use chrono::Duration;
use riichi_persistence::ProjectInviteSeed;
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use uuid::Uuid;

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

fn hash(value: &str) -> Vec<u8> {
    Sha256::digest(value.as_bytes()).to_vec()
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn oidc_login_state_can_be_consumed_only_once() {
    let database = database().await;
    let state_hash = hash(&Uuid::now_v7().to_string());

    database
        .create_oidc_login_state(
            &state_hash,
            "https://idp.example.test",
            "nonce",
            "pkce-verifier",
            "/",
            Duration::hours(1),
        )
        .await
        .unwrap();

    let consumed = database
        .consume_oidc_login_state(&state_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(consumed.issuer, "https://idp.example.test");
    assert_eq!(consumed.nonce, "nonce");
    assert_eq!(consumed.pkce_verifier, "pkce-verifier");

    assert!(
        database
            .consume_oidc_login_state(&state_hash)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn expired_oidc_login_state_cannot_be_consumed() {
    let database = database().await;
    let state_hash = hash(&Uuid::now_v7().to_string());

    database
        .create_oidc_login_state(
            &state_hash,
            "https://idp.example.test",
            "nonce",
            "pkce-verifier",
            "/",
            Duration::hours(1),
        )
        .await
        .unwrap();
    sqlx::query("UPDATE oidc_login_states SET expires_at = now() - interval '1 second'")
        .execute(database.pool())
        .await
        .unwrap();

    assert!(
        database
            .consume_oidc_login_state(&state_hash)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn human_accounts_are_stable_for_an_issuer_and_subject_pair() {
    let database = database().await;

    let first = database
        .upsert_human_account(
            "https://idp.example.test",
            "subject-1",
            Some("first@example.test"),
            Some("First User"),
        )
        .await
        .unwrap();
    let same_identity = database
        .upsert_human_account(
            "https://idp.example.test",
            "subject-1",
            Some("updated@example.test"),
            Some("Updated User"),
        )
        .await
        .unwrap();
    let different_issuer = database
        .upsert_human_account(
            "https://other-idp.example.test",
            "subject-1",
            Some("updated@example.test"),
            Some("Updated User"),
        )
        .await
        .unwrap();

    assert_eq!(first, same_identity);
    assert_ne!(first, different_issuer);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn project_memberships_can_be_granted_updated_and_revoked() {
    let database = database().await;
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "membership project")
        .await
        .unwrap();
    let account_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            None,
            None,
        )
        .await
        .unwrap();

    database
        .create_project_membership(project_id, account_id, "viewer")
        .await
        .unwrap();
    let viewer = database.human_memberships(account_id).await.unwrap();
    assert_eq!(viewer.len(), 1);
    assert_eq!(viewer[0].role, "viewer");

    database
        .create_project_membership(project_id, account_id, "admin")
        .await
        .unwrap();
    let admin = database.human_memberships(account_id).await.unwrap();
    assert_eq!(admin.len(), 1);
    assert_eq!(admin[0].role, "admin");

    database
        .revoke_project_membership(project_id, account_id)
        .await
        .unwrap();
    assert!(
        database
            .human_memberships(account_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn human_session_lookup_uses_the_hash_and_revocation_is_immediate() {
    let database = database().await;
    let account_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            None,
            None,
        )
        .await
        .unwrap();
    let token = Uuid::now_v7().to_string();
    let token_hash = hash(&token);
    let session_id = Uuid::now_v7();

    database
        .create_human_session(session_id, account_id, &token_hash, Duration::hours(1))
        .await
        .unwrap();

    let session = database
        .active_human_session(&token_hash)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(session.id, session_id);
    assert_eq!(session.account_id, account_id);
    assert!(
        database
            .active_human_session(token.as_bytes())
            .await
            .unwrap()
            .is_none()
    );

    database.revoke_human_session(&token_hash).await.unwrap();
    assert!(
        database
            .active_human_session(&token_hash)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn invite_acceptance_is_email_bound_one_time_and_preserves_owner_role() {
    let database = database().await;
    let project_id = Uuid::now_v7();
    let owner_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            Some("owner@example.test"),
            Some("Owner"),
        )
        .await
        .unwrap();
    database
        .create_project(project_id, "invite project")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, owner_id, "owner")
        .await
        .unwrap();
    let member_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            Some("member@example.test"),
            Some("Member"),
        )
        .await
        .unwrap();
    let token_hash = hash(&Uuid::now_v7().to_string());
    let invite_id = Uuid::now_v7();
    database
        .create_project_invite(ProjectInviteSeed {
            id: invite_id,
            project_id,
            invited_by: owner_id,
            role: "member".to_owned(),
            email_hint: Some("member@example.test".to_owned()),
            token_hash: token_hash.clone(),
            lifetime: Duration::hours(1),
        })
        .await
        .unwrap();

    assert!(
        database
            .accept_project_invite(&token_hash, member_id, Some("wrong@example.test"))
            .await
            .unwrap()
            .is_none()
    );
    let accepted = database
        .accept_project_invite(&token_hash, member_id, Some("member@example.test"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(accepted.project_id, project_id);
    assert_eq!(accepted.role, "member");
    assert!(
        database
            .accept_project_invite(&token_hash, member_id, Some("member@example.test"))
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        database.human_memberships(member_id).await.unwrap()[0].role,
        "member"
    );

    let owner_token_hash = hash(&Uuid::now_v7().to_string());
    database
        .create_project_invite(ProjectInviteSeed {
            id: Uuid::now_v7(),
            project_id,
            invited_by: owner_id,
            role: "admin".to_owned(),
            email_hint: None,
            token_hash: owner_token_hash.clone(),
            lifetime: Duration::hours(1),
        })
        .await
        .unwrap();
    database
        .accept_project_invite(&owner_token_hash, owner_id, Some("owner@example.test"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        database.human_memberships(owner_id).await.unwrap()[0].role,
        "owner"
    );

    let viewer_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            Some("viewer@example.test"),
            Some("Viewer"),
        )
        .await
        .unwrap();
    let viewer_token_hash = hash(&Uuid::now_v7().to_string());
    database
        .create_project_invite(ProjectInviteSeed {
            id: Uuid::now_v7(),
            project_id,
            invited_by: owner_id,
            role: "viewer".to_owned(),
            email_hint: Some("viewer@example.test".to_owned()),
            token_hash: viewer_token_hash.clone(),
            lifetime: Duration::hours(1),
        })
        .await
        .unwrap();
    database
        .accept_project_invite(&viewer_token_hash, viewer_id, Some("viewer@example.test"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        database.human_team_memberships(viewer_id).await.unwrap()[0].role,
        "viewer"
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn revoked_and_expired_invites_cannot_be_accepted() {
    let database = database().await;
    let project_id = Uuid::now_v7();
    let inviter_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            None,
            None,
        )
        .await
        .unwrap();
    let account_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            None,
            None,
        )
        .await
        .unwrap();
    database
        .create_project(project_id, "invite expiry project")
        .await
        .unwrap();

    let revoked_id = Uuid::now_v7();
    let revoked_token = hash(&Uuid::now_v7().to_string());
    database
        .create_project_invite(ProjectInviteSeed {
            id: revoked_id,
            project_id,
            invited_by: inviter_id,
            role: "viewer".to_owned(),
            email_hint: None,
            token_hash: revoked_token.clone(),
            lifetime: Duration::hours(1),
        })
        .await
        .unwrap();
    database
        .revoke_project_invite(project_id, revoked_id)
        .await
        .unwrap();
    assert!(
        database
            .accept_project_invite(&revoked_token, account_id, None)
            .await
            .unwrap()
            .is_none()
    );

    let expired_id = Uuid::now_v7();
    let expired_token = hash(&Uuid::now_v7().to_string());
    database
        .create_project_invite(ProjectInviteSeed {
            id: expired_id,
            project_id,
            invited_by: inviter_id,
            role: "viewer".to_owned(),
            email_hint: None,
            token_hash: expired_token.clone(),
            lifetime: Duration::hours(1),
        })
        .await
        .unwrap();
    sqlx::query(
        "UPDATE project_invites SET expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(expired_id)
    .execute(database.pool())
    .await
    .unwrap();
    assert!(
        database
            .accept_project_invite(&expired_token, account_id, None)
            .await
            .unwrap()
            .is_none()
    );
}
