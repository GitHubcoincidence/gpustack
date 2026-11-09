#!/usr/bin/env bash
# Retires only the temporary M7-F operator credential after user acceptance.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE="${M7F_RETIREMENT_EVIDENCE:?M7F_RETIREMENT_EVIDENCE is required}"
readonly EXPECTED_HEAD="${M7F_EXPECTED_HEAD:?M7F_EXPECTED_HEAD is required}"
readonly BINARY="$REPO/rust/target/debug/gpustack-rs"
readonly SOURCE_TOKEN='/home/wujiaxing2/.config/gpustack-rust/demo-operator-token'
readonly TARGET_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m7f-operator-token'
readonly CLUSTER_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token'
readonly WORKER_TOKEN='/home/wujiaxing2/.local/state/gpustack-rust-worker/1/managed/worker-token'
readonly CONTEXT_FILE='/home/wujiaxing2/.config/gpustack-rs/contexts.json'
readonly CONTEXT='m7f-operator'
readonly KEY_ID='26'
readonly KEY_NAME='rust-worker-m7f-operator-20261106'
readonly PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'
readonly POST_AUDIT='/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m7f-post-audit-attempt-3'
readonly POST_AUDIT_MANIFEST_SHA='7dbba49d28326f07219ac3474c5156cfa1712c44b47608e15ecc42604e6cb9ce'

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: retirement evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi

install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec >"$EVIDENCE/console.log" 2>&1

temporary_dir=''
key_deleted=0
context_removed=0
token_removed=0
cleanup_ok=1
current_step='initialization'
target_device=''
target_inode=''
target_mode=''
target_owner=''
target_links=''
target_size=''
target_sha=''

api_request_file() {
  local token_file="$1"
  local method="$2"
  local url="$3"
  local output="$4"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$token_file"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --request "$method" --output "$output" --write-out '%{http_code}'
}

