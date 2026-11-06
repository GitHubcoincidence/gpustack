#!/usr/bin/env bash
# Restores the named NVIDIA Docker runtime required by the existing Python Worker.
# This script reloads dockerd configuration; it never restarts or recreates Docker.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly DAEMON_CONFIG='/etc/docker/daemon.json'
readonly BACKUP_CONFIG='/etc/docker/daemon.json.m7f-20261106-pre-runtime.bak'
readonly EVIDENCE='/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m7f-runtime-repair-attempt-1'
readonly PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'
readonly SERVER_NAME='gpustack'
readonly EVIDENCE_OWNER='wujiaxing2'
readonly EVIDENCE_GROUP='wujiaxing2'

if [[ "$(id -u)" -ne 0 ]]; then
  printf 'STOP: run this script through sudo\n' >&2
  exit 77
fi
if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: repair evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi
if [[ -e "$BACKUP_CONFIG" || -L "$BACKUP_CONFIG" ]]; then
  printf 'STOP: reserved backup path already exists: %s\n' "$BACKUP_CONFIG" >&2
  exit 91
fi

install -d -o "$EVIDENCE_OWNER" -g "$EVIDENCE_GROUP" -m 0700 "$EVIDENCE"
exec 3>&1
exec > >(tee -a "$EVIDENCE/console.log") 2>&1

current_step='initialization'
server_id=''
python_started=0
config_changed=0
success=0
rollback_ok=1

docker_ready() {
  docker info >/dev/null 2>&1
}

wait_for_docker() {
  local attempt

  for attempt in $(seq 1 30); do
    docker_ready && return 0
    sleep 1
  done
  return 1
}

wait_for_nvidia_runtime() {
  local attempt

  for attempt in $(seq 1 30); do
    if docker info --format '{{json .Runtimes}}' 2>/dev/null |
      jq -e '.nvidia.path == "nvidia-container-runtime"' >/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

http_status() {
  curl --silent --show-error --max-time 5 --output /dev/null \
    --write-out '%{http_code}' "$1" 2>/dev/null || true
}

seal_evidence() {
  local status="$1"
  local reason="$2"

  printf 'status=%s\nstage=M7-F.3-runtime-repair\nreason=%s\nstep=%s\nrollback_ok=%s\n' \
    "$status" "$reason" "$current_step" "$rollback_ok" >"$EVIDENCE/result.txt"
  find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  chown -R "$EVIDENCE_OWNER:$EVIDENCE_GROUP" "$EVIDENCE"
  if grep -REIl \
    'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer' \
    "$EVIDENCE" >/dev/null; then
    printf 'secret_scan=FAIL\n' >"$EVIDENCE/secret-scan.txt"
    sed -i 's/^status=.*/status=FAIL/; s/^rollback_ok=.*/rollback_ok=0/' \
      "$EVIDENCE/result.txt"
    rollback_ok=0
  else
    printf 'secret_scan=PASS\n' >"$EVIDENCE/secret-scan.txt"
  fi
  chown "$EVIDENCE_OWNER:$EVIDENCE_GROUP" "$EVIDENCE/secret-scan.txt"
  chmod 0600 "$EVIDENCE/secret-scan.txt"
  find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
  chown "$EVIDENCE_OWNER:$EVIDENCE_GROUP" "$EVIDENCE/manifest.sha256"
  chmod 0600 "$EVIDENCE/manifest.sha256"
}

finish() {
  local code=$?

  trap - EXIT
  set +o errexit
  if [[ "$success" -ne 1 ]]; then
    current_step="rollback_after_${current_step}"
    if [[ "$python_started" -eq 1 ]] &&
      [[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID" 2>/dev/null)" == 'true' ]]; then
      docker stop --time 30 "$PYTHON_WORKER_ID" \
        >"$EVIDENCE/rollback-python-worker-stop.txt" 2>"$EVIDENCE/rollback-python-worker-stop.stderr" ||
        rollback_ok=0
    fi
    if [[ "$config_changed" -eq 1 && -f "$BACKUP_CONFIG" ]]; then
      cp --preserve=mode,ownership,timestamps "$BACKUP_CONFIG" "$DAEMON_CONFIG" || rollback_ok=0
      jq empty "$DAEMON_CONFIG" >/dev/null 2>&1 || rollback_ok=0
      systemctl reload docker >"$EVIDENCE/rollback-docker-reload.txt" \
        2>"$EVIDENCE/rollback-docker-reload.stderr" || rollback_ok=0
      wait_for_docker || rollback_ok=0
    fi
    if [[ -n "$server_id" ]] &&
      [[ "$(docker inspect --format '{{.State.Running}}' "$server_id" 2>/dev/null)" != 'true' ]]; then
      docker start "$server_id" >"$EVIDENCE/rollback-server-start.txt" \
        2>"$EVIDENCE/rollback-server-start.stderr" || rollback_ok=0
    fi
    seal_evidence STOP runtime_repair_failed
    printf 'm7f_runtime_repair=STOP\nfailed_step=%s\nrollback_ok=%s\n' \
      "$current_step" "$rollback_ok" >&3
    [[ "$code" -ne 0 ]] && exit "$code"
    exit 1
  fi

  seal_evidence PASS nvidia_runtime_and_python_worker_restored
  if [[ "$rollback_ok" -ne 1 ]]; then
    printf 'm7f_runtime_repair=FAIL\nreason=evidence_secret_scan_failed\n' >&3
    exit 1
  fi
  printf 'repair_evidence=%s\nmanifest_file_sha256=%s\nbackup_config=%s\nm7f_runtime_repair=PASS\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" \
    "$BACKUP_CONFIG" >&3
}
trap finish EXIT

printf '[1/5] verify exact broken baseline and reserve rollback material\n'
current_step='baseline'
[[ -f "$DAEMON_CONFIG" && ! -L "$DAEMON_CONFIG" ]]
jq empty "$DAEMON_CONFIG"
command -v nvidia-ctk >/dev/null
command -v nvidia-container-runtime >/dev/null
command -v docker >/dev/null
command -v systemctl >/dev/null
mapfile -t server_ids < <(docker ps -aq --no-trunc --filter "name=^/${SERVER_NAME}$")
[[ "${#server_ids[@]}" -eq 1 ]]
server_id="${server_ids[0]}"
[[ "$(docker inspect --format '{{.State.Running}}' "$server_id")" == 'true' ]]
[[ "$(http_status 'http://127.0.0.1/healthz')" == '200' ]]
[[ "$(docker inspect --format '{{.Id}}' "$PYTHON_WORKER_ID")" == "$PYTHON_WORKER_ID" ]]
[[ "$(docker inspect --format '{{.HostConfig.Runtime}}' "$PYTHON_WORKER_ID")" == 'nvidia' ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'false' ]]
docker info --format '{{json .Runtimes}}' >"$EVIDENCE/docker-runtimes-before.json"
jq -e 'has("nvidia") | not' "$EVIDENCE/docker-runtimes-before.json" >/dev/null
jq '{registry_mirrors:."registry-mirrors",runtimes}' "$DAEMON_CONFIG" \
  >"$EVIDENCE/daemon-config-before.json"
cp --preserve=mode,ownership,timestamps "$DAEMON_CONFIG" "$BACKUP_CONFIG"
printf 'backup_sha256=%s\n' "$(sha256sum "$BACKUP_CONFIG" | awk '{print $1}')" \
  >"$EVIDENCE/backup-identity.txt"
docker inspect --format \
  'id={{.Id}} runtime={{.HostConfig.Runtime}} status={{.State.Status}} exit={{.State.ExitCode}} error={{json .State.Error}}' \
  "$PYTHON_WORKER_ID" >"$EVIDENCE/python-worker-before.txt"
nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader,nounits >"$EVIDENCE/gpus-before.csv"

printf '[2/5] register NVIDIA runtime while preserving existing daemon settings\n'
current_step='configure_runtime'
nvidia-ctk runtime configure --runtime=docker --config="$DAEMON_CONFIG" \
  >"$EVIDENCE/nvidia-ctk.stdout" 2>"$EVIDENCE/nvidia-ctk.stderr"
config_changed=1
jq -e '.runtimes.nvidia.path == "nvidia-container-runtime"' "$DAEMON_CONFIG" >/dev/null
jq -e '."registry-mirrors" == [
  "https://dockerproxy.cn",
  "https://mirror.baidubce.com",
  "https://docker.m.daocloud.io"
]' "$DAEMON_CONFIG" >/dev/null
jq '{registry_mirrors:."registry-mirrors",runtimes}' "$DAEMON_CONFIG" \
  >"$EVIDENCE/daemon-config-after-write.json"

