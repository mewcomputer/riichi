CREATE TABLE project_invites (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    invited_by UUID NOT NULL REFERENCES human_accounts(id),
    role TEXT NOT NULL CHECK (role IN ('admin', 'member', 'viewer')),
    email_hint TEXT,
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    accepted_at TIMESTAMPTZ,
    accepted_by UUID REFERENCES human_accounts(id),
    revoked_at TIMESTAMPTZ
);

CREATE INDEX project_invites_active_idx
    ON project_invites (project_id, expires_at)
    WHERE accepted_at IS NULL AND revoked_at IS NULL;
