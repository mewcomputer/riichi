# AGENTS.md

Guide for AI agents (and humans) working in this repository.

## What this is

Riichi is an agent dispatch platform — a system where AI agents pick up work
from a shared issue queue, claim leases, and report progress under fencing
constraints. The frontend is a React SPA; the backend is a Rust workspace
with an axum API server, a background worker, and an agent CLI.

## Repository layout

```
.
├── apps/
│   ├── api/           Rust — axum HTTP API server (OpenAPI, SSE agent protocol)
│   ├── agent-cli/     Rust — CLI tool for the agent dispatch protocol
│   ├── worker/        Rust — outbox message processor (lease/event delivery)
│   └── web/           TypeScript — React SPA (Vite, TanStack Router/Query, Tiptap)
├── crates/
│   ├── domain/         Rust — domain types (IssueStatus, IssueId, errors)
│   ├── application/   Rust — business logic layer (Application struct wrapping Database)
│   ├── persistence/   Rust — PostgreSQL data layer (sqlx, migrations, models)
│   ├── auth/           Rust — OIDC authentication (Pocket ID)
│   ├── integrations-github/  Rust — GitHub API client + webhook parsing
│   └── storage/        Rust — shared local/S3-compatible attachment storage
├── notes/             Product and architecture docs (PRD, RFC, runbooks)
├── scripts/           Shell scripts (API type gen, e2e test runners, metrics SQL)
├── justfile           Task runner — primary entry point for all commands
├── Cargo.toml          Rust workspace root (edition 2024, resolver 3)
├── package.json        pnpm workspace root
├── pnpm-workspace.yaml
├── rust-toolchain.toml  (Rust 1.94.0, rustfmt + clippy)
└── HISTORY.md         Changelog of notable changes
```

## Tech stack

**Backend (Rust):** axum, sqlx (PostgreSQL), tokio, chrono, serde, uuid v7,
tower-http, tracing. Edition 2024, `unsafe_code = "forbid"` workspace-wide.

**Frontend (TypeScript):** React 19, Vite 8, TanStack Router + Query,
Tiptap v3 (rich text), Tailwind CSS v4, shadcn/ui, lucide-react icons.

**Database:** PostgreSQL 16. Migrations in `crates/persistence/migrations/`.

**Auth:** OIDC via Pocket ID. Session cookies, role-based access
(owner / admin / member / viewer).

## Prerequisites

- Rust 1.94.0 (pinned in `rust-toolchain.toml`)
- Node 22 + pnpm 10.33.0
- Docker (for local Postgres and testcontainers-based e2e tests)

## Common commands

All commands are defined in the `justfile`. Use `just` as the primary entry point.

| Command | What it does |
|---|---|
| `just install` | `pnpm install` |
| `just start-db` | Start or reuse a local Postgres 16 Docker container |
| `just start-api` | `cargo run --bin riichi-api` |
| `just start-worker` | `cargo run --bin riichi-worker` |
| `just start-web` | Vite dev server with API proxy to `:3000` |
| `just start-electric` | Start local Electric against the Docker Postgres |
| `just test-electric-local` | Health and real-shape smoke test for local Electric |
| `just start` | Start db, api, worker, and web in parallel |
| `just check` | `cargo fmt --check` + `cargo check --workspace` + `pnpm typecheck` |
| `just test` | `cargo test --workspace` + `pnpm test` (vitest) |
| `just test-e2e` | Rust integration e2e tests (testcontainers Postgres) |
| `just test-browser-e2e` | Playwright browser tests |
| `just test-populated-restore` | Restore a dataful disposable PostgreSQL fixture and verify projections/metrics |
| `just test-s3-storage` | Real MinIO S3-compatible attachment round trip and restore |
| `just generate-api` | Regenerate OpenAPI JSON + TypeScript API types |
| `just test-release` | Full pre-merge gate (fmt, clippy, all tests, build) |
| `just pilot-metrics` | Run pilot metrics SQL against the database |

