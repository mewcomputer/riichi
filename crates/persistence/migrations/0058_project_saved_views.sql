ALTER TABLE human_saved_views
    ADD COLUMN project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    ADD COLUMN visibility TEXT NOT NULL DEFAULT 'personal'
        CHECK (visibility IN ('personal', 'project'));

ALTER TABLE human_saved_views
    ADD CONSTRAINT human_saved_views_scope_check
    CHECK (
        (visibility = 'personal' AND project_id IS NULL)
        OR (visibility = 'project' AND project_id IS NOT NULL)
    );

CREATE UNIQUE INDEX human_saved_views_project_name_idx
    ON human_saved_views (project_id, lower(name))
    WHERE visibility = 'project';

CREATE INDEX human_saved_views_project_idx
    ON human_saved_views (project_id, updated_at DESC)
    WHERE visibility = 'project';
