#![recursion_limit = "256"]

use std::{convert::Infallible, env};

use axum::{
    Router,
    body::Body,
    extract::{Json, Multipart, Path, Query, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Redirect, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, patch, post, put},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Duration;
use futures_util::StreamExt;
use riichi_application::Application;
use riichi_auth::{AuthError, AuthService, HumanPrincipal};
use riichi_integrations_github::{ClientError, GithubClient, WebhookError, parse_issues_webhook};
use riichi_persistence::{Error as PersistenceError, Report};
use riichi_storage::ObjectAttachmentStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use tower_http::trace::TraceLayer;
use tracing::error;
use uuid::Uuid;

mod agent_protocol;
mod agents;
mod auth;
mod document_sync;
mod documents;
mod github;
mod issues;
mod navigation;
mod onboarding;
mod projects;
mod views;

use agent_protocol::*;
use agents::*;
use auth::*;
use document_sync::*;
use documents::*;
use github::*;
use issues::*;
use navigation::*;
use onboarding::*;
use projects::*;
use views::*;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
}

#[derive(Clone, Copy, Debug)]
enum EventWakeup {
    Project(Uuid),
    Account(Uuid),
}

pub fn app() -> Router {
    Router::new().route("/health", get(health))
}

#[derive(Clone)]
pub struct AppState {
    application: Application,
    auth: Option<AuthService>,
    event_wakeups: broadcast::Sender<EventWakeup>,
    document_sync: DocumentSyncRegistry,
    attachment_store: ObjectAttachmentStore,
    electric_url: Option<String>,
    electric_secret: Option<String>,
    http_client: reqwest::Client,
}

pub fn app_with_state(application: Application) -> Router {
    app_with_optional_auth(application, None)
}

pub fn app_with_auth(application: Application, auth: AuthService) -> Router {
    app_with_optional_auth(application, Some(auth))
}

fn app_with_optional_auth(application: Application, auth: Option<AuthService>) -> Router {
    app_with_optional_auth_and_electric_url(application, auth, env::var("RIICHI_ELECTRIC_URL").ok())
}

pub fn app_with_auth_and_electric_url(
    application: Application,
    auth: AuthService,
    electric_url: Option<String>,
) -> Router {
    app_with_optional_auth_and_electric_url(application, Some(auth), electric_url)
}

pub fn app_with_auth_and_attachment_store(
    application: Application,
    auth: AuthService,
    attachment_store: ObjectAttachmentStore,
) -> Router {
    app_with_optional_auth_and_electric_url_and_attachment_store(
        application,
        Some(auth),
        env::var("RIICHI_ELECTRIC_URL").ok(),
        attachment_store,
    )
}

fn app_with_optional_auth_and_electric_url(
    application: Application,
    auth: Option<AuthService>,
    electric_url: Option<String>,
) -> Router {
    let attachment_store =
        ObjectAttachmentStore::from_env().expect("attachment storage configuration must be valid");
    app_with_optional_auth_and_electric_url_and_attachment_store(
        application,
        auth,
        electric_url,
        attachment_store,
    )
}

fn app_with_optional_auth_and_electric_url_and_attachment_store(
    application: Application,
    auth: Option<AuthService>,
    electric_url: Option<String>,
    attachment_store: ObjectAttachmentStore,
) -> Router {
    let event_wakeups = spawn_event_hub(application.database());
    let document_sync = DocumentSyncRegistry::new();
    spawn_document_sync_hub(application.database(), document_sync.clone());
    Router::new()
        .route("/health", get(health))
        .route("/openapi.json", get(openapi_document))
        .route("/readyz", get(readiness))
        .route("/auth/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/auth/logout", post(logout))
        .route("/api/v1/auth/me", get(human_me))
        .route(
            "/api/v1/auth/me/avatar",
            get(human_avatar).put(upload_human_avatar).delete(delete_human_avatar),
        )
        .route("/api/v1/navigation", get(navigation))
        .route(
            "/api/v1/views",
            get(list_saved_views).post(save_view),
        )
        .route("/api/v1/views/{view_id}", delete(delete_saved_view))
        .route("/api/v1/views/{view_id}/pin", post(pin_saved_view))
        .route(
            "/api/v1/projects/{project_id}/views",
            get(list_project_saved_views).post(save_project_view),
        )
        .route(
            "/api/v1/projects/{project_id}/views/{view_id}",
            delete(delete_project_saved_view),
        )
        .route(
            "/api/v1/projects/{project_id}/views/{view_id}/pin",
            post(pin_project_saved_view),
        )
        .route(
            "/api/v1/organizations/{organization_id}/documents",
            get(list_organization_documents).post(create_organization_document),
        )
        .route(
            "/api/v1/teams/{team_id}/documents",
            get(list_team_documents).post(create_team_document),
        )
        .route(
            "/api/v1/projects/{project_id}/documents",
            get(list_project_documents).post(create_project_document),
        )
        .route(
            "/api/v1/documents/{document_id}",
            get(get_document)
                .patch(update_document_metadata)
                .delete(delete_document),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/description-document",
            get(get_issue_description_document),
        )
        .route(
            "/api/v1/documents/{document_id}/version",
            get(get_document_version).patch(update_document_content),
        )
        .route(
            "/api/v1/documents/{document_id}/loro-snapshot",
            get(get_document_loro_snapshot),
        )
        .route(
            "/api/v1/documents/{document_id}/loro-updates",
            post(apply_document_loro_update),
        )
        .route(
            "/api/v1/documents/{document_id}/loro-sync",
            get(document_loro_sync),
        )
        .route(
            "/api/v1/documents/{document_id}/references",
            get(get_document_references).put(replace_document_references),
        )
        .route(
            "/api/v1/documents/{document_id}/backlinks",
            get(get_document_backlinks),
        )
        .route(
            "/api/v1/documents/{document_id}/attachments",
            post(create_attachment_upload),
        )
        .route(
            "/api/v1/attachment-uploads/{upload_id}",
            put(put_attachment_upload),
        )
        .route(
            "/api/v1/attachment-uploads/{upload_id}/complete",
            post(complete_attachment_upload),
        )
        .route("/api/v1/attachments/{attachment_id}", get(get_attachment))
        .route("/api/v1/issues", get(human_all_issues))
        .route("/api/v1/issues/{issue_id}", get(human_get_issue))
        .route(
            "/api/v1/teams/{team_id}/issues",
            get(human_team_issues).post(create_team_issue),
        )
        .route("/api/v1/teams/{team_id}", patch(update_team))
        .route(
            "/api/v1/organizations/{organization_id}/logo",
            get(organization_logo).put(upload_organization_logo).delete(delete_organization_logo),
        )
        .route("/api/v1/approvals", get(human_pending_approvals))
        .route("/api/v1/inbox", get(human_inbox))
        .route("/api/v1/inbox/unread-count", get(human_inbox_unread_count))
        .route(
            "/api/v1/inbox/{notification_id}/read",
            post(mark_inbox_notification_read),
        )
        .route("/api/v1/projects/{project_id}/queue", get(human_queue))
        .route(
            "/api/v1/projects/{project_id}/events",
            get(project_events),
        )
        .route(
            "/api/v1/projects/{project_id}/sync/issues",
            get(electric_issue_shape),
        )
        .route("/api/v1/sync/issues", get(electric_human_issue_shape))
        .route("/api/v1/sync/documents", get(electric_human_document_shape))
        .route(
            "/api/v1/projects/{project_id}/sync/issues/{issue_id}/activity",
            get(electric_issue_activity_shape),
        )
        .route("/api/v1/sync/inbox", get(electric_inbox_shape))
        .route("/api/v1/sync/navigation", get(electric_navigation_shape))
        .route("/api/v1/sync/approvals", get(electric_approval_shape))
        .route(
            "/api/v1/projects/{project_id}/issues",
            post(create_issue),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}",
            get(get_issue).patch(update_issue).delete(delete_issue),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/comments",
            post(create_comment),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/activity",
            get(issue_activity),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/edges",
            post(create_issue_edge),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/collaborators",
            get(get_issue_collaborators).post(grant_issue_collaborator),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/quarantined-attempts",
            get(get_quarantined_attempts),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/collaborators/{session_id}/{capability}/revoke",
            post(revoke_issue_collaborator),
        )
        .route(
            "/api/v1/projects/{project_id}/edges/{edge_id}",
            delete(remove_issue_edge),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/holds",
            post(create_hold),
        )
        .route(
            "/api/v1/projects/{project_id}/holds/{hold_id}/release",
            post(release_hold),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/takeover",
            post(takeover_issue),
        )
        .route(
            "/api/v1/projects/{project_id}/recovery/{checklist_id}/complete",
            post(complete_recovery),
        )
        .route(
            "/api/v1/projects/{project_id}/issues/{issue_id}/approvals",
            post(create_approval_request),
        )
        .route(
            "/api/v1/projects/{project_id}/approvals/{approval_id}/approve",
            post(approve_approval_request),
        )
        .route(
            "/api/v1/projects/{project_id}/approvals/{approval_id}/reject",
            post(reject_approval_request),
        )
        .route(
            "/api/v1/projects/{project_id}/agents",
            get(agent_roster),
        )
        .route("/api/v1/teams/{team_id}/agents", get(team_agent_roster))
        .route(
            "/api/v1/teams/{team_id}/sync/agents",
            get(electric_human_agent_shape),
        )
        .route(
            "/api/v1/projects/{project_id}/agent-roles",
            post(create_agent_role),
        )
        .route(
            "/api/v1/projects/{project_id}/onboarding-sample",
            post(create_onboarding_sample),
        )
        .route(
            "/api/v1/projects/{project_id}/agent-roles/{role_id}/sessions",
            post(create_agent_session),
        )
        .route(
            "/api/v1/projects/{project_id}/agent-sessions/{session_id}/revoke",
            post(revoke_agent_session),
        )
        .route(
            "/api/v1/projects/{project_id}/agent-roles/{role_id}/revoke",
            post(revoke_agent_role),
        )
        .route(
            "/api/v1/projects/{project_id}/integrations/github/import",
            post(import_github_issues),
        )
        .route("/api/v1/projects", post(create_project))
        .route(
            "/api/v1/projects/{project_id}/invites",
            post(create_invite),
        )
        .route(
            "/api/v1/projects/{project_id}/invites/{invite_id}/revoke",
            post(revoke_invite),
        )
        .route(
            "/api/v1/projects/{project_id}/outbox/{message_id}/redrive",
            post(redrive_outbox),
        )
        .route("/api/v1/invites/accept", post(accept_invite))
        .route("/api/v1/ready", post(ready))
        .route("/api/v1/claim", post(claim))
        .route("/api/v1/renew", post(renew))
        .route("/api/v1/report", post(report))
        .route("/api/v1/report/batch", post(report_batch))
        .route("/api/v1/context", post(context))
        .route(
            "/api/v1/documents/{document_id}/agent-read",
            post(read_document),
        )
        .route(
            "/api/v1/documents/{document_id}/agent-edit",
            post(apply_document_edit),
        )
        .route(
            "/api/v1/context/{issue_id}/resources/{resource}",
            get(context_resource),
        )
        .route(
            "/api/v1/recovery/{issue_id}/quarantined-attempts",
            get(agent_quarantined_attempts),
        )
        .route("/api/v1/integrations/github/webhook", post(github_webhook))
        .with_state(AppState {
            application,
            auth,
            event_wakeups,
            document_sync,
            attachment_store,
            electric_url,
            electric_secret: env::var("RIICHI_ELECTRIC_SOURCE_SECRET").ok(),
            http_client: reqwest::Client::new(),
        })
        .layer(
            TraceLayer::new_for_http().make_span_with(|request: &Request<_>| {
                let request_id = request
                    .headers()
                    .get("x-request-id")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("missing");
                tracing::info_span!(
                    "http_request",
                    request_id = %request_id,
                    method = %request.method(),
                    uri = %request.uri()
                )
            }),
        )
        .layer(middleware::from_fn(request_id))
}

