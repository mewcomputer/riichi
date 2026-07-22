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
