pub mod config;
pub mod documents;
pub mod loro_document;

pub use documents::{tiptap_document_references, tiptap_plain_text, tiptap_sanitized_html};

use chrono::Duration;
use riichi_persistence::{
    ApprovalRequest, Claim, ContextResponse, Database, Error, HumanQueueIssue, IssueCreate,
    IssueEdge, IssueRecord, IssueUpdate, ReadyIssue, ReadySnapshot, RecoveryChecklist, Report,
    ReportBatch, ReportBatchResult,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone)]
pub struct Application {
    database: Database,
}

pub struct AgentDocumentRead {
    pub document: riichi_persistence::Document,
    pub version: riichi_persistence::DocumentVersion,
    pub frontiers: Vec<loro_document::LoroFrontier>,
}

pub struct AgentDocumentInsertText {
    pub project_id: Uuid,
    pub session_id: Uuid,
    pub document_id: Uuid,
    pub idempotency_key: String,
    pub previous_frontiers: Vec<loro_document::LoroFrontier>,
    pub node_path: Vec<usize>,
    pub offset: usize,
    pub text: String,
}

impl Application {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn database(&self) -> Database {
        self.database.clone()
    }

    pub async fn human_queue(
        &self,
        project_id: Uuid,
        limit: i64,
    ) -> Result<Vec<HumanQueueIssue>, Error> {
        self.database.human_queue(project_id, limit).await
    }

    pub async fn create_issue(
        &self,
        project_id: Uuid,
        issue: IssueCreate,
        actor_id: Uuid,
    ) -> Result<IssueRecord, Error> {
        let issue_id = self
            .database
            .create_issue_with_metadata(project_id, issue, actor_id)
            .await?;
        self.database.get_issue(project_id, issue_id).await
    }

    pub async fn get_issue(&self, project_id: Uuid, issue_id: Uuid) -> Result<IssueRecord, Error> {
        self.database.get_issue(project_id, issue_id).await
    }

    pub async fn context(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
        max_bytes: Option<usize>,
        requested_frontiers: Option<serde_json::Value>,
    ) -> Result<ContextResponse, Error> {
        self.database
            .context(
                project_id,
                session_id,
                issue_id,
                max_bytes,
                requested_frontiers,
            )
            .await
    }

    pub async fn context_resource(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
        resource: &str,
    ) -> Result<riichi_persistence::ContextSection, Error> {
        self.database
            .context_resource(project_id, session_id, issue_id, resource)
            .await
    }

    pub async fn update_issue(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        update: IssueUpdate,
        actor_id: Uuid,
    ) -> Result<IssueRecord, Error> {
        Ok(self
            .update_issue_with_transaction(project_id, issue_id, update, actor_id)
            .await?
            .issue)
    }

    pub async fn update_issue_with_transaction(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        update: IssueUpdate,
        actor_id: Uuid,
    ) -> Result<riichi_persistence::IssueUpdateResult, Error> {
        self.database
            .update_issue_with_transaction(project_id, issue_id, update, actor_id)
            .await
    }

    pub async fn create_issue_edge(
        &self,
        project_id: Uuid,
        source_issue_id: Uuid,
        target_issue_id: Uuid,
        edge_type: &str,
        actor_id: Uuid,
    ) -> Result<IssueEdge, Error> {
        let edge_id = self
            .database
            .create_issue_edge(
                project_id,
                source_issue_id,
                target_issue_id,
                edge_type,
                actor_id,
            )
            .await?;
        self.database
            .get_issue(project_id, source_issue_id)
            .await?
            .edges
            .into_iter()
            .find(|edge| edge.id == edge_id)
            .ok_or(Error::EdgeNotFound)
    }

    pub async fn remove_issue_edge(
        &self,
        project_id: Uuid,
        edge_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        self.database
            .remove_issue_edge(project_id, edge_id, actor_id)
            .await
    }

    pub async fn create_hold(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        hold_type: &str,
        reason: &str,
        actor_id: Uuid,
        lifetime: Option<Duration>,
    ) -> Result<IssueRecord, Error> {
        self.database
            .create_hold(project_id, issue_id, hold_type, reason, actor_id, lifetime)
            .await?;
        self.database.get_issue(project_id, issue_id).await
    }

