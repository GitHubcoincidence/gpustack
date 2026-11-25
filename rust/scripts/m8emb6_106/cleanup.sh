#!/usr/bin/env bash
# Idempotent exact cleanup wrapper for an interrupted M8-EMB.6 106 window.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
set -o allexport
# shellcheck disable=SC1091
source "$SCRIPT_DIR/common.sh"
set +o allexport

readonly PREFLIGHT="${M8EMB6106_PREFLIGHT_EVIDENCE:?M8EMB6106_PREFLIGHT_EVIDENCE is required}"
readonly PREFLIGHT_SHA="${M8EMB6106_PREFLIGHT_MANIFEST_SHA:?M8EMB6106_PREFLIGHT_MANIFEST_SHA is required}"
readonly FORMAL="${M8EMB6106_EVIDENCE:?M8EMB6106_EVIDENCE is required}"
readonly CLEANUP="${M8EMB6106_CLEANUP_EVIDENCE:?M8EMB6106_CLEANUP_EVIDENCE is required}"

exec python3 "$SCRIPT_DIR/runtime_tool.py" cleanup \
  --preflight "$PREFLIGHT" \
  --preflight-sha "$PREFLIGHT_SHA" \
  --formal-evidence "$FORMAL" \
  --evidence "$CLEANUP"
