use crate::{
    Database, Error,
    documents::{replace_document_references, require_document_access_tx},
};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, Postgres, Transaction};
use uuid::Uuid;

pub const MAX_LORO_UPDATE_BYTES: usize = 1_000_000;

#[derive(Debug, Clone, FromRow)]
pub struct LoroSnapshotRecord {
    pub document_id: Uuid,
    pub source_revision: i64,
    pub schema_version: i32,
    pub frontiers: serde_json::Value,
    pub snapshot: Vec<u8>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LoroSnapshotHistoryRecord {
    pub id: Uuid,
    pub document_id: Uuid,
    pub source_revision: i64,
    pub schema_version: i32,
    pub frontiers: serde_json::Value,
    pub snapshot: Vec<u8>,
    pub reason: String,
    pub archived_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LoroSnapshotSeed {
    pub document_id: Uuid,
    pub source_revision: i64,
    pub schema_version: i32,
    pub frontiers: serde_json::Value,
    pub snapshot: Vec<u8>,
}

#[derive(Debug, Clone, FromRow)]
pub struct LoroUpdateRecord {
    pub update_id: Uuid,
    pub document_id: Uuid,
    pub principal_id: Uuid,
    pub source: String,
    pub peer_id: String,
    pub idempotency_key: Option<String>,
    pub previous_frontiers: serde_json::Value,
    pub resulting_frontiers: serde_json::Value,
    pub payload: Vec<u8>,
    pub payload_sha256: Vec<u8>,
    pub accepted_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct LoroUpdateSeed {
    pub update_id: Uuid,
    pub document_id: Uuid,
    pub principal_id: Uuid,
    pub source: String,
    pub peer_id: String,
    pub idempotency_key: Option<String>,
    pub previous_frontiers: serde_json::Value,
    pub resulting_frontiers: serde_json::Value,
    pub payload: Vec<u8>,
    pub payload_sha256: Vec<u8>,
    pub snapshot: Vec<u8>,
    pub content: serde_json::Value,
    pub plain_text: String,
    pub sanitized_html: String,
    pub references: Vec<crate::DocumentReferenceInput>,
}

#[derive(Debug, Clone)]
pub struct LoroSchemaMigration {
    pub account_id: Uuid,
    pub document_id: Uuid,
    pub expected_schema_version: i32,
    pub expected_frontiers: serde_json::Value,
    pub target_schema_version: i32,
    pub snapshot: Vec<u8>,
    pub frontiers: serde_json::Value,
    pub content: serde_json::Value,
    pub plain_text: String,
    pub sanitized_html: String,
    pub references: Vec<crate::DocumentReferenceInput>,
    pub archive_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoroUpdateOutcome {
    Accepted,
    Replayed,
}

impl Database {
    pub async fn get_loro_update_for_broadcast(
        &self,
        document_id: Uuid,
        update_id: Uuid,
    ) -> Result<Option<LoroUpdateRecord>, Error> {
        Ok(sqlx::query_as::<_, LoroUpdateRecord>(
            "SELECT update_id, document_id, principal_id, source, peer_id,
                    idempotency_key, previous_frontiers, resulting_frontiers,
                    payload, payload_sha256, accepted_at
             FROM document_loro_updates
             WHERE document_id = $1 AND update_id = $2",
        )
        .bind(document_id)
        .bind(update_id)
        .fetch_optional(self.pool())
        .await?)
    }

    pub async fn get_loro_update(
        &self,
        account_id: Uuid,
        document_id: Uuid,
        update_id: Option<Uuid>,
        idempotency_key: Option<&str>,
    ) -> Result<Option<LoroUpdateRecord>, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        Ok(sqlx::query_as::<_, LoroUpdateRecord>(
            "SELECT update_id, document_id, principal_id, source, peer_id,
                    idempotency_key, previous_frontiers, resulting_frontiers,
                    payload, payload_sha256, accepted_at
             FROM document_loro_updates
             WHERE document_id = $1
               AND (($2::uuid IS NOT NULL AND update_id = $2)
                 OR ($3::text IS NOT NULL AND idempotency_key = $3))
             LIMIT 1",
        )
        .bind(document_id)
        .bind(update_id)
        .bind(idempotency_key)
        .fetch_optional(self.pool())
        .await?)
    }

    pub async fn get_loro_snapshot(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<LoroSnapshotRecord>, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        Ok(sqlx::query_as::<_, LoroSnapshotRecord>(
            "SELECT document_id, source_revision, schema_version, frontiers,
                    snapshot, updated_at
             FROM document_loro_snapshots
             WHERE document_id = $1",
        )
        .bind(document_id)
        .fetch_optional(self.pool())
        .await?)
    }

    pub async fn get_latest_loro_snapshot_history(
        &self,
        account_id: Uuid,
        document_id: Uuid,
    ) -> Result<Option<LoroSnapshotHistoryRecord>, Error> {
        self.require_document_access(account_id, document_id, 0)
            .await?;
        Ok(sqlx::query_as::<_, LoroSnapshotHistoryRecord>(
            "SELECT id, document_id, source_revision, schema_version, frontiers,
                    snapshot, reason, archived_at
             FROM document_loro_snapshot_history
             WHERE document_id = $1
             ORDER BY archived_at DESC, id DESC
             LIMIT 1",
        )
        .bind(document_id)
        .fetch_optional(self.pool())
        .await?)
    }

    pub async fn initialize_loro_snapshot(
        &self,
        account_id: Uuid,
        seed: LoroSnapshotSeed,
    ) -> Result<LoroSnapshotRecord, Error> {
        validate_snapshot_seed(&seed)?;
        let mut tx = self.pool.begin().await?;
        lock_document(&mut tx, seed.document_id).await?;
        require_document_access_tx(&mut *tx, account_id, seed.document_id, 0).await?;
        sqlx::query(
            "INSERT INTO document_loro_snapshots
             (document_id, source_revision, schema_version, frontiers, snapshot)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (document_id) DO NOTHING",
        )
        .bind(seed.document_id)
        .bind(seed.source_revision)
        .bind(seed.schema_version)
        .bind(&seed.frontiers)
        .bind(&seed.snapshot)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE document_versions
             SET frontiers = $2
             WHERE document_id = $1 AND revision = $3 AND frontiers IS NULL",
        )
        .bind(seed.document_id)
        .bind(&seed.frontiers)
        .bind(seed.source_revision)
        .execute(&mut *tx)
        .await?;
        let snapshot = select_snapshot(&mut tx, seed.document_id).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub async fn accept_loro_update(
        &self,
        account_id: Uuid,
        seed: LoroUpdateSeed,
    ) -> Result<(LoroUpdateRecord, LoroUpdateOutcome), Error> {
        validate_update_seed(&seed)?;
        let mut tx = self.pool.begin().await?;
        lock_document(&mut tx, seed.document_id).await?;
        require_document_access_tx(&mut *tx, account_id, seed.document_id, 1).await?;

        if let Some(existing) = find_existing_update(&mut tx, &seed).await? {
            if existing.document_id != seed.document_id
                || existing.principal_id != seed.principal_id
                || existing.payload_sha256 != seed.payload_sha256
            {
                return Err(Error::IdempotencyConflict);
            }
            tx.commit().await?;
            return Ok((existing, LoroUpdateOutcome::Replayed));
        }

        let snapshot = select_snapshot(&mut tx, seed.document_id).await?;
        if snapshot.frontiers != seed.previous_frontiers {
            return Err(Error::LoroFrontierConflict);
        }
        let next_revision: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(revision), 0) + 1
             FROM document_versions
             WHERE document_id = $1",
        )
        .bind(seed.document_id)
        .fetch_one(&mut *tx)
        .await?;
        let record = sqlx::query_as::<_, LoroUpdateRecord>(
            "INSERT INTO document_loro_updates
             (update_id, document_id, principal_id, source, peer_id, idempotency_key,
              previous_frontiers, resulting_frontiers, payload, payload_sha256)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING update_id, document_id, principal_id, source, peer_id,
                       idempotency_key, previous_frontiers, resulting_frontiers,
                       payload, payload_sha256, accepted_at",
        )
        .bind(seed.update_id)
        .bind(seed.document_id)
        .bind(seed.principal_id)
        .bind(&seed.source)
        .bind(&seed.peer_id)
        .bind(&seed.idempotency_key)
        .bind(&seed.previous_frontiers)
        .bind(&seed.resulting_frontiers)
        .bind(&seed.payload)
        .bind(&seed.payload_sha256)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_activity
             (id, document_id, update_id, actor_id, source, previous_frontiers, resulting_frontiers)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(seed.document_id)
        .bind(seed.update_id)
        .bind(seed.principal_id)
        .bind(&seed.source)
        .bind(&seed.previous_frontiers)
        .bind(&seed.resulting_frontiers)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE document_loro_snapshots
             SET source_revision = $2, frontiers = $3, snapshot = $4, updated_at = now()
             WHERE document_id = $1",
        )
        .bind(seed.document_id)
        .bind(next_revision)
        .bind(&seed.resulting_frontiers)
        .bind(&seed.snapshot)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_versions
             (document_id, revision, content, plain_text, sanitized_html,
              frontiers, schema_version, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(seed.document_id)
        .bind(next_revision)
        .bind(seed.content)
        .bind(&seed.plain_text)
        .bind(&seed.sanitized_html)
        .bind(&seed.resulting_frontiers)
        .bind(snapshot.schema_version)
        .bind(seed.principal_id)
        .execute(&mut *tx)
        .await?;
        replace_document_references(&mut tx, account_id, seed.document_id, &seed.references)
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
        .bind(seed.document_id)
        .bind(next_revision)
        .bind(&seed.plain_text)
        .bind(&seed.sanitized_html)
        .bind(snapshot.schema_version)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE documents SET updated_at = now() WHERE id = $1")
            .bind(seed.document_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("SELECT pg_notify('riichi_loro_updates', $1)")
            .bind(
                serde_json::json!({
                    "document_id": seed.document_id,
                    "update_id": seed.update_id,
                })
                .to_string(),
            )
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok((record, LoroUpdateOutcome::Accepted))
    }

    pub async fn migrate_loro_document_schema(
        &self,
        migration: LoroSchemaMigration,
    ) -> Result<LoroSnapshotRecord, Error> {
        let LoroSchemaMigration {
            account_id,
            document_id,
            expected_schema_version,
            expected_frontiers,
            target_schema_version,
            snapshot,
            frontiers,
            content,
            plain_text,
            sanitized_html,
            references,
            archive_reason,
        } = migration;
        validate_snapshot_seed(&LoroSnapshotSeed {
            document_id,
            source_revision: 1,
            schema_version: target_schema_version,
            frontiers: frontiers.clone(),
            snapshot: snapshot.clone(),
        })?;
        if archive_reason.trim().is_empty() || archive_reason.len() > 128 {
            return Err(Error::InvalidDocument(
                "schema migration archive reason is invalid".to_owned(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        lock_document(&mut tx, document_id).await?;
        require_document_access_tx(&mut *tx, account_id, document_id, 1).await?;
        let current = select_snapshot(&mut tx, document_id).await?;
        if current.schema_version != expected_schema_version
            || current.frontiers != expected_frontiers
        {
            return Err(Error::LoroFrontierConflict);
        }
        if current.schema_version == target_schema_version {
            tx.commit().await?;
            return Ok(current);
        }
        let next_revision: i64 = sqlx::query_scalar(
            "SELECT COALESCE(max(revision), 0) + 1
             FROM document_versions
             WHERE document_id = $1",
        )
        .bind(document_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_loro_snapshot_history
             (id, document_id, source_revision, schema_version, frontiers, snapshot, reason)
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(document_id)
        .bind(current.source_revision)
        .bind(current.schema_version)
        .bind(&current.frontiers)
        .bind(&current.snapshot)
        .bind(&archive_reason)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE document_loro_snapshots
             SET source_revision = $2, schema_version = $3, frontiers = $4,
                 snapshot = $5, updated_at = now()
             WHERE document_id = $1",
        )
        .bind(document_id)
        .bind(next_revision)
        .bind(target_schema_version)
        .bind(&frontiers)
        .bind(&snapshot)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO document_versions
             (document_id, revision, content, plain_text, sanitized_html,
              frontiers, schema_version, created_by)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(document_id)
        .bind(next_revision)
        .bind(content)
        .bind(&plain_text)
        .bind(&sanitized_html)
        .bind(&frontiers)
        .bind(target_schema_version)
        .bind(account_id)
        .execute(&mut *tx)
        .await?;
        replace_document_references(&mut tx, account_id, document_id, &references).await?;
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
        .bind(&plain_text)
        .bind(&sanitized_html)
        .bind(target_schema_version)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE documents SET updated_at = now() WHERE id = $1")
            .bind(document_id)
            .execute(&mut *tx)
            .await?;
        let migrated = select_snapshot(&mut tx, document_id).await?;
        tx.commit().await?;
        Ok(migrated)
    }

    pub async fn compact_loro_document(
        &self,
        document_id: Uuid,
        expected_frontiers: serde_json::Value,
        snapshot: Vec<u8>,
        plain_text: &str,
        sanitized_html: &str,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        lock_document(&mut tx, document_id).await?;
        let current = select_snapshot(&mut tx, document_id).await?;
        if current.frontiers != expected_frontiers {
            return Err(Error::LoroFrontierConflict);
        }
        let revision = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(max(revision), 1)
             FROM document_versions
             WHERE document_id = $1",
        )
        .bind(document_id)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE document_loro_snapshots
             SET source_revision = $2,
                 schema_version = $3,
                 frontiers = $4,
                 snapshot = $5,
                 updated_at = now()
             WHERE document_id = $1",
        )
        .bind(document_id)
        .bind(revision)
        .bind(current.schema_version)
        .bind(&expected_frontiers)
        .bind(snapshot)
        .execute(&mut *tx)
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
        .bind(current.schema_version)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM document_loro_updates WHERE document_id = $1")
            .bind(document_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

fn validate_snapshot_seed(seed: &LoroSnapshotSeed) -> Result<(), Error> {
    if seed.source_revision <= 0 || seed.schema_version <= 0 || seed.snapshot.is_empty() {
        return Err(Error::InvalidDocument(
            "Loro snapshots require positive revisions and non-empty data".to_owned(),
        ));
    }
    Ok(())
}

fn validate_update_seed(seed: &LoroUpdateSeed) -> Result<(), Error> {
    let content_size = serde_json::to_vec(&seed.content)
        .map(|content| content.len())
        .unwrap_or(usize::MAX);
    if seed.source.trim().is_empty()
        || seed.peer_id.trim().is_empty()
        || seed.payload.is_empty()
        || seed.payload.len() > MAX_LORO_UPDATE_BYTES
        || seed.payload_sha256.len() != 32
        || Sha256::digest(&seed.payload).as_slice() != seed.payload_sha256.as_slice()
        || content_size > 1_000_000
        || seed.plain_text.chars().count() > 200_000
        || seed.sanitized_html.len() > 500_000
    {
        return Err(Error::InvalidDocument(
            "Loro update metadata or payload is invalid".to_owned(),
        ));
    }
    Ok(())
}

async fn lock_document(tx: &mut Transaction<'_, Postgres>, document_id: Uuid) -> Result<(), Error> {
    sqlx::query("SELECT id FROM documents WHERE id = $1 AND deleted_at IS NULL FOR UPDATE")
        .bind(document_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::DocumentNotFound)?;
    Ok(())
}

async fn select_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    document_id: Uuid,
) -> Result<LoroSnapshotRecord, Error> {
    sqlx::query_as::<_, LoroSnapshotRecord>(
        "SELECT document_id, source_revision, schema_version, frontiers,
                snapshot, updated_at
         FROM document_loro_snapshots
         WHERE document_id = $1",
    )
    .bind(document_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| Error::InvalidDocument("Loro snapshot is not initialized".to_owned()))
}

async fn find_existing_update(
    tx: &mut Transaction<'_, Postgres>,
    seed: &LoroUpdateSeed,
) -> Result<Option<LoroUpdateRecord>, Error> {
    Ok(sqlx::query_as::<_, LoroUpdateRecord>(
        "SELECT update_id, document_id, principal_id, source, peer_id,
                idempotency_key, previous_frontiers, resulting_frontiers,
                payload, payload_sha256, accepted_at
         FROM document_loro_updates
         WHERE update_id = $1
            OR (document_id = $2 AND idempotency_key IS NOT NULL
                AND idempotency_key = $3)
         LIMIT 1",
    )
    .bind(seed.update_id)
    .bind(seed.document_id)
    .bind(&seed.idempotency_key)
    .fetch_optional(&mut **tx)
    .await?)
}
