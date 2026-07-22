use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::FromRow;
use uuid::Uuid;

use crate::Error;

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
    let issue_id = Uuid::now_v7();
    let display_key = super::Database::allocate_issue_display_key(tx, project_id).await?;
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
    .bind("This issue is part of the guided Riichi workflow.")
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
    Ok(issue_id)
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
        sqlx::query(
            "INSERT INTO leases (id, issue_id, owner_session_id, fencing_token, state, expires_at)
             VALUES ($1, $2, $3, 1, 'active', now() + interval '30 minutes')",
        )
        .bind(agent_lease_id)
        .bind(agent_issue_id)
        .bind(session_id)
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

        let recovery_lease_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO leases (id, issue_id, owner_session_id, fencing_token, state, expires_at)
             VALUES ($1, $2, $3, 1, 'active', now() + interval '30 minutes')",
        )
        .bind(recovery_lease_id)
        .bind(recovery_issue_id)
        .bind(session_id)
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
        sqlx::query(
            "UPDATE leases SET state = 'revoked', release_reason = 'human_takeover' WHERE id = $1",
        )
        .bind(recovery_lease_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE issue_dispatch SET active_lease_id = NULL, fencing_token = fencing_token + 1, dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $1")
            .bind(recovery_issue_id)
            .execute(&mut *tx)
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
        let created_at = Utc::now();
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
