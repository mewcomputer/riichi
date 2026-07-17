CREATE TABLE document_loro_snapshot_history (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    source_revision BIGINT NOT NULL CHECK (source_revision > 0),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    frontiers JSONB NOT NULL,
    snapshot BYTEA NOT NULL CHECK (octet_length(snapshot) > 0),
    reason TEXT NOT NULL CHECK (char_length(reason) BETWEEN 1 AND 128),
    archived_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX document_loro_snapshot_history_document_idx
    ON document_loro_snapshot_history (document_id, archived_at DESC, id);
