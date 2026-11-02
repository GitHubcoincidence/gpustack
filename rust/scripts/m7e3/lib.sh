#!/usr/bin/env bash
# Shared identity, proxy, and recovery helpers for the M7-E.3 formal toolchain.
# shellcheck disable=SC2034 # Constants form the interface consumed by sourcing scripts.

readonly M7E3_REPO='/home/wujiaxing2/workspaces/gpustack'
readonly M7E3_BINARY="$M7E3_REPO/rust/target/debug/gpustack-rs"
readonly M7E3_DATA_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/1'
readonly M7E3_LOG_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d-logs'
readonly M7E3_MODEL_ROOT='/home/wujiaxing2/flowscale-system-validation/models'
readonly M7E3_MODEL_PATH="$M7E3_MODEL_ROOT/Qwen2.5-0.5B-Instruct"
readonly M7E3_MODEL_MANIFEST="$M7E3_REPO/rust/fixtures/m5e/qwen2.5-0.5b-instruct-7ae557604adf.sha256"
readonly M7E3_MODEL_MANIFEST_SHA='645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6'
readonly M7E3_MODEL_WEIGHT_SHA='fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe'
readonly M7E3_MODEL_BYTES='999604126'
readonly M7E3_TARGET_MODEL='m7e3-qwen25-05b-gpu0'
readonly M7E3_RUNNER_IMAGE='quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M7E3_RUNNER_IMAGE_ID='sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M7E3_WORKER_ID='5'
readonly M7E3_WORKER_NAME='rust-m3d-r760xa'
readonly M7E3_WORKER_UUID='7c4e07a3-6b97-4577-9ff0-1fc6c2319596'
readonly M7E3_WORKER_IP='10.213.80.90'
readonly M7E3_GPU0_UUID='GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3'
readonly M7E3_GPU1_UUID='GPU-b1f89e65-e9c4-26f5-a306-8a520df07708'
readonly M7E3_OPERATOR_CONTEXT='m7e3-operator'
readonly M7E3_OPERATOR_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m7e3-operator-token'
readonly M7E3_CLUSTER_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token'
readonly M7E3_WORKER_TOKEN="$M7E3_DATA_DIR/managed/worker-token"
readonly M7E3_PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'
readonly M7E3_SERVER_PROXY_PORT='18080'
readonly M7E3_SERVER_PROXY_URL="http://127.0.0.1:$M7E3_SERVER_PROXY_PORT/"
readonly M7E3_PROXY_STATE_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/m7'
readonly M7E3_DOCKER_PROXY_SOCKET="$M7E3_PROXY_STATE_DIR/docker-proxy.sock"
readonly M7E3_SERVER_OUTAGE_SECONDS='70'
readonly M7E3_DOCKER_OUTAGE_SECONDS='20'

m7e3_timestamp() {
  date --iso-8601=ns
}

