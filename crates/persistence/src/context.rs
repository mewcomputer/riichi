use super::*;
use serde::Serialize;
use sqlx::FromRow;

const MAX_CONTEXT_BYTES: usize = 64 * 1024;
const DEFAULT_CONTEXT_BYTES: usize = 16 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct ContextResponse {
    pub issue_id: Uuid,
    pub snapshot_cursor: String,
    pub max_bytes: usize,
    pub document_frontiers: Option<serde_json::Value>,
    pub document_source_revision: Option<i64>,
    pub document_projection_revision: Option<i64>,
    pub sections: Vec<ContextSection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSection {
    pub name: String,
    pub content: Option<String>,
    pub byte_size: usize,
    pub source_object_ids: Vec<Uuid>,
    pub state_version: i64,
    pub trust_class: String,
    pub omitted: bool,
    pub truncated: bool,
    pub omission_reason: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct ContextComment {
    id: Uuid,
    session_id: Option<Uuid>,
    body: String,
    created_at: DateTime<Utc>,
}

fn truncate_to_bytes(value: &str, budget: usize) -> (String, bool) {
    if value.len() <= budget {
        return (value.to_owned(), false);
    }
    let mut end = budget;
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), true)
}

#[allow(clippy::too_many_arguments)]
fn add_section(
    sections: &mut Vec<ContextSection>,
    remaining: &mut usize,
    name: &str,
    content: Option<&str>,
    source_object_ids: Vec<Uuid>,
    state_version: i64,
    trust_class: &str,
    omission_reason: Option<&str>,
) {
    let Some(content) = content else {
        sections.push(ContextSection {
            name: name.to_owned(),
            content: None,
            byte_size: 0,
            source_object_ids,
            state_version,
            trust_class: trust_class.to_owned(),
            omitted: true,
            truncated: false,
            omission_reason: omission_reason.map(str::to_owned),
        });
        return;
    };
    if *remaining == 0 {
        sections.push(ContextSection {
            name: name.to_owned(),
            content: None,
            byte_size: 0,
            source_object_ids,
            state_version,
            trust_class: trust_class.to_owned(),
            omitted: true,
            truncated: false,
            omission_reason: Some("context budget was exhausted".to_owned()),
        });
        return;
    }
    let (content, truncated) = truncate_to_bytes(content, *remaining);
    let byte_size = content.len();
    *remaining -= byte_size;
    sections.push(ContextSection {
        name: name.to_owned(),
        content: Some(content),
        byte_size,
        source_object_ids,
        state_version,
        trust_class: trust_class.to_owned(),
        omitted: false,
        truncated,
        omission_reason: truncated
            .then_some("section was truncated to fit the context budget".to_owned()),
    });
}

fn build_context(
    issue: &models::IssueRecord,
    comments: &[ContextComment],
    description: &str,
    requested_bytes: usize,
    document_frontiers: Option<serde_json::Value>,
    document_source_revision: Option<i64>,
    document_projection_revision: Option<i64>,
) -> ContextResponse {
    let max_bytes = requested_bytes.clamp(1024, MAX_CONTEXT_BYTES);
    let reserved_metadata = 1024.min(max_bytes.saturating_sub(1));
    let mut remaining = max_bytes.saturating_sub(reserved_metadata);
    let issue_source = vec![issue.id];
    let mut sections = Vec::new();

    add_section(
        &mut sections,
        &mut remaining,
        "identity",
        Some(&format!("{} ({})", issue.display_key, issue.status)),
        issue_source.clone(),
        issue.version,
        "project_content",
        None,
    );
    let prior_attempts = comments
        .iter()
        .filter(|comment| comment.session_id.is_some())
        .map(|comment| {
            format!(
                "{} [{}] {}",
                comment.created_at.to_rfc3339(),
                comment.id,
                comment.body
            )
        })
        .collect::<Vec<_>>();
    add_section(
        &mut sections,
        &mut remaining,
        "prior_attempt",
        Some(&if prior_attempts.is_empty() {
            "none recorded".to_owned()
        } else {
            prior_attempts.join("\n")
        }),
        comments
            .iter()
            .filter(|comment| comment.session_id.is_some())
            .map(|comment| comment.id)
            .collect(),
        issue.version,
        "agent_generated",
        None,
    );
    add_section(
        &mut sections,
        &mut remaining,
        "description",
        Some(&format!("# {}\n\n{}", issue.title, description)),
        issue_source.clone(),
        issue.version,
        "project_content",
        None,
    );
    add_section(
        &mut sections,
        &mut remaining,
        "dispatch",
        Some(&format!(
            "rank={} rank_scope={} agent_eligible={} spec_complete={} blockers={} holds={} lease={}",
            issue.rank,
            issue.rank_scope,
            issue.agent_eligible,
            issue.spec_complete,
            issue.unresolved_blocker_count,
            issue.active_hold_count,
            issue
                .active_lease_id
                .map(|id| id.to_string())
                .as_deref()
                .unwrap_or("none")
        )),
        issue_source.clone(),
        issue.dispatch_version,
        "project_policy",
        None,
    );

    let blockers = issue
        .edges
        .iter()
        .filter(|edge| edge.edge_type == "blocks" && edge.target_issue_id == issue.id)
        .map(|edge| edge.source_issue_id.to_string())
        .collect::<Vec<_>>();
    add_section(
        &mut sections,
        &mut remaining,
        "blockers",
        Some(&if blockers.is_empty() {
            "none".to_owned()
        } else {
            blockers.join("\n")
        }),
        issue.edges.iter().map(|edge| edge.id).collect(),
        issue.version,
        "project_content",
        None,
    );

    let relationships = issue
        .edges
        .iter()
        .map(|edge| {
            format!(
                "{} {} {}",
                edge.source_issue_id, edge.edge_type, edge.target_issue_id
            )
        })
        .collect::<Vec<_>>();
    add_section(
        &mut sections,
        &mut remaining,
        "relationships",
        Some(&if relationships.is_empty() {
            "none".to_owned()
        } else {
            relationships.join("\n")
        }),
        issue.edges.iter().map(|edge| edge.id).collect(),
        issue.version,
        "project_content",
        None,
    );

    add_section(
        &mut sections,
        &mut remaining,
        "external_context",
        None,
        Vec::new(),
        issue.version,
        "external_untrusted",
        Some("no cached external snapshot is available"),
    );

    ContextResponse {
        issue_id: issue.id,
        snapshot_cursor: format!(
            "issue-v{}-dispatch-v{}",
            issue.version, issue.dispatch_version
        ),
        max_bytes,
        document_frontiers,
        document_source_revision,
        document_projection_revision,
        sections,
    }
}

