#!/usr/bin/env bash
# Read-only gate for the M7-E.1 Worker crash maintenance window.
# shellcheck disable=SC1090,SC1091,SC2317
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE="${M7E1_PREFLIGHT_EVIDENCE:?M7E1_PREFLIGHT_EVIDENCE is required}"
readonly INJECTION_EVIDENCE="${M7E1_INJECTION_EVIDENCE:?M7E1_INJECTION_EVIDENCE is required}"
readonly EXPECTED_HEAD="${M7E1_EXPECTED_HEAD:?M7E1_EXPECTED_HEAD is required}"
readonly PREFLIGHT_ATTEMPT="${M7E1_PREFLIGHT_ATTEMPT:?M7E1_PREFLIGHT_ATTEMPT is required}"
readonly KEY_ID="${M7E1_KEY_ID:?M7E1_KEY_ID is required}"
readonly KEY_NAME="${M7E1_KEY_NAME:?M7E1_KEY_NAME is required}"
readonly PRODUCT_COMMIT='0d861a4c9d5502341cbc78eefc60e30bb32c24fd'
readonly EXPECTED_LOCK_SHA='90f1aeaa22b4f06dc77e48497f2fe3394ab07ab55441c8ddb3124a67dca07010'
readonly LIB="$REPO/rust/scripts/m7e1/lib.sh"
readonly EXECUTOR="$REPO/rust/scripts/m7e1/executor.sh"
readonly WATCHDOG="$REPO/rust/scripts/m7e1/watchdog.sh"
readonly POST_AUDIT="$REPO/rust/scripts/m7e1/post-audit.sh"

