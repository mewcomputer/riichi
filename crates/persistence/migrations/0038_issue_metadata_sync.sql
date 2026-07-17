CREATE TABLE issue_metadata_sync (
    issue_id UUID PRIMARY KEY REFERENCES issues(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    importance TEXT NOT NULL,
    agent_eligible BOOLEAN NOT NULL,
    spec_complete BOOLEAN NOT NULL,
    version BIGINT NOT NULL,
    rank BIGINT NOT NULL,
    labels TEXT[] NOT NULL DEFAULT ARRAY[]::text[],
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX issue_metadata_sync_project_idx
    ON issue_metadata_sync (project_id, updated_at, issue_id);

CREATE OR REPLACE FUNCTION refresh_issue_metadata_sync(target_issue_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO issue_metadata_sync (
        issue_id,
        project_id,
        title,
        status,
        importance,
        agent_eligible,
        spec_complete,
        version,
        rank,
        labels,
        updated_at
    )
    SELECT i.id,
           i.project_id,
           i.title,
           i.status,
           i.importance,
           i.agent_eligible,
           i.spec_complete,
           i.version,
           COALESCE(d.rank, 0),
           COALESCE(
               (SELECT array_agg(il.label ORDER BY il.label)
                FROM issue_labels il
                WHERE il.issue_id = i.id),
               ARRAY[]::text[]
           ),
           GREATEST(
               i.updated_at,
               COALESCE(d.updated_at, i.updated_at),
               COALESCE(
                   (SELECT max(il.created_at)
                    FROM issue_labels il
                    WHERE il.issue_id = i.id),
                   i.updated_at
               )
           )
    FROM issues i
    LEFT JOIN issue_dispatch d ON d.issue_id = i.id
    WHERE i.id = target_issue_id
    ON CONFLICT (issue_id) DO UPDATE SET
        project_id = EXCLUDED.project_id,
        title = EXCLUDED.title,
        status = EXCLUDED.status,
        importance = EXCLUDED.importance,
        agent_eligible = EXCLUDED.agent_eligible,
        spec_complete = EXCLUDED.spec_complete,
        version = EXCLUDED.version,
        rank = EXCLUDED.rank,
        labels = EXCLUDED.labels,
        updated_at = EXCLUDED.updated_at;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_issue_metadata_sync_from_issue()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_issue_metadata_sync(CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END);
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_issue_metadata_sync_from_issue_dispatch()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_issue_metadata_sync(CASE WHEN TG_OP = 'DELETE' THEN OLD.issue_id ELSE NEW.issue_id END);
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_issue_metadata_sync_from_label()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_issue_metadata_sync(CASE WHEN TG_OP = 'DELETE' THEN OLD.issue_id ELSE NEW.issue_id END);
    RETURN CASE WHEN TG_OP = 'DELETE' THEN OLD ELSE NEW END;
END;
$$;

CREATE TRIGGER issues_metadata_sync_trigger
AFTER INSERT OR UPDATE OF project_id, title, status, importance, agent_eligible, spec_complete, version, updated_at OR DELETE ON issues
FOR EACH ROW EXECUTE FUNCTION refresh_issue_metadata_sync_from_issue();

CREATE TRIGGER issue_dispatch_metadata_sync_trigger
AFTER INSERT OR UPDATE OF rank, updated_at OR DELETE ON issue_dispatch
FOR EACH ROW EXECUTE FUNCTION refresh_issue_metadata_sync_from_issue_dispatch();

CREATE TRIGGER issue_labels_metadata_sync_trigger
AFTER INSERT OR UPDATE OF label OR DELETE ON issue_labels
FOR EACH ROW EXECUTE FUNCTION refresh_issue_metadata_sync_from_label();

INSERT INTO issue_metadata_sync (
    issue_id,
    project_id,
    title,
    status,
    importance,
    agent_eligible,
    spec_complete,
    version,
    rank,
    labels,
    updated_at
)
SELECT i.id,
       i.project_id,
       i.title,
       i.status,
       i.importance,
       i.agent_eligible,
       i.spec_complete,
       i.version,
       COALESCE(d.rank, 0),
       COALESCE(
           (SELECT array_agg(il.label ORDER BY il.label)
            FROM issue_labels il
            WHERE il.issue_id = i.id),
           ARRAY[]::text[]
       ),
       GREATEST(i.updated_at, COALESCE(d.updated_at, i.updated_at))
FROM issues i
LEFT JOIN issue_dispatch d ON d.issue_id = i.id
ON CONFLICT (issue_id) DO NOTHING;
