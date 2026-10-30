#!/usr/bin/env bash
# Independent recovery audit after an M7-E.1 formal attempt.
# shellcheck disable=SC1090,SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly LIB="${M7E1_LIB:?M7E1_LIB is required}"
readonly EXPECTED_LIB_SHA="${M7E1_LIB_SHA:?M7E1_LIB_SHA is required}"
if [[ ! -f "$LIB" ]] ||
  [[ "$(sha256sum "$LIB" | awk '{print $1}')" != "$EXPECTED_LIB_SHA" ]]; then
  printf 'STOP: shared library identity mismatch\n' >&2
  exit 88
fi
source "$LIB"

readonly EVIDENCE="${M7E1_POST_AUDIT_EVIDENCE:?M7E1_POST_AUDIT_EVIDENCE is required}"
readonly PREFLIGHT="${M7E1_PREFLIGHT:?M7E1_PREFLIGHT is required}"
readonly EXPECTED_PREFLIGHT_MANIFEST_SHA="${M7E1_PREFLIGHT_MANIFEST_SHA:?M7E1_PREFLIGHT_MANIFEST_SHA is required}"
readonly FORMAL_EVIDENCE="${M7E1_EVIDENCE:?M7E1_EVIDENCE is required}"
readonly WATCHDOG_EVIDENCE="${M7E1_WATCHDOG_EVIDENCE:?M7E1_WATCHDOG_EVIDENCE is required}"
readonly EXPECTED_HEAD="${M7E1_EXPECTED_HEAD:?M7E1_EXPECTED_HEAD is required}"
readonly FORMAL_ATTEMPT="${M7E1_ATTEMPT:?M7E1_ATTEMPT is required}"
readonly PRODUCT_COMMIT='0d861a4c9d5502341cbc78eefc60e30bb32c24fd'
readonly EXPECTED_LOCK_SHA='90f1aeaa22b4f06dc77e48497f2fe3394ab07ab55441c8ddb3124a67dca07010'

