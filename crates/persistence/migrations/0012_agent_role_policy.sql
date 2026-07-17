ALTER TABLE agent_roles
    ADD COLUMN owner_account_id UUID REFERENCES human_accounts(id),
    ADD COLUMN capabilities JSONB NOT NULL DEFAULT '["comment", "request_spec", "discover", "complete", "release"]'::jsonb;
