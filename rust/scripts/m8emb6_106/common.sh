#!/usr/bin/env bash
# Shared identities and fail-closed helpers for the 106 embedding compatibility gate.
# shellcheck disable=SC2034 # Constants are consumed by the scripts sourcing this file.

readonly M8EMB6106_REPO='/home/wujiaxing/gpustack'
readonly M8EMB6106_BINARY="$M8EMB6106_REPO/rust/target/debug/gpustack-rs"
readonly M8EMB6106_PRODUCT_COMMIT='92184388070f961e8bbc1fbed4a3ec3c84691829'
readonly M8EMB6106_CARGO_LOCK_SHA='ce89eb7d47c0666ed35301705302ed260fc4f2d68c9f85b64f5fe8b6416e382a'
readonly M8EMB6106_PYTHON_SERVER_URL='http://10.213.80.90/'
readonly M8EMB6106_RUST_SERVER_URL='http://127.0.0.1:18091/'
readonly M8EMB6106_ORIGINAL_WORKER_NAME='jiaxingHaoyuNB-106'
readonly M8EMB6106_ORIGINAL_WORKER_ID='17'
readonly M8EMB6106_ORIGINAL_MODEL_NAME='rust-rs6-106-qwen25-05b-gpu0'
readonly M8EMB6106_ORIGINAL_MODEL_ID='21'
readonly M8EMB6106_ORIGINAL_INSTANCE_ID='87'
readonly M8EMB6106_ORIGINAL_WORKER_PORT='20150'
readonly M8EMB6106_ORIGINAL_METRICS_PORT='20151'
readonly M8EMB6106_ORIGINAL_MODEL_PORT='40000'
readonly M8EMB6106_ORIGINAL_TOKEN='/home/wujiaxing/.config/liantiao/registration-token'
readonly M8EMB6106_ORIGINAL_DATA_DIR='/home/wujiaxing/.local/state/gpustack-rust-worker/liantiao-data'
readonly M8EMB6106_ORIGINAL_LOG_DIR='/home/wujiaxing/.local/state/gpustack-rust-worker/liantiao-logs'
readonly M8EMB6106_ORIGINAL_IDENTITY="$M8EMB6106_ORIGINAL_DATA_DIR/managed/worker-identity.json"
readonly M8EMB6106_ORIGINAL_WORKER_TOKEN="$M8EMB6106_ORIGINAL_DATA_DIR/managed/worker-token"
readonly M8EMB6106_ORIGINAL_MANIFEST="$M8EMB6106_ORIGINAL_DATA_DIR/managed/manifests/87.json"
readonly M8EMB6106_WORKER_NAME='rust-m8emb6-106'
readonly M8EMB6106_WORKER_HOSTNAME='lenovo-ThinkStation-P5'
readonly M8EMB6106_WORKER_IP='10.213.80.106'
readonly M8EMB6106_WORKER_IFNAME='eno8403'
readonly M8EMB6106_WORKER_PORT='20250'
readonly M8EMB6106_METRICS_PORT='20251'
readonly M8EMB6106_TARGET_MODEL='m8emb6-bge-small-zh-106'
readonly M8EMB6106_MODEL_REVISION='7999e1d3359715c523056ef9478215996d62a620'
readonly M8EMB6106_MODEL_PATH='/home/wujiaxing/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d'
readonly M8EMB6106_MODEL_MANIFEST="$M8EMB6106_REPO/rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.sha256"
readonly M8EMB6106_MODEL_METADATA="$M8EMB6106_REPO/rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.metadata.json"
readonly M8EMB6106_MODEL_BYTES='96378336'
readonly M8EMB6106_MODEL_FILES='11'
readonly M8EMB6106_EMBEDDING_DIMENSION='512'
readonly M8EMB6106_RUNNER_IMAGE='quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M8EMB6106_RUNNER_IMAGE_ID='sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly M8EMB6106_GPU0_UUID='GPU-be80b448-dd25-a523-676f-a279bf5c9c9c'
readonly M8EMB6106_CONTEXTS_FILE='/home/wujiaxing/.config/gpustack-rust/m8emb6-106-contexts.json'
readonly M8EMB6106_PYTHON_CONTEXT='m8emb6-106-python'
readonly M8EMB6106_RUST_CONTEXT='m8emb6-106-restore'
readonly M8EMB6106_PYTHON_OPERATOR_TOKEN='/home/wujiaxing/.config/gpustack-rust/m8emb6-106-python-operator-token'
readonly M8EMB6106_RUST_OPERATOR_TOKEN='/home/wujiaxing/.config/gpustack-rust/m8rs6-rs63-management-api-key'
readonly M8EMB6106_CLUSTER_TOKEN='/home/wujiaxing/.config/gpustack-rust/m8emb6-106-registration-token'
readonly M8EMB6106_DATA_DIR='/home/wujiaxing/.local/state/gpustack-rust-worker/m8emb6-106-python'
readonly M8EMB6106_LOG_DIR='/home/wujiaxing/.local/state/gpustack-rust-worker/m8emb6-106-python-logs'
readonly M8EMB6106_EVIDENCE_ROOT='/home/wujiaxing/.local/state/gpustack-rust-worker/evidence/m8emb6-106'
readonly M8EMB6106_JSON_TOOL="$M8EMB6106_REPO/rust/scripts/m8emb6_106/json_tool.py"

