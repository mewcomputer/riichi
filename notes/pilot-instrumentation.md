# pilot instrumentation

The API applies a request trace layer to every route. Each request records the route, status, elapsed time, and an `x-request-id` correlation value through the configured `tracing` subscriber. The agent intention paths are therefore observable as `/api/v1/ready`, `/api/v1/claim`, `/api/v1/context`, and `/api/v1/report/batch` without adding a second queue or metrics service.

Run the API with `RUST_LOG=info` during a pilot exercise. Capture these fields in the log sink:

- request route and method;
- response status;
- request latency;
- workspace and session identifiers where the handler logs them;
- rejection code for typed API failures;
- outbox message type and attempt count for delivery failures.

The first pilot review should graph p50 and p95 latency for the four agent intentions, rejection counts by API error code, claim operations, stale-report rejections, takeovers, pending outbox depth, dead-letter count, active leases, and issues with active holds. The authoritative sources are the API trace stream and these PostgreSQL projections:

```sql
SELECT message_type, count(*)
FROM outbox_messages
WHERE delivered_at IS NULL
GROUP BY message_type;

SELECT state, count(*)
FROM leases
GROUP BY state;

SELECT count(*)
FROM dispatch_holds
WHERE released_at IS NULL
  AND (expires_at IS NULL OR expires_at > now());

SELECT count(*)
FROM quarantined_attempts
WHERE created_at >= now() - interval '24 hours';

SELECT state, count(*)
FROM webhook_deliveries
WHERE received_at >= now() - interval '24 hours'
GROUP BY state;
```

The trace layer is diagnostic only. Claims, reports, approvals, SSE delivery, and GitHub webhook acceptance remain authoritative in PostgreSQL transactions.

The document and metadata-sync boundary adds these baseline measures:

- accepted Loro updates and document activity over the last 24 hours;
- pending and failed document provisioning;
- document projection lag and orphaned bindings/projections;
- issue metadata-sync row count;
- account-scoped human document, agent-roster, and issue read-model row
  counts;
- Electric shape connections, transferred rows/bytes, replication lag, and
  acknowledgement time from the Electric deployment’s own telemetry;
- metadata optimistic rollback and acknowledgement-timeout counts from the web
  client.

The database-backed portion is in `scripts/pilot-metrics.sql`. Electric and
browser acknowledgement measures cannot be inferred safely from PostgreSQL
alone and must be collected from the sync service and client instrumentation.

The release-gate metrics query is available at `scripts/pilot-metrics.sql` and via
`just pilot-metrics`. It is intentionally read-only and reports the baseline
counts needed during a pilot exercise.
