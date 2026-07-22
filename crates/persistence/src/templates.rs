use super::*;

impl Database {
    pub async fn get_issue_template(
        &self,
        project_id: Uuid,
        template_id: Uuid,
    ) -> Result<models::IssueTemplateRecord, Error> {
        sqlx::query_as::<_, models::IssueTemplateRecord>(
            "SELECT id, project_id, name, version, snapshot, created_by, created_at
             FROM issue_templates WHERE project_id = $1 AND id = $2",
        )
        .bind(project_id)
        .bind(template_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistenceError::IssueNotFound)
    }

    pub async fn list_issue_templates(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<models::IssueTemplateRecord>, Error> {
        Ok(sqlx::query_as::<_, models::IssueTemplateRecord>(
            "SELECT DISTINCT ON (name) id, project_id, name, version, snapshot, created_by, created_at
             FROM issue_templates
             WHERE project_id = $1
             ORDER BY name, version DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_issue_template(
        &self,
        project_id: Uuid,
        actor_id: Uuid,
        name: &str,
        snapshot: serde_json::Value,
    ) -> Result<models::IssueTemplateRecord, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("issue-template:{project_id}:{name}"))
            .execute(&mut *tx)
            .await?;
        let record = sqlx::query_as::<_, models::IssueTemplateRecord>(
            "INSERT INTO issue_templates (id, project_id, name, version, snapshot, created_by)
             VALUES ($1, $2, $3, COALESCE((SELECT max(version) + 1 FROM issue_templates WHERE project_id = $2 AND name = $3), 1), $4, $5)
             RETURNING id, project_id, name, version, snapshot, created_by, created_at",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(name)
        .bind(snapshot)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(record)
    }

    pub async fn record_template_instance(
        &self,
        issue_id: Uuid,
        template_id: Uuid,
        template_version: i64,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO issue_template_instances (issue_id, template_id, template_version)
             VALUES ($1, $2, $3)",
        )
        .bind(issue_id)
        .bind(template_id)
        .bind(template_version)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
