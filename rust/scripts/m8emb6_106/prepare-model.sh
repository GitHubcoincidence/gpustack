#!/usr/bin/env bash
# Install the pinned BGE model on 106 without contacting either Server.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly SOURCE_DIR="${M8EMB6106_MODEL_SOURCE_DIR:?M8EMB6106_MODEL_SOURCE_DIR is required}"

m8emb6106_require_commands find install mv python3 stat
[[ -d "$SOURCE_DIR" && ! -L "$SOURCE_DIR" ]] ||
  m8emb6106_die "model source directory is unavailable"

python3 "$M8EMB6106_JSON_TOOL" verify-model \
  --model-path "$SOURCE_DIR" \
  --manifest "$M8EMB6106_MODEL_MANIFEST" \
  --metadata "$M8EMB6106_MODEL_METADATA" \
  --revision "$M8EMB6106_MODEL_REVISION" \
  --files "$M8EMB6106_MODEL_FILES" \
  --bytes "$M8EMB6106_MODEL_BYTES" \
  --dimension "$M8EMB6106_EMBEDDING_DIMENSION"

if [[ -e "$M8EMB6106_MODEL_PATH" || -L "$M8EMB6106_MODEL_PATH" ]]; then
  m8emb6106_verify_model
  printf 'model_install=already_valid\nmodel_path=%s\n' "$M8EMB6106_MODEL_PATH"
  exit 0
fi

readonly MODEL_PARENT="$(dirname "$M8EMB6106_MODEL_PATH")"
readonly TEMP_TARGET="$MODEL_PARENT/.m8emb6-106-model-install-$$"
install -d -m 0755 "$MODEL_PARENT"
[[ ! -e "$TEMP_TARGET" && ! -L "$TEMP_TARGET" ]] ||
  m8emb6106_die "temporary model target already exists"

cleanup_temp() {
  if [[ -d "$TEMP_TARGET" && ! -L "$TEMP_TARGET" ]]; then
    find "$TEMP_TARGET" -type f -exec chmod 0600 {} + 2>/dev/null || true
    find "$TEMP_TARGET" -type f -exec unlink {} \; 2>/dev/null || true
    find "$TEMP_TARGET" -depth -type d -exec rmdir {} + 2>/dev/null || true
  fi
}
trap cleanup_temp EXIT

while IFS= read -r relative_path; do
  install -d -m 0755 "$TEMP_TARGET/$(dirname "$relative_path")"
  install -m 0444 "$SOURCE_DIR/$relative_path" "$TEMP_TARGET/$relative_path"
done < <(sed -n 's/^[0-9a-f]\{64\}  //p' "$M8EMB6106_MODEL_MANIFEST")
find "$TEMP_TARGET" -type d -exec chmod 0555 {} +
mv -- "$TEMP_TARGET" "$M8EMB6106_MODEL_PATH"
trap - EXIT

m8emb6106_verify_model
printf 'model_install=PASS\nmodel_path=%s\nrevision=%s\nfiles=%s\nbytes=%s\n' \
  "$M8EMB6106_MODEL_PATH" "$M8EMB6106_MODEL_REVISION" \
  "$M8EMB6106_MODEL_FILES" "$M8EMB6106_MODEL_BYTES"
