#!/usr/bin/env bash
# Exact M8-EMB.6 cleanup and restoration; safe to invoke after partial execution.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly PREFLIGHT="${M8EMB6_PREFLIGHT_EVIDENCE:?M8EMB6_PREFLIGHT_EVIDENCE is required}"
readonly EXPECTED_PREFLIGHT_SHA="${M8EMB6_PREFLIGHT_MANIFEST_SHA:?M8EMB6_PREFLIGHT_MANIFEST_SHA is required}"
readonly FORMAL_EVIDENCE="${M8EMB6_EVIDENCE:?M8EMB6_EVIDENCE is required}"
readonly EVIDENCE="${M8EMB6_CLEANUP_EVIDENCE:?M8EMB6_CLEANUP_EVIDENCE is required}"

[[ -d "$PREFLIGHT" && ! -L "$PREFLIGHT" ]] || m8emb6_die "preflight evidence is unavailable"
[[ "$(sha256sum "$PREFLIGHT/manifest.sha256" | awk '{print $1}')" == \
  "$EXPECTED_PREFLIGHT_SHA" ]] || m8emb6_die "preflight manifest identity mismatch"
(cd "$PREFLIGHT" && sha256sum --check manifest.sha256) >/dev/null
[[ -f "$PREFLIGHT/recovery-plan.json" ]] || m8emb6_die "recovery plan is unavailable"
if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  m8emb6_die "cleanup evidence path already exists: $EVIDENCE"
fi
install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

cleanup_ok=1
current_step='initialization'
finish() {
  local code=$?
  trap - EXIT
  if [[ "$code" -ne 0 ]]; then
    cleanup_ok=0
  fi
  printf 'status=%s\nstage=M8-EMB.6-cleanup\ncleanup_ok=%s\nfailed_step=%s\n' \
    "$([[ "$cleanup_ok" -eq 1 ]] && printf PASS || printf FAIL)" \
    "$cleanup_ok" "$current_step" >"$EVIDENCE/result.txt"
  m8emb6_seal_directory "$EVIDENCE"
  printf 'cleanup_ok=%s\ncleanup_evidence=%s\n' "$cleanup_ok" "$EVIDENCE" >&3
  [[ "$cleanup_ok" -eq 1 ]] || exit 1
}
trap finish EXIT

existing_worker_id="$(jq -r '.worker_id' "$PREFLIGHT/recovery-plan.json")"
existing_worker_uuid="$(jq -r '.worker_uuid' "$PREFLIGHT/recovery-plan.json")"
existing_model_id="$(jq -r '.model_id' "$PREFLIGHT/recovery-plan.json")"
existing_instance_id="$(jq -r '.instance_id' "$PREFLIGHT/recovery-plan.json")"
existing_container_id="$(jq -r '.container_id' "$PREFLIGHT/recovery-plan.json")"
existing_maintenance="$(jq -r '.maintenance_enabled' "$PREFLIGHT/recovery-plan.json")"

current_step='stop_experimental_worker'
if [[ -f "$FORMAL_EVIDENCE/experimental-worker-process.txt" ]]; then
  experimental_pid="$(sed -n 's/^pid=//p' "$FORMAL_EVIDENCE/experimental-worker-process.txt")"
  if [[ -r "/proc/$experimental_pid/stat" ]]; then
    m8emb6_process_identity_matches "$FORMAL_EVIDENCE/experimental-worker-process.txt" ||
      m8emb6_die "experimental Worker process identity drifted"
    kill -TERM "$experimental_pid"
    for _ in $(seq 1 60); do
      [[ ! -r "/proc/$experimental_pid/stat" ]] && break
      sleep 1
    done
    [[ ! -r "/proc/$experimental_pid/stat" ]] || m8emb6_die "experimental Worker did not stop"
  fi
else
  mapfile -t fallback_pids < <(pgrep -f \
    '[g]pustack-rs.*worker.*--worker-name rust-m8emb6-r760xa.*run' || true)
  [[ "${#fallback_pids[@]}" -le 1 ]] || m8emb6_die "experimental Worker process is ambiguous"
  if [[ "${#fallback_pids[@]}" -eq 1 ]]; then
    fallback_pid="${fallback_pids[0]}"
    [[ "$(m8emb6_process_cmdline_sha "$fallback_pid")" == \
      "$(m8emb6_expected_worker_cmdline_sha)" ]] ||
      m8emb6_die "fallback experimental Worker cmdline is invalid"
    m8emb6_record_process_identity "$fallback_pid" fallback_experimental_worker \
      "$EVIDENCE/fallback-experimental-worker.txt"
    kill -TERM "$fallback_pid"
    for _ in $(seq 1 60); do
      [[ ! -r "/proc/$fallback_pid/stat" ]] && break
      sleep 1
    done
    [[ ! -r "/proc/$fallback_pid/stat" ]] ||
      m8emb6_die "fallback experimental Worker did not stop"
  fi
