use super::*;
use sha2::{Digest, Sha256};

impl Database {
    pub async fn create_cli_login_handoff(
        &self,
        token: &str,
        lifetime: Duration,
    ) -> Result<(), Error> {
        let token_hash = Sha256::digest(token.as_bytes());
        sqlx::query("DELETE FROM cli_login_handoffs WHERE expires_at <= now()")
            .execute(&self.pool)
            .await?;
        sqlx::query("INSERT INTO cli_login_handoffs (token_hash, expires_at) VALUES ($1, now() + $2::interval)")
            .bind(token_hash.as_slice())
            .bind(format!("{} seconds", lifetime.num_seconds().max(1)))
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn complete_cli_login_handoff(
        &self,
        token: &str,
        account_id: Uuid,
    ) -> Result<bool, Error> {
        let token_hash = Sha256::digest(token.as_bytes());
        let updated = sqlx::query("UPDATE cli_login_handoffs SET account_id = $2 WHERE token_hash = $1 AND expires_at > now() AND account_id IS NULL")
            .bind(token_hash.as_slice())
            .bind(account_id)
            .execute(&self.pool)
            .await?;
        Ok(updated.rows_affected() == 1)
    }

    pub async fn exchange_cli_login_handoff(&self, token: &str) -> Result<Option<Uuid>, Error> {
        let token_hash = Sha256::digest(token.as_bytes());
        let mut tx = self.pool.begin().await?;
        let account_id = sqlx::query_scalar::<_, Uuid>("SELECT account_id FROM cli_login_handoffs WHERE token_hash = $1 AND expires_at > now() AND exchanged_at IS NULL FOR UPDATE")
            .bind(token_hash.as_slice())
            .fetch_optional(&mut *tx)
            .await?;
        if account_id.is_some() {
            sqlx::query("UPDATE cli_login_handoffs SET exchanged_at = now() WHERE token_hash = $1")
                .bind(token_hash.as_slice())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(account_id)
    }
    pub async fn human_get_issue(
        &self,
        account_id: Uuid,
        issue_id: Uuid,
    ) -> Result<models::IssueRecord, Error> {
        let project_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT i.project_id
             FROM issues i
             WHERE i.id = $1
               AND (
                   EXISTS (
                       SELECT 1 FROM project_memberships pm
                       WHERE pm.project_id = i.project_id
                         AND pm.account_id = $2
                         AND pm.revoked_at IS NULL
                   )
                   OR EXISTS (
                       SELECT 1 FROM team_memberships tm
                       WHERE tm.team_id = i.team_id
                         AND tm.account_id = $2
                         AND tm.revoked_at IS NULL
                   )
               )
             ORDER BY i.project_id
             LIMIT 1",
        )
        .bind(issue_id)
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistenceError::IssueNotFound)?;
        self.get_issue(project_id, issue_id).await
    }

