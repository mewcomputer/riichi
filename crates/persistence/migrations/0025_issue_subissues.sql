ALTER TABLE issues
    ADD COLUMN parent_issue_id UUID,
    ADD CONSTRAINT issues_project_id_id_key UNIQUE (project_id, id),
    ADD CONSTRAINT issues_parent_same_project_fk
        FOREIGN KEY (project_id, parent_issue_id)
        REFERENCES issues (project_id, id),
    ADD CONSTRAINT issues_parent_not_self_check
        CHECK (parent_issue_id IS NULL OR parent_issue_id <> id);

CREATE INDEX issues_parent_issue_idx ON issues (parent_issue_id, id);
