#!/usr/bin/env bash
# shellcheck disable=SC1090,SC1091,SC2317
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly LIB="${M7E1_LIB:?M7E1_LIB is required}"
readonly EXPECTED_LIB_SHA="${M7E1_LIB_SHA:?M7E1_LIB_SHA is required}"
if [[ ! -f "$LIB" ]] ||
  [[ "$(sha256sum "$LIB" | awk '{print $1}')" != "$EXPECTED_LIB_SHA" ]]; then
  printf 'STOP: shared library identity mismatch\n' >&2
  exit 88
fi
source "$LIB"

readonly EVIDENCE="${M7E1_EVIDENCE:?M7E1_EVIDENCE is required}"
readonly PREFLIGHT="${M7E1_PREFLIGHT:?M7E1_PREFLIGHT is required}"
readonly WATCHDOG="${M7E1_WATCHDOG:?M7E1_WATCHDOG is required}"
readonly EXPECTED_WATCHDOG_SHA="${M7E1_WATCHDOG_SHA:?M7E1_WATCHDOG_SHA is required}"
readonly EXPECTED_HEAD="${M7E1_EXPECTED_HEAD:?M7E1_EXPECTED_HEAD is required}"
readonly PRODUCT_COMMIT="${M7E1_PRODUCT_COMMIT:?M7E1_PRODUCT_COMMIT is required}"
readonly EXPECTED_PREFLIGHT_MANIFEST_SHA="${M7E1_PREFLIGHT_MANIFEST_SHA:?M7E1_PREFLIGHT_MANIFEST_SHA is required}"
readonly EXPECTED_LOCK_SHA="${M7E1_LOCK_SHA:?M7E1_LOCK_SHA is required}"
readonly FORMAL_ATTEMPT="${M7E1_ATTEMPT:?M7E1_ATTEMPT is required}"

if [[ ! "$FORMAL_ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7E1_ATTEMPT must be a positive integer\n' >&2
  exit 89
fi
if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi
if [[ ! -f "$WATCHDOG" ]] ||
  [[ "$(sha256sum "$WATCHDOG" | awk '{print $1}')" != "$EXPECTED_WATCHDOG_SHA" ]]; then
  printf 'STOP: watchdog identity mismatch\n' >&2
  exit 91
fi

install -d -m 0700 "$EVIDENCE"
install -m 0600 "$0" "$EVIDENCE/executor.sh"
install -m 0600 "$WATCHDOG" "$EVIDENCE/watchdog.sh"
install -m 0600 "$LIB" "$EVIDENCE/lib.sh"
exec 3>&1
exec >"$EVIDENCE/live.log" 2>&1

phase='initializing'
failure_reason=''
baseline_ready=0
window_entered=0
recovery_passed=0
cleanup_ok=1
finalizing=0
python_worker_id=''
rust_pid=''
model_id=''
instance_id=''
model_file_id=''
assigned_port=''
owned_container_id=''
owned_spec_hash=''
crash_started_epoch=''
adoption_completed_epoch=''

event() {
  jq -cn \
    --arg observed_at "$(m7e1_timestamp)" \
    --arg phase "$phase" \
    --arg kind "$1" \
    --arg detail "$2" \
    '{observed_at:$observed_at,phase:$phase,kind:$kind,detail:$detail}' \
    >>"$EVIDENCE/timeline.ndjson"
}

fail() {
  failure_reason="$1"
  printf 'FAIL: %s (phase=%s)\n' "$failure_reason" "$phase" >&2
  event failure "$failure_reason"
  exit 1
}

require_equal() {
  [[ "$1" == "$2" ]] || fail "$3"
}

require_zero() {
  [[ "$1" == '0' ]] || fail "$2"
}

declare -a worker_args
mapfile -d '' -t worker_args < <(m7e1_fixed_worker_args)

write_process_identity() {
  local pid="$1"
  local output="$2"
  local exe sha cmdline_sha starttime pgid sid
  [[ -r "/proc/$pid/stat" ]] || return 1
  exe="$(readlink -f "/proc/$pid/exe")"
  sha="$(sha256sum "$exe" | awk '{print $1}')"
  cmdline_sha="$(m7e1_process_cmdline_sha "$pid")"
  starttime="$(m7e1_process_starttime "$pid")"
  pgid="$(m7e1_process_pgid "$pid")"
  sid="$(m7e1_process_sid "$pid")"
  [[ "$exe" == "$(readlink -f "$M7E1_BINARY")" ]] || return 1
  [[ "$cmdline_sha" == "$(m7e1_expected_worker_cmdline_sha)" ]] || return 1
  [[ "$pgid" == "$pid" && "$sid" == "$pid" ]] || return 1
  {
    printf 'pid=%s\nstarttime=%s\nexe=%s\nsha256=%s\n' \
      "$pid" "$starttime" "$exe" "$sha"
    printf 'cmdline_sha256=%s\npgid=%s\nsid=%s\n' "$cmdline_sha" "$pgid" "$sid"
  } >"$output"
  chmod 0600 "$output"
}

start_rust_worker() {
  local label="$1"
  local summary="$EVIDENCE/rust-summary-${label}.json"
  local events="$EVIDENCE/rust-events-${label}.log"
  setsid "$M7E1_BINARY" --output json worker "${worker_args[@]}" run --managed-runtime \
    >"$summary" \
    2> >(while IFS= read -r line; do
      printf '%s\t%s\n' "$(m7e1_timestamp)" "$line"
    done >>"$events") &
  rust_pid=$!
  for _ in $(seq 1 50); do
    [[ -r "/proc/$rust_pid/stat" ]] && break
    sleep 0.1
  done
  kill -0 "$rust_pid" 2>/dev/null || return 1
  write_process_identity "$rust_pid" "$EVIDENCE/rust-process-${label}.txt" || return 1
  install -m 0600 "$EVIDENCE/rust-process-${label}.txt" \
    "$EVIDENCE/rust-process-current.txt"
  printf '%s\n' "$rust_pid" >"$EVIDENCE/rust-worker-${label}.pid"
}

wait_for_worker_ready() {
  local output="$1"
  local ready=0
  for _ in $(seq 1 120); do
    kill -0 "$rust_pid" 2>/dev/null || return 1
    if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      "http://$M7E1_WORKER_IP:20150/healthz" 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        "http://$M7E1_WORKER_IP:20150/readyz" 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        "http://$M7E1_WORKER_IP:20151/metrics" 2>/dev/null)" == '200' ]] &&
      "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
        worker get "$M7E1_WORKER_ID" >"$output" 2>"$output.stderr" &&
      jq -e '.id == 5 and .unreachable == false and .gpu_count == 2' \
        "$output" >/dev/null; then
      ready=1
      break
    fi
    sleep 1
  done
  [[ "$ready" -eq 1 ]] || return 1
  require_equal "$(m7e1_listener_owner_pids 20150)" "$rust_pid" \
    'worker_api_listener_owner_mismatch'
  require_equal "$(m7e1_listener_owner_pids 20151)" "$rust_pid" \
    'worker_metrics_listener_owner_mismatch'
  m7e1_process_identity_matches_file "$EVIDENCE/rust-process-current.txt"
}

capture_runtime_identity() {
  local prefix="$1"
  local status_file="$EVIDENCE/${prefix}-model-status.json"
  local container_file="$EVIDENCE/${prefix}-container.json"
  local manifest_file="$M7E1_DATA_DIR/managed/manifests/$instance_id.json"
  "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    model status "$model_id" >"$status_file"
  jq -S '{
    model:{id:.model.id,name:.model.name,restart_on_error:.model.restart_on_error},
    instances:[.instances[] | {
      id,state,worker_id,port,restart_count,last_restart_time
    }]
  }' "$status_file" >"$EVIDENCE/${prefix}-runtime-contract.json"
  jq -e \
    --argjson model "$model_id" \
    --arg name "$M7E1_TARGET_MODEL" \
    --argjson instance "$instance_id" \
    --argjson port "$assigned_port" '
      .model.id == $model and .model.name == $name and .model.restart_on_error == true and
      (.instances | length) == 1 and .instances[0].id == $instance and
      .instances[0].state == "running" and .instances[0].worker_id == 5 and
      .instances[0].port == $port and .instances[0].restart_count == 0 and
      .instances[0].last_restart_time == null
    ' "$status_file" >/dev/null || return 1
  require_equal "$(m7e1_owned_container_count "$instance_id")" '1' \
    "${prefix}_owned_container_count_not_one"
  require_equal "$(m7e1_owned_container_ids "$instance_id")" "$owned_container_id" \
    "${prefix}_owned_container_identity_drift"
  m7e1_capture_container "$owned_container_id" "$container_file"
  m7e1_validate_owned_container "$container_file" "$instance_id" "$owned_spec_hash"
  [[ -f "$manifest_file" ]] || return 1
  jq -S . "$manifest_file" >"$EVIDENCE/${prefix}-manifest.json"
  jq -e \
    --argjson instance "$instance_id" \
    --arg uuid "$M7E1_WORKER_UUID" \
    --arg container "$owned_container_id" '
      .schema_version == 1 and .instance_id == $instance and .worker_uuid == $uuid and
      .container_id == $container and .restart_count == 0 and
      .spec.mode == "managed" and .spec.deployment_id == ($instance|tostring) and
      .last_observed_state == "ready"
    ' "$EVIDENCE/${prefix}-manifest.json" >/dev/null || return 1
  find "$M7E1_LOG_DIR/serve" -maxdepth 1 -type f \
    -name "${instance_id}.*.log" -printf '%f\n' | sort \
    >"$EVIDENCE/${prefix}-log-files.txt"
  require_equal "$(wc -l <"$EVIDENCE/${prefix}-log-files.txt")" '1' \
    "${prefix}_log_generation_count"
  grep -qx "${instance_id}.0.log" "$EVIDENCE/${prefix}-log-files.txt" || return 1
  stat -c 'device=%d\ninode=%i\nmode=%a\nowner=%U:%G\n' \
    "$M7E1_LOG_DIR/serve/${instance_id}.0.log" \
    >"$EVIDENCE/${prefix}-log-identity.txt"
  require_equal "$(m7e1_listener_count "$assigned_port")" '1' \
    "${prefix}_managed_listener_missing"
}

