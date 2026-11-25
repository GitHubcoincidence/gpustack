#!/usr/bin/env bash
# Read-only external preflight for the M8-EMB.6 Python Server compatibility window.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly EXPECTED_HEAD="${M8EMB6_EXPECTED_HEAD:?M8EMB6_EXPECTED_HEAD is required}"
readonly EVIDENCE="${M8EMB6_PREFLIGHT_EVIDENCE:?M8EMB6_PREFLIGHT_EVIDENCE is required}"

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  m8emb6_die "preflight evidence path already exists: $EVIDENCE"
fi
m8emb6_require_commands cargo curl docker find git install jq nvidia-smi pgrep sha256sum ss stat
for token_file in \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" \
  "$M8EMB6_CLUSTER_TOKEN"; do
  m8emb6_require_private_file "$token_file"
done
m8emb6_verify_model

install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

current_step='initialization'
result_written=0
finish() {
  local code=$?
  trap - EXIT
  if [[ "$result_written" -eq 0 ]]; then
    printf 'status=STOP\nstage=M8-EMB.6-preflight\nfailed_step=%s\nexit_code=%s\n' \
      "$current_step" "$code" >"$EVIDENCE/result.txt"
  fi
  m8emb6_seal_directory "$EVIDENCE"
  exit "$code"
}
trap finish EXIT

announce() {
  printf '\n[%s/7] %s\n' "$1" "$2"
}

current_step='source_identity'
announce 1 '固定源码、二进制、工具链与模型身份'
cd "$M8EMB6_REPO"
source /home/wujiaxing2/.cargo/env
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
git merge-base --is-ancestor "$M8EMB6_PRODUCT_COMMIT" HEAD
git diff --quiet "$M8EMB6_PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates
[[ "$(sha256sum rust/Cargo.lock | awk '{print $1}')" == "$M8EMB6_CARGO_LOCK_SHA" ]]
CARGO_NET_OFFLINE=true cargo build --manifest-path rust/Cargo.toml \
  --workspace --locked --offline >"$EVIDENCE/build.log" 2>&1
"$M8EMB6_BINARY" --output json version >"$EVIDENCE/version.json"
[[ "$(jq -r '.git_commit' "$EVIDENCE/version.json")" == "${EXPECTED_HEAD:0:12}" ]]
"$M8EMB6_BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e \
  --arg python_name "$M8EMB6_PYTHON_CONTEXT" \
  --arg python_url "$M8EMB6_PYTHON_SERVER_URL" \
  --arg python_token "$M8EMB6_PYTHON_OPERATOR_TOKEN" \
  --arg rust_name "$M8EMB6_RUST106_CONTEXT" \
  --arg rust_url "$M8EMB6_RUST106_SERVER_URL" \
  --arg rust_token "$M8EMB6_RUST106_OPERATOR_TOKEN" '
    ([.[] | select(
      .name == $python_name and .server_url == $python_url and
      .token_file == $python_token
    )] | length) == 1 and
    ([.[] | select(
      .name == $rust_name and .server_url == $rust_url and
      .token_file == $rust_token
    )] | length) == 1
  ' "$EVIDENCE/contexts.json" >/dev/null
m8emb6_verify_model
{
  printf 'governance_head=%s\nproduct_commit=%s\n' "$EXPECTED_HEAD" "$M8EMB6_PRODUCT_COMMIT"
  printf 'cargo_lock_sha256=%s\nbinary_sha256=%s\n' \
    "$M8EMB6_CARGO_LOCK_SHA" "$(sha256sum "$M8EMB6_BINARY" | awk '{print $1}')"
  printf 'model_revision=%s\nmodel_manifest_sha256=%s\n' \
    "$M8EMB6_MODEL_REVISION" "$(sha256sum "$M8EMB6_MODEL_MANIFEST" | awk '{print $1}')"
} >"$EVIDENCE/source-identity.txt"

current_step='service_identity'
announce 2 '复核90 Python Server、Python Worker与106 Rust Server'
{
  printf 'python_server=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' "${M8EMB6_PYTHON_SERVER_URL}healthz")"
  printf 'python_worker=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' http://127.0.0.1:10150/healthz)"
  printf 'python_metrics=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' http://127.0.0.1:10151/metrics)"
  printf 'rust106_server=%s\n' "$(curl --silent --show-error --max-time 10 \
    --output /dev/null --write-out '%{http_code}' "${M8EMB6_RUST106_SERVER_URL}healthz")"
} >"$EVIDENCE/services.txt"
grep -qx 'python_server=200' "$EVIDENCE/services.txt"
grep -qx 'python_worker=200' "$EVIDENCE/services.txt"
grep -qx 'python_metrics=200' "$EVIDENCE/services.txt"
grep -qx 'rust106_server=200' "$EVIDENCE/services.txt"

current_step='reserved_identity'
announce 3 '证明实验Worker、模型、端口与本地状态均未占用'
for path in "$M8EMB6_DATA_DIR" "$M8EMB6_LOG_DIR"; do
  [[ ! -e "$path" && ! -L "$path" ]] || m8emb6_die "reserved path is occupied: $path"
done
for port in "$M8EMB6_WORKER_PORT" "$M8EMB6_METRICS_PORT"; do
  [[ "$(ss -H -lnt "sport = :$port" | wc -l)" == '0' ]] ||
    m8emb6_die "reserved port is occupied: $port"
done
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/workers?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers.json")"
[[ "$python_status" == '200' ]]
jq -e --arg name "$M8EMB6_WORKER_NAME" \
  '[.items[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/python-workers.json" >/dev/null
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/models?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-models.json")"
[[ "$python_status" == '200' ]]
jq -e --arg name "$M8EMB6_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/python-models.json" >/dev/null
# A newly registered Worker is not maintenance-enabled by default. With no Model
# on the Server, the executor can establish maintenance before it becomes ready.
jq -e '.items | length == 0' "$EVIDENCE/python-models.json" >/dev/null

