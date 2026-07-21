mod support;

use chrono::Duration;
use riichi_persistence::{Action, Database, Error, NewIssue, OutboxRetryOutcome, Report};
use serde_json::Value;
use sqlx::Row;
use support::PostgresHarness;
use uuid::Uuid;

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

async fn fixture(database: &Database, session_count: usize) -> (Uuid, Vec<Uuid>) {
    let project_id = Uuid::now_v7();
    let role_id = Uuid::now_v7();
    database
        .create_project(project_id, "dispatch test project")
        .await
        .unwrap();
    database
        .create_agent_role(role_id, project_id, "implementation")
        .await
        .unwrap();

    let mut sessions = Vec::with_capacity(session_count);
    for _ in 0..session_count {
        let session_id = Uuid::now_v7();
        database
            .create_session(
                session_id,
                project_id,
                role_id,
                Duration::hours(1),
                &format!("dispatch-token-{session_id}"),
            )
            .await
            .unwrap();
        sessions.push(session_id);
    }
    (project_id, sessions)
}

async fn ready_issue(database: &Database, project_id: Uuid, key: &str, title: &str) -> Uuid {
    let issue_id = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: issue_id,
            project_id,
            display_key: key.to_owned(),
            title: title.to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 0,
        })
        .await
        .unwrap();
    issue_id
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn sessions_cannot_cross_project_role_boundaries() {
    let database = database().await;
    let first_project = Uuid::now_v7();
    let second_project = Uuid::now_v7();
    let role_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    database
        .create_project(first_project, "first project")
        .await
        .unwrap();
    database
        .create_project(second_project, "second project")
        .await
        .unwrap();
    let first_team: Uuid =
        sqlx::query_scalar("SELECT team_id FROM project_teams WHERE project_id = $1 LIMIT 1")
            .bind(first_project)
            .fetch_one(database.pool())
            .await
            .unwrap();
    sqlx::query("UPDATE project_teams SET team_id = $2 WHERE project_id = $1")
        .bind(second_project)
        .bind(first_team)
        .execute(database.pool())
        .await
        .unwrap();
    database
        .create_agent_role(role_id, first_project, "first role")
        .await
        .unwrap();

    let result = database
        .create_session(
            session_id,
            second_project,
            role_id,
            Duration::hours(1),
            "cross-project-token",
        )
        .await;
    assert!(matches!(result, Err(Error::AgentRoleNotFound)));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn onboarding_claim_has_one_winner_per_project() {
    let database = database().await;
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "onboarding claim project")
        .await
        .unwrap();

    assert!(database.claim_onboarding_sample(project_id).await.unwrap());
    assert!(matches!(
        database.claim_onboarding_sample(project_id).await,
        Err(Error::Contended)
    ));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn team_agent_roster_preserves_team_and_project_context() {
    let database = database().await;
    let (project_id, session_ids) = fixture(&database, 1).await;
    let team_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();
    let account_id = database
        .upsert_human_account(
            "https://idp.example.test",
            "agent-roster-viewer",
            None,
            Some("Agent roster viewer"),
        )
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO organization_memberships (organization_id, account_id, role)
         VALUES ('00000000-0000-0000-0000-000000000001', $1, 'member')",
    )
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO team_memberships (team_id, account_id, role)
         VALUES ($1, $2, 'viewer')",
    )
    .bind(team_id)
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();

    let roles = database.agent_roster_for_team(team_id).await.unwrap();
    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].project_id, project_id);
    assert_eq!(roles[0].team_id, team_id);

    let sessions = database
        .agent_sessions_for_team(team_id, None)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].project_id, project_id);
    assert_eq!(sessions[0].team_id, team_id);

    let (active_session_count, synced_sessions): (i64, Value) = sqlx::query_as(
        "SELECT active_session_count, sessions
         FROM human_agent_sync
         WHERE account_id = $1 AND team_id = $2 AND agent_role_id = $3",
    )
    .bind(account_id)
    .bind(team_id)
    .bind(roles[0].id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(active_session_count, 1);
    assert_eq!(synced_sessions.as_array().unwrap().len(), 1);
    assert_eq!(synced_sessions[0]["id"], session_ids[0].to_string());

    database
        .revoke_agent_session(project_id, session_ids[0], account_id)
        .await
        .unwrap();
    let active_after_revoke: i64 = sqlx::query_scalar(
        "SELECT active_session_count FROM human_agent_sync
         WHERE account_id = $1 AND team_id = $2 AND agent_role_id = $3",
    )
    .bind(account_id)
    .bind(team_id)
    .bind(roles[0].id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(active_after_revoke, 0);

    sqlx::query(
        "UPDATE team_memberships SET revoked_at = now()
         WHERE team_id = $1 AND account_id = $2",
    )
    .bind(team_id)
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    let visible_after_revoke: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM human_agent_sync
         WHERE account_id = $1 AND team_id = $2",
    )
    .bind(account_id)
    .bind(team_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(visible_after_revoke, 0);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn human_queue_is_project_scoped_and_includes_active_lease_state() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let other_project_id = Uuid::now_v7();
    database
        .create_project(other_project_id, "other project")
        .await
        .unwrap();

    let issue_id = ready_issue(&database, project_id, "RII-QUEUE-1", "visible issue").await;
    let other_issue_id =
        ready_issue(&database, other_project_id, "RII-QUEUE-2", "hidden issue").await;
    let claim = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "human-queue-claim",
        )
        .await
        .unwrap();

    let issues = database.human_queue(project_id, 200).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, issue_id);
    assert_eq!(issues[0].display_key, "RII-1");
    assert_eq!(issues[0].active_lease_id, Some(claim.lease_id));
    assert!(issues.iter().all(|issue| issue.id != other_issue_id));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn concurrent_claims_have_one_winner() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 2).await;
    let issue_id = ready_issue(&database, project_id, "RII-1", "claim me once").await;

    let first = database.clone();
    let second = database.clone();
    let first_session = sessions[0];
    let second_session = sessions[1];
    let (first_result, second_result) = tokio::join!(
        first.claim(
            project_id,
            first_session,
            issue_id,
            Duration::minutes(30),
            "claim-a",
        ),
        second.claim(
            project_id,
            second_session,
            issue_id,
            Duration::minutes(30),
            "claim-b",
        ),
    );

    let successes = [first_result, second_result]
        .into_iter()
        .filter(Result::is_ok)
        .count();
    assert_eq!(successes, 1);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn concurrent_retries_with_the_same_idempotency_key_replay_one_result() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-IDEMPOTENT", "retry me safely").await;
    let first = database.clone();
    let second = database.clone();
    let session_id = sessions[0];

    let (first_result, second_result) = tokio::join!(
        first.claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(30),
            "same-request",
        ),
        second.claim(
            project_id,
            session_id,
            issue_id,
            Duration::minutes(30),
            "same-request",
        ),
    );

    let first_claim = first_result.unwrap();
    let second_claim = second_result.unwrap();
    assert_eq!(first_claim.lease_id, second_claim.lease_id);
    assert_eq!(first_claim.fencing_token, second_claim.fencing_token);
    let active_leases: i64 =
        sqlx::query_scalar("SELECT count(*) FROM leases WHERE issue_id = $1 AND state = 'active'")
            .bind(issue_id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(active_leases, 1);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn expired_lease_can_be_reclaimed_and_old_report_is_rejected() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 2).await;
    let issue_id = ready_issue(&database, project_id, "RII-2", "recover me").await;

    let old_claim = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::seconds(1),
            "claim-old",
        )
        .await
        .unwrap();
    sqlx::query("UPDATE leases SET expires_at = now() - interval '1 second' WHERE id = $1")
        .bind(old_claim.lease_id)
        .execute(database.pool())
        .await
        .unwrap();

    let new_claim = database
        .claim(
            project_id,
            sessions[1],
            issue_id,
            Duration::minutes(30),
            "claim-new",
        )
        .await
        .unwrap();
    assert!(new_claim.fencing_token > old_claim.fencing_token);

    let old_report = database
        .report(
            project_id,
            sessions[0],
            old_claim.lease_id,
            old_claim.fencing_token,
            Report {
                action: Action::Complete,
                comment: None,
                resolution_summary: Some("late result".to_owned()),
            },
        )
        .await;
    assert!(matches!(
        old_report,
        Err(Error::LeaseNotActive | Error::StaleLease)
    ));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn renewal_cannot_extend_past_session_lifetime() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-3", "renew me").await;
    let claim = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-renew",
        )
        .await
        .unwrap();

    let expires_at = database
        .renew(
            project_id,
            sessions[0],
            claim.lease_id,
            claim.fencing_token,
            Duration::hours(1),
        )
        .await
        .unwrap();
    let remaining = expires_at - chrono::Utc::now();
    assert!(remaining <= Duration::hours(1));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn completion_requires_summary_and_records_done_state() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-4", "complete me").await;
    let claim = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-complete",
        )
        .await
        .unwrap();

    let missing_summary = database
        .report(
            project_id,
            sessions[0],
            claim.lease_id,
            claim.fencing_token,
            Report {
                action: Action::Complete,
                comment: None,
                resolution_summary: None,
            },
        )
        .await;
    assert!(matches!(
        missing_summary,
        Err(Error::ResolutionSummaryRequired)
    ));

    database
        .report(
            project_id,
            sessions[0],
            claim.lease_id,
            claim.fencing_token,
            Report {
                action: Action::Complete,
                comment: Some("implementation finished".to_owned()),
                resolution_summary: Some("completed the pilot behavior".to_owned()),
            },
        )
        .await
        .unwrap();

    let row = sqlx::query("SELECT status FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_one(database.pool())
        .await
        .unwrap();
    let status: String = row.get("status");
    assert_eq!(status, "done");
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn matching_claim_retry_replays_and_mismatched_retry_conflicts() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-5", "retry me").await;

    let first = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-retry",
        )
        .await
        .unwrap();
    let replay = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-retry",
        )
        .await
        .unwrap();
    assert_eq!(first.lease_id, replay.lease_id);

    let conflict = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(29),
            "claim-retry",
        )
        .await;
    assert!(matches!(conflict, Err(Error::IdempotencyConflict)));
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn claim_publishes_outbox_message_that_can_be_acknowledged() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-6", "notify me").await;

    let claim = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-outbox",
        )
        .await
        .unwrap();
    let message = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(message.message_type, "lease_changed");
    let lease_id = claim.lease_id.to_string();
    assert_eq!(
        message.payload["lease_id"].as_str(),
        Some(lease_id.as_str())
    );

    database.deliver_outbox_event(message.id).await.unwrap();
    let pending_for_project: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM outbox_messages
         WHERE project_id = $1 AND delivered_at IS NULL",
    )
    .bind(project_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(pending_for_project, 0);
    let delivery_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM delivery_events WHERE id = $1")
            .bind(message.id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(delivery_count, 1);

    let second_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO outbox_messages (id, project_id, message_type, payload)
         VALUES ($1, $2, 'issue_changed', '{}'::jsonb)",
    )
    .bind(second_id)
    .bind(project_id)
    .execute(database.pool())
    .await
    .unwrap();
    let second = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();
    database.deliver_outbox_event(second.id).await.unwrap();

    let events = database.events_since(project_id, None, 100).await.unwrap();
    assert!(events.len() >= 2);
    assert!(events[0].event_seq < events[1].event_seq);
    let resumed = database
        .events_since(project_id, Some(events[0].event_seq), 100)
        .await
        .unwrap();
    assert_eq!(resumed[0].event_seq, events[1].event_seq);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn ready_filters_ineligible_work_and_orders_by_rank() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let ready_late = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: ready_late,
            project_id,
            display_key: "RII-10".to_owned(),
            title: "later ready work".to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 20,
        })
        .await
        .unwrap();
    let ready_early = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: ready_early,
            project_id,
            display_key: "RII-11".to_owned(),
            title: "earlier ready work".to_owned(),
            agent_eligible: true,
            spec_complete: true,
            rank: 10,
        })
        .await
        .unwrap();
    let not_agent_eligible = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: not_agent_eligible,
            project_id,
            display_key: "RII-12".to_owned(),
            title: "human-only work".to_owned(),
            agent_eligible: false,
            spec_complete: true,
            rank: 0,
        })
        .await
        .unwrap();
    let held = ready_issue(&database, project_id, "RII-13", "held work").await;
    sqlx::query(
        "INSERT INTO dispatch_holds (id, issue_id, hold_type, reason)
         VALUES ($1, $2, 'manual', 'waiting for review')",
    )
    .bind(Uuid::now_v7())
    .bind(held)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query("UPDATE issue_dispatch SET active_hold_count = 1 WHERE issue_id = $1")
        .bind(held)
        .execute(database.pool())
        .await
        .unwrap();

    let issues = database.ready(project_id, sessions[0], 20).await.unwrap();
    let keys: Vec<_> = issues
        .iter()
        .map(|issue| issue.display_key.as_str())
        .collect();
    assert_eq!(keys, vec!["RII-2", "RII-1"]);
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn ready_clamps_zero_and_large_limits() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    ready_issue(&database, project_id, "RII-14", "first").await;
    ready_issue(&database, project_id, "RII-15", "second").await;

    assert_eq!(
        database
            .ready(project_id, sessions[0], 0)
            .await
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        database
            .ready(project_id, sessions[0], 10_000)
            .await
            .unwrap()
            .len(),
        2
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn project_scope_prevents_cross_project_read_and_claim() {
    let database = database().await;
    let (project_a, sessions_a) = fixture(&database, 1).await;
    let (project_b, sessions_b) = fixture(&database, 1).await;
    let issue_a = ready_issue(&database, project_a, "RII-16", "private work").await;

    assert!(
        database
            .ready(project_b, sessions_b[0], 20)
            .await
            .unwrap()
            .is_empty()
    );
    let cross_project_claim = database
        .claim(
            project_b,
            sessions_b[0],
            issue_a,
            Duration::minutes(30),
            "cross-project",
        )
        .await;
    assert!(matches!(cross_project_claim, Err(Error::IssueNotFound)));

    assert_eq!(
        database
            .ready(project_a, sessions_a[0], 20)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn release_returns_issue_to_ready() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-17", "release me").await;
    let claim = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-release",
        )
        .await
        .unwrap();

    database
        .report(
            project_id,
            sessions[0],
            claim.lease_id,
            claim.fencing_token,
            Report {
                action: Action::Release,
                comment: Some("not enough context yet".to_owned()),
                resolution_summary: None,
            },
        )
        .await
        .unwrap();

    let issues = database.ready(project_id, sessions[0], 20).await.unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0].id, issue_id);
    assert_eq!(issues[0].status, "todo");
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn empty_idempotency_key_does_not_create_a_lease() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-18", "key required").await;

    let result = database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "  ",
        )
        .await;
    assert!(matches!(result, Err(Error::IdempotencyKeyRequired)));
    assert_eq!(
        database
            .ready(project_id, sessions[0], 20)
            .await
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn outbox_retry_makes_message_available_again() {
    let database = database().await;
    let (project_id, sessions) = fixture(&database, 1).await;
    let issue_id = ready_issue(&database, project_id, "RII-19", "retry delivery").await;
    database
        .claim(
            project_id,
            sessions[0],
            issue_id,
            Duration::minutes(30),
            "claim-retry-outbox",
        )
        .await
        .unwrap();

    let message = database
        .claim_next_outbox(Some(project_id))
        .await
        .unwrap()
        .unwrap();
    let retry = database
        .retry_outbox(
            message.id,
            "temporary delivery failure",
            Duration::seconds(1),
            5,
        )
        .await
        .unwrap();
    assert_eq!(retry, OutboxRetryOutcome::Scheduled);
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
