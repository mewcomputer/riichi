use super::*;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct NavigationRow {
    pub organization_id: Uuid,
    pub organization_name: String,
    pub organization_role: String,
    pub organization_has_logo: bool,
    pub team_id: Uuid,
    pub team_name: String,
    pub team_key: String,
    pub team_emoji: Option<String>,
    pub project_id: Uuid,
    pub project_name: String,
    pub project_icon: Option<String>,
    pub project_role: String,
}

impl Database {
    pub async fn organization_membership_role(
        &self,
        account_id: Uuid,
        organization_id: Uuid,
    ) -> Result<Option<String>, Error> {
        Ok(sqlx::query_scalar(
            "SELECT role FROM organization_memberships
             WHERE account_id = $1 AND organization_id = $2 AND revoked_at IS NULL",
        )
        .bind(account_id)
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn organization_logo(
        &self,
        organization_id: Uuid,
    ) -> Result<Option<(Vec<u8>, String)>, Error> {
        Ok(sqlx::query_as::<_, (Vec<u8>, String)>(
            "SELECT logo_bytes, logo_content_type
             FROM organizations
             WHERE id = $1 AND logo_bytes IS NOT NULL AND logo_content_type IS NOT NULL",
        )
        .bind(organization_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn set_organization_logo(
        &self,
        organization_id: Uuid,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), Error> {
        sqlx::query(
            "UPDATE organizations
             SET logo_bytes = $2, logo_content_type = $3
             WHERE id = $1",
        )
        .bind(organization_id)
        .bind(bytes)
        .bind(content_type)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_organization_logo(&self, organization_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "UPDATE organizations
             SET logo_bytes = NULL, logo_content_type = NULL
             WHERE id = $1",
        )
        .bind(organization_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_team_emoji(&self, team_id: Uuid, emoji: Option<&str>) -> Result<(), Error> {
        let normalized = emoji.map(str::trim).filter(|value| !value.is_empty());
        if normalized
            .is_some_and(|value| value.chars().count() > 100 || value.contains(['\n', '\r']))
        {
            return Err(Error::InvalidIssue(
                "team mark must be at most 100 characters".to_owned(),
            ));
        }
        let updated = sqlx::query("UPDATE teams SET emoji = $2 WHERE id = $1")
            .bind(team_id)
            .bind(normalized)
            .execute(&self.pool)
            .await?;
        if updated.rows_affected() == 0 {
            return Err(Error::InvalidIssue("team was not found".to_owned()));
        }
        Ok(())
    }

    pub async fn human_project_role(
        &self,
        account_id: Uuid,
        project_id: Uuid,
    ) -> Result<Option<String>, Error> {
        Ok(sqlx::query_scalar(
            "SELECT role FROM project_memberships
             WHERE account_id = $1 AND project_id = $2 AND revoked_at IS NULL",
        )
        .bind(account_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn update_project_icon(
        &self,
        project_id: Uuid,
        icon: Option<&str>,
    ) -> Result<(), Error> {
        let normalized = icon.map(str::trim).filter(|value| !value.is_empty());
        if normalized
            .is_some_and(|value| value.chars().count() > 100 || value.contains(['\n', '\r']))
        {
            return Err(Error::InvalidIssue(
                "project icon must be at most 100 characters".to_owned(),
            ));
        }
        let updated = sqlx::query("UPDATE projects SET icon = $2 WHERE id = $1")
            .bind(project_id)
            .bind(normalized)
            .execute(&self.pool)
            .await?;
        if updated.rows_affected() == 0 {
            return Err(Error::InvalidIssue("project was not found".to_owned()));
        }
        Ok(())
    }

    pub async fn delete_project(
        &self,
        account_id: Uuid,
        project_id: Uuid,
        team_name: &str,
        project_name: &str,
    ) -> Result<bool, Error> {
        let mut tx = self.pool.begin().await?;
        let confirmed = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1
                 FROM projects p
                 JOIN project_teams pt ON pt.project_id = p.id
                 JOIN teams t ON t.id = pt.team_id
                 WHERE p.id = $1
                   AND p.name = $2
                   AND t.name = $3
                   AND (
                       EXISTS (
                           SELECT 1
                           FROM project_memberships pm
                           WHERE pm.project_id = p.id
                             AND pm.account_id = $4
                             AND pm.role IN ('owner', 'admin')
                             AND pm.revoked_at IS NULL
                       )
                       OR EXISTS (
                           SELECT 1
                           FROM team_memberships tm
                           JOIN organization_memberships om
                             ON om.organization_id = t.organization_id
                            AND om.account_id = tm.account_id
                           WHERE tm.team_id = t.id
                             AND tm.account_id = $4
                             AND tm.role IN ('owner', 'admin')
                             AND tm.revoked_at IS NULL
                             AND om.revoked_at IS NULL
                       )
                   )
             )",
        )
        .bind(project_id)
        .bind(project_name)
        .bind(team_name)
        .bind(account_id)
        .fetch_one(&mut *tx)
        .await?;
        if !confirmed {
            return Ok(false);
        }

        sqlx::query(
            "CREATE TEMP TABLE project_delete_issues ON COMMIT DROP AS
             SELECT id FROM issues WHERE project_id = $1",
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;

        for statement in [
            "DELETE FROM onboarding_samples WHERE project_id = $1",
            "DELETE FROM onboarding_sample_claims WHERE project_id = $1",
            "DELETE FROM issue_subscriptions WHERE project_id = $1",
            "DELETE FROM issue_template_instances WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM issue_metadata_sync WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM issue_activity_sync WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM human_issue_sync WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM approval_sync WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM quarantined_attempts WHERE project_id = $1",
            "DELETE FROM recovery_checklists WHERE project_id = $1",
            "DELETE FROM approval_requests WHERE project_id = $1",
            "DELETE FROM notifications WHERE project_id = $1 OR issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM external_links WHERE project_id = $1",
            "DELETE FROM github_pull_request_snapshots WHERE project_id = $1",
            "DELETE FROM issue_labels WHERE project_id = $1",
            "DELETE FROM issue_projects WHERE project_id = $1",
            "DELETE FROM dispatch_holds WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM comments WHERE project_id = $1",
            "DELETE FROM issue_edges WHERE project_id = $1",
            "DELETE FROM lease_collaborators WHERE lease_id IN (SELECT id FROM leases WHERE issue_id IN (SELECT id FROM project_delete_issues))",
            "DELETE FROM leases WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM issue_dispatch WHERE issue_id IN (SELECT id FROM project_delete_issues)",
            "DELETE FROM issues WHERE project_id = $1",
            "DELETE FROM human_agent_sync WHERE agent_role_id IN (SELECT id FROM agent_roles WHERE project_id = $1)",
            "DELETE FROM sessions WHERE project_id = $1",
            "DELETE FROM agent_roles WHERE project_id = $1",
            "DELETE FROM delivery_events WHERE project_id = $1",
            "DELETE FROM webhook_deliveries WHERE project_id = $1",
            "DELETE FROM outbox_messages WHERE project_id = $1",
            "DELETE FROM audit_records WHERE project_id = $1",
            "DELETE FROM idempotency_records WHERE project_id = $1",
            "DELETE FROM project_invites WHERE project_id = $1",
            "DELETE FROM project_memberships WHERE project_id = $1",
            "DELETE FROM project_teams WHERE project_id = $1",
            "DELETE FROM documents WHERE owner_project_id = $1",
            "DELETE FROM github_project_integrations WHERE project_id = $1",
            "DELETE FROM workflow_aliases WHERE project_id = $1",
            "DELETE FROM workflow_alias_versions WHERE project_id = $1",
            "DELETE FROM issue_templates WHERE project_id = $1",
        ] {
            sqlx::query(statement)
                .bind(project_id)
                .execute(&mut *tx)
                .await?;
        }

        let deleted = sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(project_id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
        tx.commit().await?;
        Ok(deleted)
    }

    pub async fn project_belongs_to_team(
        &self,
        project_id: Uuid,
        team_id: Uuid,
    ) -> Result<bool, Error> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM project_teams
                 WHERE project_id = $1 AND team_id = $2
             )",
        )
        .bind(project_id)
        .bind(team_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn team_organization_id(&self, team_id: Uuid) -> Result<Option<Uuid>, Error> {
        Ok(
            sqlx::query_scalar("SELECT organization_id FROM teams WHERE id = $1")
                .bind(team_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn project_organization_id(&self, project_id: Uuid) -> Result<Option<Uuid>, Error> {
        Ok(
            sqlx::query_scalar("SELECT organization_id FROM projects WHERE id = $1")
                .bind(project_id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn human_navigation(&self, account_id: Uuid) -> Result<Vec<NavigationRow>, Error> {
        Ok(sqlx::query_as::<_, NavigationRow>(
            "SELECT o.id AS organization_id, o.name AS organization_name,
                    om.role AS organization_role,
                    (o.logo_bytes IS NOT NULL AND o.logo_content_type IS NOT NULL) AS organization_has_logo,
                    t.id AS team_id, t.name AS team_name, t.key AS team_key, t.emoji AS team_emoji,
                    w.id AS project_id, w.name AS project_name, w.icon AS project_icon,
                    CASE
                        WHEN wt.role = 'admin' AND tm.role IN ('admin', 'owner') THEN 'admin'
                        WHEN wt.role = 'admin' AND tm.role = 'member' THEN 'member'
                        WHEN wt.role IN ('commenter', 'operator')
                            AND tm.role IN ('member', 'admin', 'owner') THEN 'member'
                        ELSE 'viewer'
                    END AS project_role
             FROM organization_memberships om
             JOIN organizations o ON o.id = om.organization_id
             JOIN team_memberships tm ON tm.account_id = om.account_id
                AND tm.revoked_at IS NULL
             JOIN teams t ON t.id = tm.team_id AND t.organization_id = o.id
             JOIN project_teams wt ON wt.team_id = t.id
             JOIN projects w ON w.id = wt.project_id AND w.organization_id = o.id
             WHERE om.account_id = $1 AND om.revoked_at IS NULL
             ORDER BY o.name, t.name, w.name",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }
}