if [[ ! "$FORMAL_ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7E1_ATTEMPT must be a positive integer\n' >&2
  exit 89
fi
if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: post-audit evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi
for directory in "$PREFLIGHT" "$FORMAL_EVIDENCE" "$WATCHDOG_EVIDENCE"; do
  [[ -d "$directory" && ! -L "$directory" ]] || {
    printf 'STOP: required evidence directory is unavailable: %s\n' "$directory" >&2
    exit 91
  }
done

install -d -m 0700 "$EVIDENCE"
install -m 0600 "$0" "$EVIDENCE/post-audit.sh"
install -m 0600 "$LIB" "$EVIDENCE/lib.sh"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

current_step='initialization'
result_written=0

finish() {
  local code=$?
  trap - EXIT
  if [[ -d "$EVIDENCE" && "$result_written" -eq 0 ]]; then
    printf 'status=FAIL\nstage=M7-E.1-post-audit\nattempt=%s\nfailed_step=%s\nexit_code=%s\n' \
      "$FORMAL_ATTEMPT" "$current_step" "$code" >"$EVIDENCE/result.txt"
    chmod 0600 "$EVIDENCE/result.txt"
  fi
  exit "$code"
}
trap finish EXIT

announce() {
  printf '\n[%s/6] %s\n' "$1" "$2"
}

current_step='evidence_chain'
announce 1 '验证 preflight、formal 与 watchdog 证据链'
[[ "$(sha256sum "$PREFLIGHT/manifest.sha256" | awk '{print $1}')" == \
  "$EXPECTED_PREFLIGHT_MANIFEST_SHA" ]]
(cd "$PREFLIGHT" && sha256sum --check manifest.sha256) \
  >"$EVIDENCE/preflight-manifest-check.txt"
[[ "$(sed -n 's/^status=//p' "$PREFLIGHT/preflight-result.txt")" == 'PASS' ]]
formal_status='missing'
formal_cleanup='unknown'
if [[ -f "$FORMAL_EVIDENCE/manifest.sha256" ]]; then
  (cd "$FORMAL_EVIDENCE" && sha256sum --check manifest.sha256) \
    >"$EVIDENCE/formal-manifest-check.txt"
  formal_status="$(sed -n 's/^status=//p' "$FORMAL_EVIDENCE/result.txt")"
  formal_cleanup="$(sed -n 's/^cleanup_ok=//p' "$FORMAL_EVIDENCE/result.txt")"
else
  printf 'formal_manifest=missing\n' >"$EVIDENCE/formal-manifest-check.txt"
fi
[[ -f "$WATCHDOG_EVIDENCE/manifest.sha256" ]]
(cd "$WATCHDOG_EVIDENCE" && sha256sum --check manifest.sha256) \
  >"$EVIDENCE/watchdog-manifest-check.txt"
watchdog_status="$(sed -n 's/^status=//p' "$WATCHDOG_EVIDENCE/result.txt")"
watchdog_recovery="$(sed -n 's/^recovery_ok=//p' "$WATCHDOG_EVIDENCE/result.txt")"
[[ "$watchdog_recovery" == '1' ]]
{
  printf 'formal_status=%s\nformal_cleanup=%s\n' "$formal_status" "$formal_cleanup"
  printf 'watchdog_status=%s\nwatchdog_recovery=%s\n' \
    "$watchdog_status" "$watchdog_recovery"
  printf 'preflight_manifest_file_sha256=%s\n' "$EXPECTED_PREFLIGHT_MANIFEST_SHA"
  printf 'formal_manifest_file_sha256=%s\n' \
    "$(if [[ -f "$FORMAL_EVIDENCE/manifest.sha256" ]]; then
      sha256sum "$FORMAL_EVIDENCE/manifest.sha256" | awk '{print $1}'
    else
      printf 'missing'
    fi)"
  printf 'watchdog_manifest_file_sha256=%s\n' \
    "$(sha256sum "$WATCHDOG_EVIDENCE/manifest.sha256" | awk '{print $1}')"
} >"$EVIDENCE/evidence-chain.txt"

current_step='source_identity'
announce 2 '复核源码、工具链和固定模型未漂移'
cd "$M7E1_REPO"
source /home/wujiaxing2/.cargo/env
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
git merge-base --is-ancestor "$PRODUCT_COMMIT" HEAD
git diff --quiet "$PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates
[[ "$(sha256sum rust/Cargo.lock | awk '{print $1}')" == "$EXPECTED_LOCK_SHA" ]]
[[ "$(sha256sum "$M7E1_MODEL_MANIFEST" | awk '{print $1}')" == \
  "$M7E1_MODEL_MANIFEST_SHA" ]]
(cd "$M7E1_MODEL_PATH" && sha256sum --check --strict "$M7E1_MODEL_MANIFEST") \
  >"$EVIDENCE/model-manifest-check.txt"
[[ "$(find "$M7E1_MODEL_PATH" -type f -printf '%s\n' | \
  awk '{sum += $1} END {print sum+0}')" == "$M7E1_MODEL_BYTES" ]]
{
  printf 'head=%s\norigin_head=%s\nproduct_commit=%s\n' \
    "$EXPECTED_HEAD" "$(git rev-parse origin/codex/rust-worker)" "$PRODUCT_COMMIT"
  printf 'cargo_lock_sha256=%s\nlib_sha256=%s\n' "$EXPECTED_LOCK_SHA" "$EXPECTED_LIB_SHA"
  printf 'post_audit_sha256=%s\n' "$(sha256sum "$0" | awk '{print $1}')"
} >"$EVIDENCE/source-identity.txt"

current_step='services_and_worker'
announce 3 '复核 Server、Python Worker 与 maintenance 基线'
{
  printf 'server=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' http://127.0.0.1/healthz)"
  printf 'python_worker=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' http://127.0.0.1:10150/healthz)"
  printf 'python_metrics=%s\n' "$(curl --silent --max-time 10 --output /dev/null \
    --write-out '%{http_code}' http://127.0.0.1:10151/metrics)"
} >"$EVIDENCE/services.txt"
grep -qx 'server=200' "$EVIDENCE/services.txt"
grep -qx 'python_worker=200' "$EVIDENCE/services.txt"
grep -qx 'python_metrics=200' "$EVIDENCE/services.txt"
[[ "$(docker inspect --format '{{.State.Running}}' "$M7E1_PYTHON_WORKER_ID")" == 'true' ]]
m7e1_capture_python_worker "$EVIDENCE/python-worker.json"
cmp "$PREFLIGHT/before-python-worker.json" "$EVIDENCE/python-worker.json"
"$M7E1_BINARY" --context "$M7E1_OPERATOR_CONTEXT" --output json \
  worker get "$M7E1_WORKER_ID" >"$EVIDENCE/worker.json"
jq -e '.id == 5 and .name == "rust-m3d-r760xa" and .maintenance.enabled == true and
  .state == "maintenance" and .unreachable == false' "$EVIDENCE/worker.json" >/dev/null

current_step='server_and_docker_resources'
announce 4 '复核测试资源、Rust 容器、进程和端口全部清空'
m7e1_global_snapshot "$EVIDENCE" after
cmp "$PREFLIGHT/before-global-ids.json" "$EVIDENCE/after-global-ids.json"
jq -e --arg name "$M7E1_TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/after-models.json" >/dev/null
jq -e '[.items[] | select(.worker_id == 5)] | length == 0' \
  "$EVIDENCE/after-model-instances.json" >/dev/null
m7e1_capture_docker_ids >"$EVIDENCE/docker-ids.txt"
cmp "$PREFLIGHT/before-docker-ids.txt" "$EVIDENCE/docker-ids.txt"
docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed | sort >"$EVIDENCE/rust-container-ids.txt"
[[ ! -s "$EVIDENCE/rust-container-ids.txt" ]]
pgrep -af '[g]pustack-rs' >"$EVIDENCE/rust-processes.txt" || true
[[ ! -s "$EVIDENCE/rust-processes.txt" ]]
: >"$EVIDENCE/listeners.txt"
for port in 20150 20151 $(seq 40000 40063); do
  ss -H -lntp "sport = :$port" >>"$EVIDENCE/listeners.txt"
done
[[ ! -s "$EVIDENCE/listeners.txt" ]]
instance_id=''
if [[ -f "$FORMAL_EVIDENCE/result.txt" ]]; then
  instance_id="$(sed -n 's/^model_instance_id=//p' "$FORMAL_EVIDENCE/result.txt")"
  [[ "$instance_id" != 'none' ]] || instance_id=''
fi
if [[ "$instance_id" =~ ^[1-9][0-9]*$ ]]; then
  [[ ! -e "$M7E1_DATA_DIR/managed/manifests/$instance_id.json" ]]
  [[ ! -e "$M7E1_DATA_DIR/managed/cache/$instance_id" ]]
  [[ "$(find "$M7E1_LOG_DIR/serve" -maxdepth 1 -type f \
    -name "${instance_id}.*.log" 2>/dev/null | wc -l)" == '0' ]]
fi

current_step='gpu_and_secret_isolation'
announce 5 '复核 GPU0/GPU1 空闲且证据不含秘密'
m7e1_capture_gpu "$EVIDENCE" after
[[ "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" == '0' ]]
[[ "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" == '0' ]]
[[ "$(m7e1_gpu_memory_used 0)" == '0' ]]
[[ "$(m7e1_gpu_memory_used 1)" == '0' ]]
operator_token="$(tr -d '\r\n' <"$M7E1_OPERATOR_TOKEN")"
cluster_token="$(tr -d '\r\n' <"$M7E1_CLUSTER_TOKEN")"
if grep -RIlF -- "$operator_token" \
  "$PREFLIGHT" "$FORMAL_EVIDENCE" "$WATCHDOG_EVIDENCE" "$EVIDENCE" >/dev/null ||
  grep -RIlF -- "$cluster_token" \
    "$PREFLIGHT" "$FORMAL_EVIDENCE" "$WATCHDOG_EVIDENCE" "$EVIDENCE" >/dev/null; then
  unset operator_token cluster_token
  printf 'STOP: evidence contains a token value\n' >&2
  exit 1
fi
unset operator_token cluster_token
[[ "$(m7e1_secret_file_count \
  "$PREFLIGHT" "$FORMAL_EVIDENCE" "$WATCHDOG_EVIDENCE" "$EVIDENCE")" == '0' ]]
printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"

current_step='seal'
announce 6 '封存独立恢复审计'
printf 'status=PASS\nstage=M7-E.1-post-audit\nattempt=%s\nrecovery_baseline=restored\nformal_status=%s\nwatchdog_status=%s\n' \
  "$FORMAL_ATTEMPT" "$formal_status" "$watchdog_status" >"$EVIDENCE/result.txt"
result_written=1
m7e1_seal_directory "$EVIDENCE"
printf 'post_audit_evidence=%s\npost_audit_manifest_sha256=%s\nm7e1_recovery_audit=PASS\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
