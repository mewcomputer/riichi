CREATE TABLE documents (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    kind TEXT NOT NULL CHECK (kind IN ('issue_description', 'team_page', 'project_page', 'standalone_page')),
    title TEXT NOT NULL CHECK (char_length(title) BETWEEN 1 AND 240),
    parent_document_id UUID REFERENCES documents(id),
    position BIGINT NOT NULL DEFAULT 0,
    owner_team_id UUID REFERENCES teams(id),
    owner_project_id UUID REFERENCES projects(id),
    provisioning_state TEXT NOT NULL DEFAULT 'ready'
        CHECK (provisioning_state IN ('pending', 'ready', 'failed', 'deleted')),
    created_by UUID NOT NULL REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CHECK (NOT (owner_team_id IS NOT NULL AND owner_project_id IS NOT NULL)),
    CHECK (kind <> 'issue_description' OR owner_team_id IS NOT NULL),
    CHECK (kind <> 'team_page' OR owner_team_id IS NOT NULL),
    CHECK (kind <> 'project_page' OR owner_project_id IS NOT NULL),
    CHECK (kind <> 'standalone_page' OR (owner_team_id IS NULL AND owner_project_id IS NULL))
);

CREATE INDEX documents_scope_idx
    ON documents (organization_id, owner_team_id, owner_project_id, parent_document_id, position)
    WHERE deleted_at IS NULL;

CREATE TABLE document_bindings (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('issue', 'team', 'project', 'document')),
    resource_id UUID NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('description', 'page', 'reference')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (document_id, resource_kind, resource_id, role),
    UNIQUE (resource_kind, resource_id, role)
);

CREATE TABLE document_versions (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    revision BIGINT NOT NULL CHECK (revision > 0),
    content JSONB NOT NULL,
    plain_text TEXT NOT NULL,
    sanitized_html TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    created_by UUID NOT NULL REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (document_id, revision)
);

CREATE INDEX document_versions_latest_idx
    ON document_versions (document_id, revision DESC);

CREATE TABLE document_projections (
    document_id UUID PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
    content_revision BIGINT NOT NULL,
    plain_text TEXT NOT NULL,
    sanitized_html TEXT NOT NULL,
    schema_version INTEGER NOT NULL CHECK (schema_version > 0),
    projected_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE attachments (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    state TEXT NOT NULL CHECK (state IN ('pending', 'ready', 'quarantined', 'deleted')),
    storage_key TEXT NOT NULL UNIQUE,
    filename TEXT NOT NULL CHECK (char_length(filename) BETWEEN 1 AND 255),
    media_type TEXT NOT NULL CHECK (char_length(media_type) BETWEEN 1 AND 255),
    byte_size BIGINT NOT NULL CHECK (byte_size >= 0),
    checksum BYTEA NOT NULL CHECK (octet_length(checksum) = 32),
    uploaded_by UUID NOT NULL REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    deleted_at TIMESTAMPTZ
);

CREATE TABLE attachment_uploads (
    id UUID PRIMARY KEY,
    attachment_id UUID NOT NULL UNIQUE REFERENCES attachments(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    source_block_id TEXT NOT NULL,
    expected_byte_size BIGINT NOT NULL CHECK (expected_byte_size >= 0),
    expected_checksum BYTEA NOT NULL CHECK (octet_length(expected_checksum) = 32),
    expires_at TIMESTAMPTZ NOT NULL,
    completed_at TIMESTAMPTZ
);

CREATE TABLE document_attachments (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    attachment_id UUID NOT NULL REFERENCES attachments(id),
    source_block_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (document_id, attachment_id, source_block_id)
);

CREATE TABLE document_references (
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    source_block_id TEXT NOT NULL,
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('issue', 'team', 'project', 'document')),
    resource_id UUID NOT NULL,
    reference_kind TEXT NOT NULL CHECK (reference_kind IN ('inline', 'backlink')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (document_id, source_block_id, resource_kind, resource_id, reference_kind)
);

CREATE INDEX document_references_target_idx
    ON document_references (resource_kind, resource_id, document_id);

CREATE TABLE document_jobs (
    id UUID PRIMARY KEY,
    document_id UUID REFERENCES documents(id) ON DELETE CASCADE,
    job_type TEXT NOT NULL CHECK (job_type IN ('provision', 'project', 'compact', 'archive', 'delete', 'attachment_cleanup')),
    idempotency_key TEXT NOT NULL,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    completed_at TIMESTAMPTZ,
    last_error TEXT,
    UNIQUE (job_type, idempotency_key)
);

CREATE INDEX document_jobs_pending_idx
    ON document_jobs (available_at, id)
    WHERE completed_at IS NULL;
