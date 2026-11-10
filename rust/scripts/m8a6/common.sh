#!/usr/bin/env bash
# Shared fail-closed validation for the M8 Rust Worker container tools.
set -o errexit -o errtrace -o nounset -o pipefail

readonly M8A6_CONTAINER_NAME='gpustack-rust-worker'
readonly M8A6_COMPONENT='rust-worker'
readonly M8A6_IMPLEMENTATION='rust'

fail() {
  printf 'STOP: %s\n' "$*" >&2
  exit 1
}

require_env() {
  local name="$1"
  [[ -n "${!name:-}" ]] || fail "$name is required"
}

require_lower_hex_commit() {
  local value="$1"
  [[ "$value" =~ ^[0-9a-f]{40}$ ]] || fail 'expected Git commit must be 40 lowercase hexadecimal characters'
}

require_image_id() {
  local value="$1"
  [[ "$value" =~ ^sha256:[0-9a-f]{64}$ ]] || fail 'expected image ID must be a full sha256 identity'
}

require_safe_image_reference() {
  local value="$1"
  [[ "$value" =~ ^[a-zA-Z0-9][a-zA-Z0-9._/@:-]*$ ]] || fail 'image reference contains unsupported characters'
}

require_executable() {
  local path="$1"
  [[ "$path" = /* && -x "$path" ]] || fail "executable is unavailable: $path"
}

canonical_path() {
  local label="$1"
  local path="$2"
  local resolved

  [[ "$path" = /* ]] || fail "$label must be an absolute path"
  [[ "$path" != *','* && "$path" != *$'\n'* ]] || fail "$label cannot contain a comma or newline"
  [[ ! -L "$path" ]] || fail "$label cannot be a symbolic link"
  resolved="$(readlink -f -- "$path")" || fail "$label cannot be canonicalized"
  [[ "$resolved" == "$path" ]] || fail "$label must already be canonical: $path"
  printf '%s\n' "$resolved"
}

require_private_file() {
  local label="$1"
  local path="$2"
  local expected_uid="$3"

  [[ -f "$path" ]] || fail "$label must be a regular file"
  [[ "$(stat -c '%a' -- "$path")" == '600' ]] || fail "$label must have mode 0600"
  [[ "$(stat -c '%u' -- "$path")" == "$expected_uid" ]] || fail "$label owner must match the container UID"
  [[ "$(stat -c '%h' -- "$path")" == '1' ]] || fail "$label must have exactly one hard link"
}

require_private_directory() {
  local label="$1"
  local path="$2"
  local expected_uid="$3"

  [[ -d "$path" ]] || fail "$label must be a directory"
  [[ "$(stat -c '%a' -- "$path")" == '700' ]] || fail "$label must have mode 0700"
  [[ "$(stat -c '%u' -- "$path")" == "$expected_uid" ]] || fail "$label owner must match the container UID"
}

normalize_optional_label() {
  local value="$1"
  if [[ "$value" == '<no value>' ]]; then
    printf '\n'
  else
    printf '%s\n' "$value"
  fi
}