    pub async fn release_hold(
        &self,
        project_id: Uuid,
        hold_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        self.database
            .release_hold(project_id, hold_id, actor_id)
            .await
    }

    pub async fn ready(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        limit: i64,
    ) -> Result<Vec<ReadyIssue>, Error> {
        self.database.ready(project_id, session_id, limit).await
    }

    pub async fn ready_snapshot(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        limit: i64,
    ) -> Result<ReadySnapshot, Error> {
        self.database
            .ready_snapshot(project_id, session_id, limit)
            .await
    }

    pub async fn claim(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
        requested_ttl: Duration,
        idempotency_key: &str,
    ) -> Result<Claim, Error> {
        self.database
            .claim(
                project_id,
                session_id,
                issue_id,
                requested_ttl,
                idempotency_key,
            )
            .await
    }

    pub async fn renew(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        lease_id: Uuid,
        fencing_token: i64,
        requested_ttl: Duration,
    ) -> Result<chrono::DateTime<chrono::Utc>, Error> {
        self.database
            .renew(
                project_id,
                session_id,
                lease_id,
                fencing_token,
                requested_ttl,
            )
            .await
    }

    pub async fn report(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        lease_id: Uuid,
        fencing_token: i64,
        input: Report,
    ) -> Result<(), Error> {
        self.database
            .report(project_id, session_id, lease_id, fencing_token, input)
            .await
    }

    pub async fn report_batch(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        lease_id: Uuid,
        fencing_token: i64,
        batch: ReportBatch,
    ) -> Result<ReportBatchResult, Error> {
        self.database
            .report_batch(project_id, session_id, lease_id, fencing_token, batch)
            .await
    }

    pub async fn takeover_issue(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        actor_id: Uuid,
        reason: &str,
    ) -> Result<RecoveryChecklist, Error> {
        self.database
            .takeover_issue(project_id, issue_id, actor_id, reason)
            .await
    }

    pub async fn complete_recovery(
        &self,
        project_id: Uuid,
        checklist_id: Uuid,
        actor_id: Uuid,
        expected_version: i64,
        action: &str,
        resolution_summary: Option<&str>,
    ) -> Result<IssueRecord, Error> {
        self.database
            .complete_recovery(
                project_id,
                checklist_id,
                actor_id,
                expected_version,
                action,
                resolution_summary,
            )
            .await
    }

    pub async fn create_approval_request(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
        requested_by: Uuid,
        target_version: i64,
        proposed_operation: serde_json::Value,
        lifetime: Duration,
    ) -> Result<ApprovalRequest, Error> {
        self.database
            .create_approval_request(
                project_id,
                issue_id,
                requested_by,
                target_version,
                proposed_operation,
                lifetime,
            )
            .await
    }

    pub async fn decide_approval_request(
        &self,
        project_id: Uuid,
        approval_id: Uuid,
        actor_id: Uuid,
        approve: bool,
    ) -> Result<ApprovalRequest, Error> {
        self.database
            .decide_approval_request(project_id, approval_id, actor_id, approve)
            .await
    }

