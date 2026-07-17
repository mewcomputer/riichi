use super::*;

const ALLOWED_CAPABILITIES: [&str; 10] = [
    "comment",
    "request_spec",
    "discover",
    "complete",
    "release",
    "edit_issue",
    "manage_relationships",
    "recovery_review",
    "doc.read",
    "doc.apply_edit",
];

pub(crate) fn operation_capability(operation: &models::ReportOperation) -> &'static str {
    match operation {
        models::ReportOperation::Comment { .. } => "comment",
        models::ReportOperation::RequestSpec { .. } => "request_spec",
        models::ReportOperation::CreateDiscovered { .. } => "discover",
        models::ReportOperation::Complete { .. } => "complete",
        models::ReportOperation::Release => "release",
        models::ReportOperation::SetStatus { .. } => "edit_issue",
        models::ReportOperation::AddBlocker { .. }
        | models::ReportOperation::MarkDuplicate { .. } => "manage_relationships",
    }
}

fn valid_capability(capability: &str) -> bool {
    ALLOWED_CAPABILITIES.contains(&capability)
}

fn valid_grant_mode(grant_mode: &str) -> bool {
    matches!(grant_mode, "auto" | "approval_required")
}

impl Database {
    #[allow(clippy::too_many_arguments)]
    pub async fn grant_lease_collaborator(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        lease_id: Uuid,
        session_id: Uuid,
        capability: &str,
        grant_mode: &str,
        granted_by: Uuid,
        expires_after: Option<Duration>,
    ) -> Result<(), Error> {
        if !valid_capability(capability) || !valid_grant_mode(grant_mode) {
            return Err(PersistenceError::InvalidCapability);
        }
        let mut tx = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1 FROM leases l
                 JOIN issues i ON i.id = l.issue_id
                 WHERE l.id = $1 AND l.issue_id = $2 AND i.project_id = $3
                   AND l.state = 'active'
             )",
        )
        .bind(lease_id)
        .bind(issue_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(PersistenceError::LeaseNotActive);
        }
        let session_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1 FROM sessions s
                 JOIN agent_roles r ON r.id = s.agent_role_id
                 WHERE s.id = $1 AND s.project_id = $2 AND s.state = 'active'
                   AND s.max_lifetime_ends_at > now() AND r.revoked_at IS NULL
             )",
        )
        .bind(session_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !session_exists {
            return Err(PersistenceError::AgentSessionNotFound);
        }
        sqlx::query(
            "INSERT INTO lease_collaborators
             (lease_id, session_id, capability, grant_mode, granted_by, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6::timestamptz)
             ON CONFLICT (lease_id, session_id, capability)
             DO UPDATE SET grant_mode = EXCLUDED.grant_mode,
                           granted_by = EXCLUDED.granted_by,
                           granted_at = now(),
                           expires_at = EXCLUDED.expires_at,
                           revoked_at = NULL",
        )
        .bind(lease_id)
        .bind(session_id)
        .bind(capability)
        .bind(grant_mode)
        .bind(granted_by)
        .bind(expires_after.map(|duration| Utc::now() + duration))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO audit_records
             (id, project_id, actor_id, request_id, operation, target_type, target_id, change_summary)
             VALUES ($1, $2, $3, $4, 'grant_lease_collaborator', 'issue', $5, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(granted_by)
        .bind(current_request_id())
        .bind(issue_id)
        .bind(serde_json::json!({
            "lease_id": lease_id,
            "session_id": session_id,
            "capability": capability,
            "grant_mode": grant_mode
        }))
        .execute(&mut *tx)
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({
                "issue_id": issue_id,
                "lease_id": lease_id,
                "event": "collaborator_granted",
                "session_id": session_id,
                "capability": capability
            }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn revoke_lease_collaborator(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        lease_id: Uuid,
        session_id: Uuid,
        capability: &str,
        revoked_by: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let result = sqlx::query(
            "UPDATE lease_collaborators c
             SET revoked_at = now()
             FROM leases l JOIN issues i ON i.id = l.issue_id
             WHERE c.lease_id = l.id AND l.id = $1 AND l.issue_id = $2
               AND i.project_id = $3 AND c.session_id = $4
               AND c.capability = $5 AND c.revoked_at IS NULL",
        )
        .bind(lease_id)
        .bind(issue_id)
        .bind(project_id)
        .bind(session_id)
        .bind(capability)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(PersistenceError::CollaboratorNotFound);
        }
        sqlx::query(
            "INSERT INTO audit_records
             (id, project_id, actor_id, request_id, operation, target_type, target_id, change_summary)
             VALUES ($1, $2, $3, $4, 'revoke_lease_collaborator', 'issue', $5, $6)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(revoked_by)
        .bind(current_request_id())
        .bind(issue_id)
        .bind(serde_json::json!({
            "lease_id": lease_id,
            "session_id": session_id,
            "capability": capability
        }))
        .execute(&mut *tx)
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({
                "issue_id": issue_id,
                "lease_id": lease_id,
                "event": "collaborator_revoked",
                "session_id": session_id,
                "capability": capability
            }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn lease_collaborators(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<Vec<models::LeaseCollaboratorRecord>, Error> {
        sqlx::query_as::<_, models::LeaseCollaboratorRecord>(
            "SELECT c.lease_id, c.session_id, c.capability, c.grant_mode,
                    c.granted_by, c.granted_at, c.expires_at, c.revoked_at
             FROM lease_collaborators c
             JOIN leases l ON l.id = c.lease_id
             JOIN issues i ON i.id = l.issue_id
             WHERE i.project_id = $1 AND l.issue_id = $2
             ORDER BY c.granted_at DESC, c.session_id, c.capability",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }
}
