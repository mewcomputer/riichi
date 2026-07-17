#!/usr/bin/env bash
set -euo pipefail

container="${RIICHI_POSTGRES_CONTAINER_NAME:-riichi-postgres}"

if ! docker container inspect "$container" >/dev/null 2>&1; then
  echo "Postgres container '$container' does not exist. Run 'just start-db' first." >&2
  exit 1
fi

docker start "$container" >/dev/null 2>&1 || true
current="$(docker exec "$container" psql -U postgres -d postgres -Atc 'SHOW wal_level')"
if [[ "$current" == "logical" ]]; then
  exit 0
fi

echo "configuring $container for logical replication (wal_level=$current)"
docker exec "$container" psql -U postgres -d postgres -v ON_ERROR_STOP=1 \
  -c "ALTER SYSTEM SET wal_level = 'logical'" >/dev/null
docker restart "$container" >/dev/null

for _ in {1..30}; do
  if [[ "$(docker exec "$container" pg_isready -U postgres -d postgres 2>/dev/null || true)" == *"accepting connections"* ]]; then
    break
  fi
  sleep 1
done

current="$(docker exec "$container" psql -U postgres -d postgres -Atc 'SHOW wal_level')"
if [[ "$current" != "logical" ]]; then
  echo "Postgres did not restart with wal_level=logical" >&2
  exit 1
fi