fi

current_step='delete_experimental_model'
experimental_instance_id=''
experimental_worker_id=''
experimental_worker_uuid=''
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/workers?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers-discovery.json")"
[[ "$python_status" == '200' ]]
experimental_worker_uuid="$(jq -r --arg name "$M8EMB6_WORKER_NAME" \
  '[.items[] | select(.name == $name)] |
   if length == 1 then .[0].uuid else empty end' \
  "$EVIDENCE/python-workers-discovery.json")"
experimental_worker_id="$(jq -r --arg name "$M8EMB6_WORKER_NAME" \
  '[.items[] | select(.name == $name)] |
   if length == 1 then .[0].id else empty end' \
  "$EVIDENCE/python-workers-discovery.json")"
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/models?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-models-before.json")"
[[ "$python_status" == '200' ]]
mapfile -t target_model_ids < <(jq -r --arg name "$M8EMB6_TARGET_MODEL" \
  '.items[] | select(.name == $name) | .id' "$EVIDENCE/python-models-before.json")
[[ "${#target_model_ids[@]}" -le 1 ]] || m8emb6_die "experimental model identity is ambiguous"
if [[ "${#target_model_ids[@]}" -eq 1 ]]; then
  target_model_id="${target_model_ids[0]}"
  python_status="$(m8emb6_auth_request GET \
    "${M8EMB6_PYTHON_SERVER_URL}v2/model-instances?page=-1&model_id=$target_model_id" \
    "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-instances-discovery.json")"
  [[ "$python_status" == '200' ]]
  experimental_instance_id="$(jq -r '
    if (.items | length) == 1 then .items[0].id else empty end
  ' "$EVIDENCE/python-instances-discovery.json")"
  if [[ -f "$FORMAL_EVIDENCE/result.txt" ]]; then
    recorded_model_id="$(sed -n 's/^model_id=//p' "$FORMAL_EVIDENCE/result.txt")"
    [[ -z "$recorded_model_id" || "$recorded_model_id" == 'none' ||
      "$recorded_model_id" == "$target_model_id" ]] ||
      m8emb6_die "experimental model ID conflicts with formal evidence"
  fi
  python_status="$(m8emb6_auth_request DELETE \
    "${M8EMB6_PYTHON_SERVER_URL}v2/models/$target_model_id" \
    "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/model-delete.json")"
  [[ "$python_status" == '200' || "$python_status" == '204' ]]
fi
for _ in $(seq 1 180); do
  python_status="$(m8emb6_auth_request GET \
    "${M8EMB6_PYTHON_SERVER_URL}v2/models?page=-1" \
    "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-models-current.json")"
  [[ "$python_status" == '200' ]]
  if jq -e --arg name "$M8EMB6_TARGET_MODEL" \
    '[.items[] | select(.name == $name)] | length == 0' \
    "$EVIDENCE/python-models-current.json" >/dev/null; then
    break
  fi
  sleep 1
done
jq -e --arg name "$M8EMB6_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/python-models-current.json" >/dev/null

current_step='remove_exact_experimental_runtime'
if [[ -f "$FORMAL_EVIDENCE/result.txt" ]]; then
  recorded_worker_id="$(sed -n 's/^worker_id=//p' "$FORMAL_EVIDENCE/result.txt")"
  recorded_instance_id="$(sed -n 's/^model_instance_id=//p' "$FORMAL_EVIDENCE/result.txt")"
  recorded_worker_uuid="$(sed -n 's/^worker_uuid=//p' "$FORMAL_EVIDENCE/result.txt")"
  if [[ "$recorded_worker_id" =~ ^[1-9][0-9]*$ ]]; then
    [[ -z "$experimental_worker_id" || "$experimental_worker_id" == "$recorded_worker_id" ]] ||
      m8emb6_die "experimental Worker ID discovery conflicts with formal evidence"
    experimental_worker_id="$recorded_worker_id"
  fi
  if [[ "$recorded_instance_id" =~ ^[1-9][0-9]*$ ]]; then
    [[ -z "$experimental_instance_id" || "$experimental_instance_id" == "$recorded_instance_id" ]] ||
      m8emb6_die "experimental instance discovery conflicts with formal evidence"
    experimental_instance_id="$recorded_instance_id"
  fi
  if [[ "$recorded_worker_uuid" =~ ^[0-9a-f-]{36}$ ]]; then
    [[ -z "$experimental_worker_uuid" || "$experimental_worker_uuid" == "$recorded_worker_uuid" ]] ||
      m8emb6_die "experimental Worker UUID discovery conflicts with formal evidence"
    experimental_worker_uuid="$recorded_worker_uuid"
  fi
