#!/usr/bin/env bash
# Formal M8-EMB.6 executor. It requires a one-use maintenance authorization file.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly EXPECTED_HEAD="${M8EMB6_EXPECTED_HEAD:?M8EMB6_EXPECTED_HEAD is required}"
readonly PREFLIGHT="${M8EMB6_PREFLIGHT_EVIDENCE:?M8EMB6_PREFLIGHT_EVIDENCE is required}"
readonly EXPECTED_PREFLIGHT_SHA="${M8EMB6_PREFLIGHT_MANIFEST_SHA:?M8EMB6_PREFLIGHT_MANIFEST_SHA is required}"
readonly EVIDENCE="${M8EMB6_EVIDENCE:?M8EMB6_EVIDENCE is required}"
readonly CLEANUP_EVIDENCE="${M8EMB6_CLEANUP_EVIDENCE:?M8EMB6_CLEANUP_EVIDENCE is required}"
readonly AUTHORIZATION="${M8EMB6_WINDOW_AUTHORIZATION:?M8EMB6_WINDOW_AUTHORIZATION is required}"

m8emb6_require_commands base64 curl docker find install jq nvidia-smi setsid sha256sum ss timeout
m8emb6_require_private_file "$AUTHORIZATION"
jq -e --arg head "$EXPECTED_HEAD" --arg preflight "$EXPECTED_PREFLIGHT_SHA" '
  .stage == "M8-EMB.6-maintenance-window" and .approved == true and
  .expected_head == $head and .preflight_manifest_sha256 == $preflight and
  (.nonce | type == "string" and length >= 16)
' "$AUTHORIZATION" >/dev/null || m8emb6_die "maintenance authorization is invalid"
[[ -d "$PREFLIGHT" && ! -L "$PREFLIGHT" ]] || m8emb6_die "preflight evidence is unavailable"
[[ "$(sha256sum "$PREFLIGHT/manifest.sha256" | awk '{print $1}')" == \
  "$EXPECTED_PREFLIGHT_SHA" ]] || m8emb6_die "preflight manifest identity mismatch"
(cd "$PREFLIGHT" && sha256sum --check manifest.sha256) >/dev/null
[[ "$(sed -n 's/^status=//p' "$PREFLIGHT/result.txt")" == 'PASS' ]]
if [[ -e "$EVIDENCE" || -L "$EVIDENCE" || -e "$CLEANUP_EVIDENCE" ||
  -L "$CLEANUP_EVIDENCE" ]]; then
  m8emb6_die "formal or cleanup evidence path is already used"
fi

install -d -m 0700 "$EVIDENCE"
install -m 0600 "$0" "$EVIDENCE/executor.sh"
install -m 0600 "$SCRIPT_DIR/common.sh" "$EVIDENCE/common.sh"
install -m 0600 "$SCRIPT_DIR/cleanup.sh" "$EVIDENCE/cleanup.sh"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

phase='initialization'
reason='executor_incomplete'
window_entered=0
smoke_passed=0
cleanup_ran=0
cleanup_ok=0
model_id=''
instance_id=''
worker_id=''
worker_uuid=''
container_id=''
spec_hash=''
experimental_pid=''

write_result() {
  local status='FAIL'
  [[ "$smoke_passed" -eq 1 && "$cleanup_ok" -eq 1 ]] && status='PASS'
  {
    printf 'status=%s\nstage=M8-EMB.6-formal\nreason=%s\nphase=%s\n' \
      "$status" "$reason" "$phase"
    printf 'window_entered=%s\nsmoke_passed=%s\ncleanup_ok=%s\n' \
      "$window_entered" "$smoke_passed" "$cleanup_ok"
    printf 'worker_id=%s\nworker_uuid=%s\nmodel_id=%s\nmodel_instance_id=%s\n' \
      "${worker_id:-none}" "${worker_uuid:-none}" "${model_id:-none}" "${instance_id:-none}"
    printf 'container_id=%s\nspec_hash=%s\n' \
      "${container_id:-none}" "${spec_hash:-none}"
  } >"$EVIDENCE/result.txt"
  chmod 0600 "$EVIDENCE/result.txt"
}

run_cleanup() {
  [[ "$cleanup_ran" -eq 0 ]] || return
  cleanup_ran=1
  write_result
  if M8EMB6_PREFLIGHT_EVIDENCE="$PREFLIGHT" \
    M8EMB6_PREFLIGHT_MANIFEST_SHA="$EXPECTED_PREFLIGHT_SHA" \
    M8EMB6_EVIDENCE="$EVIDENCE" \
    M8EMB6_CLEANUP_EVIDENCE="$CLEANUP_EVIDENCE" \
    "$SCRIPT_DIR/cleanup.sh"; then
    cleanup_ok=1
  else
    cleanup_ok=0
  fi
}

