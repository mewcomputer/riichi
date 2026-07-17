-- Disposable, dataful fixture for backup/restore verification.
-- Run only against a database created for a restore drill.
\set ON_ERROR_STOP on
\set loro_snapshot 'bG9ybwAAAAAAAAAAAAAAAMoVM2sAA7AAAABMT1JPAAGvr/WjtMzF7X8qAAIAdnYBr6/1o7TMxe1/LAAMAH/bFmNEfVevAAAAAAAWABYBEAGvV31EYxbbfwEBAAAAAAAFAQAAAQAGAQQBAgAABQR0ZXh0AA4BBAIBAAIBAAIBBQIBFgAXFnJlc3RvcmUgZml4dHVyZSB1cGRhdGUAAAsAGwADACrRcPUBAAAABQAAAAIAZnIADAB/2xZjRH1XrwAAAABxtDd+jQAAAGEAAABMT1JPAAIBABZyZXN0b3JlIGZpeHR1cmUgdXBkYXRlAa9XfURjFtt/AwQCAQACAQACAQACASwAAAAAAQBzCLEJAQAAAAUAAAAGAIIEdGV4dAAGAIIEdGV4dGU0Pm9AAAAAAAAAAA=='
\set loro_update 'bG9ybwAAAAAAAAAAAAAAABu94AcABFIAFgAWARABr1d9RGMW238BAQAAAAAABQEAAAEABgEEAQIAAAUEdGV4dAAOAQQCAQACAQACAQUCARYAFxZyZXN0b3JlIGZpeHR1cmUgdXBkYXRl'
\set loro_frontiers '[{"peer":"9212982078323120047","counter":21}]'
\set loro_sha256 'gUqNSeqS69Bp06XtJQXOJAo18kEE+vMIoIus7dAugcY='

BEGIN;

INSERT INTO human_accounts (id, issuer, subject, email, display_name)
VALUES
    ('10000000-0000-4000-8000-000000000001', 'https://idp.example.test', 'restore-owner', 'owner@example.test', 'Restore Owner'),
    ('10000000-0000-4000-8000-000000000002', 'https://idp.example.test', 'restore-member', 'member@example.test', 'Restore Member');

INSERT INTO projects (id, name, organization_id)
VALUES
    ('20000000-0000-4000-8000-000000000001', 'Restore Project', '00000000-0000-0000-0000-000000000001'),
    ('20000000-0000-4000-8000-000000000002', 'Second Restore Project', '00000000-0000-0000-0000-000000000001');

INSERT INTO organization_memberships (organization_id, account_id, role)
VALUES
    ('00000000-0000-0000-0000-000000000001', '10000000-0000-4000-8000-000000000001', 'owner'),
    ('00000000-0000-0000-0000-000000000001', '10000000-0000-4000-8000-000000000002', 'member');

INSERT INTO team_memberships (team_id, account_id, role)
VALUES
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-4000-8000-000000000001', 'owner'),
    ('00000000-0000-0000-0000-000000000002', '10000000-0000-4000-8000-000000000002', 'member');

INSERT INTO project_teams (project_id, team_id, role)
VALUES
    ('20000000-0000-4000-8000-000000000001', '00000000-0000-0000-0000-000000000002', 'admin'),
    ('20000000-0000-4000-8000-000000000002', '00000000-0000-0000-0000-000000000002', 'operator');

INSERT INTO project_memberships (project_id, account_id, role)
VALUES
    ('20000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000001', 'owner'),
    ('20000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000002', 'member'),
    ('20000000-0000-4000-8000-000000000002', '10000000-0000-4000-8000-000000000001', 'owner');

INSERT INTO issues (id, project_id, team_id, display_key, title, body, status, importance, agent_eligible, spec_complete)
VALUES
    ('30000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', '00000000-0000-0000-0000-000000000002', 'RII-101', 'Restore the primary issue', 'Primary restored issue.', 'todo', 'high', true, true),
    ('30000000-0000-4000-8000-000000000002', '20000000-0000-4000-8000-000000000001', '00000000-0000-0000-0000-000000000002', 'RII-102', 'Restore the child issue', 'Child restored issue.', 'done', 'medium', false, true),
    ('30000000-0000-4000-8000-000000000003', '20000000-0000-4000-8000-000000000002', '00000000-0000-0000-0000-000000000002', 'RII-103', 'Restore the second project issue', 'Second project issue.', 'blocked', 'urgent', false, false);

