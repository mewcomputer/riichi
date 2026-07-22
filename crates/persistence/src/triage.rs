use super::*;

const ISSUE_STATUSES: [&str; 6] = [
    "triage",
    "todo",
    "in_progress",
    "blocked",
    "done",
    "canceled",
];
const ISSUE_IMPORTANCES: [&str; 5] = ["none", "low", "medium", "high", "urgent"];
const EDGE_TYPES: [&str; 4] = ["blocks", "related", "discovered_from", "duplicate_of"];
const HOLD_TYPES: [&str; 5] = [
    "manual",
    "needs_spec",
    "awaiting_approval",
    "scheduled",
    "integration",
];

#[derive(Debug, sqlx::FromRow)]
struct IssueAuditSnapshot {
    title: String,
    status: String,
    importance: String,
    agent_eligible: bool,
    spec_complete: bool,
    assignee_account_id: Option<Uuid>,
    rank: i64,
    due_date: Option<chrono::NaiveDate>,
    snoozed_until: Option<chrono::NaiveDate>,
    labels: Vec<String>,
}

fn validate_text(field: &str, value: &str, max: usize) -> Result<(), Error> {
    let length = value.trim().chars().count();
    if length == 0 || length > max {
        return Err(PersistenceError::InvalidIssue(format!(
            "{field} must contain between 1 and {max} characters"
        )));
    }
    Ok(())
}

fn validate_issue_create(issue: &models::IssueCreate) -> Result<(), Error> {
    validate_text("title", &issue.title, 500)?;
    if issue.body.chars().count() > 100_000 {
        return Err(PersistenceError::InvalidIssue(
            "body cannot exceed 100000 characters".to_owned(),
        ));
    }
    if !ISSUE_STATUSES.contains(&issue.status.as_str()) {
        return Err(PersistenceError::InvalidIssue(
            "invalid issue status".to_owned(),
        ));
    }
    validate_labels(&issue.labels)
}

fn validate_labels(labels: &[String]) -> Result<(), Error> {
    if labels.len() > 50 {
        return Err(PersistenceError::InvalidIssue(
            "an issue cannot have more than 50 labels".to_owned(),
        ));
    }
    if labels.iter().any(|label| {
        let length = label.trim().chars().count();
        length == 0 || length > 64
    }) {
        return Err(PersistenceError::InvalidIssue(
            "labels must contain between 1 and 64 characters".to_owned(),
        ));
    }
    Ok(())
}

async fn insert_human_audit(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor_id: Uuid,
    operation: &str,
    target_id: Uuid,
) -> Result<(), Error> {
    insert_human_audit_with_summary(
        tx,
        project_id,
        actor_id,
        operation,
        target_id,
        serde_json::json!({}),
    )
    .await
}

