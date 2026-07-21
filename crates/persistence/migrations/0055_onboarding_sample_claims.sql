CREATE TABLE onboarding_sample_claims (
    project_id UUID PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
