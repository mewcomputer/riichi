use super::*;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProjectOverviewSummary {
    pub project_id: Uuid,
    pub total_issue_count: i64,
    pub moving_count: i64,
    pub blocked_count: i64,
    pub needs_human_count: i64,
    pub agent_handling_count: i64,
    pub stale_lease_count: i64,
    pub pending_approval_count: i64,
    pub unowned_count: i64,
    pub due_soon_count: i64,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProjectOverviewIssue {
    pub id: Uuid,
    pub display_key: String,
    pub title: String,
    pub status: String,
    pub importance: String,
    pub assignee_account_id: Option<Uuid>,
    pub active_lease_id: Option<Uuid>,
    pub lease_expires_at: Option<DateTime<Utc>>,
    pub due_date: Option<chrono::NaiveDate>,
    pub unresolved_blocker_count: i32,
    pub active_hold_count: i32,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ProjectOverviewChange {
    pub id: Uuid,
    pub operation: String,
    pub target_id: Option<Uuid>,
    pub issue_display_key: Option<String>,
    pub actor_id: Uuid,
    pub change_summary: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

const ISSUE_CATEGORY_SQL: &str = "CASE
    WHEN i.status = 'blocked' OR d.unresolved_blocker_count > 0 THEN 'blocked'
    WHEN NOT i.agent_eligible OR d.active_hold_count > 0 OR EXISTS (
        SELECT 1 FROM approval_requests a WHERE a.issue_id = i.id AND a.state = 'pending'
    ) THEN 'needs_human'
    WHEN d.active_lease_id IS NOT NULL THEN 'agent_handling'
    WHEN i.assignee_account_id IS NULL THEN 'unowned'
    WHEN i.due_date BETWEEN current_date AND current_date + 7 THEN 'due_soon'
    WHEN i.status = 'in_progress' THEN 'moving'
    ELSE 'other'
END";

impl Database {
    pub async fn project_overview(
        &self,
        project_id: Uuid,
    ) -> Result<
        (
            ProjectOverviewSummary,
            Vec<ProjectOverviewIssue>,
            bool,
            Vec<ProjectOverviewChange>,
        ),
        Error,
    > {
        let summary_sql = format!("WITH categorized AS (
                 SELECT i.*, d.active_lease_id, d.unresolved_blocker_count, d.active_hold_count,
                        l.expires_at, {ISSUE_CATEGORY_SQL} AS category
                 FROM issues i
                 JOIN issue_dispatch d ON d.issue_id = i.id
                 LEFT JOIN leases l ON l.id = d.active_lease_id AND l.state = 'active'
                 WHERE i.project_id = $1
             )
             SELECT $1 AS project_id,
                    count(*)::bigint AS total_issue_count,
                    count(*) FILTER (WHERE status = 'in_progress')::bigint AS moving_count,
                    count(*) FILTER (WHERE status = 'blocked' OR unresolved_blocker_count > 0)::bigint AS blocked_count,
                    count(*) FILTER (WHERE NOT agent_eligible OR active_hold_count > 0 OR EXISTS (
                        SELECT 1 FROM approval_requests a WHERE a.issue_id = categorized.id AND a.state = 'pending'
                    ))::bigint AS needs_human_count,
                    count(*) FILTER (WHERE active_lease_id IS NOT NULL)::bigint AS agent_handling_count,
                    count(*) FILTER (WHERE expires_at <= now() + interval '24 hours')::bigint AS stale_lease_count,
                    (SELECT count(*)::bigint FROM approval_requests a WHERE a.project_id = $1 AND a.state = 'pending') AS pending_approval_count,
                    count(*) FILTER (WHERE category = 'unowned')::bigint AS unowned_count,
                    count(*) FILTER (WHERE due_date BETWEEN current_date AND current_date + 7)::bigint AS due_soon_count
             FROM categorized") ;
        let summary = sqlx::query_as::<_, ProjectOverviewSummary>(&summary_sql)
            .bind(project_id)
            .fetch_one(&self.pool)
            .await?;
        let issues_sql = format!(
            "SELECT i.id, i.display_key, i.title, i.status, i.importance,
                    i.assignee_account_id, d.active_lease_id, l.expires_at,
                    i.due_date, d.unresolved_blocker_count, d.active_hold_count,
                    {ISSUE_CATEGORY_SQL} AS category
             FROM issues i
             JOIN issue_dispatch d ON d.issue_id = i.id
             LEFT JOIN leases l ON l.id = d.active_lease_id AND l.state = 'active'
             WHERE i.project_id = $1
             ORDER BY CASE WHEN i.status IN ('done', 'canceled') THEN 1 ELSE 0 END,
                      COALESCE(i.due_date, current_date + 3650), d.rank, i.id
             LIMIT 201"
        );
        let mut issues = sqlx::query_as::<_, ProjectOverviewIssue>(&issues_sql)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        let issues_truncated = issues.len() > 200;
        issues.truncate(200);
        let changes = sqlx::query_as::<_, ProjectOverviewChange>(
            "SELECT a.id, a.operation, a.target_id, i.display_key AS issue_display_key,
                    a.actor_id, a.change_summary, a.created_at
             FROM audit_records a
             LEFT JOIN issues i ON i.id = a.target_id AND a.target_type = 'issue'
             WHERE a.project_id = $1
             ORDER BY a.created_at DESC, a.id DESC
             LIMIT 25",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok((summary, issues, issues_truncated, changes))
    }
}
