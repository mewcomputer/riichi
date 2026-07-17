ALTER TABLE delivery_events
    ADD COLUMN event_seq BIGINT GENERATED ALWAYS AS IDENTITY;

CREATE UNIQUE INDEX delivery_events_project_sequence_idx
    ON delivery_events (project_id, event_seq);
