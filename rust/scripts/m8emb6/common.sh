#!/usr/bin/env bash
# Shared immutable inputs and identity checks for the M8-EMB.6 toolchain.
# shellcheck disable=SC2034 # Constants are consumed by the scripts that source this file.

readonly M8EMB6_REPO='/home/wujiaxing2/workspaces/gpustack'
readonly M8EMB6_BINARY="$M8EMB6_REPO/rust/target/debug/gpustack-rs"
readonly M8EMB6_PRODUCT_COMMIT='92184388070f961e8bbc1fbed4a3ec3c84691829'
readonly M8EMB6_CARGO_LOCK_SHA='ce89eb7d47c0666ed35301705302ed260fc4f2d68c9f85b64f5fe8b6416e382a'
readonly M8EMB6_PYTHON_SERVER_URL='http://127.0.0.1/'
readonly M8EMB6_RUST106_SERVER_URL='http://10.213.80.106:18091/'
readonly M8EMB6_WORKER_NAME='rust-m8emb6-r760xa'
readonly M8EMB6_WORKER_HOSTNAME='r760xa'
readonly M8EMB6_WORKER_IP='10.213.80.90'
readonly M8EMB6_WORKER_IFNAME='eno8403'
readonly M8EMB6_WORKER_PORT='20250'
readonly M8EMB6_METRICS_PORT='20251'
readonly M8EMB6_TARGET_MODEL='m8emb6-bge-small-zh'
readonly M8EMB6_MODEL_REPOSITORY='BAAI/bge-small-zh-v1.5'
readonly M8EMB6_MODEL_REVISION='7999e1d3359715c523056ef9478215996d62a620'
readonly M8EMB6_MODEL_PATH='/home/wujiaxing2/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d'
readonly M8EMB6_MODEL_MANIFEST="$M8EMB6_REPO/rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.sha256"
readonly M8EMB6_MODEL_METADATA="$M8EMB6_REPO/rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.metadata.json"
readonly M8EMB6_MODEL_BYTES='96378336'
readonly M8EMB6_MODEL_FILES='11'
readonly M8EMB6_EMBEDDING_DIMENSION='512'
readonly M8EMB6_RUNNER_IMAGE='quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M8EMB6_RUNNER_IMAGE_ID='sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M8EMB6_GPU0_UUID='GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3'
readonly M8EMB6_GPU1_UUID='GPU-b1f89e65-e9c4-26f5-a306-8a520df07708'
readonly M8EMB6_PYTHON_CONTEXT='m8emb6-python'
readonly M8EMB6_RUST106_CONTEXT='m8emb6-rust106-restore'
readonly M8EMB6_PYTHON_OPERATOR_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m8emb6-python-operator-token'
readonly M8EMB6_RUST106_OPERATOR_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m8emb6-rust106-operator-token'
readonly M8EMB6_CLUSTER_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m8emb6-registration-token'
readonly M8EMB6_DATA_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/m8emb6-python'
readonly M8EMB6_LOG_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/m8emb6-python-logs'
readonly M8EMB6_EVIDENCE_ROOT='/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m8emb6'

m8emb6_die() {
  printf 'STOP: %s\n' "$*" >&2
  return 1
}

m8emb6_timestamp() {
  date --iso-8601=ns
}

m8emb6_require_commands() {
  local command_name
  for command_name in "$@"; do
    command -v "$command_name" >/dev/null 2>&1 ||
      m8emb6_die "required command is missing: $command_name" || return 1
  done
}

m8emb6_require_private_file() {
  local path="$1"
  [[ -f "$path" && ! -L "$path" ]] ||
    m8emb6_die "protected file is unavailable: $path" || return 1
  [[ "$(stat -c '%U:%G' "$path")" == 'wujiaxing2:wujiaxing2' ]] ||
    m8emb6_die "protected file owner is invalid: $path" || return 1
  [[ "$(stat -c '%a' "$path")" == '600' ]] ||
    m8emb6_die "protected file mode must be 600: $path" || return 1
  [[ "$(stat -c '%h' "$path")" == '1' ]] ||
    m8emb6_die "protected file must have one hard link: $path" || return 1
}

