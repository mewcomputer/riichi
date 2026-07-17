#!/usr/bin/env bash
set -euo pipefail

: "${RIICHI_DATABASE_URL:?RIICHI_DATABASE_URL must be set}"
: "${RIICHI_ELECTRIC_SOURCE_SECRET:?RIICHI_ELECTRIC_SOURCE_SECRET must be set}"

port="${RIICHI_ELECTRIC_PORT:-5133}"
postgres_container="${RIICHI_POSTGRES_CONTAINER_NAME:-riichi-postgres}"
./scripts/start-electric.sh

if command -v psql >/dev/null 2>&1; then
  table_exists="$(psql "$RIICHI_DATABASE_URL" -Atc "SELECT to_regclass('public.human_issue_sync') IS NOT NULL")"
else
  database_name="${RIICHI_DATABASE_URL##*/}"
  database_name="${database_name%%\?*}"
  table_exists="$(docker exec "$postgres_container" psql -U postgres -d "$database_name" -Atc "SELECT to_regclass('public.human_issue_sync') IS NOT NULL")"
fi
if [[ "$table_exists" != "t" ]]; then
  echo "human_issue_sync is missing; run 'just migrate' before the Electric smoke test." >&2
  exit 1
fi

response_file="$(mktemp)"
trap 'rm -f "$response_file"' EXIT

curl --fail --silent --show-error --get "http://127.0.0.1:$port/v1/shape" \
  --data-urlencode 'table=human_issue_sync' \
  --data-urlencode 'where=account_id = $1' \
  --data-urlencode 'params[1]=00000000-0000-0000-0000-000000000000' \
  --data-urlencode 'offset=-1' \
  --data-urlencode "secret=$RIICHI_ELECTRIC_SOURCE_SECRET" >"$response_file"

if ! rg -q 'up-to-date|snapshot-end|insert|update|delete' "$response_file"; then
  echo "Electric returned an unexpected shape response" >&2
  cat "$response_file" >&2
  exit 1
fi

echo "Electric health and human_issue_sync shape checks passed"
