use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use sha2::Sha256;
use std::time::Duration;
use thiserror::Error;

const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
const MAX_IMPORT_ISSUES: usize = 1_000;
const MAX_IMPORT_PULL_REQUESTS: usize = 100;
const PAGE_SIZE: usize = 100;
const ALLOWED_ACTIONS: [&str; 6] = [
    "opened",
    "edited",
    "closed",
    "reopened",
    "transferred",
    "deleted",
];

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WebhookError {
    #[error("webhook payload is too large")]
    PayloadTooLarge,
    #[error("webhook signature is invalid")]
    InvalidSignature,
    #[error("unsupported webhook event")]
    UnsupportedEvent,
    #[error("unsupported webhook action")]
    UnsupportedAction,
    #[error("pull request payloads are ignored")]
    PullRequestIgnored,
    #[error("webhook payload is malformed")]
    Malformed,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("GitHub repository must use the owner/name form")]
    InvalidRepository,
    #[error("GitHub import limit must be between 1 and {MAX_IMPORT_ISSUES}")]
    InvalidImportLimit,
    #[error("GitHub API request failed with status {0}")]
    HttpStatus(reqwest::StatusCode),
    #[error("GitHub API request failed")]
    Request(#[source] reqwest::Error),
    #[error("GitHub API returned malformed issue data")]
    MalformedResponse(#[source] serde_json::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GithubIssueWebhook {
    pub delivery_id: String,
    pub action: String,
    pub repository: String,
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct GithubImportedIssue {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub html_url: String,
    pub state: String,
    pub updated_at: Option<String>,
    pub pull_request: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubImport {
    pub issues: Vec<GithubImportedIssue>,
    pub pull_requests_skipped: usize,
}

#[derive(Clone)]
pub struct GithubClient {
    http: reqwest::Client,
    base_url: String,
}

impl GithubClient {
    pub fn new(base_url: impl Into<String>) -> Result<Self, ClientError> {
        let base_url = base_url.into();
        if !base_url.starts_with("https://") && !base_url.starts_with("http://") {
            return Err(ClientError::InvalidRepository);
        }
        Ok(Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .map_err(ClientError::Request)?,
            base_url: base_url.trim_end_matches('/').to_owned(),
        })
    }

    pub async fn import_issues(
        &self,
        repository: &str,
        token: &str,
        max_issues: usize,
    ) -> Result<GithubImport, ClientError> {
        validate_repository(repository)?;
        if !(1..=MAX_IMPORT_ISSUES).contains(&max_issues) {
            return Err(ClientError::InvalidImportLimit);
        }
        let mut issues = Vec::with_capacity(max_issues);
        let mut pull_requests_skipped = 0;
        let mut page = 1;
        while issues.len() < max_issues {
            let response = self
                .http
                .get(format!("{}/repos/{repository}/issues", self.base_url))
                .query(&[
                    ("state", "all"),
                    ("per_page", PAGE_SIZE.to_string().as_str()),
                    ("page", page.to_string().as_str()),
                ])
                .header(reqwest::header::ACCEPT, "application/vnd.github+json")
                .header(reqwest::header::USER_AGENT, "riichi-pilot")
                .bearer_auth(token)
                .send()
                .await
                .map_err(ClientError::Request)?;
            if !response.status().is_success() {
                return Err(ClientError::HttpStatus(response.status()));
            }
            let page_issues = response
                .json::<Vec<GithubImportedIssue>>()
                .await
                .map_err(ClientError::Request)?;
            if page_issues.is_empty() {
                break;
            }
            for issue in page_issues {
                if issue.pull_request.is_some() {
                    pull_requests_skipped += 1;
                    continue;
                }
                issues.push(issue);
                if issues.len() == max_issues {
                    break;
                }
            }
            page += 1;
        }
        Ok(GithubImport {
            issues,
            pull_requests_skipped,
        })
    }

    pub async fn import_pull_requests(
        &self,
        repository: &str,
        token: &str,
        max_pull_requests: usize,
    ) -> Result<Vec<serde_json::Value>, ClientError> {
        validate_repository(repository)?;
        if !(1..=MAX_IMPORT_PULL_REQUESTS).contains(&max_pull_requests) {
            return Err(ClientError::InvalidImportLimit);
        }
        let response = self
            .http
            .get(format!("{}/repos/{repository}/pulls", self.base_url))
            .query(&[
                ("state", "all"),
                ("per_page", max_pull_requests.to_string().as_str()),
            ])
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, "riichi-pilot")
            .bearer_auth(token)
            .send()
            .await
            .map_err(ClientError::Request)?;
        if !response.status().is_success() {
            return Err(ClientError::HttpStatus(response.status()));
        }
        let pulls = response
            .json::<Vec<serde_json::Value>>()
            .await
            .map_err(ClientError::Request)?;
        let mut snapshots = Vec::with_capacity(pulls.len().min(max_pull_requests));
        for pull in pulls.into_iter().take(max_pull_requests) {
            let Some(number) = pull.get("number").and_then(serde_json::Value::as_i64) else {
                continue;
            };
            let reviews = self
                .get_json(
                    format!(
                        "{}/repos/{repository}/pulls/{number}/reviews",
                        self.base_url
                    ),
                    token,
                )
                .await?;
            let checks = pull
                .get("head")
                .and_then(|head| head.get("sha"))
                .and_then(serde_json::Value::as_str)
                .map(|sha| async move {
                    self.get_json(
                        format!(
                            "{}/repos/{repository}/commits/{sha}/check-runs",
                            self.base_url
                        ),
                        token,
                    )
                    .await
                });
            let checks = match checks {
                Some(future) => future.await?,
                None => serde_json::json!({"check_runs": []}),
            };
            snapshots.push(serde_json::json!({ "pull_request": pull, "reviews": reviews, "checks": checks, "trust": "external_untrusted" }));
        }
        Ok(snapshots)
    }

    async fn get_json(&self, url: String, token: &str) -> Result<serde_json::Value, ClientError> {
        let response = self
            .http
            .get(url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header(reqwest::header::USER_AGENT, "riichi-pilot")
            .bearer_auth(token)
            .send()
            .await
            .map_err(ClientError::Request)?;
        if !response.status().is_success() {
            return Err(ClientError::HttpStatus(response.status()));
        }
        response.json().await.map_err(ClientError::Request)
    }
}

fn validate_repository(repository: &str) -> Result<(), ClientError> {
    let mut parts = repository.split('/');
    let Some(owner) = parts.next() else {
        return Err(ClientError::InvalidRepository);
    };
    let Some(name) = parts.next() else {
        return Err(ClientError::InvalidRepository);
    };
    if parts.next().is_some()
        || owner.is_empty()
        || name.is_empty()
        || owner.chars().any(char::is_whitespace)
        || name.chars().any(char::is_whitespace)
        || owner.contains('.')
        || name.contains('.')
    {
        return Err(ClientError::InvalidRepository);
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct GithubWebhookPayload {
    action: String,
    repository: GithubRepository,
    issue: GithubIssue,
}

#[derive(Debug, Deserialize)]
struct GithubRepository {
    full_name: String,
}

#[derive(Debug, Deserialize)]
struct GithubIssue {
    number: i64,
    title: String,
    body: Option<String>,
    html_url: String,
    updated_at: Option<String>,
    pull_request: Option<serde_json::Value>,
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).ok())
        .collect()
}

pub fn verify_signature(secret: &[u8], payload: &[u8], signature: &str) -> bool {
    let Some(hex) = signature.strip_prefix("sha256=") else {
        return false;
    };
    let Some(expected) = decode_hex(hex) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        return false;
    };
    mac.update(payload);
    mac.verify_slice(&expected).is_ok()
}

pub fn parse_issues_webhook(
    delivery_id: &str,
    event_name: &str,
    signature: &str,
    secret: &[u8],
    payload: &[u8],
) -> Result<GithubIssueWebhook, WebhookError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(WebhookError::PayloadTooLarge);
    }
    if delivery_id.trim().is_empty() || event_name != "issues" {
        return Err(WebhookError::UnsupportedEvent);
    }
    if !verify_signature(secret, payload, signature) {
        return Err(WebhookError::InvalidSignature);
    }
    let payload: GithubWebhookPayload =
        serde_json::from_slice(payload).map_err(|_| WebhookError::Malformed)?;
    if !ALLOWED_ACTIONS.contains(&payload.action.as_str()) {
        return Err(WebhookError::UnsupportedAction);
    }
    if payload.issue.pull_request.is_some() {
        return Err(WebhookError::PullRequestIgnored);
    }
    Ok(GithubIssueWebhook {
        delivery_id: delivery_id.to_owned(),
        action: payload.action,
        repository: payload.repository.full_name,
        number: payload.issue.number,
        title: payload.issue.title,
        body: payload.issue.body,
        html_url: payload.issue.html_url,
        updated_at: payload.issue.updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Json, Router,
        extract::Query,
        http::{HeaderMap, StatusCode, header},
        routing::get,
    };
    use std::collections::HashMap;

    fn signed(payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(b"secret").unwrap();
        mac.update(payload);
        format!(
            "sha256={}",
            mac.finalize()
                .into_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }

    fn payload() -> Vec<u8> {
        serde_json::json!({
            "action": "edited",
            "repository": {"full_name": "natalie/riichi"},
            "issue": {
                "number": 42,
                "title": "bounded context",
                "body": "external text",
                "html_url": "https://github.com/natalie/riichi/issues/42",
                "updated_at": "2026-07-12T00:00:00Z"
            }
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn accepts_signed_allowed_issue_events_without_trusting_the_payload() {
        let payload = payload();
        let webhook = parse_issues_webhook(
            "delivery-1",
            "issues",
            &signed(&payload),
            b"secret",
            &payload,
        )
        .unwrap();
        assert_eq!(webhook.repository, "natalie/riichi");
        assert_eq!(webhook.number, 42);
        assert_eq!(webhook.action, "edited");
    }

    #[test]
    fn rejects_bad_signatures_before_parsing() {
        assert_eq!(
            parse_issues_webhook("delivery-1", "issues", "sha256=00", b"secret", &payload()),
            Err(WebhookError::InvalidSignature)
        );
    }

    #[test]
    fn ignores_pull_requests_and_unapproved_actions() {
        let value = serde_json::json!({
            "action": "opened",
            "repository": {"full_name": "natalie/riichi"},
            "issue": {
                "number": 42,
                "title": "pull request",
                "body": null,
                "html_url": "https://github.com/natalie/riichi/pull/42",
                "updated_at": null,
                "pull_request": {"url": "https://api.github.com/pulls/42"}
            }
        });
        let pull_request = value.to_string().into_bytes();
        assert_eq!(
            parse_issues_webhook(
                "delivery-1",
                "issues",
                &signed(&pull_request),
                b"secret",
                &pull_request
            ),
            Err(WebhookError::PullRequestIgnored)
        );
        let mut unsupported = payload();
        let mut value: serde_json::Value = serde_json::from_slice(&unsupported).unwrap();
        value["action"] = serde_json::Value::String("labeled".to_owned());
        unsupported = value.to_string().into_bytes();
        assert_eq!(
            parse_issues_webhook(
                "delivery-1",
                "issues",
                &signed(&unsupported),
                b"secret",
                &unsupported
            ),
            Err(WebhookError::UnsupportedAction)
        );
    }

    #[test]
    fn enforces_a_bounded_raw_payload() {
        let payload = vec![b'x'; MAX_PAYLOAD_BYTES + 1];
        assert_eq!(
            parse_issues_webhook("delivery-1", "issues", "", b"secret", &payload),
            Err(WebhookError::PayloadTooLarge)
        );
    }

    #[tokio::test]
    async fn imports_issue_pages_filters_pull_requests_and_honors_the_bounded_limit() {
        async fn issues(
            Query(query): Query<HashMap<String, String>>,
            headers: HeaderMap,
        ) -> Result<Json<serde_json::Value>, StatusCode> {
            if headers
                .get(header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                != Some("Bearer token")
                || query.get("state") != Some(&"all".to_owned())
                || query.get("per_page") != Some(&PAGE_SIZE.to_string())
            {
                return Err(StatusCode::UNAUTHORIZED);
            }
            let response = match query.get("page").map(String::as_str) {
                Some("1") => serde_json::json!([
                    {
                        "number": 1,
                        "title": "first",
                        "body": "body",
                        "html_url": "https://github.com/acme/riichi/issues/1",
                        "state": "open",
                        "updated_at": "2026-07-12T00:00:00Z"
                    },
                    {
                        "number": 2,
                        "title": "pull request",
                        "body": null,
                        "html_url": "https://github.com/acme/riichi/pull/2",
                        "state": "open",
                        "updated_at": null,
                        "pull_request": {"url": "https://api.github.com/pulls/2"}
                    }
                ]),
                Some("2") => serde_json::json!([{
                    "number": 3,
                    "title": "second",
                    "body": null,
                    "html_url": "https://github.com/acme/riichi/issues/3",
                    "state": "closed",
                    "updated_at": "2026-07-12T00:01:00Z"
                }]),
                _ => serde_json::json!([]),
            };
            Ok(Json(response))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
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
            .import_issues("acme/riichi", "token", 2)
            .await
            .unwrap();

        server.abort();
        assert_eq!(imported.pull_requests_skipped, 1);
        assert_eq!(
            imported
                .issues
                .iter()
                .map(|issue| issue.number)
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
    }

    #[tokio::test]
    async fn rejects_unbounded_or_ambiguous_import_requests() {
        let client = GithubClient::new("https://api.github.com").unwrap();
        assert!(matches!(
            client.import_issues("acme/riichi/extra", "token", 1).await,
            Err(ClientError::InvalidRepository)
        ));
        assert!(matches!(
            client
                .import_issues("acme/riichi", "token", MAX_IMPORT_ISSUES + 1)
                .await,
            Err(ClientError::InvalidImportLimit)
        ));
        assert!(matches!(
            client
                .import_pull_requests("acme/riichi", "token", MAX_IMPORT_PULL_REQUESTS + 1)
                .await,
            Err(ClientError::InvalidImportLimit)
        ));
    }
}
