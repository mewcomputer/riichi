use thiserror::Error;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("migration error: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),

    #[error("session is not active")]
    SessionNotActive,

    #[error("issue was not found")]
    IssueNotFound,

    #[error("issue is not eligible for dispatch")]
    IssueNotEligible,

    #[error("issue is already claimed")]
    Contended,

    #[error("lease was not found")]
    LeaseNotFound,

    #[error("lease is no longer active")]
    LeaseNotActive,

    #[error("lease fencing token is stale")]
    StaleLease,

    #[error("completion requires a non-empty resolution summary")]
    ResolutionSummaryRequired,

    #[error("idempotency key was reused with a different request")]
    IdempotencyConflict,

    #[error("claim requires a non-empty idempotency key")]
    IdempotencyKeyRequired,

    #[error("agent session requires a non-empty credential")]
    AgentTokenRequired,

    #[error("issue data is invalid: {0}")]
    InvalidIssue(String),

    #[error("issue was changed by another actor")]
    VersionConflict,

    #[error("the issue relationship is invalid")]
    InvalidEdge,

    #[error("the issue relationship would create a cycle")]
    EdgeCycle,

    #[error("issue edge was not found")]
    EdgeNotFound,

    #[error("hold was not found or is already released")]
    HoldNotFound,

    #[error("context resource was not found or is unavailable")]
    ContextResourceNotFound,

    #[error("the document projection is still being updated")]
    DocumentProjectionPending,

    #[error("the requested document frontier is no longer available")]
    DocumentFrontierUnavailable,

    #[error("the issue has no active lease available for takeover")]
    TakeoverNotAvailable,

    #[error("recovery checklist was not found or is already closed")]
    RecoveryNotFound,

    #[error("approval request was not found or is already decided")]
    ApprovalNotFound,

    #[error("approval request has expired")]
    ApprovalExpired,

    #[error("approval request target has changed")]
    ApprovalSuperseded,

    #[error("agent session was not found")]
    AgentSessionNotFound,

    #[error("agent role was not found")]
    AgentRoleNotFound,

    #[error("the agent session lacks the capability for this operation")]
    CapabilityDenied,

    #[error("the collaborator capability or grant mode is invalid")]
    InvalidCapability,

    #[error("the collaborator grant was not found")]
    CollaboratorNotFound,

    #[error("document was not found")]
    DocumentNotFound,

    #[error("document access was denied")]
    DocumentAccessDenied,

    #[error("document revision is stale")]
    DocumentVersionConflict,

    #[error("the Loro document frontier is stale")]
    LoroFrontierConflict,

    #[error("document data is invalid: {0}")]
    InvalidDocument(String),

    #[error("attachment upload was not found or has expired")]
    AttachmentUploadNotFound,

    #[error("attachment checksum or size did not match")]
    AttachmentVerificationFailed,
}
