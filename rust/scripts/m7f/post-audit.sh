#!/usr/bin/env bash
# Independent recovery audit for the M7-F dual-GPU formal window.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE="${M7F_POST_AUDIT_EVIDENCE:?M7F_POST_AUDIT_EVIDENCE is required}"
readonly PREFLIGHT="${M7F_PREFLIGHT:?M7F_PREFLIGHT is required}"
readonly FORMAL="${M7F_FORMAL_EVIDENCE:?M7F_FORMAL_EVIDENCE is required}"
readonly WATCHDOG="${M7F_WATCHDOG_EVIDENCE:?M7F_WATCHDOG_EVIDENCE is required}"
readonly EXPECTED_HEAD="${M7F_EXPECTED_HEAD:?M7F_EXPECTED_HEAD is required}"
readonly EXPECTED_PREFLIGHT_MANIFEST_SHA="${M7F_PREFLIGHT_MANIFEST_SHA:?M7F_PREFLIGHT_MANIFEST_SHA is required}"
readonly EXPECTED_FORMAL_MANIFEST_SHA="${M7F_FORMAL_MANIFEST_SHA:?M7F_FORMAL_MANIFEST_SHA is required}"
readonly EXPECTED_WATCHDOG_MANIFEST_SHA="${M7F_WATCHDOG_MANIFEST_SHA:?M7F_WATCHDOG_MANIFEST_SHA is required}"
readonly ATTEMPT="${M7F_ATTEMPT:?M7F_ATTEMPT is required}"
readonly BINARY="$REPO/rust/target/debug/gpustack-rs"
readonly CONTEXT='m7f-operator'
readonly TARGET_MODEL='m7f-qwen25-05b-tp2'
readonly PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'
readonly GPU0_UUID='GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3'
readonly GPU1_UUID='GPU-b1f89e65-e9c4-26f5-a306-8a520df07708'