    pub async fn human_team_memberships(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<TeamMembership>, Error> {
        Ok(sqlx::query_as::<_, TeamMembership>(
            "SELECT tm.team_id, t.name AS team_name, t.key AS team_key, tm.role
             FROM team_memberships tm
             JOIN teams t ON t.id = tm.team_id
             WHERE tm.account_id = $1 AND tm.revoked_at IS NULL
             ORDER BY t.name, t.id",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn human_avatar(&self, account_id: Uuid) -> Result<Option<(Vec<u8>, String)>, Error> {
        Ok(sqlx::query_as::<_, (Vec<u8>, String)>(
            "SELECT avatar_bytes, avatar_content_type
             FROM human_accounts
             WHERE id = $1 AND avatar_bytes IS NOT NULL AND avatar_content_type IS NOT NULL",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn set_human_avatar(
        &self,
        account_id: Uuid,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<(), Error> {
        sqlx::query(
            "UPDATE human_accounts
             SET avatar_bytes = $2, avatar_content_type = $3, updated_at = now()
             WHERE id = $1",
        )
        .bind(account_id)
        .bind(bytes)
        .bind(content_type)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_human_avatar(&self, account_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "UPDATE human_accounts
             SET avatar_bytes = NULL, avatar_content_type = NULL, updated_at = now()
             WHERE id = $1",
        )
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_oidc_login_state(
        &self,
        state_hash: &[u8],
        issuer: &str,
        nonce: &str,
        pkce_verifier: &str,
        return_to: &str,
        lifetime: Duration,
    ) -> Result<(), Error> {
        sqlx::query("DELETE FROM oidc_login_states WHERE expires_at <= now()")
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO oidc_login_states
             (state_hash, issuer, nonce, pkce_verifier, return_to, expires_at)
             VALUES ($1, $2, $3, $4, $5, now() + $6::interval)",
        )
        .bind(state_hash)
        .bind(issuer)
        .bind(nonce)
        .bind(pkce_verifier)
        .bind(return_to)
        .bind(format!("{} seconds", lifetime.num_seconds().max(1)))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn consume_oidc_login_state(
        &self,
        state_hash: &[u8],
    ) -> Result<Option<OidcLoginState>, Error> {
        let state = sqlx::query_as::<_, OidcLoginState>(
            "DELETE FROM oidc_login_states
             WHERE state_hash = $1 AND expires_at > now()
             RETURNING issuer, nonce, pkce_verifier, return_to",
        )
        .bind(state_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(state)
    }

    pub async fn upsert_human_account(
        &self,
        issuer: &str,
        subject: &str,
        email: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<Uuid, Error> {
        let account_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO human_accounts (id, issuer, subject, email, display_name)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (issuer, subject) DO UPDATE
             SET email = COALESCE(EXCLUDED.email, human_accounts.email),
                 display_name = COALESCE(EXCLUDED.display_name, human_accounts.display_name),
                 updated_at = now()
             RETURNING id",
        )
        .bind(Uuid::now_v7())
        .bind(issuer)
        .bind(subject)
        .bind(email)
        .bind(display_name)
        .fetch_one(&self.pool)
        .await?;
        Ok(account_id)
    }

    pub async fn human_account(&self, account_id: Uuid) -> Result<Option<HumanAccount>, Error> {
        let account = sqlx::query_as::<_, HumanAccount>(
            "SELECT id, issuer, subject, email, display_name,
                    last_completed_nux_version, last_completed_nux_at
             FROM human_accounts WHERE id = $1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(account)
    }

    pub async fn complete_nux(
        &self,
        account_id: Uuid,
        version: &str,
    ) -> Result<Option<HumanAccount>, Error> {
        Ok(sqlx::query_as::<_, HumanAccount>(
            "UPDATE human_accounts
             SET last_completed_nux_version = $2, last_completed_nux_at = now(), updated_at = now()
             WHERE id = $1
             RETURNING id, issuer, subject, email, display_name,
                       last_completed_nux_version, last_completed_nux_at",
        )
        .bind(account_id)
        .bind(version)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn create_project_membership(
        &self,
        project_id: Uuid,
        account_id: Uuid,
        role: &str,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO project_memberships (project_id, account_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT (project_id, account_id) DO UPDATE
             SET role = EXCLUDED.role, revoked_at = NULL",
        )
        .bind(project_id)
        .bind(account_id)
        .bind(role)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn human_memberships(&self, account_id: Uuid) -> Result<Vec<HumanMembership>, Error> {
        let memberships = sqlx::query_as::<_, HumanMembership>(
            "WITH effective_memberships AS (
                 SELECT pm.project_id, pm.account_id, pm.role,
                        CASE pm.role
                            WHEN 'viewer' THEN 0
                            WHEN 'member' THEN 1
                            WHEN 'admin' THEN 2
                            WHEN 'owner' THEN 3
                            ELSE -1
                        END AS role_rank
                 FROM project_memberships pm
                 WHERE pm.account_id = $1 AND pm.revoked_at IS NULL
                 UNION ALL
                 SELECT pt.project_id, tm.account_id,
                        CASE
                            WHEN pt.role = 'admin' AND tm.role IN ('admin', 'owner') THEN 'admin'
                            WHEN pt.role = 'admin' AND tm.role = 'member' THEN 'member'
                            WHEN pt.role IN ('commenter', 'operator')
                                AND tm.role IN ('member', 'admin', 'owner') THEN 'member'
                            ELSE 'viewer'
                        END AS role,
                        CASE
                            WHEN pt.role = 'admin' AND tm.role IN ('admin', 'owner') THEN 2
                            WHEN pt.role = 'admin' AND tm.role = 'member' THEN 1
                            WHEN pt.role IN ('commenter', 'operator')
                                AND tm.role IN ('member', 'admin', 'owner') THEN 1
                            ELSE 0
                        END AS role_rank
                 FROM project_teams pt
                 JOIN team_memberships tm ON tm.team_id = pt.team_id
                 JOIN projects p ON p.id = pt.project_id
                 JOIN organization_memberships om
                     ON om.organization_id = p.organization_id
                    AND om.account_id = tm.account_id
                 WHERE tm.account_id = $1 AND tm.revoked_at IS NULL
                   AND om.revoked_at IS NULL
             ), ranked_memberships AS (
                 SELECT DISTINCT ON (project_id)
                        project_id, role
                 FROM effective_memberships
                 ORDER BY project_id, role_rank DESC
             )
             SELECT memberships.project_id, projects.name AS project_name, memberships.role
             FROM ranked_memberships memberships
             JOIN projects ON projects.id = memberships.project_id
             ORDER BY memberships.project_id",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(memberships)
    }

    pub async fn revoke_project_membership(
        &self,
        project_id: Uuid,
        account_id: Uuid,
    ) -> Result<(), Error> {
        sqlx::query(
            "UPDATE project_memberships SET revoked_at = COALESCE(revoked_at, now())
             WHERE project_id = $1 AND account_id = $2",
        )
        .bind(project_id)
        .bind(account_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn human_session_is_active(
        &self,
        session_id: Uuid,
        account_id: Uuid,
    ) -> Result<bool, Error> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM human_sessions
                 WHERE id = $1 AND account_id = $2
                   AND revoked_at IS NULL AND expires_at > now()
             )",
        )
        .bind(session_id)
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn human_can_access_project(
        &self,
        account_id: Uuid,
        project_id: Uuid,
    ) -> Result<bool, Error> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1
                 FROM project_memberships pm
                 WHERE pm.project_id = $1
                   AND pm.account_id = $2
                   AND pm.revoked_at IS NULL
             ) OR EXISTS (
                 SELECT 1
                 FROM project_teams pt
                 JOIN projects p ON p.id = pt.project_id
                 JOIN teams t ON t.id = pt.team_id
                 JOIN team_memberships tm ON tm.team_id = t.id
                 JOIN organization_memberships om
                   ON om.organization_id = p.organization_id
                  AND om.account_id = tm.account_id
                 WHERE pt.project_id = $1
                   AND tm.account_id = $2
                   AND tm.revoked_at IS NULL
                   AND om.revoked_at IS NULL
             )",
        )
        .bind(project_id)
        .bind(account_id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn create_human_project(
        &self,
        project_id: Uuid,
        name: &str,
        owner_account_id: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO projects (id, name, organization_id)
             VALUES ($1, $2, '00000000-0000-0000-0000-000000000001')",
        )
        .bind(project_id)
        .bind(name)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO project_memberships (project_id, account_id, role)
             VALUES ($1, $2, 'owner')",
        )
        .bind(project_id)
        .bind(owner_account_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, account_id, role)
             VALUES ('00000000-0000-0000-0000-000000000001', $1, 'owner')
             ON CONFLICT (organization_id, account_id) DO UPDATE
             SET role = CASE WHEN organization_memberships.role = 'owner' THEN 'owner' ELSE 'admin' END,
                 revoked_at = NULL",
        )
        .bind(owner_account_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO team_memberships (team_id, account_id, role)
             VALUES ('00000000-0000-0000-0000-000000000002', $1, 'owner')
             ON CONFLICT (team_id, account_id) DO UPDATE
             SET role = CASE WHEN team_memberships.role = 'owner' THEN 'owner' ELSE 'admin' END,
                 revoked_at = NULL",
        )
        .bind(owner_account_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO project_teams (project_id, team_id, role)
             VALUES ($1, '00000000-0000-0000-0000-000000000002', 'admin')
             ON CONFLICT (project_id, team_id) DO NOTHING",
        )
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_project_invite(
        &self,
        invite: ProjectInviteSeed,
    ) -> Result<DateTime<Utc>, Error> {
        let expires_at = sqlx::query_scalar::<_, DateTime<Utc>>(
            "INSERT INTO project_invites
             (id, project_id, invited_by, role, email_hint, token_hash, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, now() + $7::interval)
             RETURNING expires_at",
        )
        .bind(invite.id)
        .bind(invite.project_id)
        .bind(invite.invited_by)
        .bind(invite.role)
        .bind(invite.email_hint)
        .bind(invite.token_hash)
        .bind(format!("{} seconds", invite.lifetime.num_seconds().max(1)))
        .fetch_one(&self.pool)
        .await?;
        Ok(expires_at)
    }

    pub async fn accept_project_invite(
        &self,
        token_hash: &[u8],
        account_id: Uuid,
        account_email: Option<&str>,
    ) -> Result<Option<AcceptedInvite>, Error> {
        let mut tx = self.pool.begin().await?;
        let accepted = sqlx::query_as::<_, AcceptedInvite>(
            "UPDATE project_invites
             SET accepted_at = now(), accepted_by = $2
             WHERE token_hash = $1
               AND accepted_at IS NULL
               AND revoked_at IS NULL
               AND expires_at > now()
               AND (email_hint IS NULL OR lower(email_hint) = lower(COALESCE($3::text, '')))
             RETURNING project_id, role",
        )
        .bind(token_hash)
        .bind(account_id)
        .bind(account_email)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(accepted) = accepted else {
            tx.rollback().await?;
            return Ok(None);
        };

        sqlx::query(
            "INSERT INTO project_memberships (project_id, account_id, role)
             VALUES ($1, $2, $3)
             ON CONFLICT (project_id, account_id) DO UPDATE
             SET role = CASE
                 WHEN project_memberships.role = 'owner' THEN 'owner'
                 WHEN project_memberships.role = 'admin' OR EXCLUDED.role = 'admin' THEN 'admin'
                 WHEN project_memberships.role = 'member' OR EXCLUDED.role = 'member' THEN 'member'
                 ELSE 'viewer'
             END,
             revoked_at = NULL",
        )
        .bind(accepted.project_id)
        .bind(account_id)
        .bind(&accepted.role)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, account_id, role)
             SELECT organization_id, $2, CASE WHEN $3 = 'admin' THEN 'admin' ELSE 'member' END
             FROM projects WHERE id = $1
             ON CONFLICT (organization_id, account_id) DO UPDATE
             SET revoked_at = NULL",
        )
        .bind(accepted.project_id)
        .bind(account_id)
        .bind(&accepted.role)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO team_memberships (team_id, account_id, role)
             SELECT team_id, $2,
                    CASE
                        WHEN $3 = 'admin' THEN 'admin'
                        WHEN $3 = 'viewer' THEN 'viewer'
                        ELSE 'member'
                    END
             FROM project_teams WHERE project_id = $1
             ON CONFLICT (team_id, account_id) DO UPDATE
             SET revoked_at = NULL",
        )
        .bind(accepted.project_id)
        .bind(account_id)
        .bind(&accepted.role)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(accepted))
    }

    pub async fn revoke_project_invite(
        &self,
        project_id: Uuid,
        invite_id: Uuid,
    ) -> Result<(), Error> {
        sqlx::query(
            "UPDATE project_invites
             SET revoked_at = COALESCE(revoked_at, now())
             WHERE id = $1 AND project_id = $2
               AND accepted_at IS NULL AND revoked_at IS NULL",
        )
        .bind(invite_id)
        .bind(project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_human_session(
        &self,
        id: Uuid,
        account_id: Uuid,
        token_hash: &[u8],
        lifetime: Duration,
    ) -> Result<DateTime<Utc>, Error> {
        let expires_at = sqlx::query_scalar::<_, DateTime<Utc>>(
            "INSERT INTO human_sessions (id, account_id, token_hash, expires_at)
             VALUES ($1, $2, $3, now() + $4::interval)
             RETURNING expires_at",
        )
        .bind(id)
        .bind(account_id)
        .bind(token_hash)
        .bind(format!("{} seconds", lifetime.num_seconds().max(1)))
        .fetch_one(&self.pool)
        .await?;
        Ok(expires_at)
    }

    pub async fn active_human_session(
        &self,
        token_hash: &[u8],
    ) -> Result<Option<HumanSession>, Error> {
        let session = sqlx::query_as::<_, HumanSession>(
            "UPDATE human_sessions
             SET last_seen_at = now()
             WHERE token_hash = $1 AND revoked_at IS NULL AND expires_at > now()
             RETURNING id, account_id, expires_at",
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(session)
    }

    pub async fn revoke_human_session(&self, token_hash: &[u8]) -> Result<(), Error> {
        sqlx::query(
            "UPDATE human_sessions SET revoked_at = COALESCE(revoked_at, now())
             WHERE token_hash = $1",
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