if [[ ! "$PREFLIGHT_ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7E1_PREFLIGHT_ATTEMPT must be a positive integer\n' >&2
  exit 88
fi
if [[ ! "$KEY_ID" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7E1_KEY_ID must be a positive integer\n' >&2
  exit 89
fi
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

current_step='initialization'
result_written=0

finish() {
  local code=$?
  trap - EXIT
  if [[ -d "$EVIDENCE" && "$result_written" -eq 0 ]]; then
    printf 'status=STOP\nstage=M7-E.1-preflight\nattempt=%s\nfailed_step=%s\nexit_code=%s\n' \
      "$PREFLIGHT_ATTEMPT" "$current_step" "$code" >"$EVIDENCE/preflight-result.txt"
    chmod 0600 "$EVIDENCE/preflight-result.txt"
  fi
  exit "$code"
}
trap finish EXIT

announce() {
  printf '\n[%s/8] %s\n' "$1" "$2"
}

snapshot() {
  local prefix="$1"
  local port
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

  m7e1_capture_docker_ids >"$EVIDENCE/${prefix}-docker-ids.txt"
  docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed | sort \
    >"$EVIDENCE/${prefix}-rust-container-ids.txt"
  m7e1_capture_python_worker "$EVIDENCE/${prefix}-python-worker.json"
  m7e1_capture_gpu "$EVIDENCE" "$prefix"
  : >"$EVIDENCE/${prefix}-listeners.txt"
  for port in 20150 20151 $(seq 40000 40063); do
    ss -H -lntp "sport = :$port" >>"$EVIDENCE/${prefix}-listeners.txt"
  done
  pgrep -af '[g]pustack-rs' >"$EVIDENCE/${prefix}-rust-processes.txt" || true
  m7e1_global_snapshot "$EVIDENCE" "$prefix"
}

current_step='source_and_script_identity'
announce 1 '冻结源码、工具链和五份正式脚本身份'
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
for script in "$0" "$LIB" "$EXECUTOR" "$WATCHDOG" "$POST_AUDIT"; do
  [[ -f "$script" && ! -L "$script" ]]
  bash -n "$script"
done
lib_sha="$(sha256sum "$LIB" | awk '{print $1}')"
executor_sha="$(sha256sum "$EXECUTOR" | awk '{print $1}')"
watchdog_sha="$(sha256sum "$WATCHDOG" | awk '{print $1}')"
post_audit_sha="$(sha256sum "$POST_AUDIT" | awk '{print $1}')"
install -m 0600 "$0" "$EVIDENCE/preflight.sh"
install -m 0600 "$LIB" "$EVIDENCE/lib.sh"
install -m 0600 "$EXECUTOR" "$EVIDENCE/executor.sh"
install -m 0600 "$WATCHDOG" "$EVIDENCE/watchdog.sh"
install -m 0600 "$POST_AUDIT" "$EVIDENCE/post-audit.sh"
source "$LIB"
{
  printf 'governance_head=%s\nproduct_commit=%s\norigin_head=%s\n' \
    "$EXPECTED_HEAD" "$PRODUCT_COMMIT" "$(git rev-parse origin/codex/rust-worker)"
  printf 'cargo_lock_sha256=%s\nlib_sha256=%s\n' "$EXPECTED_LOCK_SHA" "$lib_sha"
  printf 'executor_sha256=%s\nwatchdog_sha256=%s\npost_audit_sha256=%s\n' \
    "$executor_sha" "$watchdog_sha" "$post_audit_sha"
  printf 'preflight_sha256=%s\nrustc=%s\ncargo=%s\n' \
    "$(sha256sum "$0" | awk '{print $1}')" "$(rustc --version)" "$(cargo --version)"
} >"$EVIDENCE/source-identity.txt"

current_step='offline_build_and_recovery_tests'
announce 2 'locked/offline 构建和接管边界定向测试'
cargo build --manifest-path rust/Cargo.toml --workspace --all-targets --all-features \
  --locked --offline >"$EVIDENCE/cargo-build.log" 2>&1
"$M7E1_BINARY" --output json version >"$EVIDENCE/binary-version.json"
jq -e --arg commit "${EXPECTED_HEAD:0:12}" '.git_commit == $commit' \
  "$EVIDENCE/binary-version.json" >/dev/null
printf 'binary_sha256=%s\n' "$(sha256sum "$M7E1_BINARY" | awk '{print $1}')" \
  >"$EVIDENCE/binary-identity.txt"
tests=(
  'managed::reconciler::tests::running_snapshot_adopts_the_same_healthy_container_without_mutation'
  'managed::reconciler::tests::running_snapshot_rebuilds_a_missing_recovery_manifest'
  'managed::reconciler::tests::new_coordinator_rebuilds_route_and_log_capture_without_restarting_container'
  'managed::reconciler::tests::startup_adopts_a_nonzero_restart_generation_and_keeps_two_logs'
  'managed::reconciler::tests::unknown_and_conflicting_containers_are_never_mutated'
  'managed::reconciler::tests::running_snapshot_refuses_a_conflicting_recovery_manifest'
)
: >"$EVIDENCE/recovery-test-summary.txt"
for test_name in "${tests[@]}"; do
  safe_name="${test_name//::/_}"
  cargo test --manifest-path rust/Cargo.toml -p gpustack-worker --lib \
    "$test_name" --locked --offline -- --exact \
    >"$EVIDENCE/test-${safe_name}.log" 2>&1
  grep -q 'test result: ok. 1 passed; 0 failed' "$EVIDENCE/test-${safe_name}.log"
  printf 'PASS %s\n' "$test_name" >>"$EVIDENCE/recovery-test-summary.txt"
done

current_step='credential_and_context'
announce 3 '阶段凭据、Context 与双 scope 认证'
[[ -f "$M7E1_OPERATOR_TOKEN" && ! -L "$M7E1_OPERATOR_TOKEN" ]]
[[ "$(stat -c '%a' "$M7E1_OPERATOR_TOKEN")" == '600' ]]
[[ "$(stat -c '%U:%G' "$M7E1_OPERATOR_TOKEN")" == 'wujiaxing2:wujiaxing2' ]]
[[ "$(stat -c '%h' "$M7E1_OPERATOR_TOKEN")" == '1' ]]
[[ "$(stat -c '%a' "$(dirname "$M7E1_OPERATOR_TOKEN")")" == '700' ]]
[[ -f "$M7E1_CLUSTER_TOKEN" && ! -L "$M7E1_CLUSTER_TOKEN" ]]
[[ "$(stat -c '%a' "$M7E1_CLUSTER_TOKEN")" == '600' ]]
[[ "$(stat -c '%U:%G' "$M7E1_CLUSTER_TOKEN")" == 'wujiaxing2:wujiaxing2' ]]
[[ "$(stat -c '%h' "$M7E1_CLUSTER_TOKEN")" == '1' ]]
"$M7E1_BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e --arg context "$M7E1_OPERATOR_CONTEXT" --arg token "$M7E1_OPERATOR_TOKEN" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
    .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts.json" >/dev/null
[[ "$(m7e1_filtered_get operator \
  "http://127.0.0.1/v2/api-keys?page=-1&search=$KEY_NAME" \
  "$EVIDENCE/api-keys.json" \
  '{items:[.items[] | {id,name,scope,created_at,expires_at}]}')" == '200' ]]
jq -e --argjson id "$KEY_ID" --arg name "$KEY_NAME" '
  [.items[] | select(.id == $id and .name == $name and
    ((.scope | sort) == ["inference","management"]))] | length == 1
' "$EVIDENCE/api-keys.json" >/dev/null
created_at="$(jq -r --argjson id "$KEY_ID" \
  '.items[] | select(.id == $id) | .created_at' "$EVIDENCE/api-keys.json")"
expires_at="$(jq -r --argjson id "$KEY_ID" \
  '.items[] | select(.id == $id) | .expires_at' "$EVIDENCE/api-keys.json")"
lifetime_seconds=$(( $(date -d "$expires_at" +%s) - $(date -d "$created_at" +%s) ))
[[ "$lifetime_seconds" -gt 0 && "$lifetime_seconds" -le 86400 ]]
[[ "$(date -d "$expires_at" +%s)" -gt "$(date +%s)" ]]
[[ "$(m7e1_filtered_get operator 'http://127.0.0.1/v2/workers/5' \
  "$EVIDENCE/management-auth.json" \
  '{id,name,state,unreachable,maintenance,port,metrics_port,gpu_count,ip}')" == '200' ]]
[[ "$(m7e1_filtered_get operator 'http://127.0.0.1/v1/models' \
  "$EVIDENCE/inference-auth.json" \
  '{object,data:[.data[] | {id,object,created,owned_by}]}')" == '200' ]]
[[ "$(m7e1_filtered_get cluster 'http://127.0.0.1/v2/workers?me=true' \
  "$EVIDENCE/cluster-auth.json" \
  '{items:[.items[] | {id,name,state,unreachable,maintenance,port,metrics_port,gpu_count,ip}]}')" == '200' ]]
{
  printf 'api_key_id=%s\napi_key_name=%s\nscope=management,inference\n' "$KEY_ID" "$KEY_NAME"
  printf 'created_at=%s\nexpires_at=%s\nlifetime_seconds=%s\n' \
    "$created_at" "$expires_at" "$lifetime_seconds"
  stat -c 'operator_token=%n mode=%a owner=%U:%G links=%h' "$M7E1_OPERATOR_TOKEN"
  stat -c 'cluster_token=%n mode=%a owner=%U:%G links=%h' "$M7E1_CLUSTER_TOKEN"
} >"$EVIDENCE/credential-metadata.txt"

current_step='external_baseline'
announce 4 '维护窗口前的 Server、Docker、GPU、端口与资源基线'
snapshot before
[[ ! -s "$EVIDENCE/before-rust-container-ids.txt" ]]
[[ ! -s "$EVIDENCE/before-listeners.txt" ]]
[[ ! -s "$EVIDENCE/before-rust-processes.txt" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$M7E1_PYTHON_WORKER_ID")" == 'true' ]]
jq -e --argjson id "$M7E1_WORKER_ID" --arg name "$M7E1_WORKER_NAME" '
  [.items[] | select(.id == $id and .name == $name and
    .maintenance.enabled == true and .state == "maintenance")] |
  length == 1
' "$EVIDENCE/before-workers.json" >/dev/null
jq -e --arg name "$M7E1_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/before-models.json" >/dev/null
jq -e '[.items[] | select(.worker_id == 5)] | length == 0' \
  "$EVIDENCE/before-model-instances.json" >/dev/null
model_file_count="$(jq --arg path "$M7E1_MODEL_PATH" '
  [.items[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
  length
' "$EVIDENCE/before-model-files.json")"
[[ "$model_file_count" -le 1 ]]
if [[ "$model_file_count" -eq 1 ]]; then
  jq -e --arg path "$M7E1_MODEL_PATH" --argjson bytes "$M7E1_MODEL_BYTES" '
    [.items[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
    .[0].state == "ready" and .[0].download_progress == 100 and .[0].size == $bytes
  ' "$EVIDENCE/before-model-files.json" >/dev/null
fi

current_step='fixed_runtime_inputs'
announce 5 '固定 Server、Runner、模型、Worker、Python Worker 和双 GPU identity'
curl --silent --show-error --fail --max-time 10 http://127.0.0.1/version \
  >"$EVIDENCE/server-version.json"
jq -e '.version == "v2.2.3" and .git_commit == "1cfcbbf"' \
  "$EVIDENCE/server-version.json" >/dev/null
[[ "$(docker image inspect --format '{{.Id}}' "$M7E1_RUNNER_IMAGE")" == \
  "$M7E1_RUNNER_IMAGE_ID" ]]
[[ "$(readlink -f "$M7E1_MODEL_PATH")" == "$M7E1_MODEL_PATH" ]]
[[ "$(sha256sum "$M7E1_MODEL_MANIFEST" | awk '{print $1}')" == \
  "$M7E1_MODEL_MANIFEST_SHA" ]]
(cd "$M7E1_MODEL_PATH" && sha256sum --check --strict "$M7E1_MODEL_MANIFEST") \
  >"$EVIDENCE/model-manifest-check.txt"
[[ "$(sha256sum "$M7E1_MODEL_PATH/model.safetensors" | awk '{print $1}')" == \
  "$M7E1_MODEL_WEIGHT_SHA" ]]
[[ "$(find "$M7E1_MODEL_PATH" -type f -printf '%s\n' | \
  awk '{sum += $1} END {print sum+0}')" == "$M7E1_MODEL_BYTES" ]]
[[ "$(jq -r '.worker_uuid' "$M7E1_DATA_DIR/managed/worker-identity.json")" == \
  "$M7E1_WORKER_UUID" ]]
[[ "$(awk -F', *' '$1 == 0 {print $2}' "$EVIDENCE/before-gpus.csv")" == \
  "$M7E1_GPU0_UUID" ]]
[[ "$(awk -F', *' '$1 == 1 {print $2}' "$EVIDENCE/before-gpus.csv")" == \
  "$M7E1_GPU1_UUID" ]]
[[ "$(m7e1_gpu_memory_used 0)" == '0' && "$(m7e1_gpu_memory_used 1)" == '0' ]]
[[ "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" == '0' ]]
[[ "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" == '0' ]]
[[ "$(docker inspect --format '{{.Id}}' "$M7E1_PYTHON_WORKER_ID")" == \
  "$M7E1_PYTHON_WORKER_ID" ]]
{
  printf 'python_worker_id=%s\n' "$M7E1_PYTHON_WORKER_ID"
  docker inspect --format 'image={{.Image}} restart={{.HostConfig.RestartPolicy.Name}} network={{.HostConfig.NetworkMode}} running={{.State.Running}}' \
    "$M7E1_PYTHON_WORKER_ID"
  printf 'runner_image=%s\nrunner_image_id=%s\n' "$M7E1_RUNNER_IMAGE" "$M7E1_RUNNER_IMAGE_ID"
  printf 'worker_id=%s\nworker_uuid=%s\ngpu0_uuid=%s\ngpu1_uuid=%s\n' \
    "$M7E1_WORKER_ID" "$M7E1_WORKER_UUID" "$M7E1_GPU0_UUID" "$M7E1_GPU1_UUID"
  printf 'model_path=%s\nmodel_bytes=%s\nmodel_file_baseline_count=%s\n' \
    "$M7E1_MODEL_PATH" "$M7E1_MODEL_BYTES" "$model_file_count"
} >"$EVIDENCE/fixed-runtime-inputs.txt"

current_step='script_static_audit'
announce 6 '故障注入、同容器验收、回滚和秘密边界静态审计'
for required in \
  'm7e1_process_identity_matches_file' \
  "kill -KILL \"\$rust_pid\"" \
  'docker events' \
  'compare_runtime_identity' \
  'm7e1_direct_vllm_probe' \
  'm7e1_remove_exact_owned_container' \
  'm7e1_restore_python_worker_if_safe' \
  'pre_window_safety_checks'; do
  grep -Fq -- "$required" "$EXECUTOR" "$LIB"
done
grep -Fq -- 'timeout --foreground --signal=TERM --kill-after=180 1200' "$WATCHDOG"
grep -Fq -- 'm7e1_process_identity_matches_file' "$EXECUTOR"
grep -Fq -- 'm7e1_validate_owned_container' "$LIB"
grep -Fq -- 'm7e1_validate_owned_container_identity' "$LIB"
grep -Fq -- 'm7e1_maintenance_worker_has_new_heartbeat' "$EXECUTOR" "$LIB"
grep -Fq -- 'already_stopped=true' "$LIB"
if sed -n '/^wait_for_worker_ready() {/,/^}/p' "$EXECUTOR" |
  grep -Fq -- '.unreachable == false'; then
  printf 'STOP: maintenance startup must not trust the stale unreachable field\n' >&2
  exit 1
fi
maintenance_rearm_line="$(grep -nF "phase='prepare_worker_restart_safety'" "$EXECUTOR" |
  cut -d: -f1)"
intentional_crash_line="$(grep -nF "phase='intentional_worker_crash'" "$EXECUTOR" |
  cut -d: -f1)"
[[ "$maintenance_rearm_line" =~ ^[1-9][0-9]*$ ]]
[[ "$intentional_crash_line" =~ ^[1-9][0-9]*$ ]]
[[ "$maintenance_rearm_line" -lt "$intentional_crash_line" ]]
grep -Fq -- 'event gate_passed restart_safety_maintenance_enabled' "$EXECUTOR"
grep -Fq -- 'Config.Env' "$POST_AUDIT" && {
  printf 'STOP: post-audit must not capture container environment\n' >&2
  exit 1
}
for script in "$LIB" "$EXECUTOR" "$WATCHDOG" "$POST_AUDIT"; do
  if grep -Eq '(^|[[:space:]])pkill([[:space:]]|$)|kill[[:space:]]+--[[:space:]]+-' "$script"; then
    printf 'STOP: broad process termination is forbidden: %s\n' "$script" >&2
    exit 1
  fi
done
(
  heartbeat_file="$(mktemp)"
  trap 'unlink "$heartbeat_file"' EXIT
  jq -n '{
    id:5,
    state:"maintenance",
    unreachable:true,
    maintenance:{enabled:true,message:"formal maintenance"},
    gpu_count:2,
    heartbeat_time:"2026-10-30T07:24:39Z"
  }' >"$heartbeat_file"
  heartbeat_epoch="$(date -d '2026-10-30T07:24:39Z' +%s)"
  m7e1_maintenance_worker_has_new_heartbeat "$heartbeat_file" \
    "$((heartbeat_epoch - 1))"
  if m7e1_maintenance_worker_has_new_heartbeat "$heartbeat_file" \
    "$heartbeat_epoch"; then
    printf 'STOP: an unchanged maintenance heartbeat was accepted\n' >&2
    exit 1
  fi
) >"$EVIDENCE/maintenance-heartbeat-readiness-test.txt"
(
  test_root="$(mktemp -d)"
  trap 'find "$test_root" -xdev -depth -delete' EXIT
  test_container='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
  test_instance='42'
  test_spec='bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb'
  # The cleanup helper resolves these mock command names dynamically in the subshell.
  # shellcheck disable=SC2329
  m7e1_owned_container_ids() {
    printf '%s\n' "$test_container"
  }
  # shellcheck disable=SC2329
  docker() {
    case "$1" in
      inspect)
        [[ ! -e "$test_root/removed" ]] || return 1
        if [[ "$2" == '--format' ]]; then
          printf 'false\n'
          return 0
        fi
        jq -n \
          --arg id "$test_container" \
          --arg image "$M7E1_RUNNER_IMAGE_ID" \
          --arg uuid "$M7E1_WORKER_UUID" \
          --arg instance "$test_instance" \
          --arg hash "$test_spec" '[{
            Id:$id,Image:$image,Name:"/gpustack-rs-managed-42",
            Config:{Labels:{
              "io.gpustack.implementation":"rust",
              "io.gpustack.schema-version":"2",
              "io.gpustack.mode":"managed",
              "io.gpustack.owner-id":$uuid,
              "io.gpustack.component":"vllm",
              "io.gpustack.worker-uuid":$uuid,
              "io.gpustack.model-instance-id":$instance,
              "io.gpustack.spec-hash":$hash
            },Cmd:[],Entrypoint:[]},
            HostConfig:{RestartPolicy:{Name:"no"},NetworkMode:"host"},
            Mounts:[],State:{Running:false,Status:"exited",StartedAt:"",Pid:0}
          }]'
        ;;
      stop)
        return 91
        ;;
      rm)
        [[ "$2" == "$test_container" ]] || return 1
        : >"$test_root/removed"
        printf '%s\n' "$test_container"
        ;;
      *)
        return 1
        ;;
    esac
  }
  m7e1_remove_exact_owned_container \
    "$test_instance" "$test_container" "$test_spec" "$test_root"
  [[ -f "$test_root/removed" ]]
  grep -qx 'already_stopped=true' "$test_root/fallback-container-stop.txt"
) >"$EVIDENCE/stopped-container-cleanup-test.txt"
printf 'bash_n=PASS\nexact_pid_guard=PASS\nsame_container_contract=PASS\n' \
  >"$EVIDENCE/script-static-audit.txt"
{
  printf 'watchdog_timeout=1200\nunknown_process_policy=fail_closed\nconfig_env_capture=absent\n'
  printf 'restart_maintenance_rearm=PASS\nstopped_container_cleanup=PASS\n'
  printf 'maintenance_fresh_heartbeat=PASS\nmaintenance_stale_unreachable_ignored=PASS\n'
} >>"$EVIDENCE/script-static-audit.txt"