m7e3_server_timestamp_epoch() {
  local timestamp="$1"

  # GPUStack v2.2.3 may omit the suffix from UTC API timestamps. Normalize only
  # that exact shape so host timezone settings cannot change formal evidence.
  if [[ "$timestamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]+)?$ ]]; then
    timestamp="${timestamp}Z"
  elif [[ ! "$timestamp" =~ ^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}([.][0-9]+)?(Z|[+-][0-9]{2}:[0-9]{2})$ ]]; then
    return 1
  fi

  date -u -d "$timestamp" +%s 2>/dev/null
}

m7e3_auth_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$M7E3_OPERATOR_TOKEN"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

m7e3_cluster_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$M7E3_CLUSTER_TOKEN"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

m7e3_worker_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$M7E3_WORKER_TOKEN"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

m7e3_filtered_get() {
  local principal="$1"
  local url="$2"
  local output="$3"
  local filter="$4"
  local raw status
  raw="$(mktemp "${output}.raw.XXXXXX")"
  if [[ "$principal" == 'operator' ]]; then
    if ! status="$(m7e3_auth_get "$url" "$raw")"; then
      unlink "$raw"
      return 1
    fi
  elif [[ "$principal" == 'cluster' ]]; then
    if ! status="$(m7e3_cluster_get "$url" "$raw")"; then
      unlink "$raw"
      return 1
    fi
  elif [[ "$principal" == 'worker' ]]; then
    if ! status="$(m7e3_worker_get "$url" "$raw")"; then
      unlink "$raw"
      return 1
    fi
  else
    unlink "$raw"
    return 1
  fi
  if ! jq "$filter" "$raw" >"$output"; then
    unlink "$raw"
    if [[ -e "$output" || -L "$output" ]]; then
      unlink "$output"
    fi
    return 1
  fi
  unlink "$raw"
  printf '%s' "$status"
}

m7e3_model_instance_get() {
  local url="$1"
  local output="$2"
  m7e3_filtered_get operator "$url" "$output" '
    if has("items") then
      {items:[.items[] | {id,model_id,worker_id,state,port,restart_count,last_restart_time}]}
    elif has("id") then
      {id,model_id,worker_id,state,port,restart_count,last_restart_time}
    else
      {detail}
    end
  '
}

m7e3_process_starttime() {
  awk '{print $22}' "/proc/$1/stat"
}

m7e3_process_pgid() {
  ps -o pgid= -p "$1" | tr -d '[:space:]'
}

m7e3_process_sid() {
  ps -o sid= -p "$1" | tr -d '[:space:]'
}

m7e3_process_cmdline_sha() {
  sha256sum "/proc/$1/cmdline" | awk '{print $1}'
}

m7e3_proxy_args() {
  local kind="$1"
  case "$kind" in
    server)
      printf '%s\0' \
        "TCP4-LISTEN:$M7E3_SERVER_PROXY_PORT,bind=127.0.0.1,reuseaddr,fork" \
        'TCP4:127.0.0.1:80'
      ;;
    docker)
      printf '%s\0' \
        "UNIX-LISTEN:$M7E3_DOCKER_PROXY_SOCKET,unlink-early,mode=0600,fork" \
        'UNIX-CONNECT:/var/run/docker.sock'
      ;;
    *)
      return 1
      ;;
  esac
}

m7e3_expected_proxy_cmdline_sha() {
  local kind="$1"
  local socat_path
  local -a args
  socat_path="$(readlink -f "$(command -v socat)")" || return 1
  mapfile -d '' -t args < <(m7e3_proxy_args "$kind")
  printf '%s\0' "$socat_path" "${args[@]}" | sha256sum | awk '{print $1}'
}

m7e3_record_process_identity() {
  local pid="$1"
  local role="$2"
  local output="$3"
  local exe
  [[ "$pid" =~ ^[1-9][0-9]*$ && -r "/proc/$pid/stat" ]] || return 1
  exe="$(readlink -f "/proc/$pid/exe")" || return 1
  {
    printf 'role=%s\npid=%s\n' "$role" "$pid"
    printf 'starttime=%s\n' "$(m7e3_process_starttime "$pid")"
    printf 'pgid=%s\nsid=%s\n' \
      "$(m7e3_process_pgid "$pid")" "$(m7e3_process_sid "$pid")"
    printf 'exe=%s\nsha256=%s\ncmdline_sha256=%s\n' \
      "$exe" "$(sha256sum "$exe" | awk '{print $1}')" \
      "$(m7e3_process_cmdline_sha "$pid")"
  } >"$output"
  chmod 0600 "$output"
}

