CREATE TABLE approval_sync (
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    team_key TEXT NOT NULL,
    project_name TEXT NOT NULL,
    issue_title TEXT NOT NULL,
    id UUID NOT NULL REFERENCES approval_requests(id) ON DELETE CASCADE,
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    requested_by UUID NOT NULL,
    target_version BIGINT NOT NULL,
    proposed_operation JSONB NOT NULL,
    state TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    decided_by UUID,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (account_id, id)
);

CREATE INDEX approval_sync_account_created_idx
    ON approval_sync (account_id, created_at DESC, id DESC);

CREATE OR REPLACE FUNCTION refresh_approval_sync_for_account(target_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM approval_sync
    WHERE account_id = target_account_id;

    INSERT INTO approval_sync (
        account_id,
        project_id,
        team_key,
        project_name,
        issue_title,
        id,
        issue_id,
        requested_by,
        target_version,
        proposed_operation,
        state,
        expires_at,
        decided_by,
        decided_at,
        created_at
    )
    SELECT pm.account_id,
           a.project_id,
           t.key,
           p.name,
           i.title,
           a.id,
           a.issue_id,
           a.requested_by,
           a.target_version,
           a.proposed_operation,
           a.state,
           a.expires_at,
           a.decided_by,
           a.decided_at,
           a.created_at
    FROM approval_requests a
    JOIN projects p ON p.id = a.project_id
    JOIN issues i ON i.id = a.issue_id
    JOIN teams t ON t.id = i.team_id
    JOIN project_memberships pm
      ON pm.project_id = a.project_id
     AND pm.account_id = target_account_id
     AND pm.revoked_at IS NULL
     AND pm.role IN ('owner', 'admin')
    WHERE a.state = 'pending';
END;
$$;

CREATE OR REPLACE FUNCTION refresh_approval_sync_for_project(target_project_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT account_id
        FROM project_memberships
        WHERE project_id = target_project_id
    LOOP
        PERFORM refresh_approval_sync_for_account(target_account);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_approval_sync_from_approval()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_approval_sync_for_project(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.project_id ELSE NEW.project_id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_approval_sync_from_project_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_approval_sync_for_account(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_approval_sync_from_project()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_approval_sync_for_project(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END
    );
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_approval_sync_from_issue()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_approval_sync_for_project(
        CASE WHEN TG_OP = 'DELETE' THEN OLD.project_id ELSE NEW.project_id END
    );
    RETURN NULL;
END;
$$;

CREATE TRIGGER approval_sync_approval_trigger
AFTER INSERT OR UPDATE OR DELETE ON approval_requests
FOR EACH ROW EXECUTE FUNCTION refresh_approval_sync_from_approval();

CREATE TRIGGER approval_sync_project_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON project_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_approval_sync_from_project_membership();

CREATE TRIGGER approval_sync_project_trigger
AFTER INSERT OR UPDATE OR DELETE ON projects
FOR EACH ROW EXECUTE FUNCTION refresh_approval_sync_from_project();

CREATE TRIGGER approval_sync_issue_trigger
AFTER INSERT OR UPDATE OR DELETE ON issues
FOR EACH ROW EXECUTE FUNCTION refresh_approval_sync_from_issue();

INSERT INTO approval_sync (
    account_id,
    project_id,
    team_key,
    project_name,
    issue_title,
    id,
    issue_id,
    requested_by,
    target_version,
    proposed_operation,
    state,
    expires_at,
    decided_by,
    decided_at,
    created_at
)
SELECT pm.account_id,
       a.project_id,
       t.key,
       p.name,
       i.title,
       a.id,
       a.issue_id,
       a.requested_by,
       a.target_version,
       a.proposed_operation,
       a.state,
       a.expires_at,
       a.decided_by,
       a.decided_at,
       a.created_at
FROM approval_requests a
JOIN projects p ON p.id = a.project_id
JOIN issues i ON i.id = a.issue_id
JOIN teams t ON t.id = i.team_id
JOIN project_memberships pm
  ON pm.project_id = a.project_id
 AND pm.revoked_at IS NULL
 AND pm.role IN ('owner', 'admin')
WHERE a.state = 'pending';
