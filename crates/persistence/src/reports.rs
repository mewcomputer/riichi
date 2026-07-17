use super::*;
use sha2::{Digest, Sha256};

fn report_request_hash(batch: &models::ReportBatch) -> Result<Vec<u8>, Error> {
    let payload =
        serde_json::to_vec(batch).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    Ok(Sha256::digest(payload).to_vec())
}

fn valid_status(status: &str) -> bool {
    matches!(
        status,
        "triage" | "todo" | "in_progress" | "blocked" | "done" | "canceled"
    )
}

fn require_text(field: &str, value: &str, max: usize) -> Result<(), Error> {
    let length = value.trim().chars().count();
    if length == 0 || length > max {
        return Err(PersistenceError::InvalidIssue(format!(
            "{field} must contain between 1 and {max} characters"
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn quarantine_report(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    issue_id: Uuid,
    session_id: Uuid,
    role_id: Uuid,
    lease_id: Uuid,
    fencing_token: i64,
    reason: &str,
    batch: &models::ReportBatch,
) -> Result<(), Error> {
    let request_id = current_request_id();
    let payload =
        serde_json::to_value(batch).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
    sqlx::query(
        "INSERT INTO quarantined_attempts
         (id, project_id, issue_id, session_id, role_id, lease_id, fencing_token, request_id, reason, payload)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
        .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(issue_id)
    .bind(session_id)
    .bind(role_id)
    .bind(lease_id)
    .bind(fencing_token)
    .bind(request_id)
    .bind(reason)
    .bind(payload)
    .execute(&mut **tx)
    .await?;
    insert_audit(
        tx,
        project_id,
        session_id,
        role_id,
        "quarantine_stale_report",
        issue_id,
    )
    .await
}

impl Database {
    pub async fn report_batch(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        lease_id: Uuid,
        fencing_token: i64,
        batch: models::ReportBatch,
    ) -> Result<models::ReportBatchResult, Error> {
        if batch.idempotency_key.trim().is_empty() {
            return Err(PersistenceError::IdempotencyKeyRequired);
        }
        if batch.operations.is_empty() || batch.operations.len() > 50 {
            return Err(PersistenceError::InvalidIssue(
                "report batch must contain between 1 and 50 operations".to_owned(),
            ));
        }
        let request_hash = report_request_hash(&batch)?;
        let mut tx = self.pool.begin().await?;
        let session = self.session(&mut *tx, project_id, session_id).await?;
        ensure_session_active(&session)?;
        if let Some(existing) = sqlx::query_as::<_, IdempotencyRow>(
            "SELECT request_hash, response FROM idempotency_records
             WHERE project_id = $1 AND actor_id = $2 AND operation = 'report_batch'
               AND idempotency_key = $3 FOR UPDATE",
        )
        .bind(project_id)
        .bind(session_id)
        .bind(&batch.idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            if existing.request_hash != request_hash {
                return Err(PersistenceError::IdempotencyConflict);
            }
            let result = serde_json::from_value(existing.response)
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
            tx.commit().await?;
            return Ok(result);
        }

        let lease = sqlx::query_as::<_, LeaseRow>(
            "SELECT l.id, l.issue_id, l.owner_session_id, l.fencing_token, l.state, l.expires_at
             FROM leases l JOIN issues i ON i.id = l.issue_id
             WHERE l.id = $1 AND i.project_id = $2 FOR UPDATE",
        )
        .bind(lease_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::LeaseNotFound)?;
        if lease.fencing_token != fencing_token {
            quarantine_report(
                &mut tx,
                project_id,
                lease.issue_id,
                session_id,
                session.agent_role_id,
                lease_id,
                fencing_token,
                "stale_lease",
                &batch,
            )
            .await?;
            tx.commit().await?;
            return Err(PersistenceError::StaleLease);
        }
        if lease.state != "active" || lease.expires_at <= Utc::now() {
            quarantine_report(
                &mut tx,
                project_id,
                lease.issue_id,
                session_id,
                session.agent_role_id,
                lease_id,
                fencing_token,
                "lease_not_active",
                &batch,
            )
            .await?;
            tx.commit().await?;
            return Err(PersistenceError::LeaseNotActive);
        }

        if lease.owner_session_id == session_id && session.owner_account_id.is_some() {
            let capabilities = session
                .capabilities
                .as_array()
                .ok_or(PersistenceError::InvalidCapability)?;
            for operation in &batch.operations {
                let capability = collaborators::operation_capability(operation);
                if !capabilities
                    .iter()
                    .any(|granted| granted.as_str() == Some(capability))
                {
                    return Err(PersistenceError::CapabilityDenied);
                }
            }
        } else if lease.owner_session_id != session_id {
            let grants = sqlx::query_as::<_, (String, String)>(
                "SELECT capability, grant_mode
                 FROM lease_collaborators
                 WHERE lease_id = $1 AND session_id = $2
                   AND revoked_at IS NULL
                   AND (expires_at IS NULL OR expires_at > now())",
            )
            .bind(lease_id)
            .bind(session_id)
            .fetch_all(&mut *tx)
            .await?;
            for operation in &batch.operations {
                let capability = collaborators::operation_capability(operation);
                if !grants.iter().any(|(granted_capability, grant_mode)| {
                    granted_capability == capability && grant_mode == "auto"
                }) {
                    return Err(PersistenceError::CapabilityDenied);
                }
            }
        }

        let mut terminal: Option<&str> = None;
        for operation in &batch.operations {
            match operation {
                models::ReportOperation::Comment { body } => {
                    require_text("comment", body, 20_000)?;
                }
                models::ReportOperation::SetStatus { status } => {
                    if !valid_status(status) || matches!(status.as_str(), "done" | "canceled") {
                        return Err(PersistenceError::InvalidIssue(
                            "terminal statuses require a terminal report operation".to_owned(),
                        ));
                    }
                }
                models::ReportOperation::Release => {
                    if terminal.replace("release").is_some() {
                        return Err(PersistenceError::InvalidIssue(
                            "report batch may contain only one terminal operation".to_owned(),
                        ));
                    }
                }
                models::ReportOperation::Complete { resolution_summary } => {
                    require_text("resolution_summary", resolution_summary, 20_000)?;
                    if terminal.replace("complete").is_some() {
                        return Err(PersistenceError::InvalidIssue(
                            "report batch may contain only one terminal operation".to_owned(),
                        ));
                    }
                }
                models::ReportOperation::CreateDiscovered {
                    display_key,
                    title,
                    body,
                    rank,
                } => {
                    require_text("display_key", display_key, 128)?;
                    require_text("title", title, 500)?;
                    if body.chars().count() > 100_000 || *rank < 0 {
                        return Err(PersistenceError::InvalidIssue(
                            "invalid discovered issue fields".to_owned(),
                        ));
                    }
                }
                models::ReportOperation::AddBlocker { blocker_issue_id } => {
                    if *blocker_issue_id == lease.issue_id {
                        return Err(PersistenceError::InvalidEdge);
                    }
                    let exists = sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
                    )
                    .bind(blocker_issue_id)
                    .bind(project_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if !exists {
                        return Err(PersistenceError::IssueNotFound);
                    }
                    let creates_cycle = sqlx::query_scalar::<_, bool>(
                        "WITH RECURSIVE reachable(id) AS (
                             SELECT target_issue_id FROM issue_edges
                             WHERE project_id = $1 AND edge_type = 'blocks' AND source_issue_id = $2
                             UNION
                             SELECT e.target_issue_id FROM issue_edges e
                             JOIN reachable r ON r.id = e.source_issue_id
                             WHERE e.project_id = $1 AND e.edge_type = 'blocks'
                         )
                         SELECT EXISTS (SELECT 1 FROM reachable WHERE id = $3)",
                    )
                    .bind(project_id)
                    .bind(lease.issue_id)
                    .bind(blocker_issue_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if creates_cycle {
                        return Err(PersistenceError::EdgeCycle);
                    }
                }
                models::ReportOperation::RequestSpec { reason } => {
                    require_text("specification reason", reason, 20_000)?;
                    if terminal.replace("needs_spec").is_some() {
                        return Err(PersistenceError::InvalidIssue(
                            "report batch may contain only one terminal operation".to_owned(),
                        ));
                    }
                }
                models::ReportOperation::MarkDuplicate { duplicate_of } => {
                    if *duplicate_of == lease.issue_id {
                        return Err(PersistenceError::InvalidEdge);
                    }
                    let exists = sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
                    )
                    .bind(duplicate_of)
                    .bind(project_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if !exists {
                        return Err(PersistenceError::IssueNotFound);
                    }
                    if terminal.replace("duplicate").is_some() {
                        return Err(PersistenceError::InvalidIssue(
                            "report batch may contain only one terminal operation".to_owned(),
                        ));
                    }
                }
            }
        }

        let mut created_issue_ids = Vec::new();
        for operation in &batch.operations {
            match operation {
                models::ReportOperation::Comment { body } => {
                    sqlx::query(
                        "INSERT INTO comments
                         (id, project_id, issue_id, author_id, role_id, session_id, body)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)",
                    )
                    .bind(Uuid::now_v7())
                    .bind(project_id)
                    .bind(lease.issue_id)
                    .bind(session_id)
                    .bind(session.agent_role_id)
                    .bind(session_id)
                    .bind(body.trim())
                    .execute(&mut *tx)
                    .await?;
                }
                models::ReportOperation::SetStatus { status } => {
                    sqlx::query(
                        "UPDATE issues SET status = $1, version = version + 1, updated_at = now()
                         WHERE id = $2 AND project_id = $3",
                    )
                    .bind(status)
                    .bind(lease.issue_id)
                    .bind(project_id)
                    .execute(&mut *tx)
                    .await?;
                }
                models::ReportOperation::CreateDiscovered {
                    display_key: _,
                    title,
                    body,
                    rank,
                } => {
                    let issue_id = Uuid::now_v7();
                    let display_key = Self::allocate_issue_display_key(&mut tx, project_id).await?;
                    sqlx::query(
                        "INSERT INTO issues
                         (id, project_id, team_id, display_key, title, body, status, agent_eligible, spec_complete)
                         VALUES ($1, $2,
                                 (SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1),
                                 $3, $4, $5, 'triage', false, false)",
                    )
                    .bind(issue_id)
                    .bind(project_id)
                    .bind(display_key)
                    .bind(title)
                    .bind(body)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query("INSERT INTO issue_dispatch (issue_id, rank) VALUES ($1, $2)")
                        .bind(issue_id)
                        .bind(rank)
                        .execute(&mut *tx)
                        .await?;
                    sqlx::query(
                        "INSERT INTO issue_edges
                         (id, project_id, source_issue_id, target_issue_id, edge_type)
                         VALUES ($1, $2, $3, $4, 'discovered_from')",
                    )
                    .bind(Uuid::now_v7())
                    .bind(project_id)
                    .bind(lease.issue_id)
                    .bind(issue_id)
                    .execute(&mut *tx)
                    .await?;
                    created_issue_ids.push(issue_id);
                }
                models::ReportOperation::AddBlocker { blocker_issue_id } => {
                    let duplicate = sqlx::query_scalar::<_, bool>(
                        "SELECT EXISTS (SELECT 1 FROM issue_edges
                         WHERE source_issue_id = $1 AND target_issue_id = $2 AND edge_type = 'blocks')",
                    )
                    .bind(blocker_issue_id)
                    .bind(lease.issue_id)
                    .fetch_one(&mut *tx)
                    .await?;
                    if !duplicate {
                        sqlx::query(
                            "INSERT INTO issue_edges
                             (id, project_id, source_issue_id, target_issue_id, edge_type)
                             VALUES ($1, $2, $3, $4, 'blocks')",
                        )
                        .bind(Uuid::now_v7())
                        .bind(project_id)
                        .bind(blocker_issue_id)
                        .bind(lease.issue_id)
                        .execute(&mut *tx)
                        .await?;
                        sqlx::query(
                            "UPDATE issue_dispatch SET unresolved_blocker_count = unresolved_blocker_count + 1,
                             dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1",
                        )
                        .bind(lease.issue_id)
                        .execute(&mut *tx)
                        .await?;
                    }
                }
                models::ReportOperation::RequestSpec { reason } => {
                    sqlx::query(
                        "INSERT INTO dispatch_holds (id, issue_id, hold_type, reason, created_by)
                         VALUES ($1, $2, 'needs_spec', $3, $4)",
                    )
                    .bind(Uuid::now_v7())
                    .bind(lease.issue_id)
                    .bind(reason.trim())
                    .bind(session_id)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE issue_dispatch SET active_hold_count = active_hold_count + 1,
                         dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1",
                    )
                    .bind(lease.issue_id)
                    .execute(&mut *tx)
                    .await?;
                }
                models::ReportOperation::MarkDuplicate { duplicate_of } => {
                    sqlx::query(
                        "INSERT INTO issue_edges
                         (id, project_id, source_issue_id, target_issue_id, edge_type)
                         VALUES ($1, $2, $3, $4, 'duplicate_of')",
                    )
                    .bind(Uuid::now_v7())
                    .bind(project_id)
                    .bind(lease.issue_id)
                    .bind(duplicate_of)
                    .execute(&mut *tx)
                    .await?;
                    sqlx::query(
                        "UPDATE issues SET status = 'canceled', version = version + 1,
                         updated_at = now() WHERE id = $1",
                    )
                    .bind(lease.issue_id)
                    .execute(&mut *tx)
                    .await?;
                }
                models::ReportOperation::Release | models::ReportOperation::Complete { .. } => {}
            }
        }

        match terminal {
            Some("release") => {
                sqlx::query("UPDATE leases SET state = 'released', release_reason = 'reported' WHERE id = $1")
                    .bind(lease_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE issues SET status = 'todo', version = version + 1, updated_at = now()
                     WHERE id = $1 AND status = 'in_progress'",
                )
                .bind(lease.issue_id)
                .execute(&mut *tx)
                .await?;
            }
            Some("complete") => {
                let summary = batch
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        models::ReportOperation::Complete { resolution_summary } => {
                            Some(resolution_summary)
                        }
                        _ => None,
                    });
                sqlx::query(
                    "INSERT INTO comments
                     (id, project_id, issue_id, author_id, role_id, session_id, body)
                     VALUES ($1, $2, $3, $4, $5, $6, $7)",
                )
                .bind(Uuid::now_v7())
                .bind(project_id)
                .bind(lease.issue_id)
                .bind(session_id)
                .bind(session.agent_role_id)
                .bind(session_id)
                .bind(summary.expect("validated complete operation"))
                .execute(&mut *tx)
                .await?;
                sqlx::query("UPDATE leases SET state = 'completed', release_reason = 'reported' WHERE id = $1")
                    .bind(lease_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE issues SET status = 'done', completed_at = now(), version = version + 1,
                     updated_at = now() WHERE id = $1",
                )
                .bind(lease.issue_id)
                .execute(&mut *tx)
                .await?;
            }
            Some("needs_spec") => {
                sqlx::query("UPDATE leases SET state = 'released', release_reason = 'needs_spec' WHERE id = $1")
                    .bind(lease_id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query(
                    "UPDATE issues SET status = 'blocked', version = version + 1, updated_at = now()
                     WHERE id = $1",
                )
                .bind(lease.issue_id)
                .execute(&mut *tx)
                .await?;
            }
            Some("duplicate") => {
                sqlx::query("UPDATE leases SET state = 'completed', release_reason = 'duplicate' WHERE id = $1")
                    .bind(lease_id)
                    .execute(&mut *tx)
                    .await?;
            }
            None => {}
            Some(_) => unreachable!(),
        }
        if terminal.is_some() {
            sqlx::query(
                "UPDATE issue_dispatch SET active_lease_id = NULL, dispatch_version = dispatch_version + 1,
                 updated_at = now() WHERE issue_id = $1 AND active_lease_id = $2",
            )
            .bind(lease.issue_id)
            .bind(lease_id)
            .execute(&mut *tx)
            .await?;
        }
        insert_audit(
            &mut tx,
            project_id,
            session_id,
            session.agent_role_id,
            "report_batch",
            lease.issue_id,
        )
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": lease.issue_id, "event": "reported" }),
        )
        .await?;
        let result = models::ReportBatchResult {
            issue_id: lease.issue_id,
            created_issue_ids,
            applied_operations: batch.operations.len(),
        };
        sqlx::query(
            "INSERT INTO idempotency_records
             (project_id, actor_id, operation, idempotency_key, request_hash, response)
             VALUES ($1, $2, 'report_batch', $3, $4, $5)",
        )
        .bind(project_id)
        .bind(session_id)
        .bind(&batch.idempotency_key)
        .bind(request_hash)
        .bind(serde_json::to_value(&result).map_err(|error| sqlx::Error::Encode(Box::new(error)))?)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    pub async fn quarantined_attempts(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        limit: i64,
    ) -> Result<Vec<models::QuarantinedAttemptRecord>, Error> {
        let limit = limit.clamp(1, 100);
        sqlx::query_as::<_, models::QuarantinedAttemptRecord>(
            "SELECT id, issue_id, session_id, role_id, lease_id, fencing_token,
                    request_id, reason, payload, created_at
             FROM quarantined_attempts
             WHERE project_id = $1 AND issue_id = $2
             ORDER BY created_at DESC, id DESC
             LIMIT $3",
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn quarantined_attempts_for_agent(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
    ) -> Result<Vec<models::QuarantinedAttemptRecord>, Error> {
        let allowed = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1
                 FROM lease_collaborators c
                 JOIN leases l ON l.id = c.lease_id
                 JOIN issues i ON i.id = l.issue_id
                 WHERE i.project_id = $1 AND l.issue_id = $2
                   AND c.session_id = $3 AND c.capability = 'recovery_review'
                   AND c.revoked_at IS NULL
                   AND (c.expires_at IS NULL OR c.expires_at > now())
             )",
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(session_id)
        .fetch_one(&self.pool)
        .await?;
        if !allowed {
            return Err(PersistenceError::CapabilityDenied);
        }
        self.quarantined_attempts(project_id, issue_id, 100).await
    }
}