current_step='existing_worker_process'
announce 4 '固定106 Worker进程、二进制与原始argv'
mapfile -t existing_pids < <(pgrep -f \
  '[g]pustack-rs.*worker.*--server-url http://10[.]213[.]80[.]106:18091/.*run' || true)
[[ "${#existing_pids[@]}" == '1' ]] ||
  m8emb6_die "expected exactly one 106 Rust Worker process"
existing_pid="${existing_pids[0]}"
m8emb6_record_process_identity "$existing_pid" existing_rust106_worker \
  "$EVIDENCE/existing-worker-process.txt"
install -m 0600 "/proc/$existing_pid/cmdline" "$EVIDENCE/existing-worker-cmdline.bin"
install -m 0700 "/proc/$existing_pid/exe" "$EVIDENCE/existing-worker-binary"
[[ "$(sha256sum "$EVIDENCE/existing-worker-binary" | awk '{print $1}')" == \
  "$(sed -n 's/^exe_sha256=//p' "$EVIDENCE/existing-worker-process.txt")" ]]

current_step='existing_server_resources'
announce 5 '固定106 Worker、Instance与GPU0容器所有权'
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/workers?page=-1" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/rust106-workers.json")"
[[ "$rust_status" == '200' ]]
existing_worker_count="$(jq '[.items[] | select(.name == "worker-90")] | length' \
  "$EVIDENCE/rust106-workers.json")"
[[ "$existing_worker_count" == '1' ]] || m8emb6_die "106 worker-90 identity is ambiguous"
jq '[.items[] | select(.name == "worker-90")][0]' \
  "$EVIDENCE/rust106-workers.json" >"$EVIDENCE/existing-worker.json"
existing_worker_id="$(jq -r '.id' "$EVIDENCE/existing-worker.json")"
existing_worker_uuid="$(jq -r '.uuid' "$EVIDENCE/existing-worker.json")"
jq -e '(.maintenance.enabled // false) == false' \
  "$EVIDENCE/existing-worker.json" >/dev/null

mapfile -t gpu0_containers < <(
  docker ps -q --no-trunc --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed \
    --filter label=io.gpustack.component=vllm |
    while IFS= read -r candidate; do
      docker inspect "$candidate" | jq -r --arg gpu "$M8EMB6_GPU0_UUID" '
        .[0] | select(.HostConfig.DeviceRequests[0].DeviceIDs == [$gpu]) | .Id
      '
    done
)
[[ "${#gpu0_containers[@]}" == '1' ]] ||
  m8emb6_die "expected exactly one running Rust container on GPU0"
existing_container_id="${gpu0_containers[0]}"
m8emb6_capture_container "$existing_container_id" "$EVIDENCE/existing-container.json"
existing_instance_id="$(jq -r '.labels["io.gpustack.model-instance-id"]' \
  "$EVIDENCE/existing-container.json")"
m8emb6_validate_container_tuple "$EVIDENCE/existing-container.json" \
  "$existing_worker_uuid" "$existing_instance_id"
[[ "$(jq -r '.Image' "$EVIDENCE/existing-container.json")" == "$M8EMB6_RUNNER_IMAGE_ID" ]]
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/model-instances/$existing_instance_id" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/existing-instance.json")"
[[ "$rust_status" == '200' ]]
existing_model_id="$(jq -r '.model_id' "$EVIDENCE/existing-instance.json")"
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/models/$existing_model_id" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/existing-model.json")"
[[ "$rust_status" == '200' ]]

current_step='gpu_and_image'
announce 6 '固定双GPU观察基线与Runner镜像'
m8emb6_capture_gpu "$EVIDENCE" before
[[ "$(m8emb6_gpu_compute_count "$M8EMB6_GPU0_UUID")" -gt 0 ]]
docker image inspect "$M8EMB6_RUNNER_IMAGE_ID" >"$EVIDENCE/runner-image.json"
jq -e --arg image "$M8EMB6_RUNNER_IMAGE_ID" '.[0].Id == $image' \
  "$EVIDENCE/runner-image.json" >/dev/null

jq -n \
  --argjson worker_id "$existing_worker_id" \
  --arg worker_uuid "$existing_worker_uuid" \
  --argjson model_id "$existing_model_id" \
  --argjson instance_id "$existing_instance_id" \
  --arg container_id "$existing_container_id" \
  --arg maintenance "$(jq -r '.maintenance.enabled' "$EVIDENCE/existing-worker.json")" \
  --arg process_identity 'existing-worker-process.txt' \
  --arg process_cmdline 'existing-worker-cmdline.bin' \
  --arg process_binary 'existing-worker-binary' \
  --arg container_snapshot 'existing-container.json' \
  '{
    worker_id:$worker_id,
    worker_uuid:$worker_uuid,
    model_id:$model_id,
    instance_id:$instance_id,
    container_id:$container_id,
    maintenance_enabled:($maintenance == "true"),
    process_identity:$process_identity,
    process_cmdline:$process_cmdline,
    process_binary:$process_binary,
    container_snapshot:$container_snapshot
  }' >"$EVIDENCE/recovery-plan.json"

current_step='seal'
announce 7 '封存只读preflight'
[[ "$(m8emb6_secret_scan_count "$EVIDENCE")" == '0' ]]
printf 'status=PASS\nstage=M8-EMB.6-preflight\ngovernance_head=%s\n' \
  "$EXPECTED_HEAD" >"$EVIDENCE/result.txt"
result_written=1
m8emb6_seal_directory "$EVIDENCE"
printf 'preflight=PASS\nevidence=%s\nmanifest_sha256=%s\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
