use chrono::Duration;
use riichi_persistence::{
    Database, DocumentJob, DocumentJobRetryOutcome, Error as PersistenceError, Message,
    OutboxRetryOutcome,
};
use riichi_storage::{AttachmentStore, ObjectAttachmentStore};
use serde::Deserialize;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_DELIVERY_ATTEMPTS: i32 = 5;
pub const MAX_DOCUMENT_JOB_ATTEMPTS: i32 = 5;
const INITIAL_RETRY_DELAY_SECONDS: i64 = 5;
const MAX_RETRY_DELAY_SECONDS: i64 = 300;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryEvent {
    LeaseChanged {
        project_id: Uuid,
        issue_id: Uuid,
        lease_id: Uuid,
        event: String,
    },
    IssueChanged {
        project_id: Uuid,
        issue_id: Uuid,
        lease_id: Option<Uuid>,
        event: String,
    },
}

#[derive(Debug, Error)]
pub enum DeliveryError {
    #[error("outbox message has no project scope")]
    MissingProject,

    #[error("outbox message type is not supported: {0}")]
    UnsupportedMessageType(String),

    #[error("outbox message payload is invalid: {0}")]
    InvalidPayload(#[from] serde_json::Error),

    #[error("lease event payload is missing a lease id")]
    MissingLeaseId,
}

#[derive(Debug, Error)]
pub enum WorkerError {
    #[error("delivery failed: {0}")]
    Delivery(#[source] DeliveryError),

    #[error("delivery dead-lettered: {0}")]
    DeadLettered(#[source] DeliveryError),

    #[error("database error: {0}")]
    Database(#[from] PersistenceError),

    #[error("document job failed: {0}")]
    DocumentJob(#[source] DocumentJobError),

    #[error("document job dead-lettered: {0}")]
    DocumentJobDeadLettered(#[source] DocumentJobError),
}

#[derive(Debug, Error)]
pub enum DocumentJobError {
    #[error("document job type is not supported: {0}")]
    UnsupportedType(String),

    #[error("document job storage key is invalid")]
    InvalidStorageKey,

    #[error("document job storage cleanup failed: {0}")]
    Storage(#[source] std::io::Error),

    #[error("document job {0} is missing a document id")]
    MissingDocumentId(String),

    #[error("document provisioning failed: {0}")]
    Provision(String),

    #[error("document projection failed: {0}")]
    Projection(String),
}

#[derive(Debug, Deserialize)]
struct IssueEventPayload {
    issue_id: Uuid,
    lease_id: Option<Uuid>,
    event: String,
}

pub fn decode_message(message: &Message) -> Result<DeliveryEvent, DeliveryError> {
    let project_id = message.project_id.ok_or(DeliveryError::MissingProject)?;
    let payload: IssueEventPayload = serde_json::from_value(message.payload.clone())?;
    match message.message_type.as_str() {
        "lease_changed" => Ok(DeliveryEvent::LeaseChanged {
            project_id,
            issue_id: payload.issue_id,
            lease_id: payload.lease_id.ok_or(DeliveryError::MissingLeaseId)?,
            event: payload.event,
        }),
        "issue_changed" => Ok(DeliveryEvent::IssueChanged {
            project_id,
            issue_id: payload.issue_id,
            lease_id: payload.lease_id,
            event: payload.event,
        }),
        message_type => Err(DeliveryError::UnsupportedMessageType(
            message_type.to_owned(),
        )),
    }
}

fn retry_delay(attempt_count: i32) -> Duration {
    let exponent = attempt_count.saturating_sub(1).clamp(0, 6) as u32;
    Duration::seconds(
        (INITIAL_RETRY_DELAY_SECONDS.saturating_mul(1_i64 << exponent))
            .min(MAX_RETRY_DELAY_SECONDS),
    )
}

pub async fn process_message(
    database: &Database,
    message: &Message,
) -> Result<DeliveryEvent, WorkerError> {
    let event = match decode_message(message) {
        Ok(event) => event,
        Err(error) => {
            let outcome = database
                .retry_outbox(
                    message.id,
                    &error.to_string(),
                    retry_delay(message.attempt_count),
                    MAX_DELIVERY_ATTEMPTS,
                )
                .await?;
            return Err(match outcome {
                OutboxRetryOutcome::DeadLettered => WorkerError::DeadLettered(error),
                OutboxRetryOutcome::Scheduled | OutboxRetryOutcome::AlreadyHandled => {
                    WorkerError::Delivery(error)
                }
            });
        }
    };
    database.deliver_outbox_event(message.id).await?;
    Ok(event)
}

pub async fn process_document_job(
    database: &Database,
    job: &DocumentJob,
    attachment_store: &ObjectAttachmentStore,
) -> Result<(), WorkerError> {
    let result = match job.job_type.as_str() {
        "provision" => provision_document(database, job).await,
        "project" => project_document(database, job).await,
        "compact" => compact_document(database, job).await,
        "archive" => archive_document(database, job).await,
        "delete" => delete_document(database, job).await,
        "attachment_cleanup" => cleanup_attachments(database, attachment_store).await,
        job_type => Err(DocumentJobError::UnsupportedType(job_type.to_owned())),
    };
    match result {
        Ok(()) => {
            database.complete_document_job(job.id).await?;
            Ok(())
        }
        Err(error) => {
            let outcome = database
                .retry_document_job(
                    job.id,
                    &error.to_string(),
                    retry_delay(job.attempt_count),
                    MAX_DOCUMENT_JOB_ATTEMPTS,
                )
                .await?;
            match outcome {
                DocumentJobRetryOutcome::DeadLettered => {
                    if let Some(document_id) = job.document_id {
                        database.mark_document_failed(document_id).await?;
                    }
                    Err(WorkerError::DocumentJobDeadLettered(error))
                }
                DocumentJobRetryOutcome::Scheduled | DocumentJobRetryOutcome::AlreadyHandled => {
                    Err(WorkerError::DocumentJob(error))
                }
            }
        }
    }
}

async fn provision_document(
    database: &Database,
    job: &DocumentJob,
) -> Result<(), DocumentJobError> {
    let document_id = job
        .document_id
        .ok_or_else(|| DocumentJobError::MissingDocumentId(job.job_type.clone()))?;
    riichi_application::Application::new(database.clone())
        .provision_document(document_id)
        .await
        .map_err(|error| DocumentJobError::Provision(error.to_string()))
}

async fn project_document(database: &Database, job: &DocumentJob) -> Result<(), DocumentJobError> {
    let document_id = job
        .document_id
        .ok_or_else(|| DocumentJobError::MissingDocumentId(job.job_type.clone()))?;
    riichi_application::Application::new(database.clone())
        .project_document(document_id)
        .await
        .map_err(|error| DocumentJobError::Projection(error.to_string()))
}

async fn compact_document(database: &Database, job: &DocumentJob) -> Result<(), DocumentJobError> {
    let document_id = job
        .document_id
        .ok_or_else(|| DocumentJobError::MissingDocumentId(job.job_type.clone()))?;
    riichi_application::Application::new(database.clone())
        .compact_document(document_id)
        .await
        .map_err(|error| DocumentJobError::Projection(error.to_string()))
}

async fn archive_document(database: &Database, job: &DocumentJob) -> Result<(), DocumentJobError> {
    let document_id = job
        .document_id
        .ok_or_else(|| DocumentJobError::MissingDocumentId(job.job_type.clone()))?;
    database
        .archive_document_internal(document_id)
        .await
        .map_err(|error| DocumentJobError::Projection(error.to_string()))
}

async fn delete_document(database: &Database, job: &DocumentJob) -> Result<(), DocumentJobError> {
    let document_id = job
        .document_id
        .ok_or_else(|| DocumentJobError::MissingDocumentId(job.job_type.clone()))?;
    database
        .delete_document_internal(document_id)
        .await
        .map_err(|error| DocumentJobError::Projection(error.to_string()))
}

async fn cleanup_attachments(
    database: &Database,
    attachment_store: &ObjectAttachmentStore,
) -> Result<(), DocumentJobError> {
    for (attachment_id, storage_key) in database
        .claim_expired_attachment_uploads()
        .await
        .map_err(|error| DocumentJobError::Storage(std::io::Error::other(error)))?
    {
        match attachment_store.delete(&storage_key).await {
            Ok(()) => {}
            Err(error) if ObjectAttachmentStore::is_not_found(&error) => {}
            Err(error) => return Err(DocumentJobError::Storage(std::io::Error::other(error))),
        }
        database
            .finalize_expired_attachment_upload(attachment_id)
            .await
            .map_err(|error| DocumentJobError::Storage(std::io::Error::other(error)))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn message(message_type: &str, payload: serde_json::Value) -> Message {
        Message {
            id: Uuid::new_v4(),
            project_id: Some(Uuid::new_v4()),
            message_type: message_type.to_owned(),
            payload,
            attempt_count: 1,
        }
    }

    #[test]
    fn decodes_claimed_lease_events() {
        let project_id = Uuid::new_v4();
        let issue_id = Uuid::new_v4();
        let lease_id = Uuid::new_v4();
        let mut message = message(
            "lease_changed",
            json!({"issue_id": issue_id, "lease_id": lease_id, "event": "claimed"}),
        );
        message.project_id = Some(project_id);

        assert_eq!(
            decode_message(&message).unwrap(),
            DeliveryEvent::LeaseChanged {
                project_id,
                issue_id,
                lease_id,
                event: "claimed".to_owned(),
            }
        );
    }

    #[test]
    fn rejects_unknown_types_and_events_without_panicking() {
        let unknown = message(
            "github_delivery",
            json!({"issue_id": Uuid::new_v4(), "lease_id": Uuid::new_v4(), "event": "sent"}),
        );
        assert!(matches!(
            decode_message(&unknown),
            Err(DeliveryError::UnsupportedMessageType(message_type)) if message_type == "github_delivery"
        ));

        let unexpected = message(
            "lease_changed",
            json!({"issue_id": Uuid::new_v4(), "lease_id": Uuid::new_v4(), "event": "released"}),
        );
        assert!(decode_message(&unexpected).is_ok());
    }

    #[test]
    fn requires_project_scope_and_the_complete_payload_shape() {
        let mut missing_scope = message(
            "lease_changed",
            json!({"issue_id": Uuid::new_v4(), "lease_id": Uuid::new_v4(), "event": "claimed"}),
        );
        missing_scope.project_id = None;
        assert!(matches!(
            decode_message(&missing_scope),
            Err(DeliveryError::MissingProject)
        ));

        let malformed = message("lease_changed", json!({"event": "claimed"}));
        assert!(matches!(
            decode_message(&malformed),
            Err(DeliveryError::InvalidPayload(_))
        ));
    }
}
