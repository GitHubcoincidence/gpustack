#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd -P)"
# shellcheck disable=SC1091
source "$REPO_ROOT/hack/lib/init.sh"

python3 "$SCRIPT_DIR/runtime_tool.py" post-audit \
  --expected-head "${M8EMB7_EXPECTED_HEAD:?expected HEAD is required}" \
  --preflight "${M8EMB7_PREFLIGHT_EVIDENCE:?preflight evidence is required}" \
  --preflight-sha "${M8EMB7_PREFLIGHT_SHA:?preflight SHA is required}" \
  --evidence "${M8EMB7_AUDIT_EVIDENCE:?audit evidence is required}" \
  --formal-evidence "${M8EMB7_FORMAL_EVIDENCE:?formal evidence is required}" \
  --experimental-instance-id "${M8EMB7_INSTANCE_ID:-none}"
