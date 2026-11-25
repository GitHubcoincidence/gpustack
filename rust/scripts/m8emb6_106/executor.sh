#!/usr/bin/env bash
# Formal executor wrapper. All lifecycle changes are implemented by runtime_tool.py.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
set -o allexport
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"
set +o allexport

readonly EXPECTED_HEAD="${M8EMB6106_EXPECTED_HEAD:?M8EMB6106_EXPECTED_HEAD is required}"
readonly PREFLIGHT="${M8EMB6106_PREFLIGHT_EVIDENCE:?M8EMB6106_PREFLIGHT_EVIDENCE is required}"
readonly PREFLIGHT_SHA="${M8EMB6106_PREFLIGHT_MANIFEST_SHA:?M8EMB6106_PREFLIGHT_MANIFEST_SHA is required}"
readonly EVIDENCE="${M8EMB6106_EVIDENCE:?M8EMB6106_EVIDENCE is required}"
readonly CLEANUP="${M8EMB6106_CLEANUP_EVIDENCE:?M8EMB6106_CLEANUP_EVIDENCE is required}"
readonly AUTHORIZATION="${M8EMB6106_WINDOW_AUTHORIZATION:?M8EMB6106_WINDOW_AUTHORIZATION is required}"

exec python3 "$SCRIPT_DIR/runtime_tool.py" execute \
  --expected-head "$EXPECTED_HEAD" \
  --preflight "$PREFLIGHT" \
  --preflight-sha "$PREFLIGHT_SHA" \
  --evidence "$EVIDENCE" \
  --cleanup-evidence "$CLEANUP" \
  --authorization "$AUTHORIZATION"
