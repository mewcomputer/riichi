ALTER TABLE attachment_uploads
    ADD COLUMN cleanup_claimed_at TIMESTAMPTZ;

CREATE INDEX attachment_uploads_cleanup_idx
    ON attachment_uploads (expires_at, cleanup_claimed_at)
    WHERE completed_at IS NULL;