async fn insert_human_audit_with_summary(
    tx: &mut Transaction<'_, Postgres>,
    project_id: Uuid,
    actor_id: Uuid,
    operation: &str,
    target_id: Uuid,
    change_summary: serde_json::Value,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO audit_records
         (id, project_id, actor_id, request_id, operation, target_type, target_id, change_summary)
         VALUES ($1, $2, $3, $4, $5, 'issue', $6, $7)",
    )
    .bind(Uuid::now_v7())
    .bind(project_id)
    .bind(actor_id)
    .bind(current_request_id())
    .bind(operation)
    .bind(target_id)
    .bind(change_summary)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn issue_change_summary(
    snapshot: &IssueAuditSnapshot,
    update: &models::IssueUpdate,
) -> serde_json::Value {
    let mut changes = Vec::new();
    if let Some(value) = &update.title
        && value != &snapshot.title
    {
        changes.push(serde_json::json!({ "field": "title", "from": snapshot.title, "to": value }));
    }
    if let Some(value) = &update.status
        && value != &snapshot.status
    {
        changes
            .push(serde_json::json!({ "field": "status", "from": snapshot.status, "to": value }));
    }
    if let Some(value) = &update.importance
        && value != &snapshot.importance
    {
        changes.push(
            serde_json::json!({ "field": "importance", "from": snapshot.importance, "to": value }),
        );
    }
    if let Some(value) = update.agent_eligible
        && value != snapshot.agent_eligible
    {
        changes.push(serde_json::json!({ "field": "agent eligibility", "from": snapshot.agent_eligible, "to": value }));
    }
    if let Some(value) = update.spec_complete
        && value != snapshot.spec_complete
    {
        changes.push(serde_json::json!({ "field": "specification", "from": snapshot.spec_complete, "to": value }));
    }
    if let Some(value) = update.rank
        && value != snapshot.rank
    {
        changes.push(serde_json::json!({ "field": "rank", "from": snapshot.rank, "to": value }));
    }
    if let Some(value) = &update.labels
        && value != &snapshot.labels
    {
        changes
            .push(serde_json::json!({ "field": "labels", "from": snapshot.labels, "to": value }));
    }
    if let Some(value) = &update.due_date {
        let value = value.map(|date| date.to_string());
        let from = snapshot.due_date.map(|date| date.to_string());
        if value != from {
            changes.push(serde_json::json!({ "field": "due date", "from": from, "to": value }));
        }
    }
    if let Some(value) = &update.snoozed_until {
        let value = value.map(|date| date.to_string());
        let from = snapshot.snoozed_until.map(|date| date.to_string());
        if value != from {
            changes
                .push(serde_json::json!({ "field": "snoozed until", "from": from, "to": value }));
        }
    }
    if let Some(value) = update.assignee_account_id
        && Some(value) != snapshot.assignee_account_id
    {
        changes.push(serde_json::json!({ "field": "assignee", "changed": true }));
    }
    serde_json::json!({ "changes": changes })
}

