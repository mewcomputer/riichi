mod support;

use bytes::Bytes;
use chrono::Duration;
use riichi_persistence::{
    Action, AttachmentUploadSeed, Database, DocumentCreate, NewIssue, Report,
};
use riichi_storage::{AttachmentStore, ObjectAttachmentStore};
use riichi_worker::{
    DeliveryError, DeliveryEvent, MAX_DELIVERY_ATTEMPTS, WorkerError, process_document_job,
    process_message,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use uuid::Uuid;

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

async fn fixture(database: &Database) -> (Uuid, Uuid, Uuid) {
    let project_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let session_id = Uuid::new_v4();
    let issue_id = Uuid::new_v4();
    database
        .create_project(project_id, "worker delivery project")
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
            &format!("worker-token-{session_id}"),
        )
        .await
        .unwrap();
    database
        .create_issue(NewIssue {
            id: issue_id,
            project_id,
            display_key: "RII-WORKER-1".to_owned(),
            title: "deliver this event".to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 0,
        })
        .await
        .unwrap();
    (project_id, session_id, issue_id)
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn recognized_lease_events_are_acknowledged() {
    let database = database().await;
    let (project_id, session_id, issue_id) = fixture(&database).await;
    let claim = database
        .claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(30),
            "worker-delivery-claim",
        )
        .await
        .unwrap();
    let message = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();

    let event = process_message(&database, &message).await.unwrap();
    assert_eq!(
        event,
        DeliveryEvent::LeaseChanged {
            project_id,
            issue_id,
            lease_id: claim.lease_id,
            event: "claimed".to_owned(),
        }
    );
    let delivered: bool =
        sqlx::query_scalar("SELECT delivered_at IS NOT NULL FROM outbox_messages WHERE id = $1")
            .bind(message.id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert!(delivered);
    let delivery_event: (String, serde_json::Value) =
        sqlx::query_as("SELECT event_type, payload FROM delivery_events WHERE id = $1")
            .bind(message.id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(delivery_event.0, "lease_changed");
    assert_eq!(delivery_event.1["event"], "claimed");

    process_message(&database, &message).await.unwrap();
    let delivery_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery_events WHERE id = $1")
            .bind(message.id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(delivery_count, 1);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn recognized_issue_events_are_acknowledged() {
    let database = database().await;
    let (project_id, session_id, issue_id) = fixture(&database).await;
    let claim = database
        .claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(30),
            "worker-delivery-report",
        )
        .await
        .unwrap();
    let claimed = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();
    process_message(&database, &claimed).await.unwrap();
    database
        .report(
            project_id,
            session_id,
            claim.lease_id,
            claim.fencing_token,
            Report {
                action: Action::Release,
                comment: Some("worker delivery test".to_owned()),
                resolution_summary: None,
            },
        )
        .await
        .unwrap();
    let message = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();

    let event = process_message(&database, &message).await.unwrap();
    assert_eq!(
        event,
        DeliveryEvent::IssueChanged {
            project_id,
            issue_id,
            lease_id: Some(claim.lease_id),
            event: "released".to_owned(),
        }
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn unsupported_messages_are_retried_with_visible_error_state() {
    let database = database().await;
    let project_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    database
        .create_project(project_id, "worker retry project")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO outbox_messages (id, project_id, message_type, payload)
         VALUES ($1, $2, 'future_delivery', $3)",
    )
    .bind(message_id)
    .bind(project_id)
    .bind(json!({
        "issue_id": Uuid::new_v4(),
        "lease_id": Uuid::new_v4(),
        "event": "sent"
    }))
    .execute(database.pool())
    .await
    .unwrap();
    let message = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();

    let result = process_message(&database, &message).await;
    assert!(matches!(
        result,
        Err(WorkerError::Delivery(DeliveryError::UnsupportedMessageType(message_type)))
            if message_type == "future_delivery"
    ));
    let (claimed_at, delivered_at, last_error): (
        Option<chrono::DateTime<chrono::Utc>>,
        Option<chrono::DateTime<chrono::Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT claimed_at, delivered_at, last_error FROM outbox_messages WHERE id = $1",
    )
    .bind(message.id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(claimed_at.is_none());
    assert!(delivered_at.is_none());
    assert_eq!(
        last_error.as_deref(),
        Some("outbox message type is not supported: future_delivery")
    );

    sqlx::query(
        "UPDATE outbox_messages SET available_at = now() - interval '1 second' WHERE id = $1",
    )
    .bind(message.id)
    .execute(database.pool())
    .await
    .unwrap();
    let retried = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retried.id, message.id);
    assert_eq!(retried.attempt_count, 2);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn poison_messages_are_dead_lettered_and_can_be_redriven() {
    let database = database().await;
    let project_id = Uuid::new_v4();
    let message_id = Uuid::new_v4();
    database
        .create_project(project_id, "worker dead letter project")
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO outbox_messages (id, project_id, message_type, payload)
         VALUES ($1, $2, 'future_delivery', $3)",
    )
    .bind(message_id)
    .bind(project_id)
    .bind(json!({
        "issue_id": Uuid::new_v4(),
        "lease_id": Uuid::new_v4(),
        "event": "sent"
    }))
    .execute(database.pool())
    .await
    .unwrap();

    for attempt in 1..=MAX_DELIVERY_ATTEMPTS {
        let message = database
            .claim_next_outbox(Some(project_id))
            .await
            .unwrap()
            .unwrap();
        let result = process_message(&database, &message).await;
        if attempt == MAX_DELIVERY_ATTEMPTS {
            assert!(matches!(
                result,
                Err(WorkerError::DeadLettered(DeliveryError::UnsupportedMessageType(message_type)))
                    if message_type == "future_delivery"
            ));
        } else {
            assert!(matches!(
                result,
                Err(WorkerError::Delivery(DeliveryError::UnsupportedMessageType(message_type)))
                    if message_type == "future_delivery"
            ));
            sqlx::query(
                "UPDATE outbox_messages SET available_at = now() - interval '1 second' WHERE id = $1",
            )
            .bind(message_id)
            .execute(database.pool())
            .await
            .unwrap();
        }
    }

    let dead_lettered: bool = sqlx::query_scalar(
        "SELECT dead_lettered_at IS NOT NULL FROM outbox_messages WHERE id = $1",
    )
    .bind(message_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert!(dead_lettered);
    assert!(
        database
            .claim_next_outbox(Some(project_id))
            .await
            .unwrap()
            .is_none()
    );

    assert!(
        database
            .redrive_outbox(project_id, message_id, Uuid::new_v4())
            .await
            .unwrap()
    );
    let redriven = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(redriven.attempt_count, 1);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn attachment_cleanup_removes_file_before_finalizing_metadata() {
    let database = database().await;
    let owner = database
        .upsert_human_account(
            "https://idp.example.test",
            "worker-attachment-owner",
            Some("worker-attachment-owner@example.test"),
            Some("Worker attachment owner"),
        )
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO organization_memberships (organization_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000001', $1, 'member')",
    )
    .bind(owner)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO team_memberships (team_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000002', $1, 'member')",
    )
    .bind(owner)
    .execute(database.pool())
    .await
    .unwrap();

    let document_id = Uuid::now_v7();
    database
        .create_document(DocumentCreate {
            id: document_id,
            organization_id: Uuid::from_u128(1),
            kind: "team_page".to_owned(),
            title: "Worker cleanup".to_owned(),
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

    let upload_id = Uuid::now_v7();
    let attachment_id = Uuid::now_v7();
    let storage_key = format!("uploads/{upload_id}.bin");
    let bytes = b"worker cleanup";
    database
        .create_attachment_upload(AttachmentUploadSeed {
            id: upload_id,
            attachment_id,
            organization_id: Uuid::from_u128(1),
            storage_key: storage_key.clone(),
            filename: "cleanup.txt".to_owned(),
            media_type: "text/plain".to_owned(),
            byte_size: bytes.len() as i64,
            checksum: Sha256::digest(bytes).to_vec(),
            uploaded_by: owner,
            document_id,
            source_block_id: "cleanup-file".to_owned(),
            lifetime: Duration::seconds(1),
        })
        .await
        .unwrap();
    sqlx::query(
        "UPDATE attachment_uploads
         SET expires_at = now() - interval '1 second'
         WHERE id = $1",
    )
    .bind(upload_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE document_jobs
         SET available_at = now() - interval '1 second'
         WHERE document_id = $1 AND job_type = 'attachment_cleanup'",
    )
    .bind(document_id)
    .execute(database.pool())
    .await
    .unwrap();

    let attachment_root = std::env::temp_dir().join(format!("riichi-worker-{upload_id}"));
    let attachment_store = ObjectAttachmentStore::local(&attachment_root).unwrap();
    attachment_store
        .put(&storage_key, Bytes::copy_from_slice(bytes))
        .await
        .unwrap();

    let job = database.claim_next_document_job().await.unwrap().unwrap();
    process_document_job(&database, &job, &attachment_store)
        .await
        .unwrap();

    assert!(attachment_store.get(&storage_key).await.is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM attachments WHERE id = $1")
            .bind(attachment_id)
            .fetch_one(database.pool())
            .await
            .unwrap(),
        0
    );
    assert!(
        sqlx::query_scalar::<_, bool>(
            "SELECT completed_at IS NOT NULL FROM document_jobs WHERE id = $1",
        )
        .bind(job.id)
        .fetch_one(database.pool())
        .await
        .unwrap()
    );
    let _ = std::fs::remove_dir_all(attachment_root);
}
