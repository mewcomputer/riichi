#!/usr/bin/env bash
set -euo pipefail

container="riichi-s3-storage-$PPID-$$"
mc_config_dir="$(mktemp -d "${TMPDIR:-/tmp}/riichi-mc.XXXXXX")"
backup_dir="$(mktemp -d "${TMPDIR:-/tmp}/riichi-s3-backup.XXXXXX")"
managed="${RIICHI_S3_TEST_MANAGED:-false}"
test_prefix="${RIICHI_S3_TEST_PREFIX:-riichi-restore-check/$PPID-$$}"
alias_name=riichi-test
bucket="${RIICHI_ATTACHMENT_S3_BUCKET:-riichi}"
endpoint=""
storage_endpoint=""
access_key=""
secret_key=""
region="${RIICHI_ATTACHMENT_S3_REGION:-us-east-1}"
allow_http="${RIICHI_ATTACHMENT_S3_ALLOW_HTTP:-false}"
mc_network_args=()
mc_ready=false

cleanup() {
    if [[ "$mc_ready" == true ]]; then
        docker run --rm "${mc_network_args[@]}" -v "$mc_config_dir:/root/.mc" minio/mc:latest \
            rm --recursive --force "$alias_name/$bucket/$test_prefix" >/dev/null 2>&1 || true
    fi
    if [[ "$managed" != true ]]; then
        docker rm -f "$container" >/dev/null 2>&1 || true
    fi
    rm -rf "$mc_config_dir"
    rm -rf "$backup_dir"
}
trap cleanup EXIT

if [[ "$managed" == true ]]; then
    : "${RIICHI_ATTACHMENT_S3_ENDPOINT:?RIICHI_ATTACHMENT_S3_ENDPOINT is required when RIICHI_S3_TEST_MANAGED=true}"
    : "${RIICHI_ATTACHMENT_S3_ACCESS_KEY_ID:?RIICHI_ATTACHMENT_S3_ACCESS_KEY_ID is required when RIICHI_S3_TEST_MANAGED=true}"
    : "${RIICHI_ATTACHMENT_S3_SECRET_ACCESS_KEY:?RIICHI_ATTACHMENT_S3_SECRET_ACCESS_KEY is required when RIICHI_S3_TEST_MANAGED=true}"
    endpoint="$RIICHI_ATTACHMENT_S3_ENDPOINT"
    storage_endpoint="$endpoint"
    access_key="$RIICHI_ATTACHMENT_S3_ACCESS_KEY_ID"
    secret_key="$RIICHI_ATTACHMENT_S3_SECRET_ACCESS_KEY"
else
    docker run --rm -d \
        --name "$container" \
        -e MINIO_ROOT_USER=minioadmin \
        -e MINIO_ROOT_PASSWORD=minioadmin \
        -p 127.0.0.1::9000 \
        minio/minio:latest server /data >/dev/null

    port="$(docker port "$container" 9000/tcp | sed -E 's/.*:([0-9]+)$/\1/')"
    ready=false
    for _ in $(seq 1 60); do
        if curl --silent --fail "http://127.0.0.1:$port/minio/health/live" >/dev/null; then
            ready=true
            break
        fi
        sleep 1
    done
    if [[ "$ready" != true ]]; then
        echo "MinIO did not become ready" >&2
        docker logs --tail 40 "$container" >&2 || true
        exit 1
    fi
    endpoint=http://127.0.0.1:9000
    storage_endpoint="http://127.0.0.1:$port"
    access_key=minioadmin
    secret_key=minioadmin
    allow_http=true
    mc_network_args=(--network "container:$container")
fi

run_mc() {
    docker run --rm "${mc_network_args[@]}" \
        -v "$mc_config_dir:/root/.mc" -v "$backup_dir:/backup" \
        minio/mc:latest "$@"
}

run_mc alias set "$alias_name" "$endpoint" "$access_key" "$secret_key" >/dev/null
mc_ready=true
if [[ "$managed" != true ]]; then
    run_mc mb --ignore-existing "$alias_name/$bucket" >/dev/null
else
    run_mc ls "$alias_name/$bucket" >/dev/null
fi

printf 'restore' | docker run --rm -i "${mc_network_args[@]}" \
    -v "$mc_config_dir:/root/.mc" minio/mc:latest \
    pipe "$alias_name/$bucket/$test_prefix/restore/attachment.txt" >/dev/null
run_mc mirror "$alias_name/$bucket/$test_prefix" /backup --quiet \
    >/dev/null 2>&1
run_mc rm --recursive --force "$alias_name/$bucket/$test_prefix/restore" >/dev/null
run_mc mirror --overwrite /backup "$alias_name/$bucket/$test_prefix" --quiet \
    >/dev/null 2>&1
restored_fixture="$(run_mc cat "$alias_name/$bucket/$test_prefix/restore/attachment.txt")"
if [[ "$restored_fixture" != "restore" ]]; then
    echo "S3 attachment backup/restore did not preserve fixture bytes" >&2
    exit 1
fi

RIICHI_ATTACHMENT_BACKEND=s3 \
RIICHI_ATTACHMENT_S3_ENDPOINT="$storage_endpoint" \
RIICHI_ATTACHMENT_S3_BUCKET="$bucket" \
RIICHI_ATTACHMENT_S3_REGION="$region" \
RIICHI_ATTACHMENT_S3_ACCESS_KEY_ID="$access_key" \
RIICHI_ATTACHMENT_S3_SECRET_ACCESS_KEY="$secret_key" \
RIICHI_ATTACHMENT_S3_ALLOW_HTTP="$allow_http" \
cargo test -p riichi-storage --test s3 -- --ignored --test-threads=1

if [[ "$managed" == true ]]; then
    echo "managed S3-compatible attachment storage verification passed for prefix $test_prefix"
else
    echo "local MinIO S3-compatible attachment storage verification passed"
fi
