#!/usr/bin/env bash
# Creates the bounded M7-F operator credential without exposing either token.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE="${M7F_CREDENTIAL_EVIDENCE:?M7F_CREDENTIAL_EVIDENCE is required}"
readonly EXPECTED_HEAD="${M7F_EXPECTED_HEAD:?M7F_EXPECTED_HEAD is required}"
readonly BINARY="$REPO/rust/target/debug/gpustack-rs"
readonly SOURCE_TOKEN='/home/wujiaxing2/.config/gpustack-rust/demo-operator-token'
readonly TARGET_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m7f-operator-token'
readonly CONTEXT_FILE='/home/wujiaxing2/.config/gpustack-rs/contexts.json'
readonly CONTEXT='m7f-operator'
readonly KEY_NAME='rust-worker-m7f-operator-20261106'
readonly EXPIRES_IN='604800'

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: credential evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi

install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

temporary_dir=''
response_file=''
request_file=''
token_candidate=''
source_keys=''
created_key_id=''
context_created=0
success=0
cleanup_ok=1
current_step='initialization'

api_request() {
  local token_file="$1"
  local method="$2"
  local url="$3"
  local output="$4"
  local body="${5:-}"
  local -a arguments=(
    --silent
    --show-error
    --max-time 20
    --request "$method"
    --output "$output"
    --write-out '%{http_code}'
  )

  if [[ -n "$body" ]]; then
    arguments+=(--header 'Content-Type: application/json' --data-binary "@$body")
  fi
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$token_file"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - "${arguments[@]}"
}

