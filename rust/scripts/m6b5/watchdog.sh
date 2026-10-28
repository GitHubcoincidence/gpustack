#!/usr/bin/env bash
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly EXECUTOR="${M6B5_EXECUTOR:?M6B5_EXECUTOR is required}"
readonly EXPECTED_EXECUTOR_SHA="${M6B5_EXECUTOR_SHA:?M6B5_EXECUTOR_SHA is required}"
readonly FORMAL_EVIDENCE="${M6B5_EVIDENCE:?M6B5_EVIDENCE is required}"
readonly WATCHDOG_EVIDENCE="${M6B5_WATCHDOG_EVIDENCE:?M6B5_WATCHDOG_EVIDENCE is required}"
readonly BINARY='/home/wujiaxing2/workspaces/gpustack/rust/target/debug/gpustack-rs'
readonly CONTEXT='m6b5-operator'
readonly WORKER_ID='5'
readonly PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'

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
exec 3>&1
exec >"$WATCHDOG_EVIDENCE/live.log" 2>&1

runner_exit=125
finalizing=0

process_starttime() {
  awk '{print $22}' "/proc/$1/stat"
}

terminate_recorded_rust_process() {
  local identity="$FORMAL_EVIDENCE/rust-process-identity.txt"
  local pid
  local starttime
  local exe
  local sha

  [[ -f "$identity" ]] || return 0
  pid="$(sed -n 's/^pid=//p' "$identity")"
  starttime="$(sed -n 's/^starttime=//p' "$identity")"
  exe="$(sed -n 's/^exe=//p' "$identity")"
  sha="$(sed -n 's/^sha256=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ ]] || return 1
  if [[ ! -r "/proc/$pid/stat" ]]; then
    return 0
  fi
  [[ "$(process_starttime "$pid")" == "$starttime" ]] || return 1
  [[ "$(readlink -f "/proc/$pid/exe")" == "$exe" ]] || return 1
  [[ "$(sha256sum "$exe" | awk '{print $1}')" == "$sha" ]] || return 1
  kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 30); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  return 1
}

restore_safety_baseline() {
  local recovery_ok=1
  local context_ready=0

  if [[ -x "$BINARY" ]] &&
    "$BINARY" --output json context list >"$WATCHDOG_EVIDENCE/contexts.json" 2>&1 &&
    jq -e '[.[] | select(.name == "m6b5-operator" and .active == true)] | length == 1' \
      "$WATCHDOG_EVIDENCE/contexts.json" >/dev/null; then
    context_ready=1
  fi
  if [[ "$context_ready" -eq 1 ]] &&
    "$BINARY" --context "$CONTEXT" --output json worker get "$WORKER_ID" \
      >"$WATCHDOG_EVIDENCE/worker-before-recovery.json" 2>&1 &&
    [[ "$(jq -r '.maintenance.enabled' "$WATCHDOG_EVIDENCE/worker-before-recovery.json")" != 'true' ]]; then
    "$BINARY" --context "$CONTEXT" --output json worker maintenance enable "$WORKER_ID" \
      --message 'M6-B.5 watchdog recovery' \
      >"$WATCHDOG_EVIDENCE/maintenance-recovery.json" 2>&1 || recovery_ok=0
  fi

  terminate_recorded_rust_process || recovery_ok=0
  if [[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID" 2>/dev/null)" != 'true' ]]; then
    docker start "$PYTHON_WORKER_ID" >"$WATCHDOG_EVIDENCE/python-worker-start.txt" \
      2>"$WATCHDOG_EVIDENCE/python-worker-start.stderr" || recovery_ok=0
  fi
  for _ in $(seq 1 120); do
    if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]]; then
      break
    fi
    sleep 1
  done
  [[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID" 2>/dev/null)" == 'true' ]] || recovery_ok=0
  [[ "$(curl --silent --max-time 5 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || recovery_ok=0
  printf 'recovery_ok=%s\n' "$recovery_ok" >"$WATCHDOG_EVIDENCE/recovery-result.txt"
}

seal_watchdog() {
  local status='FAIL'
  local secret_count
  [[ "$runner_exit" -eq 0 ]] && status='PASS'
  {
    printf 'recorded_at=%s\n' "$(date --iso-8601=seconds)"
    printf 'status=%s\nstage=M6-B.5.2-watchdog\nattempt=1\n' "$status"
    printf 'executor_exit=%s\ntimeout_seconds=1200\nkill_after_seconds=180\n' "$runner_exit"
  } >"$WATCHDOG_EVIDENCE/result.txt"
  find "$WATCHDOG_EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  secret_count="$(grep -RIlE \
    'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]' \
    "$WATCHDOG_EVIDENCE" 2>/dev/null | wc -l)"
  printf 'secret_file_count=%s\n' "$secret_count" >"$WATCHDOG_EVIDENCE/secret-scan.txt"
  if [[ "$secret_count" -ne 0 ]]; then
    runner_exit=1
    sed -i 's/^status=.*/status=FAIL/' "$WATCHDOG_EVIDENCE/result.txt"
  fi
  find "$WATCHDOG_EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$WATCHDOG_EVIDENCE/manifest.sha256"
  chmod 0600 "$WATCHDOG_EVIDENCE/manifest.sha256"
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

trap 'finalize' EXIT
trap 'on_signal TERM 143' TERM
trap 'on_signal INT 130' INT
trap 'on_signal HUP 129' HUP

set +o errexit
timeout --foreground --signal=TERM --kill-after=180 1200 "$EXECUTOR"
runner_exit=$?
set -o errexit
exit "$runner_exit"
