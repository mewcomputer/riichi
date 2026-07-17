CREATE TABLE recovery_checklists (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    issue_id UUID NOT NULL REFERENCES issues(id),
    old_lease_id UUID NOT NULL REFERENCES leases(id),
    old_session_id UUID NOT NULL REFERENCES sessions(id),
    initiated_by UUID NOT NULL,
    reason TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('open', 'completed', 'canceled')),
    actions JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ
);

CREATE INDEX recovery_checklists_issue_state_idx
    ON recovery_checklists (project_id, issue_id, state);

CREATE TABLE approval_requests (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id),
    issue_id UUID NOT NULL REFERENCES issues(id),
    requested_by UUID NOT NULL,
    target_version BIGINT NOT NULL,
    proposed_operation JSONB NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('pending', 'approved', 'rejected', 'superseded', 'expired')),
    expires_at TIMESTAMPTZ NOT NULL,
    decided_by UUID,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX approval_requests_project_state_idx
    ON approval_requests (project_id, state, expires_at);