server_timestamp_epoch() {
  local timestamp="$1"

  if [[ "$timestamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]+)?$ ]]; then
    timestamp="${timestamp}Z"
  elif [[ ! "$timestamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})$ ]]; then
    return 1
  fi
  date -u -d "$timestamp" +%s 2>/dev/null
}

context_exists() {
  "$BINARY" --output json context list 2>/dev/null |
    jq -e --arg context "$CONTEXT" 'any(.[]; .name == $context)' >/dev/null
}

seal_evidence() {
  local status="$1"
  local reason="$2"

  printf 'status=%s\nstage=M7-F.3-credential\nreason=%s\nfailed_step=%s\ncleanup_ok=%s\n' \
    "$status" "$reason" "$current_step" "$cleanup_ok" >"$EVIDENCE/result.txt"
  find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  if grep -REIl \
    'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]' \
    "$EVIDENCE" >/dev/null; then
    printf 'secret_scan=FAIL\n' >"$EVIDENCE/secret-scan.txt"
    cleanup_ok=0
    sed -i 's/^status=.*/status=FAIL/; s/^cleanup_ok=.*/cleanup_ok=0/' \
      "$EVIDENCE/result.txt"
  else
    printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
  fi
  find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
  chmod 0600 "$EVIDENCE/manifest.sha256"
}

finish() {
  local code=$?
  local delete_status=''

  trap - EXIT
  set +o errexit
  if [[ "$success" -ne 1 ]]; then
    if [[ "$context_created" -eq 1 ]] || context_exists; then
      "$BINARY" --output json context remove "$CONTEXT" --yes \
        >"$EVIDENCE/rollback-context.json" 2>"$EVIDENCE/rollback-context.stderr" || cleanup_ok=0
    fi
    if [[ -e "$TARGET_TOKEN" || -L "$TARGET_TOKEN" ]]; then
      unlink "$TARGET_TOKEN" || cleanup_ok=0
    fi
    if [[ -n "$created_key_id" ]]; then
      delete_status="$(api_request "$SOURCE_TOKEN" DELETE \
        "http://127.0.0.1/v2/api-keys/$created_key_id" /dev/null)" || cleanup_ok=0
      case "$delete_status" in
        200|204) ;;
        *) cleanup_ok=0 ;;
      esac
      printf 'rollback_key_http_status=%s\n' "$delete_status" \
        >"$EVIDENCE/rollback-key.txt"
    fi
  fi
  for path in "$response_file" "$request_file" "$token_candidate" "$source_keys"; do
    if [[ -n "$path" && ( -e "$path" || -L "$path" ) ]]; then
      unlink "$path" || cleanup_ok=0
    fi
  done
  if [[ -n "$temporary_dir" && -d "$temporary_dir" ]]; then
    rmdir "$temporary_dir" || cleanup_ok=0
  fi

  if [[ "$success" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    seal_evidence PASS credential_and_context_ready
    printf 'credential_evidence=%s\nmanifest_file_sha256=%s\nkey_id=%s\nkey_name=%s\nm7f_credential=PASS\n' \
      "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
      "$created_key_id" "$KEY_NAME" >&3
    exit 0
  fi
  seal_evidence STOP credential_preparation_failed
  printf 'm7f_credential=STOP\nfailed_step=%s\ncleanup_ok=%s\n' \
    "$current_step" "$cleanup_ok" >&3
  [[ "$code" -ne 0 ]] && exit "$code"
  exit 1
}
trap finish EXIT

printf '[1/5] source, secret-file and name gates\n'
current_step='source_and_reserved_paths'
cd "$REPO"
source /home/wujiaxing2/.cargo/env
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
[[ -x "$BINARY" ]]
[[ -f "$SOURCE_TOKEN" && ! -L "$SOURCE_TOKEN" ]]
[[ "$(stat -c '%a' "$SOURCE_TOKEN")" == '600' ]]
[[ "$(stat -c '%U:%G' "$SOURCE_TOKEN")" == 'wujiaxing2:wujiaxing2' ]]
[[ "$(stat -c '%h' "$SOURCE_TOKEN")" == '1' ]]
[[ ! -e "$TARGET_TOKEN" && ! -L "$TARGET_TOKEN" ]]
! context_exists
install -d -m 0700 "$(dirname "$TARGET_TOKEN")" "$(dirname "$CONTEXT_FILE")"
temporary_dir="$(mktemp -d "$(dirname "$TARGET_TOKEN")/.m7f-credential.XXXXXX")"
response_file="$temporary_dir/response.json"
request_file="$temporary_dir/request.json"
token_candidate="$temporary_dir/operator.token"
source_keys="$temporary_dir/source-keys.json"
[[ "$(api_request "$SOURCE_TOKEN" GET \
  'http://127.0.0.1/v2/api-keys?page=-1' "$source_keys")" == '200' ]]
[[ "$(jq --arg name "$KEY_NAME" \
  '[.items[] | select(.name == $name)] | length' "$source_keys")" == '0' ]]

printf '[2/5] create bounded management and inference key\n'
current_step='create_api_key'
jq -n --arg name "$KEY_NAME" --argjson expires "$EXPIRES_IN" '{
  name:$name,
  description:"Rust Worker M7-F dual-GPU temporary operator",
  expires_in:$expires,
  scope:["management","inference"]
}' >"$request_file"
create_status="$(api_request "$SOURCE_TOKEN" POST \
  'http://127.0.0.1/v2/api-keys' "$response_file" "$request_file")"
[[ "$create_status" == '200' ]]
created_key_id="$(jq -r '.id // empty' "$response_file")"
[[ "$created_key_id" =~ ^[1-9][0-9]*$ ]]
jq -e --arg name "$KEY_NAME" --argjson id "$created_key_id" '
  .id == $id and .name == $name and
  ((.scope | sort) == ["inference","management"]) and
  .expires_at != null and (.value | type == "string")
' "$response_file" >/dev/null
created_at="$(jq -r '.created_at' "$response_file")"
expires_at="$(jq -r '.expires_at' "$response_file")"
lifetime_seconds=$(( $(server_timestamp_epoch "$expires_at") -
  $(server_timestamp_epoch "$created_at") ))
[[ "$lifetime_seconds" -gt 0 && "$lifetime_seconds" -le "$EXPIRES_IN" ]]
jq -rj '.value' "$response_file" >"$token_candidate"
chmod 0600 "$token_candidate"
LC_ALL=C grep -q '^gpustack_[^[:space:]]\+$' "$token_candidate"
jq '{id,name,scope,created_at,expires_at}' "$response_file" >"$EVIDENCE/api-key.json"
printf 'api_key_id=%s\napi_key_name=%s\nlifetime_seconds=%s\n' \
  "$created_key_id" "$KEY_NAME" "$lifetime_seconds" >"$EVIDENCE/credential-metadata.txt"

printf '[3/5] verify both permission scopes without exposing token\n'
current_step='verify_new_key'
[[ "$(api_request "$token_candidate" GET \
  'http://127.0.0.1/v2/workers/5' /dev/null)" == '200' ]]
[[ "$(api_request "$token_candidate" GET \
  'http://127.0.0.1/v1/models' /dev/null)" == '200' ]]

printf '[4/5] install protected token and active Context\n'
current_step='install_token_and_context'
mv "$token_candidate" "$TARGET_TOKEN"
chmod 0600 "$TARGET_TOKEN"
"$BINARY" --output json context add "$CONTEXT" \
  --server-url 'http://127.0.0.1/' \
  --token-file "$TARGET_TOKEN" \
  --use >"$EVIDENCE/context-add.json"
context_created=1
chmod 0600 "$CONTEXT_FILE"

printf '[5/5] verify public identity and protected local state\n'
current_step='verify_context_and_metadata'
"$BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e --arg context "$CONTEXT" --arg token "$TARGET_TOKEN" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
    .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts.json" >/dev/null
[[ "$(stat -c '%a' "$TARGET_TOKEN")" == '600' ]]
[[ "$(stat -c '%U:%G' "$TARGET_TOKEN")" == 'wujiaxing2:wujiaxing2' ]]
[[ "$(stat -c '%h' "$TARGET_TOKEN")" == '1' ]]
[[ "$(stat -c '%a' "$CONTEXT_FILE")" == '600' ]]
[[ "$(api_request "$TARGET_TOKEN" GET \
  "http://127.0.0.1/v2/api-keys?page=-1&search=$KEY_NAME" "$source_keys")" == '200' ]]
jq --argjson id "$created_key_id" --arg name "$KEY_NAME" '{
  items:[.items[] | select(.id == $id and .name == $name) |
    {id,name,scope,created_at,expires_at}]
}' "$source_keys" >"$EVIDENCE/api-key-list.json"
jq -e '(.items | length) == 1 and
  ((.items[0].scope | sort) == ["inference","management"])' \
  "$EVIDENCE/api-key-list.json" >/dev/null
stat -c 'token_file=%n mode=%a owner=%U:%G links=%h' "$TARGET_TOKEN" \
  >>"$EVIDENCE/credential-metadata.txt"
success=1
