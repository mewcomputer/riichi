CREATE TABLE projects (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE agent_roles (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    display_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

CREATE INDEX agent_roles_project_idx ON agent_roles (project_id);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    agent_role_id UUID NOT NULL REFERENCES agent_roles(id),
    state TEXT NOT NULL CHECK (state IN ('active', 'expired', 'revoked')),
    max_lifetime_ends_at TIMESTAMPTZ NOT NULL,
    heartbeat_at TIMESTAMPTZ,
    last_action_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);

CREATE INDEX sessions_project_idx ON sessions (project_id);
CREATE INDEX sessions_role_idx ON sessions (agent_role_id);

CREATE TABLE issues (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    display_key TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL CHECK (status IN ('triage', 'todo', 'in_progress', 'blocked', 'done', 'canceled')),
    agent_eligible BOOLEAN NOT NULL DEFAULT false,
    spec_complete BOOLEAN NOT NULL DEFAULT false,
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    UNIQUE (project_id, display_key)
);

CREATE INDEX issues_project_status_idx ON issues (project_id, status);

CREATE TABLE issue_dispatch (
    issue_id UUID PRIMARY KEY REFERENCES issues(id),
    unresolved_blocker_count INTEGER NOT NULL DEFAULT 0,
    active_hold_count INTEGER NOT NULL DEFAULT 0,
    active_lease_id UUID,
    fencing_token BIGINT NOT NULL DEFAULT 0,
    rank BIGINT NOT NULL DEFAULT 0,
    rank_scope TEXT NOT NULL DEFAULT 'project',
    dispatch_version BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX issue_dispatch_ready_idx
    ON issue_dispatch (rank_scope, rank, issue_id)
    WHERE unresolved_blocker_count = 0
      AND active_hold_count = 0
      AND active_lease_id IS NULL;

CREATE TABLE issue_edges (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    source_issue_id UUID NOT NULL REFERENCES issues(id),
    target_issue_id UUID NOT NULL REFERENCES issues(id),
    edge_type TEXT NOT NULL CHECK (edge_type IN ('blocks', 'related', 'discovered_from', 'duplicate_of')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (source_issue_id <> target_issue_id),
    UNIQUE (source_issue_id, target_issue_id, edge_type)
);

CREATE INDEX issue_edges_target_idx ON issue_edges (target_issue_id, edge_type);

CREATE TABLE dispatch_holds (
    id UUID PRIMARY KEY,
    issue_id UUID NOT NULL REFERENCES issues(id),
    hold_type TEXT NOT NULL CHECK (hold_type IN ('manual', 'needs_spec', 'awaiting_approval', 'scheduled', 'integration')),
    reason TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    released_at TIMESTAMPTZ
);

CREATE INDEX dispatch_holds_issue_active_idx
    ON dispatch_holds (issue_id)
    WHERE released_at IS NULL;

CREATE TABLE leases (
    id UUID PRIMARY KEY,
    issue_id UUID NOT NULL REFERENCES issues(id),
    owner_session_id UUID NOT NULL REFERENCES sessions(id),
    fencing_token BIGINT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('active', 'released', 'expired', 'revoked', 'completed')),
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    release_reason TEXT
);

CREATE UNIQUE INDEX leases_one_active_per_issue_idx
    ON leases (issue_id)
    WHERE state = 'active';

CREATE TABLE lease_collaborators (
    lease_id UUID NOT NULL REFERENCES leases(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    capability TEXT NOT NULL,
    grant_mode TEXT NOT NULL CHECK (grant_mode IN ('auto', 'approval_required')),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (lease_id, session_id, capability)
);

CREATE TABLE comments (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    issue_id UUID NOT NULL REFERENCES issues(id),
    author_id UUID NOT NULL,
    role_id UUID,
    session_id UUID,
    body TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX comments_issue_time_idx ON comments (issue_id, created_at);


CREATE TABLE audit_records (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    actor_id UUID NOT NULL,
    role_id UUID,
    session_id UUID,
    request_id UUID NOT NULL,
    operation TEXT NOT NULL,
    target_type TEXT NOT NULL,
    target_id UUID,
    target_version BIGINT,
    change_summary JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audit_records_project_time_idx ON audit_records (project_id, created_at);

CREATE TABLE idempotency_records (
    project_id UUID NOT NULL REFERENCES projects(id),
    actor_id UUID NOT NULL,
    operation TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_hash BYTEA NOT NULL,
    response JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, actor_id, operation, idempotency_key)
);

CREATE TABLE outbox_messages (
    id UUID PRIMARY KEY,
    project_id UUID REFERENCES projects(id),
    message_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    available_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    claimed_at TIMESTAMPTZ,
    delivered_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);

CREATE INDEX outbox_messages_pending_idx
    ON outbox_messages (available_at, id)
    WHERE delivered_at IS NULL;