m7e3_proxy_identity_matches_file() {
  local identity="$1"
  local kind="$2"
  local pid pgid sid role exe expected_socat
  m7e3_process_identity_matches_file "$identity" || return 1
  pid="$(sed -n 's/^pid=//p' "$identity")"
  pgid="$(sed -n 's/^pgid=//p' "$identity")"
  sid="$(sed -n 's/^sid=//p' "$identity")"
  role="$(sed -n 's/^role=//p' "$identity")"
  exe="$(sed -n 's/^exe=//p' "$identity")"
  expected_socat="$(readlink -f "$(command -v socat)")" || return 1
  [[ "$role" == "${kind}_proxy" ]] || return 1
  [[ "$pid" == "$pgid" && "$pid" == "$sid" ]] || return 1
  [[ "$exe" == "$expected_socat" ]] || return 1
  [[ "$(sed -n 's/^cmdline_sha256=//p' "$identity")" == \
    "$(m7e3_expected_proxy_cmdline_sha "$kind")" ]]
}

m7e3_terminate_recorded_proxy() {
  local identity="$1"
  local kind="$2"
  local evidence="$3"
  local pid pgid sid member member_sid member_exe expected_socat stop_tag members_file
  [[ -f "$identity" ]] || return 0
  pid="$(sed -n 's/^pid=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ ]] || return 1
  if [[ ! -r "/proc/$pid/stat" ]]; then
    pgid="$(sed -n 's/^pgid=//p' "$identity")"
    if ps -eo pgid= | awk -v group="$pgid" '$1 == group {found=1} END {exit !found}'; then
      return 1
    fi
    return 0
  fi
  m7e3_proxy_identity_matches_file "$identity" "$kind" || return 1
  pgid="$(sed -n 's/^pgid=//p' "$identity")"
  sid="$(sed -n 's/^sid=//p' "$identity")"
  expected_socat="$(readlink -f "$(command -v socat)")" || return 1
  stop_tag="${kind}-proxy-stop-$(date +%s%N)"
  members_file="$evidence/${stop_tag}-members.txt"
  : >"$members_file"
  while IFS= read -r member; do
    [[ -n "$member" ]] || continue
    member_sid="$(m7e3_process_sid "$member")"
    member_exe="$(readlink -f "/proc/$member/exe")"
    [[ "$member_sid" == "$sid" && "$member_exe" == "$expected_socat" ]] || return 1
    printf 'pid=%s starttime=%s exe=%s\n' \
      "$member" "$(m7e3_process_starttime "$member")" "$member_exe" \
      >>"$members_file"
  done < <(ps -eo pid=,pgid= | awk -v group="$pgid" '$2 == group {print $1}')
  [[ -s "$members_file" ]] || return 1
  kill -TERM -- "-$pgid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    if ! ps -eo pgid= | awk -v group="$pgid" '$1 == group {found=1} END {exit !found}'; then
      return 0
    fi
    sleep 1
  done
  while IFS= read -r member; do
    [[ -n "$member" ]] || continue
    member_sid="$(m7e3_process_sid "$member")"
    member_exe="$(readlink -f "/proc/$member/exe")"
    [[ "$member_sid" == "$sid" && "$member_exe" == "$expected_socat" ]] || return 1
    grep -Fxq "pid=$member starttime=$(m7e3_process_starttime "$member") exe=$member_exe" \
      "$members_file" || return 1
  done < <(ps -eo pid=,pgid= | awk -v group="$pgid" '$2 == group {print $1}')
  kill -KILL -- "-$pgid" 2>/dev/null || true
  printf 'forced_sigkill=true\nrole=%s_proxy\npid=%s\npgid=%s\n' \
    "$kind" "$pid" "$pgid" >"$evidence/${stop_tag}-forced.txt"
  for _ in $(seq 1 10); do
    if ! ps -eo pgid= | awk -v group="$pgid" '$1 == group {found=1} END {exit !found}'; then
      return 0
    fi
    sleep 1
  done
  return 1
}

m7e3_server_proxy_probe() {
  [[ "$(curl --silent --show-error --max-time 5 --output /dev/null \
    --write-out '%{http_code}' "${M7E3_SERVER_PROXY_URL}healthz" 2>/dev/null)" == '200' ]]
}

