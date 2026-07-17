ALTER TABLE document_bindings
    DROP CONSTRAINT IF EXISTS document_bindings_resource_kind_resource_id_role_key;

CREATE UNIQUE INDEX document_bindings_issue_description_idx
    ON document_bindings (resource_kind, resource_id, role)
    WHERE resource_kind = 'issue' AND role = 'description';

CREATE OR REPLACE FUNCTION validate_document_parent_scope()
RETURNS trigger
LANGUAGE plpgsql
AS $$
DECLARE
    parent_scope RECORD;
BEGIN
    IF NEW.parent_document_id IS NULL THEN
        RETURN NEW;
    END IF;

    SELECT organization_id, owner_team_id, owner_project_id
    INTO parent_scope
    FROM documents
    WHERE id = NEW.parent_document_id
      AND deleted_at IS NULL;

    IF NOT FOUND
       OR parent_scope.organization_id IS DISTINCT FROM NEW.organization_id
       OR parent_scope.owner_team_id IS DISTINCT FROM NEW.owner_team_id
       OR parent_scope.owner_project_id IS DISTINCT FROM NEW.owner_project_id THEN
        RAISE EXCEPTION 'parent document must have the same organization and owner scope';
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER documents_parent_scope_trigger
    BEFORE INSERT OR UPDATE OF parent_document_id, organization_id, owner_team_id, owner_project_id
    ON documents
    FOR EACH ROW
    EXECUTE FUNCTION validate_document_parent_scope();
