use super::*;
use sha2::{Digest, Sha256};

impl Database {
    pub async fn record_github_delivery(
        &self,
        delivery_id: &str,
        project_id: Option<Uuid>,
        event_type: &str,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<bool, Error> {
        if delivery_id.trim().is_empty() {
            return Err(PersistenceError::InvalidIssue(
                "GitHub delivery id is required".to_owned(),
            ));
        }
        let payload_bytes =
            serde_json::to_vec(&payload).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
        let payload_hash = Sha256::digest(payload_bytes).to_vec();
        let inserted = sqlx::query(
            "INSERT INTO webhook_deliveries
             (delivery_id, project_id, provider, event_type, action, payload_hash, payload, state)
             VALUES ($1, $2, 'github', $3, $4, $5, $6, 'accepted')
             ON CONFLICT (delivery_id) DO NOTHING",
        )
        .bind(delivery_id)
        .bind(project_id)
        .bind(event_type)
        .bind(action)
        .bind(payload_hash)
        .bind(payload)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(inserted == 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_github_snapshot(
        &self,
        project_id: Uuid,
        issue_id: Option<Uuid>,
        repository: &str,
        external_number: i64,
        url: &str,
        title: &str,
        body: Option<&str>,
        state: &str,
        external_updated_at: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<models::ExternalIssueRecord, Error> {
        if let Some(issue_id) = issue_id {
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
            )
            .bind(issue_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
            if !exists {
                return Err(PersistenceError::IssueNotFound);
            }
        }
        let external_id = format!("{repository}#{external_number}");
        let mut tx = self.pool.begin().await?;
        let link_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO external_links
             (id, project_id, issue_id, provider, external_id, repository, external_number, url)
             VALUES ($1, $2, $3, 'github', $4, $5, $6, $7)
             ON CONFLICT (project_id, provider, external_id)
             DO UPDATE SET issue_id = COALESCE(EXCLUDED.issue_id, external_links.issue_id),
                           repository = EXCLUDED.repository,
                           external_number = EXCLUDED.external_number,
                           url = EXCLUDED.url,
                           updated_at = now()
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(issue_id)
        .bind(&external_id)
        .bind(repository)
        .bind(external_number)
        .bind(url)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO external_issue_snapshots
             (external_link_id, title, body, state, external_updated_at, payload)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (external_link_id)
             DO UPDATE SET title = EXCLUDED.title, body = EXCLUDED.body, state = EXCLUDED.state,
                           external_updated_at = EXCLUDED.external_updated_at,
                           payload = EXCLUDED.payload, fetched_at = now()",
        )
        .bind(link_id)
        .bind(title)
        .bind(body)
        .bind(state)
        .bind(external_updated_at)
        .bind(payload)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        sqlx::query_as::<_, models::ExternalIssueRecord>(
            "SELECT l.id, l.issue_id, l.provider, l.external_id, l.repository,
                    l.external_number, l.url, s.title, s.body, s.state,
                    s.external_updated_at, s.payload, l.updated_at
             FROM external_links l
             JOIN external_issue_snapshots s ON s.external_link_id = l.id
             WHERE l.id = $1 AND l.project_id = $2",
        )
        .bind(link_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }
}