compare_runtime_identity() {
  local before="$1"
  local after="$2"
  cmp -s "$EVIDENCE/${before}-runtime-contract.json" \
    "$EVIDENCE/${after}-runtime-contract.json" || return 1
  cmp -s "$EVIDENCE/${before}-container.json" \
    "$EVIDENCE/${after}-container.json" || return 1
  cmp -s "$EVIDENCE/${before}-manifest.json" \
    "$EVIDENCE/${after}-manifest.json" || return 1
  cmp -s "$EVIDENCE/${before}-log-files.txt" \
    "$EVIDENCE/${after}-log-files.txt" || return 1
  cmp -s "$EVIDENCE/${before}-log-identity.txt" \
    "$EVIDENCE/${after}-log-identity.txt"
}

server_inference_suite() {
  local prefix="$1"
  "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json inference models \
    >"$EVIDENCE/${prefix}-inference-models.json" || return 1
  jq -e --arg name "$M7E1_TARGET_MODEL" \
    '[.data[] | select(.id == $name)] | length == 1' \
    "$EVIDENCE/${prefix}-inference-models.json" >/dev/null || return 1
  "$M7E1_BINARY" --timeout 120 --context "$M7E1_OPERATOR_CONTEXT" --output json \
    inference chat --model "$M7E1_TARGET_MODEL" \
    --prompt 'Reply with exactly: rust worker ready' --max-tokens 16 \
    >"$EVIDENCE/${prefix}-chat.json" || return 1
  jq -e '.choices[0].message.content | type == "string" and length > 0' \
    "$EVIDENCE/${prefix}-chat.json" >/dev/null || return 1
  "$M7E1_BINARY" --timeout 120 --context "$M7E1_OPERATOR_CONTEXT" --output json \
    inference completion --model "$M7E1_TARGET_MODEL" --prompt 'Rust is' --max-tokens 16 \
    >"$EVIDENCE/${prefix}-completion.json" || return 1
  jq -e '.choices[0].text | type == "string" and length > 0' \
    "$EVIDENCE/${prefix}-completion.json" >/dev/null || return 1
  "$M7E1_BINARY" --timeout 60 --context "$M7E1_OPERATOR_CONTEXT" \
    model logs "$model_id" --tail 40 >"$EVIDENCE/${prefix}-logs.txt" || return 1
  [[ -s "$EVIDENCE/${prefix}-logs.txt" ]]
}

