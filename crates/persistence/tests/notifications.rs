mod support;

use riichi_persistence::NewIssue;
use serde_json::json;
use support::PostgresHarness;
use uuid::Uuid;

/// Spin up a disposable Postgres container and return a connected database.
/// The container is leaked to keep it alive for the test's lifetime.
async fn database() -> PostgresHarness {
    PostgresHarness::start().await
}

#[tokio::test]
#[ignore = "starts a disposable PostgreSQL container"]
async fn comment_notifications_are_durable_private_and_readable_once() {
    let database = database().await;
    let project_id = Uuid::now_v7();
    database
        .create_project(project_id, "notification project")
        .await
        .unwrap();
    let author_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            None,
            None,
        )
        .await
        .unwrap();
    let recipient_id = database
        .upsert_human_account(
            "https://idp.example.test",
            &Uuid::now_v7().to_string(),
            None,
            None,
        )
        .await
        .unwrap();
    database
        .create_project_membership(project_id, author_id, "member")
        .await
        .unwrap();
    database
        .create_project_membership(project_id, recipient_id, "member")
        .await
        .unwrap();
    let issue_id = Uuid::now_v7();
    database
        .create_issue(NewIssue {
            id: issue_id,
            project_id,
            display_key: "RII-NOTIFY-1".to_owned(),
            title: "notification issue".to_owned(),
            agent_eligible: false,
            spec_complete: false,
            rank: 0,
        })
        .await
        .unwrap();

    database
        .create_human_comment(
            project_id,
            issue_id,
            author_id,
            "please review",
            json!({"type": "doc"}),
        )
        .await
        .unwrap();

    let visible_issue = database
        .human_get_issue(recipient_id, issue_id)
        .await
        .unwrap();
    assert_eq!(visible_issue.project_id, project_id);
    assert_eq!(visible_issue.id, issue_id);

    let inbox = database
        .notifications_for_account(recipient_id, false, 50)
        .await
        .unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].kind, "comment");
    assert_eq!(inbox[0].issue_id, Some(issue_id));
    assert_eq!(
        database
            .unread_notification_count(recipient_id)
            .await
            .unwrap(),
        1
    );

    assert!(
        database
            .mark_notification_read(recipient_id, inbox[0].id)
            .await
            .unwrap()
    );
    assert_eq!(
        database
            .unread_notification_count(recipient_id)
            .await
            .unwrap(),
        0
    );
    assert!(
        !database
            .mark_notification_read(author_id, inbox[0].id)
            .await
            .unwrap()
    );
}
