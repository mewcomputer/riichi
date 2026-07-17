CREATE TABLE human_agent_sync (
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    agent_role_id UUID NOT NULL REFERENCES agent_roles(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL,
    owner_account_id UUID,
    capabilities JSONB NOT NULL,
    revoked_at TIMESTAMPTZ,
    active_session_count BIGINT NOT NULL,
    sessions JSONB NOT NULL,
    transaction_id BIGINT NOT NULL,
    PRIMARY KEY (account_id, agent_role_id)
);

CREATE INDEX human_agent_sync_account_team_idx
    ON human_agent_sync (account_id, team_id, display_name, agent_role_id);

CREATE OR REPLACE FUNCTION refresh_human_agent_sync_for_team(target_team_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM human_agent_sync WHERE team_id = target_team_id;

    INSERT INTO human_agent_sync (
        account_id,
        agent_role_id,
        project_id,
        team_id,
        display_name,
        owner_account_id,
        capabilities,
        revoked_at,
        active_session_count,
        sessions,
        transaction_id
    )
    SELECT tm.account_id,
           r.id,
           r.project_id,
           r.team_id,
           r.display_name,
           r.owner_account_id,
           r.capabilities,
           r.revoked_at,
           (SELECT count(*) FROM sessions s
            WHERE s.agent_role_id = r.id AND s.state = 'active'),
           COALESCE(
               (SELECT jsonb_agg(
                    jsonb_build_object(
                        'id', s.id,
                        'project_id', s.project_id,
                        'team_id', s.team_id,
                        'agent_role_id', s.agent_role_id,
                        'state', s.state,
                        'max_lifetime_ends_at', s.max_lifetime_ends_at,
                        'heartbeat_at', s.heartbeat_at,
                        'last_action_at', s.last_action_at,
                        'revoked_at', s.revoked_at
                    ) ORDER BY s.created_at DESC, s.id DESC
                )
                FROM sessions s
                WHERE s.agent_role_id = r.id),
               '[]'::jsonb
           ),
           txid_current()
    FROM team_memberships tm
    JOIN agent_roles r ON r.team_id = tm.team_id
    WHERE tm.team_id = target_team_id
      AND tm.revoked_at IS NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_agent_sync_from_role()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_human_agent_sync_for_team(OLD.team_id);
    ELSE
        PERFORM refresh_human_agent_sync_for_team(NEW.team_id);
        IF TG_OP = 'UPDATE' AND OLD.team_id IS DISTINCT FROM NEW.team_id THEN
            PERFORM refresh_human_agent_sync_for_team(OLD.team_id);
        END IF;
    END IF;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_agent_sync_from_session()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_human_agent_sync_for_team(OLD.team_id);
    ELSE
        PERFORM refresh_human_agent_sync_for_team(NEW.team_id);
        IF TG_OP = 'UPDATE' AND OLD.team_id IS DISTINCT FROM NEW.team_id THEN
            PERFORM refresh_human_agent_sync_for_team(OLD.team_id);
        END IF;
    END IF;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_agent_sync_from_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM refresh_human_agent_sync_for_team(OLD.team_id);
    ELSE
        PERFORM refresh_human_agent_sync_for_team(NEW.team_id);
        IF TG_OP = 'UPDATE' AND OLD.team_id IS DISTINCT FROM NEW.team_id THEN
            PERFORM refresh_human_agent_sync_for_team(OLD.team_id);
        END IF;
    END IF;
    RETURN NULL;
END;
$$;

CREATE TRIGGER human_agent_sync_role_trigger
AFTER INSERT OR UPDATE OR DELETE ON agent_roles
FOR EACH ROW EXECUTE FUNCTION refresh_human_agent_sync_from_role();

CREATE TRIGGER human_agent_sync_session_trigger
AFTER INSERT OR UPDATE OR DELETE ON sessions
FOR EACH ROW EXECUTE FUNCTION refresh_human_agent_sync_from_session();

CREATE TRIGGER human_agent_sync_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON team_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_human_agent_sync_from_membership();

DO $$
DECLARE
    target_team UUID;
BEGIN
    FOR target_team IN SELECT id FROM teams LOOP
        PERFORM refresh_human_agent_sync_for_team(target_team);
    END LOOP;
END;
$$;
