ALTER TABLE lease_collaborators
    ADD COLUMN granted_by UUID;

CREATE INDEX lease_collaborators_active_idx
    ON lease_collaborators (lease_id, session_id)
    WHERE revoked_at IS NULL;
