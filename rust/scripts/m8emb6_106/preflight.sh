#!/usr/bin/env bash
# Read-only preflight for running the latest Worker on 106 against Python Server 90.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly EXPECTED_HEAD="${M8EMB6106_EXPECTED_HEAD:?M8EMB6106_EXPECTED_HEAD is required}"
readonly EVIDENCE="${M8EMB6106_PREFLIGHT_EVIDENCE:?M8EMB6106_PREFLIGHT_EVIDENCE is required}"

[[ ! -e "$EVIDENCE" && ! -L "$EVIDENCE" ]] ||
  m8emb6106_die "preflight evidence path already exists: $EVIDENCE"
source /home/wujiaxing/.cargo/env
m8emb6106_require_commands cargo curl docker find git install nvidia-smi pgrep \
  python3 sha256sum ss stat
for token_file in \
  "$M8EMB6106_CONTEXTS_FILE" \
  "$M8EMB6106_PYTHON_OPERATOR_TOKEN" \
  "$M8EMB6106_RUST_OPERATOR_TOKEN" \
  "$M8EMB6106_CLUSTER_TOKEN"; do
  m8emb6106_require_private_file "$token_file"
done
m8emb6106_verify_model

install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

current_step='initialization'
result_written=0
finish() {
  local code=$?
  trap - EXIT
  if [[ "$result_written" -eq 0 ]]; then
    printf 'status=STOP\nstage=M8-EMB.6B-106-preflight\nfailed_step=%s\nexit_code=%s\n' \
      "$current_step" "$code" >"$EVIDENCE/result.txt"
  fi
  m8emb6106_seal_directory "$EVIDENCE"
  exit "$code"
}
trap finish EXIT

announce() {
  printf '\n[%s/7] %s\n' "$1" "$2"
}

current_step='source_identity'
announce 1 '固定源码、最新二进制、Context和模型身份'
cd "$M8EMB6106_REPO"
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
git merge-base --is-ancestor "$M8EMB6106_PRODUCT_COMMIT" HEAD
git diff --quiet "$M8EMB6106_PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates
[[ "$(sha256sum rust/Cargo.lock | awk '{print $1}')" == "$M8EMB6106_CARGO_LOCK_SHA" ]]
CARGO_NET_OFFLINE=true cargo build --manifest-path rust/Cargo.toml \
  --workspace --locked --offline >"$EVIDENCE/build.log" 2>&1
"$M8EMB6106_BINARY" --output json version >"$EVIDENCE/version.json"
python3 "$M8EMB6106_JSON_TOOL" check-version \
  --file "$EVIDENCE/version.json" --expected "$EXPECTED_HEAD"
"$M8EMB6106_BINARY" --output json context list >"$EVIDENCE/contexts.json"
python3 "$M8EMB6106_JSON_TOOL" check-contexts \
  --file "$EVIDENCE/contexts.json" \
  --python-name "$M8EMB6106_PYTHON_CONTEXT" \
  --python-url "$M8EMB6106_PYTHON_SERVER_URL" \
  --python-token "$M8EMB6106_PYTHON_OPERATOR_TOKEN" \
  --rust-name "$M8EMB6106_RUST_CONTEXT" \
  --rust-url "$M8EMB6106_RUST_SERVER_URL" \
  --rust-token "$M8EMB6106_RUST_OPERATOR_TOKEN"
m8emb6106_verify_model
{
  printf 'governance_head=%s\nproduct_commit=%s\n' \
    "$EXPECTED_HEAD" "$M8EMB6106_PRODUCT_COMMIT"
  printf 'cargo_lock_sha256=%s\nbinary_sha256=%s\n' \
    "$M8EMB6106_CARGO_LOCK_SHA" \
    "$(sha256sum "$M8EMB6106_BINARY" | awk '{print $1}')"
  printf 'model_revision=%s\nmodel_manifest_sha256=%s\n' \
    "$M8EMB6106_MODEL_REVISION" \
    "$(sha256sum "$M8EMB6106_MODEL_MANIFEST" | awk '{print $1}')"
} >"$EVIDENCE/source-identity.txt"

