ALTER TABLE outbox_messages
    ADD COLUMN dead_lettered_at TIMESTAMPTZ;

DROP INDEX outbox_messages_pending_idx;

CREATE INDEX outbox_messages_pending_idx
    ON outbox_messages (available_at, id)
    WHERE delivered_at IS NULL AND dead_lettered_at IS NULL;
