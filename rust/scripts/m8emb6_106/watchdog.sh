#!/usr/bin/env bash
# Bound the formal window and retry exact cleanup if the executor is terminated.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly WATCHDOG_CLEANUP="${M8EMB6106_WATCHDOG_CLEANUP_EVIDENCE:?M8EMB6106_WATCHDOG_CLEANUP_EVIDENCE is required}"
readonly FORMAL="${M8EMB6106_EVIDENCE:?M8EMB6106_EVIDENCE is required}"

set +o errexit
timeout --foreground --signal=TERM --kill-after=180 1800 \
  "$SCRIPT_DIR/executor.sh"
executor_exit=$?
set -o errexit

if [[ "$executor_exit" -ne 0 && -d "$FORMAL" && \
  ! -e "$WATCHDOG_CLEANUP" && ! -L "$WATCHDOG_CLEANUP" ]]; then
  M8EMB6106_CLEANUP_EVIDENCE="$WATCHDOG_CLEANUP" \
    "$SCRIPT_DIR/cleanup.sh" || true
fi

exit "$executor_exit"
