use super::*;
use crate::models::{
    AttachmentRecord, AttachmentUploadRecord, DocumentRecord, DocumentReferenceRecord,
    DocumentVersionRecord,
};
use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use sqlx::{Postgres, Transaction};

#[derive(Debug, Clone)]
pub struct DocumentCreate {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub kind: String,
    pub title: String,
    pub parent_document_id: Option<Uuid>,
    pub position: i64,
    pub owner_team_id: Option<Uuid>,
    pub owner_project_id: Option<Uuid>,
    pub created_by: Uuid,
    pub content: Value,
    pub plain_text: String,
    pub sanitized_html: String,
    pub schema_version: i32,
}

#[derive(Debug, Clone)]
pub struct DocumentContentUpdate {
    pub expected_revision: i64,
    pub content: Value,
    pub plain_text: String,
    pub sanitized_html: String,
    pub references: Vec<DocumentReferenceInput>,
}

#[derive(Debug, Clone)]
pub struct DocumentReferenceInput {
    pub source_block_id: String,
    pub resource_kind: String,
    pub resource_id: Uuid,
    pub reference_kind: String,
}

#[derive(Debug, Clone)]
pub struct AttachmentUploadSeed {
    pub id: Uuid,
    pub attachment_id: Uuid,
    pub organization_id: Uuid,
    pub storage_key: String,
    pub filename: String,
    pub media_type: String,
    pub byte_size: i64,
    pub checksum: Vec<u8>,
    pub uploaded_by: Uuid,
    pub document_id: Uuid,
    pub source_block_id: String,
    pub lifetime: Duration,
}