finish() {
  local code=$?
  trap - EXIT INT TERM HUP
  set +o errexit
  if [[ "$window_entered" -eq 1 ]]; then
    run_cleanup
  fi
  write_result
  if [[ "$(m8emb6_secret_scan_count "$EVIDENCE")" != '0' ]]; then
    cleanup_ok=0
    reason='secret_scan_failed'
    write_result
    code=1
  fi
  m8emb6_seal_directory "$EVIDENCE"
  printf 'formal_evidence=%s\nformal_manifest_sha256=%s\ncleanup_evidence=%s\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$CLEANUP_EVIDENCE" >&3
  if [[ "$smoke_passed" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    exit 0
  fi
  [[ "$code" -ne 0 ]] || code=1
  exit "$code"
}
trap finish EXIT
trap 'reason=terminated; exit 143' TERM HUP
trap 'reason=interrupted; exit 130' INT

fail() {
  reason="$1"
  printf 'FAIL: %s\n' "$reason" >&2
  return 1
}

phase='revalidate_preflight'
cd "$M8EMB6_REPO"
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]] || fail 'source_head_drift'
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]] ||
  fail 'origin_head_drift'
[[ -z "$(git status --porcelain)" ]] || fail 'source_worktree_dirty'
m8emb6_verify_model || fail 'model_identity_drift'
existing_worker_id="$(jq -r '.worker_id' "$PREFLIGHT/recovery-plan.json")"
existing_worker_uuid="$(jq -r '.worker_uuid' "$PREFLIGHT/recovery-plan.json")"
existing_instance_id="$(jq -r '.instance_id' "$PREFLIGHT/recovery-plan.json")"
existing_container_id="$(jq -r '.container_id' "$PREFLIGHT/recovery-plan.json")"
m8emb6_process_identity_matches "$PREFLIGHT/existing-worker-process.txt" ||
  fail 'existing_worker_process_drift'
m8emb6_capture_container "$existing_container_id" "$EVIDENCE/existing-container-recheck.json"
cmp "$PREFLIGHT/existing-container.json" "$EVIDENCE/existing-container-recheck.json" ||
  fail 'existing_container_drift'

phase='enter_window'
window_entered=1
m8emb6_set_worker_maintenance "$M8EMB6_RUST106_CONTEXT" "$existing_worker_id" true \
  'M8-EMB.6 Python Server compatibility window' \
  "$EVIDENCE/rust106-maintenance-enable.json" || fail 'rust106_maintenance_enable_failed'
jq -e '.after.enabled == true' "$EVIDENCE/rust106-maintenance-enable.json" >/dev/null ||
  fail 'rust106_maintenance_not_enabled'

existing_pid="$(sed -n 's/^pid=//p' "$PREFLIGHT/existing-worker-process.txt")"
m8emb6_process_identity_matches "$PREFLIGHT/existing-worker-process.txt" ||
  fail 'existing_worker_identity_changed_before_stop'
kill -TERM "$existing_pid"
for _ in $(seq 1 90); do
  [[ ! -r "/proc/$existing_pid/stat" ]] && break
  sleep 1
done
[[ ! -r "/proc/$existing_pid/stat" ]] || fail 'existing_worker_stop_timeout'

m8emb6_capture_container "$existing_container_id" "$EVIDENCE/existing-container-before-stop.json"
m8emb6_validate_container_tuple "$EVIDENCE/existing-container-before-stop.json" \
  "$existing_worker_uuid" "$existing_instance_id" || fail 'existing_container_identity_changed'
docker stop --time 120 "$existing_container_id" >"$EVIDENCE/existing-container-stop.txt" ||
  fail 'existing_container_stop_failed'
for _ in $(seq 1 60); do
  [[ "$(m8emb6_gpu_compute_count "$M8EMB6_GPU0_UUID")" == '0' ]] && break
  sleep 1
done
[[ "$(m8emb6_gpu_compute_count "$M8EMB6_GPU0_UUID")" == '0' ]] ||
  fail 'gpu0_not_empty_after_existing_stop'
m8emb6_capture_gpu "$EVIDENCE" window-entered

phase='start_experimental_worker'
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/models?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-models-window-entry.json")"
[[ "$python_status" == '200' ]] || fail 'python_model_recheck_failed'
jq -e '.items | length == 0' "$EVIDENCE/python-models-window-entry.json" >/dev/null ||
  fail 'python_models_appeared_before_worker_start'
