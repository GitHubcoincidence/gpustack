#!/usr/bin/env bash
# Shared identity and safety helpers for the M7-E.1 formal toolchain.
# shellcheck disable=SC2034 # Constants form the interface consumed by sourcing scripts.

readonly M7E1_REPO='/home/wujiaxing2/workspaces/gpustack'
readonly M7E1_BINARY="$M7E1_REPO/rust/target/debug/gpustack-rs"
readonly M7E1_DATA_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/1'
readonly M7E1_LOG_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d-logs'
readonly M7E1_MODEL_ROOT='/home/wujiaxing2/flowscale-system-validation/models'
readonly M7E1_MODEL_PATH="$M7E1_MODEL_ROOT/Qwen2.5-0.5B-Instruct"
readonly M7E1_MODEL_MANIFEST="$M7E1_REPO/rust/fixtures/m5e/qwen2.5-0.5b-instruct-7ae557604adf.sha256"
readonly M7E1_MODEL_MANIFEST_SHA='645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6'
readonly M7E1_MODEL_WEIGHT_SHA='fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe'
readonly M7E1_MODEL_BYTES='999604126'
readonly M7E1_TARGET_MODEL='m7e1-qwen25-05b-gpu0'
readonly M7E1_RUNNER_IMAGE='quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M7E1_RUNNER_IMAGE_ID='sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M7E1_WORKER_ID='5'
readonly M7E1_WORKER_NAME='rust-m3d-r760xa'
readonly M7E1_WORKER_UUID='7c4e07a3-6b97-4577-9ff0-1fc6c2319596'
readonly M7E1_WORKER_IP='10.213.80.90'
readonly M7E1_GPU0_UUID='GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3'
readonly M7E1_GPU1_UUID='GPU-b1f89e65-e9c4-26f5-a306-8a520df07708'
readonly M7E1_OPERATOR_CONTEXT='m7e1-operator'
readonly M7E1_OPERATOR_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m7-operator-token'
readonly M7E1_CLUSTER_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token'
readonly M7E1_PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'

m7e1_timestamp() {
  date --iso-8601=ns
}

m7e1_auth_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$M7E1_OPERATOR_TOKEN"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

m7e1_cluster_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$M7E1_CLUSTER_TOKEN"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

m7e1_filtered_get() {
  local principal="$1"
  local url="$2"
  local output="$3"
  local filter="$4"
  local raw status
  raw="$(mktemp "${output}.raw.XXXXXX")"
  if [[ "$principal" == 'operator' ]]; then
    if ! status="$(m7e1_auth_get "$url" "$raw")"; then
      unlink "$raw"
      return 1
    fi
  elif [[ "$principal" == 'cluster' ]]; then
    if ! status="$(m7e1_cluster_get "$url" "$raw")"; then
      unlink "$raw"
      return 1
    fi
  else
    unlink "$raw"
    return 1
  fi
  if ! jq "$filter" "$raw" >"$output"; then
    unlink "$raw" "$output" 2>/dev/null || true
    return 1
  fi
  unlink "$raw"
  printf '%s' "$status"
}

m7e1_model_instance_get() {
  local url="$1"
  local output="$2"
  m7e1_filtered_get operator "$url" "$output" '
    if has("items") then
      {items:[.items[] | {id,model_id,worker_id,state,port,restart_count,last_restart_time}]}
    elif has("id") then
      {id,model_id,worker_id,state,port,restart_count,last_restart_time}
    else
      {detail}
    end
  '
}

m7e1_process_starttime() {
  awk '{print $22}' "/proc/$1/stat"
}

m7e1_process_pgid() {
  ps -o pgid= -p "$1" | tr -d '[:space:]'
}

m7e1_process_sid() {
  ps -o sid= -p "$1" | tr -d '[:space:]'
}

m7e1_process_cmdline_sha() {
  sha256sum "/proc/$1/cmdline" | awk '{print $1}'
}

m7e1_listener_count() {
  ss -H -lnt "sport = :$1" 2>/dev/null | wc -l
}

m7e1_listener_owner_pids() {
  ss -H -lntp "sport = :$1" 2>/dev/null |
    sed -n 's/.*pid=\([0-9][0-9]*\).*/\1/p' | sort -u
}

m7e1_managed_listener_count() {
  ss -H -lnt 2>/dev/null |
    awk '{split($4,a,":"); p=a[length(a)]+0; if (p >= 40000 && p <= 40063) n++} END {print n+0}'
}

m7e1_gpu_compute_count() {
  local uuid="$1"
  nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader 2>/dev/null |
    awk -v uuid="$uuid" '$1 == uuid {count++} END {print count+0}'
}

m7e1_gpu_memory_used() {
  local gpu_index="$1"
  nvidia-smi --query-gpu=index,memory.used --format=csv,noheader,nounits |
    awk -F', *' -v gpu_index="$gpu_index" '$1 == gpu_index {print $2}'
}

m7e1_capture_gpu() {
  local evidence="$1"
  local prefix="$2"
  nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
    --format=csv,noheader,nounits >"$evidence/${prefix}-gpus.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader,nounits >"$evidence/${prefix}-compute-processes.csv" \
    2>"$evidence/${prefix}-compute-processes.stderr" || true
}

