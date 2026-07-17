mod support;

use riichi_persistence::{Database, IssueCreate};
use support::PostgresHarness;
use uuid::Uuid;

async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn navigation_sync_tracks_membership_and_resource_changes() {
    let harness = database().await;
    let database: &Database = &harness;
    let account_id = database
        .upsert_human_account(
            "https://idp.example.test",
            "navigation-test-account",
            None,
            Some("Navigation Test"),
        )
        .await
        .unwrap();
    let project_id = Uuid::now_v7();
    database
        .create_human_project(project_id, "Navigation project", account_id)
        .await
        .unwrap();

    let row = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT organization_name, team_name, team_key, project_name, project_role
         FROM navigation_sync
         WHERE account_id = $1 AND project_id = $2",
    )
    .bind(account_id)
    .bind(project_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(row.0, "Riichi");
    assert_eq!(row.1, "Riichi");
    assert_eq!(row.2, "RII");
    assert_eq!(row.3, "Navigation project");
    assert_eq!(row.4, "admin");

    sqlx::query("UPDATE projects SET name = 'Renamed navigation project' WHERE id = $1")
        .bind(project_id)
        .execute(database.pool())
        .await
        .unwrap();
    let renamed = sqlx::query_scalar::<_, String>(
        "SELECT project_name FROM navigation_sync WHERE account_id = $1 AND project_id = $2",
    )
    .bind(account_id)
    .bind(project_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(renamed, "Renamed navigation project");

    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate {
                id: Uuid::now_v7(),
                display_key: "RII-APPROVAL-1".to_owned(),
                title: "Review navigation approval".to_owned(),
                body: String::new(),
                status: "todo".to_owned(),
                agent_eligible: false,
                spec_complete: false,
                rank: 0,
                labels: Vec::new(),
                assignee_account_id: None,
                parent_issue_id: None,
            },
            account_id,
        )
        .await
        .unwrap();
    let approval_id = Uuid::now_v7();
    let issue_row = sqlx::query_as::<_, (Uuid, String, String, String, String, i64, i64)>(
        "SELECT issue_id, project_name, team_key, title, body, rank, transaction_id
         FROM human_issue_sync
         WHERE account_id = $1 AND issue_id = $2",
    )
    .bind(account_id)
    .bind(issue_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(issue_row.0, issue_id);
    assert_eq!(issue_row.1, "Renamed navigation project");
    assert_eq!(issue_row.2, "RII");
    assert_eq!(issue_row.3, "Review navigation approval");
    assert_eq!(issue_row.4, "");
    assert_eq!(issue_row.5, 0);
    assert!(issue_row.6 > 0);

    sqlx::query("UPDATE issues SET title = 'Renamed issue' WHERE id = $1")
        .bind(issue_id)
        .execute(database.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE issue_dispatch SET rank = 5 WHERE issue_id = $1")
        .bind(issue_id)
        .execute(database.pool())
        .await
        .unwrap();
    let refreshed = sqlx::query_as::<_, (String, i64)>(
        "SELECT title, rank FROM human_issue_sync WHERE account_id = $1 AND issue_id = $2",
    )
    .bind(account_id)
    .bind(issue_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(refreshed, ("Renamed issue".to_owned(), 5));

    sqlx::query(
        "INSERT INTO approval_requests
         (id, project_id, issue_id, requested_by, target_version, proposed_operation, state, expires_at)
         VALUES ($1, $2, $3, $4, 1, '{}'::jsonb, 'pending', now() + interval '1 hour')",
    )
    .bind(approval_id)
    .bind(project_id)
    .bind(issue_id)
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM approval_sync WHERE account_id = $1 AND id = $2",
        )
        .bind(account_id)
        .bind(approval_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );
    sqlx::query("UPDATE approval_requests SET state = 'approved' WHERE id = $1")
        .bind(approval_id)
        .execute(database.pool())
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM approval_sync WHERE account_id = $1 AND id = $2",
        )
        .bind(account_id)
        .bind(approval_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0
    );

    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM human_issue_sync WHERE account_id = $1 AND issue_id = $2",
        )
        .bind(account_id)
        .bind(issue_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        1
    );

    let second_approval_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO approval_requests
         (id, project_id, issue_id, requested_by, target_version, proposed_operation, state, expires_at)
         VALUES ($1, $2, $3, $4, 1, '{}'::jsonb, 'pending', now() + interval '1 hour')",
    )
    .bind(second_approval_id)
    .bind(project_id)
    .bind(issue_id)
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    sqlx::query(
        "UPDATE project_memberships SET revoked_at = now()
         WHERE project_id = $1 AND account_id = $2",
    )
    .bind(project_id)
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM approval_sync WHERE account_id = $1 AND id = $2",
        )
        .bind(account_id)
        .bind(second_approval_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0
    );

    sqlx::query(
        "UPDATE team_memberships
         SET revoked_at = now()
         WHERE account_id = $1 AND team_id = '00000000-0000-0000-0000-000000000002'",
    )
    .bind(account_id)
    .execute(database.pool())
    .await
    .unwrap();
    let remaining =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM navigation_sync WHERE account_id = $1")
            .bind(account_id)
            .fetch_one(database.pool())
            .await
            .unwrap();
    assert_eq!(remaining, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM human_issue_sync WHERE account_id = $1",
        )
        .bind(account_id)
        .fetch_one(database.pool())
        .await
        .unwrap(),
        0
    );
}