if [[ ! "$ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M7F_ATTEMPT must be a positive integer\n' >&2
  exit 89
fi
if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: post-audit evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi

install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

announce() {
  printf '\n[%s/4] %s\n' "$1" "$2"
}

verify_manifest() {
  local directory="$1"
  local expected_sha="$2"
  local label="$3"

  [[ -d "$directory" && ! -L "$directory" ]]
  [[ -f "$directory/manifest.sha256" && ! -L "$directory/manifest.sha256" ]]
  [[ "$(sha256sum "$directory/manifest.sha256" | awk '{print $1}')" == "$expected_sha" ]]
  (cd "$directory" && sha256sum --check manifest.sha256) \
    >"$EVIDENCE/${label}-manifest-check.txt"
}

gpu_compute_count() {
  local uuid="$1"
  nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader 2>/dev/null |
    awk -v uuid="$uuid" '$1 == uuid {count++} END {print count+0}'
}

gpu_memory_used() {
  local index="$1"
  nvidia-smi --query-gpu=index,memory.used --format=csv,noheader,nounits |
    awk -F', *' -v index="$index" '$1 == index {print $2}'
}

announce 1 '验证三段证据链和执行结果'
verify_manifest "$PREFLIGHT" "$EXPECTED_PREFLIGHT_MANIFEST_SHA" preflight
verify_manifest "$FORMAL" "$EXPECTED_FORMAL_MANIFEST_SHA" formal
verify_manifest "$WATCHDOG" "$EXPECTED_WATCHDOG_MANIFEST_SHA" watchdog
[[ "$(sed -n 's/^status=//p' "$PREFLIGHT/preflight-result.txt")" == 'PASS' ]]
[[ "$(sed -n 's/^attempt=//p' "$PREFLIGHT/preflight-result.txt")" == "$ATTEMPT" ]]
[[ "$(sed -n 's/^attempt=//p' "$FORMAL/result.txt")" == "$ATTEMPT" ]]
[[ "$(sed -n 's/^status=//p' "$FORMAL/result.txt")" == 'PASS' ]]
[[ "$(sed -n 's/^cleanup_ok=//p' "$FORMAL/result.txt")" == '1' ]]
[[ "$(sed -n 's/^attempt=//p' "$WATCHDOG/result.txt")" == "$ATTEMPT" ]]
[[ "$(sed -n 's/^status=//p' "$WATCHDOG/result.txt")" == 'PASS' ]]
if [[ -f "$WATCHDOG/recovery-result.txt" ]]; then
  [[ "$(sed -n 's/^recovery_ok=//p' "$WATCHDOG/recovery-result.txt")" == '1' ]]
fi
{
  printf 'preflight_status=%s\n' "$(sed -n 's/^status=//p' "$PREFLIGHT/preflight-result.txt")"
  printf 'formal_status=%s\n' "$(sed -n 's/^status=//p' "$FORMAL/result.txt")"
  printf 'watchdog_status=%s\n' "$(sed -n 's/^status=//p' "$WATCHDOG/result.txt")"
  printf 'cleanup_ok=1\nwatchdog_status=PASS\n'
} >"$EVIDENCE/evidence-chain.txt"

announce 2 '验证源码、服务和Python Worker恢复'
cd "$REPO"
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
[[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1/healthz)" == '200' ]]
[[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10150/healthz)" == '200' ]]
[[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10151/metrics)" == '200' ]]
[[ "$(docker inspect --format '{{.Id}}' "$PYTHON_WORKER_ID")" == "$PYTHON_WORKER_ID" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'true' ]]
docker inspect "$PYTHON_WORKER_ID" | jq '.[0] | {
  Id,Image,Name,restart_policy:.HostConfig.RestartPolicy,
  network_mode:.HostConfig.NetworkMode,running:.State.Running
}' >"$EVIDENCE/python-worker.json"
printf 'server=200\npython_worker=200\npython_metrics=200\n' >"$EVIDENCE/services.txt"

announce 3 '验证测试资源、端口和双GPU归零'
[[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]]
[[ "$(docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed | wc -l)" == '0' ]]
[[ "$(ss -H -lnt 2>/dev/null | awk '
  {split($4,a,":"); p=a[length(a)]+0;
   if (p == 20150 || p == 20151 || (p >= 40000 && p <= 40063)) n++}
  END {print n+0}')" == '0' ]]
"$BINARY" --context "$CONTEXT" --output json model list \
  >"$EVIDENCE/models.json"
jq -e --arg name "$TARGET_MODEL" '[.[] | select(.name == $name)] | length == 0' \
  "$EVIDENCE/models.json" >/dev/null
nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader,nounits >"$EVIDENCE/gpus.csv"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
  --format=csv,noheader,nounits >"$EVIDENCE/gpu-processes.csv" || true
[[ "$(awk -F', *' '$1 == 0 {print $2}' "$EVIDENCE/gpus.csv")" == "$GPU0_UUID" ]]
[[ "$(awk -F', *' '$1 == 1 {print $2}' "$EVIDENCE/gpus.csv")" == "$GPU1_UUID" ]]
[[ "$(gpu_compute_count "$GPU0_UUID")" == '0' ]]
[[ "$(gpu_compute_count "$GPU1_UUID")" == '0' ]]
[[ "$(gpu_memory_used 0)" == '0' ]]
[[ "$(gpu_memory_used 1)" == '0' ]]

announce 4 '秘密扫描和独立证据封存'
if grep -REIl 'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]' \
  "$EVIDENCE" >/dev/null; then
  printf 'STOP: post-audit evidence contains a secret-shaped value\n' >&2
  exit 1
fi
printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
printf 'status=PASS\nstage=M7-F.5-post-audit\nattempt=%s\nrecovery_baseline=restored\n' \
  "$ATTEMPT" >"$EVIDENCE/result.txt"
find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
  sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
chmod 0600 "$EVIDENCE/manifest.sha256"
printf 'post_audit_evidence=%s\npost_audit_manifest_sha256=%s\nm7f_post_audit=PASS\n' \
  "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" >&3
