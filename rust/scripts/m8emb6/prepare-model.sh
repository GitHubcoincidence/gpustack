#!/usr/bin/env bash
# Installs the already downloaded, pinned embedding model without contacting a Server.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly SOURCE_DIR="${M8EMB6_MODEL_SOURCE_DIR:?M8EMB6_MODEL_SOURCE_DIR is required}"

m8emb6_require_commands awk find install jq sha256sum stat
[[ -d "$SOURCE_DIR" && ! -L "$SOURCE_DIR" ]] ||
  m8emb6_die "model source directory is unavailable"
[[ "$(find "$SOURCE_DIR" -type l | wc -l)" == '0' ]] ||
  m8emb6_die "model source contains a symlink"

actual_files="$(find "$SOURCE_DIR" -type f -printf '%P\n' | sort)"
expected_files="$(awk '{print substr($0,67)}' "$M8EMB6_MODEL_MANIFEST" | sort)"
[[ "$actual_files" == "$expected_files" ]] ||
  m8emb6_die "staged model file set does not match the manifest"
(cd "$SOURCE_DIR" && sha256sum --check --strict "$M8EMB6_MODEL_MANIFEST") >/dev/null
[[ "$(find "$SOURCE_DIR" -type f -printf '%s\n' |
  awk '{sum += $1} END {print sum+0}')" == "$M8EMB6_MODEL_BYTES" ]] ||
  m8emb6_die "staged model byte count is invalid"

if [[ -e "$M8EMB6_MODEL_PATH" || -L "$M8EMB6_MODEL_PATH" ]]; then
  m8emb6_verify_model
  printf 'model_install=already_valid\nmodel_path=%s\n' "$M8EMB6_MODEL_PATH"
  exit 0
fi

readonly MODEL_PARENT="$(dirname "$M8EMB6_MODEL_PATH")"
readonly TEMP_TARGET="$MODEL_PARENT/.m8emb6-model-install-$$"
install -d -m 0755 "$MODEL_PARENT"
[[ ! -e "$TEMP_TARGET" && ! -L "$TEMP_TARGET" ]] ||
  m8emb6_die "temporary model target already exists"

cleanup_temp() {
  if [[ -d "$TEMP_TARGET" && ! -L "$TEMP_TARGET" ]]; then
    find "$TEMP_TARGET" -type f -exec chmod 0600 {} + 2>/dev/null || true
    find "$TEMP_TARGET" -type f -exec unlink {} \; 2>/dev/null || true
    rmdir "$TEMP_TARGET/1_Pooling" 2>/dev/null || true
    rmdir "$TEMP_TARGET" 2>/dev/null || true
  fi
}
trap cleanup_temp EXIT

install -d -m 0755 "$TEMP_TARGET/1_Pooling"
while IFS= read -r relative_path; do
  install -m 0444 "$SOURCE_DIR/$relative_path" "$TEMP_TARGET/$relative_path"
done < <(awk '{print substr($0,67)}' "$M8EMB6_MODEL_MANIFEST")
chmod 0555 "$TEMP_TARGET" "$TEMP_TARGET/1_Pooling"
mv -- "$TEMP_TARGET" "$M8EMB6_MODEL_PATH"
trap - EXIT

m8emb6_verify_model
printf 'model_install=PASS\nmodel_path=%s\nrevision=%s\nfiles=%s\nbytes=%s\n' \
  "$M8EMB6_MODEL_PATH" "$M8EMB6_MODEL_REVISION" \
  "$M8EMB6_MODEL_FILES" "$M8EMB6_MODEL_BYTES"
