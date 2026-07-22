use super::*;

#[allow(clippy::too_many_arguments)]
async fn revoke_lease(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    lease_id: Uuid,
    issue_id: Uuid,
    owner_session_id: Uuid,
    state: &str,
    event: &str,
    reason: &str,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE leases SET state = $2, release_reason = $3
         WHERE id = $1 AND state = 'active'",
    )
    .bind(lease_id)
    .bind(state)
    .bind(reason)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE issues SET status = 'todo', version = version + 1, updated_at = now()
         WHERE id = $1 AND project_id = $2 AND status = 'in_progress'",
    )
    .bind(issue_id)
    .bind(project_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "UPDATE issue_dispatch SET active_lease_id = NULL, fencing_token = fencing_token + 1,
         dispatch_version = dispatch_version + 1, updated_at = now()
         WHERE issue_id = $1 AND active_lease_id = $2",
    )
    .bind(issue_id)
    .bind(lease_id)
    .execute(&mut **tx)
    .await?;
    insert_outbox(
        tx,
        project_id,
        "lease_changed",
        serde_json::json!({
            "issue_id": issue_id,
            "lease_id": lease_id,
            "event": event,
            "owner_session_id": owner_session_id
        }),
    )
    .await?;
    insert_outbox(
        tx,
        project_id,
        "issue_changed",
        serde_json::json!({ "issue_id": issue_id, "event": "lease_expired" }),
    )
    .await?;
    Ok(())
}