mapfile -d '' -t worker_arguments < <(m8emb6_worker_args)
setsid "$M8EMB6_BINARY" --output json worker "${worker_arguments[@]}" run \
  >"$EVIDENCE/experimental-worker-events.log" \
  2>"$EVIDENCE/experimental-worker.stderr" &
experimental_pid=$!
for _ in $(seq 1 100); do
  if [[ -r "/proc/$experimental_pid/stat" ]] &&
    [[ "$(m8emb6_process_cmdline_sha "$experimental_pid")" == \
      "$(m8emb6_expected_worker_cmdline_sha)" ]]; then
    break
  fi
  sleep 0.1
done
[[ -r "/proc/$experimental_pid/stat" ]] || fail 'experimental_worker_exited_during_start'
m8emb6_record_process_identity "$experimental_pid" experimental_python_worker \
  "$EVIDENCE/experimental-worker-process.txt"
[[ "$(sed -n 's/^cmdline_sha256=//p' "$EVIDENCE/experimental-worker-process.txt")" == \
  "$(m8emb6_expected_worker_cmdline_sha)" ]] || fail 'experimental_worker_cmdline_mismatch'

worker_registered=0
for _ in $(seq 1 600); do
  python_status="$(m8emb6_auth_request GET \
    "${M8EMB6_PYTHON_SERVER_URL}v2/workers?page=-1" \
    "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers-registration.json")"
  if [[ "$python_status" == '200' ]] &&
    [[ "$(jq -r --arg name "$M8EMB6_WORKER_NAME" \
      '[.items[] | select(.name == $name)] | length' \
      "$EVIDENCE/python-workers-registration.json")" == '1' ]]; then
    jq --arg name "$M8EMB6_WORKER_NAME" \
      '[.items[] | select(.name == $name)][0]' \
      "$EVIDENCE/python-workers-registration.json" >"$EVIDENCE/experimental-worker.json"
    worker_id="$(jq -r '.id' "$EVIDENCE/experimental-worker.json")"
    worker_uuid="$(jq -r '.uuid' "$EVIDENCE/experimental-worker.json")"
    [[ "$worker_id" =~ ^[1-9][0-9]*$ && "$worker_uuid" =~ ^[0-9a-f-]{36}$ ]] ||
      fail 'experimental_worker_identity_invalid'
    m8emb6_set_worker_maintenance "$M8EMB6_PYTHON_CONTEXT" "$worker_id" true \
      'M8-EMB.6 registration barrier' \
      "$EVIDENCE/python-maintenance-enable.json" || fail 'experimental_maintenance_enable_failed'
    jq -e '.after.enabled == true' "$EVIDENCE/python-maintenance-enable.json" >/dev/null ||
      fail 'experimental_maintenance_not_enabled'
    worker_registered=1
    break
  fi
  sleep 0.1
done
[[ "$worker_registered" -eq 1 ]] || fail 'experimental_worker_registration_timeout'

worker_ready=0
for _ in $(seq 1 180); do
  kill -0 "$experimental_pid" 2>/dev/null || fail 'experimental_worker_exited'
  if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6_WORKER_IP}:${M8EMB6_WORKER_PORT}/healthz" 2>/dev/null)" == '200' ]] &&
    [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6_WORKER_IP}:${M8EMB6_WORKER_PORT}/readyz" 2>/dev/null)" == '200' ]] &&
    [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
    "http://${M8EMB6_WORKER_IP}:${M8EMB6_METRICS_PORT}/metrics" 2>/dev/null)" == '200' ]]; then
    python_status="$(m8emb6_auth_request GET \
      "${M8EMB6_PYTHON_SERVER_URL}v2/workers?page=-1" \
      "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers-current.json")"
    if [[ "$python_status" == '200' ]] &&
      jq -e --arg name "$M8EMB6_WORKER_NAME" --arg uuid "$worker_uuid" '
        [.items[] | select(
          .name == $name and .uuid == $uuid and .maintenance.enabled == true
        )] | length == 1
      ' "$EVIDENCE/python-workers-current.json" >/dev/null; then
      worker_ready=1
      break
    fi
  fi
  sleep 1
done
[[ "$worker_ready" -eq 1 ]] || fail 'experimental_worker_readiness_timeout'
m8emb6_set_worker_maintenance "$M8EMB6_PYTHON_CONTEXT" "$worker_id" false '' \
  "$EVIDENCE/python-maintenance-disable.json" || fail 'experimental_maintenance_disable_failed'