api_request_value() {
  local token="$1"
  local method="$2"
  local url="$3"
  local output="$4"
  {
    printf 'header = "Authorization: Bearer %s"\n' "$token"
    printf 'url = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --request "$method" --output "$output" --write-out '%{http_code}'
}

record_file_identity() {
  local path="$1"
  local output="$2"
  stat -c 'path=%n mode=%a owner=%U:%G links=%h device=%d inode=%i size=%s mtime=%Y' \
    "$path" >"$output"
  sha256sum "$path" >>"$output"
}

context_exists() {
  "$BINARY" --output json context list 2>/dev/null |
    jq -e --arg context "$CONTEXT" 'any(.[]; .name == $context)' >/dev/null
}

target_identity_matches() {
  [[ -f "$TARGET_TOKEN" && ! -L "$TARGET_TOKEN" ]] &&
    [[ "$(stat -c '%d' "$TARGET_TOKEN")" == "$target_device" ]] &&
    [[ "$(stat -c '%i' "$TARGET_TOKEN")" == "$target_inode" ]] &&
    [[ "$(stat -c '%a' "$TARGET_TOKEN")" == "$target_mode" ]] &&
    [[ "$(stat -c '%U:%G' "$TARGET_TOKEN")" == "$target_owner" ]] &&
    [[ "$(stat -c '%h' "$TARGET_TOKEN")" == "$target_links" ]] &&
    [[ "$(stat -c '%s' "$TARGET_TOKEN")" == "$target_size" ]] &&
    [[ "$(sha256sum "$TARGET_TOKEN" | awk '{print $1}')" == "$target_sha" ]]
}

seal_evidence() {
  local status="$1"
  local reason="$2"
  printf 'status=%s\nstage=M7-F.5-credential-retirement\nattempt=1\n' "$status" \
    >"$EVIDENCE/result.txt"
  printf 'reason=%s\ncompleted_step=%s\napi_key_id=%s\n' \
    "$reason" "$current_step" "$KEY_ID" >>"$EVIDENCE/result.txt"
  printf 'api_key_deleted=%s\ncontext_removed=%s\noperator_token_file_removed=%s\ncleanup_ok=%s\n' \
    "$key_deleted" "$context_removed" "$token_removed" "$cleanup_ok" \
    >>"$EVIDENCE/result.txt"
  if [[ "$status" == 'PASS' ]]; then
    printf 'old_token_http=401\nretained_credentials_unchanged=1\nstage_status=accepted\n' \
      >>"$EVIDENCE/result.txt"
  fi
  find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  if grep -REIl \
    'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]' \
    "$EVIDENCE" >/dev/null; then
    printf 'secret_scan=FAIL\n' >"$EVIDENCE/secret-scan.txt"
    sed -i 's/^status=.*/status=STOP/; s/^cleanup_ok=.*/cleanup_ok=0/' \
      "$EVIDENCE/result.txt"
    cleanup_ok=0
  else
    printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
  fi
  find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
  chmod 0600 "$EVIDENCE/manifest.sha256"
}

finish() {
  local code=$?
  local context_cleanup=0

  trap - EXIT
  set +o errexit
  if [[ "$code" -ne 0 && "$key_deleted" -eq 1 ]]; then
    if context_exists; then
      if "$BINARY" --output json context list 2>/dev/null |
        jq -e --arg context "$CONTEXT" --arg token "$TARGET_TOKEN" \
          'any(.[]; .name == $context and .token_file == $token)' >/dev/null; then
        "$BINARY" --output json context remove "$CONTEXT" --yes \
          >"$EVIDENCE/recovery-context-remove.json" 2>"$EVIDENCE/recovery-context-remove.stderr"
        context_cleanup=$?
        [[ "$context_cleanup" -eq 0 ]] && context_removed=1
      else
        cleanup_ok=0
      fi
    else
      context_removed=1
    fi
    if [[ -e "$TARGET_TOKEN" || -L "$TARGET_TOKEN" ]]; then
      if target_identity_matches; then
        if unlink "$TARGET_TOKEN"; then
          token_removed=1
        else
          cleanup_ok=0
        fi
      else
        cleanup_ok=0
      fi
    else
      token_removed=1
    fi
  fi
  if [[ -n "$temporary_dir" && -d "$temporary_dir" ]]; then
    find "$temporary_dir" -maxdepth 1 -type f -exec unlink {} +
    rmdir "$temporary_dir"
  fi
  if [[ "$code" -eq 0 && "$cleanup_ok" -eq 1 ]]; then
    seal_evidence PASS credential_retired_and_old_token_rejected
    printf 'retirement_evidence=%s\nmanifest_file_sha256=%s\nm7f_retirement=PASS\n' \
      "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
    exit 0
  fi
  seal_evidence STOP credential_retirement_failed
  printf 'm7f_retirement=STOP\nfailed_step=%s\ncleanup_ok=%s\n' \
    "$current_step" "$cleanup_ok" >&3
  [[ "$code" -ne 0 ]] && exit "$code"
  exit 1
}
trap finish EXIT

printf '[1/5] verify accepted evidence, repository and exact credential identity\n'
current_step='pre_delete_identity'
cd "$REPO"
source /home/wujiaxing2/.cargo/env
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
[[ -x "$BINARY" ]]
[[ "$(sha256sum "$POST_AUDIT/manifest.sha256" | awk '{print $1}')" == \
  "$POST_AUDIT_MANIFEST_SHA" ]]
(cd "$POST_AUDIT" && sha256sum --check manifest.sha256) \
  >"$EVIDENCE/post-audit-manifest-check.txt"
[[ "$(sed -n 's/^status=//p' "$POST_AUDIT/result.txt")" == 'PASS' ]]
[[ "$(sed -n 's/^recovery_baseline=//p' "$POST_AUDIT/result.txt")" == 'restored' ]]

for path in "$SOURCE_TOKEN" "$TARGET_TOKEN" "$CLUSTER_TOKEN" "$WORKER_TOKEN"; do
  [[ -f "$path" && ! -L "$path" ]]
  [[ "$(stat -c '%a' "$path")" == '600' ]]
  [[ "$(stat -c '%U:%G' "$path")" == 'wujiaxing2:wujiaxing2' ]]
  [[ "$(stat -c '%h' "$path")" == '1' ]]
done
[[ -f "$CONTEXT_FILE" && ! -L "$CONTEXT_FILE" ]]
[[ "$(stat -c '%a' "$CONTEXT_FILE")" == '600' ]]

target_device="$(stat -c '%d' "$TARGET_TOKEN")"
target_inode="$(stat -c '%i' "$TARGET_TOKEN")"
target_mode="$(stat -c '%a' "$TARGET_TOKEN")"
target_owner="$(stat -c '%U:%G' "$TARGET_TOKEN")"
target_links="$(stat -c '%h' "$TARGET_TOKEN")"
target_size="$(stat -c '%s' "$TARGET_TOKEN")"
target_sha="$(sha256sum "$TARGET_TOKEN" | awk '{print $1}')"
record_file_identity "$TARGET_TOKEN" "$EVIDENCE/operator-token-before.txt"
record_file_identity "$SOURCE_TOKEN" "$EVIDENCE/source-token-before.txt"
record_file_identity "$CLUSTER_TOKEN" "$EVIDENCE/cluster-token-before.txt"
record_file_identity "$WORKER_TOKEN" "$EVIDENCE/worker-token-before.txt"

"$BINARY" --output json context list >"$EVIDENCE/contexts-before.json"
jq -e --arg context "$CONTEXT" --arg token "$TARGET_TOKEN" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
    .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts-before.json" >/dev/null
jq --arg context "$CONTEXT" '[.[] | select(.name != $context)] | sort_by(.name)' \
  "$EVIDENCE/contexts-before.json" >"$EVIDENCE/contexts-retained-before.json"

temporary_dir="$(mktemp -d "$HOME/.config/gpustack-rust/.m7f-retirement.XXXXXX")"
keys_before="$temporary_dir/keys-before.json"
keys_after="$temporary_dir/keys-after.json"
old_token="$(tr -d '\r\n' <"$TARGET_TOKEN")"
[[ "$(api_request_file "$SOURCE_TOKEN" GET \
  "http://127.0.0.1/v2/api-keys?page=-1&search=$KEY_NAME" "$keys_before")" == '200' ]]
jq --argjson id "$KEY_ID" --arg name "$KEY_NAME" '{
  items:[.items[] | select(.id == $id and .name == $name) |
    {id,name,scope,created_at,expires_at}]
}' "$keys_before" >"$EVIDENCE/api-key-before.json"
jq -e '(.items | length) == 1 and
  ((.items[0].scope | sort) == ["inference","management"])' \
  "$EVIDENCE/api-key-before.json" >/dev/null
[[ "$(api_request_value "$old_token" GET 'http://127.0.0.1/v2/workers/5' /dev/null)" == '200' ]]
printf 'old_token_http=200\n' >"$EVIDENCE/token-auth-before.txt"

printf '[2/5] delete only API Key 26 and prove the old token is rejected\n'
current_step='delete_api_key'
delete_status="$(api_request_file "$SOURCE_TOKEN" DELETE \
  "http://127.0.0.1/v2/api-keys/$KEY_ID" /dev/null)"
case "$delete_status" in
  200|204) ;;
  *) exit 1 ;;
