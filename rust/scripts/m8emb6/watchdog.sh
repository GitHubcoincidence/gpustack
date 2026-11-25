#!/usr/bin/env bash
# Runs the M8-EMB.6 executor under a hard deadline and verifies restoration.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly EXPECTED_HEAD="${M8EMB6_EXPECTED_HEAD:?M8EMB6_EXPECTED_HEAD is required}"
readonly PREFLIGHT="${M8EMB6_PREFLIGHT_EVIDENCE:?M8EMB6_PREFLIGHT_EVIDENCE is required}"
readonly EXPECTED_PREFLIGHT_SHA="${M8EMB6_PREFLIGHT_MANIFEST_SHA:?M8EMB6_PREFLIGHT_MANIFEST_SHA is required}"
readonly FORMAL_EVIDENCE="${M8EMB6_EVIDENCE:?M8EMB6_EVIDENCE is required}"
readonly CLEANUP_EVIDENCE="${M8EMB6_CLEANUP_EVIDENCE:?M8EMB6_CLEANUP_EVIDENCE is required}"
readonly EVIDENCE="${M8EMB6_WATCHDOG_EVIDENCE:?M8EMB6_WATCHDOG_EVIDENCE is required}"
readonly AUTHORIZATION="${M8EMB6_WINDOW_AUTHORIZATION:?M8EMB6_WINDOW_AUTHORIZATION is required}"
readonly EXECUTOR="$SCRIPT_DIR/executor.sh"

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  m8emb6_die "watchdog evidence path already exists: $EVIDENCE"
fi
install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

runner_exit=1
recovery_ok=0
finalizing=0

run_fallback_cleanup() {
  [[ ! -e "$CLEANUP_EVIDENCE" && ! -L "$CLEANUP_EVIDENCE" ]] || return 0
  M8EMB6_PREFLIGHT_EVIDENCE="$PREFLIGHT" \
  M8EMB6_PREFLIGHT_MANIFEST_SHA="$EXPECTED_PREFLIGHT_SHA" \
  M8EMB6_EVIDENCE="$FORMAL_EVIDENCE" \
  M8EMB6_CLEANUP_EVIDENCE="$CLEANUP_EVIDENCE" \
    "$SCRIPT_DIR/cleanup.sh"
}

verify_cleanup() {
  [[ -d "$CLEANUP_EVIDENCE" && -f "$CLEANUP_EVIDENCE/manifest.sha256" ]] || return 1
  (cd "$CLEANUP_EVIDENCE" && sha256sum --check manifest.sha256) \
    >"$EVIDENCE/cleanup-manifest-check.txt"
  [[ "$(sed -n 's/^cleanup_ok=//p' "$CLEANUP_EVIDENCE/result.txt")" == '1' ]]
}

finalize() {
  local code=$?
  [[ "$finalizing" -eq 0 ]] || return
  finalizing=1
  trap - EXIT INT TERM HUP
  set +o errexit
  if ! verify_cleanup; then
    run_fallback_cleanup
  fi
  if verify_cleanup; then
    recovery_ok=1
  fi
  if [[ "$runner_exit" -eq 0 && "$recovery_ok" -eq 1 ]]; then
    status='PASS'
  else
    status='FAIL'
    code=1
  fi
  {
    printf 'status=%s\nstage=M8-EMB.6-watchdog\n' "$status"
    printf 'executor_exit=%s\nrecovery_ok=%s\n' "$runner_exit" "$recovery_ok"
    printf 'timeout_seconds=1800\nkill_after_seconds=180\n'
  } >"$EVIDENCE/result.txt"
  [[ "$(m8emb6_secret_scan_count "$EVIDENCE")" == '0' ]] || {
    sed -i 's/^status=.*/status=FAIL/' "$EVIDENCE/result.txt"
    code=1
  }
  m8emb6_seal_directory "$EVIDENCE"
  printf 'watchdog_evidence=%s\nwatchdog_manifest_sha256=%s\nwatchdog_exit=%s\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$code" >&3
  exit "$code"
}
trap finalize EXIT
trap 'runner_exit=143; exit 143' TERM HUP
trap 'runner_exit=130; exit 130' INT

set +o errexit
timeout --foreground --signal=TERM --kill-after=180 1800 env \
  M8EMB6_EXPECTED_HEAD="$EXPECTED_HEAD" \
  M8EMB6_PREFLIGHT_EVIDENCE="$PREFLIGHT" \
  M8EMB6_PREFLIGHT_MANIFEST_SHA="$EXPECTED_PREFLIGHT_SHA" \
  M8EMB6_EVIDENCE="$FORMAL_EVIDENCE" \
  M8EMB6_CLEANUP_EVIDENCE="$CLEANUP_EVIDENCE" \
  M8EMB6_WINDOW_AUTHORIZATION="$AUTHORIZATION" \
  "$EXECUTOR"
runner_exit=$?
set -o errexit
exit "$runner_exit"
