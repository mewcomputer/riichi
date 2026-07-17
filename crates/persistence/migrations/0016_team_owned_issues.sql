-- Issues belong to a team. Projects are optional, cross-team grouping containers.
ALTER TABLE issues ADD COLUMN team_id UUID REFERENCES teams(id);

UPDATE issues i
SET team_id = pt.team_id
FROM project_teams pt
WHERE pt.project_id = i.project_id
  AND i.team_id IS NULL;

-- Existing pilot data has one default team per project. Keep the migration safe if
-- a project is already connected to more than one team by choosing the stable key.
UPDATE issues i
SET team_id = (
    SELECT t.id
    FROM project_teams pt
    JOIN teams t ON t.id = pt.team_id
    WHERE pt.project_id = i.project_id
    ORDER BY t.key, t.id
    LIMIT 1
)
WHERE i.team_id IS NULL;

ALTER TABLE issues ALTER COLUMN team_id SET NOT NULL;
CREATE INDEX issues_team_status_idx ON issues (team_id, status, updated_at DESC);
CREATE UNIQUE INDEX issues_team_display_key_idx ON issues (team_id, display_key);

CREATE TABLE issue_projects (
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    added_by UUID,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (issue_id, project_id)
);

INSERT INTO issue_projects (issue_id, project_id)
SELECT id, project_id FROM issues
ON CONFLICT (issue_id, project_id) DO NOTHING;

CREATE INDEX issue_projects_project_idx ON issue_projects (project_id, issue_id);
