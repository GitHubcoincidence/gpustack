#!/usr/bin/env bash
# Restores the exact existing Python Worker after NVIDIA runtime registration.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE='/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m7f-python-worker-restore-attempt-1'
readonly PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'
readonly EXPECTED_HEAD="${M7F_EXPECTED_HEAD:?M7F_EXPECTED_HEAD is required}"

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: restore evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi

install -d -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

current_step='initialization'
server_id=''
started=0
success=0
cleanup_ok=1

http_status() {
  curl --silent --show-error --max-time 5 --output /dev/null \
    --write-out '%{http_code}' "$1" 2>/dev/null || true
}

finish() {
  local code=$?
  local status reason

  trap - EXIT
  set +o errexit
  if [[ "$success" -ne 1 && "$started" -eq 1 ]] &&
    [[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID" 2>/dev/null)" == 'true' ]]; then
    docker stop --time 30 "$PYTHON_WORKER_ID" \
      >"$EVIDENCE/rollback-stop.txt" 2>"$EVIDENCE/rollback-stop.stderr" || cleanup_ok=0
  fi
  if [[ "$success" -eq 1 ]]; then
    status='PASS'
    reason='python_worker_restored'
  else
    status='STOP'
    reason='python_worker_restore_failed'
  fi
  printf 'status=%s\nstage=M7-F.3-python-worker-restore\nreason=%s\nstep=%s\ncleanup_ok=%s\n' \
    "$status" "$reason" "$current_step" "$cleanup_ok" >"$EVIDENCE/result.txt"
  find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  if grep -REIl \
    'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer' \
    "$EVIDENCE" >/dev/null; then
    printf 'secret_scan=FAIL\n' >"$EVIDENCE/secret-scan.txt"
    sed -i 's/^status=.*/status=FAIL/; s/^cleanup_ok=.*/cleanup_ok=0/' \
      "$EVIDENCE/result.txt"
    cleanup_ok=0
  else
    printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
  fi
  chmod 0600 "$EVIDENCE/secret-scan.txt"
  find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
  chmod 0600 "$EVIDENCE/manifest.sha256"
  printf 'restore_evidence=%s\nmanifest_file_sha256=%s\npython_worker_restore=%s\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$status" >&3
  if [[ "$success" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    exit 0
  fi
  [[ "$code" -ne 0 ]] && exit "$code"
  exit 1
}
trap finish EXIT

printf '[1/4] verify repaired Docker runtime and exact baseline\n'
current_step='baseline'
cd "$REPO"
[[ "$(git branch --show-current)" == 'codex/rust-worker' ]]
[[ "$(git rev-parse HEAD)" == "$EXPECTED_HEAD" ]]
[[ "$(git rev-parse origin/codex/rust-worker)" == "$EXPECTED_HEAD" ]]
[[ -z "$(git status --porcelain)" ]]
docker info --format '{{json .Runtimes}}' >"$EVIDENCE/docker-runtimes.json"
jq -e '.nvidia.path == "nvidia-container-runtime"' \
  "$EVIDENCE/docker-runtimes.json" >/dev/null
mapfile -t server_ids < <(docker ps -aq --no-trunc --filter 'name=^/gpustack$')
[[ "${#server_ids[@]}" -eq 1 ]]
server_id="${server_ids[0]}"
[[ "$(docker inspect --format '{{.State.Running}}' "$server_id")" == 'true' ]]
[[ "$(http_status 'http://127.0.0.1/healthz')" == '200' ]]
[[ "$(docker inspect --format '{{.Id}}' "$PYTHON_WORKER_ID")" == "$PYTHON_WORKER_ID" ]]
[[ "$(docker inspect --format '{{.HostConfig.Runtime}}' "$PYTHON_WORKER_ID")" == 'nvidia' ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'false' ]]
[[ -z "$(docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed)" ]]
[[ -z "$(pgrep -af '[g]pustack-rs' || true)" ]]
docker inspect --format \
  'id={{.Id}} runtime={{.HostConfig.Runtime}} state={{.State.Status}} error={{json .State.Error}}' \
  "$PYTHON_WORKER_ID" >"$EVIDENCE/python-before.txt"
nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader,nounits >"$EVIDENCE/gpus-before.csv"

printf '[2/4] start the exact existing Python Worker\n'
current_step='start_python_worker'
docker start "$PYTHON_WORKER_ID" >"$EVIDENCE/docker-start.txt" \
  2>"$EVIDENCE/docker-start.stderr"
started=1

printf '[3/4] wait for Worker API and metrics\n'
current_step='wait_health'
ready=0
for _ in $(seq 1 180); do
  if [[ "$(http_status 'http://127.0.0.1:10150/healthz')" == '200' ]] &&
    [[ "$(http_status 'http://127.0.0.1:10151/metrics')" == '200' ]]; then
    ready=1
    break
  fi
  sleep 1
done
[[ "$ready" -eq 1 ]]

printf '[4/4] verify the repaired service baseline\n'
current_step='verify_baseline'
[[ "$(docker inspect --format '{{.Id}}' "$PYTHON_WORKER_ID")" == "$PYTHON_WORKER_ID" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'true' ]]
[[ "$(docker inspect --format '{{.HostConfig.Runtime}}' "$PYTHON_WORKER_ID")" == 'nvidia' ]]
[[ "$(docker inspect --format '{{.Id}}' "$server_id")" == "$server_id" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$server_id")" == 'true' ]]
[[ "$(http_status 'http://127.0.0.1/healthz')" == '200' ]]
[[ "$(http_status 'http://127.0.0.1:10150/healthz')" == '200' ]]
[[ "$(http_status 'http://127.0.0.1:10151/metrics')" == '200' ]]
docker inspect --format \
  'id={{.Id}} runtime={{.HostConfig.Runtime}} state={{.State.Status}} restart={{.HostConfig.RestartPolicy.Name}}' \
  "$PYTHON_WORKER_ID" >"$EVIDENCE/python-after.txt"
printf 'server=200\npython_worker=200\npython_metrics=200\n' >"$EVIDENCE/services-after.txt"
nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader,nounits >"$EVIDENCE/gpus-after.csv"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
  --format=csv,noheader,nounits >"$EVIDENCE/gpu-processes-after.csv" || true
success=1
