#!/usr/bin/env bash
# Independent post-window audit for M8-EMB.6 evidence and restored external state.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly EXPECTED_HEAD="${M8EMB6_EXPECTED_HEAD:?M8EMB6_EXPECTED_HEAD is required}"
readonly PREFLIGHT="${M8EMB6_PREFLIGHT_EVIDENCE:?M8EMB6_PREFLIGHT_EVIDENCE is required}"
readonly FORMAL_EVIDENCE="${M8EMB6_EVIDENCE:?M8EMB6_EVIDENCE is required}"
readonly CLEANUP_EVIDENCE="${M8EMB6_CLEANUP_EVIDENCE:?M8EMB6_CLEANUP_EVIDENCE is required}"
readonly WATCHDOG_EVIDENCE="${M8EMB6_WATCHDOG_EVIDENCE:?M8EMB6_WATCHDOG_EVIDENCE is required}"
readonly EVIDENCE="${M8EMB6_POST_AUDIT_EVIDENCE:?M8EMB6_POST_AUDIT_EVIDENCE is required}"

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  m8emb6_die "post-audit evidence path already exists: $EVIDENCE"
fi
for directory in "$PREFLIGHT" "$FORMAL_EVIDENCE" "$CLEANUP_EVIDENCE" "$WATCHDOG_EVIDENCE"; do
  [[ -d "$directory" && ! -L "$directory" ]] ||
    m8emb6_die "required evidence directory is unavailable: $directory"
done
install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

current_step='initialization'
result_written=0
finish() {
  local code=$?
  trap - EXIT
  if [[ "$result_written" -eq 0 ]]; then
    printf 'status=FAIL\nstage=M8-EMB.6-post-audit\nfailed_step=%s\nexit_code=%s\n' \
      "$current_step" "$code" >"$EVIDENCE/result.txt"
  fi
  m8emb6_seal_directory "$EVIDENCE"
  exit "$code"
}
trap finish EXIT

announce() {
  printf '\n[%s/6] %s\n' "$1" "$2"
}

current_step='evidence_chain'
announce 1 '验证preflight、formal、cleanup与watchdog证据链'
for directory in "$PREFLIGHT" "$FORMAL_EVIDENCE" "$CLEANUP_EVIDENCE" "$WATCHDOG_EVIDENCE"; do
  (cd "$directory" && sha256sum --check manifest.sha256) \
    >>"$EVIDENCE/evidence-manifest-check.txt"
done
[[ "$(sed -n 's/^status=//p' "$PREFLIGHT/result.txt")" == 'PASS' ]]
[[ "$(sed -n 's/^status=//p' "$FORMAL_EVIDENCE/result.txt")" == 'PASS' ]]
[[ "$(sed -n 's/^cleanup_ok=//p' "$CLEANUP_EVIDENCE/result.txt")" == '1' ]]
[[ "$(sed -n 's/^status=//p' "$WATCHDOG_EVIDENCE/result.txt")" == 'PASS' ]]
[[ "$(sed -n 's/^recovery_ok=//p' "$WATCHDOG_EVIDENCE/result.txt")" == '1' ]]

current_step='source_and_model'
announce 2 '复核源码、二进制与固定模型未漂移'
cd "$M8EMB6_REPO"
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
git diff --quiet "$M8EMB6_PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates
[[ "$(sha256sum rust/Cargo.lock | awk '{print $1}')" == "$M8EMB6_CARGO_LOCK_SHA" ]]
m8emb6_verify_model

current_step='services'
announce 3 '复核90 Python服务与106 Rust Server'
{
  printf 'python_server=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' "${M8EMB6_PYTHON_SERVER_URL}healthz")"
  printf 'python_worker=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' http://127.0.0.1:10150/healthz)"
  printf 'python_metrics=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' http://127.0.0.1:10151/metrics)"
  printf 'rust106_server=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' "${M8EMB6_RUST106_SERVER_URL}healthz")"
} >"$EVIDENCE/services.txt"
grep -qx 'python_server=200' "$EVIDENCE/services.txt"
grep -qx 'python_worker=200' "$EVIDENCE/services.txt"
grep -qx 'python_metrics=200' "$EVIDENCE/services.txt"
grep -qx 'rust106_server=200' "$EVIDENCE/services.txt"

current_step='experimental_absence'
announce 4 '证明实验Server资源、容器、进程、端口与状态目录归零'
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/models?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-models.json")"
[[ "$python_status" == '200' ]]
jq -e --arg name "$M8EMB6_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length == 0' "$EVIDENCE/python-models.json" >/dev/null
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/workers?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers.json")"
[[ "$python_status" == '200' ]]
jq -e --arg name "$M8EMB6_WORKER_NAME" \
  '[.items[] | select(.name == $name)] | length == 0' "$EVIDENCE/python-workers.json" >/dev/null
