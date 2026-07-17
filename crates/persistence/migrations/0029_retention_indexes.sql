CREATE INDEX webhook_deliveries_received_at_idx
    ON webhook_deliveries (received_at);

CREATE INDEX quarantined_attempts_created_at_idx
    ON quarantined_attempts (created_at);

CREATE INDEX delivery_events_created_at_idx
    ON delivery_events (created_at);

CREATE INDEX audit_records_created_at_idx
    ON audit_records (created_at);

CREATE INDEX notifications_created_at_idx
    ON notifications (created_at);