fi

current_step='delete_experimental_model_files'
if [[ "$experimental_worker_id" =~ ^[1-9][0-9]*$ ]]; then
  python_status="$(m8emb6_auth_request GET \
    "${M8EMB6_PYTHON_SERVER_URL}v2/model-files?page=-1&worker_id=$experimental_worker_id" \
    "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-model-files-before.json")"
  [[ "$python_status" == '200' ]]
  jq -e --arg path "$M8EMB6_MODEL_PATH" '
    all(.items[]; .local_path == $path or (.resolved_paths | index($path)) != null)
  ' "$EVIDENCE/python-model-files-before.json" >/dev/null
  mapfile -t experimental_model_file_ids < <(
    jq -r '.items[].id' "$EVIDENCE/python-model-files-before.json"
  )
  for model_file_id in "${experimental_model_file_ids[@]}"; do
    [[ "$model_file_id" =~ ^[1-9][0-9]*$ ]] ||
      m8emb6_die "experimental ModelFile ID is invalid"
    python_status="$(m8emb6_auth_request DELETE \
      "${M8EMB6_PYTHON_SERVER_URL}v2/model-files/$model_file_id?cleanup=false" \
      "$M8EMB6_PYTHON_OPERATOR_TOKEN" \
      "$EVIDENCE/model-file-${model_file_id}-delete.json")"
    [[ "$python_status" == '200' || "$python_status" == '204' ]]
  done
fi

if [[ "$experimental_instance_id" =~ ^[1-9][0-9]*$ ]] &&
  [[ "$experimental_worker_uuid" =~ ^[0-9a-f-]{36}$ ]]; then
  mapfile -t experimental_containers < <(docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed \
    --filter label=io.gpustack.component=vllm \
    --filter label=io.gpustack.worker-uuid="$experimental_worker_uuid" \
    --filter label=io.gpustack.model-instance-id="$experimental_instance_id")
  [[ "${#experimental_containers[@]}" -le 1 ]] ||
    m8emb6_die "experimental container identity is ambiguous"
  if [[ "${#experimental_containers[@]}" -eq 1 ]]; then
    m8emb6_capture_container "${experimental_containers[0]}" \
      "$EVIDENCE/experimental-container-before-remove.json"
    m8emb6_validate_container_tuple "$EVIDENCE/experimental-container-before-remove.json" \
      "$experimental_worker_uuid" "$experimental_instance_id"
    docker rm --force "${experimental_containers[0]}" \
      >"$EVIDENCE/experimental-container-remove.txt"
  fi
fi

current_step='delete_experimental_worker'
python_status="$(m8emb6_auth_request GET \
  "${M8EMB6_PYTHON_SERVER_URL}v2/workers?page=-1" \
  "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/python-workers-before.json")"
[[ "$python_status" == '200' ]]
mapfile -t target_worker_ids < <(jq -r --arg name "$M8EMB6_WORKER_NAME" \
  '.items[] | select(.name == $name) | .id' "$EVIDENCE/python-workers-before.json")
[[ "${#target_worker_ids[@]}" -le 1 ]] || m8emb6_die "experimental Worker identity is ambiguous"
if [[ "${#target_worker_ids[@]}" -eq 1 ]]; then
  python_status="$(m8emb6_auth_request DELETE \
    "${M8EMB6_PYTHON_SERVER_URL}v2/workers/${target_worker_ids[0]}" \
    "$M8EMB6_PYTHON_OPERATOR_TOKEN" "$EVIDENCE/worker-delete.json")"
  [[ "$python_status" == '200' || "$python_status" == '204' ]]
fi
m8emb6_remove_owned_tree "$M8EMB6_DATA_DIR" "$M8EMB6_DATA_DIR"
m8emb6_remove_owned_tree "$M8EMB6_LOG_DIR" "$M8EMB6_LOG_DIR"

current_step='restore_existing_container'
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/model-instances/$existing_instance_id" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/existing-instance-before-restore.json")"
[[ "$rust_status" == '200' ]]
jq -e --argjson model "$existing_model_id" --argjson worker "$existing_worker_id" \
  '.model_id == $model and .worker_id == $worker' \
  "$EVIDENCE/existing-instance-before-restore.json" >/dev/null
m8emb6_capture_container "$existing_container_id" "$EVIDENCE/existing-container-before-restore.json"
m8emb6_validate_container_tuple "$EVIDENCE/existing-container-before-restore.json" \
  "$existing_worker_uuid" "$existing_instance_id"
if [[ "$(jq -r '.running' "$EVIDENCE/existing-container-before-restore.json")" != 'true' ]]; then
  [[ "$(m8emb6_gpu_compute_count "$M8EMB6_GPU0_UUID")" == '0' ]] ||
    m8emb6_die "GPU0 is not empty before restoring the existing container"
  docker start "$existing_container_id" >"$EVIDENCE/existing-container-start.txt"
