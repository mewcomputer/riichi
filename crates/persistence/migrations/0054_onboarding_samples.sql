CREATE TABLE onboarding_samples (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    role_id UUID NOT NULL REFERENCES agent_roles(id),
    session_id UUID NOT NULL REFERENCES sessions(id),
    triage_issue_id UUID NOT NULL REFERENCES issues(id),
    agent_issue_id UUID NOT NULL REFERENCES issues(id),
    recovery_issue_id UUID NOT NULL REFERENCES issues(id),
    approval_id UUID NOT NULL REFERENCES approval_requests(id),
    recovery_checklist_id UUID NOT NULL REFERENCES recovery_checklists(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
