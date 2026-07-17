ALTER TABLE agent_roles ADD COLUMN team_id UUID REFERENCES teams(id);

UPDATE agent_roles r
SET team_id = pt.team_id
FROM project_teams pt
WHERE pt.project_id = r.project_id
  AND r.team_id IS NULL;

ALTER TABLE agent_roles ALTER COLUMN team_id SET NOT NULL;
CREATE INDEX agent_roles_team_idx ON agent_roles (team_id, id);

ALTER TABLE sessions ADD COLUMN team_id UUID REFERENCES teams(id);

UPDATE sessions s
SET team_id = r.team_id
FROM agent_roles r
WHERE r.id = s.agent_role_id
  AND s.team_id IS NULL;

ALTER TABLE sessions ALTER COLUMN team_id SET NOT NULL;
CREATE INDEX sessions_team_idx ON sessions (team_id, id);

UPDATE issue_dispatch SET rank_scope = 'team';
ALTER TABLE issue_dispatch ALTER COLUMN rank_scope SET DEFAULT 'team';