async fn refresh_blocker_count(
    tx: &mut Transaction<'_, Postgres>,
    issue_id: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE issue_dispatch d
         SET unresolved_blocker_count = (
             SELECT count(*)::integer
             FROM issue_edges e
             JOIN issues blocker ON blocker.id = e.source_issue_id
             WHERE e.target_issue_id = $1
               AND e.edge_type = 'blocks'
               AND blocker.status NOT IN ('done', 'canceled')
         ),
         dispatch_version = dispatch_version + 1,
         updated_at = now()
         WHERE d.issue_id = $1",
    )
    .bind(issue_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn refresh_hold_count(
    tx: &mut Transaction<'_, Postgres>,
    issue_id: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "UPDATE issue_dispatch d
         SET active_hold_count = (
             SELECT count(*)::integer
             FROM dispatch_holds h
             WHERE h.issue_id = $1
               AND h.released_at IS NULL
               AND (h.expires_at IS NULL OR h.expires_at > now())
         ),
         dispatch_version = dispatch_version + 1,
         updated_at = now()
         WHERE d.issue_id = $1",
    )
    .bind(issue_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

impl Database {
    pub(crate) async fn allocate_issue_display_key(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        project_id: Uuid,
    ) -> Result<String, Error> {
        let team = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT t.id, t.key
             FROM project_teams pt
             JOIN teams t ON t.id = pt.team_id
             WHERE pt.project_id = $1
             ORDER BY pt.team_id
             LIMIT 1",
        )
        .bind(project_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| Error::InvalidIssue("issue project has no owning team".to_owned()))?;

        sqlx::query(
            "INSERT INTO team_issue_sequences (team_id, next_number)
             VALUES ($1, 1)
             ON CONFLICT (team_id) DO NOTHING",
        )
        .bind(team.0)
        .execute(&mut **tx)
        .await?;

        let next_number = sqlx::query_scalar::<_, i64>(
            "SELECT next_number
             FROM team_issue_sequences
             WHERE team_id = $1
             FOR UPDATE",
        )
        .bind(team.0)
        .fetch_one(&mut **tx)
        .await?;

        sqlx::query("UPDATE team_issue_sequences SET next_number = $2 WHERE team_id = $1")
            .bind(team.0)
            .bind(next_number + 1)
            .execute(&mut **tx)
            .await?;

        Ok(format!("{}-{}", team.1, next_number))
    }

    pub async fn create_issue_with_metadata(
        &self,
        project_id: Uuid,
        issue: models::IssueCreate,
        actor_id: Uuid,
    ) -> Result<Uuid, Error> {
        validate_issue_create(&issue)?;
        let mut tx = self.pool.begin().await?;
        if let Some(parent_issue_id) = issue.parent_issue_id {
            let parent_exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE project_id = $1 AND id = $2)",
            )
            .bind(project_id)
            .bind(parent_issue_id)
            .fetch_one(&mut *tx)
            .await?;
            if !parent_exists {
                return Err(PersistenceError::InvalidIssue(
                    "parent issue must belong to the same project".to_owned(),
                ));
            }
        }
        let display_key = Self::allocate_issue_display_key(&mut tx, project_id).await?;
        sqlx::query(
            "INSERT INTO issues
             (id, project_id, team_id, display_key, title, body, status, agent_eligible, spec_complete,
              assignee_account_id, parent_issue_id)
             VALUES ($1, $2,
                     (SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1),
                     $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(issue.id)
        .bind(project_id)
        .bind(display_key)
        .bind(&issue.title)
        .bind(&issue.body)
        .bind(&issue.status)
        .bind(issue.agent_eligible)
        .bind(issue.spec_complete)
        .bind(issue.assignee_account_id)
        .bind(issue.parent_issue_id)
        .execute(&mut *tx)
        .await?;
        let (team_id, organization_id) = sqlx::query_as::<_, (Uuid, Uuid)>(
            "SELECT i.team_id, p.organization_id
             FROM issues i
             JOIN projects p ON p.id = i.project_id
             WHERE i.id = $1",
        )
        .bind(issue.id)
        .fetch_one(&mut *tx)
        .await?;
        let description_document_id = Uuid::now_v7();
        let description_content = issue_description_content(&issue.body);
        let description_html = issue_description_html(&issue.body);
        sqlx::query(
            "INSERT INTO documents
             (id, organization_id, kind, title, owner_team_id, provisioning_state, created_by)
             VALUES ($1, $2, 'issue_description', $3, $4, 'pending', $5)",
        )
        .bind(description_document_id)
        .bind(organization_id)
        .bind(&issue.title)
        .bind(team_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_bindings
             (document_id, resource_kind, resource_id, role)
             VALUES ($1, 'issue', $2, 'description')",
        )
        .bind(description_document_id)
        .bind(issue.id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_versions
             (document_id, revision, content, plain_text, sanitized_html, schema_version, created_by)
             VALUES ($1, 1, $2, $3, $4, 2, $5)",
        )
        .bind(description_document_id)
        .bind(&description_content)
        .bind(&issue.body)
        .bind(&description_html)
        .bind(actor_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_projections
             (document_id, content_revision, plain_text, sanitized_html, schema_version)
             VALUES ($1, 1, $2, $3, 2)",
        )
        .bind(description_document_id)
        .bind(&issue.body)
        .bind(&description_html)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_jobs
             (id, document_id, job_type, idempotency_key)
             VALUES ($1, $2, 'provision', $3)
             ON CONFLICT (job_type, idempotency_key) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(description_document_id)
        .bind(format!("issue-description:{}", issue.id))
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO issue_projects (issue_id, project_id, added_by)
             VALUES ($1, $2, $3)
             ON CONFLICT (issue_id, project_id) DO NOTHING",
        )
        .bind(issue.id)
        .bind(project_id)
        .bind(actor_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO issue_dispatch (issue_id, rank) VALUES ($1, $2)")
            .bind(issue.id)
            .bind(issue.rank.max(0))
            .execute(&mut *tx)
            .await?;
        for label in issue.labels {
            sqlx::query(
                "INSERT INTO issue_labels (project_id, issue_id, label)
                 VALUES ($1, $2, $3)",
            )
            .bind(project_id)
            .bind(issue.id)
            .bind(label.trim())
            .execute(&mut *tx)
            .await?;
        }
        insert_human_audit(&mut tx, project_id, actor_id, "create_issue", issue.id).await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": issue.id, "event": "created" }),
        )
        .await?;
        tx.commit().await?;
        Ok(issue.id)
    }

    pub async fn get_issue(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<models::IssueRecord, Error> {
        let mut issue = sqlx::query_as::<_, models::IssueRecord>(
            "SELECT i.project_id,
                    i.team_id,
                    i.id,
                    i.parent_issue_id,
                    i.display_key,
                    i.title,
                    COALESCE((SELECT p.plain_text
                              FROM document_bindings b
                              JOIN document_projections p ON p.document_id = b.document_id
                              WHERE b.resource_kind = 'issue'
                                AND b.resource_id = i.id
                                AND b.role = 'description'
                              LIMIT 1), i.body) AS body,
                    i.status,
                    i.importance,
                    i.agent_eligible,
                    i.spec_complete,
                    CASE
                        WHEN NOT EXISTS (
                            SELECT 1 FROM document_bindings b
                            WHERE b.resource_kind = 'issue'
                              AND b.resource_id = i.id
                              AND b.role = 'description'
                        ) THEN false
                        WHEN i.spec_reviewed_frontiers IS NULL THEN true
                        ELSE i.spec_reviewed_frontiers IS DISTINCT FROM (
                            SELECT s.frontiers
                            FROM document_bindings b
                            JOIN document_loro_snapshots s ON s.document_id = b.document_id
                            WHERE b.resource_kind = 'issue'
                              AND b.resource_id = i.id
                              AND b.role = 'description'
                            LIMIT 1
                        )
                    END AS specification_changed_since_review,
                    i.assignee_account_id,
                    i.version,
                    i.created_at,
                    i.updated_at,
                    i.completed_at,
                    i.due_date,
                    i.snoozed_until,
                    d.rank,
                    d.rank_scope,
                    d.dispatch_version,
                    d.unresolved_blocker_count,
                    d.active_hold_count,
                    d.active_lease_id,
                    l.expires_at AS lease_expires_at,
                    l.owner_session_id AS active_owner_session_id,
                    active_session.agent_role_id AS active_owner_role_id,
                    COALESCE(array_agg(il.label ORDER BY il.label)
                        FILTER (WHERE il.label IS NOT NULL), ARRAY[]::text[]) AS labels
             FROM issues i
             JOIN issue_dispatch d ON d.issue_id = i.id
             LEFT JOIN leases l ON l.id = d.active_lease_id AND l.state = 'active'
             LEFT JOIN sessions active_session ON active_session.id = l.owner_session_id
             LEFT JOIN issue_labels il ON il.issue_id = i.id
             WHERE i.project_id = $1 AND i.id = $2
             GROUP BY i.id, d.issue_id, d.rank, d.rank_scope, d.dispatch_version,
                      d.unresolved_blocker_count, d.active_hold_count, d.active_lease_id,
                      l.expires_at, l.owner_session_id, active_session.agent_role_id",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistenceError::IssueNotFound)?;
        issue.edges = sqlx::query_as::<_, models::IssueEdgeRecord>(
            "SELECT id, source_issue_id, target_issue_id, edge_type, created_at
             FROM issue_edges WHERE project_id = $1 AND (source_issue_id = $2 OR target_issue_id = $2)
             ORDER BY created_at DESC, id DESC
             LIMIT 200",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        issue.holds = sqlx::query_as::<_, models::DispatchHoldRecord>(
            "SELECT id, issue_id, hold_type, reason, created_by, created_at, expires_at, released_at
             FROM dispatch_holds WHERE issue_id = $1 ORDER BY created_at, id",
        )
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        issue.collaborators = self.lease_collaborators(project_id, issue_id).await?;
        issue.quarantined_attempt_count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM quarantined_attempts
             WHERE project_id = $1 AND issue_id = $2",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_one(&self.pool)
        .await?;
        issue.approvals = self
            .approval_requests_for_issue(project_id, issue_id)
            .await?;
        issue.comments = sqlx::query_as::<_, models::CommentRecord>(
            "SELECT id, author_id, role_id, session_id, body, content, created_at
             FROM comments WHERE project_id = $1 AND issue_id = $2
             ORDER BY created_at, id",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        issue.projects = sqlx::query_as::<_, models::IssueProjectRecord>(
            "SELECT ip.project_id, p.name AS project_name, ip.created_at
             FROM issue_projects ip
             JOIN projects p ON p.id = ip.project_id
             WHERE ip.issue_id = $1
             ORDER BY ip.created_at, ip.project_id",
        )
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        issue.children = sqlx::query_as::<_, models::SubissueRecord>(
            "SELECT id, display_key, title, status, importance
             FROM issues i
             JOIN issue_dispatch d ON d.issue_id = i.id
             WHERE i.parent_issue_id = $1
             ORDER BY d.rank, i.id",
        )
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(issue)
    }

    pub async fn create_human_comment(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        author_id: Uuid,
        body: &str,
        content: serde_json::Value,
    ) -> Result<models::CommentRecord, Error> {
        let mut tx = self.pool.begin().await?;
        let comment = sqlx::query_as::<_, models::CommentRecord>(
            "INSERT INTO comments (id, project_id, issue_id, author_id, body, content)
             SELECT $1, $2, $3, $4, $5, $6
             WHERE EXISTS (SELECT 1 FROM issues WHERE id = $3 AND project_id = $2)
             RETURNING id, author_id, role_id, session_id, body, content, created_at",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(issue_id)
        .bind(author_id)
        .bind(body)
        .bind(content)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::IssueNotFound)?;
        sqlx::query(
            "INSERT INTO notifications
             (id, recipient_account_id, kind, project_id, issue_id, actor_id, payload, dedupe_key)
             SELECT gen_random_uuid(), pm.account_id, 'comment', $1, $2, $3,
                    jsonb_build_object('comment_id', $4, 'body', left($5, 240)),
                    'comment:' || $4::text
             FROM project_memberships pm
             WHERE pm.project_id = $1
               AND pm.revoked_at IS NULL
               AND pm.account_id <> $3
             ON CONFLICT (recipient_account_id, dedupe_key) DO NOTHING",
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(author_id)
        .bind(comment.id)
        .bind(body)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO audit_records
             (id, project_id, actor_id, request_id, operation, target_type, target_id)
             VALUES ($1, $2, $3, $4, 'create_comment', 'issue', $5)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(author_id)
        .bind(current_request_id())
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO outbox_messages (id, project_id, message_type, payload)
             VALUES ($1, $2, 'issue_changed', $3)",
        )
        .bind(Uuid::now_v7())
        .bind(project_id)
        .bind(serde_json::json!({ "issue_id": issue_id, "event": "comment_added" }))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(comment)
    }

    pub async fn issue_activity(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        limit: i64,
    ) -> Result<Vec<models::ActivityRecord>, Error> {
        let limit = limit.clamp(1, 200);
        Ok(sqlx::query_as::<_, models::ActivityRecord>(
            "SELECT id, kind, actor_id, body, metadata, created_at
             FROM issue_activity_sync
             WHERE project_id = $1 AND issue_id = $2
             ORDER BY created_at DESC, id DESC
             LIMIT $3",
        )
        .bind(project_id)
        .bind(issue_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .rev()
        .collect())
    }

    pub async fn update_issue(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        update: models::IssueUpdate,
        actor_id: Uuid,
    ) -> Result<models::IssueRecord, Error> {
        Ok(self
            .update_issue_with_transaction(project_id, issue_id, update, actor_id)
            .await?
            .issue)
    }

    pub async fn update_issue_with_transaction(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        update: models::IssueUpdate,
        actor_id: Uuid,
    ) -> Result<models::IssueUpdateResult, Error> {
        if let Some(status) = &update.status
            && !ISSUE_STATUSES.contains(&status.as_str())
        {
            return Err(PersistenceError::InvalidIssue(
                "invalid issue status".to_owned(),
            ));
        }
        if let Some(importance) = &update.importance
            && !ISSUE_IMPORTANCES.contains(&importance.as_str())
        {
            return Err(PersistenceError::InvalidIssue(
                "invalid issue importance".to_owned(),
            ));
        }
        if let Some(title) = &update.title {
            validate_text("title", title, 500)?;
        }
        if let Some(labels) = &update.labels {
            validate_labels(labels)?;
        }
        if update.expected_version < 1 || update.rank.is_some_and(|rank| rank < 0) {
            return Err(PersistenceError::InvalidIssue(
                "invalid issue version or rank".to_owned(),
            ));
        }

        let status_changed = update.status.is_some();
        let mut tx = self.pool.begin().await?;
        let snapshot = sqlx::query_as::<_, IssueAuditSnapshot>(
            "SELECT i.title,
                    i.status,
                    i.importance,
                    i.agent_eligible,
                    i.spec_complete,
                    i.assignee_account_id,
                    d.rank,
                    i.due_date,
                    i.snoozed_until,
                    COALESCE(array_agg(il.label ORDER BY il.label)
                        FILTER (WHERE il.label IS NOT NULL), ARRAY[]::text[]) AS labels
             FROM issues i
             JOIN issue_dispatch d ON d.issue_id = i.id
             LEFT JOIN issue_labels il ON il.issue_id = i.id
             WHERE i.id = $1 AND i.project_id = $2
             GROUP BY i.id, d.rank",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::IssueNotFound)?;
        let change_summary = issue_change_summary(&snapshot, &update);
        let reviewed_frontiers = if update.spec_complete == Some(true) {
            sqlx::query_scalar::<_, Option<serde_json::Value>>(
                "SELECT s.frontiers
                 FROM document_bindings b
                 JOIN document_loro_snapshots s ON s.document_id = b.document_id
                 WHERE b.resource_kind = 'issue'
                   AND b.resource_id = $1
                   AND b.role = 'description'
                 LIMIT 1",
            )
            .bind(issue_id)
            .fetch_one(&mut *tx)
            .await?
        } else {
            None
        };
        let changed = sqlx::query(
            "UPDATE issues
             SET title = COALESCE($3, title),
                 status = COALESCE($4, status),
                 importance = COALESCE($5, importance),
                 agent_eligible = COALESCE($6, agent_eligible),
                 spec_complete = COALESCE($7, spec_complete),
                 spec_reviewed_frontiers = CASE
                     WHEN $7 IS TRUE THEN COALESCE($10, spec_reviewed_frontiers)
                     WHEN $7 IS FALSE THEN NULL
                     ELSE spec_reviewed_frontiers
                 END,
                 assignee_account_id = COALESCE($8, assignee_account_id),
                 due_date = CASE WHEN $11 THEN $12 ELSE due_date END,
                 snoozed_until = CASE WHEN $13 THEN $14 ELSE snoozed_until END,
                 version = version + 1,
                 updated_at = now(),
                 completed_at = CASE
                     WHEN COALESCE($4, status) = 'done' THEN COALESCE(completed_at, now())
                     WHEN COALESCE($4, status) <> 'done' THEN NULL
                     ELSE completed_at
                 END
             WHERE id = $1 AND project_id = $2 AND version = $9",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(&update.title)
        .bind(&update.status)
        .bind(&update.importance)
        .bind(update.agent_eligible)
        .bind(update.spec_complete)
        .bind(update.assignee_account_id)
        .bind(update.expected_version)
        .bind(reviewed_frontiers)
        .bind(update.due_date.is_some())
        .bind(update.due_date.flatten())
        .bind(update.snoozed_until.is_some())
        .bind(update.snoozed_until.flatten())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed == 0 {
            let exists = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
            )
            .bind(issue_id)
            .bind(project_id)
            .fetch_one(&mut *tx)
            .await?;
            return Err(if exists {
                PersistenceError::VersionConflict
            } else {
                PersistenceError::IssueNotFound
            });
        }
        if let Some(title) = &update.title {
            sqlx::query(
                "UPDATE documents d
                 SET title = $2, updated_at = now()
                 FROM document_bindings b
                 WHERE b.document_id = d.id
                   AND b.resource_kind = 'issue'
                   AND b.resource_id = $1
                   AND b.role = 'description'
                   AND d.deleted_at IS NULL",
            )
            .bind(issue_id)
            .bind(title)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(rank) = update.rank {
            sqlx::query(
                "UPDATE issue_dispatch SET rank = $2, dispatch_version = dispatch_version + 1,
                 updated_at = now() WHERE issue_id = $1",
            )
            .bind(issue_id)
            .bind(rank)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "UPDATE issue_dispatch SET dispatch_version = dispatch_version + 1,
                 updated_at = now() WHERE issue_id = $1",
            )
            .bind(issue_id)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(labels) = update.labels {
            sqlx::query("DELETE FROM issue_labels WHERE issue_id = $1")
                .bind(issue_id)
                .execute(&mut *tx)
                .await?;
            for label in labels {
                sqlx::query(
                    "INSERT INTO issue_labels (project_id, issue_id, label) VALUES ($1, $2, $3)",
                )
                .bind(project_id)
                .bind(issue_id)
                .bind(label.trim())
                .execute(&mut *tx)
                .await?;
            }
        }
        if status_changed {
            let dependent_ids = sqlx::query_scalar::<_, Uuid>(
                "SELECT target_issue_id FROM issue_edges
                 WHERE source_issue_id = $1 AND edge_type = 'blocks'",
            )
            .bind(issue_id)
            .fetch_all(&mut *tx)
            .await?;
            for dependent_id in dependent_ids {
                refresh_blocker_count(&mut tx, dependent_id).await?;
            }
        }
        insert_human_audit_with_summary(
            &mut tx,
            project_id,
            actor_id,
            "update_issue",
            issue_id,
            change_summary,
        )
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({ "issue_id": issue_id, "event": "updated" }),
        )
        .await?;
        let transaction_id = sqlx::query_scalar::<_, i64>("SELECT txid_current()")
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        let issue = self.get_issue(project_id, issue_id).await?;
        Ok(models::IssueUpdateResult {
            issue,
            transaction_id,
        })
    }

    pub async fn create_issue_edge(
        &self,
        project_id: Uuid,
        source_issue_id: Uuid,
        target_issue_id: Uuid,
        edge_type: &str,
        actor_id: Uuid,
    ) -> Result<Uuid, Error> {
        if source_issue_id == target_issue_id || !EDGE_TYPES.contains(&edge_type) {
            return Err(PersistenceError::InvalidEdge);
        }
        let mut tx = self.pool.begin().await?;
        let issue_count = sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM issues WHERE project_id = $1 AND id IN ($2, $3)",
        )
        .bind(project_id)
        .bind(source_issue_id)
        .bind(target_issue_id)
        .fetch_one(&mut *tx)
        .await?;
        if issue_count != 2 {
            return Err(PersistenceError::IssueNotFound);
        }
        let duplicate = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM issue_edges
             WHERE source_issue_id = $1 AND target_issue_id = $2 AND edge_type = $3)",
        )
        .bind(source_issue_id)
        .bind(target_issue_id)
        .bind(edge_type)
        .fetch_one(&mut *tx)
        .await?;
        if duplicate {
            return Err(PersistenceError::InvalidEdge);
        }
        if edge_type == "blocks" {
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
            .bind(target_issue_id)
            .bind(source_issue_id)
            .fetch_one(&mut *tx)
            .await?;
            if creates_cycle {
                return Err(PersistenceError::EdgeCycle);
            }
        }
        let edge_id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO issue_edges (id, project_id, source_issue_id, target_issue_id, edge_type)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(edge_id)
        .bind(project_id)
        .bind(source_issue_id)
        .bind(target_issue_id)
        .bind(edge_type)
        .execute(&mut *tx)
        .await?;
        if edge_type == "blocks" {
            refresh_blocker_count(&mut tx, target_issue_id).await?;
        }
        insert_human_audit(
            &mut tx,
            project_id,
            actor_id,
            "create_issue_edge",
            source_issue_id,
        )
        .await?;
        tx.commit().await?;
        Ok(edge_id)
    }

    pub async fn remove_issue_edge(
        &self,
        project_id: Uuid,
        edge_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let edge = sqlx::query_as::<_, (Uuid, String)>(
            "SELECT target_issue_id, edge_type FROM issue_edges
             WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(edge_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::EdgeNotFound)?;
        sqlx::query("DELETE FROM issue_edges WHERE id = $1")
            .bind(edge_id)
            .execute(&mut *tx)
            .await?;
        if edge.1 == "blocks" {
            refresh_blocker_count(&mut tx, edge.0).await?;
        }
        insert_human_audit(&mut tx, project_id, actor_id, "remove_issue_edge", edge.0).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_hold(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        hold_type: &str,
        reason: &str,
        actor_id: Uuid,
        lifetime: Option<Duration>,
    ) -> Result<Uuid, Error> {
        if !HOLD_TYPES.contains(&hold_type) || reason.trim().is_empty() {
            return Err(PersistenceError::InvalidIssue("invalid hold".to_owned()));
        }
        let mut tx = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !exists {
            return Err(PersistenceError::IssueNotFound);
        }
        let hold_id = Uuid::now_v7();
        let expires_at = lifetime.map(|duration| Utc::now() + duration);
        sqlx::query(
            "INSERT INTO dispatch_holds
             (id, issue_id, hold_type, reason, created_by, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(hold_id)
        .bind(issue_id)
        .bind(hold_type)
        .bind(reason.trim())
        .bind(actor_id)
        .bind(expires_at)
        .execute(&mut *tx)
        .await?;
        refresh_hold_count(&mut tx, issue_id).await?;
        insert_human_audit(&mut tx, project_id, actor_id, "create_hold", issue_id).await?;
        tx.commit().await?;
        Ok(hold_id)
    }

    pub async fn release_hold(
        &self,
        project_id: Uuid,
        hold_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let issue_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT h.issue_id FROM dispatch_holds h
             JOIN issues i ON i.id = h.issue_id
             WHERE h.id = $1 AND i.project_id = $2 AND h.released_at IS NULL
             FOR UPDATE",
        )
        .bind(hold_id)
        .bind(project_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(PersistenceError::HoldNotFound)?;
        sqlx::query("UPDATE dispatch_holds SET released_at = now() WHERE id = $1")
            .bind(hold_id)
            .execute(&mut *tx)
            .await?;
        refresh_hold_count(&mut tx, issue_id).await?;
        insert_human_audit(&mut tx, project_id, actor_id, "release_hold", issue_id).await?;
        tx.commit().await?;
        Ok(())
    }
}

pub(crate) fn issue_description_content(body: &str) -> serde_json::Value {
    if body.is_empty() {
        serde_json::json!({"type": "doc", "content": []})
    } else {
        serde_json::json!({
            "type": "doc",
            "content": [{"type": "paragraph", "content": [{"type": "text", "text": body}]}]
        })
    }
}

pub(crate) fn issue_description_html(body: &str) -> String {
    format!("<p>{}</p>", html_escape(body))
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
