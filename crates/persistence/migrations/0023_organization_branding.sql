ALTER TABLE organizations
    ADD COLUMN logo_bytes BYTEA,
    ADD COLUMN logo_content_type TEXT;
