#!/usr/bin/env bash
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE="${M6B5_PREFLIGHT_EVIDENCE:?M6B5_PREFLIGHT_EVIDENCE is required}"
readonly INJECTION_EVIDENCE="${M6B5_INJECTION_EVIDENCE:?M6B5_INJECTION_EVIDENCE is required}"
readonly EXPECTED_HEAD="${M6B5_EXPECTED_HEAD:?M6B5_EXPECTED_HEAD is required}"
readonly PRODUCT_COMMIT='c78cfc5652808ee5c16b40b11df411dadc7d6b6d'
readonly EXPECTED_LOCK_SHA='90f1aeaa22b4f06dc77e48497f2fe3394ab07ab55441c8ddb3124a67dca07010'
readonly EXECUTOR="$REPO/rust/scripts/m6b5/executor.sh"
readonly WATCHDOG="$REPO/rust/scripts/m6b5/watchdog.sh"
readonly BINARY="$REPO/rust/target/debug/gpustack-rs"
readonly TOKEN_FILE='/home/wujiaxing2/.config/gpustack-rust/m6b5-operator-token'
readonly CONTEXT_FILE='/home/wujiaxing2/.config/gpustack-rs/contexts.json'
readonly CONTEXT='m6b5-operator'
readonly KEY_ID='21'
readonly KEY_NAME='rust-worker-m6b5-operator-20261028-v2'
readonly WORKER_ID='5'
readonly WORKER_NAME='rust-m3d-r760xa'
readonly PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'
readonly TARGET_MODEL='m6b5-qwen25-05b-gpu0'
readonly RUNNER_IMAGE='quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly RUNNER_IMAGE_ID='sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly MODEL_PATH='/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct'
readonly MODEL_MANIFEST="$REPO/rust/fixtures/m5e/qwen2.5-0.5b-instruct-7ae557604adf.sha256"
readonly EXPECTED_MODEL_MANIFEST_SHA='645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6'
readonly EXPECTED_WEIGHT_SHA='fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe'
readonly GPU0_UUID='GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3'

current_step='initialization'
result_written=0

finish() {
  local code=$?
  trap - EXIT
  if [[ -d "$EVIDENCE" && "$result_written" -eq 0 ]]; then
    printf 'status=STOP\nstage=M6-B.5.1\nfailed_step=%s\nexit_code=%s\n' \
      "$current_step" "$code" >"$EVIDENCE/preflight-result.txt"
    chmod 0600 "$EVIDENCE/preflight-result.txt"
  fi
  exit "$code"
}
trap finish EXIT

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: preflight evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi
if [[ -e "$INJECTION_EVIDENCE" || -L "$INJECTION_EVIDENCE" ]]; then
  printf 'STOP: injection evidence path already exists: %s\n' "$INJECTION_EVIDENCE" >&2
  exit 91
fi
install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

announce() {
  printf '\n[%s/9] %s\n' "$1" "$2"
}

auth_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$TOKEN_FILE"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

