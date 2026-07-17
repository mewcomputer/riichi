CREATE TABLE notifications (
    id UUID PRIMARY KEY,
    recipient_account_id UUID NOT NULL REFERENCES human_accounts(id),
    kind TEXT NOT NULL CHECK (kind IN ('comment', 'approval', 'assignment', 'invitation', 'takeover', 'lease')),
    project_id UUID REFERENCES projects(id),
    issue_id UUID REFERENCES issues(id),
    actor_id UUID,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    dedupe_key TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at TIMESTAMPTZ,
    UNIQUE (recipient_account_id, dedupe_key)
);

CREATE INDEX notifications_inbox_idx
    ON notifications (recipient_account_id, read_at, created_at DESC, id DESC);