    pub async fn record_github_delivery(
        &self,
        delivery_id: &str,
        project_id: Option<Uuid>,
        event_type: &str,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<bool, Error> {
        self.database
            .record_github_delivery(delivery_id, project_id, event_type, action, payload)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_github_snapshot(
        &self,
        project_id: Uuid,
        issue_id: Option<Uuid>,
        repository: &str,
        external_number: i64,
        url: &str,
        title: &str,
        body: Option<&str>,
        state: &str,
        external_updated_at: Option<&str>,
        payload: serde_json::Value,
    ) -> Result<riichi_persistence::ExternalIssueRecord, Error> {
        self.database
            .upsert_github_snapshot(
                project_id,
                issue_id,
                repository,
                external_number,
                url,
                title,
                body,
                state,
                external_updated_at,
                payload,
            )
            .await
    }

    pub async fn revoke_agent_session(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        self.database
            .revoke_agent_session(project_id, session_id, actor_id)
            .await
    }

    pub async fn revoke_agent_role(
        &self,
        project_id: Uuid,
        role_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), Error> {
        self.database
            .revoke_agent_role(project_id, role_id, actor_id)
            .await
    }

    pub async fn agent_roster(
        &self,
        project_id: Uuid,
    ) -> Result<
        (
            Vec<riichi_persistence::AgentRole>,
            Vec<riichi_persistence::AgentSession>,
        ),
        Error,
    > {
        Ok((
            self.database.agent_roster(project_id).await?,
            self.database.agent_sessions(project_id, None).await?,
        ))
    }

    pub async fn team_agent_roster(
        &self,
        team_id: Uuid,
    ) -> Result<
        (
            Vec<riichi_persistence::AgentRole>,
            Vec<riichi_persistence::AgentSession>,
        ),
        Error,
    > {
        Ok((
            self.database.agent_roster_for_team(team_id).await?,
            self.database.agent_sessions_for_team(team_id, None).await?,
        ))
    }

    pub async fn create_agent_role(
        &self,
        project_id: Uuid,
        display_name: &str,
        owner_account_id: Uuid,
        capabilities: Vec<String>,
    ) -> Result<(), Error> {
        self.database
            .create_agent_role_with_policy(
                Uuid::now_v7(),
                project_id,
                display_name,
                owner_account_id,
                capabilities,
            )
            .await
    }

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
        expires_after: Option<chrono::Duration>,
    ) -> Result<(), Error> {
        self.database
            .grant_lease_collaborator(
                project_id,
                issue_id,
                lease_id,
                session_id,
                capability,
                grant_mode,
                granted_by,
                expires_after,
            )
            .await
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
        self.database
            .revoke_lease_collaborator(
                project_id, issue_id, lease_id, session_id, capability, revoked_by,
            )
            .await
    }

    pub async fn lease_collaborators(
        &self,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<Vec<riichi_persistence::LeaseCollaborator>, Error> {
        self.database
            .lease_collaborators(project_id, issue_id)
            .await
    }

    pub async fn quarantined_attempts_for_agent(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        issue_id: Uuid,
    ) -> Result<Vec<riichi_persistence::QuarantinedAttempt>, Error> {
        self.database
            .quarantined_attempts_for_agent(project_id, session_id, issue_id)
            .await
    }

    pub async fn create_document(
        &self,
        input: riichi_persistence::DocumentCreate,
    ) -> Result<riichi_persistence::Document, Error> {
        self.database.create_document(input).await
    }

    pub async fn get_document(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<riichi_persistence::Document, Error> {
        self.database.get_document(account_id, document_id).await
    }

    pub async fn document_is_accessible(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<bool, Error> {
        self.database
            .document_is_accessible(account_id, document_id)
            .await
    }

    pub async fn provision_document(&self, document_id: Uuid) -> Result<(), Error> {
        let document = self
            .database
            .get_document_for_provision(document_id)
            .await?;
        if document.provisioning_state == "deleted" {
            return Ok(());
        }
        self.get_loro_snapshot(document.created_by, document_id, None)
            .await?;
        self.database.mark_document_ready(document_id).await
    }

    pub async fn project_document(&self, document_id: Uuid) -> Result<(), Error> {
        let document = self
            .database
            .get_document_for_provision(document_id)
            .await?;
        if document.provisioning_state == "deleted" {
            return Ok(());
        }
        let snapshot = self
            .get_loro_snapshot(document.created_by, document_id, None)
            .await?;
        let loro = loro_document::LoroDocument::from_snapshot_for_schema(
            document_id,
            &snapshot.bytes,
            snapshot.schema_version,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        self.database
            .refresh_document_projection(
                document_id,
                &loro
                    .plain_text_for_schema(snapshot.schema_version)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                &loro
                    .sanitized_html_for_schema(snapshot.schema_version)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
            )
            .await
    }

    pub async fn compact_document(&self, document_id: Uuid) -> Result<(), Error> {
        let document = self
            .database
            .get_document_for_provision(document_id)
            .await?;
        if document.provisioning_state == "deleted" {
            return Ok(());
        }
        let snapshot = self
            .get_loro_snapshot(document.created_by, document_id, None)
            .await?;
        let loro = loro_document::LoroDocument::from_snapshot_for_schema(
            document_id,
            &snapshot.bytes,
            snapshot.schema_version,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let plain_text = loro
            .plain_text_for_schema(snapshot.schema_version)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let sanitized_html = loro
            .sanitized_html_for_schema(snapshot.schema_version)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        self.database
            .compact_loro_document(
                document_id,
                serde_json::to_value(&snapshot.frontiers)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                loro.export_snapshot()
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                &plain_text,
                &sanitized_html,
            )
            .await
    }

    pub async fn migrate_document_to_v2(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<loro_document::LoroSnapshot, Error> {
        let snapshot = self
            .get_loro_snapshot(account_id, document_id, None)
            .await?;
        if snapshot.schema_version == loro_document::DOCUMENT_SCHEMA_V2 {
            return Ok(snapshot);
        }
        if snapshot.schema_version != loro_document::DOCUMENT_SCHEMA_V1 {
            return Err(Error::InvalidDocument(
                "document schema cannot be migrated".to_owned(),
            ));
        }
        let v1 = loro_document::LoroDocument::from_snapshot_for_schema(
            document_id,
            &snapshot.bytes,
            loro_document::DOCUMENT_SCHEMA_V1,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let v1_content = v1
            .to_tiptap_for_schema(loro_document::DOCUMENT_SCHEMA_V1)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let content = loro_document::migrate_v1_to_v2(&v1_content)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let v2 = loro_document::LoroDocument::from_tiptap_for_schema(
            document_id,
            &content,
            loro_document::DOCUMENT_SCHEMA_V2,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let frontiers = v2.frontiers();
        let frontiers_value = serde_json::to_value(&frontiers)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let persisted = self
            .database
            .migrate_loro_document_schema(riichi_persistence::LoroSchemaMigration {
                account_id,
                document_id,
                expected_schema_version: loro_document::DOCUMENT_SCHEMA_V1,
                expected_frontiers: serde_json::to_value(&snapshot.frontiers)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                target_schema_version: loro_document::DOCUMENT_SCHEMA_V2,
                snapshot: v2
                    .export_snapshot()
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                frontiers: frontiers_value,
                content: content.clone(),
                plain_text: tiptap_plain_text(&content),
                sanitized_html: tiptap_sanitized_html(&content)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                references: tiptap_document_references(&content),
                archive_reason: "schema_migration".to_owned(),
            })
            .await?;
        loro_snapshot_result(persisted)
    }

    pub async fn rollback_document_to_v1(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<loro_document::LoroSnapshot, Error> {
        let current = self
            .get_loro_snapshot(account_id, document_id, None)
            .await?;
        if current.schema_version != loro_document::DOCUMENT_SCHEMA_V2 {
            return Err(Error::InvalidDocument(
                "document is not using schema version 2".to_owned(),
            ));
        }
        let history = self
            .database
            .get_latest_loro_snapshot_history(account_id, document_id)
            .await?
            .ok_or_else(|| {
                Error::InvalidDocument("document has no schema migration history".to_owned())
            })?;
        if history.schema_version != loro_document::DOCUMENT_SCHEMA_V1 {
            return Err(Error::InvalidDocument(
                "document has no v1 snapshot to restore".to_owned(),
            ));
        }
        let v1 = loro_document::LoroDocument::from_snapshot_for_schema(
            document_id,
            &history.snapshot,
            loro_document::DOCUMENT_SCHEMA_V1,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let content = v1
            .to_tiptap_for_schema(loro_document::DOCUMENT_SCHEMA_V1)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let persisted = self
            .database
            .migrate_loro_document_schema(riichi_persistence::LoroSchemaMigration {
                account_id,
                document_id,
                expected_schema_version: loro_document::DOCUMENT_SCHEMA_V2,
                expected_frontiers: serde_json::to_value(&current.frontiers)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                target_schema_version: loro_document::DOCUMENT_SCHEMA_V1,
                snapshot: history.snapshot,
                frontiers: history.frontiers,
                content: content.clone(),
                plain_text: tiptap_plain_text(&content),
                sanitized_html: tiptap_sanitized_html(&content)
                    .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                references: tiptap_document_references(&content),
                archive_reason: "schema_rollback".to_owned(),
            })
            .await?;
        loro_snapshot_result(persisted)
    }

    pub async fn get_issue_description_document(
        &self,
        account_id: Uuid,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<riichi_persistence::Document, Error> {
        self.database
            .get_issue_description_document(account_id, project_id, issue_id)
            .await
    }

    pub async fn agent_document_read(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        document_id: Uuid,
    ) -> Result<AgentDocumentRead, Error> {
        let account_id = self
            .database
            .agent_document_account(project_id, session_id, document_id, "doc.read")
            .await?;
        let document = self.database.get_document(account_id, document_id).await?;
        let version = self
            .database
            .get_document_version(account_id, document_id, None)
            .await?;
        let snapshot = self
            .get_loro_snapshot(account_id, document_id, None)
            .await?;
        Ok(AgentDocumentRead {
            document,
            version,
            frontiers: snapshot.frontiers,
        })
    }

    pub async fn agent_document_apply_insert_text(
        &self,
        input: AgentDocumentInsertText,
    ) -> Result<loro_document::LoroUpdateResult, Error> {
        let account_id = self
            .database
            .agent_document_account(
                input.project_id,
                input.session_id,
                input.document_id,
                "doc.apply_edit",
            )
            .await?;
        if let Some(existing) = self
            .database
            .get_loro_update(
                account_id,
                input.document_id,
                None,
                Some(&input.idempotency_key),
            )
            .await?
        {
            return loro_update_result(existing, true);
        }
        let snapshot = self
            .get_loro_snapshot(account_id, input.document_id, None)
            .await?;
        if snapshot.frontiers != input.previous_frontiers {
            return Err(Error::LoroFrontierConflict);
        }
        let loro = loro_document::LoroDocument::from_snapshot_for_schema(
            input.document_id,
            &snapshot.bytes,
            snapshot.schema_version,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let version = loro.version_vector();
        let edited = loro;
        edited
            .insert_text(&input.node_path, input.offset, &input.text)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let payload = edited
            .export_updates_since(&version)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        if payload.is_empty() {
            return Err(Error::InvalidDocument("edit produced no update".to_owned()));
        }
        self.accept_loro_update(
            account_id,
            input.document_id,
            loro_document::LoroUpdateCommand {
                schema_version: snapshot.schema_version,
                update_id: Uuid::now_v7(),
                idempotency_key: Some(input.idempotency_key),
                previous_frontiers: input.previous_frontiers,
                payload,
                source: "agent".to_owned(),
            },
        )
        .await
    }

    pub async fn list_document_children(
        &self,
        account_id: Uuid,
        parent_document_id: Option<Uuid>,
        organization_id: Uuid,
        owner_team_id: Option<Uuid>,
        owner_project_id: Option<Uuid>,
    ) -> Result<Vec<riichi_persistence::Document>, Error> {
        self.database
            .list_child_documents(
                account_id,
                parent_document_id,
                organization_id,
                owner_team_id,
                owner_project_id,
            )
            .await
    }

    pub async fn get_document_version(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        revision: Option<i64>,
    ) -> Result<riichi_persistence::DocumentVersion, Error> {
        self.database
            .get_document_version(account_id, document_id, revision)
            .await
    }

    pub async fn get_loro_snapshot(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        revision: Option<i64>,
    ) -> Result<loro_document::LoroSnapshot, Error> {
        if revision.is_none()
            && let Some(snapshot) = self
                .database
                .get_loro_snapshot(account_id, document_id)
                .await?
        {
            return loro_snapshot_result(snapshot);
        }
        let version = self
            .database
            .get_document_version(account_id, document_id, revision)
            .await?;
        ensure_supported_document_schema(version.schema_version)?;
        let loro = loro_document::LoroDocument::from_tiptap_for_schema(
            document_id,
            &version.content,
            version.schema_version,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let bytes = loro
            .export_snapshot()
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        if revision.is_none() {
            let persisted = self
                .database
                .initialize_loro_snapshot(
                    account_id,
                    riichi_persistence::LoroSnapshotSeed {
                        document_id,
                        source_revision: version.revision,
                        schema_version: version.schema_version,
                        frontiers: serde_json::to_value(loro.frontiers())
                            .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                        snapshot: bytes,
                    },
                )
                .await?;
            return loro_snapshot_result(persisted);
        }
        Ok(loro_document::LoroSnapshot {
            document_id,
            revision: version.revision,
            schema_version: version.schema_version,
            frontiers: loro.frontiers(),
            bytes,
        })
    }

    pub async fn accept_loro_update(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        command: loro_document::LoroUpdateCommand,
    ) -> Result<loro_document::LoroUpdateResult, Error> {
        let loro_document::LoroUpdateCommand {
            schema_version,
            update_id,
            idempotency_key,
            previous_frontiers,
            payload,
            source,
        } = command;
        if payload.is_empty() || payload.len() > riichi_persistence::MAX_LORO_UPDATE_BYTES {
            return Err(Error::InvalidDocument(
                "Loro update payload must be between 1 byte and 1 MB".to_owned(),
            ));
        }

        if let Some(existing) = self
            .database
            .get_loro_update(
                account_id,
                document_id,
                Some(update_id),
                idempotency_key.as_deref(),
            )
            .await?
        {
            return loro_update_result(existing, true);
        }

        let snapshot = match self
            .database
            .get_loro_snapshot(account_id, document_id)
            .await?
        {
            Some(snapshot) => snapshot,
            None => {
                let version = self
                    .database
                    .get_document_version(account_id, document_id, None)
                    .await?;
                ensure_supported_document_schema(version.schema_version)?;
                let loro = loro_document::LoroDocument::from_tiptap_for_schema(
                    document_id,
                    &version.content,
                    version.schema_version,
                )
                .map_err(|error| Error::InvalidDocument(error.to_string()))?;
                self.database
                    .initialize_loro_snapshot(
                        account_id,
                        riichi_persistence::LoroSnapshotSeed {
                            document_id,
                            source_revision: version.revision,
                            schema_version: version.schema_version,
                            frontiers: serde_json::to_value(loro.frontiers())
                                .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                            snapshot: loro
                                .export_snapshot()
                                .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                        },
                    )
                    .await?
            }
        };
        if !loro_document::is_supported_document_schema(schema_version)
            || snapshot.schema_version != schema_version
        {
            return Err(Error::InvalidDocument(
                "unsupported document schema version".to_owned(),
            ));
        }
        let current_frontiers: Vec<loro_document::LoroFrontier> =
            serde_json::from_value(snapshot.frontiers.clone())
                .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        if current_frontiers != previous_frontiers {
            return Err(Error::LoroFrontierConflict);
        }
        let mut loro = loro_document::LoroDocument::from_snapshot_for_schema(
            document_id,
            &snapshot.snapshot,
            snapshot.schema_version,
        )
        .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let accepted = loro
            .accept_update_for_schema(
                update_id,
                account_id,
                &source,
                &payload,
                snapshot.schema_version,
            )
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let payload_sha256 = Sha256::digest(&accepted.payload).to_vec();
        let content = loro
            .to_tiptap_for_schema(snapshot.schema_version)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?;
        let references = tiptap_document_references(&content);
        let persisted = self
            .database
            .accept_loro_update(
                account_id,
                riichi_persistence::LoroUpdateSeed {
                    update_id,
                    document_id,
                    principal_id: account_id,
                    source,
                    peer_id: accepted
                        .resulting_frontiers
                        .first()
                        .map(|frontier| frontier.peer_id.to_string())
                        .unwrap_or_else(|| loro.peer_id().to_string()),
                    idempotency_key,
                    previous_frontiers: serde_json::to_value(&accepted.previous_frontiers)
                        .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                    resulting_frontiers: serde_json::to_value(&accepted.resulting_frontiers)
                        .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                    payload: accepted.payload,
                    payload_sha256,
                    snapshot: loro
                        .export_snapshot()
                        .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                    content,
                    plain_text: loro
                        .plain_text_for_schema(snapshot.schema_version)
                        .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                    references,
                    sanitized_html: loro
                        .sanitized_html_for_schema(snapshot.schema_version)
                        .map_err(|error| Error::InvalidDocument(error.to_string()))?,
                },
            )
            .await?;
        loro_update_result(
            persisted.0,
            matches!(persisted.1, riichi_persistence::LoroUpdateOutcome::Replayed),
        )
    }

    pub async fn accept_loro_transport_update(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        mut command: loro_document::LoroUpdateCommand,
    ) -> Result<loro_document::LoroUpdateResult, Error> {
        for attempt in 0..3 {
            let snapshot = self
                .get_loro_snapshot(account_id, document_id, None)
                .await?;
            command.previous_frontiers = snapshot.frontiers;
            match self
                .accept_loro_update(account_id, document_id, command.clone())
                .await
            {
                Err(Error::LoroFrontierConflict) if attempt < 2 => continue,
                result => return result,
            }
        }
        unreachable!("the transport update retry loop always returns")
    }

    pub async fn update_document_content(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        update: riichi_persistence::DocumentContentUpdate,
    ) -> Result<riichi_persistence::Document, Error> {
        self.database
            .update_document_content(account_id, document_id, update)
            .await
    }

    pub async fn update_document_metadata(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        title: String,
        parent_document_id: Option<Uuid>,
        position: i64,
    ) -> Result<riichi_persistence::Document, Error> {
        self.database
            .update_document_metadata(account_id, document_id, title, parent_document_id, position)
            .await
    }

    pub async fn delete_document(&self, account_id: Uuid, document_id: Uuid) -> Result<(), Error> {
        self.database.delete_document(account_id, document_id).await
    }

    pub async fn document_references(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<Vec<riichi_persistence::DocumentReference>, Error> {
        self.database
            .document_references(account_id, document_id)
            .await
    }

    pub async fn document_backlinks(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<Vec<riichi_persistence::DocumentReference>, Error> {
        self.database
            .document_backlinks(account_id, document_id)
            .await
    }

    pub async fn replace_document_references(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        references: &[riichi_persistence::DocumentReferenceInput],
    ) -> Result<Vec<riichi_persistence::DocumentReference>, Error> {
        self.database
            .replace_document_references(account_id, document_id, references)
            .await
    }

    pub async fn create_attachment_upload(
        &self,
        input: riichi_persistence::AttachmentUploadSeed,
    ) -> Result<riichi_persistence::AttachmentUpload, Error> {
        self.database.create_attachment_upload(input).await
    }

    pub async fn complete_attachment_upload(
        &self,
        account_id: Uuid,
        upload_id: Uuid,
        byte_size: i64,
        checksum: &[u8],
    ) -> Result<riichi_persistence::Attachment, Error> {
        self.database
            .complete_attachment_upload(account_id, upload_id, byte_size, checksum)
            .await
    }

    pub async fn authorize_attachment_upload(
        &self,
        account_id: Uuid,
        upload_id: Uuid,
    ) -> Result<(), Error> {
        self.database
            .authorize_attachment_upload(account_id, upload_id)
            .await
    }

    pub async fn get_attachment(
        &self,
        account_id: Uuid,
        attachment_id: Uuid,
    ) -> Result<riichi_persistence::Attachment, Error> {
        self.database
            .get_attachment(account_id, attachment_id)
            .await
    }
}

fn loro_snapshot_result(
    snapshot: riichi_persistence::LoroSnapshotRecord,
) -> Result<loro_document::LoroSnapshot, Error> {
    ensure_supported_document_schema(snapshot.schema_version)?;
    Ok(loro_document::LoroSnapshot {
        document_id: snapshot.document_id,
        revision: snapshot.source_revision,
        schema_version: snapshot.schema_version,
        frontiers: serde_json::from_value(snapshot.frontiers)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?,
        bytes: snapshot.snapshot,
    })
}

fn ensure_supported_document_schema(schema_version: i32) -> Result<(), Error> {
    if !loro_document::is_supported_document_schema(schema_version) {
        return Err(Error::InvalidDocument(
            "unsupported document schema version".to_owned(),
        ));
    }
    Ok(())
}

fn loro_update_result(
    record: riichi_persistence::LoroUpdateRecord,
    replayed: bool,
) -> Result<loro_document::LoroUpdateResult, Error> {
    Ok(loro_document::LoroUpdateResult {
        update_id: record.update_id,
        document_id: record.document_id,
        source: record.source,
        previous_frontiers: serde_json::from_value(record.previous_frontiers)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?,
        resulting_frontiers: serde_json::from_value(record.resulting_frontiers)
            .map_err(|error| Error::InvalidDocument(error.to_string()))?,
        accepted_at: record.accepted_at,
        replayed,
    })
}