printf '[3/5] reload Docker configuration without restarting the daemon\n'
current_step='reload_docker'
systemctl reload docker >"$EVIDENCE/docker-reload.txt" 2>"$EVIDENCE/docker-reload.stderr"
wait_for_docker
wait_for_nvidia_runtime
docker info --format '{{json .Runtimes}}' >"$EVIDENCE/docker-runtimes-after.json"
jq -e '.nvidia.path == "nvidia-container-runtime"' \
  "$EVIDENCE/docker-runtimes-after.json" >/dev/null
[[ "$(docker inspect --format '{{.Id}}' "$server_id")" == "$server_id" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$server_id")" == 'true' ]]
[[ "$(http_status 'http://127.0.0.1/healthz')" == '200' ]]

printf '[4/5] start the exact existing Python Worker and wait for health\n'
current_step='start_python_worker'
docker start "$PYTHON_WORKER_ID" >"$EVIDENCE/python-worker-start.txt" \
  2>"$EVIDENCE/python-worker-start.stderr"
python_started=1
worker_ready=0
for _ in $(seq 1 180); do
  if [[ "$(http_status 'http://127.0.0.1:10150/healthz')" == '200' ]] &&
    [[ "$(http_status 'http://127.0.0.1:10151/metrics')" == '200' ]]; then
    worker_ready=1
    break
  fi
  sleep 1
done
[[ "$worker_ready" -eq 1 ]]

printf '[5/5] verify service identity and capture the repaired baseline\n'
current_step='verify_repaired_baseline'
[[ "$(docker inspect --format '{{.Id}}' "$PYTHON_WORKER_ID")" == "$PYTHON_WORKER_ID" ]]
[[ "$(docker inspect --format '{{.State.Running}}' "$PYTHON_WORKER_ID")" == 'true' ]]
[[ "$(docker inspect --format '{{.HostConfig.Runtime}}' "$PYTHON_WORKER_ID")" == 'nvidia' ]]
[[ "$(http_status 'http://127.0.0.1/healthz')" == '200' ]]
[[ "$(http_status 'http://127.0.0.1:10150/healthz')" == '200' ]]
[[ "$(http_status 'http://127.0.0.1:10151/metrics')" == '200' ]]
docker inspect --format \
  'id={{.Id}} runtime={{.HostConfig.Runtime}} status={{.State.Status}} restart={{.HostConfig.RestartPolicy.Name}}' \
  "$PYTHON_WORKER_ID" >"$EVIDENCE/python-worker-after.txt"
printf 'server=200\npython_worker=200\npython_metrics=200\n' >"$EVIDENCE/services-after.txt"
nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader,nounits >"$EVIDENCE/gpus-after.csv"
nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
  --format=csv,noheader,nounits >"$EVIDENCE/gpu-processes-after.csv" || true
success=1
