# write-boundary compatibility matrix

This is the current phase-zero compatibility record for the document boundary.
It distinguishes behavior covered by automated tests from behavior that needs a
future client or rollout exercise.

| Surface | Current contract | Evidence | State |
| --- | --- | --- | --- |
| Browser rich text | Tiptap/ProseMirror schema with paragraphs, headings, bullet and ordered lists, task lists, blockquotes, code blocks, hard breaks, horizontal rules, links, mentions, marks, and attachment images | `apps/web/src/lib/loro-document.test.ts`, `crates/application/src/loro_document.rs` | covered |
| Native projection | The same schema round-trips through Loro and produces deterministic plain text and sanitized HTML | `round_trips_the_current_document_editor_schema`, native projection tests | covered |
| Block paste shape | A pasted structured block is accepted inside the Loro editor boundary | `keeps_block-shaped_paste_content_and_undo_inside_the_loro_boundary`, `apps/web/tests/e2e/queue.spec.ts` real Chromium clipboard test | covered for native and real browser clipboard paths; mobile-device paste remains a rollout check |
| Undo | Block insertion can be undone without leaving the Loro boundary | `keeps_block-shaped_paste_content_and_undo_inside_the_loro_boundary` | covered |
| Browser persistence | Snapshots and pending updates survive reopening and interrupted snapshot persistence | `apps/web/src/lib/loro-document.test.ts` | covered |
| Shallow snapshot recovery | A shallow Loro snapshot restores the current projection, records its retention frontier, and accepts new updates; schema migration archives the prior full snapshot for recovery | `crates/application/src/loro_document.rs`, `apps/api/tests/auth_http.rs` | native recovery and populated PostgreSQL history covered |
| Reconnect | In-flight updates are requeued after a dropped socket and sent after reconnect | browser reconnect test and API WebSocket test | covered |
| Server restart | A fresh API sync registry reloads the durable snapshot and accepts a previously used peer ID after the old server is gone | `configured_api_supports_oidc_cookie_project_and_invite_round_trip` | covered |
| Peer identity | Active duplicate peer IDs are rejected per document; missing, non-numeric, and oversized IDs fail closed | API WebSocket integration coverage | covered |
| Permission revocation | An active socket receives a terminal authorization error after its document membership is revoked, and the browser purges that document's local snapshot and pending updates; reconnect performs an authorization preflight before opening a new socket | API WebSocket integration coverage, `purges_the_scoped_local_document_when_the_server_revokes_access`, `purges_local_data_when_reconnect_authorization_is_revoked_while_offline` | covered in the client and local API path; target-deployment revocation telemetry remains a rollout check |
| Schema versioning | Durable snapshots expose v1 or v2; v2 validates bounded callouts and deterministically migrates v1 blockquotes; WebSocket hello and HTTP updates reject incompatible versions; local IndexedDB snapshots are cleared or rejected safely; the v2 editor recognizes callouts and the migration can roll back atomically | `apps/api/tests/auth_http.rs`, API WebSocket integration coverage, `apps/web/src/lib/loro-document.test.ts`, native validation tests | v2 is the default for new documents and browser sessions; existing v1 documents remain explicitly versioned until migrated |
| Attachment bytes | Local attachment storage can be archived and restored alongside the PostgreSQL fixture; S3-compatible storage can round-trip and restore an object prefix; restored bytes must match the durable size and checksum metadata | `just test-populated-restore`, `just test-s3-storage` | local filesystem and MinIO-compatible API covered; managed object-store deployment remains a rollout check |
| Mobile composition | No mobile Riichi client exists yet | N/A | pre-mobile-client gate |
| Cross-version clients | No older client/server pair is shipped yet | N/A | rollout gate |

The matrix is intentionally conservative. Passing the browser binding and
native projection tests does not establish support for mobile IME composition
or mixed-version deployments. The Chromium clipboard test proves the browser
path, not mobile clipboard or IME behavior.