formal_worker_uuid="$(sed -n 's/^worker_uuid=//p' "$FORMAL_EVIDENCE/result.txt")"
formal_worker_id="$(sed -n 's/^worker_id=//p' "$FORMAL_EVIDENCE/result.txt")"
[[ "$formal_worker_uuid" =~ ^[0-9a-f-]{36}$ ]]
[[ "$formal_worker_id" =~ ^[1-9][0-9]*$ ]]
[[ "$(docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.worker-uuid="$formal_worker_uuid" | wc -l)" == '0' ]]
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/model-files?page=-1&worker_id=$formal_worker_id" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-model-files.json")"
[[ "$python_status" == '200' ]]
jq -e '.items | length == 0' "$EVIDENCE/python-model-files.json" >/dev/null
pgrep -af '[g]pustack-rs.*worker.*--worker-name rust-m8emb6-r760xa.*run' \
  >"$EVIDENCE/experimental-processes.txt" || true
[[ ! -s "$EVIDENCE/experimental-processes.txt" ]]
[[ "$(ss -H -lnt "sport = :$M8EMB6_WORKER_PORT" | wc -l)" == '0' ]]
[[ "$(ss -H -lnt "sport = :$M8EMB6_METRICS_PORT" | wc -l)" == '0' ]]
[[ ! -e "$M8EMB6_DATA_DIR" && ! -L "$M8EMB6_DATA_DIR" ]]
[[ ! -e "$M8EMB6_LOG_DIR" && ! -L "$M8EMB6_LOG_DIR" ]]

current_step='existing_runtime_restored'
announce 5 '证明106 Worker、Model、Instance与原容器恢复'
existing_worker_id="$(jq -r '.worker_id' "$PREFLIGHT/recovery-plan.json")"
existing_worker_uuid="$(jq -r '.worker_uuid' "$PREFLIGHT/recovery-plan.json")"
existing_model_id="$(jq -r '.model_id' "$PREFLIGHT/recovery-plan.json")"
existing_instance_id="$(jq -r '.instance_id' "$PREFLIGHT/recovery-plan.json")"
existing_container_id="$(jq -r '.container_id' "$PREFLIGHT/recovery-plan.json")"
existing_maintenance="$(jq -r '.maintenance_enabled' "$PREFLIGHT/recovery-plan.json")"
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/workers/$existing_worker_id" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/existing-worker.json")"
[[ "$rust_status" == '200' ]]
jq -e --arg uuid "$existing_worker_uuid" --argjson maintenance "$existing_maintenance" '
  .uuid == $uuid and .unreachable == false and
  .maintenance.enabled == $maintenance
' "$EVIDENCE/existing-worker.json" >/dev/null
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/model-instances/$existing_instance_id" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/existing-instance.json")"
[[ "$rust_status" == '200' ]]
jq -e --argjson model "$existing_model_id" --argjson worker "$existing_worker_id" '
  .model_id == $model and .worker_id == $worker and .state == "running"
' "$EVIDENCE/existing-instance.json" >/dev/null
m8emb6_capture_container "$existing_container_id" "$EVIDENCE/existing-container.json"
m8emb6_validate_container_tuple "$EVIDENCE/existing-container.json" \
  "$existing_worker_uuid" "$existing_instance_id"
[[ "$(jq -r '.running' "$EVIDENCE/existing-container.json")" == 'true' ]]
[[ "$(jq -r '.Image' "$EVIDENCE/existing-container.json")" == \
  "$(jq -r '.Image' "$PREFLIGHT/existing-container.json")" ]]
[[ "$(jq -r '.labels["io.gpustack.spec-hash"]' "$EVIDENCE/existing-container.json")" == \
  "$(jq -r '.labels["io.gpustack.spec-hash"]' "$PREFLIGHT/existing-container.json")" ]]
[[ "$(jq -r '.restart_count' "$EVIDENCE/existing-container.json")" == \
  "$(jq -r '.restart_count' "$PREFLIGHT/existing-container.json")" ]]
mapfile -t restored_worker_pids < <(pgrep -f \
  '[g]pustack-rs.*worker.*--server-url http://10[.]213[.]80[.]106:18091/.*run' || true)
[[ "${#restored_worker_pids[@]}" == '1' ]]
m8emb6_record_process_identity "${restored_worker_pids[0]}" restored_rust106_worker \
  "$EVIDENCE/restored-worker-process.txt"
[[ "$(sed -n 's/^exe_sha256=//p' "$EVIDENCE/restored-worker-process.txt")" == \
  "$(sed -n 's/^exe_sha256=//p' "$PREFLIGHT/existing-worker-process.txt")" ]]
[[ "$(sed -n 's/^cmdline_sha256=//p' "$EVIDENCE/restored-worker-process.txt")" == \
  "$(sed -n 's/^cmdline_sha256=//p' "$PREFLIGHT/existing-worker-process.txt")" ]]

current_step='gpu_and_seal'
announce 6 '记录双GPU后审并封存结果'
m8emb6_capture_gpu "$EVIDENCE" after
[[ "$(m8emb6_gpu_compute_count "$M8EMB6_GPU0_UUID")" -gt 0 ]]
[[ "$(m8emb6_secret_scan_count "$EVIDENCE")" == '0' ]]
printf 'status=PASS\nstage=M8-EMB.6-post-audit\nrecovery_baseline=restored\n' \
  >"$EVIDENCE/result.txt"
result_written=1
m8emb6_seal_directory "$EVIDENCE"
printf 'post_audit=PASS\nevidence=%s\nmanifest_sha256=%s\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