m7e3_docker_proxy_probe() {
  [[ -S "$M7E3_DOCKER_PROXY_SOCKET" && ! -L "$M7E3_DOCKER_PROXY_SOCKET" ]] || return 1
  [[ "$(curl --silent --show-error --max-time 5 --unix-socket \
    "$M7E3_DOCKER_PROXY_SOCKET" http://localhost/_ping 2>/dev/null)" == 'OK' ]]
}

m7e3_remove_stale_docker_proxy_socket() {
  [[ ! -e "$M7E3_DOCKER_PROXY_SOCKET" && ! -L "$M7E3_DOCKER_PROXY_SOCKET" ]] && return 0
  [[ -S "$M7E3_DOCKER_PROXY_SOCKET" && ! -L "$M7E3_DOCKER_PROXY_SOCKET" ]] || return 1
  [[ "$(stat -c '%U:%G' "$M7E3_DOCKER_PROXY_SOCKET")" == 'wujiaxing2:wujiaxing2' ]] || return 1
  unlink "$M7E3_DOCKER_PROXY_SOCKET"
}

m7e3_listener_count() {
  ss -H -lnt "sport = :$1" 2>/dev/null | wc -l
}

m7e3_listener_owner_pids() {
  ss -H -lntp "sport = :$1" 2>/dev/null |
    sed -n 's/.*pid=\([0-9][0-9]*\).*/\1/p' | sort -u
}

m7e3_managed_listener_count() {
  ss -H -lnt 2>/dev/null |
    awk '{split($4,a,":"); p=a[length(a)]+0; if (p >= 40000 && p <= 40063) n++} END {print n+0}'
}

m7e3_gpu_compute_count() {
  local uuid="$1"
  nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader 2>/dev/null |
    awk -v uuid="$uuid" '$1 == uuid {count++} END {print count+0}'
}

m7e3_gpu_memory_used() {
  local gpu_index="$1"
  nvidia-smi --query-gpu=index,memory.used --format=csv,noheader,nounits |
    awk -F', *' -v gpu_index="$gpu_index" '$1 == gpu_index {print $2}'
}

m7e3_maintenance_worker_has_new_heartbeat() {
  local worker_file="$1"
  local heartbeat_floor_epoch="$2"
  local heartbeat_time heartbeat_epoch
  jq -e \
    --argjson worker_id "$M7E3_WORKER_ID" '
      .id == $worker_id and
      .maintenance.enabled == true and
      .state == "maintenance" and
      .gpu_count == 2 and
      (.heartbeat_time | type == "string" and length > 0)
    ' "$worker_file" >/dev/null || return 1
  heartbeat_time="$(jq -r '.heartbeat_time' "$worker_file")"
  heartbeat_epoch="$(m7e3_server_timestamp_epoch "$heartbeat_time")" || return 1

  # Maintenance pins the Server state, so its separately stored unreachable flag can
  # remain stale. A heartbeat newer than the pre-start snapshot proves this process
  # has rejoined the Server control loop without trusting that stale field.
  [[ "$heartbeat_epoch" -gt "$heartbeat_floor_epoch" ]]
}

m7e3_capture_gpu() {
  local evidence="$1"
  local prefix="$2"
  nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
    --format=csv,noheader,nounits >"$evidence/${prefix}-gpus.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader,nounits >"$evidence/${prefix}-compute-processes.csv" \
    2>"$evidence/${prefix}-compute-processes.stderr" || true
}

m7e3_capture_docker_ids() {
  docker ps -a --no-trunc --format '{{.ID}}' | sort
}

m7e3_owned_container_ids() {
  local instance_id="$1"
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed \
    --filter label=io.gpustack.component=vllm \
    --filter label=io.gpustack.worker-uuid="$M7E3_WORKER_UUID" \
    --filter label=io.gpustack.model-instance-id="$instance_id"
}

