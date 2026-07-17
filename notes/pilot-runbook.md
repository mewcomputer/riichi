# riichi pilot runbook

## local startup

1. Start the development PostgreSQL container and set `RIICHI_DATABASE_URL`.
2. Set the Pocket ID-compatible OIDC variables from `.env.example`.
3. Run `just migrate` once when using a database without an API startup.
4. Run `just start-riichi-api` and `just start-riichi-worker`.
5. Run `just start-riichi-web` for the browser. `just start-riichi` starts all three processes in parallel.

For GitHub import, set `RIICHI_GITHUB_TOKEN` and optionally
`RIICHI_GITHUB_API_BASE_URL` for a compatible test server. An admin can import a
bounded issue snapshot set with `POST
/api/v1/workspaces/{workspace_id}/integrations/github/import`. Configure
`RIICHI_GITHUB_WEBHOOK_SECRET` and `RIICHI_GITHUB_WORKSPACE_ID` before enabling
the signed webhook route.

Attachment bytes use `RIICHI_ATTACHMENT_BACKEND=local` during development. The
API and worker share the same storage contract. For an S3-compatible provider,
set the S3 endpoint, bucket, region, credentials, and
`RIICHI_ATTACHMENT_S3_ALLOW_HTTP` as shown in `.env.example`; keep credentials
in deployment secret injection. The populated restore drill covers local
files. `just test-s3-storage` runs the same contract against a real MinIO
container and mirrors a fixture object out and back before checking its bytes.
For a managed S3-compatible deployment, set the attachment S3 endpoint,
bucket, region, and credentials, then run `just test-managed-s3-storage`. The
check refuses to create a managed bucket, writes only to a unique temporary
prefix, mirrors that prefix out and back, and runs the real storage contract.
Remove the temporary prefix after a failed run if the provider does not allow
the cleanup request. The populated restore drill must still be run against the
same managed store and verify every ready attachment's byte size and checksum
before reopening writes.

The API applies repository migrations during startup. A deploy must keep the API and worker on the same migration version before accepting traffic.

## verification

- `cargo fmt --all -- --check`
- `cargo check --workspace`
- `cargo test --workspace`
- `pnpm run typecheck && pnpm run test && pnpm run build`
- `./scripts/test-e2e.sh`
- `./scripts/test-browser-e2e.sh`
- `just verify-projections`
- `just backup`
- `RIICHI_RESTORE_DATABASE_URL=... just restore-verify path/to/backup.dump`

The E2E scripts select the active Docker context. Run them through the scripts instead of assuming `/var/run/docker.sock` exists.

## routine recovery

1. Open the issue detail and confirm the active lease, expiry, and issue version.
2. Use takeover only when the agent session is no longer trusted to continue. Record a concrete reason.
3. Follow the recovery checklist. `reopen_for_dispatch` returns the issue to `todo`; `complete_with_summary` moves it to `done` and records the required resolution summary. Leaving the checklist open means investigation is continuing.
4. Owners and admins may inspect quarantined payloads. Ordinary team members see only that quarantined data exists. Agent collaborators require an explicit `recovery_review` grant, and revoked lease owners receive no automatic access.
5. Confirm the old lease is revoked and the fencing token has advanced.
6. If the browser missed the SSE hint, reload or use the queue refresh action. SSE is a refetch hint, never the authority.

For a quarantined stale report, confirm the issue's quarantine count first. Full
payload access is restricted to admins or agents with an explicit
`recovery_review` collaborator grant. Do not replay a quarantined payload by
copying it into an ordinary report without reviewing its lease and target
version.

## delivery inspection

The outbox is authoritative for pending side effects. A delivered outbox row means the worker copied the event into the durable delivery buffer. It does not prove that a browser received an SSE event.

When delivery is stuck, inspect the message type, attempt count, `available_at`, `last_error`, and dead-letter state. Redrive only after correcting the underlying failure. Do not manually mutate issue dispatch state to make a notification appear.

Every HTTP response includes `x-request-id`. Preserve it with the incident report; it matches the request span in the API logs. Worker logs use the outbox message ID as the durable job correlation ID.

## incident boundaries

