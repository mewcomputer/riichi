use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct SessionRecord {
    pub agent_role_id: Uuid,
    pub owner_account_id: Option<Uuid>,
    pub capabilities: serde_json::Value,
    pub max_lifetime_ends_at: DateTime<Utc>,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ReadyIssueRecord {
    pub id: Uuid,
    pub display_key: String,
    pub title: String,
    pub status: String,
    pub rank: i64,
    pub rank_scope: String,
    pub dispatch_version: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadyExclusionRecord {
    pub id: Uuid,
    pub display_key: String,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadySnapshot {
    pub snapshot_cursor: String,
    pub issues: Vec<ReadyIssueRecord>,
    pub exclusions: Vec<ReadyExclusionRecord>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct HumanQueueIssueRecord {
    pub team_id: Uuid,
    pub team_name: String,
    pub team_key: String,
    pub project_id: Uuid,
    pub project_name: String,
    pub id: Uuid,
    pub display_key: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub importance: String,
    pub agent_eligible: bool,
    pub spec_complete: bool,
    pub specification_changed_since_review: bool,
    pub unresolved_blocker_count: i32,
    pub active_hold_count: i32,
    pub active_lease_id: Option<Uuid>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub due_date: Option<NaiveDate>,
    pub snoozed_until: Option<NaiveDate>,
    pub workflow_alias: Option<String>,
    pub workflow_alias_version: Option<i64>,
    pub rank: i64,
    pub dispatch_version: i64,
    pub assignee_account_id: Option<Uuid>,
    pub labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct IssueRecord {
    pub project_id: Uuid,
    pub team_id: Uuid,
    pub id: Uuid,
    pub parent_issue_id: Option<Uuid>,
    pub display_key: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub importance: String,
    pub agent_eligible: bool,
    pub spec_complete: bool,
    pub specification_changed_since_review: bool,
    pub assignee_account_id: Option<Uuid>,
    pub version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub due_date: Option<NaiveDate>,
    pub snoozed_until: Option<NaiveDate>,
    pub workflow_alias: Option<String>,
    pub workflow_alias_version: Option<i64>,
    pub rank: i64,
    pub rank_scope: String,
    pub dispatch_version: i64,
    pub unresolved_blocker_count: i32,
    pub active_hold_count: i32,
    pub active_lease_id: Option<Uuid>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub active_owner_session_id: Option<Uuid>,
    pub active_owner_role_id: Option<Uuid>,
    pub labels: Vec<String>,
    #[sqlx(skip)]
    pub edges: Vec<IssueEdgeRecord>,
    #[sqlx(skip)]
    pub holds: Vec<DispatchHoldRecord>,
    #[sqlx(skip)]
    pub collaborators: Vec<LeaseCollaboratorRecord>,
    #[sqlx(skip)]
    pub quarantined_attempt_count: i64,
    #[sqlx(skip)]
    pub approvals: Vec<ApprovalRequestRecord>,
    #[sqlx(skip)]
    pub comments: Vec<CommentRecord>,
    #[sqlx(skip)]
    pub projects: Vec<IssueProjectRecord>,
    #[sqlx(skip)]
    pub children: Vec<SubissueRecord>,
}

#[derive(Debug, Clone)]
pub struct IssueUpdateResult {
    pub issue: IssueRecord,
    pub transaction_id: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct SubissueRecord {
    pub id: Uuid,
    pub display_key: String,
    pub title: String,
    pub status: String,
    pub importance: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DocumentRecord {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub kind: String,
    pub title: String,
    pub parent_document_id: Option<Uuid>,
    pub position: i64,
    pub owner_team_id: Option<Uuid>,
    pub owner_project_id: Option<Uuid>,
    pub provisioning_state: String,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub current_revision: Option<i64>,
    pub plain_text: Option<String>,
    pub sanitized_html: Option<String>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DocumentVersionRecord {
    pub document_id: Uuid,
    pub revision: i64,
    pub content: serde_json::Value,
    pub plain_text: String,
    pub sanitized_html: String,
    pub frontiers: Option<serde_json::Value>,
    pub schema_version: i32,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DocumentReferenceRecord {
    pub document_id: Uuid,
    pub source_block_id: String,
    pub resource_kind: String,
    pub resource_id: Uuid,
    pub reference_kind: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AttachmentRecord {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub state: String,
    pub storage_key: String,
    pub filename: String,
    pub media_type: String,
    pub byte_size: i64,
    pub checksum: Vec<u8>,
    pub uploaded_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AttachmentUploadRecord {
    pub id: Uuid,
    pub attachment_id: Uuid,
    pub expected_byte_size: i64,
    pub expected_checksum: Vec<u8>,
    pub expires_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DocumentJobRecord {
    pub id: Uuid,
    pub document_id: Option<Uuid>,
    pub job_type: String,
    pub idempotency_key: String,
    pub available_at: DateTime<Utc>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub attempt_count: i32,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub dead_lettered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct IssueProjectRecord {
    pub project_id: Uuid,
    pub project_name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct CommentRecord {
    pub id: Uuid,
    pub author_id: Uuid,
    pub role_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub body: String,
    pub content: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ActivityRecord {
    pub id: Uuid,
    pub kind: String,
    pub actor_id: Uuid,
    pub body: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct NotificationRecord {
    pub id: Uuid,
    pub recipient_account_id: Uuid,
    pub kind: String,
    pub project_id: Option<Uuid>,
    pub issue_id: Option<Uuid>,
    pub actor_id: Option<Uuid>,
    pub payload: serde_json::Value,
    pub approval_state: Option<String>,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct IssueEdgeRecord {
    pub id: Uuid,
    pub source_issue_id: Uuid,
    pub target_issue_id: Uuid,
    pub edge_type: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DispatchHoldRecord {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub hold_type: String,
    pub reason: String,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub released_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaimRecord {
    pub issue_id: Uuid,
    pub lease_id: Uuid,
    pub fencing_token: i64,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct OutboxMessage {
    pub id: Uuid,
    pub project_id: Option<Uuid>,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub attempt_count: i32,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct DeliveryEventRecord {
    pub event_seq: i64,
    pub id: Uuid,
    pub project_id: Uuid,
    pub message_type: String,
    pub payload: serde_json::Value,
    pub attempt_count: i32,
}

#[derive(Debug, Clone)]
pub struct IssueSeed {
    pub id: Uuid,
    pub project_id: Uuid,
    pub display_key: String,
    pub title: String,
    pub agent_eligible: bool,
    pub spec_complete: bool,
    pub rank: i64,
}

#[derive(Debug, Clone)]
pub struct IssueCreate {
    pub id: Uuid,
    pub display_key: String,
    pub title: String,
    pub body: String,
    pub status: String,
    pub agent_eligible: bool,
    pub spec_complete: bool,
    pub rank: i64,
    pub labels: Vec<String>,
    pub assignee_account_id: Option<Uuid>,
    pub parent_issue_id: Option<Uuid>,
}

impl IssueCreate {
    pub fn minimal(id: Uuid, display_key: &str, title: &str) -> Self {
        Self {
            id,
            display_key: display_key.to_owned(),
            title: title.to_owned(),
            body: String::new(),
            status: "todo".to_owned(),
            agent_eligible: false,
            spec_complete: false,
            rank: 0,
            labels: Vec::new(),
            assignee_account_id: None,
            parent_issue_id: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct IssueUpdate {
    pub expected_version: i64,
    pub title: Option<String>,
    pub status: Option<String>,
    pub importance: Option<String>,
    pub agent_eligible: Option<bool>,
    pub spec_complete: Option<bool>,
    pub rank: Option<i64>,
    pub labels: Option<Vec<String>>,
    pub assignee_account_id: Option<Uuid>,
    pub due_date: Option<Option<NaiveDate>>,
    pub snoozed_until: Option<Option<NaiveDate>>,
    pub workflow_alias: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WorkflowAliasRecord {
    pub project_id: Uuid,
    pub version: i64,
    pub label: String,
    pub canonical_status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct IssueTemplateRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub name: String,
    pub version: i64,
    pub snapshot: serde_json::Value,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct IssueSubscriptionRecord {
    pub id: Uuid,
    pub account_id: Uuid,
    pub project_id: Uuid,
    pub issue_id: Option<Uuid>,
    pub kind: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReportInput {
    pub action: ReportAction,
    pub comment: Option<String>,
    pub resolution_summary: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReportBatch {
    pub idempotency_key: String,
    pub operations: Vec<ReportOperation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReportOperation {
    Comment {
        body: String,
    },
    SetStatus {
        status: String,
    },
    Release,
    Complete {
        resolution_summary: String,
    },
    CreateDiscovered {
        display_key: String,
        title: String,
        body: String,
        rank: i64,
    },
    AddBlocker {
        blocker_issue_id: Uuid,
    },
    RequestSpec {
        reason: String,
    },
    MarkDuplicate {
        duplicate_of: Uuid,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReportBatchResult {
    pub issue_id: Uuid,
    pub created_issue_ids: Vec<Uuid>,
    pub applied_operations: usize,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct RecoveryChecklistRecord {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub old_lease_id: Uuid,
    pub old_session_id: Uuid,
    pub initiated_by: Uuid,
    pub reason: String,
    pub state: String,
    pub actions: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ApprovalOperation {
    SetRank {
        rank: i64,
    },
    ReopenForDispatch {
        checklist_id: Uuid,
    },
    CompleteWithSummary {
        checklist_id: Uuid,
        resolution_summary: String,
    },
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ApprovalRequestRecord {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub requested_by: Uuid,
    pub target_version: i64,
    pub proposed_operation: serde_json::Value,
    pub state: String,
    pub expires_at: DateTime<Utc>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct GlobalApprovalRequestRecord {
    pub project_id: Uuid,
    pub team_key: String,
    pub project_name: String,
    pub issue_title: String,
    pub id: Uuid,
    pub issue_id: Uuid,
    pub requested_by: Uuid,
    pub target_version: i64,
    pub proposed_operation: serde_json::Value,
    pub state: String,
    pub expires_at: DateTime<Utc>,
    pub decided_by: Option<Uuid>,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct QuarantinedAttemptRecord {
    pub id: Uuid,
    pub issue_id: Uuid,
    pub session_id: Uuid,
    pub role_id: Uuid,
    pub lease_id: Uuid,
    pub fencing_token: i64,
    pub request_id: Uuid,
    pub reason: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExternalIssueRecord {
    pub id: Uuid,
    pub issue_id: Option<Uuid>,
    pub provider: String,
    pub external_id: String,
    pub repository: String,
    pub external_number: i64,
    pub url: String,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub external_updated_at: Option<String>,
    pub payload: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AgentRoleRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub team_id: Uuid,
    pub display_name: String,
    pub owner_account_id: Option<Uuid>,
    pub capabilities: serde_json::Value,
    pub revoked_at: Option<DateTime<Utc>>,
    pub active_session_count: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AgentSessionRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub team_id: Uuid,
    pub agent_role_id: Uuid,
    pub state: String,
    pub max_lifetime_ends_at: DateTime<Utc>,
    pub heartbeat_at: Option<DateTime<Utc>>,
    pub last_action_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct LeaseCollaboratorRecord {
    pub lease_id: Uuid,
    pub session_id: Uuid,
    pub capability: String,
    pub grant_mode: String,
    pub granted_by: Option<Uuid>,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportAction {
    Release,
    Complete,
}
