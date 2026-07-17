CREATE TABLE navigation_sync (
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    organization_name TEXT NOT NULL,
    organization_role TEXT NOT NULL,
    organization_has_logo BOOLEAN NOT NULL,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    team_name TEXT NOT NULL,
    team_key TEXT NOT NULL,
    team_emoji TEXT,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    project_name TEXT NOT NULL,
    project_role TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, team_id, project_id)
);

CREATE INDEX navigation_sync_account_idx
    ON navigation_sync (account_id, organization_name, team_name, project_name);

CREATE OR REPLACE FUNCTION refresh_navigation_sync_for_account(target_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM navigation_sync
    WHERE account_id = target_account_id;

    INSERT INTO navigation_sync (
        account_id,
        organization_id,
        organization_name,
        organization_role,
        organization_has_logo,
        team_id,
        team_name,
        team_key,
        team_emoji,
        project_id,
        project_name,
        project_role
    )
    SELECT om.account_id,
           o.id,
           o.name,
           om.role,
           o.logo_bytes IS NOT NULL AND o.logo_content_type IS NOT NULL,
           t.id,
           t.name,
           t.key,
           t.emoji,
           p.id,
           p.name,
           pt.role
    FROM organization_memberships om
    JOIN organizations o ON o.id = om.organization_id
    JOIN team_memberships tm ON tm.account_id = om.account_id
       AND tm.revoked_at IS NULL
    JOIN teams t ON t.id = tm.team_id AND t.organization_id = o.id
    JOIN project_teams pt ON pt.team_id = t.id
    JOIN projects p ON p.id = pt.project_id AND p.organization_id = o.id
    WHERE om.account_id = target_account_id
      AND om.revoked_at IS NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_for_organization(target_organization_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT account_id
        FROM organization_memberships
        WHERE organization_id = target_organization_id
    LOOP
        PERFORM refresh_navigation_sync_for_account(target_account);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_for_team(target_team_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT account_id
        FROM team_memberships
        WHERE team_id = target_team_id
    LOOP
        PERFORM refresh_navigation_sync_for_account(target_account);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_for_project(target_project_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT DISTINCT tm.account_id
        FROM project_teams pt
        JOIN team_memberships tm ON tm.team_id = pt.team_id
        WHERE pt.project_id = target_project_id
    LOOP
        PERFORM refresh_navigation_sync_for_account(target_account);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_from_organization()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_navigation_sync_for_organization(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_from_organization_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_navigation_sync_for_account(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_from_team()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_navigation_sync_for_team(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_from_team_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_navigation_sync_for_account(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_from_project()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_navigation_sync_for_project(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_navigation_sync_from_project_team()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_navigation_sync_for_project(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.project_id ELSE NEW.project_id END
    );
    PERFORM refresh_navigation_sync_for_team(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.team_id ELSE NEW.team_id END
    );
    RETURN NULL;
END;
$$;

CREATE TRIGGER navigation_sync_organization_trigger
AFTER INSERT OR UPDATE OR DELETE ON organizations
FOR EACH ROW EXECUTE FUNCTION refresh_navigation_sync_from_organization();

CREATE TRIGGER navigation_sync_organization_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON organization_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_navigation_sync_from_organization_membership();

CREATE TRIGGER navigation_sync_team_trigger
AFTER INSERT OR UPDATE OR DELETE ON teams
FOR EACH ROW EXECUTE FUNCTION refresh_navigation_sync_from_team();

CREATE TRIGGER navigation_sync_team_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON team_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_navigation_sync_from_team_membership();

CREATE TRIGGER navigation_sync_project_trigger
AFTER INSERT OR UPDATE OR DELETE ON projects
FOR EACH ROW EXECUTE FUNCTION refresh_navigation_sync_from_project();

CREATE TRIGGER navigation_sync_project_team_trigger
AFTER INSERT OR UPDATE OR DELETE ON project_teams
FOR EACH ROW EXECUTE FUNCTION refresh_navigation_sync_from_project_team();

INSERT INTO navigation_sync (
    account_id,
    organization_id,
    organization_name,
    organization_role,
    organization_has_logo,
    team_id,
    team_name,
    team_key,
    team_emoji,
    project_id,
    project_name,
    project_role
)
SELECT om.account_id,
       o.id,
       o.name,
       om.role,
       o.logo_bytes IS NOT NULL AND o.logo_content_type IS NOT NULL,
       t.id,
       t.name,
       t.key,
       t.emoji,
       p.id,
       p.name,
       pt.role
FROM organization_memberships om
JOIN organizations o ON o.id = om.organization_id
JOIN team_memberships tm ON tm.account_id = om.account_id
   AND tm.revoked_at IS NULL
JOIN teams t ON t.id = tm.team_id AND t.organization_id = o.id
JOIN project_teams pt ON pt.team_id = t.id
JOIN projects p ON p.id = pt.project_id AND p.organization_id = o.id
WHERE om.revoked_at IS NULL;
