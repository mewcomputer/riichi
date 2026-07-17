#!/usr/bin/env bash
set -euo pipefail

: "${RIICHI_DATABASE_URL:?RIICHI_DATABASE_URL must be set}"
output="${1:-riichi-$(date -u +%Y%m%dT%H%M%SZ).dump}"
if [[ -e "$output" || -e "$output.sha256" ]]; then
  echo "backup output already exists: $output" >&2
  exit 1
fi
umask 077
pg_dump --format=custom --no-owner --no-privileges --file "$output" "$RIICHI_DATABASE_URL"
if command -v sha256sum >/dev/null 2>&1; then
  sha256sum "$output" > "$output.sha256"
else
  shasum -a 256 "$output" > "$output.sha256"
fi
echo "created $output"
echo "created $output.sha256"