current_step='service_identity'
announce 2 '复核90 Python控制面和106现有服务'
{
  printf 'python_server=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' \
    "${M8EMB6106_PYTHON_SERVER_URL}healthz")"
  printf 'rust_server=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' \
    "${M8EMB6106_RUST_SERVER_URL}healthz")"
  printf 'original_worker_health=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6106_WORKER_IP}:${M8EMB6106_ORIGINAL_WORKER_PORT}/healthz")"
  printf 'original_worker_ready=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6106_WORKER_IP}:${M8EMB6106_ORIGINAL_WORKER_PORT}/readyz")"
  printf 'original_worker_metrics=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6106_WORKER_IP}:${M8EMB6106_ORIGINAL_METRICS_PORT}/metrics")"
  printf 'original_model=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6106_WORKER_IP}:${M8EMB6106_ORIGINAL_MODEL_PORT}/health")"
} >"$EVIDENCE/services.txt"
[[ "$(grep -c '=200$' "$EVIDENCE/services.txt")" == '6' ]]

current_step='python_server_safety'
announce 3 '证明90 Server没有资源会调度到实验Worker'
python_status="$(m8emb6106_auth_request GET \
  "${M8EMB6106_PYTHON_SERVER_URL}v2/workers?page=-1" \
  "$M8EMB6106_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers.json")"
[[ "$python_status" == '200' ]]
python_status="$(m8emb6106_auth_request GET \
  "${M8EMB6106_PYTHON_SERVER_URL}v2/models?page=-1" \
  "$M8EMB6106_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-models.json")"
[[ "$python_status" == '200' ]]
python3 "$M8EMB6106_JSON_TOOL" python-safety \
  --workers "$EVIDENCE/python-workers.json" \
  --models "$EVIDENCE/python-models.json" \
  --target-worker "$M8EMB6106_WORKER_NAME" \
  --target-model "$M8EMB6106_TARGET_MODEL" \
  >"$EVIDENCE/python-scheduling-summary.json"
for path in "$M8EMB6106_DATA_DIR" "$M8EMB6106_LOG_DIR"; do
  [[ ! -e "$path" && ! -L "$path" ]] ||
    m8emb6106_die "reserved path is occupied: $path"
done
for port in "$M8EMB6106_WORKER_PORT" "$M8EMB6106_METRICS_PORT"; do
  [[ "$(ss -H -lnt "sport = :$port" | wc -l)" == '0' ]] ||
    m8emb6106_die "reserved port is occupied: $port"
done

current_step='original_process'
announce 4 '固定原Worker进程和最新恢复argv'
mapfile -t existing_pids < <(pgrep -f \
  '[g]pustack-rs.*worker.*--worker-name jiaxingHaoyuNB-106.*run' || true)
[[ "${#existing_pids[@]}" == '1' ]] ||
  m8emb6106_die "expected exactly one original Rust Worker process"
existing_pid="${existing_pids[0]}"
m8emb6106_record_process_identity "$existing_pid" original_rust_worker \
  "$EVIDENCE/original-worker-process.txt"
install -m 0600 "/proc/$existing_pid/cmdline" \
  "$EVIDENCE/original-worker-cmdline.bin"
python3 "$M8EMB6106_JSON_TOOL" sanitize-argv \
  --file "$EVIDENCE/original-worker-cmdline.bin" \
  --binary "$M8EMB6106_BINARY" \
  --output "$EVIDENCE/restore-worker-argv.json"

current_step='original_resources'
announce 5 '固定原Worker、Model、Instance和容器所有权'
m8emb6106_require_private_file "$M8EMB6106_ORIGINAL_TOKEN"
m8emb6106_require_private_file "$M8EMB6106_ORIGINAL_IDENTITY"
m8emb6106_require_private_file "$M8EMB6106_ORIGINAL_WORKER_TOKEN"
m8emb6106_require_private_file "$M8EMB6106_ORIGINAL_MANIFEST"
install -m 0600 "$M8EMB6106_ORIGINAL_IDENTITY" \
  "$EVIDENCE/original-worker-identity.json"
install -m 0600 "$M8EMB6106_ORIGINAL_MANIFEST" \
  "$EVIDENCE/original-instance-manifest.json"
