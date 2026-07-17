use super::*;

pub(super) async fn github_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<GithubWebhookResponse>, ApiError> {
    let secret =
        env::var("RIICHI_GITHUB_WEBHOOK_SECRET").map_err(|_| ApiError::GitHubNotConfigured)?;
    let project_id = env::var("RIICHI_GITHUB_PROJECT_ID")
        .map_err(|_| ApiError::GitHubNotConfigured)?
        .parse()
        .map_err(|_| ApiError::GitHubNotConfigured)?;
    let delivery_id = headers
        .get("x-github-delivery")
        .and_then(|value| value.to_str().ok())
        .ok_or(ApiError::GitHubWebhook(WebhookError::UnsupportedEvent))?;
    let event_name = headers
        .get("x-github-event")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let signature = headers
        .get("x-hub-signature-256")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let webhook =
        parse_issues_webhook(delivery_id, event_name, signature, secret.as_bytes(), &body)
            .map_err(ApiError::GitHubWebhook)?;
    let payload = serde_json::to_value(&webhook)
        .map_err(|_| ApiError::GitHubWebhook(WebhookError::Malformed))?;
    let accepted = state
        .application
        .record_github_delivery(
            &webhook.delivery_id,
            Some(project_id),
            "issues",
            &webhook.action,
            payload.clone(),
        )
        .await
        .map_err(ApiError::from)?;
    if accepted {
        state
            .application
            .upsert_github_snapshot(
                project_id,
                None,
                &webhook.repository,
                webhook.number,
                &webhook.html_url,
                &webhook.title,
                webhook.body.as_deref(),
                if webhook.action == "closed" {
                    "closed"
                } else {
                    "open"
                },
                webhook.updated_at.as_deref(),
                payload,
            )
            .await
            .map_err(ApiError::from)?;
    }
    Ok(Json(GithubWebhookResponse { accepted }))
}

pub(super) async fn import_github_issues(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<GithubImportRequest>,
) -> Result<Json<GithubImportResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let token = env::var("RIICHI_GITHUB_TOKEN").map_err(|_| ApiError::GitHubNotConfigured)?;
    let base_url = env::var("RIICHI_GITHUB_API_BASE_URL")
        .unwrap_or_else(|_| "https://api.github.com".to_owned());
    let client = GithubClient::new(base_url).map_err(ApiError::GitHubClient)?;
    let import = client
        .import_issues(
            &request.repository,
            &token,
            request.max_issues.unwrap_or(100),
        )
        .await
        .map_err(ApiError::GitHubClient)?;
    let issue_numbers: Vec<i64> = import.issues.iter().map(|issue| issue.number).collect();
    for issue in &import.issues {
        state
            .application
            .upsert_github_snapshot(
                project_id,
                None,
                &request.repository,
                issue.number,
                &issue.html_url,
                &issue.title,
                issue.body.as_deref(),
                &issue.state,
                issue.updated_at.as_deref(),
                json!({
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
            .map_err(ApiError::from)?;
    }
    Ok(Json(GithubImportResponse {
        repository: request.repository,
        imported: issue_numbers.len(),
        pull_requests_skipped: import.pull_requests_skipped,
        issue_numbers,
    }))
}