UPDATE issues
SET parent_issue_id = '30000000-0000-4000-8000-000000000001'
WHERE id = '30000000-0000-4000-8000-000000000002';

INSERT INTO issue_dispatch (issue_id, rank, rank_scope)
VALUES
    ('30000000-0000-4000-8000-000000000001', 1, 'team'),
    ('30000000-0000-4000-8000-000000000002', 2, 'team'),
    ('30000000-0000-4000-8000-000000000003', 3, 'team');

INSERT INTO issue_labels (project_id, issue_id, label)
VALUES
    ('20000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000001', 'pilot'),
    ('20000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000002', 'restored'),
    ('20000000-0000-4000-8000-000000000002', '30000000-0000-4000-8000-000000000003', 'blocked');

INSERT INTO documents (id, organization_id, kind, title, owner_team_id, owner_project_id, provisioning_state, created_by)
VALUES
    ('40000000-0000-4000-8000-000000000001', '00000000-0000-0000-0000-000000000001', 'issue_description', 'RII-101 description', '00000000-0000-0000-0000-000000000002', NULL, 'ready', '10000000-0000-4000-8000-000000000001'),
    ('40000000-0000-4000-8000-000000000002', '00000000-0000-0000-0000-000000000001', 'issue_description', 'RII-102 description', '00000000-0000-0000-0000-000000000002', NULL, 'ready', '10000000-0000-4000-8000-000000000001'),
    ('40000000-0000-4000-8000-000000000003', '00000000-0000-0000-0000-000000000001', 'team_page', 'Riichi team handbook', '00000000-0000-0000-0000-000000000002', NULL, 'ready', '10000000-0000-4000-8000-000000000001'),
    ('40000000-0000-4000-8000-000000000004', '00000000-0000-0000-0000-000000000001', 'project_page', 'Restore project notes', NULL, '20000000-0000-4000-8000-000000000001', 'ready', '10000000-0000-4000-8000-000000000001');

INSERT INTO document_versions (document_id, revision, content, plain_text, sanitized_html, schema_version, frontiers, created_by)
SELECT id, 1,
       jsonb_build_object('type', 'doc', 'content', jsonb_build_array(jsonb_build_object('type', 'paragraph', 'content', jsonb_build_array(jsonb_build_object('type', 'text', 'text', title))))),
       title, '<p>' || title || '</p>', 1, :'loro_frontiers'::jsonb, '10000000-0000-4000-8000-000000000001'
FROM documents
WHERE id IN (
    '40000000-0000-4000-8000-000000000001',
    '40000000-0000-4000-8000-000000000002',
    '40000000-0000-4000-8000-000000000003',
    '40000000-0000-4000-8000-000000000004'
);

INSERT INTO document_projections (document_id, content_revision, plain_text, sanitized_html, schema_version)
SELECT document_id, 1, plain_text, sanitized_html, 1
FROM document_versions
WHERE revision = 1;

INSERT INTO document_bindings (document_id, resource_kind, resource_id, role)
VALUES
    ('40000000-0000-4000-8000-000000000001', 'issue', '30000000-0000-4000-8000-000000000001', 'description'),
    ('40000000-0000-4000-8000-000000000002', 'issue', '30000000-0000-4000-8000-000000000002', 'description'),
    ('40000000-0000-4000-8000-000000000003', 'team', '00000000-0000-0000-0000-000000000002', 'page'),
    ('40000000-0000-4000-8000-000000000004', 'project', '20000000-0000-4000-8000-000000000001', 'page');

INSERT INTO document_loro_snapshots (document_id, source_revision, schema_version, frontiers, snapshot)
SELECT id, 1, 1, :'loro_frontiers'::jsonb, decode(:'loro_snapshot', 'base64')
FROM documents
WHERE id IN (
    '40000000-0000-4000-8000-000000000001',
    '40000000-0000-4000-8000-000000000002',
    '40000000-0000-4000-8000-000000000003',
    '40000000-0000-4000-8000-000000000004'
);

