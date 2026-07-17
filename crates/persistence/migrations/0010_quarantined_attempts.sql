CREATE TABLE quarantined_attempts (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    issue_id UUID NOT NULL REFERENCES issues(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    role_id UUID NOT NULL REFERENCES agent_roles(id),
    lease_id UUID NOT NULL,
    fencing_token BIGINT NOT NULL,
    request_id UUID NOT NULL,
    reason TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX quarantined_attempts_issue_time_idx
    ON quarantined_attempts (project_id, issue_id, created_at DESC);
