mod support;

use chrono::Duration;
use riichi_persistence::{
    Database, Error, IssueCreate, IssueUpdate, LoroSnapshotSeed, LoroUpdateOutcome, LoroUpdateSeed,
};
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use uuid::Uuid;

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

async fn project(database: &Database) -> (Uuid, Uuid) {
    let project_id = Uuid::now_v7();
    let actor_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &format!("triage-actor-{project_id}"),
            None,
            Some("Triage test actor"),
        )
        .await
        .unwrap();
    database
        .create_project(project_id, "triage test project")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, actor_id, "owner")
        .await
        .unwrap();
    (project_id, actor_id)
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn issue_crud_round_trip_preserves_triage_fields_and_rejects_stale_writes() {
    let database = database().await;
    let (project_id, actor_id) = project(&database).await;
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-TRIAGE-1".to_owned(),
                title: "capture the lease policy".to_owned(),
                body: "document the recovery path".to_owned(),
                status: "triage".to_owned(),
                agent_eligible: false,
                spec_complete: false,
                rank: 42,
                labels: vec!["pilot".to_owned(), "needs-context".to_owned()],
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();

    let created = database.get_issue(project_id, issue_id).await.unwrap();
    assert!(created.display_key.starts_with("RII-"));
    assert_ne!(created.display_key, "RII-TRIAGE-1");
    assert_eq!(created.body, "document the recovery path");
    assert_eq!(created.status, "triage");
    assert_eq!(created.rank, 42);
    assert_eq!(created.labels, vec!["needs-context", "pilot"]);
    assert_eq!(created.version, 1);
    let synced: (
        Uuid,
        Uuid,
        String,
        String,
        String,
        bool,
        bool,
        i64,
        i64,
        Vec<String>,
    ) = sqlx::query_as(
        "SELECT issue_id, project_id, title, status, importance, agent_eligible,
                    spec_complete, version, rank, labels
             FROM issue_metadata_sync
             WHERE issue_id = $1",
    )
    .bind(issue_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(synced.0, issue_id);
    assert_eq!(synced.1, project_id);
    assert_eq!(synced.2, "capture the lease policy");
    assert_eq!(synced.3, "triage");
    assert_eq!(synced.4, "none");
    assert!(!synced.5);
    assert!(!synced.6);
    assert_eq!(synced.7, 1);
    assert_eq!(synced.8, 42);
    assert_eq!(synced.9, vec!["needs-context", "pilot"]);
    let description = database
        .get_issue_description_document(actor_id, project_id, issue_id)
        .await
        .unwrap();
    assert_eq!(description.kind, "issue_description");
    assert_eq!(description.provisioning_state, "pending");
    assert_eq!(
        description.plain_text.as_deref(),
        Some("document the recovery path")
    );
    let job_count = sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM document_jobs WHERE document_id = $1 AND job_type = 'provision'",
    )
    .bind(description.id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(job_count, 1);
    database
        .initialize_loro_snapshot(
            actor_id,
            LoroSnapshotSeed {
                document_id: description.id,
                source_revision: 1,
                schema_version: 1,
                frontiers: serde_json::json!([]),
                snapshot: vec![1],
            },
        )
        .await
        .unwrap();
    let document_update_id = Uuid::now_v7();
    let document_payload = vec![2, 3, 4];
    let (document_update, document_outcome) = database
        .accept_loro_update(
            actor_id,
            LoroUpdateSeed {
                update_id: document_update_id,
                document_id: description.id,
                principal_id: actor_id,
                source: "human".to_owned(),
                peer_id: "triage-peer".to_owned(),
                idempotency_key: Some("triage-document-edit".to_owned()),
                previous_frontiers: serde_json::json!([]),
                resulting_frontiers: serde_json::json!([{"peer_id": 1, "counter": 1}]),
                payload: document_payload.clone(),
                payload_sha256: Sha256::digest(&document_payload).to_vec(),
                snapshot: vec![5],
                content: serde_json::json!({"type": "doc", "content": []}),
                plain_text: "updated description".to_owned(),
                sanitized_html: "<p>updated description</p>".to_owned(),
                references: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(document_outcome, LoroUpdateOutcome::Accepted);
    assert_eq!(document_update.update_id, document_update_id);
    let activity = database
        .issue_activity(project_id, issue_id, 200)
        .await
        .unwrap();
    let document_activity = activity
        .iter()
        .find(|entry| entry.kind == "document_edit")
        .expect("issue activity should include accepted document edits");
    assert_eq!(document_activity.actor_id, actor_id);
    assert_eq!(
        document_activity.metadata["update_id"],
        serde_json::json!(document_update_id)
    );
    assert_eq!(document_activity.metadata["source"], "human");

    let comment = database
        .create_human_comment(
            project_id,
            issue_id,
            actor_id,
            "a durable activity comment",
            serde_json::json!({"type": "doc", "content": []}),
        )
        .await
        .unwrap();
    let projected_activity: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT id, kind, body
         FROM issue_activity_sync
         WHERE project_id = $1 AND issue_id = $2
         ORDER BY created_at, id",
    )
    .bind(project_id)
    .bind(issue_id)
    .fetch_all(database.pool())
    .await
    .unwrap();
    assert!(projected_activity.iter().any(|entry| {
        entry.0 == comment.id
            && entry.1 == "comment"
            && entry.2.as_deref() == Some("a durable activity comment")
    }));
    assert!(
        projected_activity
            .iter()
            .any(|entry| entry.1 == "create_issue")
    );
    assert!(
        projected_activity
            .iter()
            .any(|entry| entry.1 == "document_edit")
    );

    let updated = database
        .update_issue(
            project_id,
            issue_id,
            IssueUpdate {
                expected_version: 1,
                title: Some("capture the durable lease policy".to_owned()),
                status: Some("todo".to_owned()),
                importance: Some("high".to_owned()),
                agent_eligible: Some(true),
                spec_complete: Some(true),
                rank: Some(7),
                labels: Some(vec!["pilot".to_owned()]),
                assignee_account_id: None,
                due_date: None,
                snoozed_until: None,
                workflow_alias: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    assert_eq!(updated.title, "capture the durable lease policy");
    assert_eq!(updated.status, "todo");
    assert_eq!(updated.importance, "high");
    assert_eq!(updated.rank, 7);
    assert!(!updated.specification_changed_since_review);
    let updated_synced: (
        String,
        String,
        String,
        bool,
        bool,
        i64,
        i64,
        Vec<String>,
        i64,
    ) = sqlx::query_as(
        "SELECT title, status, importance, agent_eligible, spec_complete,
                    version, rank, labels, transaction_id
             FROM issue_metadata_sync
             WHERE issue_id = $1",
    )
    .bind(issue_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(updated_synced.0, "capture the durable lease policy");
    assert_eq!(updated_synced.1, "todo");
    assert_eq!(updated_synced.2, "high");
    assert!(updated_synced.3);
    assert!(updated_synced.4);
    assert_eq!(updated_synced.5, 2);
    assert_eq!(updated_synced.6, 7);
    assert_eq!(updated_synced.7, vec!["pilot"]);
    assert!(updated_synced.8 > 0);

    let marked_update = database
        .update_issue_with_transaction(
            project_id,
            issue_id,
            IssueUpdate {
                expected_version: 2,
                status: Some("in_progress".to_owned()),
                ..IssueUpdate::default()
            },
            actor_id,
        )
        .await
        .unwrap();
    let synced_transaction_id = sqlx::query_scalar::<_, i64>(
        "SELECT transaction_id FROM issue_metadata_sync WHERE issue_id = $1",
    )
    .bind(issue_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(marked_update.transaction_id, synced_transaction_id);
    assert_eq!(marked_update.issue.version, 3);

    sqlx::query(
        "UPDATE document_loro_snapshots
         SET frontiers = $2
         WHERE document_id = $1",
    )
    .bind(description.id)
    .bind(serde_json::json!([{"peer_id": 1, "counter": 2}]))
    .execute(database.pool())
    .await
    .unwrap();
    let changed = database.get_issue(project_id, issue_id).await.unwrap();
    assert!(changed.specification_changed_since_review);
    assert_eq!(updated.version, 2);

    let stale = database
        .update_issue(
            project_id,
            issue_id,
            IssueUpdate {
                expected_version: 1,
                title: Some("stale write".to_owned()),
                ..IssueUpdate::default()
            },
            actor_id,
        )
        .await;
    assert!(matches!(stale, Err(Error::VersionConflict)));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn subissues_preserve_parent_relationship_and_same_project_boundary() {
    let database = database().await;
    let (project_id, actor_id) = project(&database).await;
    let parent_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-PARENT", "parent issue"),
            actor_id,
        )
        .await
        .unwrap();
    let child_id = Uuid::now_v7();
    database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: child_id,
                display_key: "RII-CHILD".to_owned(),
                title: "child issue".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: false,
                spec_complete: false,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: Some(parent_id),
            },
            actor_id,
        )
        .await
        .unwrap();

    let parent = database.get_issue(project_id, parent_id).await.unwrap();
    assert_eq!(parent.parent_issue_id, None);
    assert_eq!(parent.children.len(), 1);
    assert_eq!(parent.children[0].id, child_id);

    let grandchild_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-GRANDCHILD".to_owned(),
                title: "grandchild issue".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: false,
                spec_complete: false,
                rank: 1,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: Some(child_id),
            },
            actor_id,
        )
        .await
        .unwrap();
    let child = database.get_issue(project_id, child_id).await.unwrap();
    assert_eq!(
        child
            .children
            .iter()
            .map(|child| child.id)
            .collect::<Vec<_>>(),
        vec![grandchild_id]
    );

    let other_project_id = Uuid::now_v7();
    database
        .create_project(other_project_id, "other project")
        .await
        .unwrap();
    let cross_project = database
        .create_issue_with_metadata(
            other_project_id,
            IssueCreate {
                parent_issue_id: Some(parent_id),
                ..IssueCreate::minimal(Uuid::now_v7(), "RII-CROSS", "cross project")
            },
            actor_id,
        )
        .await;
    assert!(matches!(cross_project, Err(Error::InvalidIssue(_))));

    let self_parent = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                parent_issue_id: Some(parent_id),
                ..IssueCreate::minimal(parent_id, "RII-SELF", "self parent")
            },
            actor_id,
        )
        .await;
    assert!(self_parent.is_err());
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn blocking_edges_update_counts_and_reject_cycles() {
    let database = database().await;
    let (project_id, actor_id) = project(&database).await;
    let blocker = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-BLOCKER", "blocker"),
            actor_id,
        )
        .await
        .unwrap();
    let blocked = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-BLOCKED", "blocked"),
            actor_id,
        )
        .await
        .unwrap();

    database
        .create_issue_edge(project_id, blocker, blocked, "blocks", actor_id)
        .await
        .unwrap();
    let issue = database.get_issue(project_id, blocked).await.unwrap();
    assert_eq!(issue.unresolved_blocker_count, 1);

    let cycle = database
        .create_issue_edge(project_id, blocked, blocker, "blocks", actor_id)
        .await;
    assert!(matches!(cycle, Err(Error::EdgeCycle)));

    database
        .remove_issue_edge(project_id, issue.edges[0].id, actor_id)
        .await
        .unwrap();
    assert_eq!(
        database
            .get_issue(project_id, blocked)
            .await
            .unwrap()
            .unresolved_blocker_count,
        0
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn holds_are_counted_and_release_is_idempotently_rejected() {
    let database = database().await;
    let (project_id, actor_id) = project(&database).await;
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-HOLD", "hold me"),
            actor_id,
        )
        .await
        .unwrap();

    let hold_id = database
        .create_hold(
            project_id,
            issue_id,
            "needs_spec",
            "waiting for acceptance criteria",
            actor_id,
            Some(Duration::hours(1)),
        )
        .await
        .unwrap();
    assert_eq!(
        database
            .get_issue(project_id, issue_id)
            .await
            .unwrap()
            .active_hold_count,
        1
    );
    database
        .release_hold(project_id, hold_id, actor_id)
        .await
        .unwrap();
    let second_release = database.release_hold(project_id, hold_id, actor_id).await;
    assert!(matches!(second_release, Err(Error::HoldNotFound)));
}
