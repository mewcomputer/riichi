mod support;

use chrono::Duration;
use riichi_persistence::{
    DocumentContentUpdate, DocumentCreate, DocumentJobRetryOutcome, DocumentReferenceInput, Error,
    LoroSnapshotSeed, LoroUpdateOutcome, LoroUpdateSeed,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use uuid::Uuid;

async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

async fn account(database: &riichi_persistence::Database, subject: &str) -> Uuid {
    database
        .upsert_human_account(
            "https://idp.example.test",
            subject,
            Some(&format!("{subject}@example.test")),
            Some(subject),
        )
        .await
        .unwrap()
}

async fn grant_team_member(database: &riichi_persistence::Database, account_id: Uuid, role: &str) {
    let organization_role = if role == "viewer" { "member" } else { role };
    sqlx::query(
        "INSERT INTO organization_memberships (organization_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000001', $1, $2)",
    )
    .bind(account_id)
    .bind(organization_role)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO team_memberships (team_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000002', $1, $2)",
    )
    .bind(account_id)
    .bind(role)
    .execute(database.pool())
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn team_project_access_grants_effective_document_permissions() {
    let harness = database().await;
    let member = account(&harness, "project-team-member").await;
    let viewer = account(&harness, "project-team-viewer").await;
    grant_team_member(&harness, member, "member").await;
    grant_team_member(&harness, viewer, "viewer").await;

    let project_id = Uuid::now_v7();
    harness
        .create_project(project_id, "team-access project")
        .await
        .unwrap();

    let memberships = harness.human_memberships(member).await.unwrap();
    assert_eq!(
        memberships
            .iter()
            .find(|membership| membership.project_id == project_id)
            .map(|membership| membership.role.as_str()),
        Some("member")
    );

    let document = harness
        .create_document(DocumentCreate {
            id: Uuid::now_v7(),
            organization_id: Uuid::from_u128(1),
            kind: "project_page".to_owned(),
            title: "Team-accessible page".to_owned(),
            parent_document_id: None,
            position: 0,
            owner_team_id: None,
            owner_project_id: Some(project_id),
            created_by: member,
            content: json!({"type": "doc", "content": []}),
            plain_text: String::new(),
            sanitized_html: String::new(),
            schema_version: 1,
        })
        .await
        .unwrap();
    assert_eq!(
        harness
            .list_child_documents(member, None, Uuid::from_u128(1), None, Some(project_id),)
            .await
            .unwrap()
            .len(),
        1
    );
    assert!(matches!(
        harness
            .create_document(DocumentCreate {
                id: Uuid::now_v7(),
                organization_id: Uuid::from_u128(1),
                kind: "project_page".to_owned(),
                title: "Viewer cannot write".to_owned(),
                parent_document_id: None,
                position: 1,
                owner_team_id: None,
                owner_project_id: Some(project_id),
                created_by: viewer,
                content: json!({"type": "doc", "content": []}),
                plain_text: String::new(),
                sanitized_html: String::new(),
                schema_version: 1,
            })
            .await,
        Err(Error::DocumentAccessDenied)
    ));
    assert_eq!(
        harness.get_document(member, document.id).await.unwrap().id,
        document.id
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn documents_preserve_versions_scope_and_references() {
    let harness = database().await;
    let owner = account(&harness, "owner").await;
    let viewer = account(&harness, "viewer").await;
    let editor = account(&harness, "editor").await;
    grant_team_member(&harness, owner, "member").await;
    grant_team_member(&harness, viewer, "viewer").await;
    grant_team_member(&harness, editor, "member").await;

    let project_id = Uuid::now_v7();
    harness
        .create_project(project_id, "document project")
        .await
        .unwrap();
    harness
        .create_project_membership(project_id, owner, "owner")
        .await
        .unwrap();

    let document_id = Uuid::now_v7();
    let created = harness
        .create_document(DocumentCreate {
            id: document_id,
            organization_id: Uuid::from_u128(1),
            kind: "team_page".to_owned(),
            title: "Runbook".to_owned(),
            parent_document_id: None,
            position: 0,
            owner_team_id: Some(Uuid::from_u128(2)),
            owner_project_id: None,
            created_by: owner,
            content: json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "first"}]}]}),
            plain_text: "first".to_owned(),
            sanitized_html: "<p>first</p>".to_owned(),
            schema_version: 1,
        })
        .await
        .unwrap();
    assert_eq!(created.current_revision, Some(1));

    let synced = sqlx::query_as::<_, (String, String, Option<Uuid>, Option<Uuid>, i64)>(
        "SELECT title, provisioning_state, owner_team_id, owner_project_id, current_revision
         FROM human_document_sync
         WHERE account_id = $1 AND document_id = $2",
    )
    .bind(viewer)
    .bind(document_id)
    .fetch_one(harness.pool())
    .await
    .unwrap();
    assert_eq!(synced.0, "Runbook");
    assert_eq!(synced.1, "ready");
    assert_eq!(synced.2, Some(Uuid::from_u128(2)));
    assert_eq!(synced.3, None);
    assert_eq!(synced.4, 1);

    let visible_to_viewer = harness.get_document(viewer, document_id).await.unwrap();
    assert_eq!(visible_to_viewer.title, "Runbook");

    let updated = harness
        .update_document_content(
            owner,
            document_id,
            DocumentContentUpdate {
                expected_revision: 1,
                content: json!({"type": "doc", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "second"}]}]}),
                plain_text: "second".to_owned(),
                sanitized_html: "<p>second</p>".to_owned(),
                references: vec![DocumentReferenceInput {
                    source_block_id: "block-1".to_owned(),
                    resource_kind: "project".to_owned(),
                    resource_id: project_id,
                    reference_kind: "inline".to_owned(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.current_revision, Some(2));
    assert_eq!(updated.plain_text.as_deref(), Some("second"));
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT plain_text FROM human_document_sync
             WHERE account_id = $1 AND document_id = $2",
        )
        .bind(viewer)
        .bind(document_id)
        .fetch_one(harness.pool())
        .await
        .unwrap(),
        "second"
    );
    assert_eq!(
        harness
            .document_references(owner, document_id)
            .await
            .unwrap()
            .len(),
        1
    );

    let private_project_id = Uuid::now_v7();
    harness
        .create_project(private_project_id, "Private reference target")
        .await
        .unwrap();
    harness
        .create_project_membership(private_project_id, owner, "owner")
        .await
        .unwrap();
    sqlx::query("DELETE FROM project_teams WHERE project_id = $1")
        .bind(private_project_id)
        .execute(harness.pool())
        .await
        .unwrap();
    let unauthorized_reference = harness
        .update_document_content(
            editor,
            document_id,
            DocumentContentUpdate {
                expected_revision: 2,
                content: json!({"type": "doc", "content": []}),
                plain_text: String::new(),
                sanitized_html: String::new(),
                references: vec![DocumentReferenceInput {
                    source_block_id: "private-project".to_owned(),
                    resource_kind: "project".to_owned(),
                    resource_id: private_project_id,
                    reference_kind: "inline".to_owned(),
                }],
            },
        )
        .await;
    assert!(
        matches!(unauthorized_reference, Err(Error::InvalidDocument(_))),
        "unexpected reference result: {unauthorized_reference:?}"
    );

    let stale = harness
        .update_document_content(
            owner,
            document_id,
            DocumentContentUpdate {
                expected_revision: 1,
                content: json!({"type": "doc", "content": []}),
                plain_text: String::new(),
                sanitized_html: String::new(),
                references: Vec::new(),
            },
        )
        .await;
    assert!(matches!(stale, Err(Error::DocumentVersionConflict)));

    let viewer_write = harness
        .update_document_content(
            viewer,
            document_id,
            DocumentContentUpdate {
                expected_revision: 2,
                content: json!({"type": "doc", "content": []}),
                plain_text: String::new(),
                sanitized_html: String::new(),
                references: Vec::new(),
            },
        )
        .await;
    assert!(matches!(viewer_write, Err(Error::DocumentAccessDenied)));

    let renamed = harness
        .update_document_metadata(owner, document_id, "Team runbook".to_owned(), None, 4)
        .await
        .unwrap();
    assert_eq!(renamed.title, "Team runbook");
    assert_eq!(renamed.position, 4);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT title FROM human_document_sync
             WHERE account_id = $1 AND document_id = $2",
        )
        .bind(viewer)
        .bind(document_id)
        .fetch_one(harness.pool())
        .await
        .unwrap(),
        "Team runbook"
    );

    harness.delete_document(owner, document_id).await.unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM human_document_sync WHERE document_id = $1",
        )
        .bind(document_id)
        .fetch_one(harness.pool())
        .await
        .unwrap(),
        0
    );
    assert!(matches!(
        harness.get_document(owner, document_id).await,
        Err(Error::DocumentAccessDenied)
    ));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn loro_storage_serializes_frontiers_and_replays_idempotent_updates() {
    let harness = database().await;
    let owner = account(&harness, "loro-owner").await;
    let viewer = account(&harness, "loro-viewer").await;
    grant_team_member(&harness, owner, "member").await;
    grant_team_member(&harness, viewer, "viewer").await;
    let document_id = Uuid::now_v7();
    harness
        .create_document(DocumentCreate {
            id: document_id,
            organization_id: Uuid::from_u128(1),
            kind: "team_page".to_owned(),
            title: "Loro storage".to_owned(),
            parent_document_id: None,
            position: 0,
            owner_team_id: Some(Uuid::from_u128(2)),
            owner_project_id: None,
            created_by: owner,
            content: json!({"type": "doc", "content": []}),
            plain_text: String::new(),
            sanitized_html: String::new(),
            schema_version: 1,
        })
        .await
        .unwrap();

    let initial_frontiers = json!([{"peer_id": 1, "counter": 0}]);
    let initial = harness
        .initialize_loro_snapshot(
            owner,
            LoroSnapshotSeed {
                document_id,
                source_revision: 1,
                schema_version: 1,
                frontiers: initial_frontiers.clone(),
                snapshot: vec![1],
            },
        )
        .await
        .unwrap();
    assert_eq!(initial.snapshot, vec![1]);
    let preserved = harness
        .initialize_loro_snapshot(
            owner,
            LoroSnapshotSeed {
                document_id,
                source_revision: 2,
                schema_version: 1,
                frontiers: json!([]),
                snapshot: vec![9],
            },
        )
        .await
        .unwrap();
    assert_eq!(preserved.snapshot, vec![1]);

    let payload = vec![2, 3, 4];
    let checksum = Sha256::digest(&payload).to_vec();
    let seed = LoroUpdateSeed {
        update_id: Uuid::now_v7(),
        document_id,
        principal_id: owner,
        source: "human".to_owned(),
        peer_id: "1".to_owned(),
        idempotency_key: Some("edit-1".to_owned()),
        previous_frontiers: initial_frontiers.clone(),
        resulting_frontiers: json!([{"peer_id": 1, "counter": 1}]),
        payload: payload.clone(),
        payload_sha256: checksum.clone(),
        snapshot: vec![5],
        content: json!({"type": "doc", "content": []}),
        plain_text: String::new(),
        sanitized_html: String::new(),
        references: Vec::new(),
    };
    let (accepted, outcome) = harness
        .accept_loro_update(owner, seed.clone())
        .await
        .unwrap();
    assert_eq!(outcome, LoroUpdateOutcome::Accepted);
    assert_eq!(accepted.payload, payload);
    let activity: (Uuid, Uuid, String, serde_json::Value, serde_json::Value) = sqlx::query_as(
        "SELECT document_id, update_id, source, previous_frontiers, resulting_frontiers
         FROM document_activity
         WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_one(harness.pool())
    .await
    .unwrap();
    assert_eq!(activity.0, document_id);
    assert_eq!(activity.1, accepted.update_id);
    assert_eq!(activity.2, "human");
    assert_eq!(activity.3, json!([{"peer_id": 1, "counter": 0}]));
    assert_eq!(activity.4, json!([{"peer_id": 1, "counter": 1}]));

    let (replayed, outcome) = harness
        .accept_loro_update(owner, seed.clone())
        .await
        .unwrap();
    assert_eq!(outcome, LoroUpdateOutcome::Replayed);
    assert_eq!(replayed.update_id, accepted.update_id);
    assert_eq!(
        harness
            .get_loro_snapshot(owner, document_id)
            .await
            .unwrap()
            .unwrap()
            .snapshot,
        vec![5]
    );

    let stale = harness
        .accept_loro_update(
            owner,
            LoroUpdateSeed {
                update_id: Uuid::now_v7(),
                document_id,
                principal_id: owner,
                source: "human".to_owned(),
                peer_id: "1".to_owned(),
                idempotency_key: Some("edit-2".to_owned()),
                previous_frontiers: initial_frontiers,
                resulting_frontiers: json!([{"peer_id": 1, "counter": 2}]),
                payload: vec![6],
                payload_sha256: Sha256::digest([6]).to_vec(),
                snapshot: vec![7],
                content: json!({"type": "doc", "content": []}),
                plain_text: String::new(),
                sanitized_html: String::new(),
                references: Vec::new(),
            },
        )
        .await;
    assert!(matches!(stale, Err(Error::LoroFrontierConflict)));

    let conflicting_retry = harness
        .accept_loro_update(
            owner,
            LoroUpdateSeed {
                payload: vec![8],
                payload_sha256: Sha256::digest([8]).to_vec(),
                ..seed
            },
        )
        .await;
    assert!(matches!(conflicting_retry, Err(Error::IdempotencyConflict)));

    let unauthorized = harness
        .accept_loro_update(
            viewer,
            LoroUpdateSeed {
                update_id: Uuid::now_v7(),
                document_id,
                principal_id: viewer,
                source: "human".to_owned(),
                peer_id: "2".to_owned(),
                idempotency_key: Some("viewer-edit".to_owned()),
                previous_frontiers: json!([{"peer_id": 1, "counter": 1}]),
                resulting_frontiers: json!([{"peer_id": 2, "counter": 1}]),
                payload: vec![9],
                payload_sha256: Sha256::digest([9]).to_vec(),
                snapshot: vec![10],
                content: json!({"type": "doc", "content": []}),
                plain_text: String::new(),
                sanitized_html: String::new(),
                references: Vec::new(),
            },
        )
        .await;
    assert!(matches!(unauthorized, Err(Error::DocumentAccessDenied)));

    harness
        .compact_loro_document(
            document_id,
            json!([{"peer_id": 1, "counter": 1}]),
            vec![5],
            "",
            "",
        )
        .await
        .unwrap();
    let update_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM document_loro_updates WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_one(harness.pool())
    .await
    .unwrap();
    assert_eq!(update_count, 0);
    let activity_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM document_activity WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_one(harness.pool())
    .await
    .unwrap();
    assert_eq!(activity_count, 1);
    let compacted = harness
        .get_loro_snapshot(owner, document_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(compacted.source_revision, 2);
    assert_eq!(compacted.frontiers, json!([{"peer_id": 1, "counter": 1}]));
    assert_eq!(compacted.snapshot, vec![5]);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn document_projection_refresh_persists_derived_content() {
    let harness = database().await;
    let owner = account(&harness, "projection-owner").await;
    grant_team_member(&harness, owner, "member").await;
    let document_id = Uuid::now_v7();
    harness
        .create_document(DocumentCreate {
            id: document_id,
            organization_id: Uuid::from_u128(1),
            kind: "team_page".to_owned(),
            title: "Projection".to_owned(),
            parent_document_id: None,
            position: 0,
            owner_team_id: Some(Uuid::from_u128(2)),
            owner_project_id: None,
            created_by: owner,
            content: json!({"type": "doc", "content": []}),
            plain_text: String::new(),
            sanitized_html: String::new(),
            schema_version: 1,
        })
        .await
        .unwrap();

    harness
        .refresh_document_projection(document_id, "derived text", "<p>derived text</p>")
        .await
        .unwrap();

    let projected = sqlx::query_as::<_, (i64, String, String, i32)>(
        "SELECT content_revision, plain_text, sanitized_html, schema_version
         FROM document_projections
         WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_one(harness.pool())
    .await
    .unwrap();
    assert_eq!(
        projected,
        (
            1,
            "derived text".to_owned(),
            "<p>derived text</p>".to_owned(),
            1
        )
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn attachment_uploads_verify_checksum_before_becoming_ready() {
    let harness = database().await;
    let owner = account(&harness, "attachment-owner").await;
    grant_team_member(&harness, owner, "member").await;
    let document = harness
        .create_document(DocumentCreate {
            id: Uuid::now_v7(),
            organization_id: Uuid::from_u128(1),
            kind: "team_page".to_owned(),
            title: "Files".to_owned(),
            parent_document_id: None,
            position: 0,
            owner_team_id: Some(Uuid::from_u128(2)),
            owner_project_id: None,
            created_by: owner,
            content: json!({"type": "doc", "content": []}),
            plain_text: String::new(),
            sanitized_html: String::new(),
            schema_version: 1,
        })
        .await
        .unwrap();
    let bytes = b"hello attachment";
    let checksum = Sha256::digest(bytes).to_vec();
    let upload_id = Uuid::now_v7();
    harness
        .create_attachment_upload(riichi_persistence::AttachmentUploadSeed {
            id: upload_id,
            attachment_id: Uuid::now_v7(),
            organization_id: Uuid::from_u128(1),
            storage_key: format!("uploads/{upload_id}.bin"),
            filename: "hello.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            byte_size: bytes.len() as i64,
            checksum: checksum.clone(),
            uploaded_by: owner,
            document_id: document.id,
            source_block_id: "file-1".to_owned(),
            lifetime: Duration::hours(1),
        })
        .await
        .unwrap();

    let invalid = harness
        .complete_attachment_upload(owner, upload_id, bytes.len() as i64, &[0; 32])
        .await;
    assert!(matches!(invalid, Err(Error::AttachmentVerificationFailed)));

    let attachment = harness
        .complete_attachment_upload(owner, upload_id, bytes.len() as i64, &checksum)
        .await
        .unwrap();
    assert_eq!(attachment.state, "ready");
    assert_eq!(
        harness
            .get_attachment(owner, attachment.id)
            .await
            .unwrap()
            .filename,
        "hello.txt"
    );

    let expired_upload_id = Uuid::now_v7();
    let expired_attachment_id = Uuid::now_v7();
    harness
        .create_attachment_upload(riichi_persistence::AttachmentUploadSeed {
            id: expired_upload_id,
            attachment_id: expired_attachment_id,
            organization_id: Uuid::from_u128(1),
            storage_key: format!("uploads/{expired_upload_id}.bin"),
            filename: "expired.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            byte_size: bytes.len() as i64,
            checksum: checksum.clone(),
            uploaded_by: owner,
            document_id: document.id,
            source_block_id: "expired-file".to_owned(),
            lifetime: Duration::seconds(1),
        })
        .await
        .unwrap();
    sqlx::query(
        "UPDATE attachment_uploads SET expires_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(expired_upload_id)
    .execute(harness.pool())
    .await
    .unwrap();

    let claimed = harness.claim_expired_attachment_uploads().await.unwrap();
    assert_eq!(
        claimed,
        vec![(
            expired_attachment_id,
            format!("uploads/{expired_upload_id}.bin")
        )]
    );
    assert_eq!(
        harness
            .claim_expired_attachment_uploads()
            .await
            .unwrap()
            .len(),
        0
    );
    sqlx::query(
        "UPDATE attachment_uploads
         SET cleanup_claimed_at = now() - interval '6 minutes'
         WHERE id = $1",
    )
    .bind(expired_upload_id)
    .execute(harness.pool())
    .await
    .unwrap();
    assert_eq!(
        harness.claim_expired_attachment_uploads().await.unwrap(),
        vec![(
            expired_attachment_id,
            format!("uploads/{expired_upload_id}.bin")
        )]
    );
    assert!(
        harness
            .finalize_expired_attachment_upload(expired_attachment_id)
            .await
            .unwrap()
    );
    assert!(
        !harness
            .finalize_expired_attachment_upload(expired_attachment_id)
            .await
            .unwrap()
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn document_jobs_are_claimed_retried_and_dead_lettered() {
    let harness = database().await;
    harness
        .enqueue_document_job(
            Uuid::now_v7(),
            None,
            "attachment_cleanup",
            "cleanup-test",
            chrono::Utc::now(),
        )
        .await
        .unwrap();

    let job = harness.claim_next_document_job().await.unwrap().unwrap();
    assert_eq!(job.attempt_count, 1);
    assert_eq!(job.job_type, "attachment_cleanup");

    let outcome = harness
        .retry_document_job(job.id, "test failure", Duration::zero(), 1)
        .await
        .unwrap();
    assert_eq!(outcome, DocumentJobRetryOutcome::DeadLettered);
    assert!(harness.claim_next_document_job().await.unwrap().is_none());
}
