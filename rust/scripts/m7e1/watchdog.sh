#!/usr/bin/env bash
# shellcheck disable=SC1090,SC2317
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

readonly EXECUTOR="${M7E1_EXECUTOR:?M7E1_EXECUTOR is required}"
readonly EXPECTED_EXECUTOR_SHA="${M7E1_EXECUTOR_SHA:?M7E1_EXECUTOR_SHA is required}"
readonly FORMAL_EVIDENCE="${M7E1_EVIDENCE:?M7E1_EVIDENCE is required}"
readonly WATCHDOG_EVIDENCE="${M7E1_WATCHDOG_EVIDENCE:?M7E1_WATCHDOG_EVIDENCE is required}"
readonly FORMAL_ATTEMPT="${M7E1_ATTEMPT:?M7E1_ATTEMPT is required}"

if [[ ! "$FORMAL_ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7E1_ATTEMPT must be a positive integer\n' >&2
  exit 89
fi
watchdog_pid="$BASHPID"
watchdog_starttime="$(m7e1_process_starttime "$watchdog_pid")"
watchdog_pgid="$(m7e1_process_pgid "$watchdog_pid")"
watchdog_sid="$(m7e1_process_sid "$watchdog_pid")"
watchdog_shell="$(readlink -f "/proc/$watchdog_pid/exe")"
if [[ "$watchdog_pgid" != "$watchdog_pid" || "$watchdog_sid" != "$watchdog_pid" ]]; then
  printf 'STOP: watchdog must run as an independent session leader\n' >&2
  exit 93
fi
if [[ -e "$WATCHDOG_EVIDENCE" || -L "$WATCHDOG_EVIDENCE" ]]; then
  printf 'STOP: watchdog evidence path already exists: %s\n' "$WATCHDOG_EVIDENCE" >&2
  exit 90
fi
if [[ ! -f "$EXECUTOR" ]] ||
  [[ "$(sha256sum "$EXECUTOR" | awk '{print $1}')" != "$EXPECTED_EXECUTOR_SHA" ]]; then
  printf 'STOP: executor identity mismatch\n' >&2
  exit 91
fi
if [[ -e "$FORMAL_EVIDENCE" || -L "$FORMAL_EVIDENCE" ]]; then
  printf 'STOP: formal evidence path already exists: %s\n' "$FORMAL_EVIDENCE" >&2
  exit 92
fi

install -d -m 0700 "$WATCHDOG_EVIDENCE"
install -m 0600 "$0" "$WATCHDOG_EVIDENCE/watchdog.sh"
install -m 0600 "$EXECUTOR" "$WATCHDOG_EVIDENCE/executor.sh"
install -m 0600 "$LIB" "$WATCHDOG_EVIDENCE/lib.sh"
exec 3>&1
exec >"$WATCHDOG_EVIDENCE/live.log" 2>&1

runner_exit=125
finalizing=0
recovery_ok=1

{
  printf 'armed_at=%s\npid=%s\nstarttime=%s\npgid=%s\nsid=%s\n' \
    "$(m7e1_timestamp)" "$watchdog_pid" "$watchdog_starttime" "$watchdog_pgid" "$watchdog_sid"
  printf 'shell_exe=%s\nshell_sha256=%s\n' \
    "$watchdog_shell" "$(sha256sum "$watchdog_shell" | awk '{print $1}')"
  printf 'watchdog_sha256=%s\nexecutor_sha256=%s\nlib_sha256=%s\n' \
    "$(sha256sum "$0" | awk '{print $1}')" "$EXPECTED_EXECUTOR_SHA" "$EXPECTED_LIB_SHA"
  printf 'formal_evidence=%s\nwatchdog_evidence=%s\nattempt=%s\n' \
    "$FORMAL_EVIDENCE" "$WATCHDOG_EVIDENCE" "$FORMAL_ATTEMPT"
} >"$WATCHDOG_EVIDENCE/armed.txt"
chmod 0600 "$WATCHDOG_EVIDENCE/armed.txt"

enable_maintenance() {
  if [[ -x "$M7E1_BINARY" ]] &&
    "$M7E1_BINARY" --output json context list \
      >"$WATCHDOG_EVIDENCE/contexts.json" 2>&1 &&
    jq -e --arg context "$M7E1_OPERATOR_CONTEXT" '
      [.[] | select(.name == $context and .active == true)] | length == 1
    ' "$WATCHDOG_EVIDENCE/contexts.json" >/dev/null &&
    "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
      worker get "$M7E1_WORKER_ID" \
      >"$WATCHDOG_EVIDENCE/worker-before-recovery.json" 2>&1 &&
    [[ "$(jq -r '.maintenance.enabled' \
      "$WATCHDOG_EVIDENCE/worker-before-recovery.json")" != 'true' ]]; then
    "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
      worker maintenance enable "$M7E1_WORKER_ID" \
      --message "M7-E.1 attempt $FORMAL_ATTEMPT watchdog recovery" \
      >"$WATCHDOG_EVIDENCE/maintenance-recovery.json" 2>&1 || recovery_ok=0
  fi
}

terminate_rust_worker() {
  local identity="$FORMAL_EVIDENCE/rust-process-current.txt"
  if [[ -f "$identity" ]]; then
    m7e1_terminate_recorded_process "$identity" "$WATCHDOG_EVIDENCE" || recovery_ok=0
  elif [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" != '0' ]]; then
    # Unknown processes are never killed by name; recovery remains failed closed.
    recovery_ok=0
  fi
}

discover_resources() {
  local candidate
  model_id=''
  instance_id=''
  container_id=''
  spec_hash=''
  if [[ -f "$FORMAL_EVIDENCE/result.txt" ]]; then
    model_id="$(sed -n 's/^model_id=//p' "$FORMAL_EVIDENCE/result.txt")"
    instance_id="$(sed -n 's/^model_instance_id=//p' "$FORMAL_EVIDENCE/result.txt")"
    container_id="$(sed -n 's/^owned_container_id=//p' "$FORMAL_EVIDENCE/result.txt")"
    spec_hash="$(sed -n 's/^owned_spec_hash=//p' "$FORMAL_EVIDENCE/result.txt")"
    [[ "$model_id" != 'none' ]] || model_id=''
    [[ "$instance_id" != 'none' ]] || instance_id=''
    [[ "$container_id" != 'none' ]] || container_id=''
    [[ "$spec_hash" != 'none' ]] || spec_hash=''
  fi
  if "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json model list \
    >"$WATCHDOG_EVIDENCE/models.json" 2>"$WATCHDOG_EVIDENCE/models.stderr"; then
    candidate="$(jq -r --arg name "$M7E1_TARGET_MODEL" \
      '[.[] | select(.name == $name)] | if length == 1 then .[0].id else empty end' \
      "$WATCHDOG_EVIDENCE/models.json")"
    if [[ -z "$model_id" ]]; then
      model_id="$candidate"
    elif [[ -n "$candidate" && "$candidate" != "$model_id" ]]; then
      recovery_ok=0
    fi
  else
    recovery_ok=0
  fi
  if [[ "$model_id" =~ ^[1-9][0-9]*$ ]] &&
    [[ "$(m7e1_model_instance_get "http://127.0.0.1/v2/model-instances?page=-1&model_id=$model_id" \
      "$WATCHDOG_EVIDENCE/instances.json" || true)" == '200' ]]; then
    candidate="$(jq -r \
      'if (.items | length) == 1 then .items[0].id else empty end' \
      "$WATCHDOG_EVIDENCE/instances.json")"
    if [[ -z "$instance_id" ]]; then
      instance_id="$candidate"
    elif [[ -n "$candidate" && "$candidate" != "$instance_id" ]]; then
      recovery_ok=0
    fi
  fi
  if [[ -f "$FORMAL_EVIDENCE/pre-crash-container.json" ]]; then
    candidate="$(jq -r '.Id // empty' "$FORMAL_EVIDENCE/pre-crash-container.json")"
    [[ -n "$container_id" && "$container_id" != "$candidate" ]] && recovery_ok=0
    container_id="${container_id:-$candidate}"
    candidate="$(jq -r '.labels["io.gpustack.spec-hash"] // empty' \
      "$FORMAL_EVIDENCE/pre-crash-container.json")"
    [[ -n "$spec_hash" && "$spec_hash" != "$candidate" ]] && recovery_ok=0
    spec_hash="${spec_hash:-$candidate}"
    candidate="$(jq -r '.labels["io.gpustack.model-instance-id"] // empty' \
      "$FORMAL_EVIDENCE/pre-crash-container.json")"
    [[ -n "$instance_id" && "$instance_id" != "$candidate" ]] && recovery_ok=0
    instance_id="${instance_id:-$candidate}"
  fi
}

delete_authoritative_model() {
  [[ "$model_id" =~ ^[1-9][0-9]*$ ]] || return 0
  "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    model delete "$model_id" --yes >"$WATCHDOG_EVIDENCE/model-delete.json" \
    2>"$WATCHDOG_EVIDENCE/model-delete.stderr" || recovery_ok=0
  if [[ "$instance_id" =~ ^[1-9][0-9]*$ ]]; then
    instance_absent=0
    for _ in $(seq 1 120); do
      status="$(m7e1_model_instance_get "http://127.0.0.1/v2/model-instances/$instance_id" \
        "$WATCHDOG_EVIDENCE/instance-latest.json" || true)"
      if [[ "$status" == '404' ]]; then
        instance_absent=1
        break
      fi
      sleep 1
    done
    [[ "$instance_absent" -eq 1 ]] || recovery_ok=0
  fi
}

fallback_runtime_cleanup() {
  local status
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  status="$(m7e1_model_instance_get "http://127.0.0.1/v2/model-instances/$instance_id" \
    "$WATCHDOG_EVIDENCE/instance-before-fallback.json" || true)"
  [[ "$status" == '404' ]] || {
    recovery_ok=0
    return
  }
  if [[ "$(m7e1_owned_container_count "$instance_id")" != '0' ]]; then
    if [[ "$container_id" =~ ^[0-9a-f]{64}$ && "$spec_hash" =~ ^[0-9a-f]{64}$ ]]; then
      m7e1_remove_exact_owned_container "$instance_id" "$container_id" "$spec_hash" \
        "$WATCHDOG_EVIDENCE" || recovery_ok=0
    else
      recovery_ok=0
    fi
  fi
  if [[ "$(m7e1_owned_container_count "$instance_id")" == '0' ]]; then
    m7e1_remove_exact_local_state "$instance_id" || recovery_ok=0
  fi
}

delete_attempt_model_file() {
  local list="$WATCHDOG_EVIDENCE/model-files.json"
  local candidate
  local -a baseline_ids candidates
  [[ -f "$FORMAL_EVIDENCE/before-global-ids.json" ]] || {
    recovery_ok=0
    return
  }
  mapfile -t baseline_ids < <(jq -r '.model_files[]' "$FORMAL_EVIDENCE/before-global-ids.json")
  "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
    model file list --worker "$M7E1_WORKER_ID" >"$list" 2>"$list.stderr" || {
    recovery_ok=0
    return
  }
  mapfile -t candidates < <(jq -r --arg path "$M7E1_MODEL_PATH" '
    .[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path) | .id
  ' "$list")
  for candidate in "${candidates[@]:-}"; do
    [[ -n "$candidate" ]] || continue
    if printf '%s\n' "${baseline_ids[@]:-}" | grep -qx "$candidate"; then
      continue
    fi
    "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
      model file get "$candidate" >"$WATCHDOG_EVIDENCE/model-file-${candidate}.json" \
      2>"$WATCHDOG_EVIDENCE/model-file-${candidate}.stderr" || {
      recovery_ok=0
      continue
    }
    if jq -e --arg path "$M7E1_MODEL_PATH" '
      .id > 0 and .worker_id == 5 and .source == "local_path" and
      .local_path == $path and ((.cleanup_on_delete == null) or
      (.cleanup_on_delete == false))
    ' "$WATCHDOG_EVIDENCE/model-file-${candidate}.json" >/dev/null; then
      "$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
        model file delete "$candidate" --yes \
        >"$WATCHDOG_EVIDENCE/model-file-delete-${candidate}.json" \
        2>"$WATCHDOG_EVIDENCE/model-file-delete-${candidate}.stderr" || recovery_ok=0
    else
      recovery_ok=0
    fi
  done
}

restore_safety_baseline() {
  enable_maintenance
  terminate_rust_worker
  discover_resources
  delete_authoritative_model
  fallback_runtime_cleanup
  delete_attempt_model_file
  m7e1_restore_python_worker_if_safe "$WATCHDOG_EVIDENCE" || recovery_ok=0
  [[ "$(curl --silent --max-time 5 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1/healthz 2>/dev/null)" == '200' ]] || recovery_ok=0
  [[ "$(curl --silent --max-time 5 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || recovery_ok=0
  [[ "$(curl --silent --max-time 5 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]] || recovery_ok=0
  printf 'recovery_ok=%s\n' "$recovery_ok" >"$WATCHDOG_EVIDENCE/recovery-result.txt"
}

seal_watchdog() {
  local status='FAIL'
  local secret_count
  [[ "$runner_exit" -eq 0 ]] && status='PASS'
  [[ "$runner_exit" -eq 0 || "$recovery_ok" -eq 1 ]] || status='FAIL'
  {
    printf 'recorded_at=%s\n' "$(date --iso-8601=seconds)"
    printf 'status=%s\nstage=M7-E.1-watchdog\nattempt=%s\n' "$status" "$FORMAL_ATTEMPT"
    printf 'executor_exit=%s\ntimeout_seconds=1200\nkill_after_seconds=180\n' "$runner_exit"
    printf 'recovery_ok=%s\n' "$recovery_ok"
  } >"$WATCHDOG_EVIDENCE/result.txt"
  secret_count="$(m7e1_secret_file_count "$WATCHDOG_EVIDENCE")"
  printf 'secret_file_count=%s\n' "$secret_count" >"$WATCHDOG_EVIDENCE/secret-scan.txt"
  if [[ "$secret_count" -ne 0 ]]; then
    runner_exit=1
    sed -i 's/^status=.*/status=FAIL/' "$WATCHDOG_EVIDENCE/result.txt"
  fi
  m7e1_seal_directory "$WATCHDOG_EVIDENCE"
  printf 'watchdog_evidence=%s\nwatchdog_manifest_sha256=%s\nwatchdog_exit=%s\n' \
    "$WATCHDOG_EVIDENCE" \
    "$(sha256sum "$WATCHDOG_EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$runner_exit" >&3
}

finalize() {
  [[ "$finalizing" -eq 0 ]] || return
  finalizing=1
  trap - EXIT INT TERM HUP
  set +o errexit
  if [[ "$runner_exit" -ne 0 ]]; then
    restore_safety_baseline
  fi
  seal_watchdog
  exit "$runner_exit"
}

on_signal() {
  runner_exit="$2"
  exit "$2"
}

trap finalize EXIT
trap 'on_signal TERM 143' TERM
trap 'on_signal INT 130' INT
trap 'on_signal HUP 129' HUP

set +o errexit
timeout --foreground --signal=TERM --kill-after=180 1200 "$EXECUTOR"
runner_exit=$?
set -o errexit
exit "$runner_exit"
