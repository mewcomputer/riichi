use super::*;

impl Database {
    pub async fn list_issue_subscriptions(
        &self,
        account_id: Uuid,
        project_id: Uuid,
    ) -> Result<Vec<models::IssueSubscriptionRecord>, Error> {
        Ok(sqlx::query_as::<_, models::IssueSubscriptionRecord>(
            "SELECT id, account_id, project_id, issue_id, kind, created_at
             FROM issue_subscriptions WHERE account_id = $1 AND project_id = $2
             ORDER BY created_at DESC, id DESC",
        )
        .bind(account_id)
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn set_issue_subscription(
        &self,
        account_id: Uuid,
        project_id: Uuid,
        issue_id: Option<Uuid>,
        kind: &str,
        enabled: bool,
    ) -> Result<bool, Error> {
        if let Some(issue_id) = issue_id {
            let belongs = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
            )
            .bind(issue_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
            if !belongs {
                return Err(PersistenceError::IssueNotFound);
            }
        }
        if enabled {
            sqlx::query(
                "INSERT INTO issue_subscriptions (id, account_id, project_id, issue_id, kind)
                VALUES ($1, $2, $3, $4, $5) ON CONFLICT DO NOTHING",
            )
            .bind(Uuid::now_v7())
            .bind(account_id)
            .bind(project_id)
            .bind(issue_id)
            .bind(kind)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query(
                "DELETE FROM issue_subscriptions
                 WHERE account_id = $1 AND project_id = $2 AND issue_id IS NOT DISTINCT FROM $3 AND kind = $4",
            )
            .bind(account_id)
            .bind(project_id)
            .bind(issue_id)
            .bind(kind)
            .execute(&self.pool)
            .await?;
        }
        Ok(true)
    }
}
