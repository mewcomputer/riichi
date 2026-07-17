CREATE TABLE human_document_sync (
    account_id UUID NOT NULL REFERENCES human_accounts(id) ON DELETE CASCADE,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    organization_id UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    parent_document_id UUID,
    position BIGINT NOT NULL,
    owner_team_id UUID,
    owner_project_id UUID,
    provisioning_state TEXT NOT NULL,
    created_by UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    current_revision BIGINT,
    plain_text TEXT,
    sanitized_html TEXT,
    transaction_id BIGINT NOT NULL,
    PRIMARY KEY (account_id, document_id)
);

CREATE INDEX human_document_sync_account_scope_idx
    ON human_document_sync (account_id, organization_id, owner_team_id, owner_project_id, parent_document_id, position, document_id);

CREATE OR REPLACE FUNCTION refresh_human_document_sync_for_document(target_document_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM human_document_sync WHERE document_id = target_document_id;

    INSERT INTO human_document_sync (
        account_id,
        document_id,
        organization_id,
        kind,
        title,
        parent_document_id,
        position,
        owner_team_id,
        owner_project_id,
        provisioning_state,
        created_by,
        created_at,
        updated_at,
        current_revision,
        plain_text,
        sanitized_html,
        transaction_id
    )
    SELECT account.id,
           d.id,
           d.organization_id,
           d.kind,
           d.title,
           d.parent_document_id,
           d.position,
           d.owner_team_id,
           d.owner_project_id,
           d.provisioning_state,
           d.created_by,
           d.created_at,
           d.updated_at,
           p.content_revision,
           p.plain_text,
           p.sanitized_html,
           txid_current()
    FROM human_accounts account
    JOIN documents d ON d.id = target_document_id AND d.deleted_at IS NULL
    LEFT JOIN document_projections p ON p.document_id = d.id
    WHERE (
        (
            d.owner_team_id IS NULL
            AND d.owner_project_id IS NULL
            AND EXISTS (
                SELECT 1 FROM organization_memberships om
                WHERE om.organization_id = d.organization_id
                  AND om.account_id = account.id
                  AND om.revoked_at IS NULL
            )
        ) OR (
            d.owner_team_id IS NOT NULL
            AND EXISTS (
                SELECT 1 FROM team_memberships tm
                WHERE tm.team_id = d.owner_team_id
                  AND tm.account_id = account.id
                  AND tm.revoked_at IS NULL
            )
        ) OR (
            d.owner_project_id IS NOT NULL
            AND EXISTS (
                SELECT 1 FROM project_memberships pm
                WHERE pm.project_id = d.owner_project_id
                  AND pm.account_id = account.id
                  AND pm.revoked_at IS NULL
            )
        ) OR EXISTS (
            SELECT 1
            FROM document_bindings b
            JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
            WHERE b.document_id = d.id
              AND (
                  EXISTS (
                      SELECT 1 FROM team_memberships tm
                      WHERE tm.team_id = i.team_id
                        AND tm.account_id = account.id
                        AND tm.revoked_at IS NULL
                  ) OR EXISTS (
                      SELECT 1 FROM project_memberships pm
                      WHERE pm.project_id = i.project_id
                        AND pm.account_id = account.id
                        AND pm.revoked_at IS NULL
                  )
              )
        )
    )
    ON CONFLICT (account_id, document_id) DO UPDATE SET
        organization_id = EXCLUDED.organization_id,
        kind = EXCLUDED.kind,
        title = EXCLUDED.title,
        parent_document_id = EXCLUDED.parent_document_id,
        position = EXCLUDED.position,
        owner_team_id = EXCLUDED.owner_team_id,
        owner_project_id = EXCLUDED.owner_project_id,
        provisioning_state = EXCLUDED.provisioning_state,
        created_by = EXCLUDED.created_by,
        created_at = EXCLUDED.created_at,
        updated_at = EXCLUDED.updated_at,
        current_revision = EXCLUDED.current_revision,
        plain_text = EXCLUDED.plain_text,
        sanitized_html = EXCLUDED.sanitized_html,
        transaction_id = EXCLUDED.transaction_id;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_document_sync_for_account(target_account_id UUID)
RETURNS VOID
LANGUAGE plpgsql
AS $$
BEGIN
    DELETE FROM human_document_sync WHERE account_id = target_account_id;

    INSERT INTO human_document_sync (
        account_id,
        document_id,
        organization_id,
        kind,
        title,
        parent_document_id,
        position,
        owner_team_id,
        owner_project_id,
        provisioning_state,
        created_by,
        created_at,
        updated_at,
        current_revision,
        plain_text,
        sanitized_html,
        transaction_id
    )
    SELECT target_account_id,
           d.id,
           d.organization_id,
           d.kind,
           d.title,
           d.parent_document_id,
           d.position,
           d.owner_team_id,
           d.owner_project_id,
           d.provisioning_state,
           d.created_by,
           d.created_at,
           d.updated_at,
           p.content_revision,
           p.plain_text,
           p.sanitized_html,
           txid_current()
    FROM documents d
    LEFT JOIN document_projections p ON p.document_id = d.id
    WHERE d.deleted_at IS NULL
      AND (
          (
              d.owner_team_id IS NULL
              AND d.owner_project_id IS NULL
              AND EXISTS (
                  SELECT 1 FROM organization_memberships om
                  WHERE om.organization_id = d.organization_id
                    AND om.account_id = target_account_id
                    AND om.revoked_at IS NULL
              )
          ) OR (
              d.owner_team_id IS NOT NULL
              AND EXISTS (
                  SELECT 1 FROM team_memberships tm
                  WHERE tm.team_id = d.owner_team_id
                    AND tm.account_id = target_account_id
                    AND tm.revoked_at IS NULL
              )
          ) OR (
              d.owner_project_id IS NOT NULL
              AND EXISTS (
                  SELECT 1 FROM project_memberships pm
                  WHERE pm.project_id = d.owner_project_id
                    AND pm.account_id = target_account_id
                    AND pm.revoked_at IS NULL
              )
          ) OR EXISTS (
              SELECT 1
              FROM document_bindings b
              JOIN issues i ON b.resource_kind = 'issue' AND b.resource_id = i.id
              WHERE b.document_id = d.id
                AND (
                    EXISTS (
                        SELECT 1 FROM team_memberships tm
                        WHERE tm.team_id = i.team_id
                          AND tm.account_id = target_account_id
                          AND tm.revoked_at IS NULL
                    ) OR EXISTS (
                        SELECT 1 FROM project_memberships pm
                        WHERE pm.project_id = i.project_id
                          AND pm.account_id = target_account_id
                          AND pm.revoked_at IS NULL
                    )
                )
          )
      );
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_document_sync_from_document()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_document_sync_for_document(CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END);
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_document_sync_from_document_id()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_document_sync_for_document(CASE WHEN TG_OP = 'DELETE' THEN OLD.document_id ELSE NEW.document_id END);
    IF TG_OP = 'UPDATE' AND OLD.document_id IS DISTINCT FROM NEW.document_id THEN
        PERFORM refresh_human_document_sync_for_document(OLD.document_id);
    END IF;
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_document_sync_from_account_membership()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    PERFORM refresh_human_document_sync_for_account(CASE WHEN TG_OP = 'DELETE' THEN OLD.account_id ELSE NEW.account_id END);
    RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION refresh_human_document_sync_from_issue()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
DECLARE
    target_issue UUID := CASE WHEN TG_OP = 'DELETE' THEN OLD.id ELSE NEW.id END;
    target_document UUID;
BEGIN
    FOR target_document IN
        SELECT document_id
        FROM document_bindings
        WHERE resource_kind = 'issue'
          AND resource_id = target_issue
    LOOP
        PERFORM refresh_human_document_sync_for_document(target_document);
    END LOOP;
    RETURN NULL;
END;
$$;

CREATE TRIGGER human_document_sync_document_trigger
AFTER INSERT OR UPDATE OR DELETE ON documents
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_document();

CREATE TRIGGER human_document_sync_binding_trigger
AFTER INSERT OR UPDATE OR DELETE ON document_bindings
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_document_id();

CREATE TRIGGER human_document_sync_projection_trigger
AFTER INSERT OR UPDATE OR DELETE ON document_projections
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_document_id();

CREATE TRIGGER human_document_sync_issue_trigger
AFTER INSERT OR UPDATE OR DELETE ON issues
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_issue();

CREATE TRIGGER human_document_sync_organization_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON organization_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_account_membership();

CREATE TRIGGER human_document_sync_team_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON team_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_account_membership();

CREATE TRIGGER human_document_sync_project_membership_trigger
AFTER INSERT OR UPDATE OR DELETE ON project_memberships
FOR EACH ROW EXECUTE FUNCTION refresh_human_document_sync_from_account_membership();

DO $$
DECLARE
    target_document UUID;
BEGIN
    FOR target_document IN SELECT id FROM documents LOOP
        PERFORM refresh_human_document_sync_for_document(target_document);
    END LOOP;
END;
$$;
