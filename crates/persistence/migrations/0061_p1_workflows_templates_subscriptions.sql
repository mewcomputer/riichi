CREATE TABLE workflow_alias_versions (
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    version BIGINT NOT NULL,
    created_by UUID NOT NULL REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, version)
);

CREATE TABLE workflow_aliases (
    project_id UUID NOT NULL,
    version BIGINT NOT NULL,
    label TEXT NOT NULL,
    canonical_status TEXT NOT NULL CHECK (canonical_status IN ('triage', 'todo', 'in_progress', 'blocked', 'done', 'canceled')),
    PRIMARY KEY (project_id, version, label),
    FOREIGN KEY (project_id, version) REFERENCES workflow_alias_versions(project_id, version) ON DELETE CASCADE
);

ALTER TABLE issues
    ADD COLUMN workflow_alias TEXT,
    ADD COLUMN workflow_alias_version BIGINT;
ALTER TABLE human_issue_sync
    ADD COLUMN workflow_alias TEXT,
    ADD COLUMN workflow_alias_version BIGINT;

CREATE INDEX issues_workflow_alias_idx ON issues (project_id, workflow_alias_version, workflow_alias);

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_workflow_alias()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    UPDATE human_issue_sync
    SET workflow_alias = NEW.workflow_alias,
        workflow_alias_version = NEW.workflow_alias_version
    WHERE issue_id = NEW.id;
    RETURN NULL;
END;
$$;

CREATE TRIGGER human_issue_sync_zz_workflow_alias_trigger
AFTER INSERT OR UPDATE ON issues
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_workflow_alias();

CREATE TABLE issue_templates (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    version BIGINT NOT NULL,
    snapshot JSONB NOT NULL,
    created_by UUID NOT NULL REFERENCES human_accounts(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, name, version)
);

CREATE INDEX issue_templates_project_idx ON issue_templates (project_id, lower(name), version DESC);

CREATE TABLE issue_template_instances (
    issue_id UUID PRIMARY KEY REFERENCES issues(id) ON DELETE CASCADE,
    template_id UUID NOT NULL REFERENCES issue_templates(id),
    template_version BIGINT NOT NULL,
    instantiated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE issue_subscriptions (
    id UUID PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID REFERENCES issues(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('approval', 'lease_expiry', 'blocked_dependency', 'quarantine')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (account_id, project_id, issue_id, kind)
);

CREATE INDEX issue_subscriptions_account_idx ON issue_subscriptions (account_id, project_id, kind);
