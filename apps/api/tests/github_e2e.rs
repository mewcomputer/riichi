mod support;

use axum::{
    Json, Router,
    extract::Query,
    http::{HeaderMap, StatusCode, header},
    routing::get,
};
use riichi_integrations_github::GithubClient;
use riichi_persistence::{Database, IssueCreate};
use std::collections::HashMap;
use support::PostgresHarness;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn github_deliveries_are_deduplicated_and_snapshots_are_project_scoped() {
    let harness = PostgresHarness::start().await;
    let database: &Database = &harness.database;
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "github e2e project")
        .await
        .unwrap();
    let actor_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &format!("github-e2e-{project_id}"),
            None,
            Some("GitHub e2e actor"),
        )
        .await
        .unwrap();
    let issue_id = database
        .create_issue_with_metadata(
            project_id,
            IssueCreate::minimal(Uuid::now_v7(), "RII-GITHUB-1", "linked issue"),
            actor_id,
        )
        .await
        .unwrap();

    let payload = serde_json::json!({
        "repository": "natalie/riichi",
        "number": 42,
        "title": "external issue",
        "trust": "external_untrusted"
    });
    assert!(
        database
            .record_github_delivery(
                "github-delivery-1",
                Some(project_id),
                "issues",
                "edited",
                payload.clone(),
            )
            .await
            .unwrap()
    );
    assert!(
        !database
            .record_github_delivery(
                "github-delivery-1",
                Some(project_id),
                "issues",
                "edited",
                payload.clone(),
            )
            .await
            .unwrap()
    );

    let snapshot = database
        .upsert_github_snapshot(
            project_id,
            Some(issue_id),
            "natalie/riichi",
            42,
            "https://github.com/natalie/riichi/issues/42",
            "external issue",
            Some("external text"),
            "open",
            Some("2026-07-12T00:00:00Z"),
            payload,
        )
        .await
        .unwrap();
    assert_eq!(snapshot.issue_id, Some(issue_id));
    assert_eq!(snapshot.external_id, "natalie/riichi#42");
    assert_eq!(snapshot.state, "open");
    assert_eq!(snapshot.payload["trust"], "external_untrusted");
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn github_import_filters_pull_requests_before_storing_untrusted_snapshots() {
    async fn issues(
        Query(query): Query<HashMap<String, String>>,
        headers: HeaderMap,
    ) -> Result<Json<serde_json::Value>, StatusCode> {
        if headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            != Some("Bearer import-token")
        {
            return Err(StatusCode::UNAUTHORIZED);
        }
        Ok(Json(match query.get("page").map(String::as_str) {
            Some("1") => serde_json::json!([
                {
                    "number": 7,
                    "title": "import me",
                    "body": "external body",
                    "html_url": "https://github.com/acme/riichi/issues/7",
                    "state": "open",
                    "updated_at": "2026-07-12T00:00:00Z"
                },
                {
                    "number": 8,
                    "title": "ignore pull request",
                    "body": null,
                    "html_url": "https://github.com/acme/riichi/pull/8",
                    "state": "open",
                    "updated_at": null,
                    "pull_request": {"url": "https://api.github.com/pulls/8"}
                }
            ]),
            _ => serde_json::json!([]),
        }))
    }

    let harness = PostgresHarness::start().await;
    let database: &Database = &harness.database;
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "github import e2e project")
        .await
        .unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/repos/acme/riichi/issues", get(issues)),
        )
        .await
        .unwrap();
    });
    let client = GithubClient::new(format!("http://{address}")).unwrap();
    let imported = client
        .import_issues("acme/riichi", "import-token", 10)
        .await
        .unwrap();
    for issue in &imported.issues {
        database
            .upsert_github_snapshot(
                project_id,
                None,
                "acme/riichi",
                issue.number,
                &issue.html_url,
                &issue.title,
                issue.body.as_deref(),
                &issue.state,
                issue.updated_at.as_deref(),
                serde_json::json!({
                    "number": issue.number,
                    "title": issue.title,
                    "body": issue.body,
                    "html_url": issue.html_url,
                    "state": issue.state,
                    "updated_at": issue.updated_at,
                    "trust": "external_untrusted"
                }),
            )
            .await
            .unwrap();
    }
    server.abort();

    assert_eq!(imported.issues.len(), 1);
    assert_eq!(imported.pull_requests_skipped, 1);
    let external_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM external_links WHERE project_id = $1 AND provider = 'github'",
    )
    .bind(project_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(external_count, 1);
    let trust: String = sqlx::query_scalar(
        "SELECT s.payload ->> 'trust'
         FROM external_issue_snapshots s
         JOIN external_links l ON l.id = s.external_link_id
         WHERE l.project_id = $1",
    )
    .bind(project_id)
    .fetch_one(database.pool())
    .await
    .unwrap();
    assert_eq!(trust, "external_untrusted");
}
