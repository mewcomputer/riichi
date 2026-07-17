CREATE OR REPLACE FUNCTION notify_human_access_event(target_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM pg_notify(
        'riichi_human_access_events',
        json_build_object('account_id', target_account_id)::text
    );
END;
$$;

CREATE OR REPLACE FUNCTION notify_human_access_from_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM notify_human_access_event(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END
    );
    IF TG_OP = 'UPDATE' AND OLD.account_id IS DISTINCT FROM NEW.account_id THEN
        PERFORM notify_human_access_event(OLD.account_id);
    END IF;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION notify_human_access_from_session()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM notify_human_access_event(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION notify_human_access_from_project_team()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    target_project UUID := CASE WHEN TG_OP = 'DELETE' THEN OLD.project_id ELSE NEW.project_id END;
    target_team UUID := CASE WHEN TG_OP = 'DELETE' THEN OLD.team_id ELSE NEW.team_id END;
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT account_id FROM project_memberships WHERE project_id = target_project
        UNION
        SELECT account_id FROM team_memberships WHERE team_id = target_team
    LOOP
        PERFORM notify_human_access_event(target_account);
    END LOOP;

    IF TG_OP = 'UPDATE'
       AND (OLD.project_id IS DISTINCT FROM NEW.project_id
            OR OLD.team_id IS DISTINCT FROM NEW.team_id) THEN
        FOR target_account IN
            SELECT account_id FROM project_memberships WHERE project_id = OLD.project_id
            UNION
            SELECT account_id FROM team_memberships WHERE team_id = OLD.team_id
        LOOP
            PERFORM notify_human_access_event(target_account);
        END LOOP;
    END IF;
    RETURN NULL;
END;
$$;

CREATE TRIGGER human_access_organization_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON organization_memberships
FOR EACH ROW EXECUTE FUNCTION notify_human_access_from_membership();

CREATE TRIGGER human_access_team_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON team_memberships
FOR EACH ROW EXECUTE FUNCTION notify_human_access_from_membership();

CREATE TRIGGER human_access_project_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON project_memberships
FOR EACH ROW EXECUTE FUNCTION notify_human_access_from_membership();

CREATE TRIGGER human_access_session_trigger
AFTER INSERT OR UPDATE OR DELETE ON human_sessions
FOR EACH ROW EXECUTE FUNCTION notify_human_access_from_session();

CREATE TRIGGER human_access_project_team_trigger
AFTER INSERT OR UPDATE OR DELETE ON project_teams
FOR EACH ROW EXECUTE FUNCTION notify_human_access_from_project_team();
