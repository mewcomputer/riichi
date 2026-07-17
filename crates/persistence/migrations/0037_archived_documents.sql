ALTER TABLE documents
    DROP CONSTRAINT documents_provisioning_state_check;

ALTER TABLE documents
    ADD CONSTRAINT documents_provisioning_state_check
    CHECK (provisioning_state IN ('pending', 'ready', 'failed', 'archived', 'deleted'));

