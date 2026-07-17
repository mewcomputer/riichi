CREATE TABLE issue_activity_sync (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    actor_id UUID NOT NULL,
    body TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX issue_activity_sync_issue_time_idx
    ON issue_activity_sync (project_id, issue_id, created_at, id);

CREATE OR REPLACE FUNCTION refresh_issue_activity_sync_from_comment()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO issue_activity_sync
        (id, project_id, issue_id, kind, actor_id, body, metadata, created_at)
    VALUES
        (NEW.id, NEW.project_id, NEW.issue_id, 'comment', NEW.author_id, NEW.body,
         COALESCE(NEW.content, '{}'::jsonb), NEW.created_at)
    ON CONFLICT (id) DO NOTHING;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_issue_activity_sync_from_audit()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.target_type = 'issue' AND NEW.target_id IS NOT NULL THEN
        INSERT INTO issue_activity_sync
            (id, project_id, issue_id, kind, actor_id, body, metadata, created_at)
        VALUES
            (NEW.id, NEW.project_id, NEW.target_id, NEW.operation, NEW.actor_id,
             NULL, NEW.change_summary, NEW.created_at)
        ON CONFLICT (id) DO NOTHING;
    END IF;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_issue_activity_sync_from_document()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO issue_activity_sync
        (id, project_id, issue_id, kind, actor_id, body, metadata, created_at)
    SELECT NEW.id,
           i.project_id,
           b.resource_id,
           'document_edit',
           NEW.actor_id,
           NULL,
           jsonb_build_object(
               'document_id', NEW.document_id,
               'update_id', NEW.update_id,
               'source', NEW.source,
               'previous_frontiers', NEW.previous_frontiers,
               'resulting_frontiers', NEW.resulting_frontiers
           ),
           NEW.created_at
    FROM document_bindings b
    JOIN issues i ON i.id = b.resource_id
    WHERE b.document_id = NEW.document_id
      AND b.resource_kind = 'issue'
      AND b.role = 'description'
    ON CONFLICT (id) DO NOTHING;
    RETURN NEW;
END;
$$;

CREATE OR REPLACE FUNCTION remove_issue_activity_sync_for_document()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.resource_kind = 'issue' AND OLD.role = 'description' THEN
        DELETE FROM issue_activity_sync
        WHERE issue_id = OLD.resource_id
          AND kind = 'document_edit'
          AND metadata->>'document_id' = OLD.document_id::text;
    END IF;
    RETURN OLD;
END;
$$;

CREATE TRIGGER comments_issue_activity_sync_trigger
AFTER INSERT ON comments
FOR EACH ROW EXECUTE FUNCTION refresh_issue_activity_sync_from_comment();

CREATE TRIGGER audit_issue_activity_sync_trigger
AFTER INSERT ON audit_records
FOR EACH ROW EXECUTE FUNCTION refresh_issue_activity_sync_from_audit();

CREATE TRIGGER document_issue_activity_sync_trigger
AFTER INSERT ON document_activity
FOR EACH ROW EXECUTE FUNCTION refresh_issue_activity_sync_from_document();

CREATE TRIGGER document_binding_issue_activity_sync_cleanup_trigger
AFTER DELETE ON document_bindings
FOR EACH ROW EXECUTE FUNCTION remove_issue_activity_sync_for_document();

INSERT INTO issue_activity_sync
    (id, project_id, issue_id, kind, actor_id, body, metadata, created_at)
SELECT c.id, c.project_id, c.issue_id, 'comment', c.author_id, c.body,
       COALESCE(c.content, '{}'::jsonb), c.created_at
FROM comments c
ON CONFLICT (id) DO NOTHING;

INSERT INTO issue_activity_sync
    (id, project_id, issue_id, kind, actor_id, body, metadata, created_at)
SELECT a.id, a.project_id, a.target_id, a.operation, a.actor_id, NULL,
       a.change_summary, a.created_at
FROM audit_records a
WHERE a.target_type = 'issue'
  AND a.target_id IS NOT NULL
ON CONFLICT (id) DO NOTHING;

INSERT INTO issue_activity_sync
    (id, project_id, issue_id, kind, actor_id, body, metadata, created_at)
SELECT da.id,
       i.project_id,
       db.resource_id,
       'document_edit',
       da.actor_id,
       NULL,
       jsonb_build_object(
           'document_id', da.document_id,
           'update_id', da.update_id,
           'source', da.source,
           'previous_frontiers', da.previous_frontiers,
           'resulting_frontiers', da.resulting_frontiers
       ),
       da.created_at
FROM document_activity da
JOIN document_bindings db
  ON db.document_id = da.document_id
 AND db.resource_kind = 'issue'
 AND db.role = 'description'
JOIN issues i ON i.id = db.resource_id
ON CONFLICT (id) DO NOTHING;
