use super::*;
use sha2::{Digest, Sha256};

impl Database {
    pub async fn github_project_integration(
        &self,
        project_id: Uuid,
    ) -> Result<Option<models::GithubProjectIntegrationRecord>, Error> {
        Ok(sqlx::query_as::<_, models::GithubProjectIntegrationRecord>(
            "SELECT project_id, repository, enabled, configured_by, created_at, updated_at
             FROM github_project_integrations WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn set_github_project_integration(
        &self,
        project_id: Uuid,
        account_id: Uuid,
        repository: &str,
        enabled: bool,
    ) -> Result<models::GithubProjectIntegrationRecord, Error> {
        Ok(sqlx::query_as::<_, models::GithubProjectIntegrationRecord>(
            "INSERT INTO github_project_integrations (project_id, repository, enabled, configured_by)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (project_id) DO UPDATE SET repository = EXCLUDED.repository,
                 enabled = EXCLUDED.enabled, configured_by = EXCLUDED.configured_by, updated_at = now()
             RETURNING project_id, repository, enabled, configured_by, created_at, updated_at",
        )
        .bind(project_id)
        .bind(repository)
        .bind(enabled)
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn upsert_github_pull_request_snapshot(
        &self,
        project_id: Uuid,
        issue_id: Option<Uuid>,
        repository: &str,
        pull_request_number: i64,
        url: &str,
        title: &str,
        state: &str,
        review_state: Option<&str>,
        ci_state: Option<&str>,
        external_updated_at: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<models::GithubPullRequestRecord, Error> {
        let id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO github_pull_request_snapshots
             (id, project_id, issue_id, repository, pull_request_number, title, url, state, review_state, ci_state, payload, external_updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
             ON CONFLICT (project_id, repository, pull_request_number)
             DO UPDATE SET issue_id = COALESCE(EXCLUDED.issue_id, github_pull_request_snapshots.issue_id),
                           title = EXCLUDED.title, url = EXCLUDED.url, state = EXCLUDED.state,
                           review_state = EXCLUDED.review_state, ci_state = EXCLUDED.ci_state,
                           payload = EXCLUDED.payload, external_updated_at = EXCLUDED.external_updated_at,
                           fetched_at = now()
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(issue_id)
        .bind(repository)
        .bind(pull_request_number)
        .bind(title)
        .bind(url)
        .bind(state)
        .bind(review_state)
        .bind(ci_state)
        .bind(payload)
        .bind(external_updated_at)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query_as::<_, models::GithubPullRequestRecord>(
            "SELECT id, project_id, issue_id, repository, pull_request_number, title, url, state,
                    review_state, ci_state, payload, external_updated_at, fetched_at
             FROM github_pull_request_snapshots WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn github_pull_requests(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<models::GithubPullRequestRecord>, Error> {
        Ok(sqlx::query_as::<_, models::GithubPullRequestRecord>(
            "SELECT id, project_id, issue_id, repository, pull_request_number, title, url, state,
                    review_state, ci_state, payload, external_updated_at, fetched_at
             FROM github_pull_request_snapshots WHERE project_id = $1
             ORDER BY fetched_at DESC, id DESC LIMIT $2",
        )
        .bind(project_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn link_github_pull_request(
        &self,
        project_id: Uuid,
        pull_request_id: Uuid,
        issue_id: Option<Uuid>,
    ) -> Result<models::GithubPullRequestRecord, Error> {
        if let Some(issue_id) = issue_id {
            let valid = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
            )
            .bind(issue_id)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
            if !valid {
                return Err(PersistenceError::IssueNotFound);
            }
        }
        sqlx::query(
            "UPDATE github_pull_request_snapshots SET issue_id = $3
             WHERE id = $1 AND project_id = $2",
        )
        .bind(pull_request_id)
        .bind(project_id)
        .bind(issue_id)
        .execute(&self.pool)
        .await?;
        sqlx::query_as::<_, models::GithubPullRequestRecord>(
            "SELECT id, project_id, issue_id, repository, pull_request_number, title, url, state,
                    review_state, ci_state, payload, external_updated_at, fetched_at
             FROM github_pull_request_snapshots WHERE id = $1 AND project_id = $2",
        )
        .bind(pull_request_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistenceError::IssueNotFound)
    }

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