jq -e '.after.enabled == false' "$EVIDENCE/python-maintenance-disable.json" >/dev/null ||
  fail 'experimental_maintenance_still_enabled'

phase='deploy_embedding_model'
"$M8EMB6_BINARY" --timeout 600 --context "$M8EMB6_PYTHON_CONTEXT" --output json \
  model deploy "$M8EMB6_TARGET_MODEL" \
  --category embedding \
  --runner pooling \
  --worker "$M8EMB6_WORKER_NAME" \
  --local-path "$M8EMB6_MODEL_PATH" \
  --image "$M8EMB6_RUNNER_IMAGE" \
  --gpu 0 \
  --max-model-len 512 \
  --gpu-memory-utilization 0.10 \
  --dtype auto \
  --enforce-eager \
  --wait >"$EVIDENCE/model-deploy.json" \
  2>"$EVIDENCE/model-deploy-transitions.log" || fail 'embedding_model_deploy_failed'
model_id="$(jq -r '.model.id' "$EVIDENCE/model-deploy.json")"
instance_id="$(jq -r '.instances[0].id' "$EVIDENCE/model-deploy.json")"
[[ "$model_id" =~ ^[1-9][0-9]*$ && "$instance_id" =~ ^[1-9][0-9]*$ ]] ||
  fail 'embedding_resource_identity_invalid'
mapfile -t observed_states < <(jq -r '.transitions[].state' "$EVIDENCE/model-deploy.json")
required_states=(pending analyzing scheduled initializing downloading starting running)
required_index=0
for observed_state in "${observed_states[@]}"; do
  if [[ "$required_index" -lt "${#required_states[@]}" ]] &&
    [[ "$observed_state" == "${required_states[$required_index]}" ]]; then
    required_index=$((required_index + 1))
  fi
done
[[ "$required_index" == "${#required_states[@]}" ]] || fail 'complete_state_chain_missing'
jq -e --argjson worker "$worker_id" '
  .model.categories == ["embedding"] and (.instances | length) == 1 and
  .instances[0].state == "running" and .instances[0].worker_id == $worker and
  .instances[0].gpu_indexes == [0]
' "$EVIDENCE/model-deploy.json" >/dev/null || fail 'embedding_running_contract_mismatch'

mapfile -t owned_containers < <(docker ps -q --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed \
  --filter label=io.gpustack.component=vllm \
  --filter label=io.gpustack.worker-uuid="$worker_uuid" \
  --filter label=io.gpustack.model-instance-id="$instance_id")
[[ "${#owned_containers[@]}" == '1' ]] || fail 'embedding_container_count_invalid'
container_id="${owned_containers[0]}"
m8emb6_capture_container "$container_id" "$EVIDENCE/embedding-container-running.json"
m8emb6_validate_container_tuple "$EVIDENCE/embedding-container-running.json" \
  "$worker_uuid" "$instance_id" || fail 'embedding_container_identity_invalid'
[[ "$(jq -r '.Image' "$EVIDENCE/embedding-container-running.json")" == \
  "$M8EMB6_RUNNER_IMAGE_ID" ]] || fail 'embedding_runner_image_mismatch'
jq -e --arg gpu "$M8EMB6_GPU0_UUID" '
  .device_requests[0].DeviceIDs == [$gpu] and
  (.command | index("--runner=pooling")) != null and
  any(range(0; (.command | length) - 1);
    .command[.] == "--max-model-len" and .command[. + 1] == "512")
' "$EVIDENCE/embedding-container-running.json" >/dev/null ||
  fail 'embedding_container_command_or_gpu_mismatch'
spec_hash="$(jq -r '.labels["io.gpustack.spec-hash"]' \
  "$EVIDENCE/embedding-container-running.json")"
[[ "$spec_hash" =~ ^[0-9a-f]{64}$ ]] || fail 'embedding_spec_hash_invalid'

phase='embedding_inference'
"$M8EMB6_BINARY" --timeout 120 --context "$M8EMB6_PYTHON_CONTEXT" --output json \
  inference embeddings --model "$M8EMB6_TARGET_MODEL" \
  --input 'GPUStack是一个GPU集群管理平台。' \
  --input 'Rust Worker提供模型推理能力。' \
  --encoding-format float >"$EVIDENCE/server-embeddings-float.json" ||
  fail 'server_float_embeddings_failed'
