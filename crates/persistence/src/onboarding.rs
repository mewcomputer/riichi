use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::FromRow;
use uuid::Uuid;

use crate::Error;
use crate::triage::{issue_description_content, issue_description_html};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct OnboardingSampleRecord {
    pub project_id: Uuid,
    pub role_id: Uuid,
    pub session_id: Uuid,
    pub triage_issue_id: Uuid,
    pub agent_issue_id: Uuid,
    pub recovery_issue_id: Uuid,
    pub approval_id: Uuid,
    pub recovery_checklist_id: Uuid,
    pub created_at: DateTime<Utc>,
}

async fn create_onboarding_issue(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    team_id: Uuid,
    actor_id: Uuid,
    title: &str,
    agent_eligible: bool,
    spec_complete: bool,
    rank: i64,
) -> Result<Uuid, Error> {
    // This fixture intentionally composes the same persisted projections and
    // event records as the command paths, but keeps one transaction open for
    // the whole onboarding sample so a retry cannot expose a partial workflow.
    let issue_id = Uuid::now_v7();
    let display_key = super::Database::allocate_issue_display_key(tx, project_id).await?;
    let body = "This issue is part of the guided Riichi workflow.";
    sqlx::query(
        "INSERT INTO issues
         (id, project_id, team_id, display_key, title, body, status, agent_eligible, spec_complete)
         VALUES ($1, $2, $3, $4, $5, $6, 'todo', $7, $8)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(team_id)
    .bind(display_key)
    .bind(title)
    .bind(body)
    .bind(agent_eligible)
    .bind(spec_complete)
    .execute(&mut **tx)
    .await?;
    sqlx::query("INSERT INTO issue_dispatch (issue_id, rank) VALUES ($1, $2)")
        .bind(issue_id)
        .bind(rank)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO issue_projects (issue_id, project_id, added_by) VALUES ($1, $2, $3)")
        .bind(issue_id)
        .bind(project_id)
        .bind(actor_id)
        .execute(&mut **tx)
        .await?;
    sqlx::query("INSERT INTO issue_labels (project_id, issue_id, label) VALUES ($1, $2, 'onboarding-sample')")
        .bind(project_id)
    .bind(issue_id)
    .execute(&mut **tx)
    .await?;
    let organization_id: Uuid =
        sqlx::query_scalar("SELECT organization_id FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&mut **tx)
            .await?;
    let document_id = Uuid::now_v7();
    let content = issue_description_content(body);
    let html = issue_description_html(body);
    sqlx::query(
        "INSERT INTO documents
         (id, organization_id, kind, title, owner_team_id, provisioning_state, created_by)
         VALUES ($1, $2, 'issue_description', $3, $4, 'pending', $5)",
    )
    .bind(document_id)
    .bind(organization_id)
    .bind(title)
    .bind(team_id)
    .bind(actor_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO document_bindings (document_id, resource_kind, resource_id, role)
         VALUES ($1, 'issue', $2, 'description')",
    )
    .bind(document_id)
    .bind(issue_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO document_versions
         (document_id, revision, content, plain_text, sanitized_html, schema_version, created_by)
         VALUES ($1, 1, $2, $3, $4, 2, $5)",
    )
    .bind(document_id)
    .bind(content)
    .bind(body)
    .bind(html.clone())
    .bind(actor_id)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO document_projections
         (document_id, content_revision, plain_text, sanitized_html, schema_version)
         VALUES ($1, 1, $2, $3, 2)",
    )
    .bind(document_id)
    .bind(body)
    .bind(html)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO document_jobs (id, document_id, job_type, idempotency_key)
         VALUES ($1, $2, 'provision', $3)
         ON CONFLICT (job_type, idempotency_key) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(document_id)
    .bind(format!("issue-description:{issue_id}"))
    .execute(&mut **tx)
    .await?;
    super::insert_outbox(
        tx,
        project_id,
        "issue_changed",
        serde_json::json!({ "issue_id": issue_id, "event": "created" }),
    )
    .await?;
    onboarding_audit(
        tx,
        project_id,
        actor_id,
        None,
        None,
        "create_issue",
        issue_id,
    )
    .await?;
    Ok(issue_id)
}

async fn onboarding_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    project_id: Uuid,
    actor_id: Uuid,
    role_id: Option<Uuid>,
    session_id: Option<Uuid>,
    operation: &str,
    target_id: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO audit_records
         (id, project_id, actor_id, role_id, session_id, request_id, operation, target_type, target_id, change_summary)
         VALUES ($1, $2, $3, $4, $5, $6, $7, 'issue', $8, '{}'::jsonb)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(actor_id)
    .bind(role_id)
    .bind(session_id)
    .bind(super::current_request_id())
    .bind(operation)
    .bind(target_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl super::Database {
    pub async fn create_onboarding_sample(
        &self,
        project_id: Uuid,
        actor_id: Uuid,
    ) -> Result<OnboardingSampleRecord, Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT id FROM projects WHERE id = $1 FOR UPDATE")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::InvalidIssue(
                "onboarding project was not found".to_owned(),
            ))?;
        if let Some(sample) = sqlx::query_as::<_, OnboardingSampleRecord>(
            "SELECT project_id, role_id, session_id, triage_issue_id, agent_issue_id,
                    recovery_issue_id, approval_id, recovery_checklist_id, created_at
             FROM onboarding_samples WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        {
            tx.commit().await?;
            return Ok(sample);
        }

        let team_id: Uuid = sqlx::query_scalar(
            "SELECT team_id FROM project_teams WHERE project_id = $1 ORDER BY team_id LIMIT 1",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        let role_id = Uuid::now_v7();
        let capabilities = serde_json::json!(["comment", "complete", "release", "request_spec"]);
        let created_at = Utc::now();
        sqlx::query(
            "INSERT INTO agent_roles
             (id, project_id, team_id, display_name, owner_account_id, capabilities)
             VALUES ($1, $2, $3, 'Onboarding agent', $4, $5)",
        )
        .bind(role_id)
        .bind(project_id)
        .bind(team_id)
        .bind(actor_id)
        .bind(capabilities)
        .execute(&mut *tx)
        .await?;

        let agent_token = format!("{}{}", Uuid::now_v7().simple(), Uuid::now_v7().simple());
        let session_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO sessions
             (id, project_id, team_id, agent_role_id, state, max_lifetime_ends_at, agent_token_hash)
             VALUES ($1, $2, $3, $4, 'active', now() + interval '2 hours', $5)",
        )
        .bind(session_id)
        .bind(project_id)
        .bind(team_id)
        .bind(role_id)
        .bind(super::hash_secret(&agent_token))
        .execute(&mut *tx)
        .await?;

        let triage_issue_id = create_onboarding_issue(
            &mut tx,
            project_id,
            team_id,
            actor_id,
            "Sample: triage a human issue",
            false,
            false,
            10,
        )
        .await?;
        let agent_issue_id = create_onboarding_issue(
            &mut tx,
            project_id,
            team_id,
            actor_id,
            "Sample: agent claim and report",
            true,
            true,
            20,
        )
        .await?;
        let recovery_issue_id = create_onboarding_issue(
            &mut tx,
            project_id,
            team_id,
            actor_id,
            "Sample: recover an agent lease",
            true,
            true,
            30,
        )
        .await?;

        let agent_lease_id = Uuid::now_v7();
        let agent_expires_at = Utc::now() + Duration::minutes(30);
        sqlx::query(
            "INSERT INTO leases (id, issue_id, owner_session_id, fencing_token, state, expires_at)
             VALUES ($1, $2, $3, 1, 'active', $4)",
        )
        .bind(agent_lease_id)
        .bind(agent_issue_id)
        .bind(session_id)
        .bind(agent_expires_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE issues SET status = 'in_progress', version = version + 1, updated_at = now() WHERE id = $1")
            .bind(agent_issue_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE issue_dispatch SET active_lease_id = $2, fencing_token = 1, dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1")
            .bind(agent_issue_id)
            .bind(agent_lease_id)
            .execute(&mut *tx)
            .await?;
        onboarding_audit(
            &mut tx,
            project_id,
            session_id,
            Some(role_id),
            Some(session_id),
            "claim",
            agent_issue_id,
        )
        .await?;
        super::insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({ "issue_id": agent_issue_id, "lease_id": agent_lease_id, "event": "claimed" }),
        )
        .await?;
        sqlx::query(
            "INSERT INTO idempotency_records
             (project_id, actor_id, operation, idempotency_key, request_hash, response)
             VALUES ($1, $2, 'claim', 'onboarding-agent-claim', $3, $4)",
        )
        .bind(project_id)
        .bind(session_id)
        .bind({
            let mut hasher = Sha256::new();
            hasher.update(agent_issue_id.as_bytes());
            hasher.update(Duration::minutes(30).num_seconds().to_le_bytes());
            hasher.finalize().to_vec()
        })
        .bind(serde_json::json!({
            "issue_id": agent_issue_id,
            "lease_id": agent_lease_id,
            "fencing_token": 1,
            "expires_at": agent_expires_at
        }))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO comments (id, project_id, issue_id, author_id, role_id, session_id, body)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(agent_issue_id)
        .bind(session_id)
        .bind(role_id)
        .bind(session_id)
        .bind("The onboarding agent inspected this issue.")
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE leases SET state = 'released', release_reason = 'reported' WHERE id = $1",
        )
        .bind(agent_lease_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE issues SET status = 'todo', version = version + 1, updated_at = now() WHERE id = $1 AND status = 'in_progress'")
            .bind(agent_issue_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE issue_dispatch SET active_lease_id = NULL, dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1")
            .bind(agent_issue_id)
            .execute(&mut *tx)
            .await?;
        onboarding_audit(
            &mut tx,
            project_id,
            session_id,
            Some(role_id),
            Some(session_id),
            "report_batch",
            agent_issue_id,
        )
        .await?;
        super::insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": agent_issue_id, "event": "reported" }),
        )
        .await?;
        sqlx::query(
            "INSERT INTO idempotency_records
             (project_id, actor_id, operation, idempotency_key, request_hash, response)
             VALUES ($1, $2, 'report_batch', 'onboarding-agent-report', $3, $4)",
        )
        .bind(project_id)
        .bind(session_id)
        .bind({
            let batch = super::models::ReportBatch {
                idempotency_key: "onboarding-agent-report".to_owned(),
                operations: vec![
                    super::models::ReportOperation::Comment {
                        body: "The onboarding agent inspected this issue.".to_owned(),
                    },
                    super::models::ReportOperation::Release,
                ],
            };
            let payload =
                serde_json::to_vec(&batch).map_err(|error| sqlx::Error::Encode(Box::new(error)))?;
            Sha256::digest(payload).to_vec()
        })
        .bind(serde_json::json!({
            "issue_id": agent_issue_id,
            "created_issue_ids": [],
            "applied_operations": 2
        }))
        .execute(&mut *tx)
        .await?;

        let recovery_lease_id = Uuid::now_v7();
        let recovery_expires_at = Utc::now() + Duration::minutes(30);
        sqlx::query(
            "INSERT INTO leases (id, issue_id, owner_session_id, fencing_token, state, expires_at)
             VALUES ($1, $2, $3, 1, 'active', $4)",
        )
        .bind(recovery_lease_id)
        .bind(recovery_issue_id)
        .bind(session_id)
        .bind(recovery_expires_at)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE issues SET status = 'in_progress', version = version + 1, updated_at = now() WHERE id = $1")
            .bind(recovery_issue_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE issue_dispatch SET active_lease_id = $2, fencing_token = 1, dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1")
            .bind(recovery_issue_id)
        .bind(recovery_lease_id)
        .execute(&mut *tx)
        .await?;
        onboarding_audit(
            &mut tx,
            project_id,
            session_id,
            Some(role_id),
            Some(session_id),
            "claim",
            recovery_issue_id,
        )
        .await?;
        super::insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({ "issue_id": recovery_issue_id, "lease_id": recovery_lease_id, "event": "claimed" }),
        )
        .await?;
        sqlx::query(
            "INSERT INTO idempotency_records
             (project_id, actor_id, operation, idempotency_key, request_hash, response)
             VALUES ($1, $2, 'claim', 'onboarding-recovery-claim', $3, $4)",
        )
        .bind(project_id)
        .bind(session_id)
        .bind({
            let mut hasher = Sha256::new();
            hasher.update(recovery_issue_id.as_bytes());
            hasher.update(Duration::minutes(30).num_seconds().to_le_bytes());
            hasher.finalize().to_vec()
        })
        .bind(serde_json::json!({
            "issue_id": recovery_issue_id,
            "lease_id": recovery_lease_id,
            "fencing_token": 1,
            "expires_at": recovery_expires_at
        }))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE leases SET state = 'revoked', release_reason = 'human_takeover' WHERE id = $1",
        )
        .bind(recovery_lease_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE issues SET version = version + 1, updated_at = now() WHERE id = $1")
            .bind(recovery_issue_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE issue_dispatch SET active_lease_id = NULL, fencing_token = fencing_token + 1, dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1")
            .bind(recovery_issue_id)
            .execute(&mut *tx)
            .await?;
        onboarding_audit(
            &mut tx,
            project_id,
            actor_id,
            None,
            None,
            "takeover_issue",
            recovery_issue_id,
        )
        .await?;
        super::insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({ "issue_id": recovery_issue_id, "lease_id": recovery_lease_id, "event": "superseded" }),
        )
        .await?;
        super::insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": recovery_issue_id, "event": "takeover" }),
        )
        .await?;
        let recovery_checklist_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO recovery_checklists
             (id, project_id, issue_id, old_lease_id, old_session_id, initiated_by, reason, state)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'open')",
        )
        .bind(recovery_checklist_id)
        .bind(project_id)
        .bind(recovery_issue_id)
        .bind(recovery_lease_id)
        .bind(session_id)
        .bind(actor_id)
        .bind("Review the guided recovery workflow")
        .execute(&mut *tx)
        .await?;
        let approval_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO approval_requests
             (id, project_id, issue_id, requested_by, target_version, proposed_operation, state, expires_at)
             VALUES ($1, $2, $3, $4, 1, $5, 'pending', $6)",
        )
        .bind(approval_id)
        .bind(project_id)
        .bind(triage_issue_id)
        .bind(actor_id)
        .bind(serde_json::json!({"type": "set_rank", "rank": 5}))
        .bind(created_at + Duration::days(1))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO notifications
             (id, recipient_account_id, kind, project_id, issue_id, actor_id, payload, dedupe_key)
             SELECT gen_random_uuid(), pm.account_id, 'approval', $1, $2, $3,
                    jsonb_build_object('approval_id', $4, 'target_version', 1),
                    'approval:' || $4::text
             FROM project_memberships pm
             WHERE pm.project_id = $1
               AND pm.revoked_at IS NULL
               AND pm.role IN ('owner', 'admin')
               AND pm.account_id <> $3
             ON CONFLICT (recipient_account_id, dedupe_key) DO NOTHING",
        )
        .bind(project_id)
        .bind(triage_issue_id)
        .bind(actor_id)
        .bind(approval_id)
        .execute(&mut *tx)
        .await?;
        onboarding_audit(
            &mut tx,
            project_id,
            actor_id,
            None,
            None,
            "create_approval_request",
            triage_issue_id,
        )
        .await?;

        sqlx::query(
            "INSERT INTO onboarding_samples
             (project_id, role_id, session_id, triage_issue_id, agent_issue_id, recovery_issue_id,
              approval_id, recovery_checklist_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(project_id)
        .bind(role_id)
        .bind(session_id)
        .bind(triage_issue_id)
        .bind(agent_issue_id)
        .bind(recovery_issue_id)
        .bind(approval_id)
        .bind(recovery_checklist_id)
        .bind(created_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(OnboardingSampleRecord {
            project_id,
            role_id,
            session_id,
            triage_issue_id,
            agent_issue_id,
            recovery_issue_id,
            approval_id,
            recovery_checklist_id,
            created_at,
        })
    }

    pub async fn onboarding_sample(
        &self,
        project_id: Uuid,
    ) -> Result<Option<OnboardingSampleRecord>, Error> {
        Ok(sqlx::query_as::<_, OnboardingSampleRecord>(
            "SELECT project_id, role_id, session_id, triage_issue_id, agent_issue_id,
                    recovery_issue_id, approval_id, recovery_checklist_id, created_at
             FROM onboarding_samples WHERE project_id = $1",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?)
    }
}
