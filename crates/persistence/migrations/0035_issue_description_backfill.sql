CREATE TEMP TABLE issue_description_backfill (
    issue_id UUID PRIMARY KEY,
    document_id UUID NOT NULL,
    organization_id UUID NOT NULL,
    team_id UUID NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    created_by UUID NOT NULL
) ON COMMIT DROP;

INSERT INTO issue_description_backfill
    (issue_id, document_id, organization_id, team_id, title, body, created_by)
SELECT i.id,
       gen_random_uuid(),
       p.organization_id,
       i.team_id,
       i.title,
       i.body,
       membership.account_id
FROM issues i
JOIN projects p ON p.id = i.project_id
JOIN LATERAL (
    SELECT pm.account_id
    FROM project_memberships pm
    WHERE pm.project_id = i.project_id
      AND pm.revoked_at IS NULL
    ORDER BY CASE pm.role
        WHEN 'owner' THEN 0
        WHEN 'admin' THEN 1
        WHEN 'member' THEN 2
        ELSE 3
    END, pm.account_id
    LIMIT 1
) membership ON true
WHERE NOT EXISTS (
    SELECT 1
    FROM document_bindings b
    WHERE b.resource_kind = 'issue'
      AND b.resource_id = i.id
      AND b.role = 'description'
);

INSERT INTO documents
    (id, organization_id, kind, title, owner_team_id, provisioning_state, created_by)
SELECT document_id, organization_id, 'issue_description', title, team_id, 'pending', created_by
FROM issue_description_backfill;

INSERT INTO document_bindings (document_id, resource_kind, resource_id, role)
SELECT document_id, 'issue', issue_id, 'description'
FROM issue_description_backfill;

INSERT INTO document_versions
    (document_id, revision, content, plain_text, sanitized_html, schema_version, created_by)
SELECT document_id,
       1,
       CASE WHEN body = '' THEN
           '{"type":"doc","content":[]}'::jsonb
       ELSE
           jsonb_build_object(
               'type', 'doc',
               'content', jsonb_build_array(jsonb_build_object(
                   'type', 'paragraph',
                   'content', jsonb_build_array(jsonb_build_object('type', 'text', 'text', body))
               ))
           )
       END,
       body,
       '<p>' || replace(replace(replace(replace(replace(body, '&', '&amp;'), '<', '&lt;'), '>', '&gt;'), '"', '&quot;'), '''', '&#39;') || '</p>',
       1,
       created_by
FROM issue_description_backfill;

INSERT INTO document_projections
    (document_id, content_revision, plain_text, sanitized_html, schema_version)
SELECT document_id,
       1,
       body,
       '<p>' || replace(replace(replace(replace(replace(body, '&', '&amp;'), '<', '&lt;'), '>', '&gt;'), '"', '&quot;'), '''', '&#39;') || '</p>',
       1
FROM issue_description_backfill;

INSERT INTO document_jobs (id, document_id, job_type, idempotency_key)
SELECT gen_random_uuid(), document_id, 'provision', 'issue-description:' || issue_id
FROM issue_description_backfill;