jq -e --argjson dimension "$M8EMB6_EMBEDDING_DIMENSION" '
  (.data | length) == 2 and
  all(.data[]; (.embedding | type) == "array" and
    (.embedding | length) == $dimension and all(.embedding[]; type == "number"))
' "$EVIDENCE/server-embeddings-float.json" >/dev/null || fail 'float_embedding_shape_invalid'

"$M8EMB6_BINARY" --timeout 120 --context "$M8EMB6_PYTHON_CONTEXT" --output json \
  inference embeddings --model "$M8EMB6_TARGET_MODEL" \
  --input 'GPUStack embedding base64 verification' \
  --encoding-format base64 >"$EVIDENCE/server-embeddings-base64.json" ||
  fail 'server_base64_embeddings_failed'
jq -r '.data[0].embedding' "$EVIDENCE/server-embeddings-base64.json" |
  base64 --decode >"$EVIDENCE/base64-vector.bin"
[[ "$(stat -c '%s' "$EVIDENCE/base64-vector.bin")" == \
  "$((M8EMB6_EMBEDDING_DIMENSION * 4))" ]] || fail 'base64_embedding_size_invalid'
unlink "$EVIDENCE/base64-vector.bin"

"$M8EMB6_BINARY" --timeout 120 --context "$M8EMB6_PYTHON_CONTEXT" \
  inference embeddings --model "$M8EMB6_TARGET_MODEL" \
  --input 'GPUStack table verification' --encoding-format float \
  >"$EVIDENCE/server-embeddings-table.txt" || fail 'table_embeddings_failed'
grep -Eq $'0\t512\tfloat\t' "$EVIDENCE/server-embeddings-table.txt" ||
  fail 'table_embedding_dimension_missing'

jq -n --arg model "$M8EMB6_TARGET_MODEL" \
  '{model:$model,input:["Worker direct embedding"],encoding_format:"float"}' \
  >"$EVIDENCE/worker-direct-request.json"
destination="model-${model_id}-${instance_id}.static"
cluster_token="$(tr -d '\r\n' <"$M8EMB6_CLUSTER_TOKEN")"
exec {direct_fd}<<<"header = \"Authorization: Bearer $cluster_token\""
direct_status="$(curl --config "/dev/fd/$direct_fd" --silent --show-error --max-time 120 \
  --header 'Content-Type: application/json' \
  --header "X-GPUStack-Model-Instance: $destination" \
  --data-binary "@$EVIDENCE/worker-direct-request.json" \
  --output "$EVIDENCE/worker-direct-response.json" --write-out '%{http_code}' \
  "http://${M8EMB6_WORKER_IP}:${M8EMB6_WORKER_PORT}/v1/embeddings")"
exec {direct_fd}<&-
unset cluster_token
[[ "$direct_status" == '200' ]] || fail 'worker_direct_embeddings_failed'
jq -e --argjson dimension "$M8EMB6_EMBEDDING_DIMENSION" \
  '(.data[0].embedding | type) == "array" and
    (.data[0].embedding | length) == $dimension' \
  "$EVIDENCE/worker-direct-response.json" >/dev/null || fail 'worker_direct_shape_invalid'

phase='repeat_and_idempotency'
for sample in 1 2 3; do
  "$M8EMB6_BINARY" --timeout 120 --context "$M8EMB6_PYTHON_CONTEXT" --output json \
    inference embeddings --model "$M8EMB6_TARGET_MODEL" \
    --input 'GPUStack repeated embedding request' --encoding-format float \
    >"$EVIDENCE/repeat-${sample}.json" || fail "repeat_${sample}_failed"
  jq -e --argjson dimension "$M8EMB6_EMBEDDING_DIMENSION" \
    '(.data[0].embedding | type) == "array" and
      (.data[0].embedding | length) == $dimension' \
    "$EVIDENCE/repeat-${sample}.json" >/dev/null || fail "repeat_${sample}_shape_invalid"
  m8emb6_capture_container "$container_id" "$EVIDENCE/container-repeat-${sample}.json"
  jq -e --arg id "$container_id" --arg hash "$spec_hash" '
    .Id == $id and .restart_count == 0 and
    .labels["io.gpustack.spec-hash"] == $hash and .running == true
  ' "$EVIDENCE/container-repeat-${sample}.json" >/dev/null ||
    fail "repeat_${sample}_runtime_identity_drift"
done
smoke_passed=1
reason='smoke_passed'

phase='cleanup_and_restore'
run_cleanup
[[ "$cleanup_ok" -eq 1 ]] || fail 'cleanup_or_restore_failed'
phase='complete'
reason='accepted_gates_passed'