export GPUSTACK_RS_CONTEXTS_FILE="$M8EMB6106_CONTEXTS_FILE"

m8emb6106_die() {
  printf 'STOP: %s\n' "$*" >&2
  return 1
}

m8emb6106_require_commands() {
  local command_name
  for command_name in "$@"; do
    command -v "$command_name" >/dev/null 2>&1 ||
      m8emb6106_die "required command is missing: $command_name" || return 1
  done
}

m8emb6106_require_private_file() {
  local path="$1"
  [[ -f "$path" && ! -L "$path" ]] ||
    m8emb6106_die "protected file is unavailable: $path" || return 1
  [[ "$(stat -c '%U:%G' "$path")" == 'wujiaxing:wujiaxing' ]] ||
    m8emb6106_die "protected file owner is invalid: $path" || return 1
  [[ "$(stat -c '%a' "$path")" == '600' ]] ||
    m8emb6106_die "protected file mode must be 600: $path" || return 1
  [[ "$(stat -c '%h' "$path")" == '1' ]] ||
    m8emb6106_die "protected file must have one hard link: $path" || return 1
}

m8emb6106_verify_model() {
  python3 "$M8EMB6106_JSON_TOOL" verify-model \
    --model-path "$M8EMB6106_MODEL_PATH" \
    --manifest "$M8EMB6106_MODEL_MANIFEST" \
    --metadata "$M8EMB6106_MODEL_METADATA" \
    --revision "$M8EMB6106_MODEL_REVISION" \
    --files "$M8EMB6106_MODEL_FILES" \
    --bytes "$M8EMB6106_MODEL_BYTES" \
    --dimension "$M8EMB6106_EMBEDDING_DIMENSION"
}