fn spawn_event_hub(database: riichi_persistence::Database) -> broadcast::Sender<EventWakeup> {
    let (sender, _) = broadcast::channel(1024);
    let publisher = sender.clone();
    tokio::spawn(async move {
        loop {
            let mut listener = match database.event_listener().await {
                Ok(listener) => listener,
                Err(error) => {
                    tracing::warn!(%error, "delivery event listener unavailable");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            loop {
                match listener.recv().await {
                    Ok(notification) => {
                        let payload: Value = match serde_json::from_str(notification.payload()) {
                            Ok(payload) => payload,
                            Err(_) => continue,
                        };
                        let wakeup = match notification.channel() {
                            "riichi_delivery_events" => payload
                                .get("project_id")
                                .and_then(Value::as_str)
                                .and_then(|value| value.parse().ok())
                                .map(EventWakeup::Project),
                            "riichi_human_access_events" => payload
                                .get("account_id")
                                .and_then(Value::as_str)
                                .and_then(|value| value.parse().ok())
                                .map(EventWakeup::Account),
                            _ => None,
                        };
                        if let Some(wakeup) = wakeup {
                            let _ = publisher.send(wakeup);
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "delivery event listener disconnected");
                        break;
                    }
                }
            }
        }
    });
    sender
}

async fn request_id(mut request: Request<axum::body::Body>, next: Next) -> Response {
    let request_id = request
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::now_v7);
    let request_id_value = request_id;
    let request_id = request_id_value.to_string();
    request.headers_mut().insert(
        "x-request-id",
        request_id.parse().expect("UUID is a valid header value"),
    );
    let mut response =
        riichi_persistence::with_request_id(request_id_value, next.run(request)).await;
    response.headers_mut().insert(
        "x-request-id",
        request_id.parse().expect("UUID is a valid header value"),
    );
    response
}

async fn health() -> (StatusCode, Json<HealthResponse>) {
    (StatusCode::OK, Json(HealthResponse { status: "ok" }))
}

pub fn openapi_document_value() -> Value {
    let mut document = json!({
        "openapi": "3.0.3",
        "info": {
            "title": "Riichi API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Generated contract for Riichi agent and human collaboration APIs."
        },
        "paths": {
            "/api/v1/ready": {"post": {"operationId": "ready", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ReadyRequest"}}}}, "responses": {"200": {"description": "Eligible work and snapshot exclusions"}}}},
            "/api/v1/claim": {"post": {"operationId": "claim", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ClaimRequest"}}}}, "responses": {"200": {"description": "Fenced lease"}}}},
            "/api/v1/context": {"post": {"operationId": "context", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ContextRequest"}}}}, "responses": {"200": {"description": "Bounded provenance-aware context"}}}},
            "/api/v1/report/batch": {"post": {"operationId": "reportBatch", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ReportBatchRequest"}}}}, "responses": {"200": {"description": "Idempotent report result"}}}},
            "/api/v1/inbox": {"get": {"operationId": "getInbox", "parameters": [{"name": "unread_only", "in": "query", "schema": {"type": "boolean"}}, {"name": "limit", "in": "query", "schema": {"type": "integer", "format": "int64"}}], "responses": {"200": {"description": "Durable human notifications", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/InboxResponse"}}}}}}},
            "/api/v1/inbox/unread-count": {"get": {"operationId": "getInboxUnreadCount", "responses": {"200": {"description": "Unread notification count", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/UnreadCountResponse"}}}}}}},
            "/api/v1/inbox/{notification_id}/read": {"post": {"operationId": "markInboxNotificationRead", "parameters": [{"name": "notification_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"204": {"description": "Notification marked read"}}}},
            "/api/v1/teams/{team_id}/agents": {"get": {"operationId": "getTeamAgentRoster", "parameters": [{"name": "team_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Team agent roster", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/AgentRoster"}}}}}}}
            ,"/api/v1/navigation": {"get": {"operationId": "getNavigation", "responses": {"200": {"description": "Accessible organizations, projects, and teams", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/NavigationResponse"}}}}}}}
            ,"/api/v1/views": {"get": {"operationId": "listSavedViews", "responses": {"200": {"description": "Account-owned saved queue views", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/SavedView"}}}}}}}, "post": {"operationId": "saveSavedView", "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SaveViewRequest"}}}}, "responses": {"200": {"description": "Saved queue view", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SavedView"}}}}}}}
            ,"/api/v1/views/{view_id}": {"delete": {"operationId": "deleteSavedView", "parameters": [{"name": "view_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"204": {"description": "Saved queue view deleted"}}}}
            ,"/api/v1/views/{view_id}/pin": {"post": {"operationId": "pinSavedView", "parameters": [{"name": "view_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/PinViewRequest"}}}}, "responses": {"204": {"description": "Personal saved view pin updated"}}}}
            ,"/api/v1/projects/{project_id}/views": {"get": {"operationId": "listProjectSavedViews", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Project-scoped saved queue views", "content": {"application/json": {"schema": {"type": "array", "items": {"$ref": "#/components/schemas/SavedView"}}}}}}}, "post": {"operationId": "saveProjectView", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SaveViewRequest"}}}}, "responses": {"200": {"description": "Project-scoped saved queue view", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/SavedView"}}}}}}}
            ,"/api/v1/projects/{project_id}/views/{view_id}": {"delete": {"operationId": "deleteProjectSavedView", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "view_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"204": {"description": "Project-scoped saved queue view deleted"}}}}
            ,"/api/v1/projects/{project_id}/views/{view_id}/pin": {"post": {"operationId": "pinProjectSavedView", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "view_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/PinViewRequest"}}}}, "responses": {"204": {"description": "Project saved view pin updated"}}}}
            ,"/api/v1/projects/{project_id}/agent-roles/{role_id}/sessions": {"post": {"operationId": "createAgentSession", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "role_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateAgentSessionRequest"}}}}, "responses": {"200": {"description": "One-time agent session credential", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateAgentSessionResponse"}}}}}}}
            ,"/api/v1/projects/{project_id}/onboarding-sample": {"post": {"operationId": "createOnboardingSample", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Authoritative guided workflow sample", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/OnboardingSample"}}}}}}}
            ,"/api/v1/issues": {"get": {"operationId": "getAllIssues", "responses": {"200": {"description": "Accessible human issues"}}}}
            ,"/api/v1/issues/{issue_id}": {"get": {"operationId": "getGlobalIssue", "parameters": [{"name": "issue_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Issue"}}}}
            ,"/api/v1/teams/{team_id}/issues": {"get": {"operationId": "getTeamIssues", "parameters": [{"name": "team_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Team issues"}}}, "post": {"operationId": "createTeamIssue", "responses": {"200": {"description": "Created issue"}}}}
            ,"/api/v1/projects/{project_id}/queue": {"get": {"operationId": "getProjectQueue", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Project queue"}}}}
            ,"/api/v1/projects/{project_id}/sync/issues": {"get": {"operationId": "syncProjectIssueMetadata", "description": "Authenticated Electric shape proxy for the project issue metadata read model.", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Electric shape stream"}, "401": {"description": "Human authentication required"}, "403": {"description": "Project membership required"}, "503": {"description": "Electric synchronization is not configured"}}}}
            ,"/api/v1/projects/{project_id}/sync/issues/{issue_id}/activity": {"get": {"operationId": "syncIssueActivity", "description": "Authenticated Electric shape proxy for the authoritative issue activity read model.", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "issue_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Electric shape stream"}, "401": {"description": "Human authentication required"}, "403": {"description": "Project membership required"}, "503": {"description": "Electric synchronization is not configured"}}}}
            ,"/api/v1/sync/inbox": {"get": {"operationId": "syncInboxNotifications", "description": "Authenticated Electric shape proxy for the current human notification inbox.", "responses": {"200": {"description": "Electric shape stream"}, "401": {"description": "Human authentication required"}, "503": {"description": "Electric synchronization is not configured"}}}}
            ,"/api/v1/sync/navigation": {"get": {"operationId": "syncNavigation", "description": "Authenticated Electric shape proxy for the current human navigation read model.", "responses": {"200": {"description": "Electric shape stream"}, "401": {"description": "Human authentication required"}, "503": {"description": "Electric synchronization is not configured"}}}}
            ,"/api/v1/sync/approvals": {"get": {"operationId": "syncApprovals", "description": "Authenticated Electric shape proxy for the current human approval queue.", "responses": {"200": {"description": "Electric shape stream"}, "401": {"description": "Human authentication required"}, "503": {"description": "Electric synchronization is not configured"}}}}
            ,"/api/v1/projects/{project_id}/issues": {"post": {"operationId": "createIssue", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"200": {"description": "Created issue"}}}}
            ,"/api/v1/projects/{project_id}/issues/{issue_id}": {"get": {"operationId": "getIssue", "responses": {"200": {"description": "Issue"}}}, "patch": {"operationId": "updateIssue", "responses": {"200": {"description": "Updated issue", "headers": {"X-Riichi-Transaction-Id": {"schema": {"type": "integer", "format": "int64"}}}}}}, "delete": {"operationId": "deleteIssue", "responses": {"204": {"description": "Deleted issue"}}}}
            ,"/api/v1/projects/{project_id}/issues/{issue_id}/comments": {"post": {"operationId": "createComment", "responses": {"200": {"description": "Created comment"}}}}
            ,"/api/v1/projects/{project_id}/issues/{issue_id}/activity": {"get": {"operationId": "getIssueActivity", "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "issue_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "limit", "in": "query", "schema": {"type": "integer", "maximum": 200}}], "responses": {"200": {"description": "Bounded issue activity"}}}}
            ,"/api/v1/documents/{document_id}/loro-snapshot": {"get": {"operationId": "getDocumentLoroSnapshot", "parameters": [{"name": "document_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}, {"name": "revision", "in": "query", "schema": {"type": "integer", "format": "int64"}}], "responses": {"200": {"description": "Loro snapshot bytes", "headers": {"X-Riichi-Document-Revision": {"schema": {"type": "integer", "format": "int64"}}, "X-Riichi-Document-Schema-Version": {"schema": {"type": "integer"}}, "X-Riichi-Document-Frontiers": {"schema": {"type": "string"}}}, "content": {"application/octet-stream": {"schema": {"type": "string", "format": "binary"}}}}}}}
            ,"/api/v1/documents/{document_id}/loro-updates": {"post": {"operationId": "applyDocumentLoroUpdate", "parameters": [{"name": "document_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ApplyLoroUpdateRequest"}}}}, "responses": {"200": {"description": "Accepted or replayed Loro update", "content": {"application/json": {"schema": {"$ref": "#/components/schemas/ApplyLoroUpdateResponse"}}}}}}}
            ,"/api/v1/documents/{document_id}/loro-sync": {"get": {"operationId": "connectDocumentLoroSync", "description": "Authenticated WebSocket upgrade for live Loro document synchronization.", "parameters": [{"name": "document_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}], "responses": {"101": {"description": "WebSocket protocol upgrade"}}}}
        },
        "components": {"schemas": {
            "ReadyRequest": {"type": "object", "properties": {"limit": {"type": "integer", "format": "int64"}}},
            "ClaimRequest": {"type": "object", "required": ["issue_id", "idempotency_key"], "properties": {"issue_id": {"type": "string", "format": "uuid"}, "requested_ttl_seconds": {"type": "integer", "format": "int64"}, "idempotency_key": {"type": "string"}}},
            "ContextRequest": {"type": "object", "required": ["issue_id"], "properties": {"issue_id": {"type": "string", "format": "uuid"}, "max_bytes": {"type": "integer", "format": "int64"}, "document_frontiers": {"type": ["array", "null"], "items": {"$ref": "#/components/schemas/LoroFrontier"}}}},
            "ReportBatchRequest": {"type": "object", "required": ["lease_id", "fencing_token", "idempotency_key", "operations"], "properties": {"lease_id": {"type": "string", "format": "uuid"}, "fencing_token": {"type": "integer", "format": "int64"}, "idempotency_key": {"type": "string"}, "operations": {"type": "array", "maxItems": 50, "items": {"type": "object"}}}},
            "Notification": {"type": "object", "required": ["id", "recipient_account_id", "kind", "payload", "created_at"], "properties": {"id": {"type": "string", "format": "uuid"}, "recipient_account_id": {"type": "string", "format": "uuid"}, "kind": {"type": "string", "enum": ["comment", "approval", "assignment", "invitation", "takeover", "lease"]}, "project_id": {"type": ["string", "null"], "format": "uuid"}, "issue_id": {"type": ["string", "null"], "format": "uuid"}, "actor_id": {"type": ["string", "null"], "format": "uuid"}, "payload": {"type": "object"}, "approval_state": {"type": ["string", "null"], "enum": ["pending", "approved", "rejected", "superseded", "expired", null]}, "created_at": {"type": "string", "format": "date-time"}, "read_at": {"type": ["string", "null"], "format": "date-time"}}},
            "InboxResponse": {"type": "object", "required": ["notifications", "unread_count"], "properties": {"notifications": {"type": "array", "items": {"$ref": "#/components/schemas/Notification"}}, "unread_count": {"type": "integer", "format": "int64"}}},
            "UnreadCountResponse": {"type": "object", "required": ["unread_count"], "properties": {"unread_count": {"type": "integer", "format": "int64"}}},
            "AgentRole": {"type": "object", "properties": {"id": {"type": "string", "format": "uuid"}, "project_id": {"type": "string", "format": "uuid"}, "team_id": {"type": "string", "format": "uuid"}, "display_name": {"type": "string"}, "owner_account_id": {"type": ["string", "null"], "format": "uuid"}, "capabilities": {"type": "object"}, "revoked_at": {"type": ["string", "null"], "format": "date-time"}, "active_session_count": {"type": "integer", "format": "int64"}}},
            "AgentSession": {"type": "object", "properties": {"id": {"type": "string", "format": "uuid"}, "project_id": {"type": "string", "format": "uuid"}, "team_id": {"type": "string", "format": "uuid"}, "agent_role_id": {"type": "string", "format": "uuid"}, "state": {"type": "string"}, "max_lifetime_ends_at": {"type": "string", "format": "date-time"}, "heartbeat_at": {"type": ["string", "null"], "format": "date-time"}, "last_action_at": {"type": ["string", "null"], "format": "date-time"}, "revoked_at": {"type": ["string", "null"], "format": "date-time"}}},
            "AgentRoster": {"type": "object", "required": ["roles", "sessions"], "properties": {"roles": {"type": "array", "items": {"$ref": "#/components/schemas/AgentRole"}}, "sessions": {"type": "array", "items": {"$ref": "#/components/schemas/AgentSession"}}}},
            "NavigationResponse": {"type": "object", "required": ["organizations"], "properties": {"organizations": {"type": "array", "items": {"$ref": "#/components/schemas/NavigationOrganization"}}}},
            "NavigationOrganization": {"type": "object", "required": ["id", "name", "role", "logo_url", "teams"], "properties": {"id": {"type": "string", "format": "uuid"}, "name": {"type": "string"}, "role": {"type": "string"}, "logo_url": {"type": ["string", "null"]}, "teams": {"type": "array", "items": {"$ref": "#/components/schemas/NavigationTeam"}}}},
            "NavigationTeam": {"type": "object", "required": ["id", "name", "key", "emoji", "projects", "views"], "properties": {"id": {"type": "string", "format": "uuid"}, "name": {"type": "string"}, "key": {"type": "string"}, "emoji": {"type": ["string", "null"]}, "projects": {"type": "array", "items": {"$ref": "#/components/schemas/NavigationProject"}}, "views": {"type": "array", "items": {"$ref": "#/components/schemas/NavigationView"}}}},
            "NavigationProject": {"type": "object", "required": ["id", "name", "role"], "properties": {"id": {"type": "string", "format": "uuid"}, "name": {"type": "string"}, "role": {"type": "string"}}},
            "NavigationView": {"type": "object", "required": ["id", "name"], "properties": {"id": {"type": "string", "format": "uuid"}, "name": {"type": "string"}}}
            ,"PinViewRequest": {"type": "object", "required": ["pinned"], "properties": {"pinned": {"type": "boolean"}}}
            ,"SavedView": {"type": "object", "required": ["id", "account_id", "project_id", "visibility", "pinned", "name", "filters", "created_at", "updated_at"], "properties": {"id": {"type": "string", "format": "uuid"}, "account_id": {"type": "string", "format": "uuid"}, "project_id": {"type": ["string", "null"], "format": "uuid"}, "visibility": {"type": "string"}, "pinned": {"type": "boolean"}, "name": {"type": "string"}, "filters": {"type": "object"}, "created_at": {"type": "string", "format": "date-time"}, "updated_at": {"type": "string", "format": "date-time"}}}
            ,"SaveViewRequest": {"type": "object", "required": ["name", "filters"], "properties": {"name": {"type": "string", "maxLength": 80}, "filters": {"type": "object"}}}
            ,"CreateAgentSessionRequest": {"type": "object", "properties": {"lifetime_seconds": {"type": "integer", "minimum": 60, "maximum": 86400}}}
            ,"CreateAgentSessionResponse": {"type": "object", "required": ["session_id", "agent_token", "expires_at"], "properties": {"session_id": {"type": "string", "format": "uuid"}, "agent_token": {"type": "string"}, "expires_at": {"type": "string", "format": "date-time"}}}
            ,"OnboardingSample": {"type": "object", "required": ["project_id", "role_id", "session_id", "triage_issue_id", "agent_issue_id", "recovery_issue_id", "approval_id", "recovery_checklist_id", "created_at"], "properties": {"project_id": {"type": "string", "format": "uuid"}, "role_id": {"type": "string", "format": "uuid"}, "session_id": {"type": "string", "format": "uuid"}, "triage_issue_id": {"type": "string", "format": "uuid"}, "agent_issue_id": {"type": "string", "format": "uuid"}, "recovery_issue_id": {"type": "string", "format": "uuid"}, "approval_id": {"type": "string", "format": "uuid"}, "recovery_checklist_id": {"type": "string", "format": "uuid"}, "created_at": {"type": "string", "format": "date-time"}}}
            ,"LoroFrontier": {"type": "object", "required": ["peer_id", "counter"], "properties": {"peer_id": {"type": "string"}, "counter": {"type": "integer", "format": "int32"}}}
            ,"ApplyLoroUpdateRequest": {"type": "object", "required": ["update_id", "previous_frontiers", "payload_base64"], "properties": {"schema_version": {"type": ["integer", "null"]}, "update_id": {"type": "string", "format": "uuid"}, "idempotency_key": {"type": ["string", "null"]}, "previous_frontiers": {"type": "array", "items": {"$ref": "#/components/schemas/LoroFrontier"}}, "payload_base64": {"type": "string"}}}
            ,"ApplyLoroUpdateResponse": {"type": "object", "required": ["update_id", "document_id", "source", "previous_frontiers", "resulting_frontiers", "accepted_at", "replayed"], "properties": {"update_id": {"type": "string", "format": "uuid"}, "document_id": {"type": "string", "format": "uuid"}, "source": {"type": "string"}, "previous_frontiers": {"type": "array", "items": {"$ref": "#/components/schemas/LoroFrontier"}}, "resulting_frontiers": {"type": "array", "items": {"$ref": "#/components/schemas/LoroFrontier"}}, "accepted_at": {"type": "string", "format": "date-time"}, "replayed": {"type": "boolean"}}}
        }}
    });
    document["paths"]["/api/v1/sync/issues"] = json!({
        "get": {
            "operationId": "syncHumanIssues",
            "description": "Authenticated Electric shape proxy for all human-accessible issue queue rows.",
            "responses": {
                "200": {"description": "Electric shape stream"},
                "401": {"description": "Human authentication required"},
                "503": {"description": "Electric synchronization is not configured"}
            }
        }
    });
    document["paths"]["/api/v1/sync/documents"] = json!({
        "get": {
            "operationId": "syncHumanDocuments",
            "description": "Authenticated Electric shape proxy for all human-accessible document metadata and projections.",
            "responses": {
                "200": {"description": "Electric shape stream"},
                "401": {"description": "Human authentication required"},
                "503": {"description": "Electric synchronization is not configured"}
            }
        }
    });
    document["paths"]["/api/v1/teams/{team_id}/sync/agents"] = json!({
        "get": {
            "operationId": "syncHumanAgentRoster",
            "description": "Authenticated Electric shape proxy for the current human-visible agent roster and sessions for a team.",
            "parameters": [{"name": "team_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
            "responses": {
                "200": {"description": "Electric shape stream"},
                "401": {"description": "Human authentication required"},
                "403": {"description": "Team membership required"},
                "503": {"description": "Electric synchronization is not configured"}
            }
        }
    });

    for (path, method) in DOCUMENTED_ROUTE_SURFACE {
        let path_item = &mut document["paths"][*path];
        if !path_item.is_object() {
            *path_item = json!({});
        }
        if !path_item[*method].is_object() {
            path_item[*method] = json!({
                "operationId": format!("{}{}", method, path.replace(['/', '{', '}'], "_")),
                "responses": {"200": {"description": "Successful response"}}
            });
        }
    }
    document["paths"]["/api/v1/projects/{project_id}/agent-roles"]["post"] = json!({
        "operationId": "createAgentRole",
        "parameters": [{"name": "project_id", "in": "path", "required": true, "schema": {"type": "string", "format": "uuid"}}],
        "requestBody": {"required": true, "content": {"application/json": {"schema": {"$ref": "#/components/schemas/CreateAgentRoleRequest"}}}},
        "responses": {"204": {"description": "Agent role created"}}
    });
    document["components"]["schemas"]["CreateAgentRoleRequest"] = json!({
        "type": "object",
        "required": ["display_name"],
        "properties": {"display_name": {"type": "string"}, "owner_account_id": {"type": ["string", "null"], "format": "uuid"}, "capabilities": {"type": "array", "items": {"type": "string"}}}
    });
    document
}

const DOCUMENTED_ROUTE_SURFACE: &[(&str, &str)] = &[
    ("/api/v1/auth/me", "get"),
    ("/api/v1/auth/me/avatar", "get"),
    ("/api/v1/auth/me/avatar", "put"),
    ("/api/v1/auth/me/avatar", "delete"),
    ("/api/v1/teams/{team_id}", "patch"),
    ("/api/v1/organizations/{organization_id}/logo", "get"),
    ("/api/v1/organizations/{organization_id}/logo", "put"),
    ("/api/v1/organizations/{organization_id}/logo", "delete"),
    ("/api/v1/approvals", "get"),
    ("/api/v1/projects/{project_id}/events", "get"),
    ("/api/v1/projects/{project_id}/sync/issues", "get"),
    ("/api/v1/sync/issues", "get"),
    ("/api/v1/sync/documents", "get"),
    ("/api/v1/teams/{team_id}/sync/agents", "get"),
    ("/api/v1/sync/inbox", "get"),
    ("/api/v1/sync/navigation", "get"),
    ("/api/v1/sync/approvals", "get"),
    (
        "/api/v1/projects/{project_id}/sync/issues/{issue_id}/activity",
        "get",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/edges",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/collaborators",
        "get",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/collaborators",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/quarantined-attempts",
        "get",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/holds",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/holds/{hold_id}/release",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/takeover",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/recovery/{checklist_id}/complete",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/approvals",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/approvals/{approval_id}/approve",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/approvals/{approval_id}/reject",
        "post",
    ),
    ("/api/v1/projects/{project_id}/agent-roles", "post"),
    ("/api/v1/projects/{project_id}/onboarding-sample", "post"),
    (
        "/api/v1/projects/{project_id}/agent-roles/{role_id}/sessions",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/agent-sessions/{session_id}/revoke",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/agent-roles/{role_id}/revoke",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/integrations/github/import",
        "post",
    ),
    ("/api/v1/projects", "post"),
    ("/api/v1/projects/{project_id}/invites", "post"),
    (
        "/api/v1/projects/{project_id}/invites/{invite_id}/revoke",
        "post",
    ),
    (
        "/api/v1/projects/{project_id}/outbox/{message_id}/redrive",
        "post",
    ),
    ("/api/v1/invites/accept", "post"),
    ("/api/v1/renew", "post"),
    ("/api/v1/report", "post"),
    ("/api/v1/context/{issue_id}/resources/{resource}", "get"),
    ("/api/v1/recovery/{issue_id}/quarantined-attempts", "get"),
    ("/api/v1/integrations/github/webhook", "post"),
    ("/api/v1/organizations/{organization_id}/documents", "get"),
    ("/api/v1/organizations/{organization_id}/documents", "post"),
    ("/api/v1/teams/{team_id}/documents", "get"),
    ("/api/v1/teams/{team_id}/documents", "post"),
    ("/api/v1/projects/{project_id}/documents", "get"),
    ("/api/v1/projects/{project_id}/documents", "post"),
    ("/api/v1/documents/{document_id}", "get"),
    ("/api/v1/documents/{document_id}", "patch"),
    ("/api/v1/documents/{document_id}", "delete"),
    (
        "/api/v1/projects/{project_id}/issues/{issue_id}/description-document",
        "get",
    ),
    ("/api/v1/documents/{document_id}/version", "get"),
    ("/api/v1/documents/{document_id}/version", "patch"),
    ("/api/v1/documents/{document_id}/loro-snapshot", "get"),
    ("/api/v1/documents/{document_id}/loro-updates", "post"),
    ("/api/v1/documents/{document_id}/loro-sync", "get"),
    ("/api/v1/documents/{document_id}/agent-read", "post"),
    ("/api/v1/documents/{document_id}/agent-edit", "post"),
    ("/api/v1/documents/{document_id}/references", "get"),
    ("/api/v1/documents/{document_id}/references", "put"),
    ("/api/v1/documents/{document_id}/backlinks", "get"),
    ("/api/v1/documents/{document_id}/attachments", "post"),
    ("/api/v1/attachment-uploads/{upload_id}", "put"),
    ("/api/v1/attachment-uploads/{upload_id}/complete", "post"),
    ("/api/v1/attachments/{attachment_id}", "get"),
];

async fn openapi_document() -> Json<Value> {
    Json(openapi_document_value())
}

async fn readiness(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<HealthResponse>), ApiError> {
    if let Err(error) = state.application.database().ping().await {
        tracing::error!(error = %error, "readiness probe database check failed");
        return Err(ApiError::NotReady);
    }
    Ok((StatusCode::OK, Json(HealthResponse { status: "ready" })))
}

#[derive(Debug, Deserialize)]
pub struct ReadyRequest {
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ReadyResponse {
    pub issues: Vec<riichi_persistence::ReadyIssue>,
    pub snapshot_cursor: String,
    pub exclusions: Vec<riichi_persistence::ReadyExclusion>,
}

#[derive(Debug, Deserialize)]
struct ContextRequest {
    issue_id: Uuid,
    max_bytes: Option<usize>,
    document_frontiers: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct ClaimRequest {
    pub issue_id: Uuid,
    pub requested_ttl_seconds: Option<i64>,
    pub idempotency_key: String,
}

#[derive(Debug, Deserialize)]
pub struct RenewRequest {
    pub lease_id: Uuid,
    pub fencing_token: i64,
    pub requested_ttl_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct ReportRequest {
    pub lease_id: Uuid,
    pub fencing_token: i64,
    pub action: ReportAction,
    pub comment: Option<String>,
    pub resolution_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReportBatchRequest {
    lease_id: Uuid,
    fencing_token: i64,
    idempotency_key: String,
    operations: Vec<riichi_persistence::ReportOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportAction {
    Release,
    Complete,
}

#[derive(Debug, Serialize)]
pub struct RenewResponse {
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: String,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Serialize)]
struct HumanMeResponse {
    account_id: Uuid,
    email: Option<String>,
    display_name: Option<String>,
    avatar_url: Option<String>,
    memberships: Vec<HumanMembershipResponse>,
    teams: Vec<HumanTeamMembershipResponse>,
}

#[derive(Debug, Serialize)]
struct HumanMembershipResponse {
    project_id: Uuid,
    project_name: String,
    role: String,
}

#[derive(Debug, Serialize)]
struct HumanTeamMembershipResponse {
    team_id: Uuid,
    team_name: String,
    team_key: String,
    role: String,
}

#[derive(Debug, Serialize)]
struct NavigationResponse {
    organizations: Vec<NavigationOrganizationResponse>,
}

#[derive(Debug, Serialize)]
struct NavigationOrganizationResponse {
    id: Uuid,
    name: String,
    role: String,
    logo_url: Option<String>,
    teams: Vec<NavigationTeamResponse>,
}

#[derive(Debug, Serialize)]
struct NavigationTeamResponse {
    id: Uuid,
    name: String,
    key: String,
    emoji: Option<String>,
    projects: Vec<NavigationProjectResponse>,
    views: Vec<NavigationViewResponse>,
}

#[derive(Debug, Serialize)]
struct NavigationViewResponse {
    id: Uuid,
    name: String,
}

#[derive(Debug, Serialize)]
struct NavigationProjectResponse {
    id: Uuid,
    name: String,
    role: String,
}

#[derive(Debug, Serialize)]
struct HumanQueueResponse {
    issues: Vec<riichi_persistence::HumanQueueIssue>,
}

#[derive(Debug, Deserialize)]
struct InboxQuery {
    unread_only: Option<bool>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize)]
struct InboxResponse {
    notifications: Vec<riichi_persistence::Notification>,
    unread_count: i64,
}

#[derive(Debug, Serialize)]
struct UnreadCountResponse {
    unread_count: i64,
}

fn default_issue_status() -> String {
    "todo".to_owned()
}

#[derive(Debug, Deserialize)]
struct CreateIssueRequest {
    project_id: Option<Uuid>,
    title: String,
    #[serde(default)]
    body: String,
    #[serde(default = "default_issue_status")]
    status: String,
    #[serde(default)]
    agent_eligible: bool,
    #[serde(default)]
    spec_complete: bool,
    #[serde(default)]
    rank: i64,
    #[serde(default)]
    labels: Vec<String>,
    assignee_account_id: Option<Uuid>,
    parent_issue_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct UpdateTeamRequest {
    emoji: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateCommentRequest {
    content: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpdateIssueRequest {
    expected_version: i64,
    title: Option<String>,
    status: Option<String>,
    importance: Option<String>,
    agent_eligible: Option<bool>,
    spec_complete: Option<bool>,
    rank: Option<i64>,
    labels: Option<Vec<String>>,
    assignee_account_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
struct DeleteIssueQuery {
    expected_version: i64,
}

#[derive(Debug, Deserialize)]
struct CreateIssueEdgeRequest {
    source_issue_id: Uuid,
    target_issue_id: Uuid,
    edge_type: String,
}

#[derive(Debug, Deserialize)]
struct CreateHoldRequest {
    hold_type: String,
    reason: String,
    expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TakeoverRequest {
    reason: String,
}

#[derive(Debug, Deserialize)]
struct CompleteRecoveryRequest {
    expected_version: i64,
    action: String,
    resolution_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateApprovalRequest {
    target_version: i64,
    proposed_operation: riichi_persistence::ApprovalOperation,
    expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct GrantCollaboratorRequest {
    lease_id: Uuid,
    session_id: Uuid,
    capability: String,
    grant_mode: String,
    expires_in_seconds: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CollaboratorLeaseQuery {
    lease_id: Uuid,
}

#[derive(Debug, Serialize)]
struct CollaboratorResponse {
    collaborators: Vec<riichi_persistence::LeaseCollaborator>,
}

#[derive(Debug, Serialize)]
struct GithubWebhookResponse {
    accepted: bool,
}

#[derive(Debug, Deserialize)]
struct GithubImportRequest {
    repository: String,
    max_issues: Option<usize>,
}

#[derive(Debug, Serialize)]
struct GithubImportResponse {
    repository: String,
    imported: usize,
    pull_requests_skipped: usize,
    issue_numbers: Vec<i64>,
}

#[derive(Debug, Serialize)]
struct AgentRosterResponse {
    roles: Vec<riichi_persistence::AgentRole>,
    sessions: Vec<riichi_persistence::AgentSession>,
}

fn default_agent_capabilities() -> Vec<String> {
    [
        "comment",
        "request_spec",
        "discover",
        "complete",
        "release",
        "doc.read",
        "doc.apply_edit",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

#[derive(Debug, Deserialize)]
struct CreateAgentRoleRequest {
    display_name: String,
    owner_account_id: Option<Uuid>,
    #[serde(default = "default_agent_capabilities")]
    capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CreateAgentSessionRequest {
    lifetime_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateAgentSessionResponse {
    session_id: Uuid,
    agent_token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
struct CreateProjectRequest {
    name: String,
}

#[derive(Debug, Serialize)]
struct CreateProjectResponse {
    project_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct CreateInviteRequest {
    role: String,
    email_hint: Option<String>,
    expires_in_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct CreateInviteResponse {
    invite_id: Uuid,
    project_id: Uuid,
    role: String,
    email_hint: Option<String>,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
struct AcceptInviteRequest {
    token: String,
}

#[derive(Debug, Serialize)]
struct AcceptInviteResponse {
    project_id: Uuid,
    role: String,
}

async fn human_principal(state: &AppState, jar: &CookieJar) -> Result<HumanPrincipal, ApiError> {
    let auth = state.auth.as_ref().ok_or(ApiError::AuthNotConfigured)?;
    let session_token = jar
        .get(auth.cookie_name())
        .map(|cookie| cookie.value().to_owned())
        .ok_or(ApiError::HumanUnauthenticated)?;
    auth.authenticate(&state.application.database(), &session_token)
        .await
        .map_err(ApiError::from)?
        .ok_or(ApiError::HumanUnauthenticated)
}

const ELECTRIC_PROTOCOL_QUERY_PARAMS: &[&str] = &[
    "live",
    "live_sse",
    "experimental_live_sse",
    "handle",
    "offset",
    "cursor",
    "expired_handle",
    "log",
    "subset__where",
    "subset__limit",
    "subset__offset",
    "subset__order_by",
    "subset__params",
    "subset__where_expr",
    "subset__order_by_expr",
    "cache-buster",
];

fn electric_shape_url_for(
    base_url: &str,
    table: &str,
    predicate: &str,
    params: &[(u8, String)],
    query: Option<&str>,
    source_secret: Option<&str>,
) -> Result<reqwest::Url, ApiError> {
    let mut url = reqwest::Url::parse(base_url).map_err(|_| ApiError::ElectricInvalidUrl)?;
    url.set_path("/v1/shape");
    url.set_query(None);

    if let Some(query) = query {
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            if ELECTRIC_PROTOCOL_QUERY_PARAMS.contains(&key.as_ref()) {
                url.query_pairs_mut().append_pair(&key, &value);
            }
        }
    }

    url.query_pairs_mut()
        .append_pair("table", table)
        .append_pair("where", predicate);
    for (index, value) in params {
        url.query_pairs_mut()
            .append_pair(&format!("params[{index}]"), value);
    }
    if let Some(source_secret) = source_secret {
        url.query_pairs_mut().append_pair("secret", source_secret);
    }
    Ok(url)
}

async fn electric_issue_shape(
    State(state): State<AppState>,
    Path(project_id): Path<Uuid>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Viewer) {
        return Err(ApiError::ProjectAccessDenied);
    }
    electric_shape_proxy(
        state,
        "issue_metadata_sync",
        "project_id = $1",
        vec![(1, project_id.to_string())],
        uri,
        ElectricShapeAccess {
            account_id: principal.account.id,
            session_id: principal.session_id,
            project_id: Some(project_id),
        },
    )
    .await
}

async fn electric_human_issue_shape(
    State(state): State<AppState>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    electric_shape_proxy(
        state,
        "human_issue_sync",
        "account_id = $1",
        vec![(1, principal.account.id.to_string())],
        uri,
        ElectricShapeAccess::account(&principal),
    )
    .await
}

async fn electric_human_document_shape(
    State(state): State<AppState>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    electric_shape_proxy(
        state,
        "human_document_sync",
        "account_id = $1",
        vec![(1, principal.account.id.to_string())],
        uri,
        ElectricShapeAccess::account(&principal),
    )
    .await
}

async fn electric_human_agent_shape(
    State(state): State<AppState>,
    Path(team_id): Path<Uuid>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    require_team_viewer(&principal, team_id)?;
    electric_shape_proxy(
        state,
        "human_agent_sync",
        "team_id = $1 AND account_id = $2",
        vec![
            (1, team_id.to_string()),
            (2, principal.account.id.to_string()),
        ],
        uri,
        ElectricShapeAccess::account(&principal),
    )
    .await
}

async fn electric_issue_activity_shape(
    State(state): State<AppState>,
    Path((project_id, issue_id)): Path<(Uuid, Uuid)>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    if !principal.can_access_project(project_id, riichi_auth::HumanRole::Viewer) {
        return Err(ApiError::ProjectAccessDenied);
    }
    electric_shape_proxy(
        state,
        "issue_activity_sync",
        "project_id = $1 AND issue_id = $2",
        vec![(1, project_id.to_string()), (2, issue_id.to_string())],
        uri,
        ElectricShapeAccess {
            account_id: principal.account.id,
            session_id: principal.session_id,
            project_id: Some(project_id),
        },
    )
    .await
}

async fn electric_inbox_shape(
    State(state): State<AppState>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    electric_shape_proxy(
        state,
        "notifications",
        "recipient_account_id = $1",
        vec![(1, principal.account.id.to_string())],
        uri,
        ElectricShapeAccess::account(&principal),
    )
    .await
}

async fn electric_navigation_shape(
    State(state): State<AppState>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    electric_shape_proxy(
        state,
        "navigation_sync",
        "account_id = $1",
        vec![(1, principal.account.id.to_string())],
        uri,
        ElectricShapeAccess::account(&principal),
    )
    .await
}

async fn electric_approval_shape(
    State(state): State<AppState>,
    uri: Uri,
    jar: CookieJar,
) -> Result<Response, ApiError> {
    let principal = human_principal(&state, &jar).await?;
    electric_shape_proxy(
        state,
        "approval_sync",
        "account_id = $1",
        vec![(1, principal.account.id.to_string())],
        uri,
        ElectricShapeAccess::account(&principal),
    )
    .await
}

#[derive(Clone, Copy)]
struct ElectricShapeAccess {
    account_id: Uuid,
    session_id: Uuid,
    project_id: Option<Uuid>,
}

impl ElectricShapeAccess {
    fn account(principal: &HumanPrincipal) -> Self {
        Self {
            account_id: principal.account.id,
            session_id: principal.session_id,
            project_id: None,
        }
    }
}

async fn electric_shape_proxy(
    state: AppState,
    table: &str,
    predicate: &str,
    params: Vec<(u8, String)>,
    uri: Uri,
    access: ElectricShapeAccess,
) -> Result<Response, ApiError> {
    let base_url = state
        .electric_url
        .as_deref()
        .ok_or(ApiError::ElectricNotConfigured)?;
    let target = electric_shape_url_for(
        base_url,
        table,
        predicate,
        &params,
        uri.query(),
        state.electric_secret.as_deref(),
    )?;
    let upstream = state
        .http_client
        .get(target)
        .send()
        .await
        .map_err(ApiError::ElectricUpstream)?;

    let status = upstream.status();
    let upstream_headers = upstream.headers().clone();
    let mut upstream_stream = upstream.bytes_stream();
    let mut access_wakeups = state.event_wakeups.subscribe();
    let database = state.application.database();
    let mut authorization_check = tokio::time::interval(std::time::Duration::from_secs(1));
    authorization_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let body = Body::from_stream(async_stream::stream! {
        loop {
            tokio::select! {
                chunk = upstream_stream.next() => {
                    match chunk {
                        Some(chunk) => yield chunk,
                        None => break,
                    }
                }
                account_id = access_wakeups.recv() => {
                    match account_id {
                        Ok(EventWakeup::Account(account_id)) if account_id == access.account_id => break,
                        Ok(_) => continue,
                        Err(_) => break,
                    }
                }
                _ = authorization_check.tick() => {
                    let authorized = match database
                        .human_session_is_active(access.session_id, access.account_id)
                        .await
                    {
                        Ok(false) => Ok(false),
                        Ok(true) => match access.project_id {
                            Some(project_id) => database
                                .human_can_access_project(access.account_id, project_id)
                                .await,
                            None => Ok(true),
                        },
                        Err(error) => Err(error),
                    };
                    match authorized {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(error) => {
                            tracing::warn!(%error, account_id = %access.account_id, "Electric shape authorization check failed");
                            break;
                        }
                    }
                }
            }
        }
    });
    let mut response = Response::new(body);
    *response.status_mut() = status;
    for (name, value) in &upstream_headers {
        if name == header::CONTENT_ENCODING || name == header::CONTENT_LENGTH {
            continue;
        }
        response.headers_mut().insert(name, value.clone());
    }
    response.headers_mut().insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static(
            "electric-offset, electric-handle, electric-schema, electric-cursor",
        ),
    );
    Ok(response)
}

async fn principal(state: &AppState, headers: &HeaderMap) -> Result<(Uuid, Uuid), ApiError> {
    let project_id = headers
        .get("x-riichi-project-id")
        .ok_or_else(|| ApiError::MissingPrincipal("x-riichi-project-id"))?
        .to_str()
        .map_err(|_| ApiError::InvalidPrincipal)?
        .parse()
        .map_err(|_| ApiError::InvalidPrincipal)?;
    let session_id = headers
        .get("x-riichi-session-id")
        .ok_or_else(|| ApiError::MissingPrincipal("x-riichi-session-id"))?
        .to_str()
        .map_err(|_| ApiError::InvalidPrincipal)?
        .parse()
        .map_err(|_| ApiError::InvalidPrincipal)?;
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split_once(' '))
        .filter(|(scheme, token)| scheme.eq_ignore_ascii_case("bearer") && !token.trim().is_empty())
        .map(|(_, token)| token.trim())
        .ok_or(ApiError::InvalidAgentCredentials)?;
    if !state
        .application
        .database()
        .authenticate_agent_session(project_id, session_id, authorization)
        .await?
    {
        return Err(ApiError::InvalidAgentCredentials);
    }
    Ok((project_id, session_id))
}

#[derive(Debug)]
enum ApiError {
    MissingPrincipal(&'static str),
    InvalidPrincipal,
    InvalidAgentCredentials,
    Persistence(PersistenceError),
    AuthNotConfigured,
    HumanUnauthenticated,
    InvalidRequest,
    NotFound,
    ProjectAccessDenied,
    ProjectActionDenied,
    NotReady,
    OutboxNotFound,
    Auth(AuthError),
    GitHubNotConfigured,
    GitHubClient(ClientError),
    GitHubWebhook(WebhookError),
    ElectricNotConfigured,
    ElectricInvalidUrl,
    ElectricUpstream(reqwest::Error),
}

impl From<PersistenceError> for ApiError {
    fn from(error: PersistenceError) -> Self {
        Self::Persistence(error)
    }
}

impl From<AuthError> for ApiError {
    fn from(error: AuthError) -> Self {
        Self::Auth(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match self {
            Self::MissingPrincipal(header) => (
                StatusCode::UNAUTHORIZED,
                "missing_principal",
                format!("missing required development header: {header}"),
            ),
            Self::InvalidPrincipal => (
                StatusCode::UNAUTHORIZED,
                "invalid_principal",
                "development principal headers must contain UUIDs".to_owned(),
            ),
            Self::InvalidAgentCredentials => (
                StatusCode::UNAUTHORIZED,
                "invalid_agent_credentials",
                "a valid agent credential is required".to_owned(),
            ),
            Self::AuthNotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "auth_not_configured",
                "human authentication is not configured".to_owned(),
            ),
            Self::HumanUnauthenticated => (
                StatusCode::UNAUTHORIZED,
                "human_unauthenticated",
                "a valid human session is required".to_owned(),
            ),
            Self::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "the request could not be accepted".to_owned(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "the requested resource was not found".to_owned(),
            ),
            Self::ProjectAccessDenied => (
                StatusCode::FORBIDDEN,
                "project_access_denied",
                "you don't have access to this project information".to_owned(),
            ),
            Self::ProjectActionDenied => (
                StatusCode::FORBIDDEN,
                "project_action_denied",
                "you don't have permission to perform this action in this project".to_owned(),
            ),
            Self::NotReady => (
                StatusCode::SERVICE_UNAVAILABLE,
                "not_ready",
                "the service dependencies are not ready".to_owned(),
            ),
            Self::OutboxNotFound => (
                StatusCode::NOT_FOUND,
                "outbox_message_not_found",
                "the outbox message is not available for redrive".to_owned(),
            ),
            Self::Auth(error) => match error {
                AuthError::InvalidState => (
                    StatusCode::BAD_REQUEST,
                    "invalid_auth_state",
                    "the login state was invalid or expired".to_owned(),
                ),
                AuthError::ProviderRejected(_) => (
                    StatusCode::BAD_REQUEST,
                    "provider_rejected",
                    "the identity provider rejected the login".to_owned(),
                ),
                AuthError::InsufficientRole => (
                    StatusCode::FORBIDDEN,
                    "insufficient_role",
                    "you don't have permission to perform this action in this project".to_owned(),
                ),
                AuthError::InvalidInvite => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_invite",
                    "the invite is invalid, expired, revoked, or already accepted".to_owned(),
                ),
                AuthError::InvalidInviteRole => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_invite_role",
                    "the invite role is not supported".to_owned(),
                ),
                AuthError::ProjectNameRequired => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "project_name_required",
                    "project name cannot be empty".to_owned(),
                ),
                AuthError::ProjectNameTooLong => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "project_name_too_long",
                    "project name is too long".to_owned(),
                ),
                internal_error => {
                    error!(error = %internal_error, "internal authentication error");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "authentication_error",
                        "human authentication could not be completed".to_owned(),
                    )
                }
            },
            Self::GitHubNotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "github_not_configured",
                "GitHub integration is not configured".to_owned(),
            ),
            Self::GitHubClient(error) => {
                let status = match error {
                    ClientError::InvalidRepository | ClientError::InvalidImportLimit => {
                        StatusCode::UNPROCESSABLE_ENTITY
                    }
                    ClientError::HttpStatus(status) if status == StatusCode::UNAUTHORIZED => {
                        StatusCode::BAD_GATEWAY
                    }
                    ClientError::HttpStatus(_) | ClientError::Request(_) => StatusCode::BAD_GATEWAY,
                    ClientError::MalformedResponse(_) => StatusCode::BAD_GATEWAY,
                };
                (status, "github_import_failed", error.to_string())
            }
            Self::GitHubWebhook(error) => {
                let status = match error {
                    WebhookError::InvalidSignature => StatusCode::UNAUTHORIZED,
                    WebhookError::UnsupportedEvent
                    | WebhookError::UnsupportedAction
                    | WebhookError::PullRequestIgnored
                    | WebhookError::Malformed
                    | WebhookError::PayloadTooLarge => StatusCode::UNPROCESSABLE_ENTITY,
                };
                (status, "invalid_github_webhook", error.to_string())
            }
            Self::ElectricNotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "electric_not_configured",
                "metadata synchronization is not configured".to_owned(),
            ),
            Self::ElectricInvalidUrl => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "electric_configuration_invalid",
                "metadata synchronization has an invalid upstream URL".to_owned(),
            ),
            Self::ElectricUpstream(error) => {
                error!(error = %error, "Electric shape request failed");
                (
                    StatusCode::BAD_GATEWAY,
                    "electric_upstream_failed",
                    "metadata synchronization is temporarily unavailable".to_owned(),
                )
            }
            Self::Persistence(error) => match error {
                PersistenceError::SessionNotActive => (
                    StatusCode::UNAUTHORIZED,
                    "session_not_active",
                    error.to_string(),
                ),
                PersistenceError::IssueNotFound => {
                    (StatusCode::NOT_FOUND, "issue_not_found", error.to_string())
                }
                PersistenceError::Contended => {
                    (StatusCode::CONFLICT, "contended", error.to_string())
                }
                PersistenceError::StaleLease => {
                    (StatusCode::CONFLICT, "stale_lease", error.to_string())
                }
                PersistenceError::LeaseNotFound => {
                    (StatusCode::NOT_FOUND, "lease_not_found", error.to_string())
                }
                PersistenceError::LeaseNotActive => {
                    (StatusCode::CONFLICT, "lease_not_active", error.to_string())
                }
                PersistenceError::IssueNotEligible => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "issue_not_eligible",
                    error.to_string(),
                ),
                PersistenceError::ResolutionSummaryRequired => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "resolution_summary_required",
                    error.to_string(),
                ),
                PersistenceError::IdempotencyConflict => (
                    StatusCode::CONFLICT,
                    "idempotency_conflict",
                    error.to_string(),
                ),
                PersistenceError::IdempotencyKeyRequired => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "idempotency_key_required",
                    error.to_string(),
                ),
                PersistenceError::AgentTokenRequired => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "agent_token_required",
                    error.to_string(),
                ),
                PersistenceError::InvalidIssue(message) => {
                    (StatusCode::UNPROCESSABLE_ENTITY, "invalid_issue", message)
                }
                PersistenceError::VersionConflict => {
                    (StatusCode::CONFLICT, "version_conflict", error.to_string())
                }
                PersistenceError::InvalidEdge => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_edge",
                    error.to_string(),
                ),
                PersistenceError::EdgeCycle => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "edge_cycle",
                    error.to_string(),
                ),
                PersistenceError::EdgeNotFound => {
                    (StatusCode::NOT_FOUND, "edge_not_found", error.to_string())
                }
                PersistenceError::HoldNotFound => {
                    (StatusCode::NOT_FOUND, "hold_not_found", error.to_string())
                }
                PersistenceError::ContextResourceNotFound => (
                    StatusCode::NOT_FOUND,
                    "context_resource_not_found",
                    error.to_string(),
                ),
                PersistenceError::DocumentProjectionPending => (
                    StatusCode::CONFLICT,
                    "document_projection_pending",
                    error.to_string(),
                ),
                PersistenceError::DocumentFrontierUnavailable => (
                    StatusCode::CONFLICT,
                    "document_frontier_unavailable",
                    error.to_string(),
                ),
                PersistenceError::TakeoverNotAvailable => (
                    StatusCode::CONFLICT,
                    "takeover_not_available",
                    error.to_string(),
                ),
                PersistenceError::RecoveryNotFound => (
                    StatusCode::NOT_FOUND,
                    "recovery_not_found",
                    error.to_string(),
                ),
                PersistenceError::ApprovalNotFound => (
                    StatusCode::NOT_FOUND,
                    "approval_not_found",
                    error.to_string(),
                ),
                PersistenceError::ApprovalExpired => {
                    (StatusCode::CONFLICT, "approval_expired", error.to_string())
                }
                PersistenceError::ApprovalSuperseded => (
                    StatusCode::CONFLICT,
                    "approval_superseded",
                    error.to_string(),
                ),
                PersistenceError::AgentSessionNotFound => (
                    StatusCode::NOT_FOUND,
                    "agent_session_not_found",
                    error.to_string(),
                ),
                PersistenceError::AgentRoleNotFound => (
                    StatusCode::NOT_FOUND,
                    "agent_role_not_found",
                    error.to_string(),
                ),
                PersistenceError::CapabilityDenied => (
                    StatusCode::FORBIDDEN,
                    "capability_denied",
                    error.to_string(),
                ),
                PersistenceError::InvalidCapability => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_capability",
                    error.to_string(),
                ),
                PersistenceError::CollaboratorNotFound => (
                    StatusCode::NOT_FOUND,
                    "collaborator_not_found",
                    error.to_string(),
                ),
                PersistenceError::DocumentNotFound => (
                    StatusCode::NOT_FOUND,
                    "document_not_found",
                    error.to_string(),
                ),
                PersistenceError::DocumentAccessDenied => (
                    StatusCode::FORBIDDEN,
                    "document_access_denied",
                    "you don't have access to this document".to_owned(),
                ),
                PersistenceError::DocumentVersionConflict => (
                    StatusCode::CONFLICT,
                    "document_version_conflict",
                    error.to_string(),
                ),
                PersistenceError::LoroFrontierConflict => (
                    StatusCode::CONFLICT,
                    "loro_frontier_conflict",
                    error.to_string(),
                ),
                PersistenceError::InvalidDocument(message) => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_document",
                    message,
                ),
                PersistenceError::AttachmentUploadNotFound => (
                    StatusCode::NOT_FOUND,
                    "attachment_upload_not_found",
                    error.to_string(),
                ),
                PersistenceError::AttachmentVerificationFailed => (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "attachment_verification_failed",
                    error.to_string(),
                ),
                PersistenceError::Database(error) => {
                    tracing::error!(error = %error, "database error serving request");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "database_error",
                        "the database operation failed".to_owned(),
                    )
                }
                PersistenceError::Migration(error) => {
                    tracing::error!(error = %error, "migration error serving request");
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "migration_error",
                        "the service is not ready".to_owned(),
                    )
                }
            },
        };

        (status, Json(ErrorResponse { code, message })).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn internal_database_errors_use_generic_public_messages() {
        let response = ApiError::Persistence(PersistenceError::Database(sqlx::Error::Protocol(
            "secret schema detail".to_owned(),
        )))
        .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert!(body.contains("the database operation failed"));
        assert!(!body.contains("secret schema detail"));
    }

    #[test]
    fn issue_metadata_updates_reject_the_legacy_body_field() {
        let error = serde_json::from_value::<UpdateIssueRequest>(serde_json::json!({
            "expected_version": 1,
            "body": "legacy description write",
        }))
        .expect_err("the issue metadata command must not accept document content");

        assert!(error.to_string().contains("unknown field `body`"));
    }

    #[test]
    fn electric_shape_proxy_pins_the_table_and_project_scope() {
        let project_id = Uuid::from_u128(7);
        let url = electric_shape_url_for(
            "http://electric.internal:3000/ignored",
            "issue_metadata_sync",
            "project_id = $1",
            &[(1, project_id.to_string())],
            Some("table=users&where=org_id%20%3D%20%271%27&live=true&offset=10_2"),
            Some("server-only-secret"),
        )
        .unwrap();

        let pairs: Vec<_> = url.query_pairs().into_owned().collect();
        assert!(pairs.contains(&("table".to_owned(), "issue_metadata_sync".to_owned())));
        assert!(pairs.contains(&("where".to_owned(), "project_id = $1".to_owned())));
        assert!(pairs.contains(&("params[1]".to_owned(), project_id.to_string())));
        assert!(pairs.contains(&("live".to_owned(), "true".to_owned())));
        assert!(pairs.contains(&("offset".to_owned(), "10_2".to_owned())));
        assert!(pairs.contains(&("secret".to_owned(), "server-only-secret".to_owned())));
        assert!(!pairs.iter().any(|(key, value)| {
            (key == "table" && value == "users") || (key == "where" && value.contains("org_id"))
        }));
    }

    #[test]
    fn openapi_document_describes_the_four_agent_intentions() {
        let document = openapi_document_value();
        assert_eq!(document["openapi"], "3.0.3");
        for path in [
            "/api/v1/ready",
            "/api/v1/claim",
            "/api/v1/context",
            "/api/v1/report/batch",
        ] {
            assert!(
                document["paths"][path]["post"].is_object(),
                "missing {path}"
            );
        }
        assert!(document["paths"]["/api/v1/inbox"].is_object());
        assert!(document["paths"]["/api/v1/sync/issues"].is_object());
        assert!(document["paths"]["/api/v1/sync/documents"].is_object());
        assert!(document["paths"]["/api/v1/teams/{team_id}/sync/agents"].is_object());
        assert!(document["paths"]["/api/v1/sync/navigation"].is_object());
        assert!(document["paths"]["/api/v1/sync/approvals"].is_object());
        assert!(document["paths"]["/api/v1/teams/{team_id}/agents"].is_object());
        assert!(document["paths"]["/api/v1/views"]["get"].is_object());
        assert!(document["paths"]["/api/v1/views"]["post"].is_object());
        assert!(document["paths"]["/api/v1/views/{view_id}"]["delete"].is_object());
        assert!(document["paths"]["/api/v1/views/{view_id}/pin"]["post"].is_object());
        assert!(document["paths"]["/api/v1/projects/{project_id}/views"]["get"].is_object());
        assert!(document["paths"]["/api/v1/projects/{project_id}/views"]["post"].is_object());
        assert!(
            document["paths"]["/api/v1/projects/{project_id}/views/{view_id}"]["delete"]
                .is_object()
        );
        assert!(
            document["paths"]["/api/v1/projects/{project_id}/views/{view_id}/pin"]["post"]
                .is_object()
        );
        assert!(document["paths"]["/api/v1/projects/{project_id}/agent-roles/{role_id}/sessions"]["post"].is_object());
        assert!(
            document["paths"]["/api/v1/projects/{project_id}/onboarding-sample"]["post"]
                .is_object()
        );
        assert_eq!(
            document["paths"]["/api/v1/documents/{document_id}/loro-snapshot"]["get"]["responses"]
                ["200"]["content"]["application/octet-stream"]["schema"]["format"],
            "binary"
        );
        for (path, method) in DOCUMENTED_ROUTE_SURFACE {
            assert!(
                document["paths"][*path][*method].is_object(),
                "missing {method} {path}"
            );
        }
    }
}
