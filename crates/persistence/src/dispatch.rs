use super::*;

impl Database {
    pub async fn human_queue(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<models::HumanQueueIssueRecord>, Error> {
        let limit = limit.clamp(1, 200);
        let issues = sqlx::query_as::<_, models::HumanQueueIssueRecord>(
            "SELECT i.team_id,
                    t.name AS team_name,
                    t.key AS team_key,
                    i.project_id,
                    p.name AS project_name,
                    i.id,
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
                        WHEN NOT EXISTS (
                            SELECT 1
                            FROM document_bindings b
                            JOIN document_loro_snapshots s ON s.document_id = b.document_id
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
                    d.unresolved_blocker_count,
                    d.active_hold_count,
                    d.active_lease_id,
                    l.expires_at AS lease_expires_at,
                    i.created_at,
                    i.updated_at,
                    d.rank,
                    d.dispatch_version,
                    i.assignee_account_id,
                    COALESCE(array_agg(il.label ORDER BY il.label)
                        FILTER (WHERE il.label IS NOT NULL), ARRAY[]::text[]) AS labels
             FROM issues i
             JOIN teams t ON t.id = i.team_id
             JOIN projects p ON p.id = i.project_id
             JOIN issue_dispatch d ON d.issue_id = i.id
             LEFT JOIN leases l
               ON l.id = d.active_lease_id
              AND l.state = 'active'
             LEFT JOIN issue_labels il ON il.issue_id = i.id
             WHERE i.project_id = $1
             GROUP BY i.team_id, t.name, t.key, i.project_id, p.name, i.id,
                      d.issue_id, d.unresolved_blocker_count, d.active_hold_count,
                      d.active_lease_id, l.expires_at, d.rank, d.dispatch_version
             ORDER BY CASE i.status
                        WHEN 'in_progress' THEN 0
                        WHEN 'todo' THEN 1
                        WHEN 'triage' THEN 2
                        WHEN 'blocked' THEN 3
                        WHEN 'done' THEN 4
                        WHEN 'canceled' THEN 5
                        ELSE 6
                      END,
                      d.rank,
                      i.id
             LIMIT $2",
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(issues)
    }

    pub async fn human_all_issues(
        &self,
        account_id: Uuid,
        team_id: Option<Uuid>,
        limit: i64,
    ) -> Result<Vec<models::HumanQueueIssueRecord>, Error> {
        let limit = limit.clamp(1, 200);
        Ok(sqlx::query_as::<_, models::HumanQueueIssueRecord>(
            "SELECT i.team_id,
                    t.name AS team_name,
                    t.key AS team_key,
                    i.project_id,
                    p.name AS project_name,
                    i.id,
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
                        WHEN NOT EXISTS (
                            SELECT 1
                            FROM document_bindings b
                            JOIN document_loro_snapshots s ON s.document_id = b.document_id
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
                    d.unresolved_blocker_count,
                    d.active_hold_count,
                    d.active_lease_id,
                    l.expires_at AS lease_expires_at,
                    i.created_at,
                    i.updated_at,
                    d.rank,
                    d.dispatch_version,
                    i.assignee_account_id,
                    COALESCE(array_agg(il.label ORDER BY il.label)
                        FILTER (WHERE il.label IS NOT NULL), ARRAY[]::text[]) AS labels
             FROM issues i
             JOIN teams t ON t.id = i.team_id
             JOIN projects p ON p.id = i.project_id
             JOIN issue_dispatch d ON d.issue_id = i.id
             LEFT JOIN leases l
               ON l.id = d.active_lease_id
              AND l.state = 'active'
             LEFT JOIN issue_labels il ON il.issue_id = i.id
             WHERE ($2::uuid IS NULL OR i.team_id = $2)
               AND (EXISTS (
                       SELECT 1 FROM project_memberships pm
                       WHERE pm.project_id = i.project_id
                         AND pm.account_id = $1
                         AND pm.revoked_at IS NULL
                   )
                OR EXISTS (
                       SELECT 1 FROM team_memberships tm
                       WHERE tm.team_id = i.team_id
                         AND tm.account_id = $1
                         AND tm.revoked_at IS NULL
                   ))
             GROUP BY i.team_id, t.name, t.key, i.project_id, p.name, i.id, d.issue_id, d.unresolved_blocker_count,
                      d.active_hold_count, d.active_lease_id, l.expires_at, d.rank,
                      d.dispatch_version
             ORDER BY CASE i.status
                        WHEN 'in_progress' THEN 0
                        WHEN 'todo' THEN 1
                        WHEN 'triage' THEN 2
                        WHEN 'blocked' THEN 3
                        WHEN 'done' THEN 4
                        WHEN 'canceled' THEN 5
                        ELSE 6
                      END,
                      d.rank,
                      i.id
             LIMIT $3",
        )
        .bind(account_id)
        .bind(team_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn create_project(&self, id: Uuid, name: &str) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "INSERT INTO projects (id, name, organization_id)
             VALUES ($1, $2, '00000000-0000-0000-0000-000000000001')",
        )
        .bind(id)
        .bind(name)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO project_teams (project_id, team_id, role)
             VALUES ($1, '00000000-0000-0000-0000-000000000002', 'admin')",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn create_agent_role(
        &self,
        id: Uuid,
        project_id: Uuid,
        display_name: &str,
    ) -> Result<(), Error> {
        sqlx::query(
            "INSERT INTO agent_roles (id, project_id, team_id, display_name)
             VALUES ($1, $2,
                     (SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1),
                     $3)",
        )
            .bind(id)
            .bind(project_id)
            .bind(display_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn create_agent_role_with_policy(
        &self,
        id: Uuid,
        project_id: Uuid,
        display_name: &str,
        owner_account_id: Uuid,
        capabilities: Vec<String>,
    ) -> Result<(), Error> {
        if display_name.trim().is_empty()
            || capabilities.iter().any(|capability| {
                !matches!(
                    capability.as_str(),
                    "comment"
                        | "request_spec"
                        | "discover"
                        | "complete"
                        | "release"
                        | "edit_issue"
                        | "manage_relationships"
                        | "recovery_review"
                        | "doc.read"
                        | "doc.apply_edit"
                )
            })
        {
            return Err(PersistenceError::InvalidCapability);
        }
        sqlx::query(
            "INSERT INTO agent_roles
             (id, project_id, team_id, display_name, owner_account_id, capabilities)
             VALUES ($1, $2,
                     (SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1),
                     $3, $4, $5)",
        )
        .bind(id)
        .bind(project_id)
        .bind(display_name.trim())
        .bind(owner_account_id)
        .bind(
            serde_json::to_value(capabilities)
                .map_err(|error| sqlx::Error::Encode(Box::new(error)))?,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn create_session(
        &self,
        id: Uuid,
        project_id: Uuid,
        agent_role_id: Uuid,
        lifetime: Duration,
        agent_token: &str,
    ) -> Result<(), Error> {
        if agent_token.is_empty() {
            return Err(PersistenceError::AgentTokenRequired);
        }
        let role_active = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1 FROM agent_roles
                 WHERE id = $1 AND team_id = (
                     SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1
                 ) AND revoked_at IS NULL
             )",
        )
        .bind(agent_role_id)
        .bind(project_id)
        .fetch_one(&self.pool)
        .await?;
        if !role_active {
            return Err(PersistenceError::AgentRoleNotFound);
        }
        sqlx::query(
            "INSERT INTO sessions
             (id, project_id, team_id, agent_role_id, state, max_lifetime_ends_at, agent_token_hash)
             VALUES ($1, $2, (SELECT team_id FROM agent_roles WHERE id = $3),
                     $3, 'active', now() + $4::interval, $5)",
        )
        .bind(id)
        .bind(project_id)
        .bind(agent_role_id)
        .bind(format!("{} seconds", lifetime.num_seconds()))
        .bind(hash_secret(agent_token))
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn authenticate_agent_session(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        agent_token: &str,
    ) -> Result<bool, Error> {
        let authenticated = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                 SELECT 1 FROM sessions s
                 JOIN agent_roles r ON r.id = s.agent_role_id
                 WHERE s.id = $1
                   AND s.team_id = (
                       SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1
                   )
                   AND s.state = 'active'
                   AND s.max_lifetime_ends_at > now()
                   AND s.agent_token_hash = $3
                   AND r.revoked_at IS NULL
             )",
        )
        .bind(session_id)
        .bind(project_id)
        .bind(hash_secret(agent_token))
        .fetch_one(&self.pool)
        .await?;
        Ok(authenticated)
    }

    pub async fn create_issue(&self, issue: models::IssueSeed) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let display_key = Self::allocate_issue_display_key(&mut tx, issue.project_id).await?;
        sqlx::query(
            "INSERT INTO issues (id, project_id, team_id, display_key, title, status, agent_eligible, spec_complete)
             VALUES ($1, $2,
                     (SELECT team_id FROM project_teams WHERE project_id = $2 ORDER BY team_id LIMIT 1),
                     $3, $4, 'todo', $5, $6)",
        )
        .bind(issue.id)
        .bind(issue.project_id)
        .bind(display_key)
        .bind(issue.title)
        .bind(issue.agent_eligible)
        .bind(issue.spec_complete)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO issue_dispatch (issue_id, rank) VALUES ($1, $2)")
            .bind(issue.id)
            .bind(issue.rank)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn ready(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ReadyIssueRecord>, Error> {
        Ok(self
            .ready_snapshot(project_id, session_id, limit)
            .await?
            .issues)
    }

    pub async fn ready_snapshot(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        limit: i64,
    ) -> Result<models::ReadySnapshot, Error> {
        let mut tx = self.pool.begin().await?;
        let session = self.session(&mut *tx, project_id, session_id).await?;
        ensure_session_active(&session)?;

        let issues = sqlx::query_as::<_, ReadyIssueRecord>(
            "SELECT i.id, i.display_key, i.title, i.status, d.rank, d.rank_scope, d.dispatch_version
             FROM issues i
             JOIN issue_dispatch d ON d.issue_id = i.id
             WHERE i.project_id = $1
               AND i.status = 'todo'
               AND i.agent_eligible
               AND i.spec_complete
               AND (
                   NOT EXISTS (
                       SELECT 1 FROM document_bindings b
                       WHERE b.resource_kind = 'issue'
                         AND b.resource_id = i.id
                       AND b.role = 'description'
                   )
                   OR NOT EXISTS (
                       SELECT 1
                       FROM document_bindings b
                       JOIN document_loro_snapshots s ON s.document_id = b.document_id
                       WHERE b.resource_kind = 'issue'
                         AND b.resource_id = i.id
                         AND b.role = 'description'
                   )
                   OR (
                       i.spec_reviewed_frontiers IS NOT NULL
                       AND i.spec_reviewed_frontiers IS NOT DISTINCT FROM (
                           SELECT s.frontiers
                           FROM document_bindings b
                           JOIN document_loro_snapshots s ON s.document_id = b.document_id
                           WHERE b.resource_kind = 'issue'
                             AND b.resource_id = i.id
                             AND b.role = 'description'
                           LIMIT 1
                       )
                   )
               )
               AND d.unresolved_blocker_count = 0
               AND d.active_hold_count = 0
               AND d.active_lease_id IS NULL
             ORDER BY d.rank, i.id
             LIMIT $2",
        )
        .bind(project_id)
        .bind(limit.clamp(1, 100))
        .fetch_all(&mut *tx)
        .await?;

        #[derive(Debug, sqlx::FromRow)]
        struct CandidateRow {
            id: Uuid,
            display_key: String,
            status: String,
            agent_eligible: bool,
            spec_complete: bool,
            specification_changed_since_review: bool,
            unresolved_blocker_count: i32,
            active_hold_count: i32,
            active_lease_id: Option<Uuid>,
        }
        let candidates = sqlx::query_as::<_, CandidateRow>(
            "SELECT i.id, i.display_key, i.status, i.agent_eligible, i.spec_complete,
                    CASE
                        WHEN NOT EXISTS (
                            SELECT 1 FROM document_bindings b
                            WHERE b.resource_kind = 'issue'
                              AND b.resource_id = i.id
                              AND b.role = 'description'
                        ) THEN false
                        WHEN NOT EXISTS (
                            SELECT 1
                            FROM document_bindings b
                            JOIN document_loro_snapshots s ON s.document_id = b.document_id
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
                    d.unresolved_blocker_count, d.active_hold_count, d.active_lease_id
             FROM issues i
             JOIN issue_dispatch d ON d.issue_id = i.id
             WHERE i.project_id = $1
             ORDER BY d.rank, i.id",
        )
        .bind(project_id)
        .fetch_all(&mut *tx)
        .await?;
        let snapshot_version = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(max(dispatch_version), 0) FROM issue_dispatch d
             JOIN issues i ON i.id = d.issue_id WHERE i.project_id = $1",
        )
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        let exclusions = candidates
            .into_iter()
            .filter_map(|candidate| {
                let mut reasons = Vec::new();
                if candidate.status != "todo" {
                    reasons.push(format!("status:{0}", candidate.status));
                }
                if !candidate.agent_eligible {
                    reasons.push("agent_ineligible".to_owned());
                }
                if !candidate.spec_complete {
                    reasons.push("missing_specification".to_owned());
                }
                if candidate.specification_changed_since_review {
                    reasons.push("specification_changed_since_review".to_owned());
                }
                if candidate.unresolved_blocker_count > 0 {
                    reasons.push("blocked".to_owned());
                }
                if candidate.active_hold_count > 0 {
                    reasons.push("held".to_owned());
                }
                if candidate.active_lease_id.is_some() {
                    reasons.push("claimed".to_owned());
                }
                if reasons.is_empty() {
                    None
                } else {
                    Some(models::ReadyExclusionRecord {
                        id: candidate.id,
                        display_key: candidate.display_key,
                        reasons,
                    })
                }
            })
            .collect();
        tx.commit().await?;
        Ok(models::ReadySnapshot {
            snapshot_cursor: format!("project-dispatch-v{snapshot_version}"),
            issues,
            exclusions,
        })
    }

    pub async fn claim(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
        requested_ttl: Duration,
        idempotency_key: &str,
    ) -> Result<ClaimRecord, Error> {
        if idempotency_key.trim().is_empty() {
            return Err(PersistenceError::IdempotencyKeyRequired);
        }
        let mut tx = self.pool.begin().await?;
        let session = self.session(&mut *tx, project_id, session_id).await?;
        ensure_session_active(&session)?;
        let request_hash = claim_request_hash(issue_id, requested_ttl);
        if let Some(existing) = sqlx::query_as::<_, IdempotencyRow>(
            "SELECT request_hash, response FROM idempotency_records
             WHERE project_id = $1 AND actor_id = $2 AND operation = 'claim' AND idempotency_key = $3
             FOR UPDATE",
        )
        .bind(project_id)
        .bind(session_id)
        .bind(idempotency_key)
        .fetch_optional(&mut *tx)
        .await?
        {
            if existing.request_hash != request_hash {
                return Err(PersistenceError::IdempotencyConflict);
            }
            let claim = serde_json::from_value(existing.response)
                .map_err(|error| sqlx::Error::Decode(Box::new(error)))?;
            tx.commit().await?;
            return Ok(claim);
        }

        let issue_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM issues WHERE id = $1 AND project_id = $2)",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !issue_exists {
            return Err(PersistenceError::IssueNotFound);
        }

        sqlx::query("SELECT id FROM issues WHERE id = $1 AND project_id = $2 FOR UPDATE")
            .bind(issue_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        let dispatch = sqlx::query_as::<_, DispatchRow>(
            "SELECT issue_id, active_lease_id, fencing_token
             FROM issue_dispatch WHERE issue_id = $1 FOR UPDATE",
        )
        .bind(issue_id)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(active_lease_id) = dispatch.active_lease_id {
            let expired = sqlx::query_scalar::<_, bool>(
                "SELECT expires_at <= now() FROM leases WHERE id = $1 AND state = 'active'",
            )
            .bind(active_lease_id)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(false);

            if expired {
                sqlx::query(
                    "UPDATE leases SET state = 'expired', release_reason = 'lease_expired'
                     WHERE id = $1 AND state = 'active'",
                )
                .bind(active_lease_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE issues SET status = 'todo', version = version + 1, updated_at = now()
                     WHERE id = $1 AND status = 'in_progress'",
                )
                .bind(issue_id)
                .execute(&mut *tx)
                .await?;
                sqlx::query(
                    "UPDATE issue_dispatch SET active_lease_id = NULL, dispatch_version = dispatch_version + 1,
                     updated_at = now() WHERE issue_id = $1",
                )
                .bind(issue_id)
                .execute(&mut *tx)
                .await?;
            } else {
                return Err(PersistenceError::Contended);
            }
        }

        let eligible = sqlx::query_scalar::<_, bool>(
            "SELECT i.status = 'todo' AND i.agent_eligible AND i.spec_complete
                    AND (
                        NOT EXISTS (
                            SELECT 1 FROM document_bindings b
                            WHERE b.resource_kind = 'issue'
                              AND b.resource_id = i.id
                            AND b.role = 'description'
                        )
                        OR NOT EXISTS (
                            SELECT 1
                            FROM document_bindings b
                            JOIN document_loro_snapshots s ON s.document_id = b.document_id
                            WHERE b.resource_kind = 'issue'
                              AND b.resource_id = i.id
                              AND b.role = 'description'
                        )
                        OR (
                            i.spec_reviewed_frontiers IS NOT NULL
                            AND i.spec_reviewed_frontiers IS NOT DISTINCT FROM (
                                SELECT s.frontiers
                                FROM document_bindings b
                                JOIN document_loro_snapshots s ON s.document_id = b.document_id
                                WHERE b.resource_kind = 'issue'
                                  AND b.resource_id = i.id
                                  AND b.role = 'description'
                                LIMIT 1
                            )
                        )
                    )
                    AND d.unresolved_blocker_count = 0 AND d.active_hold_count = 0
             FROM issues i JOIN issue_dispatch d ON d.issue_id = i.id
             WHERE i.id = $1 AND i.project_id = $2",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_one(&mut *tx)
        .await?;
        if !eligible {
            return Err(PersistenceError::IssueNotEligible);
        }

        let lease_id = Uuid::now_v7();
        let fencing_token = dispatch.fencing_token + 1;
        let ttl = requested_ttl.clamp(Duration::seconds(1), Duration::hours(1));
        let expires_at = Utc::now() + ttl;
        sqlx::query(
            "INSERT INTO leases (id, issue_id, owner_session_id, fencing_token, state, expires_at)
             VALUES ($1, $2, $3, $4, 'active', $5)",
        )
        .bind(lease_id)
        .bind(issue_id)
        .bind(session_id)
        .bind(fencing_token)
        .bind(expires_at)
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
            "UPDATE issue_dispatch SET active_lease_id = $1, fencing_token = $2,
             dispatch_version = dispatch_version + 1, updated_at = now() WHERE issue_id = $3",
        )
        .bind(lease_id)
        .bind(fencing_token)
        .bind(issue_id)
        .execute(&mut *tx)
        .await?;
        insert_audit(
            &mut tx,
            project_id,
            session_id,
            session.agent_role_id,
            "claim",
            issue_id,
        )
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "lease_changed",
            serde_json::json!({
                "issue_id": issue_id,
                "lease_id": lease_id,
                "event": "claimed"
            }),
        )
        .await?;
        let claim = ClaimRecord {
            issue_id,
            lease_id,
            fencing_token,
            expires_at,
        };
        sqlx::query(
            "INSERT INTO idempotency_records
             (project_id, actor_id, operation, idempotency_key, request_hash, response)
             VALUES ($1, $2, 'claim', $3, $4, $5)",
        )
        .bind(project_id)
        .bind(session_id)
        .bind(idempotency_key)
        .bind(request_hash)
        .bind(serde_json::to_value(&claim).map_err(|error| sqlx::Error::Encode(Box::new(error)))?)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(claim)
    }

    pub async fn renew(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        lease_id: Uuid,
        fencing_token: i64,
        requested_ttl: Duration,
    ) -> Result<DateTime<Utc>, Error> {
        let mut tx = self.pool.begin().await?;
        let session = self.session(&mut *tx, project_id, session_id).await?;
        ensure_session_active(&session)?;
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
        if lease.owner_session_id != session_id || lease.fencing_token != fencing_token {
            return Err(PersistenceError::StaleLease);
        }
        if lease.state != "active" || lease.expires_at <= Utc::now() {
            return Err(PersistenceError::LeaseNotActive);
        }
        let max_expiry = session.max_lifetime_ends_at;
        let requested_expiry =
            Utc::now() + requested_ttl.clamp(Duration::seconds(1), Duration::hours(1));
        let expires_at = requested_expiry.min(max_expiry);
        sqlx::query("UPDATE leases SET heartbeat_at = now(), expires_at = $1 WHERE id = $2")
            .bind(expires_at)
            .bind(lease_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE sessions SET heartbeat_at = now(), last_action_at = now() WHERE id = $1",
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(expires_at)
    }

    pub async fn report(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        lease_id: Uuid,
        fencing_token: i64,
        input: ReportInput,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        let session = self.session(&mut *tx, project_id, session_id).await?;
        ensure_session_active(&session)?;
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
        if lease.owner_session_id != session_id || lease.fencing_token != fencing_token {
            return Err(PersistenceError::StaleLease);
        }
        if lease.state != "active" || lease.expires_at <= Utc::now() {
            return Err(PersistenceError::LeaseNotActive);
        }
        let action = input.action;
        if matches!(action, ReportAction::Complete)
            && input
                .resolution_summary
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
        {
            return Err(PersistenceError::ResolutionSummaryRequired);
        }
        if let Some(comment) = input.comment.filter(|body| !body.trim().is_empty()) {
            sqlx::query(
                "INSERT INTO comments (id, project_id, issue_id, author_id, role_id, session_id, body)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(Uuid::now_v7())
            .bind(project_id)
            .bind(lease.issue_id)
            .bind(session_id)
            .bind(session.agent_role_id)
            .bind(session_id)
            .bind(comment)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(summary) = input
            .resolution_summary
            .filter(|body| !body.trim().is_empty())
        {
            sqlx::query(
                "INSERT INTO comments (id, project_id, issue_id, author_id, role_id, session_id, body)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(Uuid::now_v7())
            .bind(project_id)
            .bind(lease.issue_id)
            .bind(session_id)
            .bind(session.agent_role_id)
            .bind(session_id)
            .bind(summary)
            .execute(&mut *tx)
            .await?;
        }
        let (lease_state, issue_status) = match action {
            ReportAction::Release => ("released", "todo"),
            ReportAction::Complete => ("completed", "done"),
        };
        sqlx::query("UPDATE leases SET state = $1, release_reason = $2 WHERE id = $3")
            .bind(lease_state)
            .bind(lease_state)
            .bind(lease_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "UPDATE issues SET status = $1, version = version + 1, updated_at = now(),
             completed_at = CASE WHEN $1 = 'done' THEN now() ELSE completed_at END
             WHERE id = $2",
        )
        .bind(issue_status)
        .bind(lease.issue_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE issue_dispatch SET active_lease_id = NULL, dispatch_version = dispatch_version + 1,
             updated_at = now() WHERE issue_id = $1",
        )
        .bind(lease.issue_id)
        .execute(&mut *tx)
        .await?;
        insert_audit(
            &mut tx,
            project_id,
            session_id,
            session.agent_role_id,
            lease_state,
            lease.issue_id,
        )
        .await?;
        insert_outbox(
            &mut tx,
            project_id,
            "issue_changed",
            serde_json::json!({
                "issue_id": lease.issue_id,
                "lease_id": lease_id,
                "event": lease_state
            }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }
}
