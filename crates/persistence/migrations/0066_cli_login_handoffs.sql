CREATE TABLE cli_login_handoffs (
    token_hash BYTEA PRIMARY KEY,
    account_id UUID REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    exchanged_at TIMESTAMPTZ
);

CREATE INDEX cli_login_handoffs_expiry_idx ON cli_login_handoffs (expires_at);
