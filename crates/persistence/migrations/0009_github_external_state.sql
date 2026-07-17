CREATE TABLE external_links (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    issue_id UUID REFERENCES issues(id),
    provider TEXT NOT NULL CHECK (provider IN ('github')),
    external_id TEXT NOT NULL,
    repository TEXT NOT NULL,
    external_number BIGINT NOT NULL,
    url TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, provider, external_id)
);

CREATE INDEX external_links_issue_idx ON external_links (project_id, issue_id);

CREATE TABLE external_issue_snapshots (
    external_link_id UUID PRIMARY KEY REFERENCES external_links(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT,
    state TEXT NOT NULL,
    external_updated_at TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE webhook_deliveries (
    delivery_id TEXT PRIMARY KEY,
    project_id UUID REFERENCES projects(id),
    provider TEXT NOT NULL CHECK (provider IN ('github')),
    event_type TEXT NOT NULL,
    action TEXT NOT NULL,
    payload_hash BYTEA NOT NULL,
    payload JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('accepted', 'processed', 'rejected', 'failed')),
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    error TEXT
);

CREATE INDEX webhook_deliveries_project_time_idx
    ON webhook_deliveries (project_id, received_at);
