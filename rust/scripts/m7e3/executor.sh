#!/usr/bin/env bash
# shellcheck disable=SC1090,SC1091,SC2317,SC2329
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly LIB="${M7E3_LIB:?M7E3_LIB is required}"
readonly EXPECTED_LIB_SHA="${M7E3_LIB_SHA:?M7E3_LIB_SHA is required}"
if [[ ! -f "$LIB" ]] ||
  [[ "$(sha256sum "$LIB" | awk '{print $1}')" != "$EXPECTED_LIB_SHA" ]]; then
  printf 'STOP: shared library identity mismatch\n' >&2
  exit 88
fi
source "$LIB"

readonly EVIDENCE="${M7E3_EVIDENCE:?M7E3_EVIDENCE is required}"
readonly PREFLIGHT="${M7E3_PREFLIGHT:?M7E3_PREFLIGHT is required}"
readonly WATCHDOG="${M7E3_WATCHDOG:?M7E3_WATCHDOG is required}"
readonly EXPECTED_WATCHDOG_SHA="${M7E3_WATCHDOG_SHA:?M7E3_WATCHDOG_SHA is required}"
readonly EXPECTED_HEAD="${M7E3_EXPECTED_HEAD:?M7E3_EXPECTED_HEAD is required}"
readonly PRODUCT_COMMIT="${M7E3_PRODUCT_COMMIT:?M7E3_PRODUCT_COMMIT is required}"
readonly EXPECTED_PREFLIGHT_MANIFEST_SHA="${M7E3_PREFLIGHT_MANIFEST_SHA:?M7E3_PREFLIGHT_MANIFEST_SHA is required}"
readonly EXPECTED_LOCK_SHA="${M7E3_LOCK_SHA:?M7E3_LOCK_SHA is required}"
readonly FORMAL_ATTEMPT="${M7E3_ATTEMPT:?M7E3_ATTEMPT is required}"

