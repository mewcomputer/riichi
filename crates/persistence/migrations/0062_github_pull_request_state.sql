ALTER TABLE external_links
    ADD COLUMN external_kind TEXT NOT NULL DEFAULT 'issue'
        CHECK (external_kind IN ('issue', 'pull_request'));
ALTER TABLE external_links
    DROP CONSTRAINT IF EXISTS external_links_project_provider_external_id_key;

CREATE UNIQUE INDEX external_links_project_external_key_idx
    ON external_links (project_id, provider, external_kind, external_id);

CREATE TABLE github_pull_request_snapshots (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID REFERENCES issues(id) ON DELETE SET NULL,
    repository TEXT NOT NULL,
    pull_request_number BIGINT NOT NULL,
    title TEXT NOT NULL,
    url TEXT NOT NULL,
    state TEXT NOT NULL,
    review_state TEXT,
    ci_state TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    external_updated_at TEXT,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, repository, pull_request_number)
);

CREATE INDEX github_pull_request_project_idx
    ON github_pull_request_snapshots (project_id, fetched_at DESC);

CREATE TABLE github_project_integrations (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    repository TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    configured_by UUID NOT NULL REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