enable_maintenance() {
  local current="$EVIDENCE/cleanup-worker.json"
  if timeout 30 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    worker get "$M7E1_WORKER_ID" >"$current" 2>"$current.stderr" &&
    [[ "$(jq -r '.maintenance.enabled' "$current")" != 'true' ]]; then
    timeout 40 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
      worker maintenance enable "$M7E1_WORKER_ID" \
      --message "M7-E.1 attempt $FORMAL_ATTEMPT cleanup" \
      >"$EVIDENCE/cleanup-maintenance-enable.json" \
      2>"$EVIDENCE/cleanup-maintenance-enable.stderr" || cleanup_ok=0
  fi
}

discover_attempt_resources() {
  local candidate
  if timeout 30 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    model list >"$EVIDENCE/cleanup-model-discovery.json" \
    2>"$EVIDENCE/cleanup-model-discovery.stderr"; then
    candidate="$(jq -r \
      --arg name "$M7E1_TARGET_MODEL" \
      --slurpfile baseline "$EVIDENCE/before-global-ids.json" '
        [.[] | select(.name == $name) | .id as $id |
          select(($baseline[0].models | index($id)) == null) | $id] |
        if length == 1 then .[0] else empty end
      ' "$EVIDENCE/cleanup-model-discovery.json")"
    if [[ -z "$model_id" ]]; then
      model_id="$candidate"
    elif [[ -n "$candidate" && "$candidate" != "$model_id" ]]; then
      cleanup_ok=0
    fi
  else
    cleanup_ok=0
  fi
  if [[ -n "$model_id" && -z "$instance_id" ]] &&
    [[ "$(m7e1_model_instance_get "http://127.0.0.1/v2/model-instances?page=-1&model_id=$model_id" \
      "$EVIDENCE/cleanup-instance-discovery.json" || true)" == '200' ]]; then
    instance_id="$(jq -r \
      'if (.items | length) == 1 then .items[0].id else empty end' \
      "$EVIDENCE/cleanup-instance-discovery.json")"
  fi
}

delete_attempt_model() {
  local deadline status
  [[ -n "$model_id" ]] || return 0
  timeout 40 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    model delete "$model_id" --yes >"$EVIDENCE/cleanup-model-delete.json" \
    2>"$EVIDENCE/cleanup-model-delete.stderr" || cleanup_ok=0
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  deadline=$((SECONDS + 180))
  while ((SECONDS < deadline)); do
    status="$(m7e1_model_instance_get "http://127.0.0.1/v2/model-instances/$instance_id" \
      "$EVIDENCE/cleanup-instance-latest.json" || true)"
    if [[ "$status" == '404' ]] &&
      [[ "$(m7e1_owned_container_count "$instance_id")" == '0' ]] &&
      [[ ! -e "$M7E1_DATA_DIR/managed/manifests/$instance_id.json" ]] &&
      [[ ! -e "$M7E1_DATA_DIR/managed/cache/$instance_id" ]] &&
      [[ "$(find "$M7E1_LOG_DIR/serve" -maxdepth 1 -type f \
        -name "${instance_id}.*.log" 2>/dev/null | wc -l)" == '0' ]]; then
      printf 'instance_404=true\nowned_container_count=0\nlocal_runtime_state=absent\n' \
        >"$EVIDENCE/cleanup-owned-runtime.txt"
      return 0
    fi
    sleep 1
  done
  cleanup_ok=0
}

delete_attempt_model_file() {
  local list="$EVIDENCE/cleanup-model-files.json"
  local candidate fetched
  local -a candidates
  if ! timeout 30 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    model file list --worker "$M7E1_WORKER_ID" >"$list" 2>"$list.stderr"; then
    cleanup_ok=0
    return
  fi
  mapfile -t candidates < <(jq -r \
    --arg path "$M7E1_MODEL_PATH" \
    --slurpfile baseline "$EVIDENCE/before-global-ids.json" '
      .[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path) |
      .id as $id | select(($baseline[0].model_files | index($id)) == null) | $id
    ' "$list")
  [[ "${#candidates[@]}" -le 1 ]] || {
    cleanup_ok=0
    return
  }
  for candidate in "${candidates[@]:-}"; do
    [[ -n "$candidate" ]] || continue
    fetched="$EVIDENCE/cleanup-model-file-${candidate}.json"
    if timeout 30 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
      model file get "$candidate" >"$fetched" 2>"$fetched.stderr" &&
      jq -e --arg path "$M7E1_MODEL_PATH" '
        .id > 0 and .worker_id == 5 and .source == "local_path" and
        .local_path == $path and ((.cleanup_on_delete == null) or
        (.cleanup_on_delete == false))
      ' "$fetched" >/dev/null; then
      timeout 40 "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
        model file delete "$candidate" --yes \
        >"$EVIDENCE/cleanup-model-file-delete-${candidate}.json" \
        2>"$EVIDENCE/cleanup-model-file-delete-${candidate}.stderr" || cleanup_ok=0
    else
      cleanup_ok=0
    fi
  done
}

stop_current_rust_worker() {
  [[ -n "$rust_pid" ]] || return 0
  if [[ ! -r "/proc/$rust_pid/stat" ]]; then
    wait "$rust_pid" 2>/dev/null || true
    rust_pid=''
    return 0
  fi
  m7e1_terminate_recorded_process "$EVIDENCE/rust-process-current.txt" "$EVIDENCE" || return 1
  wait "$rust_pid" 2>/dev/null || true
  rust_pid=''
}

fallback_cleanup_runtime() {
  local status
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  status="$(m7e1_model_instance_get "http://127.0.0.1/v2/model-instances/$instance_id" \
    "$EVIDENCE/cleanup-instance-after-stop.json" || true)"
  [[ "$status" == '404' ]] || return 1
  if [[ "$(m7e1_owned_container_count "$instance_id")" != '0' ]]; then
    m7e1_remove_exact_owned_container "$instance_id" "$owned_container_id" \
      "$owned_spec_hash" "$EVIDENCE" || return 1
  fi
  m7e1_remove_exact_local_state "$instance_id"
}

