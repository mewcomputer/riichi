ALTER TABLE issue_subscriptions
    DROP CONSTRAINT IF EXISTS issue_subscriptions_account_id_project_id_issue_id_kind_key;

DELETE FROM issue_subscriptions s
USING issue_subscriptions duplicate
WHERE s.ctid < duplicate.ctid
  AND s.account_id = duplicate.account_id
  AND s.project_id = duplicate.project_id
  AND s.issue_id IS NOT DISTINCT FROM duplicate.issue_id
  AND s.kind = duplicate.kind;

CREATE UNIQUE INDEX IF NOT EXISTS issue_subscriptions_scope_kind_unique
    ON issue_subscriptions (account_id, project_id, COALESCE(issue_id, '00000000-0000-0000-0000-000000000000'::uuid), kind);
