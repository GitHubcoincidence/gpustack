#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd -P)"
# shellcheck disable=SC1091
source "$REPO_ROOT/hack/lib/init.sh"

set +o errexit
timeout --foreground --signal=TERM --kill-after=180 1800 \
  bash "$SCRIPT_DIR/executor.sh"
executor_exit=$?
set -o errexit

if [[ "$executor_exit" -ne 0 && \
  -d "${M8EMB7_FORMAL_EVIDENCE:?formal evidence is required}" && \
  ! -e "${M8EMB7_WATCHDOG_CLEANUP_EVIDENCE:?watchdog cleanup is required}" ]]; then
  M8EMB7_CLEANUP_EVIDENCE="$M8EMB7_WATCHDOG_CLEANUP_EVIDENCE" \
    bash "$SCRIPT_DIR/cleanup.sh" || true
fi

exit "$executor_exit"