m8emb6_verify_model() {
  local actual_files expected_files total_bytes revision
  [[ -d "$M8EMB6_MODEL_PATH" && ! -L "$M8EMB6_MODEL_PATH" ]] ||
    m8emb6_die "fixed model directory is unavailable" || return 1
  [[ -f "$M8EMB6_MODEL_MANIFEST" && -f "$M8EMB6_MODEL_METADATA" ]] ||
    m8emb6_die "model identity files are unavailable" || return 1
  [[ "$(find "$M8EMB6_MODEL_PATH" -type l | wc -l)" == '0' ]] ||
    m8emb6_die "model directory contains a symlink" || return 1
  actual_files="$(find "$M8EMB6_MODEL_PATH" -type f -printf '%P\n' | sort)"
  expected_files="$(awk '{print substr($0,67)}' "$M8EMB6_MODEL_MANIFEST" | sort)"
  [[ "$actual_files" == "$expected_files" ]] ||
    m8emb6_die "model file set does not match the manifest" || return 1
  (cd "$M8EMB6_MODEL_PATH" && sha256sum --check --strict "$M8EMB6_MODEL_MANIFEST") \
    >/dev/null || m8emb6_die "model SHA-256 verification failed" || return 1
  total_bytes="$(find "$M8EMB6_MODEL_PATH" -type f -printf '%s\n' |
    awk '{sum += $1} END {print sum+0}')"
  [[ "$total_bytes" == "$M8EMB6_MODEL_BYTES" ]] ||
    m8emb6_die "model byte count is invalid" || return 1
  [[ "$(find "$M8EMB6_MODEL_PATH" -type f | wc -l)" == "$M8EMB6_MODEL_FILES" ]] ||
    m8emb6_die "model file count is invalid" || return 1
  revision="$(<"$M8EMB6_MODEL_PATH/REVISION")"
  [[ "$revision" == "$M8EMB6_MODEL_REVISION" ]] ||
    m8emb6_die "model revision is invalid" || return 1
  jq -e --arg revision "$M8EMB6_MODEL_REVISION" \
    --argjson bytes "$M8EMB6_MODEL_BYTES" \
    --argjson dimension "$M8EMB6_EMBEDDING_DIMENSION" '
      .revision == $revision and .total_bytes == $bytes and
      .embedding_dimension == $dimension and .pooling_type == "CLS"
    ' "$M8EMB6_MODEL_METADATA" >/dev/null ||
    m8emb6_die "model metadata contract is invalid" || return 1
}

m8emb6_auth_request() {
  local method="$1"
  local url="$2"
  local token_file="$3"
  local output="$4"
  local body_file="${5:-}"
  local config_fd token
  m8emb6_require_private_file "$token_file" || return 1
  token="$(tr -d '\r\n' <"$token_file")"
  [[ -n "$token" ]] || m8emb6_die "token file is empty: $token_file" || return 1
  exec {config_fd}<<<"header = \"Authorization: Bearer $token\""
  if [[ -n "$body_file" ]]; then
    curl --config "/dev/fd/$config_fd" --silent --show-error --max-time 30 \
      --request "$method" --header 'Content-Type: application/json' \
      --data-binary "@$body_file" --output "$output" --write-out '%{http_code}' \
      "$url"
  else
    curl --config "/dev/fd/$config_fd" --silent --show-error --max-time 30 \
      --request "$method" --output "$output" --write-out '%{http_code}' "$url"
  fi
  exec {config_fd}<&-
  unset token
}

m8emb6_set_worker_maintenance() {
  local context="$1"
  local worker="$2"
  local enabled="$3"
  local message="$4"
  local output="$5"
  if [[ "$enabled" == 'true' ]]; then
    "$M8EMB6_BINARY" --context "$context" --output json \
      worker maintenance enable "$worker" --message "$message" >"$output"
  elif [[ "$enabled" == 'false' ]]; then
    "$M8EMB6_BINARY" --context "$context" --output json \
      worker maintenance disable "$worker" --yes >"$output"
  else
    m8emb6_die "maintenance target must be true or false"
  fi
}

m8emb6_process_starttime() {
  awk '{print $22}' "/proc/$1/stat"
}

m8emb6_process_cmdline_sha() {
  sha256sum "/proc/$1/cmdline" | awk '{print $1}'
}

m8emb6_record_process_identity() {
  local pid="$1"
  local role="$2"
  local output="$3"
  local exe_link
  [[ "$pid" =~ ^[1-9][0-9]*$ && -r "/proc/$pid/stat" ]] || return 1
  exe_link="$(readlink "/proc/$pid/exe")" || return 1
  {
    printf 'role=%s\npid=%s\nstarttime=%s\n' \
      "$role" "$pid" "$(m8emb6_process_starttime "$pid")"
    printf 'exe=%s\nexe_sha256=%s\ncmdline_sha256=%s\n' \
      "$exe_link" "$(sha256sum "/proc/$pid/exe" | awk '{print $1}')" \
      "$(m8emb6_process_cmdline_sha "$pid")"
  } >"$output"
  chmod 0600 "$output"
}

