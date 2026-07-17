-- Bound raw integration and quarantined payload retention. Run with psql.
\set ON_ERROR_STOP on
\if :{?retention_days}
\else
\set retention_days 90
\endif

SELECT (:'retention_days'::integer > 0) AS retention_days_valid \gset
\if :retention_days_valid
\else
\error 'retention_days must be a positive integer'
\endif

BEGIN;
DELETE FROM webhook_deliveries
 WHERE received_at < now() - (:'retention_days' || ' days')::interval;
DELETE FROM quarantined_attempts
 WHERE created_at < now() - (:'retention_days' || ' days')::interval;
DELETE FROM delivery_events
 WHERE created_at < now() - (:'retention_days' || ' days')::interval;
DELETE FROM outbox_messages
 WHERE delivered_at IS NOT NULL
   AND delivered_at < now() - (:'retention_days' || ' days')::interval;
DELETE FROM notifications
 WHERE read_at IS NOT NULL
   AND read_at < now() - (:'retention_days' || ' days')::interval;
DELETE FROM idempotency_records
 WHERE created_at < now() - (:'retention_days' || ' days')::interval;
COMMIT;