m7e1_capture_docker_ids() {
  docker ps -a --no-trunc --format '{{.ID}}' | sort
}

m7e1_owned_container_ids() {
  local instance_id="$1"
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 0
  docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed \
    --filter label=io.gpustack.component=vllm \
    --filter label=io.gpustack.worker-uuid="$M7E1_WORKER_UUID" \
    --filter label=io.gpustack.model-instance-id="$instance_id"
}

m7e1_owned_container_count() {
  m7e1_owned_container_ids "$1" | sed '/^$/d' | wc -l
}

m7e1_capture_container() {
  local container_id="$1"
  local output="$2"
  docker inspect "$container_id" | jq '.[0] | {
    Id, Image, Name,
    labels:.Config.Labels,
    command:.Config.Cmd,
    entrypoint:.Config.Entrypoint,
    restart_policy:.HostConfig.RestartPolicy,
    network_mode:.HostConfig.NetworkMode,
    mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
    running:.State.Running,
    status:.State.Status,
    health:(.State.Health.Status // null),
    started_at:.State.StartedAt,
    pid:.State.Pid
  }' >"$output"
}

m7e1_validate_owned_container() {
  local file="$1"
  local instance_id="$2"
  local spec_hash="$3"
  m7e1_validate_owned_container_identity "$file" "$instance_id" "$spec_hash" || return 1
  jq -e '.running == true and .status == "running"' "$file" >/dev/null
}

m7e1_validate_owned_container_identity() {
  local file="$1"
  local instance_id="$2"
  local spec_hash="$3"
  jq -e \
    --arg uuid "$M7E1_WORKER_UUID" \
    --arg instance "$instance_id" \
    --arg hash "$spec_hash" \
    --arg image "$M7E1_RUNNER_IMAGE_ID" '
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

m7e1_global_snapshot() {
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
    status="$(m7e1_filtered_get operator \
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

m7e1_capture_python_worker() {
  local output="$1"
  docker inspect "$M7E1_PYTHON_WORKER_ID" | jq '.[0] | {
    Id, Image, Name,
    restart_policy:.HostConfig.RestartPolicy,
    network_mode:.HostConfig.NetworkMode,
    mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
    running:.State.Running
  }' >"$output"
}

m7e1_fixed_worker_args() {
  printf '%s\0' \
    --server-url http://127.0.0.1/ \
    --worker-name "$M7E1_WORKER_NAME" \
    --worker-hostname r760xa \
    --worker-ip "$M7E1_WORKER_IP" \
    --worker-ifname eno8403 \
    --worker-port 20150 \
    --metrics-port 20151 \
    --token-file "$M7E1_CLUSTER_TOKEN" \
    --data-dir "$M7E1_DATA_DIR" \
    --log-dir "$M7E1_LOG_DIR" \
    --model-root "$M7E1_MODEL_ROOT" \
    --docker-socket /var/run/docker.sock
}

m7e1_expected_worker_cmdline_sha() {
  local -a args
  mapfile -d '' -t args < <(m7e1_fixed_worker_args)
  printf '%s\0' "$M7E1_BINARY" --output json worker "${args[@]}" run --managed-runtime |
    sha256sum | awk '{print $1}'
}

m7e1_process_identity_matches_file() {
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
  [[ "$(m7e1_process_starttime "$pid")" == "$starttime" ]] || return 1
  [[ "$(readlink -f "/proc/$pid/exe")" == "$exe" ]] || return 1
  [[ "$(sha256sum "$exe" | awk '{print $1}')" == "$sha" ]] || return 1
  [[ "$(m7e1_process_cmdline_sha "$pid")" == "$cmdline_sha" ]] || return 1
  [[ "$(m7e1_process_pgid "$pid")" == "$pgid" ]] || return 1
  [[ "$(m7e1_process_sid "$pid")" == "$sid" ]]
}

m7e1_terminate_recorded_process() {
  local identity="$1"
  local evidence="$2"
  local pid
  [[ -f "$identity" ]] || return 0
  pid="$(sed -n 's/^pid=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ -r "/proc/$pid/stat" ]] || return 0
  m7e1_process_identity_matches_file "$identity" || return 1
  kill -INT "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  m7e1_process_identity_matches_file "$identity" || return 1
  kill -TERM "$pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  m7e1_process_identity_matches_file "$identity" || return 1
  kill -KILL "$pid" 2>/dev/null || true
  printf 'forced_sigkill=true\npid=%s\n' "$pid" >"$evidence/rust-process-forced-stop.txt"
  for _ in $(seq 1 10); do
    [[ ! -r "/proc/$pid/stat" ]] && return 0
    sleep 1
  done
  return 1
}

m7e1_direct_vllm_probe() {
  local port="$1"
  local prefix="$2"
  local body
  [[ "$port" =~ ^[0-9]+$ && "$port" -ge 40000 && "$port" -le 40063 ]] || return 1
  curl --silent --show-error --fail --max-time 20 \
    "http://$M7E1_WORKER_IP:$port/health" >"${prefix}-health.txt"
  curl --silent --show-error --fail --max-time 20 \
    "http://$M7E1_WORKER_IP:$port/v1/models" >"${prefix}-models.json"
  jq -e --arg model "$M7E1_TARGET_MODEL" \
    '[.data[] | select(.id == $model)] | length == 1' "${prefix}-models.json" >/dev/null
  body="$(jq -nc --arg model "$M7E1_TARGET_MODEL" '{
    model:$model,
    messages:[{role:"user",content:"Reply with exactly: runtime alive"}],
    max_tokens:16,
    stream:false
  }')"
  curl --silent --show-error --fail --max-time 120 \
    -H 'Content-Type: application/json' \
    --data-binary "$body" \
    "http://$M7E1_WORKER_IP:$port/v1/chat/completions" >"${prefix}-chat.json"
  jq -e '.choices[0].message.content | type == "string" and length > 0' \
    "${prefix}-chat.json" >/dev/null
}