m7e3_owned_container_count() {
  m7e3_owned_container_ids "$1" | sed '/^$/d' | wc -l
}

m7e3_validate_recovery_state_chain() {
  local watch_file="$1"
  local instance_id="$2"
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 1
  jq -s -e --argjson instance "$instance_id" '
    [.[] |
      select(.data.id == $instance and (.data.state | type) == "string") |
      .data.state] |
    reduce .[] as $state ([];
      if length == 0 or .[-1] != $state then . + [$state] else . end) |
    . == ["running", "error", "scheduled", "initializing", "starting", "running"]
  ' "$watch_file" >/dev/null
}

m7e3_capture_container() {
  local container_id="$1"
  local output="$2"
  docker inspect "$container_id" | jq '.[0] | {
    Id, Image, Name,
    labels:.Config.Labels,
    command:.Config.Cmd,
    entrypoint:.Config.Entrypoint,
    restart_policy:.HostConfig.RestartPolicy,
    network_mode:.HostConfig.NetworkMode,
    device_requests:.HostConfig.DeviceRequests,
    mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
    running:.State.Running,
    status:.State.Status,
    health:(.State.Health.Status // null),
    started_at:.State.StartedAt,
    finished_at:.State.FinishedAt,
    exit_code:.State.ExitCode,
    error:.State.Error,
    pid:.State.Pid
  }' >"$output"
}

m7e3_validate_owned_container() {
  local file="$1"
  local instance_id="$2"
  local spec_hash="$3"
  m7e3_validate_owned_container_identity "$file" "$instance_id" "$spec_hash" || return 1
  jq -e '.running == true and .status == "running"' "$file" >/dev/null
}

m7e3_validate_owned_container_identity() {
  local file="$1"
  local instance_id="$2"
  local spec_hash="$3"
  jq -e \
    --arg uuid "$M7E3_WORKER_UUID" \
    --arg instance "$instance_id" \
    --arg hash "$spec_hash" \
    --arg image "$M7E3_RUNNER_IMAGE_ID" '
      .Id != null and .Image == $image and
      .labels["io.gpustack.implementation"] == "rust" and
      .labels["io.gpustack.schema-version"] == "2" and
      .labels["io.gpustack.mode"] == "managed" and
      .labels["io.gpustack.owner-id"] == $uuid and
      .labels["io.gpustack.component"] == "vllm" and
      .labels["io.gpustack.worker-uuid"] == $uuid and
      .labels["io.gpustack.model-instance-id"] == $instance and
      .labels["io.gpustack.spec-hash"] == $hash
    ' "$file" >/dev/null
}

m7e3_global_snapshot() {
  local evidence="$1"
  local prefix="$2"
  local resource
  local status
  local filter
  for resource in models model-instances model-files workers; do
    case "$resource" in
      models)
        filter='{items:[.items[] | {id,name,backend,restart_on_error}]}'
        ;;
      model-instances)
        filter='{items:[.items[] | {id,model_id,worker_id,state,port,restart_count,last_restart_time}]}'
        ;;
      model-files)
        filter='{items:[.items[] | {id,worker_id,source,local_path,state,download_progress,size,resolved_paths,cleanup_on_delete}]}'
        ;;
      workers)
        filter='{items:[.items[] | {id,name,state,unreachable,maintenance,port,metrics_port,gpu_count,ip}]}'
        ;;
    esac
    status="$(m7e3_filtered_get operator \
      "http://127.0.0.1/v2/${resource}?page=-1" \
      "$evidence/${prefix}-${resource}.json" "$filter")"
    [[ "$status" == '200' ]] || return 1
  done
  jq -n \
    --argjson models "$(jq '[.items[].id] | sort' "$evidence/${prefix}-models.json")" \
    --argjson instances "$(jq '[.items[].id] | sort' "$evidence/${prefix}-model-instances.json")" \
    --argjson files "$(jq '[.items[].id] | sort' "$evidence/${prefix}-model-files.json")" \
    --argjson workers "$(jq '[.items[].id] | sort' "$evidence/${prefix}-workers.json")" \
    '{models:$models,model_instances:$instances,model_files:$files,workers:$workers}' \
    >"$evidence/${prefix}-global-ids.json"
}

