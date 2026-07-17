CREATE TABLE document_activity (
    id UUID PRIMARY KEY,
    document_id UUID NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
    update_id UUID NOT NULL UNIQUE REFERENCES document_loro_updates(update_id) ON DELETE CASCADE,
    actor_id UUID NOT NULL REFERENCES human_accounts(id),
    source TEXT NOT NULL,
    previous_frontiers JSONB NOT NULL,
    resulting_frontiers JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX document_activity_document_time_idx
    ON document_activity (document_id, created_at, id);