fi

current_step='restore_existing_worker'
existing_identity="$PREFLIGHT/existing-worker-process.txt"
existing_pid="$(sed -n 's/^pid=//p' "$existing_identity")"
if [[ ! -r "/proc/$existing_pid/stat" ]]; then
  mapfile -d '' -t original_argv <"$PREFLIGHT/existing-worker-cmdline.bin"
  [[ "${#original_argv[@]}" -gt 2 ]] || m8emb6_die "captured existing Worker argv is invalid"
  [[ "$(stat -c '%a' "$PREFLIGHT/existing-worker-binary")" == '700' ]] ||
    m8emb6_die "captured existing Worker binary mode is invalid"
  setsid bash -c 'exec -a "$1" "$2" "${@:3}"' m8emb6-restore \
    "${original_argv[0]}" "$PREFLIGHT/existing-worker-binary" \
    "${original_argv[@]:1}" \
    >"$EVIDENCE/existing-worker-restore.stdout" \
    2>"$EVIDENCE/existing-worker-restore.stderr" &
  restored_pid=$!
  restored_identity_ready=0
  for _ in $(seq 1 100); do
    if [[ -r "/proc/$restored_pid/stat" ]] &&
      [[ "$(m8emb6_process_cmdline_sha "$restored_pid")" == \
        "$(sed -n 's/^cmdline_sha256=//p' "$existing_identity")" ]] &&
      [[ "$(sha256sum "/proc/$restored_pid/exe" | awk '{print $1}')" == \
        "$(sed -n 's/^exe_sha256=//p' "$existing_identity")" ]]; then
      restored_identity_ready=1
      break
    fi
    sleep 0.1
  done
  [[ "$restored_identity_ready" -eq 1 ]] ||
    m8emb6_die "restored 106 Worker process identity did not converge"
  m8emb6_record_process_identity "$restored_pid" restored_rust106_worker \
    "$EVIDENCE/restored-worker-process.txt"
else
  m8emb6_process_identity_matches "$existing_identity" ||
    m8emb6_die "surviving existing Worker identity drifted"
  restored_pid="$existing_pid"
fi

restore_ready=0
for _ in $(seq 1 240); do
  kill -0 "$restored_pid" 2>/dev/null || m8emb6_die "restored 106 Worker exited"
  rust_status="$(m8emb6_auth_request GET \
    "${M8EMB6_RUST106_SERVER_URL}v2/workers/$existing_worker_id" \
    "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/existing-worker-current.json" || true)"
  if [[ "$rust_status" == '200' ]] &&
    jq -e --arg uuid "$existing_worker_uuid" \
      '.uuid == $uuid and .unreachable == false' \
      "$EVIDENCE/existing-worker-current.json" >/dev/null; then
    restore_ready=1
    break
  fi
  sleep 1
done
[[ "$restore_ready" -eq 1 ]] || m8emb6_die "106 Worker did not recover"

current_step='restore_maintenance'
rust_status="$(m8emb6_auth_request GET \
  "${M8EMB6_RUST106_SERVER_URL}v2/workers/$existing_worker_id" \
  "$M8EMB6_RUST106_OPERATOR_TOKEN" "$EVIDENCE/worker-before-maintenance-restore.json")"
[[ "$rust_status" == '200' ]]
current_maintenance="$(jq -r '(.maintenance.enabled // false)' \
  "$EVIDENCE/worker-before-maintenance-restore.json")"
if [[ "$current_maintenance" != "$existing_maintenance" ]]; then
  [[ "$existing_maintenance" == 'false' ]] ||
    m8emb6_die "preflight maintenance baseline is unsupported"
  m8emb6_set_worker_maintenance "$M8EMB6_RUST106_CONTEXT" "$existing_worker_id" \
    false '' "$EVIDENCE/maintenance-restore-response.json"
  jq -e '.after.enabled == false' "$EVIDENCE/maintenance-restore-response.json" >/dev/null
else
  jq -n --argjson enabled "$existing_maintenance" \
    '{changed:false,after:{enabled:$enabled}}' >"$EVIDENCE/maintenance-restore-response.json"
fi

current_step='final_checks'
m8emb6_capture_gpu "$EVIDENCE" after
m8emb6_capture_container "$existing_container_id" "$EVIDENCE/existing-container-after.json"
m8emb6_validate_container_tuple "$EVIDENCE/existing-container-after.json" \
  "$existing_worker_uuid" "$existing_instance_id"
[[ "$(jq -r '.running' "$EVIDENCE/existing-container-after.json")" == 'true' ]]
[[ "$(m8emb6_secret_scan_count "$EVIDENCE")" == '0' ]]
cleanup_ok=1
