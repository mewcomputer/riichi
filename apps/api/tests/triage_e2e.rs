mod support;

use chrono::Duration;
use riichi_persistence::{
    Database, IssueCreate, IssueUpdate, LoroSnapshotSeed, LoroUpdateOutcome, LoroUpdateSeed,
    ReportBatch, ReportOperation,
};
use sha2::{Digest, Sha256};
use support::PostgresHarness;
use uuid::Uuid;

async fn human_actor(database: &Database, subject: &str) -> Uuid {
    database
        .upsert_human_account(
            "https://idp.example.test",
            subject,
            None,
            Some("Triage e2e actor"),
        )
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn triage_primitives_round_trip_against_real_postgres() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "triage-primitives").await;
    database
        .create_project(project_id, "triage e2e project")
        .await
        .unwrap();

    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-E2E-1".to_owned(),
                title: "round trip triage".to_owned(),
                body: "real postgres data".to_owned(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 5,
                labels: vec!["e2e".to_owned()],
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let dependent_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-E2E-2", "dependent"),
            actor_id,
        )
        .await
        .unwrap();
    database
        .create_issue_edge(project_id, issue_id, dependent_id, "blocks", actor_id)
        .await
        .unwrap();
    let dependent = database.get_issue(project_id, dependent_id).await.unwrap();
    assert_eq!(dependent.unresolved_blocker_count, 1);

    let updated = database
        .update_issue(
            project_id,
            issue_id,
            IssueUpdate {
                expected_version: 1,
                status: Some("done".to_owned()),
                ..Default::default()
            },
            actor_id,
        )
        .await
        .unwrap();
    assert_eq!(updated.status, "done");
    assert_eq!(
        database
            .get_issue(project_id, dependent_id)
            .await
            .unwrap()
            .unresolved_blocker_count,
        0
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn context_is_bounded_provenance_aware_and_includes_prior_agent_activity() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "context").await;
    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_project(project_id, "context e2e project")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, actor_id, "owner")
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
            "context-e2e-token",
        )
        .await
        .unwrap();
    let roster = database.agent_roster(project_id).await.unwrap();
    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].capabilities[0], "comment");
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-CONTEXT-1".to_owned(),
                title: "bounded context".to_owned(),
                body: "x".repeat(20_000),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 0,
                labels: vec!["context".to_owned()],
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO comments (id, project_id, issue_id, author_id, role_id, session_id, body)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(issue_id)
    .bind(session_id)
    .bind(role_id)
    .bind(session_id)
    .bind("previous agent made progress before expiry")
    .execute(database.pool())
    .await
    .unwrap();

    let description = database
        .get_issue_description_document(actor_id, project_id, issue_id)
        .await
        .unwrap();
    let initial_frontiers = serde_json::json!([]);
    database
        .initialize_loro_snapshot(
            actor_id,
            LoroSnapshotSeed {
                document_id: description.id,
                source_revision: 1,
                schema_version: 1,
                frontiers: initial_frontiers.clone(),
                snapshot: vec![1],
            },
        )
        .await
        .unwrap();
    let payload = vec![2, 3, 4];
    let (_update, outcome) = database
        .accept_loro_update(
            actor_id,
            LoroUpdateSeed {
                update_id: Uuid::now_v7(),
                document_id: description.id,
                principal_id: actor_id,
                source: "human".to_owned(),
                peer_id: "context-peer".to_owned(),
                idempotency_key: Some("context-edit".to_owned()),
                previous_frontiers: initial_frontiers.clone(),
                resulting_frontiers: serde_json::json!([{"peer_id": 1, "counter": 1}]),
                payload: payload.clone(),
                payload_sha256: Sha256::digest(&payload).to_vec(),
                snapshot: vec![5],
                content: serde_json::json!({"type": "doc", "content": []}),
                plain_text: format!("current context description {}", "y".repeat(20_000)),
                sanitized_html: "<p>current context description</p>".to_owned(),
                references: Vec::new(),
            },
        )
        .await
        .unwrap();
    assert_eq!(outcome, LoroUpdateOutcome::Accepted);
    database
        .update_issue(
            project_id,
            issue_id,
            IssueUpdate {
                expected_version: 1,
                spec_complete: Some(true),
                ..IssueUpdate::default()
            },
            actor_id,
        )
        .await
        .unwrap();

    let context = database
        .context(project_id, session_id, issue_id, Some(2_048), None)
        .await
        .unwrap();
    assert_eq!(context.issue_id, issue_id);
    assert!(context.snapshot_cursor.contains("issue-v2"));
    assert!(context.sections.iter().any(|section| {
        section.name == "external_context"
            && section.omitted
            && section.trust_class == "external_untrusted"
    }));
    assert!(context.sections.iter().any(|section| {
        section.name == "prior_attempt"
            && section.trust_class == "agent_generated"
            && section
                .content
                .as_deref()
                .unwrap()
                .contains("previous agent")
    }));
    assert!(context.sections.iter().any(|section| section.truncated));
    assert!(
        context
            .sections
            .iter()
            .map(|section| section.byte_size)
            .sum::<usize>()
            <= 2_048
    );
    let description = database
        .context_resource(project_id, session_id, issue_id, "description")
        .await
        .unwrap();
    assert_eq!(description.name, "description");
    assert!(!description.omitted);
    let historical = database
        .context(
            project_id,
            session_id,
            issue_id,
            Some(2_048),
            Some(initial_frontiers),
        )
        .await
        .unwrap();
    assert_eq!(historical.document_frontiers, Some(serde_json::json!([])));
    assert!(historical.sections.iter().any(|section| {
        section.name == "description"
            && section
                .content
                .as_deref()
                .unwrap()
                .contains("bounded context")
    }));
    let unavailable = database
        .context(
            project_id,
            session_id,
            issue_id,
            Some(2_048),
            Some(serde_json::json!([{"peer_id": 99, "counter": 1}])),
        )
        .await;
    assert!(matches!(
        unavailable,
        Err(riichi_persistence::Error::DocumentFrontierUnavailable)
    ));

    let excluded_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-CONTEXT-2", "human-only work"),
            actor_id,
        )
        .await
        .unwrap();
    let snapshot = database
        .ready_snapshot(project_id, session_id, 20)
        .await
        .unwrap();
    assert!(snapshot.snapshot_cursor.starts_with("project-dispatch-v"));
    assert!(snapshot.issues.iter().any(|issue| issue.id == issue_id));
    let exclusion = snapshot
        .exclusions
        .iter()
        .find(|exclusion| exclusion.id == excluded_id)
        .unwrap();
    assert!(exclusion.reasons.contains(&"agent_ineligible".to_owned()));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn report_batch_commits_discovered_work_and_needs_spec_idempotently() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "report-batch").await;
    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_project(project_id, "report e2e project")
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
            "report-e2e-token",
        )
        .await
        .unwrap();
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-REPORT-1".to_owned(),
                title: "report work".to_owned(),
                body: "reportable task".to_owned(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let claim = database
        .claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(10),
            "report-claim",
        )
        .await
        .unwrap();
    let batch = ReportBatch {
        idempotency_key: "report-batch-1".to_owned(),
        operations: vec![
            ReportOperation::Comment {
                body: "implemented the core path".to_owned(),
            },
            ReportOperation::CreateDiscovered {
                display_key: "RII-REPORT-2".to_owned(),
                title: "follow-up discovered work".to_owned(),
                body: "needs human triage".to_owned(),
                rank: 4,
            },
            ReportOperation::Complete {
                resolution_summary: "completed the report path".to_owned(),
            },
        ],
    };
    let first = database
        .report_batch(
            project_id,
            session_id,
            claim.lease_id,
            claim.fencing_token,
            batch.clone(),
        )
        .await
        .unwrap();
    let replay = database
        .report_batch(
            project_id,
            session_id,
            claim.lease_id,
            claim.fencing_token,
            batch,
        )
        .await
        .unwrap();
    assert_eq!(first.created_issue_ids, replay.created_issue_ids);
    assert_eq!(first.applied_operations, 3);
    let completed = database.get_issue(project_id, issue_id).await.unwrap();
    assert_eq!(completed.status, "done");
    let discovered = database
        .get_issue(project_id, first.created_issue_ids[0])
        .await
        .unwrap();
    assert_eq!(discovered.status, "triage");
    assert!(
        discovered
            .edges
            .iter()
            .any(|edge| edge.edge_type == "discovered_from" && edge.source_issue_id == issue_id)
    );

    let needs_spec_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-REPORT-3".to_owned(),
                title: "needs specification".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 1,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let needs_spec_claim = database
        .claim(
            project_id,
            session_id,
            needs_spec_id,
            Duration::minutes(10),
            "report-claim-needs-spec",
        )
        .await
        .unwrap();
    database
        .report_batch(
            project_id,
            session_id,
            needs_spec_claim.lease_id,
            needs_spec_claim.fencing_token,
            ReportBatch {
                idempotency_key: "report-batch-needs-spec".to_owned(),
                operations: vec![ReportOperation::RequestSpec {
                    reason: "acceptance criteria are missing".to_owned(),
                }],
            },
        )
        .await
        .unwrap();
    let held = database.get_issue(project_id, needs_spec_id).await.unwrap();
    assert_eq!(held.status, "blocked");
    assert_eq!(held.active_hold_count, 1);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn human_takeover_recovery_and_approval_are_versioned_and_auditable() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "takeover").await;
    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_project(project_id, "controls e2e project")
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
            "controls-e2e-token",
        )
        .await
        .unwrap();
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-CONTROLS-1".to_owned(),
                title: "recover me".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let claim = database
        .claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(10),
            "controls-claim",
        )
        .await
        .unwrap();
    let checklist = database
        .takeover_issue(
            project_id,
            issue_id,
            actor_id,
            "agent session stopped responding",
        )
        .await
        .unwrap();
    assert_eq!(checklist.state, "open");
    let taken_over = database.get_issue(project_id, issue_id).await.unwrap();
    assert_eq!(taken_over.active_lease_id, None);
    assert_eq!(taken_over.status, "in_progress");
    let old_lease_state: String = sqlx::query_scalar("SELECT state FROM leases WHERE id = $1")
        .bind(claim.lease_id)
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(old_lease_state, "revoked");

    let recovered = database
        .complete_recovery(
            project_id,
            checklist.id,
            actor_id,
            taken_over.version,
            "release",
            None,
        )
        .await
        .unwrap();
    assert_eq!(recovered.status, "todo");
    let approval = database
        .create_approval_request(
            project_id,
            issue_id,
            actor_id,
            recovered.version,
            serde_json::json!({ "operation": "set_rank", "rank": 3 }),
            Duration::hours(1),
        )
        .await
        .unwrap();
    assert_eq!(approval.state, "pending");
    assert_eq!(approval.target_version, recovered.version);
    let approved = database
        .decide_approval_request(project_id, approval.id, actor_id, true)
        .await
        .unwrap();
    assert_eq!(approved.state, "approved");
    assert_eq!(approved.proposed_operation["operation"], "set_rank");
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn agent_revocation_and_expiry_revoke_leases_and_preserve_recovery_state() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "revocation").await;
    database
        .create_project(project_id, "session controls e2e project")
        .await
        .unwrap();

    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
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
            "session-controls-token",
        )
        .await
        .unwrap();
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-SESSIONS-1".to_owned(),
                title: "session revoke me".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let claim = database
        .claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(10),
            "session-revoke-claim",
        )
        .await
        .unwrap();
    assert!(
        database
            .authenticate_agent_session(project_id, session_id, "session-controls-token")
            .await
            .unwrap()
    );

    database
        .revoke_agent_session(project_id, session_id, actor_id)
        .await
        .unwrap();
    assert!(
        !database
            .authenticate_agent_session(project_id, session_id, "session-controls-token")
            .await
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM leases WHERE id = $1")
            .bind(claim.lease_id)
            .fetch_one(database.pool())
            .await
            .unwrap(),
        "revoked"
    );
    let recovered_issue = database.get_issue(project_id, issue_id).await.unwrap();
    assert_eq!(recovered_issue.status, "todo");
    assert_eq!(recovered_issue.active_lease_id, None);

    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_agent_role(role_id, project_id, "review")
        .await
        .unwrap();
    database
        .create_session(
            session_id,
            project_id,
            role_id,
            Duration::hours(1),
            "role-controls-token",
        )
        .await
        .unwrap();
    let role_issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-SESSIONS-2".to_owned(),
                title: "role revoke me".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 1,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let role_claim = database
        .claim(
            project_id,
            session_id,
            role_issue_id,
            Duration::minutes(10),
            "role-revoke-claim",
        )
        .await
        .unwrap();
    database
        .revoke_agent_role(project_id, role_id, actor_id)
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM leases WHERE id = $1")
            .bind(role_claim.lease_id)
            .fetch_one(database.pool())
            .await
            .unwrap(),
        "revoked"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(database.pool())
            .await
            .unwrap(),
        "revoked"
    );
    assert!(matches!(
        database
            .create_session(
                Uuid::now_v7(),
                project_id,
                role_id,
                Duration::hours(1),
                "revoked-role-token",
            )
            .await,
        Err(riichi_persistence::Error::AgentRoleNotFound)
    ));

    let expiry_role_id = Uuid::now_v7();
    let expiry_session_id = Uuid::now_v7();
    database
        .create_agent_role(expiry_role_id, project_id, "expiry")
        .await
        .unwrap();
    database
        .create_session(
            expiry_session_id,
            project_id,
            expiry_role_id,
            Duration::hours(1),
            "expiry-controls-token",
        )
        .await
        .unwrap();
    let expiry_issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-SESSIONS-3".to_owned(),
                title: "expiry sweep me".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 2,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let expiry_claim = database
        .claim(
            project_id,
            expiry_session_id,
            expiry_issue_id,
            Duration::minutes(10),
            "expiry-claim",
        )
        .await
        .unwrap();
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(expiry_claim.lease_id)
        .execute(database.pool())
        .await
        .unwrap();
    assert_eq!(database.sweep_expired_leases().await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT state FROM leases WHERE id = $1")
            .bind(expiry_claim.lease_id)
            .fetch_one(database.pool())
            .await
            .unwrap(),
        "expired"
    );
    assert_eq!(
        database
            .get_issue(project_id, expiry_issue_id)
            .await
            .unwrap()
            .status,
        "todo"
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn stale_reports_are_quarantined_without_mutating_authoritative_issue_state() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "quarantine").await;
    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_project(project_id, "quarantine e2e project")
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
            "quarantine-e2e-token",
        )
        .await
        .unwrap();
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-QUARANTINE-1".to_owned(),
                title: "reject stale report".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let claim = database
        .claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(10),
            "quarantine-claim",
        )
        .await
        .unwrap();
    let result = database
        .report_batch(
            project_id,
            session_id,
            claim.lease_id,
            claim.fencing_token + 1,
            ReportBatch {
                idempotency_key: "quarantine-report".to_owned(),
                operations: vec![ReportOperation::Comment {
                    body: "this must not be applied".to_owned(),
                }],
            },
        )
        .await;
    assert!(matches!(result, Err(riichi_persistence::Error::StaleLease)));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM comments WHERE project_id = $1 AND issue_id = $2",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0
    );
    let quarantined = database
        .quarantined_attempts(project_id, issue_id, 100)
        .await
        .unwrap();
    assert_eq!(quarantined.len(), 1);
    assert_eq!(quarantined[0].reason, "stale_lease");
    assert_eq!(quarantined[0].payload["operations"][0]["type"], "comment");
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn collaborator_reports_require_explicit_automatic_capabilities() {
    let harness = PostgresHarness::start().await;
    let database = &harness.database;
    let project_id = Uuid::now_v7();
    let actor_id = human_actor(database, "collaborator").await;
    let owner_role_id = Uuid::now_v7();
    let owner_session_id = Uuid::now_v7();
    let collaborator_role_id = Uuid::now_v7();
    let collaborator_session_id = Uuid::now_v7();
    database
        .create_project(project_id, "collaborator e2e project")
        .await
        .unwrap();
    database
        .create_agent_role(owner_role_id, project_id, "implementation")
        .await
        .unwrap();
    database
        .create_session(
            owner_session_id,
            project_id,
            owner_role_id,
            Duration::hours(1),
            "owner-collaborator-token",
        )
        .await
        .unwrap();
    database
        .create_agent_role(collaborator_role_id, project_id, "review")
        .await
        .unwrap();
    database
        .create_session(
            collaborator_session_id,
            project_id,
            collaborator_role_id,
            Duration::hours(1),
            "collaborator-token",
        )
        .await
        .unwrap();
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-COLLABORATOR-1".to_owned(),
                title: "collaborate safely".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: true,
                spec_complete: true,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            actor_id,
        )
        .await
        .unwrap();
    let claim = database
        .claim(
            project_id,
            owner_session_id,
            issue_id,
            Duration::minutes(10),
            "collaborator-claim",
        )
        .await
        .unwrap();
    database
        .grant_lease_collaborator(
            project_id,
            issue_id,
            claim.lease_id,
            collaborator_session_id,
            "comment",
            "auto",
            actor_id,
            None,
        )
        .await
        .unwrap();
    let comment_result = database
        .report_batch(
            project_id,
            collaborator_session_id,
            claim.lease_id,
            claim.fencing_token,
            ReportBatch {
                idempotency_key: "collaborator-comment".to_owned(),
                operations: vec![ReportOperation::Comment {
                    body: "reviewer progress".to_owned(),
                }],
            },
        )
        .await
        .unwrap();
    assert_eq!(comment_result.applied_operations, 1);
    assert_eq!(
        sqlx::query_scalar::<_, Uuid>(
            "SELECT author_id FROM comments WHERE issue_id = $1 ORDER BY created_at DESC LIMIT 1",
        )
        .bind(issue_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        collaborator_session_id
    );

    database
        .grant_lease_collaborator(
            project_id,
            issue_id,
            claim.lease_id,
            collaborator_session_id,
            "complete",
            "approval_required",
            actor_id,
            None,
        )
        .await
        .unwrap();
    let approval_required = database
        .report_batch(
            project_id,
            collaborator_session_id,
            claim.lease_id,
            claim.fencing_token,
            ReportBatch {
                idempotency_key: "collaborator-complete".to_owned(),
                operations: vec![ReportOperation::Complete {
                    resolution_summary: "should require approval".to_owned(),
                }],
            },
        )
        .await;
    assert!(matches!(
        approval_required,
        Err(riichi_persistence::Error::CapabilityDenied)
    ));
    assert_eq!(
        database
            .get_issue(project_id, issue_id)
            .await
            .unwrap()
            .status,
        "in_progress"
    );

    database
        .revoke_lease_collaborator(
            project_id,
            issue_id,
            claim.lease_id,
            collaborator_session_id,
            "comment",
            actor_id,
        )
        .await
        .unwrap();
    let revoked = database
        .report_batch(
            project_id,
            collaborator_session_id,
            claim.lease_id,
            claim.fencing_token,
            ReportBatch {
                idempotency_key: "collaborator-comment-after-revoke".to_owned(),
                operations: vec![ReportOperation::Comment {
                    body: "must be denied".to_owned(),
                }],
            },
        )
        .await;
    assert!(matches!(
        revoked,
        Err(riichi_persistence::Error::CapabilityDenied)
    ));

    database
        .grant_lease_collaborator(
            project_id,
            issue_id,
            claim.lease_id,
            collaborator_session_id,
            "recovery_review",
            "approval_required",
            actor_id,
            None,
        )
        .await
        .unwrap();
    assert!(
        database
            .quarantined_attempts_for_agent(project_id, collaborator_session_id, issue_id)
            .await
            .unwrap()
            .is_empty()
    );
}
