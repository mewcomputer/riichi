#!/usr/bin/env bash
set -euo pipefail

: "${RIICHI_DATABASE_URL:?RIICHI_DATABASE_URL must be set}"
: "${RIICHI_ELECTRIC_URL:?RIICHI_ELECTRIC_URL must be set}"
: "${RIICHI_ELECTRIC_SOURCE_SECRET:?RIICHI_ELECTRIC_SOURCE_SECRET must be set}"
: "${RIICHI_ELECTRIC_ACCOUNT_ID:?RIICHI_ELECTRIC_ACCOUNT_ID must be set}"

postgres_query() {
    if command -v psql >/dev/null 2>&1; then
        psql "$RIICHI_DATABASE_URL" -Atc "$1"
        return
    fi

    : "${RIICHI_POSTGRES_CONTAINER_NAME:=riichi-postgres}"
    database_name="${RIICHI_DATABASE_URL##*/}"
    database_name="${database_name%%\?*}"
    docker exec "$RIICHI_POSTGRES_CONTAINER_NAME" \
        psql -U postgres -d "$database_name" -Atc "$1"
}

electric_url="${RIICHI_ELECTRIC_URL%/}"
shape_headers="$(mktemp)"
shape_body="$(mktemp)"
trap 'rm -f "$shape_headers" "$shape_body"' EXIT

health="$(curl --fail --silent --show-error "$electric_url/v1/health")"
health_status="$(printf '%s' "$health" | sed -n 's/.*"status":"\([^"]*\)".*/\1/p')"
if [[ "$health_status" != "active" ]]; then
    echo "Electric health check returned an unexpected status: ${health:-empty}" >&2
    exit 1
fi

started_at="$(date +%s%N)"
curl --fail --silent --show-error --get "$electric_url/v1/shape" \
    -D "$shape_headers" \
    --data-urlencode 'table=human_issue_sync' \
    --data-urlencode 'where=account_id = $1' \
    --data-urlencode "params[1]=$RIICHI_ELECTRIC_ACCOUNT_ID" \
    --data-urlencode 'offset=-1' \
    --data-urlencode "secret=$RIICHI_ELECTRIC_SOURCE_SECRET" \
    -o "$shape_body"
finished_at="$(date +%s%N)"

header_value() {
    awk -F': ' -v name="$1" 'tolower($1) == tolower(name) { sub(/\r$/, "", $2); print $2; exit }' "$shape_headers"
}

shape_rows="$({ rg -o '"value":' "$shape_body" || true; } | wc -l | tr -d ' ')"
shape_bytes="$(wc -c < "$shape_body" | tr -d ' ')"
latency_ms="$(( (finished_at - started_at) / 1000000 ))"
shape_up_to_date="$(header_value electric-up-to-date)"
shape_up_to_date="${shape_up_to_date:-unknown}"
shape_has_data="$(header_value electric-has-data)"
replication_slots="$(postgres_query \
    "SELECT COALESCE(json_agg(json_build_object(
       'slot_name', slot_name,
       'active', active,
       'pending_wal_bytes', GREATEST(0, COALESCE(pg_wal_lsn_diff(pg_current_wal_lsn(), confirmed_flush_lsn), 0))
     )), '[]'::json)::text
     FROM pg_replication_slots
     WHERE slot_name LIKE 'electric_slot_%'")"

cat <<EOF
electric_health=$health_status
shape_table=human_issue_sync
shape_account_id=$RIICHI_ELECTRIC_ACCOUNT_ID
shape_rows=$shape_rows
shape_bytes=$shape_bytes
shape_latency_ms=$latency_ms
shape_offset=$(header_value electric-offset)
shape_handle=$(header_value electric-handle)
shape_up_to_date=$shape_up_to_date
shape_has_data=$shape_has_data
electric_replication_slots=$replication_slots
EOF