m7e3_capture_python_worker() {
  local output="$1"
  docker inspect "$M7E3_PYTHON_WORKER_ID" | jq '.[0] | {
    Id, Image, Name,
    restart_policy:.HostConfig.RestartPolicy,
    network_mode:.HostConfig.NetworkMode,
    mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
    running:.State.Running
  }' >"$output"
}

m7e3_fixed_worker_args() {
  printf '%s\0' \
    --server-url "$M7E3_SERVER_PROXY_URL" \
    --worker-name "$M7E3_WORKER_NAME" \
    --worker-hostname r760xa \
    --worker-ip "$M7E3_WORKER_IP" \
    --worker-ifname eno8403 \
    --worker-port 20150 \
    --metrics-port 20151 \
    --token-file "$M7E3_CLUSTER_TOKEN" \
    --data-dir "$M7E3_DATA_DIR" \
    --log-dir "$M7E3_LOG_DIR" \
    --model-root "$M7E3_MODEL_ROOT" \
    --docker-socket "$M7E3_DOCKER_PROXY_SOCKET"
}

m7e3_expected_worker_cmdline_sha() {
  local -a args
  mapfile -d '' -t args < <(m7e3_fixed_worker_args)
  printf '%s\0' "$M7E3_BINARY" --output json worker "${args[@]}" run --managed-runtime |
    sha256sum | awk '{print $1}'
}

m7e3_process_identity_matches_file() {
  local identity="$1"
  local pid starttime exe sha cmdline_sha pgid sid
  [[ -f "$identity" ]] || return 1
  pid="$(sed -n 's/^pid=//p' "$identity")"
  starttime="$(sed -n 's/^starttime=//p' "$identity")"
  exe="$(sed -n 's/^exe=//p' "$identity")"
  sha="$(sed -n 's/^sha256=//p' "$identity")"
  cmdline_sha="$(sed -n 's/^cmdline_sha256=//p' "$identity")"
  pgid="$(sed -n 's/^pgid=//p' "$identity")"
  sid="$(sed -n 's/^sid=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ && -r "/proc/$pid/stat" ]] || return 1
  [[ "$(m7e3_process_starttime "$pid")" == "$starttime" ]] || return 1
  [[ "$(readlink -f "/proc/$pid/exe")" == "$exe" ]] || return 1
  [[ "$(sha256sum "$exe" | awk '{print $1}')" == "$sha" ]] || return 1
  [[ "$(m7e3_process_cmdline_sha "$pid")" == "$cmdline_sha" ]] || return 1
  [[ "$(m7e3_process_pgid "$pid")" == "$pgid" ]] || return 1
  [[ "$(m7e3_process_sid "$pid")" == "$sid" ]]
}

m7e3_terminate_recorded_process() {
  local identity="$1"
  local evidence="$2"
  local pid
  [[ -f "$identity" ]] || return 0
  pid="$(sed -n 's/^pid=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ -r "/proc/$pid/stat" ]] || return 0
  m7e3_process_identity_matches_file "$identity" || return 1
  kill -INT "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  m7e3_process_identity_matches_file "$identity" || return 1
  kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  m7e3_process_identity_matches_file "$identity" || return 1
  kill -KILL "$pid" 2>/dev/null || true
  printf 'forced_sigkill=true\npid=%s\n' "$pid" >"$evidence/rust-process-forced-stop.txt"
  for _ in $(seq 1 10); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  return 1
}

