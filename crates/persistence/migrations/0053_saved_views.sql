CREATE TABLE human_saved_views (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    filters JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (account_id, name),
    CHECK (char_length(name) BETWEEN 1 AND 80),
    CHECK (jsonb_typeof(filters) = 'object')
);

CREATE INDEX human_saved_views_account_idx ON human_saved_views (account_id, updated_at DESC);