current_step='no_write_executor_injection'
announce 7 '错误 HEAD 的执行器无写故障注入'
set +o errexit
M7E1_LIB="$LIB" \
M7E1_LIB_SHA="$lib_sha" \
M7E1_EVIDENCE="$INJECTION_EVIDENCE" \
M7E1_PREFLIGHT="$EVIDENCE" \
M7E1_WATCHDOG="$WATCHDOG" \
M7E1_WATCHDOG_SHA="$watchdog_sha" \
M7E1_EXPECTED_HEAD='0000000000000000000000000000000000000000' \
M7E1_PRODUCT_COMMIT="$PRODUCT_COMMIT" \
M7E1_PREFLIGHT_MANIFEST_SHA='0000000000000000000000000000000000000000000000000000000000000000' \
M7E1_LOCK_SHA="$EXPECTED_LOCK_SHA" \
M7E1_ATTEMPT="$PREFLIGHT_ATTEMPT" \
bash "$EXECUTOR" >"$EVIDENCE/injection-console.txt" 2>&1
injection_exit=$?
set -o errexit
[[ "$injection_exit" -ne 0 ]]
[[ "$(sed -n 's/^status=//p' "$INJECTION_EVIDENCE/result.txt")" == 'STOP' ]]
grep -q '^reason=source_head_mismatch$' "$INJECTION_EVIDENCE/result.txt"
grep -q '^window_entered=0$' "$INJECTION_EVIDENCE/result.txt"
printf 'injection_exit=%s\nstatus=PASS\nreason=source_head_mismatch\nexternal_write=none\n' \
  "$injection_exit" >"$EVIDENCE/no-write-injection.txt"

