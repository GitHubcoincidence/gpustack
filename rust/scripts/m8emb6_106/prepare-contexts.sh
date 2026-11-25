#!/usr/bin/env bash
# Create an isolated two-Server context file after protected tokens are present.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

m8emb6106_require_commands install python3 stat
for token_file in \
  "$M8EMB6106_PYTHON_OPERATOR_TOKEN" \
  "$M8EMB6106_RUST_OPERATOR_TOKEN" \
  "$M8EMB6106_CLUSTER_TOKEN"; do
  m8emb6106_require_private_file "$token_file"
done

if [[ ! -e "$M8EMB6106_CONTEXTS_FILE" && ! -L "$M8EMB6106_CONTEXTS_FILE" ]]; then
  install -d -m 0700 "$(dirname "$M8EMB6106_CONTEXTS_FILE")"
  "$M8EMB6106_BINARY" --output json context add \
    "$M8EMB6106_PYTHON_CONTEXT" \
    --server-url "$M8EMB6106_PYTHON_SERVER_URL" \
    --token-file "$M8EMB6106_PYTHON_OPERATOR_TOKEN" \
    >"${M8EMB6106_CONTEXTS_FILE}.python-add.json"
  "$M8EMB6106_BINARY" --output json context add \
    "$M8EMB6106_RUST_CONTEXT" \
    --server-url "$M8EMB6106_RUST_SERVER_URL" \
    --token-file "$M8EMB6106_RUST_OPERATOR_TOKEN" \
    >"${M8EMB6106_CONTEXTS_FILE}.rust-add.json"
  unlink "${M8EMB6106_CONTEXTS_FILE}.python-add.json"
  unlink "${M8EMB6106_CONTEXTS_FILE}.rust-add.json"
fi

m8emb6106_require_private_file "$M8EMB6106_CONTEXTS_FILE"
readonly CONTEXT_SNAPSHOT="$(mktemp /tmp/m8emb6-106-contexts.XXXXXX)"
trap 'unlink "$CONTEXT_SNAPSHOT" 2>/dev/null || true' EXIT
"$M8EMB6106_BINARY" --output json context list >"$CONTEXT_SNAPSHOT"
python3 "$M8EMB6106_JSON_TOOL" check-contexts \
  --file "$CONTEXT_SNAPSHOT" \
  --python-name "$M8EMB6106_PYTHON_CONTEXT" \
  --python-url "$M8EMB6106_PYTHON_SERVER_URL" \
  --python-token "$M8EMB6106_PYTHON_OPERATOR_TOKEN" \
  --rust-name "$M8EMB6106_RUST_CONTEXT" \
  --rust-url "$M8EMB6106_RUST_SERVER_URL" \
  --rust-token "$M8EMB6106_RUST_OPERATOR_TOKEN"
printf 'context_preparation=PASS\ncontexts_file=%s\n' "$M8EMB6106_CONTEXTS_FILE"