m7e1_remove_exact_local_state() {
  local instance_id="$1"
  local cache_path
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 1
  unlink "$M7E1_DATA_DIR/managed/manifests/$instance_id.json" 2>/dev/null || true
  cache_path="$(readlink -m "$M7E1_DATA_DIR/managed/cache/$instance_id")"
  [[ "$cache_path" == "$M7E1_DATA_DIR/managed/cache/$instance_id" ]] || return 1
  if [[ -d "$cache_path" ]]; then
    find "$cache_path" -xdev -depth -delete
  elif [[ -e "$cache_path" || -L "$cache_path" ]]; then
    return 1
  fi
  if [[ -d "$M7E1_LOG_DIR/serve" ]]; then
    find "$M7E1_LOG_DIR/serve" -maxdepth 1 -type f \
      -name "${instance_id}.*.log" -delete
  fi
}

m7e1_remove_exact_owned_container() {
  local instance_id="$1"
  local container_id="$2"
  local spec_hash="$3"
  local evidence="$4"
  local current_ids safe_file stop_exit stopped
  [[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || return 1
  [[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || return 1
  [[ "$spec_hash" =~ ^[0-9a-f]{64}$ ]] || return 1
  current_ids="$(m7e1_owned_container_ids "$instance_id")"
  [[ "$current_ids" == "$container_id" ]] || return 1
  safe_file="$evidence/fallback-owned-container.json"
  m7e1_capture_container "$container_id" "$safe_file"
  m7e1_validate_owned_container_identity "$safe_file" "$instance_id" "$spec_hash" || return 1
  stop_exit=0
  if [[ "$(jq -r '.running' "$safe_file")" == 'true' ]]; then
    timeout 60 docker stop --timeout 30 "$container_id" \
      >"$evidence/fallback-container-stop.txt" 2>&1 || stop_exit=$?
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
    printf 'already_stopped=true\n' >"$evidence/fallback-container-stop.txt"
  fi
  printf 'docker_stop_exit=%s\n' "$stop_exit" >"$evidence/fallback-container-stop-result.txt"
  m7e1_capture_container "$container_id" "$evidence/fallback-owned-container-stopped.json" || return 1
  m7e1_validate_owned_container_identity \
    "$evidence/fallback-owned-container-stopped.json" "$instance_id" "$spec_hash" || return 1
  jq -e '.running == false' "$evidence/fallback-owned-container-stopped.json" >/dev/null || return 1
  docker rm "$container_id" >"$evidence/fallback-container-remove.txt" 2>&1 || return 1
  ! docker inspect "$container_id" >/dev/null 2>&1
}

m7e1_restore_python_worker_if_safe() {
  local evidence="$1"
  [[ "$(docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed | wc -l)" == '0' ]] || return 1
  [[ "$(m7e1_managed_listener_count)" == '0' ]] || return 1
  [[ "$(m7e1_gpu_compute_count "$M7E1_GPU0_UUID")" == '0' ]] || return 1
  [[ "$(m7e1_gpu_compute_count "$M7E1_GPU1_UUID")" == '0' ]] || return 1
  [[ "$(m7e1_gpu_memory_used 0)" == '0' ]] || return 1
  [[ "$(m7e1_gpu_memory_used 1)" == '0' ]] || return 1
  if [[ "$(docker inspect --format '{{.State.Running}}' "$M7E1_PYTHON_WORKER_ID" 2>/dev/null)" != 'true' ]]; then
    docker start "$M7E1_PYTHON_WORKER_ID" >"$evidence/python-worker-start.txt" \
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

m7e1_secret_file_count() {
  local authorization_header='Authorization'
  local cookie_header='Cookie'
  local pattern
  pattern="gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|${authorization_header}:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]|${cookie_header}:[[:space:]]*[^[:space:]]+"
  {
    grep -RIlE "$pattern" "$@" 2>/dev/null || true
  } | wc -l
}

m7e1_seal_directory() {
  local directory="$1"
  find "$directory" -maxdepth 1 -type f -exec chmod 0600 {} +
  find "$directory" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$directory/manifest.sha256"
  chmod 0600 "$directory/manifest.sha256"
}
