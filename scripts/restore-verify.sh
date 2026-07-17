#!/usr/bin/env bash
set -euo pipefail

: "${RIICHI_RESTORE_DATABASE_URL:?RIICHI_RESTORE_DATABASE_URL must point to a disposable restore database}"
dump="${1:?usage: $0 path/to/riichi.dump}"

if [[ -f "$dump.sha256" ]]; then
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum --check "$dump.sha256"
  else
    shasum -a 256 --check "$dump.sha256"
  fi
fi

pg_restore --exit-on-error --clean --if-exists --no-owner --no-privileges --dbname "$RIICHI_RESTORE_DATABASE_URL" "$dump"
psql "$RIICHI_RESTORE_DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/verify-projections.sql
psql "$RIICHI_RESTORE_DATABASE_URL" -v ON_ERROR_STOP=1 -f scripts/pilot-metrics.sql