impl Database {
    pub async fn create_document(&self, input: DocumentCreate) -> Result<DocumentRecord, Error> {
        validate_document_input(&input)?;
        if !matches!(input.schema_version, 1 | 2) {
            return Err(Error::InvalidDocument(
                "unsupported document schema version".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        self.require_document_scope_member(
            &mut *tx,
            input.organization_id,
            input.owner_team_id,
            input.owner_project_id,
            input.created_by,
            1,
        )
        .await?;
        self.validate_parent_scope(
            &mut *tx,
            input.id,
            input.organization_id,
            input.parent_document_id,
            input.owner_team_id,
            input.owner_project_id,
        )
        .await?;

        sqlx::query(
            "INSERT INTO documents
             (id, organization_id, kind, title, parent_document_id, position,
              owner_team_id, owner_project_id, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
        )
        .bind(input.id)
        .bind(input.organization_id)
        .bind(&input.kind)
        .bind(&input.title)
        .bind(input.parent_document_id)
        .bind(input.position)
        .bind(input.owner_team_id)
        .bind(input.owner_project_id)
        .bind(input.created_by)
        .execute(&mut *tx)
        .await?;

        if let Some((resource_kind, resource_id)) = input
            .owner_team_id
            .map(|id| ("team", id))
            .or_else(|| input.owner_project_id.map(|id| ("project", id)))
        {
            sqlx::query(
                "INSERT INTO document_bindings
                 (document_id, resource_kind, resource_id, role)
                 VALUES ($1, $2, $3, 'page')",
            )
            .bind(input.id)
            .bind(resource_kind)
            .bind(resource_id)
            .execute(&mut *tx)
            .await?;
        }

        insert_document_version(
            &mut tx,
            input.id,
            1,
            &input.content,
            &input.plain_text,
            &input.sanitized_html,
            input.schema_version,
            input.created_by,
        )
        .await?;
        sqlx::query(
            "INSERT INTO document_projections
             (document_id, content_revision, plain_text, sanitized_html, schema_version)
             VALUES ($1, 1, $2, $3, $4)",
        )
        .bind(input.id)
        .bind(&input.plain_text)
        .bind(&input.sanitized_html)
        .bind(input.schema_version)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.get_document(input.created_by, input.id).await
    }

    pub async fn get_document(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<DocumentRecord, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        sqlx::query_as::<_, DocumentRecord>(
            "SELECT d.id, d.organization_id, d.kind, d.title, d.parent_document_id,
                    d.position, d.owner_team_id, d.owner_project_id,
                    d.provisioning_state, d.created_by, d.created_at, d.updated_at,
                    d.deleted_at, p.content_revision AS current_revision,
                    p.plain_text, p.sanitized_html
             FROM documents d
             LEFT JOIN document_projections p ON p.document_id = d.id
             WHERE d.id = $1 AND d.deleted_at IS NULL",
        )
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::DocumentNotFound)
    }

    pub async fn get_document_for_provision(
        &self,
        document_id: Uuid,
    ) -> Result<DocumentRecord, Error> {
        sqlx::query_as::<_, DocumentRecord>(
            "SELECT d.id, d.organization_id, d.kind, d.title, d.parent_document_id,
                    d.position, d.owner_team_id, d.owner_project_id,
                    d.provisioning_state, d.created_by, d.created_at, d.updated_at,
                    d.deleted_at, p.content_revision AS current_revision,
                    p.plain_text, p.sanitized_html
             FROM documents d
             LEFT JOIN document_projections p ON p.document_id = d.id
             WHERE d.id = $1 AND d.deleted_at IS NULL",
        )
        .bind(document_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::DocumentNotFound)
    }

    pub async fn mark_document_ready(&self, document_id: Uuid) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "UPDATE documents
             SET provisioning_state = 'ready', updated_at = now()
             WHERE id = $1 AND provisioning_state = 'pending' AND deleted_at IS NULL",
        )
        .bind(document_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE issues i
             SET spec_reviewed_frontiers = s.frontiers
             FROM document_bindings b
             JOIN document_loro_snapshots s ON s.document_id = b.document_id
             WHERE b.document_id = $1
               AND b.resource_kind = 'issue'
               AND b.role = 'description'
               AND i.id = b.resource_id
               AND i.spec_complete
               AND i.spec_reviewed_frontiers IS NULL",
        )
        .bind(document_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn mark_document_failed(&self, document_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "UPDATE documents
             SET provisioning_state = 'failed', updated_at = now()
             WHERE id = $1 AND provisioning_state = 'pending' AND deleted_at IS NULL",
        )
        .bind(document_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn refresh_document_projection(
        &self,
        document_id: Uuid,
        plain_text: &str,
        sanitized_html: &str,
    ) -> Result<(), Error> {
        if plain_text.chars().count() > 200_000 || sanitized_html.len() > 500_000 {
            return Err(Error::InvalidDocument(
                "document projection is too large".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let document = lock_document(&mut tx, document_id).await?;
        let revision = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(max(revision), 1)
             FROM document_versions WHERE document_id = $1",
        )
        .bind(document_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_projections
             (document_id, content_revision, plain_text, sanitized_html, schema_version)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (document_id) DO UPDATE SET
               content_revision = EXCLUDED.content_revision,
               plain_text = EXCLUDED.plain_text,
               sanitized_html = EXCLUDED.sanitized_html,
               schema_version = EXCLUDED.schema_version,
               projected_at = now()",
        )
        .bind(document_id)
        .bind(revision)
        .bind(plain_text)
        .bind(sanitized_html)
        .bind(document.schema_version)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE documents SET updated_at = now() WHERE id = $1")
            .bind(document_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_issue_description_document(
        &self,
        account_id: Uuid,
        project_id: Uuid,
        issue_id: Uuid,
    ) -> Result<DocumentRecord, Error> {
        let document_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT b.document_id
             FROM document_bindings b
             JOIN issues i ON i.id = b.resource_id
             WHERE b.resource_kind = 'issue'
               AND b.role = 'description'
               AND b.resource_id = $1
               AND i.project_id = $2",
        )
        .bind(issue_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::DocumentNotFound)?;
        self.get_document(account_id, document_id).await
    }

    pub async fn agent_document_account(
        &self,
        project_id: Uuid,
        session_id: Uuid,
        document_id: Uuid,
        capability: &str,
    ) -> Result<Uuid, Error> {
        let session = self.session(&self.pool, project_id, session_id).await?;
        if session.state != "active" {
            return Err(Error::SessionNotActive);
        }
        let capabilities = session
            .capabilities
            .as_array()
            .ok_or(Error::InvalidCapability)?;
        if !capabilities
            .iter()
            .any(|value| value.as_str() == Some(capability))
        {
            return Err(Error::CapabilityDenied);
        }
        let allowed = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1
                FROM document_bindings b
                JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
                JOIN leases l ON l.issue_id = i.id AND l.state = 'active'
                WHERE b.document_id = $1
                  AND i.project_id = $2
                  AND (
                    l.owner_session_id = $3
                    OR EXISTS (
                        SELECT 1
                        FROM lease_collaborators c
                        WHERE c.lease_id = l.id
                          AND c.session_id = $3
                          AND c.capability = $4
                          AND c.grant_mode = 'auto'
                          AND c.revoked_at IS NULL
                          AND (c.expires_at IS NULL OR c.expires_at > now())
                    )
                  )
            )",
        )
        .bind(document_id)
        .bind(project_id)
        .bind(session_id)
        .bind(capability)
        .fetch_one(&self.pool)
        .await?;
        if !allowed {
            return Err(Error::DocumentAccessDenied);
        }
        session.owner_account_id.ok_or(Error::DocumentAccessDenied)
    }

    pub async fn list_child_documents(
        &self,
        account_id: Uuid,
        parent_document_id: Option<Uuid>,
        organization_id: Uuid,
        owner_team_id: Option<Uuid>,
        owner_project_id: Option<Uuid>,
    ) -> Result<Vec<DocumentRecord>, Error> {
        self.require_document_scope_member(
            &self.pool,
            organization_id,
            owner_team_id,
            owner_project_id,
            account_id,
            0,
        )
        .await?;
        Ok(sqlx::query_as::<_, DocumentRecord>(
            "SELECT d.id, d.organization_id, d.kind, d.title, d.parent_document_id,
                    d.position, d.owner_team_id, d.owner_project_id,
                    d.provisioning_state, d.created_by, d.created_at, d.updated_at,
                    d.deleted_at, p.content_revision AS current_revision,
                    p.plain_text, p.sanitized_html
             FROM documents d
             LEFT JOIN document_projections p ON p.document_id = d.id
             WHERE d.organization_id = $1
               AND d.parent_document_id IS NOT DISTINCT FROM $2
               AND d.owner_team_id IS NOT DISTINCT FROM $3
               AND d.owner_project_id IS NOT DISTINCT FROM $4
               AND d.deleted_at IS NULL
             ORDER BY d.position, d.created_at, d.id",
        )
        .bind(organization_id)
        .bind(parent_document_id)
        .bind(owner_team_id)
        .bind(owner_project_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn get_document_version(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        revision: Option<i64>,
    ) -> Result<DocumentVersionRecord, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        sqlx::query_as::<_, DocumentVersionRecord>(
            "SELECT document_id, revision, content, plain_text, sanitized_html,
                    frontiers, schema_version, created_by, created_at
             FROM document_versions
             WHERE document_id = $1
               AND revision = COALESCE($2, (SELECT max(revision) FROM document_versions WHERE document_id = $1))",
        )
        .bind(document_id)
        .bind(revision)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::DocumentNotFound)
    }

    pub async fn update_document_content(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        update: DocumentContentUpdate,
    ) -> Result<DocumentRecord, Error> {
        if update.plain_text.chars().count() > 200_000
            || update.sanitized_html.len() > 500_000
            || serde_json::to_vec(&update.content)
                .map_err(|_| Error::InvalidDocument("content is not serializable".to_owned()))?
                .len()
                > 1_000_000
        {
            return Err(Error::InvalidDocument(
                "document content is too large".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let document = lock_document(&mut tx, document_id).await?;
        require_document_access_tx(&mut *tx, account_id, document_id, 1).await?;
        if document.current_revision != Some(update.expected_revision) {
            return Err(Error::DocumentVersionConflict);
        }
        let next_revision = update.expected_revision + 1;
        insert_document_version(
            &mut tx,
            document_id,
            next_revision,
            &update.content,
            &update.plain_text,
            &update.sanitized_html,
            document.schema_version,
            account_id,
        )
        .await?;
        sqlx::query(
            "INSERT INTO document_projections
             (document_id, content_revision, plain_text, sanitized_html, schema_version)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (document_id) DO UPDATE SET
               content_revision = EXCLUDED.content_revision,
               plain_text = EXCLUDED.plain_text,
               sanitized_html = EXCLUDED.sanitized_html,
               schema_version = EXCLUDED.schema_version,
               projected_at = now()",
        )
        .bind(document_id)
        .bind(next_revision)
        .bind(&update.plain_text)
        .bind(&update.sanitized_html)
        .bind(document.schema_version)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE documents SET updated_at = now() WHERE id = $1")
            .bind(document_id)
            .execute(&mut *tx)
            .await?;
        replace_document_references(&mut tx, account_id, document_id, &update.references).await?;
        tx.commit().await?;
        self.get_document(account_id, document_id).await
    }

    pub async fn update_document_metadata(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        title: String,
        parent_document_id: Option<Uuid>,
        position: i64,
    ) -> Result<DocumentRecord, Error> {
        if title.trim().is_empty() || title.chars().count() > 240 {
            return Err(Error::InvalidDocument(
                "title must be between 1 and 240 characters".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let document = lock_document(&mut tx, document_id).await?;
        require_document_access_tx(&mut *tx, account_id, document_id, 1).await?;
        self.validate_parent_scope(
            &mut *tx,
            document_id,
            document.organization_id,
            parent_document_id,
            document.owner_team_id,
            document.owner_project_id,
        )
        .await?;
        sqlx::query(
            "UPDATE documents
             SET title = $2, parent_document_id = $3, position = $4, updated_at = now()
             WHERE id = $1",
        )
        .bind(document_id)
        .bind(title)
        .bind(parent_document_id)
        .bind(position)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.get_document(account_id, document_id).await
    }

    pub async fn delete_document(&self, account_id: Uuid, document_id: Uuid) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        lock_document(&mut tx, document_id).await?;
        require_document_access_tx(&mut *tx, account_id, document_id, 1).await?;
        sqlx::query(
            "UPDATE documents
             SET deleted_at = now(), provisioning_state = 'deleted', updated_at = now()
             WHERE id = $1",
        )
        .bind(document_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn archive_document_internal(&self, document_id: Uuid) -> Result<(), Error> {
        sqlx::query(
            "UPDATE documents
             SET deleted_at = now(), provisioning_state = 'archived', updated_at = now()
             WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(document_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete_document_internal(&self, document_id: Uuid) -> Result<(), Error> {
        sqlx::query("DELETE FROM documents WHERE id = $1")
            .bind(document_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn replace_document_references(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        references: &[DocumentReferenceInput],
    ) -> Result<Vec<DocumentReference>, Error> {
        let mut tx = self.pool.begin().await?;
        require_document_access_tx(&mut *tx, account_id, document_id, 1).await?;
        replace_document_references(&mut tx, account_id, document_id, references).await?;
        tx.commit().await?;
        self.document_references(account_id, document_id).await
    }

    pub async fn document_references(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<Vec<DocumentReference>, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        Ok(sqlx::query_as::<_, DocumentReferenceRecord>(
            "SELECT document_id, source_block_id, resource_kind, resource_id,
                    reference_kind, created_at
             FROM document_references
             WHERE document_id = $1
             ORDER BY source_block_id, resource_kind, resource_id",
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn document_backlinks(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<Vec<DocumentReference>, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        let references = sqlx::query_as::<_, DocumentReferenceRecord>(
            "SELECT r.document_id, r.source_block_id, r.resource_kind, r.resource_id,
                    r.reference_kind, r.created_at
             FROM document_references r
             JOIN documents d ON d.id = r.document_id
             WHERE r.resource_kind = 'document'
               AND r.resource_id = $1
               AND d.deleted_at IS NULL
             ORDER BY r.created_at, r.document_id, r.source_block_id",
        )
        .bind(document_id)
        .fetch_all(&self.pool)
        .await?;
        let mut visible = Vec::with_capacity(references.len());
        for reference in references {
            match self.get_document(account_id, reference.document_id).await {
                Ok(_) => visible.push(reference),
                Err(Error::DocumentNotFound | Error::DocumentAccessDenied) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(visible)
    }

    pub async fn create_attachment_upload(
        &self,
        seed: AttachmentUploadSeed,
    ) -> Result<AttachmentUpload, Error> {
        let mut tx = self.pool.begin().await?;
        require_document_access_tx(&mut *tx, seed.uploaded_by, seed.document_id, 1).await?;
        let organization_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT organization_id FROM documents WHERE id = $1 AND deleted_at IS NULL",
        )
        .bind(seed.document_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::DocumentNotFound)?;
        if organization_id != seed.organization_id {
            return Err(Error::DocumentAccessDenied);
        }
        sqlx::query(
            "INSERT INTO attachments
             (id, organization_id, state, storage_key, filename, media_type,
              byte_size, checksum, uploaded_by)
             VALUES ($1, $2, 'pending', $3, $4, $5, $6, $7, $8)",
        )
        .bind(seed.attachment_id)
        .bind(seed.organization_id)
        .bind(&seed.storage_key)
        .bind(&seed.filename)
        .bind(&seed.media_type)
        .bind(seed.byte_size)
        .bind(&seed.checksum)
        .bind(seed.uploaded_by)
        .execute(&mut *tx)
        .await?;
        let upload = sqlx::query_as::<_, AttachmentUploadRecord>(
            "INSERT INTO attachment_uploads
             (id, attachment_id, document_id, source_block_id,
              expected_byte_size, expected_checksum, expires_at)
             VALUES ($1, $2, $3, $4, $5, $6, now() + $7::interval)
             RETURNING id, attachment_id, expected_byte_size, expected_checksum,
                       expires_at, completed_at",
        )
        .bind(seed.id)
        .bind(seed.attachment_id)
        .bind(seed.document_id)
        .bind(&seed.source_block_id)
        .bind(seed.byte_size)
        .bind(&seed.checksum)
        .bind(format!("{} seconds", seed.lifetime.num_seconds().max(1)))
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_jobs
             (id, document_id, job_type, idempotency_key, available_at)
             VALUES ($1, $2, 'attachment_cleanup', $3, $4)
             ON CONFLICT (job_type, idempotency_key) DO NOTHING",
        )
        .bind(Uuid::now_v7())
        .bind(seed.document_id)
        .bind(format!("attachment:{}", seed.attachment_id))
        .bind(upload.expires_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(upload)
    }

    pub async fn complete_attachment_upload(
        &self,
        account_id: Uuid,
        upload_id: Uuid,
        actual_byte_size: i64,
        actual_checksum: &[u8],
    ) -> Result<AttachmentRecord, Error> {
        let mut tx = self.pool.begin().await?;
        let upload = sqlx::query_as::<
            _,
            (
                Uuid,
                Uuid,
                String,
                i64,
                Vec<u8>,
                DateTime<Utc>,
                Option<DateTime<Utc>>,
            ),
        >(
            "SELECT u.attachment_id, u.document_id, u.source_block_id,
                    u.expected_byte_size, u.expected_checksum, u.expires_at,
                    u.completed_at
             FROM attachment_uploads u
             WHERE u.id = $1
             FOR UPDATE",
        )
        .bind(upload_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::AttachmentUploadNotFound)?;
        require_document_access_tx(&mut *tx, account_id, upload.1, 1).await?;
        if upload.5 <= Utc::now() || upload.6.is_some() {
            return Err(Error::AttachmentUploadNotFound);
        }
        if upload.3 != actual_byte_size || upload.4 != actual_checksum {
            return Err(Error::AttachmentVerificationFailed);
        }
        sqlx::query(
            "UPDATE attachments SET state = 'ready', completed_at = now()
             WHERE id = $1 AND state = 'pending'",
        )
        .bind(upload.0)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE attachment_uploads SET completed_at = now() WHERE id = $1")
            .bind(upload_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO document_attachments (document_id, attachment_id, source_block_id)
             VALUES ($1, $2, $3)
             ON CONFLICT DO NOTHING",
        )
        .bind(upload.1)
        .bind(upload.0)
        .bind(upload.2)
        .execute(&mut *tx)
        .await?;
        let attachment = sqlx::query_as::<_, AttachmentRecord>(
            "SELECT id, organization_id, state, storage_key, filename, media_type,
                    byte_size, checksum, uploaded_by, created_at, completed_at, deleted_at
             FROM attachments WHERE id = $1",
        )
        .bind(upload.0)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(attachment)
    }

    pub async fn authorize_attachment_upload(
        &self,
        account_id: Uuid,
        upload_id: Uuid,
    ) -> Result<(), Error> {
        let document_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT document_id FROM attachment_uploads WHERE id = $1",
        )
        .bind(upload_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::AttachmentUploadNotFound)?;
        self.require_document_access(account_id, document_id, 1)
            .await
    }

    pub async fn get_attachment(
        &self,
        account_id: Uuid,
        attachment_id: Uuid,
    ) -> Result<AttachmentRecord, Error> {
        let document_id = sqlx::query_scalar::<_, Uuid>(
            "SELECT document_id FROM document_attachments
             WHERE attachment_id = $1
             ORDER BY document_id
             LIMIT 1",
        )
        .bind(attachment_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::DocumentNotFound)?;
        self.require_document_access(account_id, document_id, 0)
            .await?;
        sqlx::query_as::<_, AttachmentRecord>(
            "SELECT id, organization_id, state, storage_key, filename, media_type,
                    byte_size, checksum, uploaded_by, created_at, completed_at, deleted_at
             FROM attachments
             WHERE id = $1 AND state = 'ready' AND deleted_at IS NULL",
        )
        .bind(attachment_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(Error::AttachmentUploadNotFound)
    }

    pub(crate) async fn require_document_access(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        required_role: i32,
    ) -> Result<(), Error> {
        require_document_access_tx(&self.pool, account_id, document_id, required_role).await
    }

    pub async fn document_is_accessible(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<bool, Error> {
        match self
            .require_document_access(account_id, document_id, 0)
            .await
        {
            Ok(()) => Ok(true),
            Err(Error::DocumentAccessDenied) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn require_document_scope_member<'a, E>(
        &self,
        executor: E,
        organization_id: Uuid,
        owner_team_id: Option<Uuid>,
        owner_project_id: Option<Uuid>,
        account_id: Uuid,
        required_role: i32,
    ) -> Result<(), Error>
    where
        E: sqlx::Executor<'a, Database = Postgres>,
    {
        let allowed = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (
                SELECT 1 FROM organization_memberships om
                WHERE om.organization_id = $1 AND om.account_id = $4
                  AND om.revoked_at IS NULL
                  AND $2::uuid IS NULL AND $3::uuid IS NULL
                  AND CASE om.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $5
            ) OR EXISTS (
                SELECT 1 FROM team_memberships tm
                WHERE tm.team_id = $2 AND tm.account_id = $4
                  AND tm.revoked_at IS NULL
                  AND CASE tm.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $5
            ) OR EXISTS (
                SELECT 1 FROM project_memberships pm
                WHERE pm.project_id = $3 AND pm.account_id = $4
                  AND pm.revoked_at IS NULL
                  AND CASE pm.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $5
            ) OR EXISTS (
                SELECT 1
                FROM project_teams pt
                JOIN team_memberships tm ON tm.team_id = pt.team_id
                JOIN projects p ON p.id = pt.project_id
                WHERE pt.project_id = $3 AND tm.account_id = $4
                  AND tm.revoked_at IS NULL
                  AND EXISTS (
                      SELECT 1 FROM organization_memberships om
                      WHERE om.organization_id = p.organization_id
                        AND om.account_id = $4
                        AND om.revoked_at IS NULL
                  )
                  AND CASE
                      WHEN pt.role = 'admin' AND tm.role IN ('admin', 'owner') THEN 2
                      WHEN pt.role = 'admin' AND tm.role = 'member' THEN 1
                      WHEN pt.role IN ('commenter', 'operator')
                          AND tm.role IN ('member', 'admin', 'owner') THEN 1
                      ELSE 0
                  END >= $5
            )",
        )
        .bind(organization_id)
        .bind(owner_team_id)
        .bind(owner_project_id)
        .bind(account_id)
        .bind(required_role)
        .fetch_one(executor)
        .await?;
        if allowed {
            Ok(())
        } else {
            Err(Error::DocumentAccessDenied)
        }
    }

    async fn validate_parent_scope<'a, E>(
        &self,
        executor: E,
        document_id: Uuid,
        organization_id: Uuid,
        parent_document_id: Option<Uuid>,
        owner_team_id: Option<Uuid>,
        owner_project_id: Option<Uuid>,
    ) -> Result<(), Error>
    where
        E: sqlx::Executor<'a, Database = Postgres>,
    {
        let Some(parent_document_id) = parent_document_id else {
            return Ok(());
        };
        let (valid_scope, invalid_hierarchy) = sqlx::query_as::<_, (bool, bool)>(
            "WITH RECURSIVE ancestors AS (
                SELECT id, parent_document_id, 1 AS depth
                FROM documents
                WHERE id = $1 AND deleted_at IS NULL
                UNION ALL
                SELECT d.id, d.parent_document_id, ancestors.depth + 1
                FROM documents d
                JOIN ancestors ON d.id = ancestors.parent_document_id
                WHERE d.deleted_at IS NULL AND ancestors.depth < 33
            )
            SELECT EXISTS (
                SELECT 1 FROM documents
                WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
                  AND owner_team_id IS NOT DISTINCT FROM $3
                  AND owner_project_id IS NOT DISTINCT FROM $4
            ), (
                EXISTS (SELECT 1 FROM ancestors WHERE id = $5)
                OR COALESCE((SELECT max(depth) FROM ancestors), 0) > 32
            )",
        )
        .bind(parent_document_id)
        .bind(organization_id)
        .bind(owner_team_id)
        .bind(owner_project_id)
        .bind(document_id)
        .fetch_one(executor)
        .await?;
        if !valid_scope {
            Err(Error::InvalidDocument(
                "parent is outside the document scope".to_owned(),
            ))
        } else if invalid_hierarchy {
            Err(Error::InvalidDocument(
                "document hierarchy contains a cycle or exceeds 32 levels".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

async fn lock_document(
    tx: &mut Transaction<'_, Postgres>,
    document_id: Uuid,
) -> Result<LockedDocument, Error> {
    sqlx::query_as::<_, LockedDocument>(
        "SELECT d.organization_id, d.owner_team_id, d.owner_project_id,
                p.content_revision AS current_revision,
                COALESCE(
                    (SELECT s.schema_version
                     FROM document_loro_snapshots s
                     WHERE s.document_id = d.id),
                    p.schema_version,
                    1
                ) AS schema_version
         FROM documents d
         LEFT JOIN document_projections p ON p.document_id = d.id
         WHERE d.id = $1 AND d.deleted_at IS NULL
         FOR UPDATE OF d",
    )
    .bind(document_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(Error::DocumentNotFound)
}

#[derive(Debug, sqlx::FromRow)]
struct LockedDocument {
    organization_id: Uuid,
    owner_team_id: Option<Uuid>,
    owner_project_id: Option<Uuid>,
    current_revision: Option<i64>,
    schema_version: i32,
}

#[allow(clippy::too_many_arguments)]
async fn insert_document_version(
    tx: &mut Transaction<'_, Postgres>,
    document_id: Uuid,
    revision: i64,
    content: &Value,
    plain_text: &str,
    sanitized_html: &str,
    schema_version: i32,
    created_by: Uuid,
) -> Result<(), Error> {
    sqlx::query(
        "INSERT INTO document_versions
         (document_id, revision, content, plain_text, sanitized_html,
          schema_version, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(document_id)
    .bind(revision)
    .bind(content)
    .bind(plain_text)
    .bind(sanitized_html)
    .bind(schema_version)
    .bind(created_by)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub(crate) async fn replace_document_references(
    tx: &mut Transaction<'_, Postgres>,
    account_id: Uuid,
    document_id: Uuid,
    references: &[DocumentReferenceInput],
) -> Result<(), Error> {
    if references.len() > 500 {
        return Err(Error::InvalidDocument(
            "too many document references".to_owned(),
        ));
    }
    sqlx::query("DELETE FROM document_references WHERE document_id = $1")
        .bind(document_id)
        .execute(&mut **tx)
        .await?;
    for reference in references {
        if reference.source_block_id.is_empty() || reference.source_block_id.len() > 128 {
            return Err(Error::InvalidDocument(
                "invalid reference source block".to_owned(),
            ));
        }
        let valid_target = sqlx::query_scalar::<_, bool>(
            "SELECT CASE $2
                WHEN 'issue' THEN EXISTS (
                    SELECT 1 FROM issues i
                    JOIN teams t ON t.id = i.team_id
                    WHERE i.id = $1
                      AND t.organization_id = (SELECT organization_id FROM documents WHERE id = $3)
                      AND (
                          EXISTS (
                              SELECT 1 FROM team_memberships tm
                              WHERE tm.team_id = i.team_id
                                AND tm.account_id = $4
                                AND tm.revoked_at IS NULL
                          )
                          OR EXISTS (
                              SELECT 1 FROM project_memberships pm
                              WHERE pm.project_id = i.project_id
                                AND pm.account_id = $4
                                AND pm.revoked_at IS NULL
                          )
                      )
                )
                WHEN 'team' THEN EXISTS (
                    SELECT 1 FROM teams t
                    WHERE t.id = $1
                      AND t.organization_id = (SELECT organization_id FROM documents WHERE id = $3)
                      AND (
                          EXISTS (
                              SELECT 1 FROM team_memberships tm
                              WHERE tm.team_id = t.id
                                AND tm.account_id = $4
                                AND tm.revoked_at IS NULL
                          )
                      )
                )
                WHEN 'project' THEN EXISTS (
                    SELECT 1 FROM projects p
                    WHERE p.id = $1
                      AND p.organization_id = (SELECT organization_id FROM documents WHERE id = $3)
                      AND (
                          EXISTS (
                              SELECT 1 FROM project_memberships pm
                              WHERE pm.project_id = p.id
                                AND pm.account_id = $4
                                AND pm.revoked_at IS NULL
                          )
                          OR EXISTS (
                              SELECT 1
                              FROM project_teams pt
                              JOIN team_memberships tm ON tm.team_id = pt.team_id
                              WHERE pt.project_id = p.id
                                AND tm.account_id = $4
                                AND tm.revoked_at IS NULL
                          )
                      )
                )
                WHEN 'document' THEN EXISTS (
                    SELECT 1 FROM documents d
                    WHERE d.id = $1 AND d.organization_id = (SELECT organization_id FROM documents WHERE id = $3)
                      AND d.deleted_at IS NULL
                      AND (
                          (
                              d.owner_team_id IS NULL
                              AND d.owner_project_id IS NULL
                              AND EXISTS (
                                  SELECT 1 FROM organization_memberships om
                                  WHERE om.organization_id = d.organization_id
                                    AND om.account_id = $4
                                    AND om.revoked_at IS NULL
                              )
                          )
                          OR (
                              d.owner_team_id IS NOT NULL
                              AND EXISTS (
                                  SELECT 1 FROM team_memberships tm
                                  WHERE tm.team_id = d.owner_team_id
                                    AND tm.account_id = $4
                                    AND tm.revoked_at IS NULL
                              )
                          )
                          OR (
                              d.owner_project_id IS NOT NULL
                              AND EXISTS (
                                  SELECT 1 FROM project_memberships pm
                                  WHERE pm.project_id = d.owner_project_id
                                    AND pm.account_id = $4
                                    AND pm.revoked_at IS NULL
                              )
                          )
                          OR EXISTS (
                              SELECT 1
                              FROM document_bindings b
                              JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
                              WHERE b.document_id = d.id
                                AND (
                                    EXISTS (
                                        SELECT 1 FROM team_memberships tm
                                        WHERE tm.team_id = i.team_id
                                          AND tm.account_id = $4
                                          AND tm.revoked_at IS NULL
                                    )
                                    OR EXISTS (
                                        SELECT 1 FROM project_memberships pm
                                        WHERE pm.project_id = i.project_id
                                          AND pm.account_id = $4
                                          AND pm.revoked_at IS NULL
                                    )
                                )
                          )
                      )
                )
                ELSE false
             END",
        )
        .bind(reference.resource_id)
        .bind(&reference.resource_kind)
        .bind(document_id)
        .bind(account_id)
        .fetch_one(&mut **tx)
        .await?;
        if !valid_target {
            return Err(Error::InvalidDocument(
                "reference target is invalid".to_owned(),
            ));
        }
        sqlx::query(
            "INSERT INTO document_references
             (document_id, source_block_id, resource_kind, resource_id, reference_kind)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(document_id)
        .bind(&reference.source_block_id)
        .bind(&reference.resource_kind)
        .bind(reference.resource_id)
        .bind(&reference.reference_kind)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub(crate) async fn require_document_access_tx<'a, E>(
    executor: E,
    account_id: Uuid,
    document_id: Uuid,
    required_role: i32,
) -> Result<(), Error>
where
    E: sqlx::Executor<'a, Database = Postgres>,
{
    let allowed = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM documents d
            WHERE d.id = $1 AND d.deleted_at IS NULL
              AND (
                (d.owner_team_id IS NULL AND d.owner_project_id IS NULL AND EXISTS (
                    SELECT 1 FROM organization_memberships om
                    WHERE om.organization_id = d.organization_id AND om.account_id = $2
                      AND om.revoked_at IS NULL
                      AND CASE om.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $3
                ))
                OR (d.owner_team_id IS NOT NULL AND EXISTS (
                    SELECT 1 FROM team_memberships tm
                    WHERE tm.team_id = d.owner_team_id AND tm.account_id = $2
                      AND tm.revoked_at IS NULL
                      AND CASE tm.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $3
                ))
                OR (d.owner_project_id IS NOT NULL AND EXISTS (
                    SELECT 1 FROM project_memberships pm
                    WHERE pm.project_id = d.owner_project_id AND pm.account_id = $2
                      AND pm.revoked_at IS NULL
                      AND CASE pm.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $3
                ))
                OR (d.owner_project_id IS NOT NULL AND EXISTS (
                    SELECT 1
                    FROM project_teams pt
                    JOIN team_memberships tm ON tm.team_id = pt.team_id
                    JOIN projects p ON p.id = pt.project_id
                    WHERE pt.project_id = d.owner_project_id AND tm.account_id = $2
                      AND tm.revoked_at IS NULL
                      AND EXISTS (
                          SELECT 1 FROM organization_memberships om
                          WHERE om.organization_id = p.organization_id
                            AND om.account_id = $2
                            AND om.revoked_at IS NULL
                      )
                      AND CASE
                          WHEN pt.role = 'admin' AND tm.role IN ('admin', 'owner') THEN 2
                          WHEN pt.role = 'admin' AND tm.role = 'member' THEN 1
                          WHEN pt.role IN ('commenter', 'operator')
                              AND tm.role IN ('member', 'admin', 'owner') THEN 1
                          ELSE 0
                      END >= $3
                ))
                OR EXISTS (
                    SELECT 1
                    FROM document_bindings b
                    JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
                    WHERE b.document_id = d.id
                      AND (
                        EXISTS (
                            SELECT 1 FROM team_memberships tm
                            WHERE tm.team_id = i.team_id AND tm.account_id = $2
                              AND tm.revoked_at IS NULL
                              AND CASE tm.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $3
                        ) OR EXISTS (
                            SELECT 1 FROM project_memberships pm
                            WHERE pm.project_id = i.project_id AND pm.account_id = $2
                              AND pm.revoked_at IS NULL
                              AND CASE pm.role WHEN 'owner' THEN 3 WHEN 'admin' THEN 2 WHEN 'member' THEN 1 ELSE 0 END >= $3
                        )
                      )
                )
              )
        )",
    )
    .bind(document_id)
    .bind(account_id)
    .bind(required_role)
    .fetch_one(executor)
    .await?;
    if allowed {
        Ok(())
    } else {
        Err(Error::DocumentAccessDenied)
    }
}

fn validate_document_input(input: &DocumentCreate) -> Result<(), Error> {
    if input.title.trim().is_empty() || input.title.chars().count() > 240 {
        return Err(Error::InvalidDocument(
            "title must be between 1 and 240 characters".to_owned(),
        ));
    }
    if input.kind == "issue_description" && input.owner_team_id.is_none() {
        return Err(Error::InvalidDocument(
            "issue descriptions require a team owner".to_owned(),
        ));
    }
    if input.owner_team_id.is_some() && input.owner_project_id.is_some() {
        return Err(Error::InvalidDocument(
            "a document may have only one owner scope".to_owned(),
        ));
    }
    if serde_json::to_vec(&input.content)
        .map_err(|_| Error::InvalidDocument("content is not serializable".to_owned()))?
        .len()
        > 1_000_000
    {
        return Err(Error::InvalidDocument(
            "document content is too large".to_owned(),
        ));
    }
    Ok(())
}