rust_status="$(m8emb6106_auth_request GET \
  "${M8EMB6106_RUST_SERVER_URL}v2/workers/$M8EMB6106_ORIGINAL_WORKER_ID" \
  "$M8EMB6106_RUST_OPERATOR_TOKEN" "$EVIDENCE/original-worker.json")"
[[ "$rust_status" == '200' ]]
rust_status="$(m8emb6106_auth_request GET \
  "${M8EMB6106_RUST_SERVER_URL}v2/models/$M8EMB6106_ORIGINAL_MODEL_ID" \
  "$M8EMB6106_RUST_OPERATOR_TOKEN" "$EVIDENCE/original-model.json")"
[[ "$rust_status" == '200' ]]
rust_status="$(m8emb6106_auth_request GET \
  "${M8EMB6106_RUST_SERVER_URL}v2/model-instances/$M8EMB6106_ORIGINAL_INSTANCE_ID" \
  "$M8EMB6106_RUST_OPERATOR_TOKEN" "$EVIDENCE/original-instance.json")"
[[ "$rust_status" == '200' ]]
mapfile -t original_containers < <(docker ps -q --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed \
  --filter label=io.gpustack.component=vllm \
  --filter label=io.gpustack.model-instance-id="$M8EMB6106_ORIGINAL_INSTANCE_ID")
[[ "${#original_containers[@]}" == '1' ]] ||
  m8emb6106_die "expected exactly one original ModelInstance container"
original_container_id="${original_containers[0]}"
docker inspect "$original_container_id" >"$EVIDENCE/original-container.json"
docker image inspect "$M8EMB6106_RUNNER_IMAGE_ID" >"$EVIDENCE/runner-image.json"
python3 "$M8EMB6106_JSON_TOOL" baseline \
  --worker "$EVIDENCE/original-worker.json" \
  --identity "$EVIDENCE/original-worker-identity.json" \
  --model "$EVIDENCE/original-model.json" \
  --instance "$EVIDENCE/original-instance.json" \
  --container "$EVIDENCE/original-container.json" \
  --process "$EVIDENCE/original-worker-process.txt" \
  --sanitized-argv "$EVIDENCE/restore-worker-argv.json" \
  --binary "$M8EMB6106_BINARY" \
  --output "$EVIDENCE/recovery-plan.json" \
  --worker-id "$M8EMB6106_ORIGINAL_WORKER_ID" \
  --worker-name "$M8EMB6106_ORIGINAL_WORKER_NAME" \
  --model-id "$M8EMB6106_ORIGINAL_MODEL_ID" \
  --model-name "$M8EMB6106_ORIGINAL_MODEL_NAME" \
  --instance-id "$M8EMB6106_ORIGINAL_INSTANCE_ID" \
  --image-id "$M8EMB6106_RUNNER_IMAGE_ID" \
  --gpu-uuid "$M8EMB6106_GPU0_UUID"

current_step='gpu_identity'
announce 6 '固定106唯一GPU观察基线'
mapfile -t gpu_uuids < <(nvidia-smi --query-gpu=uuid --format=csv,noheader)
[[ "${#gpu_uuids[@]}" == '1' && "${gpu_uuids[0]}" == "$M8EMB6106_GPU0_UUID" ]] ||
  m8emb6106_die "106 GPU inventory drifted"
m8emb6106_capture_gpu "$EVIDENCE" before
[[ "$(m8emb6106_gpu_compute_count "$M8EMB6106_GPU0_UUID")" -gt 0 ]]

current_step='seal'
announce 7 '封存只读preflight'
m8emb6106_process_identity_matches "$EVIDENCE/original-worker-process.txt"
[[ "$(m8emb6106_secret_scan_count "$EVIDENCE")" == '0' ]]
printf 'status=PASS\nstage=M8-EMB.6B-106-preflight\ngovernance_head=%s\n' \
  "$EXPECTED_HEAD" >"$EVIDENCE/result.txt"
result_written=1
m8emb6106_seal_directory "$EVIDENCE"
printf 'preflight=PASS\nevidence=%s\nmanifest_sha256=%s\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