Equivalent direct commands:

```bash
# Rust
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace -- --ignored --test-threads=1   # e2e tests

# Frontend
pnpm run typecheck     # tsc --noEmit
pnpm run test          # vitest run
pnpm run test:watch    # vitest watch
pnpm run test:e2e:browser  # playwright

# API type generation (requires Rust build)
pnpm run generate:api
```

## Architecture

### Backend layers

```
apps/api (HTTP)  →  crates/application (business logic)  →  crates/persistence (sqlx/Postgres)
apps/worker      →  crates/application                    →  crates/persistence
apps/agent-cli   →  apps/api (via HTTP/SSE)
```

- **`crates/domain`** — Core enums and value objects. No dependencies on other
  crates. Defines `IssueStatus`, `IssueId`, `DomainError`.
- **`crates/persistence`** — All SQL. The `Database` struct wraps a `PgPool`.
  Migrations are embedded via `sqlx::migrate!()`. Models are `sqlx::FromRow`
  structs. Key modules: `dispatch.rs` (claim/lease/renew/report), `auth.rs`,
  `github.rs`, `triage.rs`, `context.rs`, `controls.rs`, `collaborators.rs`.
- **`crates/application`** — Thin orchestration layer. `Application` struct
  holds a `Database` clone and delegates to persistence methods. No HTTP
  concerns here.
- **`apps/api`** — axum router with routes for issues, agents, auth, GitHub,
  navigation, projects. Serves OpenAPI spec at `/openapi.json` (via the
  `openapi` binary). Agent protocol uses SSE for real-time dispatch.
- **`apps/worker`** — Polls the outbox table and delivers messages (lease
  changes, issue changes) to subscribers. Exponential backoff with
  `MAX_DELIVERY_ATTEMPTS = 5`.
- **`crates/auth`** — OIDC flow: login state, token exchange, session cookies.
- **`crates/integrations-github`** — GitHub REST client, webhook payload
  parsing, issue snapshot upserts.
- **`crates/storage`** — Shared attachment object-store boundary. Local
  filesystem storage is the default; S3-compatible storage uses the
  `RIICHI_ATTACHMENT_*` variables.

### Frontend structure

```
apps/web/src/
├── routes/              TanStack Router page components (queue, issue-detail, agents, etc.)
├── components/
│   ├── ui/              shadcn/ui primitives (button, input, dialog, etc.)
│   ├── issues/          Issue-specific components (rich-text editors, status menu)
│   ├── queue/           Queue list, toolbar, command menu
│   ├── project/         Project shell, sidebar, header
│   └── team/            Team-specific components
├── hooks/               React Query hooks (use-all-issues, use-active-team, etc.)
├── lib/
│   ├── api.ts           Typed API client (hand-written, wraps fetch)
│   ├── generated/       Auto-generated OpenAPI types (do not edit)
│   ├── organization-slug.ts
│   └── utils.ts
├── data/                Queue data transformations
└── index.css            Global styles, theme variables, Tiptap placeholder CSS
```

- Path alias: `@/` maps to `apps/web/src/`.
- Vite dev server proxies `/api` and `/auth` to the Rust API at `:3000`.
- API types are generated from the Rust OpenAPI spec via
  `scripts/generate-api-types.sh`. The CI `contracts` job verifies these
  are up to date — always run `just generate-api` after changing API
  shapes and commit the result.

### Key domain concepts

- **Issues** — Work items with status (triage → todo → in_progress → done/canceled),
  rank, version (optimistic concurrency), labels, edges, holds.
- **Leases** — Time-bounded ownership of an issue by an agent session. Fenced
  with a monotonically increasing `fencing_token` for optimistic concurrency.
- **Claims** — Idempotent lease acquisition (hash of issue_id + requested TTL).
- **Reports** — Agents report progress back against an active lease. Supports
  batch reports.
