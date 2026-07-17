CREATE TABLE human_accounts (
    id UUID PRIMARY KEY,
    issuer TEXT NOT NULL,
    subject TEXT NOT NULL,
    email TEXT,
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (issuer, subject)
);

CREATE TABLE project_memberships (
    project_id UUID NOT NULL REFERENCES projects(id),
    account_id UUID NOT NULL REFERENCES human_accounts(id),
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member', 'viewer')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (project_id, account_id)
);

CREATE TABLE oidc_login_states (
    state_hash BYTEA PRIMARY KEY,
    issuer TEXT NOT NULL,
    nonce TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    return_to TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX oidc_login_states_expiry_idx ON oidc_login_states (expires_at);

CREATE TABLE human_sessions (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES human_accounts(id),
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

CREATE INDEX human_sessions_active_idx ON human_sessions (account_id, expires_at)
    WHERE revoked_at IS NULL;
