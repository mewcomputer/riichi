use super::*;

impl Database {
    pub async fn events_since(
        &self,
        project_id: Uuid,
        after: Option<i64>,
        limit: i64,
    ) -> Result<Vec<models::DeliveryEventRecord>, Error> {
        let limit = limit.clamp(1, 100);
        let messages = if let Some(after) = after {
            sqlx::query_as::<_, models::DeliveryEventRecord>(
                "SELECT event_seq, id, project_id, event_type AS message_type, payload, 0 AS attempt_count
                 FROM delivery_events
                 WHERE project_id = $1 AND event_seq > $2
                 ORDER BY event_seq
                 LIMIT $3",
            )
            .bind(project_id)
            .bind(after)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, models::DeliveryEventRecord>(
                "SELECT event_seq, id, project_id, event_type AS message_type, payload, 0 AS attempt_count
                 FROM delivery_events
                 WHERE project_id = $1
                 ORDER BY event_seq
                 LIMIT $2",
            )
            .bind(project_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?
        };
        Ok(messages)
    }

    pub async fn claim_next_outbox(
        &self,
        project_id: Option<Uuid>,
    ) -> Result<Option<models::OutboxMessage>, Error> {
        let message = sqlx::query_as::<_, models::OutboxMessage>(
            "WITH candidate AS (
                 SELECT id
                 FROM outbox_messages
                 WHERE delivered_at IS NULL
                   AND dead_lettered_at IS NULL
                   AND available_at <= now()
                   AND (claimed_at IS NULL OR claimed_at < now() - interval '1 minute')
                   AND ($1::uuid IS NULL OR project_id = $1)
                 ORDER BY available_at, id
                 FOR UPDATE SKIP LOCKED
                 LIMIT 1
             )
             UPDATE outbox_messages message
             SET claimed_at = now(), attempt_count = attempt_count + 1
             FROM candidate
             WHERE message.id = candidate.id
             RETURNING message.id, message.project_id, message.message_type,
                       message.payload, message.attempt_count",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(message)
    }

    pub async fn deliver_outbox_event(&self, message_id: Uuid) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let event = sqlx::query_as::<_, (Uuid, Uuid, i64)>(
            "INSERT INTO delivery_events (id, project_id, event_type, payload)
             SELECT id, project_id, message_type, payload
             FROM outbox_messages
             WHERE id = $1
             ON CONFLICT (id) DO UPDATE SET id = delivery_events.id
             RETURNING id, project_id, event_seq",
        )
        .bind(message_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("SELECT pg_notify('riichi_delivery_events', $1)")
            .bind(
                serde_json::json!({
                    "project_id": event.1,
                    "event_seq": event.2,
                })
                .to_string(),
            )
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE outbox_messages SET delivered_at = now(), claimed_at = NULL
             WHERE id = $1 AND delivered_at IS NULL",
        )
        .bind(message_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn retry_outbox(
        &self,
        message_id: Uuid,
        error: &str,
        delay: Duration,
        max_attempts: i32,
    ) -> Result<OutboxRetryOutcome, Error> {
        let max_attempts = max_attempts.max(1);
        let dead_lettered = sqlx::query_scalar::<_, bool>(
            "UPDATE outbox_messages
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
             WHERE id = $1
               AND delivered_at IS NULL
               AND dead_lettered_at IS NULL
             RETURNING attempt_count >= $4",
        )
        .bind(message_id)
        .bind(format!("{} seconds", delay.num_seconds().max(1)))
        .bind(error)
        .bind(max_attempts)
        .fetch_optional(&self.pool)
        .await?;
        let outcome = match dead_lettered {
            Some(true) => OutboxRetryOutcome::DeadLettered,
            Some(false) => OutboxRetryOutcome::Scheduled,
            None => OutboxRetryOutcome::AlreadyHandled,
        };
        Ok(outcome)
    }

    pub async fn redrive_outbox(
        &self,
        project_id: Uuid,
        message_id: Uuid,
        actor_id: Uuid,
    ) -> Result<bool, Error> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE outbox_messages
             SET available_at = now(), claimed_at = NULL, attempt_count = 0,
                 last_error = NULL, dead_lettered_at = NULL
             WHERE id = $1 AND project_id = $2
               AND delivered_at IS NULL AND dead_lettered_at IS NOT NULL",
        )
        .bind(message_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        sqlx::query(
            "INSERT INTO audit_records
             (id, project_id, actor_id, request_id, operation, target_type, target_id, change_summary)
             VALUES ($1, $2, $3, $4, 'outbox_redrive', 'outbox_message', $5, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(actor_id)
        .bind(current_request_id())
        .bind(message_id)
        .bind(serde_json::json!({"reason": "operator_redrive"}))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}
