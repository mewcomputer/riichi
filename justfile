set dotenv-load := true

default:
    @just --list

install:
    pnpm install

generate-api:
    pnpm run generate:api

migrate:
    cargo run --bin riichi-migrate

start-riichi-api:
    cargo run --bin riichi-api

start-riichi-worker:
    cargo run --bin riichi-worker

start-riichi-web:
    RIICHI_WEB_PROXY_TARGET=http://127.0.0.1:3000 pnpm dev --host

start-api: start-riichi-api

start-worker: start-riichi-worker

start-web: start-riichi-web

start-db:
    if docker container inspect riichi-postgres >/dev/null 2>&1; then docker start riichi-postgres >/dev/null 2>&1 || true; else docker run --name riichi-postgres -e POSTGRES_PASSWORD=password -e POSTGRES_DB=riichi -p 5432:5432 -d postgres:16 -c wal_level=logical; fi
    ./scripts/configure-local-postgres.sh

start-electric:
    ./scripts/start-electric.sh

stop-electric:
    docker stop "$${RIICHI_ELECTRIC_CONTAINER_NAME:-riichi-electric}" >/dev/null 2>&1 || true

test-electric-local:
    ./scripts/test-electric-local.sh

electric-observability account_id:
    RIICHI_ELECTRIC_ACCOUNT_ID="{{account_id}}" ./scripts/electric-observability.sh

test-s3-storage:
    ./scripts/test-s3-storage.sh

test-managed-s3-storage:
    RIICHI_S3_TEST_MANAGED=true ./scripts/test-s3-storage.sh

start: start-db
    just --parallel start-riichi-api start-riichi-worker start-riichi-web

check:
    cargo fmt --all -- --check
    cargo check --workspace
    pnpm run typecheck

test:
    cargo test --workspace
    pnpm run test

test-e2e:
    ./scripts/test-e2e.sh

test-browser-e2e:
    ./scripts/test-browser-e2e.sh

pilot-metrics:
    psql "$RIICHI_DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/pilot-metrics.sql

backup output="riichi-{{`date -u +%Y%m%dT%H%M%SZ`}}.dump":
    ./scripts/backup.sh "{{output}}"

verify-projections:
    psql "$RIICHI_DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/verify-projections.sql

retention days="90":
    psql "$RIICHI_DATABASE_URL" -v ON_ERROR_STOP=1 -v retention_days="{{days}}" -f scripts/retention.sql

restore-verify dump:
    ./scripts/restore-verify.sh "{{dump}}"

test-populated-restore:
    ./scripts/test-populated-restore.sh

test-release:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    cargo test --workspace -- --ignored --test-threads=1
    pnpm run typecheck
    pnpm run test
    pnpm run build
