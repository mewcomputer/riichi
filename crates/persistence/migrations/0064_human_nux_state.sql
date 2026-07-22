ALTER TABLE human_accounts
    ADD COLUMN last_completed_nux_version TEXT,
    ADD COLUMN last_completed_nux_at TIMESTAMPTZ;