impl Database {
    pub async fn agent_roster_for_team(
        &self,
        team_id: Uuid,
    ) -> Result<Vec<models::AgentRoleRecord>, Error> {
        sqlx::query_as::<_, models::AgentRoleRecord>(
            "SELECT r.id, r.project_id, r.team_id, r.display_name, r.owner_account_id, r.capabilities,
                    r.revoked_at,
                    count(s.id) FILTER (WHERE s.state = 'active') AS active_session_count
             FROM agent_roles r
             LEFT JOIN sessions s ON s.agent_role_id = r.id
             WHERE r.team_id = $1
             GROUP BY r.id
             ORDER BY r.display_name, r.id",
        )
        .bind(team_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn agent_sessions_for_team(
        &self,
        team_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<Vec<models::AgentSessionRecord>, Error> {
        sqlx::query_as::<_, models::AgentSessionRecord>(
            "SELECT s.id, s.project_id, s.team_id, s.agent_role_id, s.state, s.max_lifetime_ends_at,
                    s.heartbeat_at, s.last_action_at, s.revoked_at
             FROM sessions s
             WHERE s.team_id = $1 AND ($2::uuid IS NULL OR s.agent_role_id = $2)
             ORDER BY s.created_at DESC, s.id DESC",
        )
        .bind(team_id)
        .bind(role_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn revoke_agent_session(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let session_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM sessions WHERE id = $1 AND project_id = $2)",
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !session_exists {
            return Err(PersistenceError::AgentSessionNotFound);
        }
        sqlx::query(
            "UPDATE sessions SET state = 'revoked', revoked_at = now()
             WHERE id = $1 AND project_id = $2",
        )
        .bind(session_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        let leases = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT l.id, l.issue_id FROM leases l
             JOIN issues i ON i.id = l.issue_id
             WHERE l.owner_session_id = $1 AND i.project_id = $2 AND l.state = 'active'
             FOR UPDATE",
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await?;
        for (lease_id, issue_id) in leases {
            revoke_lease(
                &mut tx,
                project_id,
                lease_id,
                issue_id,
                session_id,
                "revoked",
                "lease_revoked",
                "session_revoked",
            )
            .await?;
        }
        sqlx::query(
            "INSERT INTO audit_records
             (id, project_id, actor_id, request_id, operation, target_type, target_id)
             VALUES ($1, $2, $3, $4, 'revoke_agent_session', 'session', $5)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(actor_id)
        .bind(current_request_id())
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn revoke_agent_role(
        &self,
        project_id: Uuid,
        role_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let role_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM agent_roles WHERE id = $1 AND project_id = $2)",
        )
        .bind(role_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !role_exists {
            return Err(PersistenceError::AgentRoleNotFound);
        }
        let sessions = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM sessions WHERE agent_role_id = $1 AND project_id = $2 AND state = 'active' FOR UPDATE",
        )
        .bind(role_id)
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await?;
        sqlx::query("UPDATE agent_roles SET revoked_at = now() WHERE id = $1 AND project_id = $2")
            .bind(role_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE sessions SET state = 'revoked', revoked_at = now()
             WHERE agent_role_id = $1 AND project_id = $2 AND state = 'active'",
        )
        .bind(role_id)
        .bind(project_id)
        .execute(&mut *tx)
        .await?;
        for session_id in sessions {
            let leases = sqlx::query_as::<_, (Uuid, Uuid)>(
                "SELECT l.id, l.issue_id FROM leases l
                 JOIN issues i ON i.id = l.issue_id
                 WHERE l.owner_session_id = $1 AND i.project_id = $2 AND l.state = 'active'
                 FOR UPDATE",
            )
            .bind(session_id)
            .bind(project_id)
            .fetch_all(&mut *tx)
            .await?;
            for (lease_id, issue_id) in leases {
                revoke_lease(
                    &mut tx,
                    project_id,
                    lease_id,
                    issue_id,
                    session_id,
                    "revoked",
                    "lease_revoked",
                    "role_revoked",
                )
                .await?;
            }
        }
        sqlx::query(
            "INSERT INTO audit_records
             (id, project_id, actor_id, request_id, operation, target_type, target_id)
             VALUES ($1, $2, $3, $4, 'revoke_agent_role', 'agent_role', $5)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(actor_id)
        .bind(current_request_id())
        .bind(role_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn agent_roster(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<models::AgentRoleRecord>, Error> {
        sqlx::query_as::<_, models::AgentRoleRecord>(
            "SELECT r.id, r.project_id, r.team_id, r.display_name, r.owner_account_id, r.capabilities,
                    r.revoked_at,
                    count(s.id) FILTER (WHERE s.state = 'active') AS active_session_count
             FROM agent_roles r
             LEFT JOIN sessions s ON s.agent_role_id = r.id
             WHERE r.project_id = $1
             GROUP BY r.id
             ORDER BY r.display_name, r.id",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn agent_sessions(
        &self,
        project_id: Uuid,
        role_id: Option<Uuid>,
    ) -> Result<Vec<models::AgentSessionRecord>, Error> {
        sqlx::query_as::<_, models::AgentSessionRecord>(
            "SELECT s.id, s.project_id, s.team_id, s.agent_role_id, s.state, s.max_lifetime_ends_at,
                    s.heartbeat_at, s.last_action_at, s.revoked_at
             FROM sessions s JOIN agent_roles r ON r.id = s.agent_role_id
             WHERE s.project_id = $1 AND ($2::uuid IS NULL OR s.agent_role_id = $2)
             ORDER BY s.created_at DESC, s.id DESC",
        )
        .bind(project_id)
        .bind(role_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn sweep_expired_leases(&self) -> Result<i64, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE sessions SET state = 'expired'
             WHERE state = 'active' AND max_lifetime_ends_at <= now()",
        )
        .execute(&mut *tx)
        .await?;
        let leases = sqlx::query_as::<_, (Uuid, Uuid, Uuid, Uuid, Uuid)>(
            "SELECT l.id, l.issue_id, i.project_id, l.owner_session_id, s.agent_role_id
             FROM leases l JOIN issues i ON i.id = l.issue_id
             JOIN sessions s ON s.id = l.owner_session_id
             WHERE l.state = 'active' AND (l.expires_at <= now() OR s.state <> 'active')
             FOR UPDATE",
        )
        .fetch_all(&mut *tx)
        .await?;
        for (lease_id, issue_id, project_id, session_id, role_id) in &leases {
            revoke_lease(
                &mut tx,
                *project_id,
                *lease_id,
                *issue_id,
                *session_id,
                "expired",
                "lease_expired",
                "lease_expired",
            )
            .await?;
            sqlx::query(
                "INSERT INTO notifications
                 (id, recipient_account_id, kind, project_id, issue_id, payload, dedupe_key)
                 SELECT gen_random_uuid(), sub.account_id, 'lease', $1, $2,
                        jsonb_build_object('subscription_kind', 'lease_expiry', 'lease_id', $3),
                        'subscription:lease_expiry:' || $3::text
                 FROM issue_subscriptions sub
                 WHERE sub.project_id = $1 AND sub.kind = 'lease_expiry'
                   AND (sub.issue_id IS NULL OR sub.issue_id = $2)
                 ON CONFLICT DO NOTHING",
            )
            .bind(project_id)
            .bind(issue_id)
            .bind(lease_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO audit_records
                 (id, project_id, actor_id, role_id, session_id, request_id, operation, target_type, target_id)
                 SELECT $1, i.project_id, $2, $3, $4, $5, 'expire_lease', 'issue', $6
                 FROM issues i WHERE i.id = $6",
            )
            .bind(Uuid::now_v7())
            .bind(session_id)
            .bind(role_id)
            .bind(session_id)
            .bind(current_request_id())
            .bind(issue_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(leases.len() as i64)
    }
}
