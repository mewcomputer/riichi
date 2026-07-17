CREATE TABLE organizations (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO organizations (id, name)
VALUES ('00000000-0000-0000-0000-000000000001', 'Riichi')
ON CONFLICT (id) DO NOTHING;

ALTER TABLE projects
    ADD COLUMN organization_id UUID REFERENCES organizations(id);

UPDATE projects
SET organization_id = '00000000-0000-0000-0000-000000000001'
WHERE organization_id IS NULL;

ALTER TABLE projects
    ALTER COLUMN organization_id SET NOT NULL;

CREATE INDEX projects_organization_idx ON projects (organization_id, id);

CREATE TABLE teams (
    id UUID PRIMARY KEY,
    organization_id UUID NOT NULL REFERENCES organizations(id),
    name TEXT NOT NULL,
    key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (organization_id, key)
);

CREATE TABLE organization_memberships (
    organization_id UUID NOT NULL REFERENCES organizations(id),
    account_id UUID NOT NULL REFERENCES human_accounts(id),
    role TEXT NOT NULL CHECK (role IN ('member', 'admin', 'owner')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (organization_id, account_id)
);

CREATE TABLE team_memberships (
    team_id UUID NOT NULL REFERENCES teams(id),
    account_id UUID NOT NULL REFERENCES human_accounts(id),
    role TEXT NOT NULL CHECK (role IN ('member', 'admin', 'owner')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (team_id, account_id)
);

CREATE TABLE project_teams (
    project_id UUID NOT NULL REFERENCES projects(id),
    team_id UUID NOT NULL REFERENCES teams(id),
    role TEXT NOT NULL CHECK (role IN ('viewer', 'commenter', 'operator', 'admin')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, team_id)
);

CREATE INDEX team_memberships_account_idx ON team_memberships (account_id, team_id)
    WHERE revoked_at IS NULL;
CREATE INDEX project_teams_team_idx ON project_teams (team_id, project_id);

INSERT INTO teams (id, organization_id, name, key)
VALUES ('00000000-0000-0000-0000-000000000002', '00000000-0000-0000-0000-000000000001', 'Riichi', 'RII')
ON CONFLICT (id) DO NOTHING;

INSERT INTO project_teams (project_id, team_id, role)
SELECT id, '00000000-0000-0000-0000-000000000002', 'admin'
FROM projects
ON CONFLICT (project_id, team_id) DO NOTHING;

INSERT INTO organization_memberships (organization_id, account_id, role)
SELECT DISTINCT
    w.organization_id,
    memberships.account_id,
    CASE
        WHEN bool_or(memberships.role = 'owner') THEN 'owner'
        WHEN bool_or(memberships.role = 'admin') THEN 'admin'
        ELSE 'member'
    END
FROM project_memberships memberships
JOIN projects w ON w.id = memberships.project_id
WHERE memberships.revoked_at IS NULL
GROUP BY w.organization_id, memberships.account_id
ON CONFLICT (organization_id, account_id) DO NOTHING;

INSERT INTO team_memberships (team_id, account_id, role)
SELECT
    '00000000-0000-0000-0000-000000000002',
    memberships.account_id,
    CASE
        WHEN bool_or(memberships.role = 'owner') THEN 'owner'
        WHEN bool_or(memberships.role = 'admin') THEN 'admin'
        ELSE 'member'
    END
FROM project_memberships memberships
JOIN projects w ON w.id = memberships.project_id
WHERE memberships.revoked_at IS NULL
GROUP BY memberships.account_id
ON CONFLICT (team_id, account_id) DO NOTHING;