if [[ ! "$FORMAL_ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7E3_ATTEMPT must be a positive integer\n' >&2
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
server_outage_passed=0
docker_outage_passed=0
cleanup_ok=1
finalizing=0
python_worker_id=''
rust_pid=''
rust_heartbeat_floor_epoch=''
model_id=''
instance_id=''
model_file_id=''
assigned_port=''
old_container_id=''
owned_spec_hash=''
fault_started_epoch=''
recovery_completed_epoch=''
instance_watch_pid=''
container_wait_pid=''
server_proxy_pid=''
docker_proxy_pid=''

event() {
  jq -cn \
    --arg observed_at "$(m7e3_timestamp)" \
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
mapfile -d '' -t worker_args < <(m7e3_fixed_worker_args)

start_proxy() {
  local kind="$1"
  local label="$2"
  local identity="$EVIDENCE/${kind}-proxy-${label}.txt"
  local log="$EVIDENCE/${kind}-proxy-${label}.log"
  local pid ready=0
  local -a args
  mapfile -d '' -t args < <(m7e3_proxy_args "$kind")
  if [[ "$kind" == 'docker' ]]; then
    install -d -m 0700 "$M7E3_PROXY_STATE_DIR"
    m7e3_remove_stale_docker_proxy_socket || return 1
  fi
  setsid "$(readlink -f "$(command -v socat)")" "${args[@]}" >"$log" 2>&1 &
  pid=$!
  for _ in $(seq 1 50); do
    [[ -r "/proc/$pid/stat" ]] && break
    sleep 0.1
  done
  kill -0 "$pid" 2>/dev/null || return 1
  m7e3_record_process_identity "$pid" "${kind}_proxy" "$identity" || return 1
  m7e3_proxy_identity_matches_file "$identity" "$kind" || return 1
  install -m 0600 "$identity" "$EVIDENCE/${kind}-proxy-current.txt"
  for _ in $(seq 1 30); do
    if [[ "$kind" == 'server' ]] && m7e3_server_proxy_probe; then
      ready=1
      break
    fi
    if [[ "$kind" == 'docker' ]] && m7e3_docker_proxy_probe; then
      ready=1
      break
    fi
    sleep 1
  done
  [[ "$ready" -eq 1 ]] || return 1
  if [[ "$kind" == 'server' ]]; then
    server_proxy_pid="$pid"
  else
    docker_proxy_pid="$pid"
  fi
}

stop_proxy() {
  local kind="$1"
  local pid
  if [[ "$kind" == 'server' ]]; then
    pid="$server_proxy_pid"
  else
    pid="$docker_proxy_pid"
  fi
  m7e3_terminate_recorded_proxy \
    "$EVIDENCE/${kind}-proxy-current.txt" "$kind" "$EVIDENCE" || return 1
  if [[ -n "$pid" ]]; then
    wait "$pid" 2>/dev/null || true
  fi
  if [[ "$kind" == 'server' ]]; then
    server_proxy_pid=''
    [[ "$(m7e3_listener_count "$M7E3_SERVER_PROXY_PORT")" == '0' ]] || return 1
  else
    docker_proxy_pid=''
    m7e3_remove_stale_docker_proxy_socket || return 1
  fi
}

capture_control_metrics() {
  local prefix="$1"
  curl --silent --show-error --fail --max-time 10 \
    "http://$M7E3_WORKER_IP:20151/metrics" >"$EVIDENCE/${prefix}-metrics.txt"
}

metric_value() {
  local file="$1"
  local metric="$2"
  awk -v metric="$metric" '$1 == metric {print $2; exit}' "$file"
}

watch_metric_sum() {
  local file="$1"
  local metric="$2"
  awk -v metric="$metric" 'index($1, metric "{") == 1 {sum += $2} END {print sum+0}' "$file"
}

wait_for_control_disconnect() {
  local baseline_reconnects="$1"
  local current reconnects
  for sample in $(seq 1 100); do
    current="server-outage-control-$sample"
    capture_control_metrics "$current" || {
      sleep 1
      continue
    }
    reconnects="$(watch_metric_sum \
      "$EVIDENCE/${current}-metrics.txt" 'gpustack:rust_worker_watch_reconnects_total')"
    if [[ "$(metric_value "$EVIDENCE/${current}-metrics.txt" \
      'gpustack:rust_worker_heartbeat_success')" == '0' ]] &&
      [[ "$(metric_value "$EVIDENCE/${current}-metrics.txt" \
        'gpustack:rust_worker_status_success')" == '0' ]] &&
      [[ "$(watch_metric_sum "$EVIDENCE/${current}-metrics.txt" \
        'gpustack:rust_worker_watch_connected')" == '0' ]] &&
      [[ "$reconnects" -gt "$baseline_reconnects" ]]; then
      printf 'sample=%s\nreconnect_sum=%s\n' "$sample" "$reconnects" \
        >"$EVIDENCE/server-outage-control-observed.txt"
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_control_recovery() {
  local prefix="$1"
  local current
  for sample in $(seq 1 120); do
    current="${prefix}-$sample"
    if capture_control_metrics "$current" &&
      [[ "$(metric_value "$EVIDENCE/${current}-metrics.txt" \
        'gpustack:rust_worker_heartbeat_success')" == '1' ]] &&
      [[ "$(metric_value "$EVIDENCE/${current}-metrics.txt" \
        'gpustack:rust_worker_status_success')" == '1' ]] &&
      [[ "$(watch_metric_sum "$EVIDENCE/${current}-metrics.txt" \
        'gpustack:rust_worker_watch_connected')" == '3' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        "http://$M7E3_WORKER_IP:20150/readyz" 2>/dev/null)" == '200' ]]; then
      printf 'sample=%s\n' "$sample" >"$EVIDENCE/${prefix}-observed.txt"
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_docker_disconnect() {
  local first_event_line="$1"
  local current
  for sample in $(seq 1 30); do
    current="docker-outage-$sample"
    if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      "http://$M7E3_WORKER_IP:20150/readyz" 2>/dev/null)" == '503' ]] &&
      tail -n "+$first_event_line" "$EVIDENCE/rust-events-before-outage.log" |
        grep -F 'outcome=failed message=' >/dev/null; then
      tail -n "+$first_event_line" "$EVIDENCE/rust-events-before-outage.log" \
        >"$EVIDENCE/${current}-runtime-events.log"
      printf 'sample=%s\n' "$sample" >"$EVIDENCE/docker-outage-observed.txt"
      return 0
    fi
    sleep 1
  done
  return 1
}

wait_for_docker_recovery() {
  local first_event_line="$1"
  for sample in $(seq 1 60); do
    if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      "http://$M7E3_WORKER_IP:20150/readyz" 2>/dev/null)" == '200' ]] &&
      tail -n "+$first_event_line" "$EVIDENCE/rust-events-before-outage.log" |
        grep -F "outcome=running container_id=\"$old_container_id\"" >/dev/null; then
      printf 'sample=%s\ncontainer_id=%s\n' "$sample" "$old_container_id" \
        >"$EVIDENCE/docker-recovery-observed.txt"
      return 0
    fi
    sleep 1
  done
  return 1
}

write_process_identity() {
  local pid="$1"
  local output="$2"
  local exe sha cmdline_sha starttime pgid sid
  [[ -r "/proc/$pid/stat" ]] || return 1
  exe="$(readlink -f "/proc/$pid/exe")"
  sha="$(sha256sum "$exe" | awk '{print $1}')"
  cmdline_sha="$(m7e3_process_cmdline_sha "$pid")"
  starttime="$(m7e3_process_starttime "$pid")"
  pgid="$(m7e3_process_pgid "$pid")"
  sid="$(m7e3_process_sid "$pid")"
  [[ "$exe" == "$(readlink -f "$M7E3_BINARY")" ]] || return 1
  [[ "$cmdline_sha" == "$(m7e3_expected_worker_cmdline_sha)" ]] || return 1
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
  local baseline="$EVIDENCE/rust-worker-${label}-baseline.json"
  local summary="$EVIDENCE/rust-summary-${label}.json"
  local events="$EVIDENCE/rust-events-${label}.log"
  "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    worker get "$M7E3_WORKER_ID" >"$baseline" 2>"$baseline.stderr" || return 1
  rust_heartbeat_floor_epoch="$(m7e3_server_timestamp_epoch \
    "$(jq -r '.heartbeat_time // empty' "$baseline")")" || return 1
  setsid "$M7E3_BINARY" --output json worker "${worker_args[@]}" run --managed-runtime \
    >"$summary" \
    2> >(while IFS= read -r line; do
      printf '%s\t%s\n' "$(m7e3_timestamp)" "$line"
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
      "http://$M7E3_WORKER_IP:20150/healthz" 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        "http://$M7E3_WORKER_IP:20150/readyz" 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        "http://$M7E3_WORKER_IP:20151/metrics" 2>/dev/null)" == '200' ]] &&
      "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
        worker get "$M7E3_WORKER_ID" >"$output" 2>"$output.stderr" &&
      m7e3_maintenance_worker_has_new_heartbeat \
        "$output" "$rust_heartbeat_floor_epoch"; then
      ready=1
      break
    fi
    sleep 1
  done
  [[ "$ready" -eq 1 ]] || return 1
  require_equal "$(m7e3_listener_owner_pids 20150)" "$rust_pid" \
    'worker_api_listener_owner_mismatch'
  require_equal "$(m7e3_listener_owner_pids 20151)" "$rust_pid" \
    'worker_metrics_listener_owner_mismatch'
  m7e3_process_identity_matches_file "$EVIDENCE/rust-process-current.txt"
}

start_instance_watch() {
  local watch_fd
  local token
  token="$(tr -d '\r\n' <"$M7E3_OPERATOR_TOKEN")"
  exec {watch_fd}<<<"header = \"Authorization: Bearer $token\""
  curl --config "/dev/fd/$watch_fd" --no-buffer --silent --show-error \
    --url 'http://127.0.0.1/v2/model-instances?watch=true' \
    >"$EVIDENCE/model-instance-watch.ndjson" \
    2>"$EVIDENCE/model-instance-watch.stderr" &
  instance_watch_pid=$!
  exec {watch_fd}<&-
  unset token
  for _ in $(seq 1 30); do
    kill -0 "$instance_watch_pid" 2>/dev/null || return 1
    if jq -s -e --argjson instance "$instance_id" '
      any(.[]; .data.id == $instance and .data.state == "running")
    ' "$EVIDENCE/model-instance-watch.ndjson" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  return 1
}

stop_instance_watch() {
  [[ -n "$instance_watch_pid" ]] || return 0
  if kill -0 "$instance_watch_pid" 2>/dev/null; then
    kill -TERM "$instance_watch_pid" 2>/dev/null || return 1
  fi
  wait "$instance_watch_pid" 2>/dev/null || true
  instance_watch_pid=''
}

capture_runtime_identity() {
  local prefix="$1"
  local status_file="$EVIDENCE/${prefix}-model-status.json"
  local container_file="$EVIDENCE/${prefix}-container.json"
  local manifest_file="$M7E3_DATA_DIR/managed/manifests/$instance_id.json"
  "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    model status "$model_id" >"$status_file"
  jq -S '{
    model:{id:.model.id,name:.model.name,restart_on_error:.model.restart_on_error},
    instances:[.instances[] | {
      id,state,worker_id,port,restart_count,last_restart_time
    }]
  }' "$status_file" >"$EVIDENCE/${prefix}-runtime-contract.json"
  jq -e \
    --argjson model "$model_id" \
    --arg name "$M7E3_TARGET_MODEL" \
    --argjson instance "$instance_id" \
    --argjson port "$assigned_port" '
      .model.id == $model and .model.name == $name and .model.restart_on_error == true and
      (.instances | length) == 1 and .instances[0].id == $instance and
      .instances[0].state == "running" and .instances[0].worker_id == 5 and
      .instances[0].port == $port and .instances[0].restart_count == 0 and
      .instances[0].last_restart_time == null
    ' "$status_file" >/dev/null || return 1
  require_equal "$(m7e3_owned_container_count "$instance_id")" '1' \
    "${prefix}_owned_container_count_not_one"
  require_equal "$(m7e3_owned_container_ids "$instance_id")" "$old_container_id" \
    "${prefix}_owned_container_identity_drift"
  m7e3_capture_container "$old_container_id" "$container_file"
  m7e3_validate_owned_container "$container_file" "$instance_id" "$owned_spec_hash"
  [[ -f "$manifest_file" ]] || return 1
  jq -S . "$manifest_file" >"$EVIDENCE/${prefix}-manifest.json"
  jq -e \
    --argjson instance "$instance_id" \
    --arg uuid "$M7E3_WORKER_UUID" \
    --arg container "$old_container_id" '
      .schema_version == 1 and .instance_id == $instance and .worker_uuid == $uuid and
      .container_id == $container and .restart_count == 0 and
      .spec.mode == "managed" and .spec.deployment_id == ($instance|tostring) and
      .last_observed_state == "ready"
    ' "$EVIDENCE/${prefix}-manifest.json" >/dev/null || return 1
  find "$M7E3_LOG_DIR/serve" -maxdepth 1 -type f \
    -name "${instance_id}.*.log" -printf '%f\n' | sort \
    >"$EVIDENCE/${prefix}-log-files.txt"
  require_equal "$(wc -l <"$EVIDENCE/${prefix}-log-files.txt")" '1' \
    "${prefix}_log_generation_count"
  grep -qx "${instance_id}.0.log" "$EVIDENCE/${prefix}-log-files.txt" || return 1
  stat -c 'device=%d\ninode=%i\nmode=%a\nowner=%U:%G\n' \
    "$M7E3_LOG_DIR/serve/${instance_id}.0.log" \
    >"$EVIDENCE/${prefix}-log-identity.txt"
  require_equal "$(m7e3_listener_count "$assigned_port")" '1' \
    "${prefix}_managed_listener_missing"
}

compare_runtime_contract() {
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
  local include_logs="${2:-yes}"
  "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json inference models \
    >"$EVIDENCE/${prefix}-inference-models.json" || return 1
  jq -e --arg name "$M7E3_TARGET_MODEL" \
    '[.data[] | select(.id == $name)] | length == 1' \
    "$EVIDENCE/${prefix}-inference-models.json" >/dev/null || return 1
  "$M7E3_BINARY" --timeout 120 --context "$M7E3_OPERATOR_CONTEXT" --output json \
    inference chat --model "$M7E3_TARGET_MODEL" \
    --prompt 'Reply with exactly: rust worker ready' --max-tokens 16 \
    >"$EVIDENCE/${prefix}-chat.json" || return 1
  jq -e '.choices[0].message.content | type == "string" and length > 0' \
    "$EVIDENCE/${prefix}-chat.json" >/dev/null || return 1
  "$M7E3_BINARY" --timeout 120 --context "$M7E3_OPERATOR_CONTEXT" --output json \
    inference completion --model "$M7E3_TARGET_MODEL" --prompt 'Rust is' --max-tokens 16 \
    >"$EVIDENCE/${prefix}-completion.json" || return 1
  jq -e '.choices[0].text | type == "string" and length > 0' \
    "$EVIDENCE/${prefix}-completion.json" >/dev/null || return 1
  if [[ "$include_logs" == 'yes' ]]; then
    "$M7E3_BINARY" --timeout 60 --context "$M7E3_OPERATOR_CONTEXT" \
      model logs "$model_id" --tail 40 >"$EVIDENCE/${prefix}-logs.txt" || return 1
    [[ -s "$EVIDENCE/${prefix}-logs.txt" ]] || return 1
  fi
}

enable_maintenance() {
  local current="$EVIDENCE/cleanup-worker.json"
  if timeout 30 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    worker get "$M7E3_WORKER_ID" >"$current" 2>"$current.stderr" &&
    [[ "$(jq -r '.maintenance.enabled' "$current")" != 'true' ]]; then
    timeout 40 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
      worker maintenance enable "$M7E3_WORKER_ID" \
      --message "M7-E.3 attempt $FORMAL_ATTEMPT cleanup" \
      >"$EVIDENCE/cleanup-maintenance-enable.json" \
      2>"$EVIDENCE/cleanup-maintenance-enable.stderr" || cleanup_ok=0
  fi
}

discover_attempt_resources() {
  local candidate
  if timeout 30 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    model list >"$EVIDENCE/cleanup-model-discovery.json" \
    2>"$EVIDENCE/cleanup-model-discovery.stderr"; then
    candidate="$(jq -r \
      --arg name "$M7E3_TARGET_MODEL" \
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
    [[ "$(m7e3_model_instance_get "http://127.0.0.1/v2/model-instances?page=-1&model_id=$model_id" \
      "$EVIDENCE/cleanup-instance-discovery.json" || true)" == '200' ]]; then
    instance_id="$(jq -r \
      'if (.items | length) == 1 then .items[0].id else empty end' \
      "$EVIDENCE/cleanup-instance-discovery.json")"
  fi
}

delete_attempt_model() {
  local deadline status
  [[ -n "$model_id" ]] || return 0
  timeout 40 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    model delete "$model_id" --yes >"$EVIDENCE/cleanup-model-delete.json" \
    2>"$EVIDENCE/cleanup-model-delete.stderr" || cleanup_ok=0
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  deadline=$((SECONDS + 180))
  while ((SECONDS < deadline)); do
    status="$(m7e3_model_instance_get "http://127.0.0.1/v2/model-instances/$instance_id" \
      "$EVIDENCE/cleanup-instance-latest.json" || true)"
    if [[ "$status" == '404' ]] &&
      [[ "$(m7e3_owned_container_count "$instance_id")" == '0' ]] &&
      [[ ! -e "$M7E3_DATA_DIR/managed/manifests/$instance_id.json" ]] &&
      [[ ! -e "$M7E3_DATA_DIR/managed/cache/$instance_id" ]] &&
      [[ "$(find "$M7E3_LOG_DIR/serve" -maxdepth 1 -type f \
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
  if ! timeout 30 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    model file list --worker "$M7E3_WORKER_ID" >"$list" 2>"$list.stderr"; then
    cleanup_ok=0
    return
  fi
  mapfile -t candidates < <(jq -r \
    --arg path "$M7E3_MODEL_PATH" \
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
    if timeout 30 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
      model file get "$candidate" >"$fetched" 2>"$fetched.stderr" &&
      jq -e --arg path "$M7E3_MODEL_PATH" '
        .id > 0 and .worker_id == 5 and .source == "local_path" and
        .local_path == $path and ((.cleanup_on_delete == null) or
        (.cleanup_on_delete == false))
      ' "$fetched" >/dev/null; then
      timeout 40 "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
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
  m7e3_terminate_recorded_process "$EVIDENCE/rust-process-current.txt" "$EVIDENCE" || return 1
  wait "$rust_pid" 2>/dev/null || true
  rust_pid=''
}

ensure_cleanup_proxies() {
  if ! m7e3_server_proxy_probe; then
    m7e3_terminate_recorded_proxy \
      "$EVIDENCE/server-proxy-current.txt" server "$EVIDENCE" || cleanup_ok=0
    start_proxy server cleanup-recovery || cleanup_ok=0
  fi
  if ! m7e3_docker_proxy_probe; then
    m7e3_terminate_recorded_proxy \
      "$EVIDENCE/docker-proxy-current.txt" docker "$EVIDENCE" || cleanup_ok=0
    m7e3_remove_stale_docker_proxy_socket || cleanup_ok=0
    start_proxy docker cleanup-recovery || cleanup_ok=0
  fi
}

stop_connection_proxies() {
  if [[ -f "$EVIDENCE/server-proxy-current.txt" ]]; then
    stop_proxy server || cleanup_ok=0
  fi
  if [[ -f "$EVIDENCE/docker-proxy-current.txt" ]]; then
    stop_proxy docker || cleanup_ok=0
  fi
  m7e3_remove_stale_docker_proxy_socket || cleanup_ok=0
  if [[ -d "$M7E3_PROXY_STATE_DIR" ]]; then
    rmdir "$M7E3_PROXY_STATE_DIR" 2>/dev/null || cleanup_ok=0
  fi
}

fallback_cleanup_runtime() {
  local status
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  status="$(m7e3_model_instance_get "http://127.0.0.1/v2/model-instances/$instance_id" \
    "$EVIDENCE/cleanup-instance-after-stop.json" || true)"
  [[ "$status" == '404' ]] || return 1
  if [[ "$(m7e3_owned_container_count "$instance_id")" != '0' ]]; then
    m7e3_remove_all_exact_owned_containers \
      "$instance_id" "$owned_spec_hash" "$EVIDENCE" || return 1
  fi
  m7e3_remove_exact_local_state "$instance_id"
}

final_checks() {
  (cd "$M7E3_MODEL_PATH" && sha256sum --check --strict "$M7E3_MODEL_MANIFEST") \
    >"$EVIDENCE/after-model-manifest-check.txt" 2>&1 || cleanup_ok=0
  "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    worker get "$M7E3_WORKER_ID" >"$EVIDENCE/after-worker.json" \
    2>"$EVIDENCE/after-worker.stderr" || cleanup_ok=0
  jq -e '.id == 5 and .name == "rust-m3d-r760xa" and .maintenance.enabled == true' \
    "$EVIDENCE/after-worker.json" >/dev/null 2>&1 || cleanup_ok=0
  m7e3_global_snapshot "$EVIDENCE" after || cleanup_ok=0
  cmp -s "$EVIDENCE/before-global-ids.json" "$EVIDENCE/after-global-ids.json" || cleanup_ok=0
  m7e3_capture_docker_ids >"$EVIDENCE/after-docker-ids.txt" || cleanup_ok=0
  cmp -s "$EVIDENCE/before-docker-ids.txt" "$EVIDENCE/after-docker-ids.txt" || cleanup_ok=0
  m7e3_capture_python_worker "$EVIDENCE/after-python-worker.json" || cleanup_ok=0
  cmp -s "$EVIDENCE/before-python-worker.json" "$EVIDENCE/after-python-worker.json" || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e3_listener_count "$M7E3_SERVER_PROXY_PORT")" == '0' ]] || cleanup_ok=0
  [[ ! -e "$M7E3_DOCKER_PROXY_SOCKET" && ! -L "$M7E3_DOCKER_PROXY_SOCKET" ]] || cleanup_ok=0
  [[ "$(m7e3_managed_listener_count)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e3_gpu_compute_count "$M7E3_GPU0_UUID")" == '0' ]] || cleanup_ok=0
  [[ "$(m7e3_gpu_compute_count "$M7E3_GPU1_UUID")" == '0' ]] || cleanup_ok=0
  [[ "$(m7e3_gpu_memory_used 0)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e3_gpu_memory_used 1)" == '0' ]] || cleanup_ok=0
  m7e3_capture_gpu "$EVIDENCE" after
}

seal_evidence() {
  local requested_exit="$1"
  local status='FAIL'
  local reason="${failure_reason:-maintenance_window_or_cleanup_failed}"
  local generic_secret_count known_secret_count secret_count
  if [[ "$requested_exit" -eq 0 && "$recovery_passed" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    status='PASS'
    reason='server_and_docker_connection_outages_preserved_runtime_and_reconverged'
  elif [[ "$window_entered" -eq 0 ]]; then
    status='STOP'
    reason="${failure_reason:-pre_window_gate_failed}"
    requested_exit=1
  else
    requested_exit=1
  fi
  {
    printf 'recorded_at=%s\n' "$(date --iso-8601=seconds)"
    printf 'status=%s\nstage=M7-E.3\nattempt=%s\n' "$status" "$FORMAL_ATTEMPT"
    printf 'reason=%s\ncompleted_phase=%s\n' "$reason" "$phase"
    printf 'baseline_ready=%s\nwindow_entered=%s\nserver_outage_passed=%s\n' \
      "$baseline_ready" "$window_entered" "$server_outage_passed"
    printf 'docker_outage_passed=%s\nrecovery_passed=%s\ncleanup_ok=%s\n' \
      "$docker_outage_passed" "$recovery_passed" "$cleanup_ok"
    printf 'model_id=%s\nmodel_instance_id=%s\nmodel_file_id=%s\nassigned_port=%s\n' \
      "${model_id:-none}" "${instance_id:-none}" "${model_file_id:-none}" "${assigned_port:-none}"
    printf 'container_id=%s\nowned_spec_hash=%s\n' \
      "${old_container_id:-none}" "${owned_spec_hash:-none}"
  } >"$EVIDENCE/result.txt"
  generic_secret_count="$(m7e3_secret_file_count "$EVIDENCE")"
  known_secret_count="$(m7e3_known_secret_file_count "$EVIDENCE")"
  secret_count=$((generic_secret_count + known_secret_count))
  printf 'secret_file_count=%s\n' "$secret_count" >"$EVIDENCE/secret-scan.txt"
  if [[ "$secret_count" -ne 0 ]]; then
    requested_exit=1
    sed -i \
      's/^status=.*/status=FAIL/; s/^reason=.*/reason=secret_scan_detected_sensitive_content/' \
      "$EVIDENCE/result.txt"
  fi
  m7e3_seal_directory "$EVIDENCE"
  printf 'evidence_dir=%s\nmanifest_file_sha256=%s\nm7e3_connection_recovery=%s\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$(sed -n 's/^status=//p' "$EVIDENCE/result.txt")" >&3
  return "$requested_exit"
}

pre_window_safety_checks() {
  [[ "$(docker inspect --format '{{.State.Running}}' "$M7E3_PYTHON_WORKER_ID" 2>/dev/null)" == 'true' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]] || cleanup_ok=0
  [[ "$(m7e3_listener_count "$M7E3_SERVER_PROXY_PORT")" == '0' ]] || cleanup_ok=0
  [[ ! -e "$M7E3_DOCKER_PROXY_SOCKET" && ! -L "$M7E3_DOCKER_PROXY_SOCKET" ]] || cleanup_ok=0
  [[ "$(m7e3_managed_listener_count)" == '0' ]] || cleanup_ok=0
  m7e3_capture_gpu "$EVIDENCE" pre-window-stop
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
  stop_instance_watch || cleanup_ok=0
  if [[ -n "$container_wait_pid" ]]; then
    if kill -0 "$container_wait_pid" 2>/dev/null; then
      kill -TERM "$container_wait_pid" 2>/dev/null || cleanup_ok=0
    fi
    wait "$container_wait_pid" 2>/dev/null || true
    container_wait_pid=''
  fi
  if [[ "$window_entered" -eq 0 ]]; then
    pre_window_safety_checks
  else
    ensure_cleanup_proxies
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
      { [[ "$(m7e3_owned_container_count "$instance_id")" != '0' ]] ||
        [[ -e "$M7E3_DATA_DIR/managed/manifests/$instance_id.json" ]] ||
        [[ -e "$M7E3_DATA_DIR/managed/cache/$instance_id" ]]; }; then
      fallback_cleanup_runtime || cleanup_ok=0
    fi
    stop_connection_proxies
    delete_attempt_model_file
    m7e3_restore_python_worker_if_safe "$EVIDENCE" || cleanup_ok=0
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
cd "$M7E3_REPO"
require_equal "$(git branch --show-current)" 'codex/rust-worker' 'unexpected_branch'
require_equal "$(git rev-parse HEAD)" "$EXPECTED_HEAD" 'source_head_mismatch'
require_equal "$(git rev-parse origin/codex/rust-worker)" "$EXPECTED_HEAD" 'origin_head_mismatch'
require_zero "$(git status --porcelain | wc -l)" 'remote_worktree_not_clean'
watchdog_evidence="${M7E3_WATCHDOG_EVIDENCE:-}"
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
require_equal "$(m7e3_process_starttime "$watchdog_pid")" "$watchdog_starttime" \
  'armed_watchdog_starttime_mismatch'
require_equal "$(m7e3_process_pgid "$watchdog_pid")" "$watchdog_pgid" \
  'armed_watchdog_pgid_mismatch'
require_equal "$(m7e3_process_sid "$watchdog_pid")" "$watchdog_sid" \
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
require_equal "$(sed -n 's/^governance_head=//p' "$PREFLIGHT/source-identity.txt")" \
  "$EXPECTED_HEAD" 'preflight_source_head_mismatch'
require_equal "$(sed -n 's/^product_commit=//p' "$PREFLIGHT/source-identity.txt")" \
  "$PRODUCT_COMMIT" 'preflight_product_commit_mismatch'
require_equal "$(sed -n 's/^cargo_lock_sha256=//p' "$PREFLIGHT/source-identity.txt")" \
  "$EXPECTED_LOCK_SHA" 'preflight_cargo_lock_mismatch'
require_equal "$(sed -n 's/^lib_sha256=//p' "$PREFLIGHT/source-identity.txt")" \
  "$EXPECTED_LIB_SHA" 'preflight_lib_identity_mismatch'
require_equal "$(sed -n 's/^executor_sha256=//p' "$PREFLIGHT/source-identity.txt")" \
  "$(sha256sum "$0" | awk '{print $1}')" 'preflight_executor_identity_mismatch'
require_equal "$(sed -n 's/^watchdog_sha256=//p' "$PREFLIGHT/source-identity.txt")" \
  "$EXPECTED_WATCHDOG_SHA" 'preflight_watchdog_identity_mismatch'

cargo clean --manifest-path "$M7E3_REPO/rust/Cargo.toml" -p gpustack-cli \
  >"$EVIDENCE/cargo-clean.log" 2>&1
cargo build --manifest-path "$M7E3_REPO/rust/Cargo.toml" --locked --offline \
  -p gpustack-cli >"$EVIDENCE/build.log" 2>&1
chmod 0700 "$M7E3_BINARY"
"$M7E3_BINARY" --output json version >"$EVIDENCE/binary-version.json"
require_equal "$(jq -r '.git_commit' "$EVIDENCE/binary-version.json")" \
  "${EXPECTED_HEAD:0:12}" 'binary_embedded_commit_mismatch'
rust_sha="$(sha256sum "$M7E3_BINARY" | awk '{print $1}')"
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
"$M7E3_BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e --arg context "$M7E3_OPERATOR_CONTEXT" --arg token "$M7E3_OPERATOR_TOKEN" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
    .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts.json" >/dev/null || fail 'operator_context_mismatch'
"$M7E3_BINARY" --output json worker "${worker_args[@]}" validate-config \
  >"$EVIDENCE/validate-config.json"
jq -e '
  .valid == true and .config.worker_name == "rust-m3d-r760xa" and
  .config.server_url == "http://127.0.0.1:18080/" and
  .config.worker_port == 20150 and .config.metrics_port == 20151 and
  .config.token_file == "/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token" and
  .config.data_dir == "/home/wujiaxing2/.local/state/gpustack-rust-worker/1" and
  .config.model_roots == ["/home/wujiaxing2/flowscale-system-validation/models"] and
  .config.docker_socket == "/home/wujiaxing2/.local/state/gpustack-rust-worker/m7/docker-proxy.sock"
' "$EVIDENCE/validate-config.json" >/dev/null || fail 'fixed_config_mismatch'
"$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
  worker get "$M7E3_WORKER_ID" >"$EVIDENCE/before-worker.json"
jq -e '.id == 5 and .name == "rust-m3d-r760xa" and .maintenance.enabled == true and
  .port == 20150 and .metrics_port == 20151' \
  "$EVIDENCE/before-worker.json" >/dev/null || fail 'worker_identity_or_maintenance_mismatch'
require_equal "$(jq -r '.worker_uuid' "$M7E3_DATA_DIR/managed/worker-identity.json")" \
  "$M7E3_WORKER_UUID" 'persisted_worker_uuid_mismatch'
require_equal "$(stat -c '%a' "$M7E3_OPERATOR_TOKEN")" '600' 'operator_token_mode_mismatch'
require_equal "$(stat -c '%a' "$M7E3_DATA_DIR/managed/worker-token")" '600' 'worker_token_mode_mismatch'

m7e3_global_snapshot "$EVIDENCE" before || fail 'global_baseline_query_failed'
require_zero "$(jq --arg name "$M7E3_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length' "$EVIDENCE/before-models.json")" \
  'target_model_conflict'
require_zero "$(jq '[.items[] | select(.worker_id == 5)] | length' \
  "$EVIDENCE/before-model-instances.json")" 'worker_model_instance_conflict'
model_file_baseline_count="$(jq --arg path "$M7E3_MODEL_PATH" '
  [.items[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
  length
' "$EVIDENCE/before-model-files.json")"
[[ "$model_file_baseline_count" -le 1 ]] || fail 'ambiguous_baseline_model_files'
require_zero "$(docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed | wc -l)" 'rust_owned_container_conflict'

require_equal "$(readlink -f "$M7E3_MODEL_PATH")" "$M7E3_MODEL_PATH" 'model_path_mismatch'
require_equal "$(sha256sum "$M7E3_MODEL_MANIFEST" | awk '{print $1}')" \
  "$M7E3_MODEL_MANIFEST_SHA" 'model_manifest_fixture_mismatch'
(cd "$M7E3_MODEL_PATH" && sha256sum --check --strict "$M7E3_MODEL_MANIFEST") \
  >"$EVIDENCE/before-model-manifest-check.txt" || fail 'model_manifest_check_failed'
require_equal "$(find "$M7E3_MODEL_PATH" -type f -printf '%s\n' |
  awk '{sum += $1} END {print sum+0}')" "$M7E3_MODEL_BYTES" 'model_size_mismatch'
require_equal "$(sha256sum "$M7E3_MODEL_PATH/model.safetensors" | awk '{print $1}')" \
  "$M7E3_MODEL_WEIGHT_SHA" 'model_weight_mismatch'
require_equal "$(docker image inspect --format '{{.Id}}' "$M7E3_RUNNER_IMAGE")" \
  "$M7E3_RUNNER_IMAGE_ID" 'runner_image_identity_mismatch'

m7e3_capture_gpu "$EVIDENCE" before
require_equal "$(awk -F', *' '$1 == 0 {print $2}' "$EVIDENCE/before-gpus.csv")" \
  "$M7E3_GPU0_UUID" 'gpu0_uuid_mismatch'
require_equal "$(awk -F', *' '$1 == 1 {print $2}' "$EVIDENCE/before-gpus.csv")" \
  "$M7E3_GPU1_UUID" 'gpu1_uuid_mismatch'
require_zero "$(m7e3_gpu_compute_count "$M7E3_GPU0_UUID")" 'gpu0_compute_process_exists'
require_zero "$(m7e3_gpu_compute_count "$M7E3_GPU1_UUID")" 'gpu1_compute_process_exists'
require_zero "$(m7e3_gpu_memory_used 0)" 'gpu0_memory_not_zero'
require_zero "$(m7e3_gpu_memory_used 1)" 'gpu1_memory_not_zero'
require_zero "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" 'rust_worker_process_exists'
require_zero "$(m7e3_listener_count 20150)" 'rust_worker_api_port_occupied'
require_zero "$(m7e3_listener_count 20151)" 'rust_metrics_port_occupied'
require_zero "$(m7e3_managed_listener_count)" 'managed_port_pool_occupied'
require_zero "$(m7e3_listener_count "$M7E3_SERVER_PROXY_PORT")" \
  'server_proxy_port_occupied'
[[ ! -e "$M7E3_DOCKER_PROXY_SOCKET" && ! -L "$M7E3_DOCKER_PROXY_SOCKET" ]] ||
  fail 'docker_proxy_socket_already_exists'
command -v socat >"$EVIDENCE/socat-path.txt" || fail 'socat_missing'
command -v setsid >"$EVIDENCE/setsid-path.txt" || fail 'setsid_missing'
[[ -S /var/run/docker.sock && ! -L /var/run/docker.sock ]] ||
  fail 'real_docker_socket_identity_invalid'

mapfile -t python_workers < <(docker ps -a --no-trunc --format '{{.ID}} {{.Names}}' |
  awk '$2 == "gpustack-worker" {print $1}')
require_equal "${#python_workers[@]}" '1' 'python_worker_not_unique'
python_worker_id="${python_workers[0]}"
require_equal "$python_worker_id" "$M7E3_PYTHON_WORKER_ID" 'python_worker_id_drift'
require_equal "$(docker inspect --format '{{.State.Running}}' "$python_worker_id")" \
  'true' 'python_worker_not_running'
require_equal "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10150/healthz)" '200' 'python_worker_health_not_200'
require_equal "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10151/metrics)" '200' 'python_worker_metrics_not_200'
m7e3_capture_python_worker "$EVIDENCE/before-python-worker.json"
m7e3_capture_docker_ids >"$EVIDENCE/before-docker-ids.txt"
baseline_ready=1
event gate_passed final_external_drift_gate

phase='python_worker_stop'
window_entered=1
docker stop --time 30 "$python_worker_id" >"$EVIDENCE/python-worker-stop.txt"
require_equal "$(docker inspect --format '{{.State.Running}}' "$python_worker_id")" \
  'false' 'python_worker_failed_to_stop'
require_zero "$(m7e3_listener_count 10150)" 'python_worker_api_still_listening'
require_zero "$(m7e3_listener_count 10151)" 'python_worker_metrics_still_listening'
event maintenance_window_entered "python_worker_id=$python_worker_id"

phase='connection_proxy_start'
start_proxy server initial || fail 'server_proxy_start_failed'
start_proxy docker initial || fail 'docker_proxy_start_failed'
m7e3_server_proxy_probe || fail 'server_proxy_initial_probe_failed'
m7e3_docker_proxy_probe || fail 'docker_proxy_initial_probe_failed'
event gate_passed connection_proxies_ready

phase='initial_rust_worker_start'
start_rust_worker before-outage || fail 'initial_rust_worker_start_failed'
wait_for_worker_ready "$EVIDENCE/rust-worker-ready-before-outage.json" ||
  fail 'initial_rust_worker_ready_timeout'
jq -e '.maintenance.enabled == true and .state == "maintenance"' \
  "$EVIDENCE/rust-worker-ready-before-outage.json" >/dev/null ||
  fail 'initial_worker_not_in_maintenance'
event gate_passed initial_rust_worker_start

phase='model_deploy_and_running_baseline'
"$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
  worker maintenance disable "$M7E3_WORKER_ID" --yes \
  >"$EVIDENCE/maintenance-disable.json"
jq -e '.worker_id == 5 and .after.enabled == false' \
  "$EVIDENCE/maintenance-disable.json" >/dev/null || fail 'maintenance_disable_not_confirmed'
ready_samples=0
for _ in $(seq 1 180); do
  if "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    worker get "$M7E3_WORKER_ID" >"$EVIDENCE/worker-ready.json" 2>/dev/null &&
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

"$M7E3_BINARY" --timeout 600 --context "$M7E3_OPERATOR_CONTEXT" --output json \
  model deploy "$M7E3_TARGET_MODEL" \
  --worker "$M7E3_WORKER_NAME" \
  --local-path "$M7E3_MODEL_PATH" \
  --image "$M7E3_RUNNER_IMAGE" \
  --gpu 0 --tensor-parallel-size 1 --max-model-len 2048 \
  --gpu-memory-utilization 0.5 --dtype auto --enforce-eager --wait \
  >"$EVIDENCE/model-deploy.json" \
  2> >(while IFS= read -r line; do
    printf '%s\t%s\n' "$(m7e3_timestamp)" "$line"
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

"$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
  model file list --worker "$M7E3_WORKER_ID" >"$EVIDENCE/model-files-running.json"
model_file_id="$(jq -r --arg path "$M7E3_MODEL_PATH" '
  [.[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
  if length == 1 then .[0].id else empty end
' "$EVIDENCE/model-files-running.json")"
[[ "$model_file_id" =~ ^[1-9][0-9]*$ ]] || fail 'model_file_identity_missing'
jq -e --arg path "$M7E3_MODEL_PATH" --argjson bytes "$M7E3_MODEL_BYTES" '
  [.[] | select(.worker_id == 5 and .source == "local_path" and
    .local_path == $path and .state == "ready" and .download_progress == 100 and
    .size == $bytes and .resolved_paths == [$path] and
    ((.cleanup_on_delete == null) or (.cleanup_on_delete == false)))] | length == 1
' "$EVIDENCE/model-files-running.json" >/dev/null || fail 'model_file_ready_contract_mismatch'

require_equal "$(m7e3_owned_container_count "$instance_id")" '1' \
  'owned_container_count_not_one'
old_container_id="$(m7e3_owned_container_ids "$instance_id")"
owned_spec_hash="$(docker inspect --format \
  '{{ index .Config.Labels "io.gpustack.spec-hash" }}' "$old_container_id")"
[[ "$old_container_id" =~ ^[0-9a-f]{64}$ ]] || fail 'old_container_id_invalid'
[[ "$owned_spec_hash" =~ ^[0-9a-f]{64}$ ]] || fail 'owned_spec_hash_invalid'
capture_runtime_identity baseline || fail 'baseline_runtime_identity_failed'
require_equal "$(m7e3_gpu_compute_count "$M7E3_GPU0_UUID")" '1' \
  'gpu0_compute_process_count_not_one'
require_zero "$(m7e3_gpu_compute_count "$M7E3_GPU1_UUID")" 'gpu1_was_used'
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/baseline-direct" ||
  fail 'baseline_direct_vllm_failed'
server_inference_suite baseline-server || fail 'baseline_server_inference_failed'
event gate_passed runtime_baseline

phase='prepare_connection_fault_safety'
"$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
  worker maintenance enable "$M7E3_WORKER_ID" \
  --message "M7-E.3 attempt $FORMAL_ATTEMPT connection fault safety" \
  >"$EVIDENCE/maintenance-enable-before-outage.json"
jq -e '.worker_id == 5 and .after.enabled == true' \
  "$EVIDENCE/maintenance-enable-before-outage.json" >/dev/null ||
  fail 'maintenance_reenable_not_confirmed'
for _ in $(seq 1 60); do
  if "$M7E3_BINARY" --context "$M7E3_OPERATOR_CONTEXT" --output json \
    worker get "$M7E3_WORKER_ID" >"$EVIDENCE/worker-maintenance-before-outage.json" \
    2>"$EVIDENCE/worker-maintenance-before-outage.stderr" &&
    jq -e '.id == 5 and .maintenance.enabled == true and .state == "maintenance" and
      .unreachable == false' "$EVIDENCE/worker-maintenance-before-outage.json" >/dev/null; then
    break
  fi
  sleep 1
done
jq -e '.id == 5 and .maintenance.enabled == true and .state == "maintenance" and
  .unreachable == false' "$EVIDENCE/worker-maintenance-before-outage.json" >/dev/null ||
  fail 'worker_did_not_reenter_maintenance'
capture_runtime_identity fault-safety || fail 'fault_safety_runtime_identity_failed'
compare_runtime_contract baseline fault-safety ||
  fail 'runtime_changed_when_maintenance_reenabled'
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/fault-safety-direct" ||
  fail 'fault_safety_direct_vllm_failed'
server_inference_suite fault-safety-server ||
  fail 'fault_safety_server_inference_failed'
event gate_passed connection_fault_safety_maintenance_enabled

phase='server_connection_outage'
install -m 0600 /dev/null "$EVIDENCE/connection-outage.marker"
printf 'phase=server-connection-outage\narmed_at=%s\n' "$(date --iso-8601=seconds)" \
  >"$EVIDENCE/connection-outage.marker"
start_instance_watch || fail 'model_instance_watch_did_not_observe_running_baseline'
m7e3_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
  fail 'rust_process_identity_drift_before_server_outage'
capture_control_metrics control-baseline || fail 'control_baseline_capture_failed'
baseline_reconnects="$(watch_metric_sum "$EVIDENCE/control-baseline-metrics.txt" \
  'gpustack:rust_worker_watch_reconnects_total')"
require_equal "$(watch_metric_sum "$EVIDENCE/control-baseline-metrics.txt" \
  'gpustack:rust_worker_watch_connected')" '3' 'control_watch_not_connected_at_baseline'
fault_event_first_line="$(( $(wc -l <"$EVIDENCE/rust-events-before-outage.log") + 1 ))"
fault_started_epoch="$(date +%s)"
stop_proxy server || fail 'server_proxy_exact_stop_failed'
m7e3_server_proxy_probe && fail 'server_proxy_still_reachable_after_stop'
m7e3_docker_proxy_probe || fail 'docker_proxy_changed_during_server_outage'
event fault_injected "kind=server_proxy pid_stopped=true worker_pid=$rust_pid"

m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/server-outage-immediate-direct" ||
  fail 'server_outage_immediate_direct_inference_failed'
server_inference_suite server-outage-immediate-server no ||
  fail 'server_outage_immediate_server_inference_failed'
wait_for_control_disconnect "$baseline_reconnects" ||
  fail 'server_control_disconnect_not_observed'
for target in 40 "$M7E3_SERVER_OUTAGE_SECONDS"; do
  while [[ "$(( $(date +%s) - fault_started_epoch ))" -lt "$target" ]]; do
    sleep 1
  done
  sample="server-outage-${target}s"
  m7e3_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
    fail "${sample}_rust_worker_identity_changed"
  capture_runtime_identity "$sample" || fail "${sample}_runtime_capture_failed"
  compare_runtime_contract baseline "$sample" || fail "${sample}_runtime_drift"
  m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/${sample}-direct" ||
    fail "${sample}_direct_inference_failed"
  server_inference_suite "${sample}-server" no ||
    fail "${sample}_server_inference_failed"
done
[[ "$(( $(date +%s) - fault_started_epoch ))" -ge "$M7E3_SERVER_OUTAGE_SECONDS" ]] ||
  fail 'server_outage_duration_too_short'
start_proxy server recovered || fail 'server_proxy_restore_failed'
m7e3_server_proxy_probe || fail 'server_proxy_restore_probe_failed'
wait_for_control_recovery server-control-recovery ||
  fail 'server_control_recovery_timeout'
capture_runtime_identity after-server-recovery ||
  fail 'after_server_recovery_runtime_capture_failed'
compare_runtime_contract baseline after-server-recovery ||
  fail 'runtime_changed_across_server_outage'
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/after-server-recovery-direct" ||
  fail 'after_server_recovery_direct_inference_failed'
server_inference_suite after-server-recovery-server ||
  fail 'after_server_recovery_server_inference_failed'
server_outage_passed=1
event gate_passed server_connection_recovery

phase='between_outage_recovery_barrier'
sleep 4
m7e3_server_proxy_probe || fail 'server_proxy_failed_before_docker_outage'
m7e3_docker_proxy_probe || fail 'docker_proxy_failed_before_docker_outage'
capture_runtime_identity between-outages || fail 'between_outages_runtime_capture_failed'
compare_runtime_contract baseline between-outages || fail 'between_outages_runtime_drift'
m7e3_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
  fail 'rust_worker_identity_changed_between_outages'
event gate_passed independent_fault_barrier

phase='docker_connection_outage'
docker_event_first_line="$(( $(wc -l <"$EVIDENCE/rust-events-before-outage.log") + 1 ))"
docker_fault_epoch="$(date +%s)"
stop_proxy docker || fail 'docker_proxy_exact_stop_failed'
m7e3_docker_proxy_probe && fail 'docker_proxy_still_reachable_after_stop'
m7e3_server_proxy_probe || fail 'server_proxy_changed_during_docker_outage'
docker info >"$EVIDENCE/real-docker-during-outage.txt" 2>&1 ||
  fail 'real_docker_daemon_unhealthy_during_proxy_outage'
event fault_injected "kind=docker_proxy pid_stopped=true worker_pid=$rust_pid"
wait_for_docker_disconnect "$docker_event_first_line" ||
  fail 'docker_runtime_disconnect_not_observed'
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/docker-outage-observed-direct" ||
  fail 'docker_outage_observed_direct_inference_failed'
server_inference_suite docker-outage-observed-server no ||
  fail 'docker_outage_observed_server_inference_failed'
while [[ "$(( $(date +%s) - docker_fault_epoch ))" -lt "$M7E3_DOCKER_OUTAGE_SECONDS" ]]; do
  sleep 1
done
m7e3_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
  fail 'rust_worker_identity_changed_during_docker_outage'
capture_runtime_identity docker-outage-final ||
  fail 'docker_outage_final_runtime_capture_failed'
compare_runtime_contract baseline docker-outage-final ||
  fail 'runtime_changed_during_docker_outage'
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/docker-outage-final-direct" ||
  fail 'docker_outage_final_direct_inference_failed'
server_inference_suite docker-outage-final-server no ||
  fail 'docker_outage_final_server_inference_failed'
docker_recovery_first_line="$(( $(wc -l <"$EVIDENCE/rust-events-before-outage.log") + 1 ))"
start_proxy docker recovered || fail 'docker_proxy_restore_failed'
m7e3_docker_proxy_probe || fail 'docker_proxy_restore_probe_failed'
wait_for_docker_recovery "$docker_recovery_first_line" ||
  fail 'docker_runtime_reenumeration_timeout'
capture_runtime_identity after-docker-recovery ||
  fail 'after_docker_recovery_runtime_capture_failed'
compare_runtime_contract baseline after-docker-recovery ||
  fail 'runtime_changed_across_docker_outage'
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/after-docker-recovery-direct" ||
  fail 'after_docker_recovery_direct_inference_failed'
server_inference_suite after-docker-recovery-server ||
  fail 'after_docker_recovery_server_inference_failed'
docker_outage_passed=1
recovery_completed_epoch="$(date +%s)"
event gate_passed docker_connection_recovery

phase='connection_recovery_evidence'
stop_instance_watch || fail 'model_instance_watch_stop_failed'
jq -s -e --argjson instance "$instance_id" '
  [.[] | select(.data.id == $instance and (.data.state | type) == "string") |
    .data.state] as $states |
  ($states | length) >= 1 and all($states[]; . == "running")
' "$EVIDENCE/model-instance-watch.ndjson" >/dev/null ||
  fail 'model_instance_left_running_during_connection_outages'
jq -s --argjson instance "$instance_id" '
  [.[] | select(.data.id == $instance and (.data.state | type) == "string") |
    .data.state] |
  reduce .[] as $state ([];
    if length == 0 or .[-1] != $state then . + [$state] else . end)
' "$EVIDENCE/model-instance-watch.ndjson" >"$EVIDENCE/connection-outage-state-chain.json"
docker events --since "$fault_started_epoch" --until "$((recovery_completed_epoch + 1))" \
  --filter type=container \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed \
  --filter label=io.gpustack.worker-uuid="$M7E3_WORKER_UUID" \
  --filter label=io.gpustack.model-instance-id="$instance_id" \
  --filter event=create --filter event=start --filter event=die \
  --filter event=stop --filter event=kill --filter event=restart \
  --filter event=destroy \
  --format '{{json .}}' >"$EVIDENCE/docker-events-connection-outages.ndjson"
[[ ! -s "$EVIDENCE/docker-events-connection-outages.ndjson" ]] ||
  fail 'docker_lifecycle_event_occurred_during_connection_outages'
grep -Fq 'outcome=failed message=' "$EVIDENCE/docker-outage-observed.txt" \
  "$EVIDENCE"/docker-outage-*-runtime-events.log 2>/dev/null ||
  fail 'docker_uncertainty_local_failure_evidence_missing'
tail -n "+$fault_event_first_line" "$EVIDENCE/rust-events-before-outage.log" \
  >"$EVIDENCE/connection-outage-runtime-events.log"
if grep -Eq 'outcome=(error|restart_scheduled|deleted)|created=true' \
  "$EVIDENCE/connection-outage-runtime-events.log"; then
  fail 'authoritative_or_lifecycle_mutation_observed_during_connection_outages'
fi
require_equal "$(m7e3_owned_container_count "$instance_id")" '1' \
  'owned_container_count_changed_after_connection_outages'
require_equal "$(m7e3_owned_container_ids "$instance_id")" "$old_container_id" \
  'container_identity_changed_after_connection_outages'
printf 'server_outage_seconds=%s\ndocker_outage_seconds=%s\n' \
  "$M7E3_SERVER_OUTAGE_SECONDS" "$M7E3_DOCKER_OUTAGE_SECONDS" \
  >"$EVIDENCE/outage-durations.txt"
event gate_passed connection_recovery_evidence

phase='post_recovery_idempotency'
for sample in 1 2 3; do
  sleep 2
  m7e3_process_identity_matches_file "$EVIDENCE/rust-process-current.txt" ||
    fail 'rust_worker_exited_during_idempotency'
  capture_runtime_identity "idempotency-$sample" ||
    fail "idempotency_runtime_identity_${sample}_failed"
  compare_runtime_contract baseline "idempotency-$sample" ||
    fail "idempotency_runtime_identity_${sample}_drift"
done
m7e3_direct_vllm_probe "$assigned_port" "$EVIDENCE/post-idempotency-direct" ||
  fail 'post_idempotency_direct_inference_failed'
server_inference_suite post-idempotency-server ||
  fail 'post_idempotency_server_inference_failed'
require_equal "$server_outage_passed" '1' 'server_outage_gate_not_passed'
require_equal "$docker_outage_passed" '1' 'docker_outage_gate_not_passed'
recovery_passed=1
failure_reason=''
event recovery_gate_passed server_and_docker_connection_recovery
phase='recovery_passed_pending_cleanup'
exit 0
