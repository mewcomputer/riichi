CREATE TABLE human_issue_sync (
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    team_id UUID NOT NULL REFERENCES teams(id) ON DELETE CASCADE,
    team_name TEXT NOT NULL,
    team_key TEXT NOT NULL,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    project_name TEXT NOT NULL,
    display_key TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL,
    importance TEXT NOT NULL,
    agent_eligible BOOLEAN NOT NULL,
    spec_complete BOOLEAN NOT NULL,
    specification_changed_since_review BOOLEAN NOT NULL,
    unresolved_blocker_count INTEGER NOT NULL,
    active_hold_count INTEGER NOT NULL,
    active_lease_id UUID,
    lease_expires_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    rank BIGINT NOT NULL,
    dispatch_version BIGINT NOT NULL,
    assignee_account_id UUID,
    labels TEXT[] NOT NULL DEFAULT ARRAY[]::text[],
    transaction_id BIGINT NOT NULL,
    PRIMARY KEY (account_id, issue_id)
);

CREATE INDEX human_issue_sync_account_idx
    ON human_issue_sync (account_id, status, rank, issue_id);

CREATE INDEX human_issue_sync_team_idx
    ON human_issue_sync (account_id, team_id, status, rank, issue_id);

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_for_issue(target_issue_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM human_issue_sync WHERE issue_id = target_issue_id;

    INSERT INTO human_issue_sync (
        account_id,
        issue_id,
        team_id,
        team_name,
        team_key,
        project_id,
        project_name,
        display_key,
        title,
        body,
        status,
        importance,
        agent_eligible,
        spec_complete,
        specification_changed_since_review,
        unresolved_blocker_count,
        active_hold_count,
        active_lease_id,
        lease_expires_at,
        created_at,
        updated_at,
        rank,
        dispatch_version,
        assignee_account_id,
        labels,
        transaction_id
    )
    SELECT visible.account_id,
           i.id,
           i.team_id,
           t.name,
           t.key,
           i.project_id,
           p.name,
           i.display_key,
           i.title,
           COALESCE((SELECT dp.plain_text
                     FROM document_bindings db
                     JOIN document_projections dp ON dp.document_id = db.document_id
                     WHERE db.resource_kind = 'issue'
                       AND db.resource_id = i.id
                       AND db.role = 'description'
                     LIMIT 1), i.body),
           i.status,
           i.importance,
           i.agent_eligible,
           i.spec_complete,
           CASE
               WHEN NOT EXISTS (
                   SELECT 1 FROM document_bindings db
                   WHERE db.resource_kind = 'issue'
                     AND db.resource_id = i.id
                     AND db.role = 'description'
               ) THEN false
               WHEN NOT EXISTS (
                   SELECT 1
                   FROM document_bindings db
                   JOIN document_loro_snapshots ds ON ds.document_id = db.document_id
                   WHERE db.resource_kind = 'issue'
                     AND db.resource_id = i.id
                     AND db.role = 'description'
               ) THEN false
               WHEN i.spec_reviewed_frontiers IS NULL THEN true
               ELSE i.spec_reviewed_frontiers IS DISTINCT FROM (
                   SELECT ds.frontiers
                   FROM document_bindings db
                   JOIN document_loro_snapshots ds ON ds.document_id = db.document_id
                   WHERE db.resource_kind = 'issue'
                     AND db.resource_id = i.id
                     AND db.role = 'description'
                   LIMIT 1
               )
           END,
           d.unresolved_blocker_count,
           d.active_hold_count,
           d.active_lease_id,
           l.expires_at,
           i.created_at,
           i.updated_at,
           d.rank,
           d.dispatch_version,
           i.assignee_account_id,
           COALESCE(array_agg(il.label ORDER BY il.label)
               FILTER (WHERE il.label IS NOT NULL), ARRAY[]::text[]),
           txid_current()
    FROM issues i
    JOIN teams t ON t.id = i.team_id
    JOIN projects p ON p.id = i.project_id
    JOIN issue_dispatch d ON d.issue_id = i.id
    LEFT JOIN leases l
      ON l.id = d.active_lease_id
     AND l.state = 'active'
    LEFT JOIN issue_labels il ON il.issue_id = i.id
    JOIN (
        SELECT DISTINCT pm.account_id, pm.project_id
        FROM project_memberships pm
        WHERE pm.revoked_at IS NULL
        UNION
        SELECT DISTINCT tm.account_id, i2.project_id
        FROM team_memberships tm
        JOIN issues i2 ON i2.team_id = tm.team_id
        WHERE tm.revoked_at IS NULL
    ) visible ON visible.project_id = i.project_id
    WHERE i.id = target_issue_id
      AND (EXISTS (
              SELECT 1 FROM project_memberships pm
              WHERE pm.project_id = i.project_id
                AND pm.account_id = visible.account_id
                AND pm.revoked_at IS NULL
           ) OR EXISTS (
              SELECT 1 FROM team_memberships tm
              WHERE tm.team_id = i.team_id
                AND tm.account_id = visible.account_id
                AND tm.revoked_at IS NULL
           ))
    GROUP BY visible.account_id, i.id, t.name, t.key, p.name, d.issue_id,
             d.unresolved_blocker_count, d.active_hold_count, d.active_lease_id,
             l.expires_at, d.rank, d.dispatch_version;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_for_account(target_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_issue UUID;
BEGIN
    DELETE FROM human_issue_sync WHERE account_id = target_account_id;
    FOR target_issue IN
        SELECT i.id
        FROM issues i
        WHERE EXISTS (
            SELECT 1 FROM project_memberships pm
            WHERE pm.project_id = i.project_id
              AND pm.account_id = target_account_id
              AND pm.revoked_at IS NULL
        ) OR EXISTS (
            SELECT 1 FROM team_memberships tm
            WHERE tm.team_id = i.team_id
              AND tm.account_id = target_account_id
              AND tm.revoked_at IS NULL
        )
    LOOP
        PERFORM refresh_human_issue_sync_for_issue(target_issue);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_for_project(target_project_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT account_id FROM project_memberships WHERE project_id = target_project_id
        UNION
        SELECT tm.account_id
        FROM team_memberships tm
        JOIN issues i ON i.team_id = tm.team_id
        WHERE i.project_id = target_project_id
    LOOP
        PERFORM refresh_human_issue_sync_for_account(target_account);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_for_team(target_team_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
DECLARE
    target_account UUID;
BEGIN
    FOR target_account IN
        SELECT account_id FROM team_memberships WHERE team_id = target_team_id
        UNION
        SELECT pm.account_id
        FROM project_memberships pm
        JOIN issues i ON i.project_id = pm.project_id
        WHERE i.team_id = target_team_id
    LOOP
        PERFORM refresh_human_issue_sync_for_account(target_account);
    END LOOP;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_issue()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_issue_sync_for_issue(CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END);
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_issue_id()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_issue_sync_for_issue(CASE WHEN TG_OP = 'DELETE' THEN OLD.issue_id ELSE NEW.issue_id END);
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_document()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    target_document UUID := CASE WHEN TG_OP = 'DELETE' THEN OLD.document_id ELSE NEW.document_id END;
    target_issue UUID;
BEGIN
    FOR target_issue IN
        SELECT resource_id
        FROM document_bindings
        WHERE document_id = target_document
          AND resource_kind = 'issue'
          AND role = 'description'
    LOOP
        PERFORM refresh_human_issue_sync_for_issue(target_issue);
    END LOOP;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_binding()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        IF OLD.resource_kind = 'issue' AND OLD.role = 'description' THEN
            PERFORM refresh_human_issue_sync_for_issue(OLD.resource_id);
        END IF;
    ELSE
        IF NEW.resource_kind = 'issue' AND NEW.role = 'description' THEN
            PERFORM refresh_human_issue_sync_for_issue(NEW.resource_id);
        END IF;
        IF TG_OP = 'UPDATE'
           AND OLD.resource_kind = 'issue'
           AND OLD.role = 'description'
           AND (OLD.resource_id IS DISTINCT FROM NEW.resource_id
                OR OLD.document_id IS DISTINCT FROM NEW.document_id
                OR OLD.role IS DISTINCT FROM NEW.role
                OR OLD.resource_kind IS DISTINCT FROM NEW.resource_kind) THEN
            PERFORM refresh_human_issue_sync_for_issue(OLD.resource_id);
        END IF;
    END IF;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_account_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_issue_sync_for_account(CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END);
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_project()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_issue_sync_for_project(CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END);
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_issue_sync_from_team()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_issue_sync_for_team(CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END);
    RETURN NULL;
END;
$$;

CREATE TRIGGER human_issue_sync_issue_trigger
AFTER INSERT OR UPDATE OR DELETE ON issues
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_issue();

CREATE TRIGGER human_issue_sync_dispatch_trigger
AFTER INSERT OR UPDATE OR DELETE ON issue_dispatch
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_issue_id();

CREATE TRIGGER human_issue_sync_labels_trigger
AFTER INSERT OR UPDATE OR DELETE ON issue_labels
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_issue_id();

CREATE TRIGGER human_issue_sync_leases_trigger
AFTER INSERT OR UPDATE OR DELETE ON leases
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_issue_id();

CREATE TRIGGER human_issue_sync_bindings_trigger
AFTER INSERT OR UPDATE OR DELETE ON document_bindings
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_binding();

CREATE TRIGGER human_issue_sync_projections_trigger
AFTER INSERT OR UPDATE OR DELETE ON document_projections
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_document();

CREATE TRIGGER human_issue_sync_snapshots_trigger
AFTER INSERT OR UPDATE OR DELETE ON document_loro_snapshots
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_document();

CREATE TRIGGER human_issue_sync_project_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON project_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_account_membership();

CREATE TRIGGER human_issue_sync_team_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON team_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_account_membership();

CREATE TRIGGER human_issue_sync_project_trigger
AFTER INSERT OR UPDATE OR DELETE ON projects
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_project();

CREATE TRIGGER human_issue_sync_team_trigger
AFTER INSERT OR UPDATE OR DELETE ON teams
FOR EACH ROW EXECUTE FUNCTION refresh_human_issue_sync_from_team();

INSERT INTO human_issue_sync (
    account_id,
    issue_id,
    team_id,
    team_name,
    team_key,
    project_id,
    project_name,
    display_key,
    title,
    body,
    status,
    importance,
    agent_eligible,
    spec_complete,
    specification_changed_since_review,
    unresolved_blocker_count,
    active_hold_count,
    active_lease_id,
    lease_expires_at,
    created_at,
    updated_at,
    rank,
    dispatch_version,
    assignee_account_id,
    labels,
    transaction_id
)
SELECT visible.account_id,
       i.id,
       i.team_id,
       t.name,
       t.key,
       i.project_id,
       p.name,
       i.display_key,
       i.title,
       COALESCE((SELECT dp.plain_text
                 FROM document_bindings db
                 JOIN document_projections dp ON dp.document_id = db.document_id
                 WHERE db.resource_kind = 'issue'
                   AND db.resource_id = i.id
                   AND db.role = 'description'
                 LIMIT 1), i.body),
       i.status,
       i.importance,
       i.agent_eligible,
       i.spec_complete,
       false,
       d.unresolved_blocker_count,
       d.active_hold_count,
       d.active_lease_id,
       l.expires_at,
       i.created_at,
       i.updated_at,
       d.rank,
       d.dispatch_version,
       i.assignee_account_id,
       COALESCE(array_agg(il.label ORDER BY il.label)
           FILTER (WHERE il.label IS NOT NULL), ARRAY[]::text[]),
       txid_current()
FROM issues i
JOIN teams t ON t.id = i.team_id
JOIN projects p ON p.id = i.project_id
JOIN issue_dispatch d ON d.issue_id = i.id
LEFT JOIN leases l ON l.id = d.active_lease_id AND l.state = 'active'
LEFT JOIN issue_labels il ON il.issue_id = i.id
JOIN (
    SELECT DISTINCT pm.account_id, pm.project_id
    FROM project_memberships pm
    WHERE pm.revoked_at IS NULL
    UNION
    SELECT DISTINCT tm.account_id, i2.project_id
    FROM team_memberships tm
    JOIN issues i2 ON i2.team_id = tm.team_id
    WHERE tm.revoked_at IS NULL
) visible ON visible.project_id = i.project_id
WHERE EXISTS (
    SELECT 1 FROM project_memberships pm
    WHERE pm.project_id = i.project_id
      AND pm.account_id = visible.account_id
      AND pm.revoked_at IS NULL
) OR EXISTS (
    SELECT 1 FROM team_memberships tm
    WHERE tm.team_id = i.team_id
      AND tm.account_id = visible.account_id
      AND tm.revoked_at IS NULL
)
GROUP BY visible.account_id, i.id, t.name, t.key, p.name, d.issue_id,
         d.unresolved_blocker_count, d.active_hold_count, d.active_lease_id,
         l.expires_at, d.rank, d.dispatch_version;

DO $$
DECLARE
    target_issue UUID;
BEGIN
    FOR target_issue IN SELECT id FROM issues LOOP
        PERFORM refresh_human_issue_sync_for_issue(target_issue);
    END LOOP;
END;
$$;