esac
key_deleted=1
printf 'api_key_id=%s\napi_key_delete_http=%s\n' "$KEY_ID" "$delete_status" \
  >"$EVIDENCE/api-key-delete.txt"
old_token_status="$(api_request_value "$old_token" GET \
  'http://127.0.0.1/v2/workers/5' /dev/null)"
[[ "$old_token_status" == '401' ]]
printf 'old_token_http=%s\n' "$old_token_status" >"$EVIDENCE/token-auth-after.txt"
[[ "$(api_request_file "$SOURCE_TOKEN" GET \
  "http://127.0.0.1/v2/api-keys?page=-1&search=$KEY_NAME" "$keys_after")" == '200' ]]
jq --argjson id "$KEY_ID" --arg name "$KEY_NAME" \
  '[.items[] | select(.id == $id or .name == $name)] | length == 0' \
  "$keys_after" >/dev/null

printf '[3/5] remove only the M7-F Context and protected token file\n'
current_step='remove_local_credential'
"$BINARY" --output json context remove "$CONTEXT" --yes \
  >"$EVIDENCE/context-remove.json"
context_removed=1
"$BINARY" --output json context list >"$EVIDENCE/contexts-after.json"
jq -e --arg context "$CONTEXT" 'all(.[]; .name != $context)' \
  "$EVIDENCE/contexts-after.json" >/dev/null
jq --arg context "$CONTEXT" '[.[] | select(.name != $context)] | sort_by(.name)' \
  "$EVIDENCE/contexts-after.json" >"$EVIDENCE/contexts-retained-after.json"
cmp "$EVIDENCE/contexts-retained-before.json" "$EVIDENCE/contexts-retained-after.json"
target_identity_matches
unlink "$TARGET_TOKEN"
token_removed=1
[[ ! -e "$TARGET_TOKEN" && ! -L "$TARGET_TOKEN" ]]
unset old_token

printf '[4/5] prove retained credentials and service baseline are unchanged\n'
current_step='verify_retained_state'
record_file_identity "$SOURCE_TOKEN" "$EVIDENCE/source-token-after.txt"
record_file_identity "$CLUSTER_TOKEN" "$EVIDENCE/cluster-token-after.txt"
record_file_identity "$WORKER_TOKEN" "$EVIDENCE/worker-token-after.txt"
cmp "$EVIDENCE/source-token-before.txt" "$EVIDENCE/source-token-after.txt"
cmp "$EVIDENCE/cluster-token-before.txt" "$EVIDENCE/cluster-token-after.txt"
cmp "$EVIDENCE/worker-token-before.txt" "$EVIDENCE/worker-token-after.txt"
printf 'source_token_preserved=PASS\ncluster_token_preserved=PASS\nworker_token_preserved=PASS\n' \
  >"$EVIDENCE/retained-credentials.txt"

for spec in 'server http://127.0.0.1/healthz' \
  'python_worker http://127.0.0.1:10150/healthz' \
  'python_metrics http://127.0.0.1:10151/metrics'; do
  read -r label url <<<"$spec"
  code="$(curl --silent --show-error --max-time 10 --output /dev/null \
    --write-out '%{http_code}' "$url")"
  [[ "$code" == '200' ]]
  printf '%s=%s\n' "$label" "$code" >>"$EVIDENCE/services-after.txt"
done
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'true' ]]
[[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]]
[[ "$(docker ps -aq --no-trunc --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed | wc -l)" == '0' ]]
ss -H -lnt 2>/dev/null | awk '
  {split($4,a,":"); p=a[length(a)]+0;
   if (p == 20150 || p == 20151 || (p >= 40000 && p <= 40063)) n++}
  END {print "reserved_listener_count=" n+0}' >"$EVIDENCE/project-listeners.txt"
[[ "$(sed -n 's/^reserved_listener_count=//p' "$EVIDENCE/project-listeners.txt")" == '0' ]]
nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader,nounits >"$EVIDENCE/gpus-after.csv"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
  --format=csv,noheader,nounits >"$EVIDENCE/compute-after.csv" 2>"$EVIDENCE/compute-after.stderr" || true

printf '[5/5] seal protected retirement evidence\n'
current_step='seal_evidence'
printf 'recorded_at=%s\nhead=%s\norigin_head=%s\nbranch=%s\n' \
  "$(date --iso-8601=seconds)" "$(git rev-parse HEAD)" \
  "$(git rev-parse origin/codex/rust-worker)" "$(git branch --show-current)" \
  >"$EVIDENCE/source-identity.txt"
[[ -z "$(git status --porcelain)" ]]