m8emb6_process_identity_matches() {
  local identity="$1"
  local pid starttime exe_sha cmdline_sha
  [[ -f "$identity" && ! -L "$identity" ]] || return 1
  pid="$(sed -n 's/^pid=//p' "$identity")"
  starttime="$(sed -n 's/^starttime=//p' "$identity")"
  exe_sha="$(sed -n 's/^exe_sha256=//p' "$identity")"
  cmdline_sha="$(sed -n 's/^cmdline_sha256=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ && -r "/proc/$pid/stat" ]] || return 1
  [[ "$(m8emb6_process_starttime "$pid")" == "$starttime" ]] || return 1
  [[ "$(sha256sum "/proc/$pid/exe" | awk '{print $1}')" == "$exe_sha" ]] || return 1
  [[ "$(m8emb6_process_cmdline_sha "$pid")" == "$cmdline_sha" ]]
}

m8emb6_capture_gpu() {
  local directory="$1"
  local prefix="$2"
  nvidia-smi --query-gpu=index,uuid,name,memory.total,memory.used,utilization.gpu \
    --format=csv,noheader,nounits >"$directory/${prefix}-gpus.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader,nounits >"$directory/${prefix}-compute.csv" \
    2>"$directory/${prefix}-compute.stderr" || true
}

m8emb6_gpu_compute_count() {
  local uuid="$1"
  nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader 2>/dev/null |
    awk -v uuid="$uuid" '$1 == uuid {count++} END {print count+0}'
}

m8emb6_capture_container() {
  local container_id="$1"
  local output="$2"
  docker inspect "$container_id" | jq '.[0] | {
    Id, Image, Name, Created,
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
    restart_count:.RestartCount
  }' >"$output"
}

m8emb6_validate_container_tuple() {
  local file="$1"
  local worker_uuid="$2"
  local instance_id="$3"
  jq -e --arg uuid "$worker_uuid" --arg instance "$instance_id" '
    .Id != null and
    .labels["io.gpustack.implementation"] == "rust" and
    .labels["io.gpustack.mode"] == "managed" and
    .labels["io.gpustack.component"] == "vllm" and
    .labels["io.gpustack.worker-uuid"] == $uuid and
    .labels["io.gpustack.model-instance-id"] == $instance
  ' "$file" >/dev/null
}

m8emb6_seal_directory() {
  local directory="$1"
  find "$directory" -type f ! -name manifest.sha256 \
    ! -name existing-worker-binary -exec chmod 0600 {} +
  if [[ -f "$directory/existing-worker-binary" &&
    ! -L "$directory/existing-worker-binary" ]]; then
    chmod 0700 "$directory/existing-worker-binary"
  fi
  (
    cd "$directory"
    find . -type f ! -name manifest.sha256 -printf '%P\n' | sort |
      xargs -r sha256sum >manifest.sha256
    chmod 0600 manifest.sha256
  )
}

m8emb6_secret_scan_count() {
  local directory="$1"
  {
    grep -RIlE \
      'gpustack_[[:alnum:]_]{20,}|Authorization:[[:space:]]*Bearer|BEGIN [A-Z ]*PRIVATE KEY' \
      "$directory" 2>/dev/null || true
  } | wc -l
}

m8emb6_remove_owned_tree() {
  local path="$1"
  local expected="$2"
  [[ "$path" == "$expected" ]] || return 1
  [[ ! -L "$path" ]] || return 1
  [[ ! -e "$path" ]] && return 0
  [[ -d "$path" ]] || return 1
  [[ "$(stat -c '%U:%G' "$path")" == 'wujiaxing2:wujiaxing2' ]] || return 1
  find "$path" -xdev -type f -exec unlink {} \;
  find "$path" -xdev -type l -exec unlink {} \;
  find "$path" -xdev -depth -type d -exec rmdir {} +
}

m8emb6_worker_args() {
  printf '%s\0' \
    --server-url "$M8EMB6_PYTHON_SERVER_URL" \
    --worker-name "$M8EMB6_WORKER_NAME" \
    --worker-hostname "$M8EMB6_WORKER_HOSTNAME" \
    --worker-ip "$M8EMB6_WORKER_IP" \
    --worker-ifname "$M8EMB6_WORKER_IFNAME" \
    --worker-port "$M8EMB6_WORKER_PORT" \
    --metrics-port "$M8EMB6_METRICS_PORT" \
    --token-file "$M8EMB6_CLUSTER_TOKEN" \
    --data-dir "$M8EMB6_DATA_DIR" \
    --log-dir "$M8EMB6_LOG_DIR" \
    --docker-socket /var/run/docker.sock
}

m8emb6_expected_worker_cmdline_sha() {
  local -a arguments
  mapfile -d '' -t arguments < <(m8emb6_worker_args)
  printf '%s\0' "$M8EMB6_BINARY" --output json worker "${arguments[@]}" run |
    sha256sum | awk '{print $1}'
}