snapshot() {
  local prefix="$1"
  local resource
  local status
  {
    printf 'server=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
      --write-out '%{http_code}' http://127.0.0.1/healthz)"
    printf 'python_worker=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
      --write-out '%{http_code}' http://127.0.0.1:10150/healthz)"
    printf 'python_metrics=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
      --write-out '%{http_code}' http://127.0.0.1:10151/metrics)"
  } >"$EVIDENCE/${prefix}-services.txt"
  grep -qx 'server=200' "$EVIDENCE/${prefix}-services.txt"
  grep -qx 'python_worker=200' "$EVIDENCE/${prefix}-services.txt"
  grep -qx 'python_metrics=200' "$EVIDENCE/${prefix}-services.txt"

  docker ps -aq --no-trunc | sort >"$EVIDENCE/${prefix}-container-ids.txt"
  docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed | sort \
    >"$EVIDENCE/${prefix}-rust-managed-container-ids.txt"
  docker inspect "$PYTHON_WORKER_ID" | jq '.[0] | {
    Id, Image, Name, restart_policy:.HostConfig.RestartPolicy,
    network_mode:.HostConfig.NetworkMode,
    mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
    running:.State.Running
  }' \
    >"$EVIDENCE/${prefix}-python-worker-inspect.json"
  nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
    --format=csv,noheader,nounits >"$EVIDENCE/${prefix}-gpus.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader,nounits >"$EVIDENCE/${prefix}-compute-processes.csv" || true
  : >"$EVIDENCE/${prefix}-listeners.txt"
  for port in 20150 20151 $(seq 40000 40063); do
    ss -H -lnt "sport = :$port" >>"$EVIDENCE/${prefix}-listeners.txt"
  done
  pgrep -af '[g]pustack-rs' >"$EVIDENCE/${prefix}-rust-processes.txt" || true

  for resource in models model-instances model-files workers; do
    status="$(auth_get "http://127.0.0.1/v2/${resource}?page=-1" \
      "$EVIDENCE/${prefix}-${resource}.json")"
    [[ "$status" == '200' ]]
  done
  jq -n \
    --argjson models "$(jq '[.items[].id] | sort' "$EVIDENCE/${prefix}-models.json")" \
    --argjson instances "$(jq '[.items[].id] | sort' "$EVIDENCE/${prefix}-model-instances.json")" \
    --argjson files "$(jq '[.items[].id] | sort' "$EVIDENCE/${prefix}-model-files.json")" \
    --argjson workers "$(jq '[.items[].id] | sort' "$EVIDENCE/${prefix}-workers.json")" \
    '{models:$models,model_instances:$instances,model_files:$files,workers:$workers}' \
    >"$EVIDENCE/${prefix}-global-ids.json"
}

current_step='source_and_scripts'
announce 1 '冻结源码、工具链与正式脚本身份'
cd "$REPO"
source /home/wujiaxing2/.cargo/env
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
git merge-base --is-ancestor "$PRODUCT_COMMIT" HEAD
git diff --quiet "$PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates
[[ "$(rustc --version)" == 'rustc 1.99.1 (58cf8f9c3 2026-10-13)' ]]
[[ "$(cargo --version)" == 'cargo 1.99.1 (5f94df478 2026-08-27)' ]]
[[ "$(sha256sum rust/Cargo.lock | awk '{print $1}')" == "$EXPECTED_LOCK_SHA" ]]
bash -n "$EXECUTOR"
bash -n "$WATCHDOG"
executor_sha="$(sha256sum "$EXECUTOR" | awk '{print $1}')"
watchdog_sha="$(sha256sum "$WATCHDOG" | awk '{print $1}')"
install -m 0600 "$0" "$EVIDENCE/preflight.sh"
install -m 0600 "$EXECUTOR" "$EVIDENCE/executor.sh"
install -m 0600 "$WATCHDOG" "$EVIDENCE/watchdog.sh"
{
  printf 'governance_head=%s\nproduct_commit=%s\norigin_head=%s\n' \
    "$EXPECTED_HEAD" "$PRODUCT_COMMIT" "$(git rev-parse origin/codex/rust-worker)"
  printf 'cargo_lock_sha256=%s\nexecutor_sha256=%s\nwatchdog_sha256=%s\n' \
    "$EXPECTED_LOCK_SHA" "$executor_sha" "$watchdog_sha"
  printf 'rustc=%s\ncargo=%s\n' "$(rustc --version)" "$(cargo --version)"
} >"$EVIDENCE/source-identity.txt"

