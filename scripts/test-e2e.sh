#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${DOCKER_HOST:-}" ]]; then
  DOCKER_HOST="$(docker context inspect --format '{{.Endpoints.docker.Host}}')"
  export DOCKER_HOST
fi

if [[ "${DOCKER_HOST}" == unix://* && -z "${TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE:-}" ]]; then
  TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE="${DOCKER_HOST#unix://}"
  export TESTCONTAINERS_DOCKER_SOCKET_OVERRIDE
fi

cargo test --workspace -- --ignored --nocapture --test-threads=1
