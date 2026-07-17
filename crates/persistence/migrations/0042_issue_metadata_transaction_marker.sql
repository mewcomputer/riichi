ALTER TABLE issue_metadata_sync
    ADD COLUMN transaction_id BIGINT;

UPDATE issue_metadata_sync
SET transaction_id = txid_current();

ALTER TABLE issue_metadata_sync
    ALTER COLUMN transaction_id SET NOT NULL;

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
        updated_at,
        transaction_id
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
           ),
           txid_current()
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
        updated_at = EXCLUDED.updated_at,
        transaction_id = EXCLUDED.transaction_id;
END;
$$;