INSERT INTO document_loro_updates
    (update_id, document_id, principal_id, source, peer_id, previous_frontiers, resulting_frontiers, payload, payload_sha256)
VALUES
    ('41000000-0000-4000-8000-000000000001', '40000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000001', 'restore-fixture', 'restore-peer', '[]', :'loro_frontiers'::jsonb, decode(:'loro_update', 'base64'), decode(:'loro_sha256', 'base64'));

INSERT INTO document_activity
    (id, document_id, update_id, actor_id, source, previous_frontiers, resulting_frontiers)
VALUES
    ('42000000-0000-4000-8000-000000000001', '40000000-0000-4000-8000-000000000001', '41000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000001', 'restore-fixture', '[]', :'loro_frontiers'::jsonb);

INSERT INTO attachments
    (id, organization_id, state, storage_key, filename, media_type, byte_size, checksum, uploaded_by, completed_at)
VALUES
    ('43000000-0000-4000-8000-000000000001', '00000000-0000-0000-0000-000000000001', 'ready', 'restore/attachment.txt', 'attachment.txt', 'text/plain', 7, decode('f329e3a317eee6a8a1a7357f69bc0488e0fad238ad58b30fc99139445f51e6ab', 'hex'), '10000000-0000-4000-8000-000000000001', now());

INSERT INTO document_attachments (document_id, attachment_id, source_block_id)
VALUES ('40000000-0000-4000-8000-000000000001', '43000000-0000-4000-8000-000000000001', 'restore-block');

INSERT INTO comments (id, project_id, issue_id, author_id, body, content)
VALUES
    ('50000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000002', 'Restored comment.', jsonb_build_object('type', 'doc', 'content', jsonb_build_array(jsonb_build_object('type', 'paragraph'))));

INSERT INTO audit_records
    (id, project_id, actor_id, request_id, operation, target_type, target_id, target_version, change_summary)
VALUES
    ('51000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000001', '52000000-0000-4000-8000-000000000001', 'update_issue', 'issue', '30000000-0000-4000-8000-000000000001', 1, jsonb_build_object('title', jsonb_build_object('before', 'Old title', 'after', 'Restore the primary issue')));

INSERT INTO approval_requests
    (id, project_id, issue_id, requested_by, target_version, proposed_operation, state, expires_at)
VALUES
    ('53000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000002', 1, jsonb_build_object('op', 'set_rank', 'rank', 4), 'pending', now() + interval '1 day');

INSERT INTO agent_roles (id, project_id, team_id, display_name, owner_account_id)
VALUES ('60000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', '00000000-0000-0000-0000-000000000002', 'restore-agent', '10000000-0000-4000-8000-000000000001');

INSERT INTO notifications (id, recipient_account_id, kind, project_id, issue_id, actor_id, payload)
VALUES ('70000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000001', 'approval', '20000000-0000-4000-8000-000000000001', '30000000-0000-4000-8000-000000000001', '10000000-0000-4000-8000-000000000002', jsonb_build_object('message', 'Restore fixture approval'));

INSERT INTO outbox_messages (id, project_id, message_type, payload, delivered_at)
VALUES ('80000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', 'issue_changed', jsonb_build_object('issue_id', '30000000-0000-4000-8000-000000000001'), now());

INSERT INTO delivery_events (id, project_id, event_type, payload)
VALUES ('80000000-0000-4000-8000-000000000001', '20000000-0000-4000-8000-000000000001', 'issue_changed', jsonb_build_object('issue_id', '30000000-0000-4000-8000-000000000001'));

COMMIT;

SELECT 'restore_fixture' AS fixture, count(*) AS issues FROM issues WHERE id::text LIKE '30000000-0000-4000-8000-%';
SELECT 'restore_fixture' AS fixture, count(*) AS documents FROM documents WHERE id::text LIKE '40000000-0000-4000-8000-%';
