mod auth;
mod collaborators;
mod context;
mod controls;
mod dispatch;
mod document_jobs;
mod documents;
mod error;
mod github;
mod human;
mod loro;
mod models;
mod notifications;
mod organization;
mod outbox;
mod reports;
mod sessions;
mod triage;
mod views;

use chrono::{DateTime, Duration, Utc};
use error::PersistenceError;
use models::{ClaimRecord, ReadyIssueRecord, ReportAction, ReportInput, SessionRecord};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction, postgres::PgListener};
use std::future::Future;
use uuid::Uuid;

pub use auth::{
    AcceptedInvite, HumanAccount, HumanMembership, HumanSession, OidcLoginState, ProjectInviteSeed,
    TeamMembership,
};
pub use context::{ContextResponse, ContextSection};
pub use document_jobs::DocumentJobRetryOutcome;
pub use documents::{
    AttachmentUploadSeed, DocumentContentUpdate, DocumentCreate, DocumentReferenceInput,
};
pub use error::PersistenceError as Error;
pub use loro::{
    LoroSchemaMigration, LoroSnapshotHistoryRecord, LoroSnapshotRecord, LoroSnapshotSeed,
    LoroUpdateOutcome, LoroUpdateRecord, LoroUpdateSeed, MAX_LORO_UPDATE_BYTES,
};
pub use models::{
    ActivityRecord as Activity, AgentRoleRecord as AgentRole, AgentSessionRecord as AgentSession,
    ApprovalOperation, ApprovalRequestRecord as ApprovalRequest, AttachmentRecord as Attachment,
    AttachmentUploadRecord as AttachmentUpload, ClaimRecord as Claim, CommentRecord as Comment,
    DeliveryEventRecord, DispatchHoldRecord as DispatchHold, DocumentJobRecord as DocumentJob,
    DocumentRecord as Document, DocumentReferenceRecord as DocumentReference,
    DocumentVersionRecord as DocumentVersion, ExternalIssueRecord,
    GlobalApprovalRequestRecord as GlobalApprovalRequest, HumanQueueIssueRecord as HumanQueueIssue,
    IssueCreate, IssueEdgeRecord as IssueEdge, IssueProjectRecord, IssueRecord,
    IssueSeed as NewIssue, IssueUpdate, IssueUpdateResult,
    LeaseCollaboratorRecord as LeaseCollaborator, NotificationRecord as Notification,
    OutboxMessage as Message, QuarantinedAttemptRecord as QuarantinedAttempt,
    ReadyExclusionRecord as ReadyExclusion, ReadyIssueRecord as ReadyIssue, ReadySnapshot,
    RecoveryChecklistRecord as RecoveryChecklist, ReportAction as Action, ReportBatch,
    ReportBatchResult, ReportInput as Report, ReportOperation, SubissueRecord,
};
pub use organization::NavigationRow;
pub use views::SavedViewRecord;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxRetryOutcome {
    Scheduled,
    DeadLettered,
    AlreadyHandled,
}

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

tokio::task_local! {
    static REQUEST_ID: Uuid;
}

pub async fn with_request_id<F>(request_id: Uuid, future: F) -> F::Output
where
    F: Future,
{
    REQUEST_ID.scope(request_id, future).await
}

pub(crate) fn current_request_id() -> Uuid {
    REQUEST_ID
        .try_with(|request_id| *request_id)
        .unwrap_or_else(|_| Uuid::now_v7())
}

impl Database {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self, Error> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), Error> {
        MIGRATOR.run(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn delivery_event_listener(&self) -> Result<PgListener, Error> {
        let mut listener = PgListener::connect_with(&self.pool).await?;
        listener.listen("riichi_delivery_events").await?;
        Ok(listener)
    }

    pub async fn event_listener(&self) -> Result<PgListener, Error> {
        let mut listener = PgListener::connect_with(&self.pool).await?;
        listener.listen("riichi_delivery_events").await?;
        listener.listen("riichi_human_access_events").await?;
        Ok(listener)
    }

    pub async fn loro_update_listener(&self) -> Result<PgListener, Error> {
        let mut listener = PgListener::connect_with(&self.pool).await?;
        listener.listen("riichi_loro_updates").await?;
        Ok(listener)
    }

    pub async fn ping(&self) -> Result<(), Error> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    async fn session<'a, E>(
        &self,
        executor: E,
        project_id: Uuid,
        session_id: Uuid,
    ) -> Result<SessionRecord, Error>
    where
        E: sqlx::Executor<'a, Database = Postgres>,
    {
        sqlx::query_as::<_, SessionRecord>(
            "SELECT s.agent_role_id, r.owner_account_id, r.capabilities,
                    s.max_lifetime_ends_at, s.state
             FROM sessions s
             JOIN agent_roles r ON r.id = s.agent_role_id
             WHERE s.id = $1 AND s.project_id = $2 AND r.revoked_at IS NULL
             FOR UPDATE OF s",
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_optional(executor)
        .await?
        .ok_or(PersistenceError::SessionNotActive)
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DispatchRow {
    #[allow(dead_code)]
    issue_id: Uuid,
    active_lease_id: Option<Uuid>,
    fencing_token: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct LeaseRow {
    #[allow(dead_code)]
    id: Uuid,
    issue_id: Uuid,
    owner_session_id: Uuid,
    fencing_token: i64,
    state: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, sqlx::FromRow)]
struct IdempotencyRow {
    request_hash: Vec<u8>,
    response: serde_json::Value,
}

fn claim_request_hash(issue_id: Uuid, requested_ttl: Duration) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(issue_id.as_bytes());
    hasher.update(requested_ttl.num_seconds().to_le_bytes());
    hasher.finalize().to_vec()
}

fn hash_secret(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}

fn ensure_session_active(session: &SessionRecord) -> Result<(), Error> {
    if session.state != "active" || session.max_lifetime_ends_at <= Utc::now() {
        return Err(PersistenceError::SessionNotActive);
    }
    Ok(())
}

async fn insert_audit(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    session_id: Uuid,
    role_id: Uuid,
    operation: &str,
    target_id: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO audit_records (id, project_id, actor_id, role_id, session_id, request_id,
         operation, target_type, target_id, change_summary)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'issue', $8, '{}'::jsonb)",
    )
    .bind(current_request_id())
    .bind(project_id)
    .bind(session_id)
    .bind(role_id)
    .bind(session_id)
    .bind(Uuid::now_v7())
    .bind(operation)
    .bind(target_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn insert_outbox(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    message_type: &str,
    payload: serde_json::Value,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO outbox_messages (id, project_id, message_type, payload)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(message_type)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
