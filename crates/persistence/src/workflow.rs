use super::*;

impl Database {
    pub async fn current_workflow_aliases(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<models::WorkflowAliasRecord>, Error> {
        Ok(sqlx::query_as::<_, models::WorkflowAliasRecord>(
            "SELECT a.project_id, a.version, a.label, a.canonical_status, v.created_at
             FROM workflow_aliases a
             JOIN workflow_alias_versions v ON v.project_id = a.project_id AND v.version = a.version
             WHERE a.project_id = $1
               AND a.version = (SELECT max(version) FROM workflow_alias_versions WHERE project_id = $1)
             ORDER BY lower(a.label), a.label",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn save_workflow_aliases(
        &self,
        project_id: Uuid,
        actor_id: Uuid,
        aliases: &[(String, String)],
    ) -> Result<Vec<models::WorkflowAliasRecord>, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(format!("workflow-alias:{project_id}"))
            .execute(&mut *tx)
            .await?;
        let version = sqlx::query_scalar::<_, i64>(
            "INSERT INTO workflow_alias_versions (project_id, version, created_by)
             VALUES ($1, COALESCE((SELECT max(version) + 1 FROM workflow_alias_versions WHERE project_id = $1), 1), $2)
             RETURNING version",
        )
        .bind(project_id)
        .bind(actor_id)
        .fetch_one(&mut *tx)
        .await?;
        for (label, canonical_status) in aliases {
            sqlx::query(
                "INSERT INTO workflow_aliases (project_id, version, label, canonical_status)
                 VALUES ($1, $2, $3, $4)",
            )
            .bind(project_id)
            .bind(version)
            .bind(label)
            .bind(canonical_status)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.current_workflow_aliases(project_id).await
    }
}
