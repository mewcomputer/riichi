use super::*;

#[derive(serde::Serialize)]
pub(super) struct GithubPullRequestsResponse {
    pub pull_requests: Vec<riichi_persistence::GithubPullRequest>,
    pub truncated: bool,
}

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
    let configured = state
        .application
        .database()
        .github_project_integration(project_id)
        .await
        .map_err(ApiError::from)?
        .filter(|integration| integration.enabled);
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
    if configured
        .as_ref()
        .is_none_or(|integration| integration.repository != webhook.repository)
    {
        return Err(ApiError::GitHubNotConfigured);
    }
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

pub(super) async fn get_github_integration(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<Option<riichi_persistence::GithubProjectIntegration>>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    Ok(Json(
        state
            .application
            .database()
            .github_project_integration(project_id)
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn set_github_integration(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<GithubIntegrationRequest>,
) -> Result<Json<riichi_persistence::GithubProjectIntegration>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    if request.repository.trim().is_empty() || request.repository.chars().count() > 200 {
        return Err(ApiError::InvalidRequest);
    }
    Ok(Json(
        state
            .application
            .database()
            .set_github_project_integration(
                project_id,
                principal.account.id,
                request.repository.trim(),
                request.enabled,
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

pub(super) async fn import_github_issues(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<GithubImportRequest>,
) -> Result<Json<GithubImportResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let integration = state
        .application
        .database()
        .github_project_integration(project_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::GitHubNotConfigured)?;
    if !integration.enabled || integration.repository != request.repository {
        return Err(ApiError::GitHubNotConfigured);
    }
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

pub(super) async fn refresh_github_pull_requests(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
    Json(request): Json<GithubPullRequestRefreshRequest>,
) -> Result<Json<GithubPullRequestRefreshResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_admin(&principal, project_id)?;
    let integration = state
        .application
        .database()
        .github_project_integration(project_id)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::GitHubNotConfigured)?;
    if !integration.enabled || integration.repository != request.repository {
        return Err(ApiError::GitHubNotConfigured);
    }
    let token = env::var("RIICHI_GITHUB_TOKEN").map_err(|_| ApiError::GitHubNotConfigured)?;
    let base_url = env::var("RIICHI_GITHUB_API_BASE_URL")
        .unwrap_or_else(|_| "https://api.github.com".to_owned());
    let client = GithubClient::new(base_url).map_err(ApiError::GitHubClient)?;
    let snapshots = client
        .import_pull_requests(
            &request.repository,
            &token,
            request.max_pull_requests.unwrap_or(50),
        )
        .await
        .map_err(ApiError::GitHubClient)?;
    for snapshot in &snapshots {
        let pull = snapshot
            .get("pull_request")
            .ok_or(ApiError::InvalidRequest)?;
        let number = pull
            .get("number")
            .and_then(Value::as_i64)
            .ok_or(ApiError::InvalidRequest)?;
        let title = pull
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("GitHub pull request");
        let url = pull
            .get("html_url")
            .and_then(Value::as_str)
            .ok_or(ApiError::InvalidRequest)?;
        let pr_state = pull.get("state").and_then(Value::as_str).unwrap_or("open");
        let review_state = snapshot.get("reviews").and_then(review_state);
        let ci_state = snapshot.get("checks").and_then(ci_state);
        state
            .application
            .database()
            .upsert_github_pull_request_snapshot(
                project_id,
                None,
                &request.repository,
                number,
                url,
                title,
                pr_state,
                review_state.as_deref(),
                ci_state.as_deref(),
                pull.get("updated_at").and_then(Value::as_str),
                snapshot.clone(),
            )
            .await
            .map_err(ApiError::from)?;
    }
    Ok(Json(GithubPullRequestRefreshResponse {
        repository: request.repository,
        imported: snapshots.len(),
    }))
}

pub(super) async fn github_pull_requests(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    jar: CookieJar,
) -> Result<Json<GithubPullRequestsResponse>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_viewer(&principal, project_id)?;
    let mut pull_requests = state
        .application
        .database()
        .github_pull_requests(project_id, 101)
        .await
        .map_err(ApiError::from)?;
    let truncated = pull_requests.len() > 100;
    pull_requests.truncate(100);
    Ok(Json(GithubPullRequestsResponse {
        pull_requests,
        truncated,
    }))
}

pub(super) async fn link_github_pull_request(
    State(state): State<AppState>,
    Path((project_id, pull_request_id)): Path<(Uuid, Uuid)>,
    jar: CookieJar,
    Json(request): Json<GithubPullRequestLinkRequest>,
) -> Result<Json<riichi_persistence::GithubPullRequest>, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_member(&principal, project_id)?;
    Ok(Json(
        state
            .application
            .database()
            .link_github_pull_request(
                project_id,
                principal.account.id,
                pull_request_id,
                request.issue_id,
            )
            .await
            .map_err(ApiError::from)?,
    ))
}

fn review_state(value: &Value) -> Option<String> {
    let reviews = value.as_array()?;
    if reviews
        .iter()
        .any(|review| review.get("state").and_then(Value::as_str) == Some("CHANGES_REQUESTED"))
    {
        return Some("changes_requested".to_owned());
    }
    if reviews
        .iter()
        .any(|review| review.get("state").and_then(Value::as_str) == Some("APPROVED"))
    {
        return Some("approved".to_owned());
    }
    Some("pending".to_owned())
}

fn ci_state(value: &Value) -> Option<String> {
    let runs = value.get("check_runs")?.as_array()?;
    if runs.is_empty() {
        return Some("none".to_owned());
    }
    if runs.iter().any(|run| {
        matches!(
            run.get("conclusion").and_then(Value::as_str),
            Some("failure" | "timed_out" | "cancelled")
        )
    }) {
        return Some("failing".to_owned());
    }
    if runs
        .iter()
        .any(|run| run.get("status").and_then(Value::as_str) != Some("completed"))
    {
        return Some("pending".to_owned());
    }
    Some("passing".to_owned())
}
