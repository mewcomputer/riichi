#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/apps/web/src/lib/generated"
cargo run -p riichi-api --bin openapi > "$root/apps/web/src/lib/generated/openapi.json"
pnpm exec openapi-typescript \
  "$root/apps/web/src/lib/generated/openapi.json" \
  -o "$root/apps/web/src/lib/generated/api.d.ts"
