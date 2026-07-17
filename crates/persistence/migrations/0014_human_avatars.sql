ALTER TABLE human_accounts
    ADD COLUMN avatar_bytes BYTEA,
    ADD COLUMN avatar_content_type TEXT;
