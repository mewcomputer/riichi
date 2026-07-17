#!/usr/bin/env bash
set -euo pipefail

source_container="riichi-restore-source-$PPID"
restore_container="riichi-restore-target-$PPID"
dump_dir="$(mktemp -d "${TMPDIR:-/tmp}/riichi-restore.XXXXXX")"
dump_path="$dump_dir/fixture.dump"
attachment_backup="$dump_dir/attachments.tar"
source_attachment_root="$dump_dir/source-attachments"
restore_attachment_root="$dump_dir/restored-attachments"

cleanup() {
    docker rm -f "$source_container" "$restore_container" >/dev/null 2>&1 || true
    rm -rf "$dump_dir"
}
trap cleanup EXIT

start_database() {
    local container="$1"
    docker run --rm -d \
        --name "$container" \
        -e POSTGRES_PASSWORD=postgres \
        -e POSTGRES_DB=riichi \
        -p 127.0.0.1::5432 \
        postgres:16-alpine >/dev/null
    for _ in $(seq 1 60); do
        if docker exec "$container" pg_isready -U postgres -d riichi >/dev/null 2>&1 \
            && docker exec "$container" psql -U postgres -d riichi -Atqc 'SELECT 1' >/dev/null 2>&1; then
            return
        fi
        sleep 1
    done
    echo "timed out waiting for $container" >&2
    exit 1
}

port_for() {
    docker port "$1" 5432/tcp | sed -E 's/.*:([0-9]+)$/\1/'
}

hash_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

start_database "$source_container"
source_port="$(port_for "$source_container")"
source_url="postgres://postgres:postgres@127.0.0.1:$source_port/riichi"
RIICHI_DATABASE_URL="$source_url" cargo run --quiet --bin riichi-migrate
docker exec -i "$source_container" psql -U postgres -d riichi -v ON_ERROR_STOP=1 < scripts/seed-restore-fixture.sql
docker exec "$source_container" pg_dump -U postgres --format=custom --no-owner --no-privileges --dbname=riichi > "$dump_path"
mkdir -p "$source_attachment_root/restore"
printf 'restore' > "$source_attachment_root/restore/attachment.txt"
tar -C "$source_attachment_root" -cf "$attachment_backup" restore/attachment.txt

start_database "$restore_container"
restore_port="$(port_for "$restore_container")"
restore_url="postgres://postgres:postgres@127.0.0.1:$restore_port/riichi"
docker exec -i "$restore_container" pg_restore -U postgres --exit-on-error --clean --if-exists --no-owner --no-privileges --dbname=riichi < "$dump_path"
mkdir -p "$restore_attachment_root"
tar -C "$restore_attachment_root" -xf "$attachment_backup"
docker exec -i "$restore_container" psql -U postgres -d riichi -v ON_ERROR_STOP=1 < scripts/verify-projections.sql
docker exec -i "$restore_container" psql -U postgres -d riichi -v ON_ERROR_STOP=1 < scripts/pilot-metrics.sql

restored_attachment_metadata="$(docker exec "$restore_container" psql -U postgres -d riichi -Atc \
    "SELECT byte_size || ' ' || encode(checksum, 'hex') FROM attachments WHERE storage_key = 'restore/attachment.txt'")"
expected_attachment_metadata="7 f329e3a317eee6a8a1a7357f69bc0488e0fad238ad58b30fc99139445f51e6ab"
if [[ "$restored_attachment_metadata" != "$expected_attachment_metadata" ]]; then
    echo "restored attachment metadata did not match fixture" >&2
    exit 1
fi
if [[ "$(wc -c < "$restore_attachment_root/restore/attachment.txt" | tr -d ' ')" != "7" ]] \
    || [[ "$(hash_file "$restore_attachment_root/restore/attachment.txt")" != "f329e3a317eee6a8a1a7357f69bc0488e0fad238ad58b30fc99139445f51e6ab" ]]; then
    echo "restored attachment bytes did not match fixture" >&2
    exit 1
fi

restored_snapshot="$(docker exec "$restore_container" psql -U postgres -d riichi -Atc \
    "SELECT encode(snapshot, 'base64') FROM document_loro_snapshots ORDER BY document_id LIMIT 1")"
restored_update="$(docker exec "$restore_container" psql -U postgres -d riichi -Atc \
    "SELECT encode(payload, 'base64') FROM document_loro_updates ORDER BY accepted_at, update_id LIMIT 1")"
RIICHI_RESTORE_SNAPSHOT="$restored_snapshot" \
RIICHI_RESTORE_UPDATE="$restored_update" \
pnpm --filter riichi-web exec node --input-type=module -e '
  import { LoroDoc } from "loro-crdt";
  const snapshot = new LoroDoc();
  snapshot.import(Buffer.from(process.env.RIICHI_RESTORE_SNAPSHOT, "base64"));
  if (snapshot.frontiers().length !== 1 || snapshot.getText("text").toString() !== "restore fixture update") {
    throw new Error("restored Loro snapshot is not readable");
  }
  const update = new LoroDoc();
  update.import(Buffer.from(process.env.RIICHI_RESTORE_UPDATE, "base64"));
  if (update.getText("text").toString() !== "restore fixture update") {
    throw new Error("restored Loro update is not readable");
  }
'

echo "populated restore verification passed"
