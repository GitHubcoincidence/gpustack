#!/usr/bin/env bash
# Performs the M8-A.6 package contract checks without invoking Docker.
set -o errexit -o errtrace -o nounset -o pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
readonly RUST_ROOT="$(readlink -f -- "$SCRIPT_DIR/../..")"
readonly DOCKERFILE="$RUST_ROOT/docker/worker/Dockerfile"
readonly IGNORE_FILE="$RUST_ROOT/.dockerignore"

required_files=(
  "$DOCKERFILE"
  "$IGNORE_FILE"
  "$SCRIPT_DIR/common.sh"
  "$SCRIPT_DIR/build-image.sh"
  "$SCRIPT_DIR/run-worker-container.sh"
  "$SCRIPT_DIR/stop-worker-container.sh"
)
for path in "${required_files[@]}"; do
  [[ -f "$path" && ! -L "$path" ]] || {
    printf 'STOP: required package file is missing or symlinked: %s\n' "$path" >&2
    exit 1
  }
done

for script in "$SCRIPT_DIR"/*.sh; do
  bash -n "$script"
done

required_dockerfile_text=(
  'rust:1.99.1-slim-bookworm@sha256:f2c004e3d2d8b547e8cf7ffbc29205385f4e71d23e53e4e383065db5e9f822cd'
  'debian:bookworm-20261027-slim@sha256:a3bba7dc7dbcc1f37c2856c51fc9a7fac96608f50cb87b1b4e4d8b8c85ae988e'
  'cargo build --release --locked --package gpustack-cli --bin gpustack-rs'
  'USER 65532:65532'
  'ENTRYPOINT ["/usr/local/bin/gpustack-rs"]'
  'CMD ["worker", "run"]'
)
for expected in "${required_dockerfile_text[@]}"; do
  grep -Fq -- "$expected" "$DOCKERFILE" || {
    printf 'STOP: Dockerfile contract is missing: %s\n' "$expected" >&2
    exit 1
  }
done

required_run_text=(
  '--network host'
  '--restart unless-stopped'
  '--runtime nvidia'
  '--gpus all'
  '--read-only'
  '--cap-drop ALL'
  '--security-opt no-new-privileges'
  '--group-add "$SOCKET_GID"'
)
for expected in "${required_run_text[@]}"; do
  grep -Fq -- "$expected" "$SCRIPT_DIR/run-worker-container.sh" || {
    printf 'STOP: run contract is missing: %s\n' "$expected" >&2
    exit 1
  }
done

if grep -Eq -- '--(control-plane-only|resource-api-only|managed-file-only|managed-runtime)' \
  "$DOCKERFILE" "$SCRIPT_DIR"/*.sh; then
  printf 'STOP: legacy Worker mode remains in the container package\n' >&2
  exit 1
fi
if grep -Fq -- '--force' "$SCRIPT_DIR/stop-worker-container.sh"; then
  printf 'STOP: protected stop script must not force-remove a container\n' >&2
  exit 1
fi

printf 'm8a6_package_contract=PASS\n'