final_checks() {
  (cd "$M7E1_MODEL_PATH" && sha256sum --check --strict "$M7E1_MODEL_MANIFEST") \
    >"$EVIDENCE/after-model-manifest-check.txt" 2>&1 || cleanup_ok=0
  "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    worker get "$M7E1_WORKER_ID" >"$EVIDENCE/after-worker.json" \
    2>"$EVIDENCE/after-worker.stderr" || cleanup_ok=0
  jq -e '.id == 5 and .name == "rust-m3d-r760xa" and .maintenance.enabled == true' \
    "$EVIDENCE/after-worker.json" >/dev/null 2>&1 || cleanup_ok=0
  m7e1_global_snapshot "$EVIDENCE" after || cleanup_ok=0
  cmp -s "$EVIDENCE/before-global-ids.json" "$EVIDENCE/after-global-ids.json" || cleanup_ok=0
  m7e1_capture_docker_ids >"$EVIDENCE/after-docker-ids.txt" || cleanup_ok=0
  cmp -s "$EVIDENCE/before-docker-ids.txt" "$EVIDENCE/after-docker-ids.txt" || cleanup_ok=0
  m7e1_capture_python_worker "$EVIDENCE/after-python-worker.json" || cleanup_ok=0
  cmp -s "$EVIDENCE/before-python-worker.json" "$EVIDENCE/after-python-worker.json" || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e1_managed_listener_count)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" == '0' ]] || cleanup_ok=0
  [[ "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" == '0' ]] || cleanup_ok=0
  [[ "$(m7e1_gpu_memory_used 0)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e1_gpu_memory_used 1)" == '0' ]] || cleanup_ok=0
  m7e1_capture_gpu "$EVIDENCE" after
}

