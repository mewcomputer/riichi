use super::*;
use crate::models::DocumentJobRecord;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentJobRetryOutcome {
    Scheduled,
    DeadLettered,
    AlreadyHandled,
}

impl Database {
    pub async fn enqueue_document_job(
        &self,
        id: Uuid,
        document_id: Option<Uuid>,
        job_type: &str,
        idempotency_key: &str,
        available_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO document_jobs
             (id, document_id, job_type, idempotency_key, available_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (job_type, idempotency_key) DO NOTHING",
        )
        .bind(id)
        .bind(document_id)
        .bind(job_type)
        .bind(idempotency_key)
        .bind(available_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn claim_next_document_job(&self) -> Result<Option<DocumentJobRecord>, Error> {
        Ok(sqlx::query_as::<_, DocumentJobRecord>(
            "WITH next_job AS (
                SELECT id
                FROM document_jobs
                WHERE completed_at IS NULL
                  AND dead_lettered_at IS NULL
                  AND available_at <= now()
                  AND (claimed_at IS NULL OR claimed_at < now() - interval '5 minutes')
                ORDER BY available_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
            )
            UPDATE document_jobs job
            SET claimed_at = now(), attempt_count = job.attempt_count + 1
            FROM next_job
            WHERE job.id = next_job.id
            RETURNING job.id, job.document_id, job.job_type, job.idempotency_key,
                      job.available_at, job.claimed_at, job.attempt_count,
                      job.completed_at, job.last_error, job.dead_lettered_at",
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn complete_document_job(&self, job_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "UPDATE document_jobs
             SET completed_at = now(), claimed_at = NULL, last_error = NULL
             WHERE id = $1 AND completed_at IS NULL AND dead_lettered_at IS NULL",
        )
        .bind(job_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn retry_document_job(
        &self,
        job_id: Uuid,
        error: &str,
        delay: Duration,
        max_attempts: i32,
    ) -> Result<DocumentJobRetryOutcome, Error> {
        let dead_lettered = sqlx::query_scalar::<_, bool>(
            "UPDATE document_jobs
             SET available_at = CASE
                     WHEN attempt_count >= $4 THEN available_at
                     ELSE now() + $2::interval
                 END,
                 claimed_at = NULL,
                 last_error = $3,
                 dead_lettered_at = CASE
                     WHEN attempt_count >= $4 THEN now()
                     ELSE dead_lettered_at
                 END
             WHERE id = $1 AND completed_at IS NULL AND dead_lettered_at IS NULL
             RETURNING attempt_count >= $4",
        )
        .bind(job_id)
        .bind(format!("{} seconds", delay.num_seconds().max(1)))
        .bind(error)
        .bind(max_attempts)
        .fetch_optional(&self.pool)
        .await?;
        Ok(match dead_lettered {
            Some(true) => DocumentJobRetryOutcome::DeadLettered,
            Some(false) => DocumentJobRetryOutcome::Scheduled,
            None => DocumentJobRetryOutcome::AlreadyHandled,
        })
    }

    pub async fn claim_expired_attachment_uploads(&self) -> Result<Vec<(Uuid, String)>, Error> {
        Ok(sqlx::query_as::<_, (Uuid, String)>(
            "WITH candidates AS (
                SELECT u.id, u.attachment_id, a.storage_key
                FROM attachment_uploads u
                JOIN attachments a ON a.id = u.attachment_id
                WHERE u.completed_at IS NULL
                  AND u.expires_at <= now()
                  AND a.state = 'pending'
                  AND (
                      u.cleanup_claimed_at IS NULL
                      OR u.cleanup_claimed_at < now() - interval '5 minutes'
                  )
                ORDER BY u.expires_at, u.id
                FOR UPDATE OF u SKIP LOCKED
                LIMIT 100
            )
            UPDATE attachment_uploads u
            SET cleanup_claimed_at = now()
            FROM candidates c
            WHERE u.id = c.id
            RETURNING c.attachment_id, c.storage_key",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn finalize_expired_attachment_upload(
        &self,
        attachment_id: Uuid,
    ) -> Result<bool, Error> {
        Ok(sqlx::query_scalar::<_, Uuid>(
            "DELETE FROM attachments a
             USING attachment_uploads u
             WHERE a.id = $1
               AND u.attachment_id = a.id
               AND u.completed_at IS NULL
               AND u.expires_at <= now()
               AND a.state = 'pending'
             RETURNING a.id",
        )
        .bind(attachment_id)
        .fetch_optional(&self.pool)
        .await?
        .is_some())
    }
}