m7e3_direct_vllm_probe() {
  local port="$1"
  local prefix="$2"
  local body
  [[ "$port" =~ ^[0-9]+$ && "$port" -ge 40000 && "$port" -le 40063 ]] || return 1
  curl --silent --show-error --fail --max-time 20 \
    "http://$M7E3_WORKER_IP:$port/health" >"${prefix}-health.txt"
  curl --silent --show-error --fail --max-time 20 \
    "http://$M7E3_WORKER_IP:$port/v1/models" >"${prefix}-models.json"
  jq -e --arg model "$M7E3_TARGET_MODEL" \
    '[.data[] | select(.id == $model)] | length == 1' "${prefix}-models.json" >/dev/null
  body="$(jq -nc --arg model "$M7E3_TARGET_MODEL" '{
    model:$model,
    messages:[{role:"user",content:"Reply with exactly: runtime alive"}],
    max_tokens:16,
    stream:false
  }')"
  curl --silent --show-error --fail --max-time 120 \
    -H 'Content-Type: application/json' \
    --data-binary "$body" \
    "http://$M7E3_WORKER_IP:$port/v1/chat/completions" >"${prefix}-chat.json"
  jq -e '.choices[0].message.content | type == "string" and length > 0' \
    "${prefix}-chat.json" >/dev/null
}

m7e3_remove_exact_local_state() {
  local instance_id="$1"
  local cache_path
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 1
  unlink "$M7E3_DATA_DIR/managed/manifests/$instance_id.json" 2>/dev/null || true
  cache_path="$(readlink -m "$M7E3_DATA_DIR/managed/cache/$instance_id")"
  [[ "$cache_path" == "$M7E3_DATA_DIR/managed/cache/$instance_id" ]] || return 1
  if [[ -d "$cache_path" ]]; then
    find "$cache_path" -xdev -depth -delete
  elif [[ -e "$cache_path" || -L "$cache_path" ]]; then
    return 1
  fi
  if [[ -d "$M7E3_LOG_DIR/serve" ]]; then
    find "$M7E3_LOG_DIR/serve" -maxdepth 1 -type f \
      -name "${instance_id}.*.log" -delete
  fi
}

m7e3_remove_exact_owned_container() {
  local instance_id="$1"
  local container_id="$2"
  local spec_hash="$3"
  local evidence="$4"
  local current_ids current_count safe_file stopped_file stop_exit stopped short_id
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || return 1
  [[ "$spec_hash" =~ ^[0-9a-f]{64}$ ]] || return 1
  current_ids="$(m7e3_owned_container_ids "$instance_id")"
  current_count="$(printf '%s\n' "$current_ids" | sed '/^$/d' | wc -l)"
  # A crash recovery can temporarily leave the stopped and replacement generations.
  # Refuse any wider ownership set, then prove the requested ID is one of those exact
  # containers before stopping or removing it.
  [[ "$current_count" -ge 1 && "$current_count" -le 2 ]] || return 1
  printf '%s\n' "$current_ids" | grep -Fxq "$container_id" || return 1
  short_id="${container_id:0:12}"
  safe_file="$evidence/fallback-owned-container-$short_id.json"
  stopped_file="$evidence/fallback-owned-container-$short_id-stopped.json"
  m7e3_capture_container "$container_id" "$safe_file"
  m7e3_validate_owned_container_identity "$safe_file" "$instance_id" "$spec_hash" || return 1
  stop_exit=0
  if [[ "$(jq -r '.running' "$safe_file")" == 'true' ]]; then
    timeout 60 docker stop --timeout 30 "$container_id" \
      >"$evidence/fallback-container-$short_id-stop.txt" 2>&1 || stop_exit=$?
    stopped=0
    for _ in $(seq 1 60); do
      if [[ "$(docker inspect --format '{{.State.Running}}' "$container_id" 2>/dev/null)" == 'false' ]]; then
        stopped=1
        break
      fi
      sleep 1
    done
    [[ "$stopped" -eq 1 ]] || return 1
  else
    printf 'already_stopped=true\n' >"$evidence/fallback-container-$short_id-stop.txt"
  fi
  printf 'docker_stop_exit=%s\n' "$stop_exit" \
    >"$evidence/fallback-container-$short_id-stop-result.txt"
  m7e3_capture_container "$container_id" "$stopped_file" || return 1
  m7e3_validate_owned_container_identity \
    "$stopped_file" "$instance_id" "$spec_hash" || return 1
  jq -e '.running == false' "$stopped_file" >/dev/null || return 1
  docker rm "$container_id" \
    >"$evidence/fallback-container-$short_id-remove.txt" 2>&1 || return 1
  ! docker inspect "$container_id" >/dev/null 2>&1
}

