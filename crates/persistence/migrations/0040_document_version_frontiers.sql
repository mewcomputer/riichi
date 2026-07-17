ALTER TABLE document_versions
    ADD COLUMN frontiers JSONB;

UPDATE document_versions v
SET frontiers = s.frontiers
FROM document_loro_snapshots s
WHERE s.document_id = v.document_id
  AND s.source_revision = v.revision;
