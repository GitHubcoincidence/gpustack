#!/usr/bin/env bash
# Independent read-only audit after formal cleanup and restoration.
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
readonly EVIDENCE="${M8EMB6106_POST_AUDIT_EVIDENCE:?M8EMB6106_POST_AUDIT_EVIDENCE is required}"
readonly INSTANCE_ID="${M8EMB6106_EXPERIMENTAL_INSTANCE_ID:-none}"

exec python3 "$SCRIPT_DIR/runtime_tool.py" post-audit \
  --expected-head "$EXPECTED_HEAD" \
  --preflight "$PREFLIGHT" \
  --preflight-sha "$PREFLIGHT_SHA" \
  --evidence "$EVIDENCE" \
  --experimental-instance-id "$INSTANCE_ID"