m7e3_remove_all_exact_owned_containers() {
  local instance_id="$1"
  local spec_hash="$2"
  local evidence="$3"
  local container_id current_ids current_count
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ "$spec_hash" =~ ^[0-9a-f]{64}$ ]] || return 1
  current_ids="$(m7e3_owned_container_ids "$instance_id")"
  current_count="$(printf '%s\n' "$current_ids" | sed '/^$/d' | wc -l)"
  [[ "$current_count" -le 2 ]] || return 1
  while IFS= read -r container_id; do
    [[ -n "$container_id" ]] || continue
    m7e3_remove_exact_owned_container \
      "$instance_id" "$container_id" "$spec_hash" "$evidence" || return 1
  done <<<"$current_ids"
  [[ "$(m7e3_owned_container_count "$instance_id")" == '0' ]]
}

m7e3_restore_python_worker_if_safe() {
  local evidence="$1"
  [[ "$(docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed | wc -l)" == '0' ]] || return 1
  [[ "$(m7e3_managed_listener_count)" == '0' ]] || return 1
  [[ "$(m7e3_gpu_compute_count "$M7E3_GPU0_UUID")" == '0' ]] || return 1
  [[ "$(m7e3_gpu_compute_count "$M7E3_GPU1_UUID")" == '0' ]] || return 1
  [[ "$(m7e3_gpu_memory_used 0)" == '0' ]] || return 1
  [[ "$(m7e3_gpu_memory_used 1)" == '0' ]] || return 1
  if [[ "$(docker inspect --format '{{.State.Running}}' "$M7E3_PYTHON_WORKER_ID" 2>/dev/null)" != 'true' ]]; then
    docker start "$M7E3_PYTHON_WORKER_ID" >"$evidence/python-worker-start.txt" \
      2>"$evidence/python-worker-start.stderr" || return 1
  fi
  for _ in $(seq 1 120); do
    if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]]; then
      return 0
    fi
    sleep 1
  done
  return 1
}

m7e3_secret_file_count() {
  local authorization_header='Authorization'
  local cookie_header='Cookie'
  local pattern
  pattern="gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|${authorization_header}:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]|${cookie_header}:[[:space:]]*[^[:space:]]+"
  {
    grep -RIlE "$pattern" "$@" 2>/dev/null || true
  } | wc -l
}

m7e3_known_secret_file_count() {
  local token_file token
  for token_file in "$M7E3_OPERATOR_TOKEN" "$M7E3_CLUSTER_TOKEN" "$M7E3_WORKER_TOKEN"; do
    [[ -f "$token_file" && ! -L "$token_file" ]] || return 1
    token="$(tr -d '\r\n' <"$token_file")"
    [[ -n "$token" ]] || return 1
    grep -RIlF -- "$token" "$@" 2>/dev/null || true
  done | sort -u | wc -l
}

m7e3_seal_directory() {
  local directory="$1"
  find "$directory" -maxdepth 1 -type f -exec chmod 0600 {} +
  find "$directory" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$directory/manifest.sha256"
  chmod 0600 "$directory/manifest.sha256"
}
