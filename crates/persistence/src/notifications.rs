use super::*;

impl Database {
    pub async fn notifications_for_account(
        &self,
        account_id: Uuid,
        unread_only: bool,
        limit: i64,
    ) -> Result<Vec<models::NotificationRecord>, Error> {
        Ok(sqlx::query_as::<_, models::NotificationRecord>(
            "SELECT id, recipient_account_id, kind, project_id, issue_id, actor_id,
                    payload, created_at, read_at
             FROM notifications
             WHERE recipient_account_id = $1
               AND ($2 = false OR read_at IS NULL)
             ORDER BY created_at DESC, id DESC
             LIMIT $3",
        )
        .bind(account_id)
        .bind(unread_only)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn unread_notification_count(&self, account_id: Uuid) -> Result<i64, Error> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM notifications
             WHERE recipient_account_id = $1 AND read_at IS NULL",
        )
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn mark_notification_read(
        &self,
        account_id: Uuid,
        notification_id: Uuid,
    ) -> Result<bool, Error> {
        Ok(sqlx::query(
            "UPDATE notifications SET read_at = COALESCE(read_at, now())
             WHERE id = $1 AND recipient_account_id = $2",
        )
        .bind(notification_id)
        .bind(account_id)
        .execute(&self.pool)
        .await?
        .rows_affected()
            > 0)
    }
}