current_step='post_snapshot_and_seal'
announce 8 '前后不变式、秘密扫描与证据封存'
snapshot after
for suffix in \
  services.txt docker-ids.txt rust-container-ids.txt python-worker.json \
  compute-processes.csv listeners.txt rust-processes.txt global-ids.json; do
  cmp "$EVIDENCE/before-$suffix" "$EVIDENCE/after-$suffix"
done
[[ "$(m7e1_gpu_memory_used 0)" == '0' && "$(m7e1_gpu_memory_used 1)" == '0' ]]
[[ "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" == '0' ]]
[[ "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" == '0' ]]
[[ -z "$(git status --porcelain)" ]]
(cd "$M7E1_MODEL_PATH" && sha256sum --check --strict "$M7E1_MODEL_MANIFEST") \
  >"$EVIDENCE/model-manifest-recheck.txt"

operator_token="$(tr -d '\r\n' <"$M7E1_OPERATOR_TOKEN")"
cluster_token="$(tr -d '\r\n' <"$M7E1_CLUSTER_TOKEN")"
if grep -RIlF -- "$operator_token" "$EVIDENCE" "$INJECTION_EVIDENCE" >/dev/null ||
  grep -RIlF -- "$cluster_token" "$EVIDENCE" "$INJECTION_EVIDENCE" >/dev/null; then
  unset operator_token cluster_token
  printf 'STOP: evidence contains a token value\n' >&2
  exit 1
fi
unset operator_token cluster_token
[[ "$(m7e1_secret_file_count "$EVIDENCE" "$INJECTION_EVIDENCE")" == '0' ]]
printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
printf 'status=PASS\nstage=M7-E.1-preflight\nattempt=%s\nexternal_mutation=none\nstatus_detail=ready_for_maintenance_authorization\n' \
  "$PREFLIGHT_ATTEMPT" >"$EVIDENCE/preflight-result.txt"
result_written=1
m7e1_seal_directory "$EVIDENCE"
printf 'evidence_dir=%s\nmanifest_file_sha256=%s\nm7e1_preflight=PASS\nstatus=ready_for_maintenance_authorization\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
