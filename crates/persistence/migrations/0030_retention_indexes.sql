CREATE INDEX outbox_messages_delivered_at_idx
    ON outbox_messages (delivered_at)
    WHERE delivered_at IS NOT NULL;

CREATE INDEX idempotency_records_created_at_idx
    ON idempotency_records (created_at);
