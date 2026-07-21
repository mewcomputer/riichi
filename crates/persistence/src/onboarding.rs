use chrono::{DateTime, Utc};
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

impl super::Database {
    pub async fn claim_onboarding_sample(&self, project_id: Uuid) -> Result<bool, Error> {
        let claimed = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO onboarding_sample_claims (project_id)
             VALUES ($1)
             ON CONFLICT (project_id) DO NOTHING
             RETURNING project_id",
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        if claimed.is_some() {
            return Ok(true);
        }
        if self.onboarding_sample(project_id).await?.is_some() {
            return Ok(false);
        }
        Err(crate::Error::Contended)
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

    pub async fn record_onboarding_sample(
        &self,
        sample: &OnboardingSampleRecord,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO onboarding_samples
             (project_id, role_id, session_id, triage_issue_id, agent_issue_id,
              recovery_issue_id, approval_id, recovery_checklist_id, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(sample.project_id)
        .bind(sample.role_id)
        .bind(sample.session_id)
        .bind(sample.triage_issue_id)
        .bind(sample.agent_issue_id)
        .bind(sample.recovery_issue_id)
        .bind(sample.approval_id)
        .bind(sample.recovery_checklist_id)
        .bind(sample.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
