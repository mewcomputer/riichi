CREATE TABLE document_loro_snapshots (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    source_revision BIGINT NOT NULL CHECK (source_revision > 0),
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    frontiers JSONB NOT NULL,
    snapshot BYTEA NOT NULL CHECK (octet_length(snapshot) > 0),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE document_loro_updates (
    update_id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    principal_id UUID NOT NULL REFERENCES human_accounts(id),
    source TEXT NOT NULL CHECK (char_length(source) BETWEEN 1 AND 64),
    peer_id TEXT NOT NULL CHECK (char_length(peer_id) BETWEEN 1 AND 64),
    idempotency_key TEXT,
    previous_frontiers JSONB NOT NULL,
    resulting_frontiers JSONB NOT NULL,
    payload BYTEA NOT NULL CHECK (octet_length(payload) BETWEEN 1 AND 1000000),
    payload_sha256 BYTEA NOT NULL CHECK (octet_length(payload_sha256) = 32),
    accepted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (document_id, idempotency_key)
);

CREATE INDEX document_loro_updates_document_idx
    ON document_loro_updates (document_id, accepted_at, update_id);