current_step='credential_and_context'
announce 2 '阶段凭据、Context 与双 scope 认证'
[[ "$(stat -c '%a' "$TOKEN_FILE")" == '600' ]]
[[ "$(stat -c '%a' "$CONTEXT_FILE")" == '600' ]]
[[ "$(stat -c '%a' "$(dirname "$TOKEN_FILE")")" == '700' ]]
[[ "$(stat -c '%a' "$(dirname "$CONTEXT_FILE")")" == '700' ]]
[[ "$(stat -c '%U:%G' "$TOKEN_FILE")" == 'wujiaxing2:wujiaxing2' ]]
[[ "$(stat -c '%h' "$TOKEN_FILE")" == '1' ]]
"$BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e --arg context "$CONTEXT" --arg token "$TOKEN_FILE" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
    .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts.json" >/dev/null
[[ "$(auth_get "http://127.0.0.1/v2/api-keys?page=-1&search=$KEY_NAME" \
  "$EVIDENCE/api-keys.json")" == '200' ]]
jq -e --argjson id "$KEY_ID" --arg name "$KEY_NAME" '
  [.items[] | select(.id == $id and .name == $name and
    ((.scope | sort) == ["inference","management"]))] | length == 1
' "$EVIDENCE/api-keys.json" >/dev/null
created_at="$(jq -r --argjson id "$KEY_ID" '.items[] | select(.id == $id) | .created_at' \
  "$EVIDENCE/api-keys.json")"
expires_at="$(jq -r --argjson id "$KEY_ID" '.items[] | select(.id == $id) | .expires_at' \
  "$EVIDENCE/api-keys.json")"
[[ $(( $(date -d "$expires_at" +%s) - $(date -d "$created_at" +%s) )) -eq 86400 ]]
[[ "$(date -d "$expires_at" +%s)" -gt "$(date +%s)" ]]
[[ "$(auth_get 'http://127.0.0.1/v2/workers/5' "$EVIDENCE/management-auth.json")" == '200' ]]
[[ "$(auth_get 'http://127.0.0.1/v1/models' "$EVIDENCE/inference-auth.json")" == '200' ]]
{
  printf 'api_key_id=%s\napi_key_name=%s\nscope=management,inference\n' "$KEY_ID" "$KEY_NAME"
  printf 'created_at=%s\nexpires_at=%s\nlifetime_seconds=86400\n' "$created_at" "$expires_at"
  stat -c 'token_file=%n mode=%a owner=%U:%G links=%h' "$TOKEN_FILE"
} >"$EVIDENCE/credential-metadata.txt"

current_step='before_snapshot'
announce 3 '维护窗口前的完整只读外部基线'
snapshot before
[[ ! -s "$EVIDENCE/before-rust-managed-container-ids.txt" ]]
[[ ! -s "$EVIDENCE/before-listeners.txt" ]]
[[ ! -s "$EVIDENCE/before-rust-processes.txt" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'true' ]]
jq -e --argjson id "$WORKER_ID" --arg name "$WORKER_NAME" '
  [.items[] | select(.id == $id and .name == $name and
    .maintenance.enabled == true and .state == "maintenance" and .unreachable == false)] |
  length == 1
' "$EVIDENCE/before-workers.json" >/dev/null
jq -e --arg name "$TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/before-models.json" >/dev/null
jq -e '[.items[] | select(.worker_id == 5)] | length == 0' \
  "$EVIDENCE/before-model-instances.json" >/dev/null
jq -e --arg path "$MODEL_PATH" '
  [.items[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
  length == 0
' "$EVIDENCE/before-model-files.json" >/dev/null

current_step='fixed_runtime_inputs'
announce 4 'Runner、模型、GPU0、端口与 Python Worker 精确身份'
curl --silent --show-error --fail --max-time 10 http://127.0.0.1/version \
  >"$EVIDENCE/server-version.json"
jq -e '.version == "v2.2.3" and .git_commit == "1cfcbbf"' \
  "$EVIDENCE/server-version.json" >/dev/null
[[ "$(docker image inspect --format '{{.Id}}' "$RUNNER_IMAGE")" == "$RUNNER_IMAGE_ID" ]]
[[ "$(readlink -f "$MODEL_PATH")" == "$MODEL_PATH" ]]
[[ "$(sha256sum "$MODEL_MANIFEST" | awk '{print $1}')" == "$EXPECTED_MODEL_MANIFEST_SHA" ]]
(cd "$MODEL_PATH" && sha256sum --check --strict "$MODEL_MANIFEST") \
  >"$EVIDENCE/model-manifest-check.txt"
[[ "$(sha256sum "$MODEL_PATH/model.safetensors" | awk '{print $1}')" == "$EXPECTED_WEIGHT_SHA" ]]
[[ "$(find "$MODEL_PATH" -type f -printf '%s\n' | awk '{s+=$1} END {print s+0}')" == '999604126' ]]
[[ "$(awk -F', *' '$1 == 0 {print $2}' "$EVIDENCE/before-gpus.csv")" == "$GPU0_UUID" ]]
[[ "$(awk -F', *' '$1 == 0 {print $4}' "$EVIDENCE/before-gpus.csv")" == '0' ]]
! grep -q "$GPU0_UUID" "$EVIDENCE/before-compute-processes.csv"
[[ "$(docker inspect --format '{{.Id}}' "$PYTHON_WORKER_ID")" == "$PYTHON_WORKER_ID" ]]
{
  printf 'python_worker_id=%s\n' "$PYTHON_WORKER_ID"
  docker inspect --format 'image={{.Image}} restart={{.HostConfig.RestartPolicy.Name}} network={{.HostConfig.NetworkMode}} running={{.State.Running}}' \
    "$PYTHON_WORKER_ID"
  printf 'runner_image=%s\nrunner_image_id=%s\n' "$RUNNER_IMAGE" "$RUNNER_IMAGE_ID"
  printf 'gpu0_uuid=%s\nmodel_path=%s\n' "$GPU0_UUID" "$MODEL_PATH"
} >"$EVIDENCE/fixed-runtime-inputs.txt"

current_step='offline_build'
announce 5 'locked/offline 全 workspace 构建'
cargo build --manifest-path rust/Cargo.toml --workspace --all-targets --all-features \
  --locked --offline >"$EVIDENCE/cargo-build.log" 2>&1
"$BINARY" --output json version >"$EVIDENCE/binary-version.json"
jq -e --arg commit "${EXPECTED_HEAD:0:12}" '.git_commit == $commit' \
  "$EVIDENCE/binary-version.json" >/dev/null
printf 'binary_sha256=%s\n' "$(sha256sum "$BINARY" | awk '{print $1}')" \
  >"$EVIDENCE/binary-identity.txt"

current_step='failure_injection_tests'
announce 6 'Managed runtime 关键失败边界离线注入'
tests=(
  'resource_run::tests::missing_model_roots_fails_before_any_runtime_or_network_work'
  'managed::reconciler::tests::input_drift_before_create_prevents_every_runtime_mutation'
  'managed::reconciler::tests::state_drift_before_running_writeback_preserves_server_authority'
  'managed::reconciler::tests::readiness_timeout_reports_error_and_preserves_container'
  'managed::reconciler::tests::cache_gap_does_not_cleanup_an_authoritative_instance'
  'worker_api::tests::inference_proxy_uses_loopback_route_and_strips_sensitive_headers'
)
: >"$EVIDENCE/failure-injection-summary.txt"
for test_name in "${tests[@]}"; do
  safe_name="${test_name//::/_}"
  cargo test --manifest-path rust/Cargo.toml -p gpustack-worker --lib \
    "$test_name" --locked --offline -- --exact \
    >"$EVIDENCE/test-${safe_name}.log" 2>&1
  grep -q 'test result: ok. 1 passed; 0 failed' "$EVIDENCE/test-${safe_name}.log"
  printf 'PASS %s\n' "$test_name" >>"$EVIDENCE/failure-injection-summary.txt"
done

current_step='script_static_audit'
announce 7 '执行器和 watchdog 静态安全审计'
for required in \
  'run --managed-runtime' \
  'model deploy' \
  'inference chat' \
  'inference completion' \
  'model logs' \
  'owned_container_count' \
  'restore_python_worker'; do
  grep -Fq -- "$required" "$EXECUTOR"
done
for forbidden in 'managed-file-only' 'lab90' 'm6a3'; do
  ! grep -Fiq -- "$forbidden" "$EXECUTOR" "$WATCHDOG"
done
grep -Fq '1200' "$WATCHDOG"
grep -Fq 'PYTHON_WORKER_ID' "$WATCHDOG"
grep -Fq 'process_starttime' "$WATCHDOG"
printf 'bash_n=PASS\nrequired_paths=PASS\nlegacy_markers_absent=PASS\n' \
  >"$EVIDENCE/script-static-audit.txt"

current_step='no_write_executor_injection'
announce 8 '正式执行器无写故障注入'
set +o errexit
M6B5_EVIDENCE="$INJECTION_EVIDENCE" \
M6B5_PREFLIGHT="$EVIDENCE" \
M6B5_WATCHDOG="$WATCHDOG" \
M6B5_WATCHDOG_SHA="$watchdog_sha" \
M6B5_EXPECTED_HEAD='0000000000000000000000000000000000000000' \
M6B5_PRODUCT_COMMIT="$PRODUCT_COMMIT" \
M6B5_PREFLIGHT_MANIFEST_SHA='0000000000000000000000000000000000000000000000000000000000000000' \
M6B5_LOCK_SHA="$EXPECTED_LOCK_SHA" \
bash "$EXECUTOR" >"$EVIDENCE/injection-console.txt" 2>&1
injection_exit=$?
set -o errexit
[[ "$injection_exit" -ne 0 ]]
[[ "$(sed -n 's/^status=//p' "$INJECTION_EVIDENCE/result.txt")" == 'STOP' ]]
grep -q '^reason=source_head_mismatch$' "$INJECTION_EVIDENCE/result.txt"
printf 'injection_exit=%s\nstatus=PASS\nreason=source_head_mismatch\n' "$injection_exit" \
  >"$EVIDENCE/no-write-injection.txt"

current_step='post_snapshot_and_seal'
announce 9 '前后不变式、秘密扫描与证据封存'
snapshot after
for suffix in \
  services.txt container-ids.txt rust-managed-container-ids.txt python-worker-inspect.json \
  gpus.csv compute-processes.csv listeners.txt rust-processes.txt global-ids.json; do
  cmp "$EVIDENCE/before-$suffix" "$EVIDENCE/after-$suffix"
done
[[ -z "$(git status --porcelain)" ]]
(cd "$MODEL_PATH" && sha256sum --check --strict "$MODEL_MANIFEST") \
  >"$EVIDENCE/model-manifest-recheck.txt"

token="$(tr -d '\r\n' <"$TOKEN_FILE")"
if grep -RIlF -- "$token" "$EVIDENCE" "$INJECTION_EVIDENCE" >/dev/null; then
  unset token
  printf 'STOP: evidence contains the operator token\n' >&2
  exit 1
fi
unset token
if grep -REIl 'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]' \
  "$EVIDENCE" "$INJECTION_EVIDENCE" >/dev/null; then
  printf 'STOP: evidence contains a secret-shaped value\n' >&2
  exit 1
fi
printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
printf 'status=PASS\nstage=M6-B.5.1\nattempt=1\nexternal_mutation=none\nstatus_detail=ready_for_maintenance_authorization\n' \
  >"$EVIDENCE/preflight-result.txt"
result_written=1
find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
  sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
chmod 0600 "$EVIDENCE/manifest.sha256"
printf 'evidence_dir=%s\nmanifest_file_sha256=%s\nm6b5_preflight=PASS\nstatus=ready_for_maintenance_authorization\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
