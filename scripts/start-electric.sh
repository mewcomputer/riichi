#!/usr/bin/env bash
set -euo pipefail

: "${RIICHI_DATABASE_URL:?RIICHI_DATABASE_URL must be set}"
: "${RIICHI_ELECTRIC_SOURCE_SECRET:?RIICHI_ELECTRIC_SOURCE_SECRET must be set}"

container="${RIICHI_ELECTRIC_CONTAINER_NAME:-riichi-electric}"
port="${RIICHI_ELECTRIC_PORT:-5133}"
postgres_container="${RIICHI_POSTGRES_CONTAINER_NAME:-riichi-postgres}"
stream_id="${RIICHI_ELECTRIC_STREAM_ID:-$container}"
stream_id="$(printf '%s' "$stream_id" | tr -c '[:alnum:]_' '_')"
volume="${RIICHI_ELECTRIC_DATA_VOLUME:-riichi-electric-data-$stream_id}"

if ! docker container inspect "$postgres_container" >/dev/null 2>&1; then
  echo "Postgres container '$postgres_container' does not exist. Run 'just start-db' first." >&2
  exit 1
fi

wal_level="$(docker exec "$postgres_container" psql -U postgres -d postgres -Atc 'SHOW wal_level')"
if [[ "$wal_level" != "logical" ]]; then
  echo "Postgres requires wal_level=logical for Electric; run 'just start-db' to configure it." >&2
  exit 1
fi

database_url="${RIICHI_ELECTRIC_DATABASE_URL:-$RIICHI_DATABASE_URL}"
database_url="${database_url//127.0.0.1/host.docker.internal}"
database_url="${database_url//localhost/host.docker.internal}"

if docker container inspect "$container" >/dev/null 2>&1; then
  docker start "$container" >/dev/null 2>&1 || true
else
  docker run --name "$container" --add-host=host.docker.internal:host-gateway \
    -e "DATABASE_URL=$database_url" \
    -e "ELECTRIC_PORT=$port" \
    -e "ELECTRIC_SECRET=$RIICHI_ELECTRIC_SOURCE_SECRET" \
    -e "ELECTRIC_REPLICATION_STREAM_ID=$stream_id" \
    -e ELECTRIC_STORAGE_DIR=/var/lib/electric \
    -v "$volume:/var/lib/electric" \
    -p "$port:$port" -d electricsql/electric:latest >/dev/null
fi

for _ in {1..60}; do
  status="$(curl --silent --output /tmp/riichi-electric-health --write-out '%{http_code}' "http://127.0.0.1:$port/v1/health" || true)"
  if [[ "$status" == "200" ]]; then
    rm -f /tmp/riichi-electric-health
    echo "Electric is ready at http://127.0.0.1:$port"
    exit 0
  fi
  sleep 1
done

echo "Electric did not become ready; inspect logs with 'docker logs $container'" >&2
docker logs --tail 40 "$container" >&2 || true
rm -f /tmp/riichi-electric-health
exit 1
