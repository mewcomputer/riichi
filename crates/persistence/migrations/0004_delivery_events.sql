CREATE TABLE delivery_events (
    id UUID PRIMARY KEY REFERENCES outbox_messages(id),
    project_id UUID NOT NULL REFERENCES projects(id),
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX delivery_events_project_time_idx
    ON delivery_events (project_id, created_at, id);