- A stale agent report must fail on lease or fencing validation and must not change issue state.
- A cross-workspace identifier must behave as not found or forbidden without revealing the other workspace.
- A context response must stay within its server budget and declare omitted or truncated sections.
- External GitHub text is untrusted content. It must never change workspace policy, permissions, or dispatch state automatically.

## pilot data safety

Back up PostgreSQL before migration or recovery exercises. Keep raw webhook and quarantined-attempt retention bounded. Secrets belong in environment injection or the future Vault integration, never in issue bodies, comments, context, audit summaries, or ordinary logs.

Run `just retention` for the default 90-day cleanup of raw webhook receipts and quarantined attempt payloads. Run `just verify-projections` after restore and migration exercises before reopening writes.

When metadata replication is enabled, set `RIICHI_ELECTRIC_URL` and keep
`RIICHI_ELECTRIC_SOURCE_SECRET` server-side. The browser flag
`VITE_ELECTRIC_SYNC_ENABLED=true` uses the authenticated project-scoped proxy;
the TanStack Query path remains the fallback during rollout. Verify the shape
with the queue read model before enabling it for a pilot.

For local Electric, run `just start-db`, `just migrate`, and then
`just start-electric`. The database container must report `wal_level=logical`;
`just start-db` configures and restarts an existing local container when needed.
`just test-electric-local` checks Electric readiness and requests the real
`human_issue_sync` shape with the configured server-side secret. The command
uses a persistent Docker volume named from the Electric stream ID, so remove
that volume and the matching Postgres publication/slot together if the database
is reset or the Electric database changes.

For a deployment-shaped replication baseline, run
`just electric-observability <account-id>` with the Electric URL, source secret,
and database URL injected. The probe checks Electric health, requests the
account-scoped issue shape, records snapshot rows, response bytes, latency,
offset, handle, and up-to-date state, and reports pending WAL bytes for Electric
replication slots. Save the output with the deployment and timestamp; repeat it
after a migration, permission change, and representative metadata mutation.

For the write-boundary rollout, retain these artifacts before enabling the v2
editor for pilot traffic:

- the real mobile IME exercise covering composition, paste, reconnect, and
  persistence;
- the target Electric probe output before and after a permission revocation,
  plus signed-in browser replication and API fallback evidence;
- the populated PostgreSQL restore and object-prefix restore against the
  managed attachment store, with every ready attachment's size and checksum
  compared before writes reopen; and
- the older-client migration exercise proving the bounded read-only path and
  rollback behavior across the v1/v2 barrier.

The local commands prove the repository and deployment shape, but do not
substitute for these target-client and target-deployment artifacts.

Active Electric streams are authorization-bound. Membership and human-session
changes publish account-scoped access events, and the API closes matching shape
streams immediately. The proxy also rechecks the session and any project scope
once per second and fails closed on denial or an authorization query error. The
real PostgreSQL test `electric_project_shape_closes_after_membership_revocation`
in `apps/api/tests/electric_proxy_e2e.rs` verifies this behavior; run it with
the repository's ignored-test Docker setup when changing the sync boundary.

The document boundary adds three useful restore checks: accepted document
activity still has a live document, document projections are not behind their
Loro snapshot, and every issue has a current metadata-sync row. A successful
`restore-verify` run is required before reopening writes after a restore.
For a dataful drill that includes issues, documents, Loro snapshots and
updates, activity, attachments, approvals, notifications, and sync rows, run
`just test-populated-restore`. It creates two disposable PostgreSQL containers,
restores a custom-format dump into the second, and runs projection verification
and pilot metrics before removing both containers. It also imports the restored
snapshot and update with the real Loro runtime, and archives/restores a local
attachment fixture while checking its bytes against the restored checksum. This
covers the local filesystem backend. An S3-compatible deployment still requires
its own object-storage backup and restore drill.

Attachment cleanup is a durable two-step handoff. The worker claims expired
pending uploads, removes the file, and then finalizes the metadata row. A
worker interruption leaves the claim reclaimable after five minutes. Include
`pending_attachment_uploads`, `expired_attachment_uploads`, and
`stale_attachment_cleanup_claims` from `scripts/pilot-metrics.sql` in the pilot
baseline, and verify the object-storage backup separately from PostgreSQL.
