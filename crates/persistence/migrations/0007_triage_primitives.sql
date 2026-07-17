ALTER TABLE issues
    ADD COLUMN assignee_account_id UUID REFERENCES human_accounts(id);

CREATE INDEX issues_assignee_idx ON issues (assignee_account_id)
    WHERE assignee_account_id IS NOT NULL;

CREATE TABLE issue_labels (
    project_id UUID NOT NULL REFERENCES projects(id),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    label TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (issue_id, label)
);

CREATE INDEX issue_labels_project_label_idx ON issue_labels (project_id, label, issue_id);

ALTER TABLE dispatch_holds
    ADD COLUMN created_by UUID;

CREATE INDEX issue_edges_project_source_idx
    ON issue_edges (project_id, source_issue_id, edge_type);

CREATE INDEX dispatch_holds_expiry_idx
    ON dispatch_holds (issue_id, expires_at)
    WHERE released_at IS NULL;