impl Database {
    pub async fn context(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
        max_bytes: Option<usize>,
        requested_frontiers: Option<serde_json::Value>,
    ) -> Result<ContextResponse, Error> {
        let session = self.session(&self.pool, project_id, session_id).await?;
        ensure_session_active(&session)?;
        let issue = self.get_issue(project_id, issue_id).await?;
        let document_state =
            sqlx::query_as::<_, (Option<serde_json::Value>, Option<i64>, Option<i64>)>(
                "SELECT s.frontiers, s.source_revision, p.content_revision
             FROM document_bindings b
             LEFT JOIN document_loro_snapshots s ON s.document_id = b.document_id
             LEFT JOIN document_projections p ON p.document_id = b.document_id
             WHERE b.resource_kind = 'issue'
               AND b.resource_id = $1
               AND b.role = 'description'
             LIMIT 1",
            )
            .bind(issue_id)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or((None, None, None));
        let current_frontiers_requested = requested_frontiers
            .as_ref()
            .is_none_or(|requested| document_state.0.as_ref() == Some(requested));
        if current_frontiers_requested
            && document_state
                .1
                .zip(document_state.2)
                .is_some_and(|(source, projection)| projection < source)
        {
            return Err(PersistenceError::DocumentProjectionPending);
        }
        let (
            description,
            document_frontiers,
            document_source_revision,
            document_projection_revision,
        ) = if current_frontiers_requested {
            (
                sqlx::query_scalar::<_, Option<String>>(
                    "SELECT p.plain_text
                         FROM document_bindings b
                         JOIN document_projections p ON p.document_id = b.document_id
                         WHERE b.resource_kind = 'issue'
                           AND b.resource_id = $1
                           AND b.role = 'description'
                         LIMIT 1",
                )
                .bind(issue_id)
                .fetch_one(&self.pool)
                .await?
                .unwrap_or_else(|| issue.body.clone()),
                document_state.0,
                document_state.1,
                document_state.2,
            )
        } else {
            let requested = requested_frontiers
                .clone()
                .expect("non-current context requires requested frontiers");
            let version = sqlx::query_as::<_, (i64, String, Option<serde_json::Value>)>(
                "SELECT v.revision, v.plain_text, v.frontiers
                     FROM document_bindings b
                     JOIN document_versions v ON v.document_id = b.document_id
                     WHERE b.resource_kind = 'issue'
                       AND b.resource_id = $1
                       AND b.role = 'description'
                       AND v.frontiers = $2
                     ORDER BY v.revision DESC
                     LIMIT 1",
            )
            .bind(issue_id)
            .bind(&requested)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(PersistenceError::DocumentFrontierUnavailable)?;
            (
                version.1,
                version.2.or(Some(requested)),
                Some(version.0),
                Some(version.0),
            )
        };
        let comments = sqlx::query_as::<_, ContextComment>(
            "SELECT id, session_id, body, created_at
             FROM comments
             WHERE project_id = $1 AND issue_id = $2
             ORDER BY created_at DESC, id DESC
             LIMIT 20",
        )
        .bind(project_id)
        .bind(issue_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(build_context(
            &issue,
            &comments,
            &description,
            max_bytes.unwrap_or(DEFAULT_CONTEXT_BYTES),
            document_frontiers,
            document_source_revision,
            document_projection_revision,
        ))
    }

    pub async fn context_resource(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
        resource: &str,
    ) -> Result<ContextSection, Error> {
        let context = self
            .context(
                project_id,
                session_id,
                issue_id,
                Some(MAX_CONTEXT_BYTES),
                None,
            )
            .await?;
        context
            .sections
            .into_iter()
            .find(|section| section.name == resource && !section.omitted)
            .ok_or(PersistenceError::ContextResourceNotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_to_bytes;

    #[test]
    fn truncation_preserves_utf8_and_declares_the_boundary() {
        let (value, truncated) = truncate_to_bytes("猫猫猫", 4);
        assert_eq!(value, "猫");
        assert!(truncated);
    }
}