- **Holds** — Manual or automatic blocks on dispatch (needs_spec,
  awaiting_approval, scheduled, integration, manual).
- **Edges** — Issue relationships (blocks, related, discovered_from, duplicate_of).
- **Approvals** — Proposed operations (e.g., rank changes) requiring human
  approval before execution.
- **Collaborators** — Bounded capability grants under a lease fence
  (comment, discover, complete, edit_issue, etc.).
- **Recovery / Quarantine** — When an agent fails, a takeover creates a
  recovery checklist; failed attempts are quarantined for review.
- **Teams / Organizations** — Multi-tenant hierarchy. Teams own issues with
  sequence-based display keys (e.g., `ENG-123`).

## Testing

### Rust tests

- **Unit tests** (`cargo test --workspace`): Embedded in each crate via
  `#[cfg(test)]` modules.
- **Integration tests** (`--ignored`): In `crates/persistence/tests/` and
  `apps/api/tests/`. These use `@testcontainers/postgresql` to spin up a real
  Postgres. Run with `--ignored --test-threads=1`.
- **E2e scripts** (`just test-e2e`): Run the `queue_e2e`, `triage_e2e`, and
  `github_e2e` test suites.

### Frontend tests

- **Vitest** (`pnpm test`): Unit/component tests in `src/**/*.{test,spec}.ts`.
  JSDOM environment, setup file at `src/test/setup.ts`.
- **Playwright** (`pnpm run test:e2e:browser`): Browser e2e tests in
  `tests/e2e/`. Chromium only, serial execution, 120s timeout.

## CI (GitHub Actions)

Defined in `.github/workflows/test.yml`. Two jobs:

1. **rust** — fmt check, clippy (-D warnings), unit tests, e2e tests
   (with Postgres 16 service container), pilot metrics SQL.
2. **contracts** — Regenerates API types and verifies they match what's
   committed (`git diff --exit-code`).

## Conventions

- **Rust:** `unsafe_code` is forbidden workspace-wide. UUIDs use v7
  (time-sortable). All DB operations go through `sqlx` prepared statements.
  Audit records and outbox messages are written in the same transaction as
  the operation they record.
- **Frontend:** TypeScript strict mode. Components use shadcn/ui patterns.
  API calls go through `lib/api.ts` (hand-written, not auto-generated code —
  only the types in `lib/generated/api.d.ts` are generated). Rich text uses
  Tiptap with `StarterKit`.
- **Migrations:** Sequential numbered SQL files in
  `crates/persistence/migrations/`. Never edit an applied migration — add a
  new one.
- **Environment:** Copy `.env.example` to `.env`. The justfile loads `.env`
  via `set dotenv-load := true`.

## Environment variables

See `.env.example` for all configuration:

- `RIICHI_API_ADDR` — API listen address (default `127.0.0.1:3000`)
- `RIICHI_DATABASE_URL` — Postgres connection string
- `RIICHI_DATABASE_MAX_CONNECTIONS` — Pool size
- `RIICHI_OIDC_*` — OIDC provider config (issuer, client ID/secret, redirect URL)
- `RIICHI_AUTH_COOKIE_SECURE` — Set `true` for HTTPS deployments
- `RIICHI_GITHUB_*` — GitHub token, API base URL, webhook secret, project ID

## Notes directory

`notes/` contains product and architecture documentation that is intentionally
separate from implementation:

- `riichi-pilot-prd.md` — Pilot product requirements
- `riichi-pilot-architecture-rfc.md` — Architecture decisions and boundaries
- `riichi-write-boundary-sync-rfc.md` — Collaborative document and metadata sync boundary
- `riichi-write-boundary-compatibility.md` — Current schema and rollout compatibility evidence
- `pilot-runbook.md` — Operations runbook
- `pilot-instrumentation.md` — Observability setup
- `project-hierarchy.md` — Team/org/project model

Do not move these into the codebase — they are working notes, not shipped docs.
