#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd -P)"
# shellcheck disable=SC1091
source "$REPO_ROOT/hack/lib/init.sh"

readonly SERVER_URL="${M8EMB7_SERVER_URL:-http://127.0.0.1:18091}"
readonly CONFIG_DIR="${M8EMB7_CONFIG_DIR:-/home/wujiaxing/.config/gpustack-rust}"
readonly COOKIE_FILE="${M8EMB7_COOKIE_FILE:-$CONFIG_DIR/m8emb7-admin-session.cookies}"
readonly TOKEN_FILE="${M8EMB7_TOKEN_FILE:-$CONFIG_DIR/m8emb7-admin-session-token}"

cookie_stage=''
token_stage=''
response_stage=''

cleanup() {
  local path
  for path in "$cookie_stage" "$token_stage" "$response_stage"; do
    if [[ -n "$path" && -f "$path" && ! -L "$path" ]]; then
      unlink "$path"
    fi
  done
}
trap cleanup EXIT

die() {
  printf 'STOP: %s\n' "$*" >&2
  exit 1
}

escape_curl_config() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  printf '%s' "$value"
}

auth_status() {
  local mode="$1"
  local url="$2"
  local output="$3"
  local token fd
  if [[ "$mode" == 'cookie' ]]; then
    curl --silent --show-error --max-time 30 \
      --cookie "$cookie_stage" --output "$output" \
      --write-out '%{http_code}' "$url"
    return
  fi

  token="$(tr -d '\r\n' <"$token_stage")"
  exec {fd}<<<"header = \"Authorization: Bearer $token\""
  curl --config "/dev/fd/$fd" --silent --show-error --max-time 30 \
    --output "$output" --write-out '%{http_code}' "$url"
  exec {fd}<&-
  unset token
}

for command_name in curl install mktemp stat tr awk wc; do
  command -v "$command_name" >/dev/null 2>&1 ||
    die "required command is missing: $command_name"
done

install -d -m 0700 "$CONFIG_DIR"
for target in "$COOKIE_FILE" "$TOKEN_FILE"; do
  [[ ! -e "$target" && ! -L "$target" ]] ||
    die "protected target already exists: $target"
done

read -r -p 'GPUStack管理员用户名: ' username
read -r -s -p 'GPUStack管理员密码（输入不回显）: ' password
printf '\n'
[[ -n "$username" && -n "$password" ]] || die 'username and password are required'

cookie_stage="$(mktemp "$CONFIG_DIR/.m8emb7-admin-cookies.XXXXXX")"
token_stage="$(mktemp "$CONFIG_DIR/.m8emb7-admin-token.XXXXXX")"
response_stage="$(mktemp "$CONFIG_DIR/.m8emb7-admin-response.XXXXXX")"
chmod 0600 "$cookie_stage" "$token_stage" "$response_stage"

exec {login_fd}< <(
  printf 'request = "POST"\n'
  printf 'url = "%s/auth/login"\n' "$SERVER_URL"
  printf 'data-urlencode = "username=%s"\n' "$(escape_curl_config "$username")"
  printf 'data-urlencode = "password=%s"\n' "$(escape_curl_config "$password")"
)
login_status="$(curl --config "/dev/fd/$login_fd" --silent --show-error \
  --max-time 30 --cookie-jar "$cookie_stage" --output "$response_stage" \
  --write-out '%{http_code}')"
exec {login_fd}<&-
unset username password
[[ "$login_status" == '200' ]] || die "admin login returned HTTP $login_status"

awk -F '\t' \
  '($0 !~ /^#/ || $0 ~ /^#HttpOnly_/) && $6 == "gpustack_session" {print $7}' \
  "$cookie_stage" >"$token_stage"
[[ "$(wc -l <"$token_stage")" == '1' && -s "$token_stage" ]] ||
  die 'login response did not contain one gpustack_session cookie'

cookie_models="$(auth_status cookie "$SERVER_URL/v2/models?page=-1" "$response_stage")"
cookie_instances="$(auth_status cookie \
  "$SERVER_URL/v2/model-instances?page=-1" "$response_stage")"
[[ "$cookie_models" == '200' && "$cookie_instances" == '200' ]] ||
  die "cookie session is not authorized: models=$cookie_models instances=$cookie_instances"

bearer_models="$(auth_status bearer "$SERVER_URL/v2/models?page=-1" "$response_stage")"
bearer_instances="$(auth_status bearer \
  "$SERVER_URL/v2/model-instances?page=-1" "$response_stage")"

install -m 0600 "$cookie_stage" "$COOKIE_FILE"
bearer_compatible='no'
if [[ "$bearer_models" == '200' && "$bearer_instances" == '200' ]]; then
  install -m 0600 "$token_stage" "$TOKEN_FILE"
  bearer_compatible='yes'
fi

for target in "$COOKIE_FILE"; do
  [[ -f "$target" && ! -L "$target" ]] || die "protected output missing: $target"
  [[ "$(stat -c '%a:%U:%h' "$target")" == '600:wujiaxing:1' ]] ||
    die "protected output metadata is invalid: $target"
done
if [[ "$bearer_compatible" == 'yes' ]]; then
  [[ "$(stat -c '%a:%U:%h' "$TOKEN_FILE")" == '600:wujiaxing:1' ]] ||
    die "protected token metadata is invalid: $TOKEN_FILE"
fi

printf 'admin_session=PASS\n'
printf 'cookie_models_http=%s cookie_instances_http=%s\n' \
  "$cookie_models" "$cookie_instances"
printf 'bearer_models_http=%s bearer_instances_http=%s\n' \
  "$bearer_models" "$bearer_instances"
printf 'bearer_compatible=%s\n' "$bearer_compatible"
printf 'cookie_file=%s\n' "$COOKIE_FILE"
if [[ "$bearer_compatible" == 'yes' ]]; then
  printf 'token_file=%s\n' "$TOKEN_FILE"
fi