seal_evidence() {
  local requested_exit="$1"
  local status='FAIL'
  local reason="${failure_reason:-maintenance_window_or_cleanup_failed}"
  local secret_count
  if [[ "$requested_exit" -eq 0 && "$recovery_passed" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    status='PASS'
    reason='same_container_adoption_inference_idempotency_and_cleanup_passed'
  elif [[ "$window_entered" -eq 0 ]]; then
    status='STOP'
    reason="${failure_reason:-pre_window_gate_failed}"
    requested_exit=1
  else
    requested_exit=1
  fi
  {
    printf 'recorded_at=%s\n' "$(date --iso-8601=seconds)"
    printf 'status=%s\nstage=M7-E.1\nattempt=%s\n' "$status" "$FORMAL_ATTEMPT"
    printf 'reason=%s\ncompleted_phase=%s\n' "$reason" "$phase"
    printf 'baseline_ready=%s\nwindow_entered=%s\nrecovery_passed=%s\ncleanup_ok=%s\n' \
      "$baseline_ready" "$window_entered" "$recovery_passed" "$cleanup_ok"
    printf 'model_id=%s\nmodel_instance_id=%s\nmodel_file_id=%s\nassigned_port=%s\n' \
      "${model_id:-none}" "${instance_id:-none}" "${model_file_id:-none}" "${assigned_port:-none}"
    printf 'owned_container_id=%s\nowned_spec_hash=%s\n' \
      "${owned_container_id:-none}" "${owned_spec_hash:-none}"
  } >"$EVIDENCE/result.txt"
  secret_count="$(m7e1_secret_file_count "$EVIDENCE")"
  printf 'secret_file_count=%s\n' "$secret_count" >"$EVIDENCE/secret-scan.txt"
  if [[ "$secret_count" -ne 0 ]]; then
    requested_exit=1
    sed -i \
      's/^status=.*/status=FAIL/; s/^reason=.*/reason=secret_scan_detected_sensitive_content/' \
      "$EVIDENCE/result.txt"
  fi
  m7e1_seal_directory "$EVIDENCE"
  printf 'evidence_dir=%s\nmanifest_file_sha256=%s\nm7e1_worker_crash=%s\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$(sed -n 's/^status=//p' "$EVIDENCE/result.txt")" >&3
  return "$requested_exit"
}

pre_window_safety_checks() {
  [[ "$(docker inspect --format '{{.State.Running}}' "$M7E1_PYTHON_WORKER_ID" 2>/dev/null)" == 'true' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e1_managed_listener_count)" == '0' ]] || cleanup_ok=0
  m7e1_capture_gpu "$EVIDENCE" pre-window-stop
}

finalize() {
  local original_exit="$1"
  local final_exit=1
  [[ "$finalizing" -eq 0 ]] || return
  finalizing=1
  trap - EXIT INT TERM HUP
  set +o errexit
  phase='cleanup'
  event cleanup_started "original_exit=$original_exit"
  if [[ "$window_entered" -eq 0 ]]; then
    pre_window_safety_checks
  else
    if [[ "$(curl --silent --max-time 5 --output /dev/null --write-out '%{http_code}' \
      http://127.0.0.1/healthz 2>/dev/null)" == '200' ]]; then
      enable_maintenance
      discover_attempt_resources
      delete_attempt_model
    else
      cleanup_ok=0
    fi
    stop_current_rust_worker || cleanup_ok=0
    if [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] &&
      { [[ "$(m7e1_owned_container_count "$instance_id")" != '0' ]] ||
        [[ -e "$M7E1_DATA_DIR/managed/manifests/$instance_id.json" ]] ||
        [[ -e "$M7E1_DATA_DIR/managed/cache/$instance_id" ]]; }; then
      fallback_cleanup_runtime || cleanup_ok=0
    fi
    delete_attempt_model_file
    m7e1_restore_python_worker_if_safe "$EVIDENCE" || cleanup_ok=0
    [[ "$baseline_ready" -eq 1 ]] && final_checks
  fi
  event cleanup_finished "cleanup_ok=$cleanup_ok"
  phase='sealed'
  if [[ "$original_exit" -eq 0 && "$recovery_passed" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    final_exit=0
  fi
  seal_evidence "$final_exit"
  final_exit=$?
  exit "$final_exit"
}

on_signal() {
  failure_reason="signal_$1"
  event signal "$1"
  exit "$2"
}

trap 'finalize "$?"' EXIT
trap 'on_signal TERM 143' TERM
trap 'on_signal INT 130' INT
trap 'on_signal HUP 129' HUP

phase='source_build_and_drift_gate'
source /home/wujiaxing2/.cargo/env
cd "$M7E1_REPO"
require_equal "$(git branch --show-current)" 'codex/rust-worker' 'unexpected_branch'
require_equal "$(git rev-parse HEAD)" "$EXPECTED_HEAD" 'source_head_mismatch'
require_equal "$(git rev-parse origin/codex/rust-worker)" "$EXPECTED_HEAD" 'origin_head_mismatch'
require_zero "$(git status --porcelain | wc -l)" 'remote_worktree_not_clean'
watchdog_evidence="${M7E1_WATCHDOG_EVIDENCE:-}"
[[ -n "$watchdog_evidence" && -d "$watchdog_evidence" && ! -L "$watchdog_evidence" ]] ||
  fail 'watchdog_evidence_unavailable'
watchdog_identity="$watchdog_evidence/armed.txt"
[[ -f "$watchdog_identity" && ! -L "$watchdog_identity" ]] ||
  fail 'watchdog_not_armed'
watchdog_pid="$(sed -n 's/^pid=//p' "$watchdog_identity")"
watchdog_starttime="$(sed -n 's/^starttime=//p' "$watchdog_identity")"
watchdog_pgid="$(sed -n 's/^pgid=//p' "$watchdog_identity")"
watchdog_sid="$(sed -n 's/^sid=//p' "$watchdog_identity")"
watchdog_shell="$(sed -n 's/^shell_exe=//p' "$watchdog_identity")"
watchdog_shell_sha="$(sed -n 's/^shell_sha256=//p' "$watchdog_identity")"
require_equal "$(sed -n 's/^watchdog_sha256=//p' "$watchdog_identity")" \
  "$EXPECTED_WATCHDOG_SHA" 'armed_watchdog_script_identity_mismatch'
require_equal "$(sed -n 's/^executor_sha256=//p' "$watchdog_identity")" \
  "$(sha256sum "$0" | awk '{print $1}')" 'armed_executor_identity_mismatch'
[[ "$watchdog_pid" =~ ^[1-9][0-9]*$ && -r "/proc/$watchdog_pid/stat" ]] ||
  fail 'armed_watchdog_pid_not_alive'
require_equal "$(m7e1_process_starttime "$watchdog_pid")" "$watchdog_starttime" \
  'armed_watchdog_starttime_mismatch'
require_equal "$(m7e1_process_pgid "$watchdog_pid")" "$watchdog_pgid" \
  'armed_watchdog_pgid_mismatch'
require_equal "$(m7e1_process_sid "$watchdog_pid")" "$watchdog_sid" \
  'armed_watchdog_sid_mismatch'
require_equal "$watchdog_pgid" "$watchdog_pid" 'watchdog_not_process_group_leader'
require_equal "$watchdog_sid" "$watchdog_pid" 'watchdog_not_session_leader'
require_equal "$(readlink -f "/proc/$watchdog_pid/exe")" "$watchdog_shell" \
  'armed_watchdog_shell_mismatch'
require_equal "$(sha256sum "$watchdog_shell" | awk '{print $1}')" "$watchdog_shell_sha" \
  'armed_watchdog_shell_sha_mismatch'
git merge-base --is-ancestor "$PRODUCT_COMMIT" HEAD || fail 'product_commit_not_ancestor'
git diff --quiet "$PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates ||
  fail 'post_product_rust_drift'
require_equal "$(rustc --version)" 'rustc 1.99.1 (58cf8f9c3 2026-10-13)' 'rustc_version_mismatch'
require_equal "$(cargo --version)" 'cargo 1.99.1 (5f94df478 2026-08-27)' 'cargo_version_mismatch'
require_equal "$(sha256sum rust/Cargo.lock | awk '{print $1}')" \
  "$EXPECTED_LOCK_SHA" 'cargo_lock_sha_mismatch'
require_equal "$(sha256sum "$PREFLIGHT/manifest.sha256" | awk '{print $1}')" \
  "$EXPECTED_PREFLIGHT_MANIFEST_SHA" 'preflight_manifest_identity_mismatch'
(cd "$PREFLIGHT" && sha256sum --check manifest.sha256) \
  >"$EVIDENCE/preflight-manifest-check.txt" || fail 'preflight_manifest_check_failed'
require_equal "$(sed -n 's/^status=//p' "$PREFLIGHT/preflight-result.txt")" \
  'PASS' 'preflight_status_not_pass'

cargo clean --manifest-path "$M7E1_REPO/rust/Cargo.toml" -p gpustack-cli \
  >"$EVIDENCE/cargo-clean.log" 2>&1
cargo build --manifest-path "$M7E1_REPO/rust/Cargo.toml" --locked --offline \
  -p gpustack-cli >"$EVIDENCE/build.log" 2>&1
chmod 0700 "$M7E1_BINARY"
"$M7E1_BINARY" --output json version >"$EVIDENCE/binary-version.json"
require_equal "$(jq -r '.git_commit' "$EVIDENCE/binary-version.json")" \
  "${EXPECTED_HEAD:0:12}" 'binary_embedded_commit_mismatch'
rust_sha="$(sha256sum "$M7E1_BINARY" | awk '{print $1}')"
{
  printf 'source_head=%s\nproduct_commit=%s\n' "$EXPECTED_HEAD" "$PRODUCT_COMMIT"
  printf 'binary_sha256=%s\ncargo_lock_sha256=%s\n' "$rust_sha" "$EXPECTED_LOCK_SHA"
  printf 'preflight_manifest_file_sha256=%s\n' "$EXPECTED_PREFLIGHT_MANIFEST_SHA"
  printf 'executor_sha256=%s\nwatchdog_sha256=%s\nlib_sha256=%s\n' \
    "$(sha256sum "$0" | awk '{print $1}')" "$EXPECTED_WATCHDOG_SHA" "$EXPECTED_LIB_SHA"
} >"$EVIDENCE/source-identity.txt"
event gate_passed source_build_and_drift_gate

phase='final_external_drift_gate'
curl --silent --show-error --fail --max-time 10 \
  http://127.0.0.1/version >"$EVIDENCE/server-version.json"
jq -e '.version == "v2.2.3" and .git_commit == "1cfcbbf"' \
  "$EVIDENCE/server-version.json" >/dev/null || fail 'server_identity_mismatch'
"$M7E1_BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e --arg context "$M7E1_OPERATOR_CONTEXT" --arg token "$M7E1_OPERATOR_TOKEN" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
    .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts.json" >/dev/null || fail 'operator_context_mismatch'
"$M7E1_BINARY" --output json worker "${worker_args[@]}" validate-config \
  >"$EVIDENCE/validate-config.json"
jq -e '
  .valid == true and .config.worker_name == "rust-m3d-r760xa" and
  .config.worker_port == 20150 and .config.metrics_port == 20151 and
  .config.token_file == "/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token" and
  .config.data_dir == "/home/wujiaxing2/.local/state/gpustack-rust-worker/1" and
  .config.model_roots == ["/home/wujiaxing2/flowscale-system-validation/models"]
' "$EVIDENCE/validate-config.json" >/dev/null || fail 'fixed_config_mismatch'
"$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
  worker get "$M7E1_WORKER_ID" >"$EVIDENCE/before-worker.json"
jq -e '.id == 5 and .name == "rust-m3d-r760xa" and .maintenance.enabled == true and
  .port == 20150 and .metrics_port == 20151' \
  "$EVIDENCE/before-worker.json" >/dev/null || fail 'worker_identity_or_maintenance_mismatch'
require_equal "$(jq -r '.worker_uuid' "$M7E1_DATA_DIR/managed/worker-identity.json")" \
  "$M7E1_WORKER_UUID" 'persisted_worker_uuid_mismatch'
require_equal "$(stat -c '%a' "$M7E1_OPERATOR_TOKEN")" '600' 'operator_token_mode_mismatch'
require_equal "$(stat -c '%a' "$M7E1_DATA_DIR/managed/worker-token")" '600' 'worker_token_mode_mismatch'

m7e1_global_snapshot "$EVIDENCE" before || fail 'global_baseline_query_failed'
require_zero "$(jq --arg name "$M7E1_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length' "$EVIDENCE/before-models.json")" \
  'target_model_conflict'
require_zero "$(jq '[.items[] | select(.worker_id == 5)] | length' \
  "$EVIDENCE/before-model-instances.json")" 'worker_model_instance_conflict'
model_file_baseline_count="$(jq --arg path "$M7E1_MODEL_PATH" '
  [.items[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
  length
' "$EVIDENCE/before-model-files.json")"
[[ "$model_file_baseline_count" -le 1 ]] || fail 'ambiguous_baseline_model_files'
require_zero "$(docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed | wc -l)" 'rust_owned_container_conflict'

require_equal "$(readlink -f "$M7E1_MODEL_PATH")" "$M7E1_MODEL_PATH" 'model_path_mismatch'
require_equal "$(sha256sum "$M7E1_MODEL_MANIFEST" | awk '{print $1}')" \
  "$M7E1_MODEL_MANIFEST_SHA" 'model_manifest_fixture_mismatch'
(cd "$M7E1_MODEL_PATH" && sha256sum --check --strict "$M7E1_MODEL_MANIFEST") \
  >"$EVIDENCE/before-model-manifest-check.txt" || fail 'model_manifest_check_failed'
require_equal "$(find "$M7E1_MODEL_PATH" -type f -printf '%s\n' |
  awk '{sum += $1} END {print sum+0}')" "$M7E1_MODEL_BYTES" 'model_size_mismatch'
require_equal "$(sha256sum "$M7E1_MODEL_PATH/model.safetensors" | awk '{print $1}')" \
  "$M7E1_MODEL_WEIGHT_SHA" 'model_weight_mismatch'
require_equal "$(docker image inspect --format '{{.Id}}' "$M7E1_RUNNER_IMAGE")" \
  "$M7E1_RUNNER_IMAGE_ID" 'runner_image_identity_mismatch'

m7e1_capture_gpu "$EVIDENCE" before
require_equal "$(awk -F', *' '$1 == 0 {print $2}' "$EVIDENCE/before-gpus.csv")" \
  "$M7E1_GPU0_UUID" 'gpu0_uuid_mismatch'
require_equal "$(awk -F', *' '$1 == 1 {print $2}' "$EVIDENCE/before-gpus.csv")" \
  "$M7E1_GPU1_UUID" 'gpu1_uuid_mismatch'
require_zero "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" 'gpu0_compute_process_exists'
require_zero "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" 'gpu1_compute_process_exists'
require_zero "$(m7e1_gpu_memory_used 0)" 'gpu0_memory_not_zero'
require_zero "$(m7e1_gpu_memory_used 1)" 'gpu1_memory_not_zero'
require_zero "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" 'rust_worker_process_exists'
require_zero "$(m7e1_listener_count 20150)" 'rust_worker_api_port_occupied'
require_zero "$(m7e1_listener_count 20151)" 'rust_metrics_port_occupied'
require_zero "$(m7e1_managed_listener_count)" 'managed_port_pool_occupied'

mapfile -t python_workers < <(docker ps -a --no-trunc --format '{{.ID}} {{.Names}}' |
  awk '$2 == "gpustack-worker" {print $1}')
require_equal "${#python_workers[@]}" '1' 'python_worker_not_unique'
python_worker_id="${python_workers[0]}"
require_equal "$python_worker_id" "$M7E1_PYTHON_WORKER_ID" 'python_worker_id_drift'
require_equal "$(docker inspect --format '{{.State.Running}}' "$python_worker_id")" \
  'true' 'python_worker_not_running'
require_equal "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10150/healthz)" '200' 'python_worker_health_not_200'
require_equal "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10151/metrics)" '200' 'python_worker_metrics_not_200'
m7e1_capture_python_worker "$EVIDENCE/before-python-worker.json"
m7e1_capture_docker_ids >"$EVIDENCE/before-docker-ids.txt"
baseline_ready=1
event gate_passed final_external_drift_gate

phase='python_worker_stop'
window_entered=1
docker stop --time 30 "$python_worker_id" >"$EVIDENCE/python-worker-stop.txt"
require_equal "$(docker inspect --format '{{.State.Running}}' "$python_worker_id")" \
  'false' 'python_worker_failed_to_stop'
require_zero "$(m7e1_listener_count 10150)" 'python_worker_api_still_listening'
require_zero "$(m7e1_listener_count 10151)" 'python_worker_metrics_still_listening'
event maintenance_window_entered "python_worker_id=$python_worker_id"

phase='initial_rust_worker_start'
start_rust_worker before-crash || fail 'initial_rust_worker_start_failed'
wait_for_worker_ready "$EVIDENCE/rust-worker-ready-before-crash.json" ||
  fail 'initial_rust_worker_ready_timeout'
jq -e '.maintenance.enabled == true and .state == "maintenance"' \
  "$EVIDENCE/rust-worker-ready-before-crash.json" >/dev/null ||
  fail 'initial_worker_not_in_maintenance'
event gate_passed initial_rust_worker_start

phase='model_deploy_and_running_baseline'
"$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
  worker maintenance disable "$M7E1_WORKER_ID" --yes \
  >"$EVIDENCE/maintenance-disable.json"
jq -e '.worker_id == 5 and .after.enabled == false' \
  "$EVIDENCE/maintenance-disable.json" >/dev/null || fail 'maintenance_disable_not_confirmed'
ready_samples=0
for _ in $(seq 1 180); do
  if "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    worker get "$M7E1_WORKER_ID" >"$EVIDENCE/worker-ready.json" 2>/dev/null &&
    jq -e '.maintenance.enabled == false and .state == "ready" and
      .unreachable == false and .gpu_count == 2' "$EVIDENCE/worker-ready.json" >/dev/null; then
    ready_samples=$((ready_samples + 1))
    [[ "$ready_samples" -ge 2 ]] && break
  else
    ready_samples=0
  fi
  sleep 1
done
require_equal "$ready_samples" '2' 'worker_did_not_become_stably_ready'

"$M7E1_BINARY" --timeout 600 --context "$M7E1_OPERATOR_CONTEXT" --output json \
  model deploy "$M7E1_TARGET_MODEL" \
  --worker "$M7E1_WORKER_NAME" \
  --local-path "$M7E1_MODEL_PATH" \
  --image "$M7E1_RUNNER_IMAGE" \
  --gpu 0 --tensor-parallel-size 1 --max-model-len 2048 \
  --gpu-memory-utilization 0.5 --dtype auto --enforce-eager --wait \
  >"$EVIDENCE/model-deploy.json" \
  2> >(while IFS= read -r line; do
    printf '%s\t%s\n' "$(m7e1_timestamp)" "$line"
  done >>"$EVIDENCE/model-deploy-transitions.log")
model_id="$(jq -r '.model.id' "$EVIDENCE/model-deploy.json")"
instance_id="$(jq -r '.instances[0].id' "$EVIDENCE/model-deploy.json")"
assigned_port="$(jq -r '.instances[0].port' "$EVIDENCE/model-deploy.json")"
[[ "$model_id" =~ ^[1-9][0-9]*$ ]] || fail 'model_id_invalid'
[[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || fail 'instance_id_invalid'
[[ "$assigned_port" -ge 40000 && "$assigned_port" -le 40063 ]] ||
  fail 'assigned_port_outside_managed_pool'
mapfile -t observed_states < <(jq -r '.transitions[].state' "$EVIDENCE/model-deploy.json")
required_states=(pending analyzing scheduled initializing downloading starting running)
required_index=0
for observed_state in "${observed_states[@]}"; do
  if [[ "$required_index" -lt "${#required_states[@]}" ]] &&
    [[ "$observed_state" == "${required_states[$required_index]}" ]]; then
    required_index=$((required_index + 1))
  fi
done
require_equal "$required_index" "${#required_states[@]}" 'complete_state_chain_missing'
jq -e '(.instances | length) == 1 and .instances[0].state == "running" and
  .instances[0].worker_id == 5 and .instances[0].restart_count == 0 and
  .instances[0].last_restart_time == null and .model.restart_on_error == true' \
  "$EVIDENCE/model-deploy.json" >/dev/null || fail 'running_instance_contract_mismatch'

"$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
  model file list --worker "$M7E1_WORKER_ID" >"$EVIDENCE/model-files-running.json"
model_file_id="$(jq -r --arg path "$M7E1_MODEL_PATH" '
  [.[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
  if length == 1 then .[0].id else empty end
' "$EVIDENCE/model-files-running.json")"
[[ "$model_file_id" =~ ^[1-9][0-9]*$ ]] || fail 'model_file_identity_missing'
jq -e --arg path "$M7E1_MODEL_PATH" --argjson bytes "$M7E1_MODEL_BYTES" '
  [.[] | select(.worker_id == 5 and .source == "local_path" and
    .local_path == $path and .state == "ready" and .download_progress == 100 and
    .size == $bytes and .resolved_paths == [$path] and
    ((.cleanup_on_delete == null) or (.cleanup_on_delete == false)))] | length == 1
' "$EVIDENCE/model-files-running.json" >/dev/null || fail 'model_file_ready_contract_mismatch'

require_equal "$(m7e1_owned_container_count "$instance_id")" '1' \
  'owned_container_count_not_one'
owned_container_id="$(m7e1_owned_container_ids "$instance_id")"
owned_spec_hash="$(docker inspect --format \
  '{{ index .Config.Labels "io.gpustack.spec-hash" }}' "$owned_container_id")"
[[ "$owned_container_id" =~ ^[0-9a-f]{64}$ ]] || fail 'owned_container_id_invalid'
[[ "$owned_spec_hash" =~ ^[0-9a-f]{64}$ ]] || fail 'owned_spec_hash_invalid'
capture_runtime_identity pre-crash || fail 'pre_crash_runtime_identity_failed'
require_equal "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" '1' \
  'gpu0_compute_process_count_not_one'
require_zero "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" 'gpu1_was_used'
m7e1_direct_vllm_probe "$assigned_port" "$EVIDENCE/pre-crash-direct" ||
  fail 'pre_crash_direct_vllm_failed'
server_inference_suite pre-crash-server || fail 'pre_crash_server_inference_failed'
event gate_passed pre_crash_runtime_baseline

phase='intentional_worker_crash'
install -m 0600 /dev/null "$EVIDENCE/intentional-worker-crash.marker"
printf 'phase=intentional-worker-crash\narmed_at=%s\n' "$(date --iso-8601=seconds)" \
  >"$EVIDENCE/intentional-worker-crash.marker"
m7e1_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
  fail 'rust_process_identity_drift_before_sigkill'
require_equal "$(m7e1_listener_owner_pids 20150)" "$rust_pid" \
  'worker_api_listener_owner_drift_before_sigkill'
require_equal "$(m7e1_listener_owner_pids 20151)" "$rust_pid" \
  'worker_metrics_listener_owner_drift_before_sigkill'
container_pid="$(jq -r '.pid' "$EVIDENCE/pre-crash-container.json")"
[[ "$container_pid" =~ ^[1-9][0-9]*$ && "$container_pid" != "$rust_pid" ]] ||
  fail 'container_pid_not_isolated_from_worker_pid'
m7e1_capture_container "$owned_container_id" "$EVIDENCE/pre-sigkill-container.json"
cmp -s "$EVIDENCE/pre-crash-container.json" "$EVIDENCE/pre-sigkill-container.json" ||
  fail 'container_drift_before_sigkill'
m7e1_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
  fail 'rust_process_identity_drift_at_sigkill'
require_equal "$(m7e1_listener_owner_pids 20150)" "$rust_pid" \
  'worker_api_listener_owner_drift_at_sigkill'
require_equal "$(m7e1_listener_owner_pids 20151)" "$rust_pid" \
  'worker_metrics_listener_owner_drift_at_sigkill'
crash_started_epoch="$(date +%s)"
kill -KILL "$rust_pid"
set +o errexit
wait "$rust_pid"
crash_exit=$?
set -o errexit
require_equal "$crash_exit" '137' 'rust_worker_sigkill_exit_not_137'
crashed_rust_pid="$rust_pid"
rust_pid=''
[[ ! -r "/proc/$crashed_rust_pid/stat" ]] || fail 'rust_worker_pid_survived_sigkill'
require_zero "$(m7e1_listener_count 20150)" 'worker_api_listener_survived_sigkill'
require_zero "$(m7e1_listener_count 20151)" 'worker_metrics_listener_survived_sigkill'
printf 'pid=%s\nexit_code=%s\ncrash_started_epoch=%s\n' \
  "$crashed_rust_pid" "$crash_exit" "$crash_started_epoch" \
  >"$EVIDENCE/intentional-crash-result.txt"
event fault_injected "pid=$crashed_rust_pid signal=SIGKILL exit=137"

phase='worker_absence_continuity'
capture_runtime_identity worker-absent || fail 'worker_absent_runtime_identity_failed'
compare_runtime_identity pre-crash worker-absent || fail 'runtime_changed_while_worker_absent'
m7e1_direct_vllm_probe "$assigned_port" "$EVIDENCE/worker-absent-direct" ||
  fail 'worker_absent_direct_vllm_failed'
require_equal "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" '1' \
  'gpu0_process_lost_while_worker_absent'
require_zero "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" 'gpu1_used_while_worker_absent'
event gate_passed worker_absence_continuity

phase='rust_worker_restart_and_adoption'
start_rust_worker after-restart || fail 'rust_worker_restart_failed'
wait_for_worker_ready "$EVIDENCE/rust-worker-ready-after-restart.json" ||
  fail 'rust_worker_restart_ready_timeout'
new_rust_pid="$rust_pid"
[[ "$new_rust_pid" != "$crashed_rust_pid" ]] || fail 'rust_worker_pid_reused'
adopted=0
for _ in $(seq 1 120); do
  if grep -Fq "outcome=running container_id=\"$owned_container_id\"" \
    "$EVIDENCE/rust-events-after-restart.log" 2>/dev/null; then
    adopted=1
    break
  fi
  kill -0 "$rust_pid" 2>/dev/null || fail 'restarted_rust_worker_exited_before_adoption'
  sleep 1
done
require_equal "$adopted" '1' 'same_container_adoption_event_missing'
capture_runtime_identity after-adoption || fail 'after_adoption_runtime_identity_failed'
compare_runtime_identity pre-crash after-adoption || fail 'runtime_identity_changed_after_adoption'
adoption_completed_epoch="$(date +%s)"
docker events --since "$crash_started_epoch" --until "$((adoption_completed_epoch + 1))" \
  --filter type=container --filter container="$owned_container_id" --format '{{json .}}' \
  >"$EVIDENCE/docker-events-crash-to-adoption.ndjson"
if grep -Eq '"Action":"(create|start|die|stop|kill|restart|destroy)"' \
  "$EVIDENCE/docker-events-crash-to-adoption.ndjson"; then
  fail 'container_lifecycle_event_detected_during_adoption'
fi
printf 'crash_started_epoch=%s\nadoption_completed_epoch=%s\n' \
  "$crash_started_epoch" "$adoption_completed_epoch" \
  >"$EVIDENCE/adoption-window.txt"
m7e1_direct_vllm_probe "$assigned_port" "$EVIDENCE/after-adoption-direct" ||
  fail 'after_adoption_direct_vllm_failed'
server_inference_suite after-adoption-server || fail 'after_adoption_server_inference_failed'
event gate_passed rust_worker_restart_and_adoption

phase='post_adoption_idempotency'
for sample in 1 2 3; do
  sleep 2
  kill -0 "$rust_pid" 2>/dev/null || fail 'rust_worker_exited_during_idempotency'
  capture_runtime_identity "idempotency-$sample" ||
    fail "idempotency_runtime_identity_${sample}_failed"
  compare_runtime_identity pre-crash "idempotency-$sample" ||
    fail "idempotency_runtime_identity_${sample}_drift"
done
"$M7E1_BINARY" --timeout 120 --context "$M7E1_OPERATOR_CONTEXT" --output json \
  inference chat --model "$M7E1_TARGET_MODEL" --prompt 'Reply: stable' --max-tokens 8 \
  >"$EVIDENCE/chat-after-idempotency.json" || fail 'post_idempotency_chat_failed'
jq -e '.choices[0].message.content | type == "string" and length > 0' \
  "$EVIDENCE/chat-after-idempotency.json" >/dev/null || fail 'post_idempotency_chat_empty'
recovery_passed=1
failure_reason=''
event recovery_gate_passed same_container_adoption_and_idempotency
phase='recovery_passed_pending_cleanup'
exit 0
