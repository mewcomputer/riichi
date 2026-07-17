use super::*;

fn require_reason(reason: &str) -> Result<(), Error> {
    if reason.trim().is_empty() || reason.chars().count() > 20_000 {
        return Err(PersistenceError::InvalidIssue(
            "a recovery reason is required and must be at most 20000 characters".to_owned(),
        ));
    }
    Ok(())
}

async fn human_audit(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor_id: Uuid,
    operation: &str,
    target_id: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO audit_records
         (id, project_id, actor_id, request_id, operation, target_type, target_id)
         VALUES ($1, $2, $3, $4, $5, 'issue', $6)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(actor_id)
    .bind(current_request_id())
    .bind(operation)
    .bind(target_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl Database {
    pub async fn human_pending_approvals(
        &self,
        account_id: Uuid,
    ) -> Result<Vec<models::GlobalApprovalRequestRecord>, Error> {
        Ok(sqlx::query_as::<_, models::GlobalApprovalRequestRecord>(
            "SELECT a.project_id, t.key AS team_key, p.name AS project_name, i.title AS issue_title,
                    a.id, a.issue_id, a.requested_by, a.target_version,
                    a.proposed_operation, a.state, a.expires_at, a.decided_by,
                    a.decided_at, a.created_at
             FROM approval_requests a
             JOIN projects p ON p.id = a.project_id
             JOIN issues i ON i.id = a.issue_id
             JOIN teams t ON t.id = i.team_id
             JOIN project_memberships pm ON pm.project_id = a.project_id
                AND pm.account_id = $1
                AND pm.revoked_at IS NULL
                AND pm.role IN ('owner', 'admin')
             WHERE a.state = 'pending'
             ORDER BY a.created_at DESC, a.id DESC",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }
    pub async fn takeover_issue(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        actor_id: Uuid,
        reason: &str,
    ) -> Result<models::RecoveryChecklistRecord, Error> {
        require_reason(reason)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM issues WHERE id = $1 AND project_id = $2 FOR UPDATE")
            .bind(issue_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(PersistenceError::IssueNotFound)?;
        let dispatch = sqlx::query_as::<_, DispatchRow>(
            "SELECT issue_id, active_lease_id, fencing_token FROM issue_dispatch
             WHERE issue_id = $1 FOR UPDATE",
        )
        .bind(issue_id)
        .fetch_one(&mut *tx)
        .await?;
        let lease_id = dispatch
            .active_lease_id
            .ok_or(PersistenceError::TakeoverNotAvailable)?;
        let lease = sqlx::query_as::<_, LeaseRow>(
            "SELECT id, issue_id, owner_session_id, fencing_token, state, expires_at
             FROM leases WHERE id = $1 FOR UPDATE",
        )
        .bind(lease_id)
        .fetch_one(&mut *tx)
        .await?;
        if lease.state != "active" {
            return Err(PersistenceError::TakeoverNotAvailable);
        }
        sqlx::query(
            "UPDATE leases SET state = 'revoked', release_reason = 'human_takeover' WHERE id = $1",
        )
        .bind(lease_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE issues SET status = 'in_progress', version = version + 1, updated_at = now()
             WHERE id = $1",
        )
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE issue_dispatch SET active_lease_id = NULL, fencing_token = fencing_token + 1,
             dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1",
        )
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
        let checklist_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO recovery_checklists
             (id, project_id, issue_id, old_lease_id, old_session_id, initiated_by, reason, state)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'open')",
        )
        .bind(checklist_id)
        .bind(project_id)
        .bind(issue_id)
        .bind(lease_id)
        .bind(lease.owner_session_id)
        .bind(actor_id)
        .bind(reason.trim())
        .execute(&mut *tx)
        .await?;
        human_audit(&mut tx, project_id, actor_id, "takeover_issue", issue_id).await?;
        insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({
                "issue_id": issue_id,
                "lease_id": lease_id,
                "event": "superseded"
            }),
        )
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": issue_id, "event": "takeover" }),
        )
        .await?;
        tx.commit().await?;
        sqlx::query_as::<_, models::RecoveryChecklistRecord>(
            "SELECT id, issue_id, old_lease_id, old_session_id, initiated_by, reason, state,
                    actions, created_at, completed_at
             FROM recovery_checklists WHERE id = $1",
        )
        .bind(checklist_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn complete_recovery(
        &self,
        project_id: Uuid,
        checklist_id: Uuid,
        actor_id: Uuid,
        expected_version: i64,
        action: &str,
        resolution_summary: Option<&str>,
    ) -> Result<models::IssueRecord, Error> {
        if !matches!(action, "release" | "complete") {
            return Err(PersistenceError::InvalidIssue(
                "recovery action must be release or complete".to_owned(),
            ));
        }
        if action == "complete"
            && resolution_summary
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
        {
            return Err(PersistenceError::ResolutionSummaryRequired);
        }
        let mut tx = self.pool.begin().await?;
        let issue_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT issue_id FROM recovery_checklists
             WHERE id = $1 AND project_id = $2 AND state = 'open' FOR UPDATE",
        )
        .bind(checklist_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::RecoveryNotFound)?;
        let version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM issues WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if version != expected_version {
            return Err(PersistenceError::VersionConflict);
        }
        let status = if action == "complete" { "done" } else { "todo" };
        sqlx::query(
            "UPDATE issues SET status = $1, version = version + 1,
             completed_at = CASE WHEN $1 = 'done' THEN now() ELSE NULL END,
             updated_at = now() WHERE id = $2",
        )
        .bind(status)
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
        if let Some(summary) = resolution_summary.filter(|summary| !summary.trim().is_empty()) {
            sqlx::query(
                "INSERT INTO comments
                 (id, project_id, issue_id, author_id, body) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(Uuid::now_v7())
            .bind(project_id)
            .bind(issue_id)
            .bind(actor_id)
            .bind(summary.trim())
            .execute(&mut *tx)
            .await?;
        }
        sqlx::query(
            "UPDATE recovery_checklists SET state = 'completed', completed_at = now(),
             actions = actions || jsonb_build_array(jsonb_build_object('actor_id', $2, 'action', $3))
             WHERE id = $1",
        )
        .bind(checklist_id)
        .bind(actor_id)
        .bind(action)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE issue_dispatch SET dispatch_version = dispatch_version + 1, updated_at = now()
             WHERE issue_id = $1",
        )
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
        human_audit(&mut tx, project_id, actor_id, "complete_recovery", issue_id).await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": issue_id, "event": "recovered" }),
        )
        .await?;
        tx.commit().await?;
        self.get_issue(project_id, issue_id).await
    }

    pub async fn create_approval_request(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        requested_by: Uuid,
        target_version: i64,
        proposed_operation: serde_json::Value,
        lifetime: Duration,
    ) -> Result<models::ApprovalRequestRecord, Error> {
        let lifetime = lifetime.clamp(Duration::minutes(1), Duration::days(7));
        let mut tx = self.pool.begin().await?;
        let current_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM issues WHERE id = $1 AND project_id = $2",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::IssueNotFound)?;
        if current_version != target_version {
            return Err(PersistenceError::VersionConflict);
        }
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO approval_requests
             (id, project_id, issue_id, requested_by, target_version, proposed_operation,
              state, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, 'pending', $7)",
        )
        .bind(id)
        .bind(project_id)
        .bind(issue_id)
        .bind(requested_by)
        .bind(target_version)
        .bind(proposed_operation)
        .bind(Utc::now() + lifetime)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO notifications
             (id, recipient_account_id, kind, project_id, issue_id, actor_id, payload, dedupe_key)
             SELECT gen_random_uuid(), pm.account_id, 'approval', $1, $2, $3,
                    jsonb_build_object('approval_id', $4, 'target_version', $5),
                    'approval:' || $4::text
             FROM project_memberships pm
             WHERE pm.project_id = $1
               AND pm.revoked_at IS NULL
               AND pm.role IN ('owner', 'admin')
               AND pm.account_id <> $3
             ON CONFLICT (recipient_account_id, dedupe_key) DO NOTHING",
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(requested_by)
        .bind(id)
        .bind(target_version)
        .execute(&mut *tx)
        .await?;
        human_audit(
            &mut tx,
            project_id,
            requested_by,
            "create_approval_request",
            issue_id,
        )
        .await?;
        tx.commit().await?;
        self.get_approval_request(project_id, id).await
    }

    pub async fn decide_approval_request(
        &self,
        project_id: Uuid,
        approval_id: Uuid,
        actor_id: Uuid,
        approve: bool,
    ) -> Result<models::ApprovalRequestRecord, Error> {
        let mut tx = self.pool.begin().await?;
        let request = sqlx::query_as::<_, (Uuid, i64, String, DateTime<Utc>)>(
            "SELECT issue_id, target_version, state, expires_at FROM approval_requests
             WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(approval_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::ApprovalNotFound)?;
        if request.2 != "pending" {
            return Err(PersistenceError::ApprovalNotFound);
        }
        if request.3 <= Utc::now() {
            sqlx::query("UPDATE approval_requests SET state = 'expired' WHERE id = $1")
                .bind(approval_id)
                .execute(&mut *tx)
                .await?;
            return Err(PersistenceError::ApprovalExpired);
        }
        let current_version = sqlx::query_scalar::<_, i64>(
            "SELECT version FROM issues WHERE id = $1 AND project_id = $2",
        )
        .bind(request.0)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if current_version != request.1 {
            sqlx::query("UPDATE approval_requests SET state = 'superseded' WHERE id = $1")
                .bind(approval_id)
                .execute(&mut *tx)
                .await?;
            return Err(PersistenceError::ApprovalSuperseded);
        }
        let state = if approve { "approved" } else { "rejected" };
        sqlx::query(
            "UPDATE approval_requests SET state = $1, decided_by = $2, decided_at = now() WHERE id = $3",
        )
        .bind(state)
        .bind(actor_id)
        .bind(approval_id)
        .execute(&mut *tx)
        .await?;
        human_audit(
            &mut tx,
            project_id,
            actor_id,
            "decide_approval_request",
            request.0,
        )
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": request.0, "event": "approval_changed" }),
        )
        .await?;
        tx.commit().await?;
        self.get_approval_request(project_id, approval_id).await
    }

    async fn get_approval_request(
        &self,
        project_id: Uuid,
        approval_id: Uuid,
    ) -> Result<models::ApprovalRequestRecord, Error> {
        sqlx::query_as::<_, models::ApprovalRequestRecord>(
            "SELECT id, issue_id, requested_by, target_version, proposed_operation, state,
                    expires_at, decided_by, decided_at, created_at
             FROM approval_requests WHERE id = $1 AND project_id = $2",
        )
        .bind(approval_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistenceError::ApprovalNotFound)
    }

    pub async fn approval_requests_for_issue(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<Vec<models::ApprovalRequestRecord>, Error> {
        sqlx::query_as::<_, models::ApprovalRequestRecord>(
            "SELECT id, issue_id, requested_by, target_version, proposed_operation, state,
                    expires_at, decided_by, decided_at, created_at
             FROM approval_requests
             WHERE project_id = $1 AND issue_id = $2
             ORDER BY created_at DESC, id DESC
             LIMIT 50",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }
}