m8emb6106_auth_request() {
  local method="$1"
  local url="$2"
  local token_file="$3"
  local output="$4"
  local body_file="${5:-}"
  local config_fd token
  m8emb6106_require_private_file "$token_file" || return 1
  token="$(tr -d '\r\n' <"$token_file")"
  [[ -n "$token" ]] || m8emb6106_die "token file is empty: $token_file" || return 1
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

m8emb6106_process_starttime() {
  awk '{print $22}' "/proc/$1/stat"
}

m8emb6106_process_cmdline_sha() {
  sha256sum "/proc/$1/cmdline" | awk '{print $1}'
}

m8emb6106_record_process_identity() {
  local pid="$1"
  local role="$2"
  local output="$3"
  local exe_link
  [[ "$pid" =~ ^[1-9][0-9]*$ && -r "/proc/$pid/stat" ]] || return 1
  exe_link="$(readlink "/proc/$pid/exe")" || return 1
  {
    printf 'role=%s\npid=%s\nstarttime=%s\n' \
      "$role" "$pid" "$(m8emb6106_process_starttime "$pid")"
    printf 'exe=%s\nexe_sha256=%s\ncmdline_sha256=%s\n' \
      "$exe_link" "$(sha256sum "/proc/$pid/exe" | awk '{print $1}')" \
      "$(m8emb6106_process_cmdline_sha "$pid")"
  } >"$output"
  chmod 0600 "$output"
}

m8emb6106_process_identity_matches() {
  local identity="$1"
  local pid starttime exe_sha cmdline_sha
  [[ -f "$identity" && ! -L "$identity" ]] || return 1
  pid="$(sed -n 's/^pid=//p' "$identity")"
  starttime="$(sed -n 's/^starttime=//p' "$identity")"
  exe_sha="$(sed -n 's/^exe_sha256=//p' "$identity")"
  cmdline_sha="$(sed -n 's/^cmdline_sha256=//p' "$identity")"
  [[ "$pid" =~ ^[1-9][0-9]*$ && -r "/proc/$pid/stat" ]] || return 1
  [[ "$(m8emb6106_process_starttime "$pid")" == "$starttime" ]] || return 1
  [[ "$(sha256sum "/proc/$pid/exe" | awk '{print $1}')" == "$exe_sha" ]] || return 1
  [[ "$(m8emb6106_process_cmdline_sha "$pid")" == "$cmdline_sha" ]]
}

m8emb6106_capture_gpu() {
  local directory="$1"
  local prefix="$2"
  nvidia-smi --query-gpu=index,uuid,name,memory.total,memory.used,utilization.gpu \
    --format=csv,noheader,nounits >"$directory/${prefix}-gpus.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader,nounits >"$directory/${prefix}-compute.csv" \
    2>"$directory/${prefix}-compute.stderr" || true
}

m8emb6106_gpu_compute_count() {
  nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader 2>/dev/null |
    awk -v uuid="$1" '$1 == uuid {count++} END {print count+0}'
}

m8emb6106_seal_directory() {
  local directory="$1"
  find "$directory" -type f ! -name manifest.sha256 -exec chmod 0600 {} +
  (
    cd "$directory"
    find . -type f ! -name manifest.sha256 -printf '%P\n' | sort |
      xargs -r sha256sum >manifest.sha256
    chmod 0600 manifest.sha256
  )
}

m8emb6106_secret_scan_count() {
  {
    grep -RIlE \
      'gpustack_[[:alnum:]_]{20,}|Authorization:[[:space:]]*Bearer|BEGIN [A-Z ]*PRIVATE KEY' \
      "$1" 2>/dev/null || true
  } | wc -l
}

m8emb6106_remove_owned_tree() {
  local path="$1"
  local expected="$2"
  [[ "$path" == "$expected" && ! -L "$path" ]] || return 1
  [[ ! -e "$path" ]] && return 0
  [[ -d "$path" ]] || return 1
  [[ "$(stat -c '%U:%G' "$path")" == 'wujiaxing:wujiaxing' ]] || return 1
  find "$path" -xdev -type f -exec unlink {} \;
  find "$path" -xdev -type l -exec unlink {} \;
  find "$path" -xdev -depth -type d -exec rmdir {} +
}

m8emb6106_worker_args() {
  printf '%s\0' \
    --server-url "$M8EMB6106_PYTHON_SERVER_URL" \
    --worker-name "$M8EMB6106_WORKER_NAME" \
    --worker-hostname "$M8EMB6106_WORKER_HOSTNAME" \
    --worker-ip "$M8EMB6106_WORKER_IP" \
    --worker-ifname "$M8EMB6106_WORKER_IFNAME" \
    --worker-port "$M8EMB6106_WORKER_PORT" \
    --metrics-port "$M8EMB6106_METRICS_PORT" \
    --token-file "$M8EMB6106_CLUSTER_TOKEN" \
    --data-dir "$M8EMB6106_DATA_DIR" \
    --log-dir "$M8EMB6106_LOG_DIR" \
    --docker-socket /var/run/docker.sock
}
