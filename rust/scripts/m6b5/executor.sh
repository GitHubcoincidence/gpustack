#!/usr/bin/env bash
# shellcheck disable=SC1091,SC2317
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly REPO='/home/wujiaxing2/workspaces/gpustack'
readonly EVIDENCE="${M6B5_EVIDENCE:?M6B5_EVIDENCE is required}"
readonly PREFLIGHT="${M6B5_PREFLIGHT:?M6B5_PREFLIGHT is required}"
readonly WATCHDOG="${M6B5_WATCHDOG:?M6B5_WATCHDOG is required}"
readonly EXPECTED_WATCHDOG_SHA="${M6B5_WATCHDOG_SHA:?M6B5_WATCHDOG_SHA is required}"
readonly EXPECTED_HEAD="${M6B5_EXPECTED_HEAD:?M6B5_EXPECTED_HEAD is required}"
readonly PRODUCT_COMMIT="${M6B5_PRODUCT_COMMIT:?M6B5_PRODUCT_COMMIT is required}"
readonly EXPECTED_PREFLIGHT_MANIFEST_SHA="${M6B5_PREFLIGHT_MANIFEST_SHA:?M6B5_PREFLIGHT_MANIFEST_SHA is required}"
readonly EXPECTED_LOCK_SHA="${M6B5_LOCK_SHA:?M6B5_LOCK_SHA is required}"
readonly FORMAL_ATTEMPT="${M6B5_ATTEMPT:?M6B5_ATTEMPT is required}"
readonly BINARY="$REPO/rust/target/debug/gpustack-rs"
readonly DATA_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/1'
readonly LOG_DIR='/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d-logs'
readonly MODEL_ROOT='/home/wujiaxing2/flowscale-system-validation/models'
readonly MODEL_PATH="$MODEL_ROOT/Qwen2.5-0.5B-Instruct"
readonly MODEL_MANIFEST="$REPO/rust/fixtures/m5e/qwen2.5-0.5b-instruct-7ae557604adf.sha256"
readonly EXPECTED_MODEL_MANIFEST_SHA='645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6'
readonly EXPECTED_WEIGHT_SHA='fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe'
readonly EXPECTED_MODEL_BYTES='999604126'
readonly TARGET_MODEL='m6b5-qwen25-05b-gpu0'
readonly RUNNER_IMAGE='quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly RUNNER_IMAGE_ID='sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
readonly WORKER_ID='5'
readonly WORKER_NAME='rust-m3d-r760xa'
readonly WORKER_UUID='7c4e07a3-6b97-4577-9ff0-1fc6c2319596'
readonly GPU0_UUID='GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3'
readonly OPERATOR_CONTEXT='m6b5-operator'
readonly OPERATOR_TOKEN='/home/wujiaxing2/.config/gpustack-rust/m6b5-operator-token'
readonly EXPECTED_PYTHON_WORKER_ID='68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165'

if [[ ! "$FORMAL_ATTEMPT" =~ ^[1-9][0-9]*$ ]]; then
  printf 'STOP: M6B5_ATTEMPT must be a positive integer\n' >&2
  exit 89
fi

if [[ -e "$EVIDENCE" || -L "$EVIDENCE" ]]; then
  printf 'STOP: evidence path already exists: %s\n' "$EVIDENCE" >&2
  exit 90
fi
if [[ ! -f "$WATCHDOG" ]] ||
  [[ "$(sha256sum "$WATCHDOG" | awk '{print $1}')" != "$EXPECTED_WATCHDOG_SHA" ]]; then
  printf 'STOP: watchdog identity mismatch\n' >&2
  exit 91
fi

install -d -m 0700 "$EVIDENCE"
install -m 0600 "$0" "$EVIDENCE/executor.sh"
install -m 0600 "$WATCHDOG" "$EVIDENCE/watchdog.sh"
exec 3>&1
exec >"$EVIDENCE/live.log" 2>&1

phase='initializing'
failure_reason=''
baseline_ready=0
window_entered=0
smoke_passed=0
cleanup_ok=1
finalizing=0
python_worker_id=''
rust_pid=''
rust_starttime=''
rust_exe=''
rust_sha=''
model_id=''
instance_id=''
model_file_id=''
assigned_port=''
owned_container_id=''
owned_spec_hash=''
state_chain_complete=0
inference_passed=0
idempotency_passed=0

timestamp() {
  date --iso-8601=ns
}

event() {
  jq -cn \
    --arg observed_at "$(timestamp)" \
    --arg phase "$phase" \
    --arg kind "$1" \
    --arg detail "$2" \
    '{observed_at:$observed_at,phase:$phase,kind:$kind,detail:$detail}' \
    >>"$EVIDENCE/timeline.ndjson"
}

fail() {
  failure_reason="$1"
  printf 'FAIL: %s (phase=%s)\n' "$failure_reason" "$phase" >&2
  event 'failure' "$failure_reason"
  exit 1
}

require_equal() {
  [[ "$1" == "$2" ]] || fail "$3"
}

require_zero() {
  [[ "$1" == '0' ]] || fail "$2"
}

auth_get() {
  local url="$1"
  local output="$2"
  {
    printf 'header = "Authorization: Bearer '
    tr -d '\r\n' <"$OPERATOR_TOKEN"
    printf '"\nurl = "%s"\n' "$url"
  } | curl --config - --silent --show-error --max-time 20 \
    --output "$output" --write-out '%{http_code}'
}

process_starttime() {
  awk '{print $22}' "/proc/$1/stat"
}

process_identity_matches() {
  local pid="$1"
  [[ -r "/proc/$pid/stat" ]] || return 1
  [[ "$(process_starttime "$pid")" == "$rust_starttime" ]] || return 1
  [[ "$(readlink -f "/proc/$pid/exe")" == "$rust_exe" ]] || return 1
  [[ "$(sha256sum "$rust_exe" | awk '{print $1}')" == "$rust_sha" ]]
}

listener_count() {
  ss -H -lnt "sport = :$1" 2>/dev/null | wc -l
}

managed_listener_count() {
  ss -H -lnt 2>/dev/null |
    awk '{split($4,a,":"); p=a[length(a)]+0; if (p >= 40000 && p <= 40063) n++} END {print n+0}'
}

gpu0_compute_count() {
  nvidia-smi --query-compute-apps=gpu_uuid --format=csv,noheader 2>/dev/null |
    awk -v uuid="$GPU0_UUID" '$1 == uuid {count++} END {print count+0}'
}

gpu0_memory_used() {
  nvidia-smi --query-gpu=index,memory.used --format=csv,noheader,nounits |
    awk -F', *' '$1 == 0 {print $2}'
}

capture_gpu() {
  local suffix="$1"
  nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
    --format=csv,noheader,nounits >"$EVIDENCE/${suffix}-gpu.csv"
  nvidia-smi --query-compute-apps=gpu_uuid,pid,process_name,used_memory \
    --format=csv,noheader,nounits >"$EVIDENCE/${suffix}-gpu-processes.csv" \
    2>"$EVIDENCE/${suffix}-gpu-processes.stderr" || true
}

capture_docker_ids() {
  docker ps -a --no-trunc --format '{{.ID}}' | sort
}

owned_container_ids() {
  [[ -n "$instance_id" ]] || return 0
  docker ps -aq --no-trunc \
    --filter label=io.gpustack.implementation=rust \
    --filter label=io.gpustack.mode=managed \
    --filter label=io.gpustack.component=vllm \
    --filter label=io.gpustack.worker-uuid="$WORKER_UUID" \
    --filter label=io.gpustack.model-instance-id="$instance_id"
}

owned_container_count() {
  owned_container_ids | sed '/^$/d' | wc -l
}

global_snapshot() {
  local suffix="$1"
  local resource
  local status
  for resource in models model-instances model-files workers; do
    status="$(auth_get "http://127.0.0.1/v2/${resource}?page=-1" \
      "$EVIDENCE/${suffix}-${resource}.json")"
    [[ "$status" == '200' ]] || return 1
  done
  jq -n \
    --argjson models "$(jq '[.items[].id] | sort' "$EVIDENCE/${suffix}-models.json")" \
    --argjson instances "$(jq '[.items[].id] | sort' "$EVIDENCE/${suffix}-model-instances.json")" \
    --argjson files "$(jq '[.items[].id] | sort' "$EVIDENCE/${suffix}-model-files.json")" \
    --argjson workers "$(jq '[.items[].id] | sort' "$EVIDENCE/${suffix}-workers.json")" \
    '{models:$models,model_instances:$instances,model_files:$files,workers:$workers}' \
    >"$EVIDENCE/${suffix}-global-ids.json"
}

fixed_worker_args() {
  printf '%s\0' \
    --server-url http://127.0.0.1/ \
    --worker-name "$WORKER_NAME" \
    --worker-hostname r760xa \
    --worker-ip 10.213.80.90 \
    --worker-ifname eno8403 \
    --worker-port 20150 \
    --metrics-port 20151 \
    --data-dir "$DATA_DIR" \
    --log-dir "$LOG_DIR" \
    --model-root "$MODEL_ROOT" \
    --docker-socket /var/run/docker.sock
}

declare -a worker_args
mapfile -d '' -t worker_args < <(fixed_worker_args)

stop_rust_worker() {
  [[ -n "$rust_pid" ]] || return 0
  if ! kill -0 "$rust_pid" 2>/dev/null; then
    wait "$rust_pid" 2>/dev/null || true
    rust_pid=''
    return 0
  fi
  if ! process_identity_matches "$rust_pid"; then
    printf '[cleanup] refusing signal after Rust PID identity drift\n' >&2
    return 1
  fi
  kill -INT "$rust_pid" 2>/dev/null || true
  for _ in $(seq 1 30); do
    if ! kill -0 "$rust_pid" 2>/dev/null; then
      wait "$rust_pid" 2>/dev/null || true
      rust_pid=''
      return 0
    fi
    sleep 1
  done
  process_identity_matches "$rust_pid" || return 1
  kill -TERM "$rust_pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    if ! kill -0 "$rust_pid" 2>/dev/null; then
      wait "$rust_pid" 2>/dev/null || true
      rust_pid=''
      return 0
    fi
    sleep 1
  done
  return 1
}

enable_maintenance() {
  local worker_file="$EVIDENCE/cleanup-worker.json"
  if timeout 30 "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
    worker get "$WORKER_ID" >"$worker_file" 2>"$EVIDENCE/cleanup-worker.stderr" &&
    [[ "$(jq -r '.maintenance.enabled' "$worker_file")" != 'true' ]]; then
    timeout 40 "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
      worker maintenance enable "$WORKER_ID" \
      --message "M6-B.5 attempt $FORMAL_ATTEMPT cleanup" \
      >"$EVIDENCE/cleanup-maintenance-enable.json" \
      2>"$EVIDENCE/cleanup-maintenance-enable.stderr" || cleanup_ok=0
  fi
}

discover_attempt_resources() {
  local candidate
  if timeout 30 "$BINARY" --context "$OPERATOR_CONTEXT" --output json model list \
    >"$EVIDENCE/cleanup-model-discovery.json" \
    2>"$EVIDENCE/cleanup-model-discovery.stderr"; then
    candidate="$(jq -r \
      --arg name "$TARGET_MODEL" \
      --slurpfile baseline "$EVIDENCE/before-global-ids.json" \
      '[.[] | select(.name == $name) | .id as $id |
        select(($baseline[0].models | index($id)) == null) | $id] |
       if length == 1 then .[0] else empty end' \
      "$EVIDENCE/cleanup-model-discovery.json")"
    if [[ -z "$model_id" ]]; then
      model_id="$candidate"
    elif [[ -n "$candidate" && "$candidate" != "$model_id" ]]; then
      cleanup_ok=0
    fi
  else
    cleanup_ok=0
  fi

  if [[ -n "$model_id" && -z "$instance_id" ]]; then
    if [[ "$(auth_get "http://127.0.0.1/v2/model-instances?page=-1&model_id=$model_id" \
      "$EVIDENCE/cleanup-instance-discovery.json" || true)" == '200' ]]; then
      instance_id="$(jq -r \
        'if (.items | length) == 1 then .items[0].id else empty end' \
        "$EVIDENCE/cleanup-instance-discovery.json")"
    else
      cleanup_ok=0
    fi
  fi
}

delete_attempt_model() {
  local deadline
  local status

  [[ -n "$model_id" ]] || return 0
  timeout 40 "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
    model delete "$model_id" --yes >"$EVIDENCE/cleanup-model-delete.json" \
    2>"$EVIDENCE/cleanup-model-delete.stderr" || cleanup_ok=0

  [[ -n "$instance_id" ]] || return 0
  deadline=$((SECONDS + 180))
  while (( SECONDS < deadline )); do
    status="$(auth_get "http://127.0.0.1/v2/model-instances/$instance_id" \
      "$EVIDENCE/cleanup-instance-latest.json" || true)"
    if [[ "$status" == '404' ]] &&
      [[ "$(owned_container_count)" == '0' ]] &&
      [[ ! -e "$DATA_DIR/managed/manifests/$instance_id.json" ]] &&
      [[ ! -e "$DATA_DIR/managed/cache/$instance_id" ]] &&
      [[ ! -e "$LOG_DIR/serve/$instance_id.0.log" ]] &&
      { [[ -z "$assigned_port" ]] || [[ "$(listener_count "$assigned_port")" == '0' ]]; }; then
      printf 'instance_404=true\nowned_container_count=0\nlocal_runtime_state=absent\n' \
        >"$EVIDENCE/cleanup-owned-runtime.txt"
      return 0
    fi
    sleep 1
  done
  cleanup_ok=0
}

delete_attempt_model_file() {
  local file_list="$EVIDENCE/cleanup-model-files.json"
  local candidate
  local fetched

  if ! timeout 30 "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
    model file list --worker "$WORKER_ID" >"$file_list" \
    2>"$EVIDENCE/cleanup-model-files.stderr"; then
    cleanup_ok=0
    return
  fi
  mapfile -t candidates < <(jq -r \
    --arg path "$MODEL_PATH" \
    --slurpfile baseline "$EVIDENCE/before-global-ids.json" \
    '.[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path) |
     .id as $id | select(($baseline[0].model_files | index($id)) == null) | $id' \
    "$file_list")
  if [[ "${#candidates[@]}" -gt 1 ]]; then
    cleanup_ok=0
    return
  fi
  for candidate in "${candidates[@]:-}"; do
    [[ -n "$candidate" ]] || continue
    fetched="$EVIDENCE/cleanup-model-file-${candidate}.json"
    if timeout 30 "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
      model file get "$candidate" >"$fetched" 2>"$fetched.stderr" &&
      jq -e --arg path "$MODEL_PATH" \
        '.id > 0 and .worker_id == 5 and .source == "local_path" and
         .local_path == $path and ((.cleanup_on_delete == null) or
         (.cleanup_on_delete == false))' "$fetched" >/dev/null; then
      timeout 40 "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
        model file delete "$candidate" --yes \
        >"$EVIDENCE/cleanup-model-file-delete-${candidate}.json" \
        2>"$EVIDENCE/cleanup-model-file-delete-${candidate}.stderr" || cleanup_ok=0
    else
      cleanup_ok=0
    fi
  done
}

restore_python_worker() {
  python_worker_id="${python_worker_id:-$EXPECTED_PYTHON_WORKER_ID}"
  if [[ "$(docker inspect --format '{{.State.Running}}' "$python_worker_id" 2>/dev/null)" != 'true' ]]; then
    docker start "$python_worker_id" >"$EVIDENCE/cleanup-python-worker-start.txt" \
      2>"$EVIDENCE/cleanup-python-worker-start.stderr" || {
      cleanup_ok=0
      return
    }
  fi
  for _ in $(seq 1 120); do
    if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] &&
      [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
        http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]]; then
      return
    fi
    sleep 1
  done
  cleanup_ok=0
}

final_checks() {
  (cd "$MODEL_PATH" && sha256sum --check "$MODEL_MANIFEST") \
    >"$EVIDENCE/after-model-manifest-check.txt" 2>&1 || cleanup_ok=0
  [[ "$(sha256sum "$MODEL_PATH/model.safetensors" | awk '{print $1}')" == "$EXPECTED_WEIGHT_SHA" ]] || cleanup_ok=0

  "$BINARY" --context "$OPERATOR_CONTEXT" --output json worker get "$WORKER_ID" \
    >"$EVIDENCE/after-worker.json" 2>"$EVIDENCE/after-worker.stderr" || cleanup_ok=0
  jq -e '.id == 5 and .name == "rust-m3d-r760xa" and .maintenance.enabled == true' \
    "$EVIDENCE/after-worker.json" >/dev/null 2>&1 || cleanup_ok=0
  global_snapshot 'after' || cleanup_ok=0
  cmp -s "$EVIDENCE/before-global-ids.json" "$EVIDENCE/after-global-ids.json" || cleanup_ok=0

  capture_docker_ids >"$EVIDENCE/after-docker-ids.txt" || cleanup_ok=0
  cmp -s "$EVIDENCE/before-docker-ids.txt" "$EVIDENCE/after-docker-ids.txt" || cleanup_ok=0
  docker inspect "$python_worker_id" | jq '.[0] | {
    Id, Image, Name, restart_policy:.HostConfig.RestartPolicy,
    network_mode:.HostConfig.NetworkMode,
    mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
    running:.State.Running
  }' >"$EVIDENCE/after-python-worker-identity.json" || cleanup_ok=0
  cmp -s "$EVIDENCE/before-python-worker-identity.json" \
    "$EVIDENCE/after-python-worker-identity.json" || cleanup_ok=0
  [[ "$(docker inspect --format '{{.State.Running}}' "$python_worker_id" 2>/dev/null)" == 'true' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10151/metrics 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]] || cleanup_ok=0
  [[ "$(managed_listener_count)" == '0' ]] || cleanup_ok=0
  [[ "$(gpu0_compute_count)" == '0' ]] || cleanup_ok=0
  [[ "$(gpu0_memory_used)" == '0' ]] || cleanup_ok=0
  capture_gpu 'after'
}

pre_window_safety_checks() {
  [[ "$(docker inspect --format '{{.State.Running}}' "$EXPECTED_PYTHON_WORKER_ID" 2>/dev/null)" == 'true' ]] || cleanup_ok=0
  [[ "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
    http://127.0.0.1:10150/healthz 2>/dev/null)" == '200' ]] || cleanup_ok=0
  [[ "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" == '0' ]] || cleanup_ok=0
  [[ "$(managed_listener_count)" == '0' ]] || cleanup_ok=0
  capture_gpu 'pre-window-stop'
}

seal_evidence() {
  local requested_exit="$1"
  local status='FAIL'
  local reason="${failure_reason:-maintenance_window_or_cleanup_failed}"
  local secret_count

  if [[ "$requested_exit" -eq 0 && "$smoke_passed" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    status='PASS'
    reason='managed_runtime_inference_idempotency_and_cleanup_passed'
  elif [[ "$window_entered" -eq 0 ]]; then
    status='STOP'
    reason="${failure_reason:-pre_window_gate_failed}"
    requested_exit=1
  else
    requested_exit=1
  fi
  {
    printf 'recorded_at=%s\n' "$(date --iso-8601=seconds)"
    printf 'status=%s\nstage=M6-B.5.2\nattempt=%s\n' "$status" "$FORMAL_ATTEMPT"
    printf 'reason=%s\ncompleted_phase=%s\n' "$reason" "$phase"
    printf 'baseline_ready=%s\nwindow_entered=%s\nsmoke_passed=%s\ncleanup_ok=%s\n' \
      "$baseline_ready" "$window_entered" "$smoke_passed" "$cleanup_ok"
    printf 'model_id=%s\nmodel_instance_id=%s\nmodel_file_id=%s\nassigned_port=%s\n' \
      "${model_id:-none}" "${instance_id:-none}" "${model_file_id:-none}" "${assigned_port:-none}"
    printf 'owned_container_id=%s\nstate_chain_complete=%s\ninference_passed=%s\nidempotency_passed=%s\n' \
      "${owned_container_id:-none}" "$state_chain_complete" "$inference_passed" "$idempotency_passed"
  } >"$EVIDENCE/result.txt"

  find "$EVIDENCE" -maxdepth 1 -type f -exec chmod 0600 {} +
  secret_count="$(grep -RIlE \
    'gpustack_[[:alnum:]]{20,}|BEGIN [A-Z ]*PRIVATE KEY|Authorization:[[:space:]]*Bearer[[:space:]]+[[:alnum:]_]' \
    "$EVIDENCE" 2>/dev/null | wc -l)"
  printf 'secret_file_count=%s\n' "$secret_count" >"$EVIDENCE/secret-scan.txt"
  if [[ "$secret_count" -ne 0 ]]; then
    status='FAIL'
    requested_exit=1
    sed -i 's/^status=.*/status=FAIL/; s/^reason=.*/reason=secret_scan_detected_sensitive_content/' \
      "$EVIDENCE/result.txt"
  fi
  find "$EVIDENCE" -maxdepth 1 -type f ! -name manifest.sha256 -print0 |
    sort -z | xargs -0 sha256sum >"$EVIDENCE/manifest.sha256"
  chmod 0600 "$EVIDENCE/manifest.sha256"
  printf 'evidence_dir=%s\nmanifest_file_sha256=%s\nm6b5_managed_runtime_smoke=%s\n' \
    "$EVIDENCE" "$(sha256sum "$EVIDENCE/manifest.sha256" | awk '{print $1}')" "$status" >&3
  return "$requested_exit"
}

finalize() {
  local original_exit="$1"
  local final_exit=1
  [[ "$finalizing" -eq 0 ]] || return
  finalizing=1
  trap - EXIT INT TERM HUP
  set +o errexit
  phase='cleanup'
  event 'cleanup_started' "original_exit=$original_exit"
  if [[ "$window_entered" -eq 0 ]]; then
    pre_window_safety_checks
  else
    if [[ "$(curl --silent --max-time 5 --output /dev/null --write-out '%{http_code}' \
      http://127.0.0.1/healthz 2>/dev/null)" == '200' ]]; then
      enable_maintenance
      discover_attempt_resources
      delete_attempt_model
      delete_attempt_model_file
    else
      cleanup_ok=0
    fi
    stop_rust_worker || cleanup_ok=0
    restore_python_worker
    if [[ "$baseline_ready" -eq 1 ]]; then
      final_checks
    fi
  fi
  event 'cleanup_finished' "cleanup_ok=$cleanup_ok"
  phase='sealed'
  if [[ "$original_exit" -eq 0 && "$smoke_passed" -eq 1 && "$cleanup_ok" -eq 1 ]]; then
    final_exit=0
  fi
  seal_evidence "$final_exit"
  final_exit=$?
  exit "$final_exit"
}

on_signal() {
  failure_reason="signal_$1"
  event 'signal' "$1"
  exit "$2"
}

trap 'finalize "$?"' EXIT
trap 'on_signal TERM 143' TERM
trap 'on_signal INT 130' INT
trap 'on_signal HUP 129' HUP

phase='source_build_and_drift_gate'
source /home/wujiaxing2/.cargo/env
cd "$REPO"
require_equal "$(git branch --show-current)" 'codex/rust-worker' 'unexpected_branch'
require_equal "$(git rev-parse HEAD)" "$EXPECTED_HEAD" 'source_head_mismatch'
require_equal "$(git rev-parse origin/codex/rust-worker)" "$EXPECTED_HEAD" 'origin_head_mismatch'
require_zero "$(git status --porcelain | wc -l)" 'remote_worktree_not_clean'
git merge-base --is-ancestor "$PRODUCT_COMMIT" HEAD || fail 'product_commit_not_ancestor'
git diff --quiet "$PRODUCT_COMMIT" HEAD -- \
  rust/Cargo.toml rust/Cargo.lock rust/rust-toolchain.toml rust/crates ||
  fail 'post_product_rust_drift'
require_equal "$(rustc --version)" 'rustc 1.99.1 (58cf8f9c3 2026-10-13)' 'rustc_version_mismatch'
require_equal "$(cargo --version)" 'cargo 1.99.1 (5f94df478 2026-08-27)' 'cargo_version_mismatch'
require_equal "$(sha256sum rust/Cargo.lock | awk '{print $1}')" "$EXPECTED_LOCK_SHA" 'cargo_lock_sha_mismatch'
require_equal "$(sha256sum "$PREFLIGHT/manifest.sha256" | awk '{print $1}')" \
  "$EXPECTED_PREFLIGHT_MANIFEST_SHA" 'preflight_manifest_identity_mismatch'
(cd "$PREFLIGHT" && sha256sum --check manifest.sha256) \
  >"$EVIDENCE/preflight-manifest-check.txt" || fail 'preflight_manifest_check_failed'
require_equal "$(sed -n 's/^status=//p' "$PREFLIGHT/preflight-result.txt")" \
  'PASS' 'preflight_status_not_pass'

cargo clean --manifest-path "$REPO/rust/Cargo.toml" -p gpustack-cli \
  >"$EVIDENCE/cargo-clean.log" 2>&1
cargo build --manifest-path "$REPO/rust/Cargo.toml" --locked --offline -p gpustack-cli \
  >"$EVIDENCE/build.log" 2>&1
chmod 0700 "$BINARY"
"$BINARY" --output json version >"$EVIDENCE/binary-version.json"
require_equal "$(jq -r '.git_commit' "$EVIDENCE/binary-version.json")" \
  "${EXPECTED_HEAD:0:12}" 'binary_embedded_commit_mismatch'
rust_sha="$(sha256sum "$BINARY" | awk '{print $1}')"
{
  printf 'source_head=%s\nproduct_commit=%s\n' "$EXPECTED_HEAD" "$PRODUCT_COMMIT"
  printf 'binary_sha256=%s\ncargo_lock_sha256=%s\n' "$rust_sha" "$EXPECTED_LOCK_SHA"
  printf 'preflight_manifest_file_sha256=%s\n' "$EXPECTED_PREFLIGHT_MANIFEST_SHA"
} >"$EVIDENCE/source-identity.txt"
event 'gate_passed' 'source_build_and_drift_gate'

phase='final_external_drift_gate'
curl --silent --show-error --fail --max-time 10 \
  http://127.0.0.1/version >"$EVIDENCE/server-version.json"
jq -e '.version == "v2.2.3" and .git_commit == "1cfcbbf"' \
  "$EVIDENCE/server-version.json" >/dev/null || fail 'server_identity_mismatch'
"$BINARY" --output json context list >"$EVIDENCE/contexts.json"
jq -e --arg context "$OPERATOR_CONTEXT" --arg token "$OPERATOR_TOKEN" '
  [.[] | select(.name == $context and .server_url == "http://127.0.0.1/" and
   .token_file == $token and .active == true)] | length == 1
' "$EVIDENCE/contexts.json" >/dev/null || fail 'operator_context_mismatch'

"$BINARY" --output json worker "${worker_args[@]}" validate-config \
  >"$EVIDENCE/validate-config.json"
jq -e '
  .valid == true and .config.worker_name == "rust-m3d-r760xa" and
  .config.worker_port == 20150 and .config.metrics_port == 20151 and
  .config.data_dir == "/home/wujiaxing2/.local/state/gpustack-rust-worker/1" and
  .config.model_roots == ["/home/wujiaxing2/flowscale-system-validation/models"]
' "$EVIDENCE/validate-config.json" >/dev/null || fail 'fixed_config_mismatch'

"$BINARY" --context "$OPERATOR_CONTEXT" --output json worker get "$WORKER_ID" \
  >"$EVIDENCE/before-worker.json"
jq -e '
  .id == 5 and .name == "rust-m3d-r760xa" and
  .maintenance.enabled == true and .port == 20150 and .metrics_port == 20151
' "$EVIDENCE/before-worker.json" >/dev/null || fail 'worker_identity_or_maintenance_mismatch'
require_equal "$(jq -r '.worker_uuid' "$DATA_DIR/managed/worker-identity.json")" \
  "$WORKER_UUID" 'persisted_worker_uuid_mismatch'
require_equal "$(stat -c '%a' "$OPERATOR_TOKEN")" '600' 'operator_token_mode_mismatch'
require_equal "$(stat -c '%a' "$DATA_DIR/managed/worker-token")" '600' 'worker_token_mode_mismatch'

global_snapshot 'before' || fail 'global_baseline_query_failed'
require_zero "$(jq --arg name "$TARGET_MODEL" \
  '[.items[] | select(.name == $name)] | length' "$EVIDENCE/before-models.json")" \
  'target_model_conflict'
require_zero "$(jq '[.items[] | select(.worker_id == 5)] | length' \
  "$EVIDENCE/before-model-instances.json")" 'worker_model_instance_conflict'
require_zero "$(jq --arg path "$MODEL_PATH" \
  '[.items[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] | length' \
  "$EVIDENCE/before-model-files.json")" 'target_model_file_conflict'
require_zero "$(docker ps -aq --no-trunc \
  --filter label=io.gpustack.implementation=rust \
  --filter label=io.gpustack.mode=managed | wc -l)" 'rust_owned_container_conflict'

require_equal "$(readlink -f "$MODEL_PATH")" "$MODEL_PATH" 'model_path_mismatch'
require_equal "$(sha256sum "$MODEL_MANIFEST" | awk '{print $1}')" \
  "$EXPECTED_MODEL_MANIFEST_SHA" 'model_manifest_fixture_mismatch'
(cd "$MODEL_PATH" && sha256sum --check "$MODEL_MANIFEST") \
  >"$EVIDENCE/before-model-manifest-check.txt" || fail 'model_manifest_check_failed'
require_equal "$(find "$MODEL_PATH" -type f -printf '%s\n' |
  awk '{sum += $1} END {print sum+0}')" "$EXPECTED_MODEL_BYTES" 'model_size_mismatch'
require_equal "$(sha256sum "$MODEL_PATH/model.safetensors" | awk '{print $1}')" \
  "$EXPECTED_WEIGHT_SHA" 'model_weight_mismatch'
require_equal "$(docker image inspect --format '{{.Id}}' "$RUNNER_IMAGE")" \
  "$RUNNER_IMAGE_ID" 'runner_image_identity_mismatch'

capture_gpu 'before'
require_equal "$(awk -F', *' '$1 == 0 {print $2}' "$EVIDENCE/before-gpu.csv")" \
  "$GPU0_UUID" 'gpu0_uuid_mismatch'
require_zero "$(gpu0_compute_count)" 'gpu0_compute_process_exists'
require_zero "$(gpu0_memory_used)" 'gpu0_memory_not_zero'
require_zero "$(pgrep -af '[g]pustack-rs' 2>/dev/null | wc -l)" 'rust_worker_process_exists'
require_zero "$(listener_count 20150)" 'rust_worker_api_port_occupied'
require_zero "$(listener_count 20151)" 'rust_metrics_port_occupied'
require_zero "$(managed_listener_count)" 'managed_port_pool_occupied'

mapfile -t python_workers < <(docker ps -a --no-trunc --format '{{.ID}} {{.Names}}' |
  awk '$2 == "gpustack-worker" {print $1}')
require_equal "${#python_workers[@]}" '1' 'python_worker_not_unique'
python_worker_id="${python_workers[0]}"
require_equal "$python_worker_id" "$EXPECTED_PYTHON_WORKER_ID" 'python_worker_id_drift'
require_equal "$(docker inspect --format '{{.State.Running}}' "$python_worker_id")" \
  'true' 'python_worker_not_running'
require_equal "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10150/healthz)" '200' 'python_worker_health_not_200'
require_equal "$(curl --silent --max-time 10 --output /dev/null --write-out '%{http_code}' \
  http://127.0.0.1:10151/metrics)" '200' 'python_worker_metrics_not_200'
docker inspect "$python_worker_id" | jq '.[0] | {
  Id, Image, Name, restart_policy:.HostConfig.RestartPolicy,
  network_mode:.HostConfig.NetworkMode,
  mounts:([.Mounts[] | {Type,Source,Destination,Mode,RW,Propagation}] | sort_by(.Destination)),
  running:.State.Running
}' >"$EVIDENCE/before-python-worker-identity.json"
capture_docker_ids >"$EVIDENCE/before-docker-ids.txt"
baseline_ready=1
event 'gate_passed' 'final_external_drift_gate'

phase='python_worker_stop'
window_entered=1
docker stop --time 30 "$python_worker_id" >"$EVIDENCE/python-worker-stop.txt"
require_equal "$(docker inspect --format '{{.State.Running}}' "$python_worker_id")" \
  'false' 'python_worker_failed_to_stop'
require_zero "$(listener_count 10150)" 'python_worker_api_still_listening'
require_zero "$(listener_count 10151)" 'python_worker_metrics_still_listening'
event 'maintenance_window_entered' "python_worker_id=$python_worker_id"

phase='rust_worker_start'
"$BINARY" --output json worker "${worker_args[@]}" run --managed-runtime \
  >"$EVIDENCE/rust-summary.json" \
  2> >(while IFS= read -r line; do
    printf '%s\t%s\n' "$(timestamp)" "$line"
  done >>"$EVIDENCE/rust-events.log") &
rust_pid=$!
printf '%s\n' "$rust_pid" >"$EVIDENCE/rust-worker.pid"
for _ in $(seq 1 50); do
  [[ -r "/proc/$rust_pid/stat" ]] && break
  sleep 0.1
done
kill -0 "$rust_pid" 2>/dev/null || fail 'rust_worker_exited_during_startup'
rust_starttime="$(process_starttime "$rust_pid")"
rust_exe="$(readlink -f "/proc/$rust_pid/exe")"
require_equal "$rust_exe" "$(readlink -f "$BINARY")" 'rust_worker_exe_mismatch'
require_equal "$(sha256sum "$rust_exe" | awk '{print $1}')" "$rust_sha" \
  'rust_worker_binary_identity_mismatch'
{
  printf 'pid=%s\nstarttime=%s\nexe=%s\nsha256=%s\n' \
    "$rust_pid" "$rust_starttime" "$rust_exe" "$rust_sha"
} >"$EVIDENCE/rust-process-identity.txt"

startup_ok=0
for _ in $(seq 1 120); do
  kill -0 "$rust_pid" 2>/dev/null || fail 'rust_worker_exited_during_startup'
  if [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
    http://10.213.80.90:20150/healthz 2>/dev/null)" == '200' ]] &&
    [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      http://10.213.80.90:20150/readyz 2>/dev/null)" == '200' ]] &&
    [[ "$(curl --silent --max-time 3 --output /dev/null --write-out '%{http_code}' \
      http://10.213.80.90:20151/metrics 2>/dev/null)" == '200' ]] &&
    "$BINARY" --context "$OPERATOR_CONTEXT" --output json worker get "$WORKER_ID" \
      >"$EVIDENCE/rust-worker-ready.json" 2>"$EVIDENCE/rust-worker-ready.stderr" &&
    jq -e '.id == 5 and .maintenance.enabled == true and .state == "maintenance" and
      .unreachable == false and .gpu_count == 2' \
      "$EVIDENCE/rust-worker-ready.json" >/dev/null; then
    startup_ok=1
    break
  fi
  sleep 1
done
require_equal "$startup_ok" '1' 'rust_worker_startup_gate_timeout'
event 'gate_passed' 'rust_worker_start'

phase='model_deploy_and_state_chain'
"$BINARY" --context "$OPERATOR_CONTEXT" --output json \
  worker maintenance disable "$WORKER_ID" --yes \
  >"$EVIDENCE/maintenance-disable.json"
jq -e '.worker_id == 5 and .after.enabled == false' \
  "$EVIDENCE/maintenance-disable.json" >/dev/null || fail 'maintenance_disable_not_confirmed'

schedulable_ok=0
ready_samples=0
for _ in $(seq 1 180); do
  if "$BINARY" --context "$OPERATOR_CONTEXT" --output json \
    worker get "$WORKER_ID" >"$EVIDENCE/worker-after-maintenance.json" \
    2>"$EVIDENCE/worker-after-maintenance.stderr"; then
    jq -c '{observed_at:now|todateiso8601,id,state,unreachable,
      maintenance:.maintenance.enabled,gpu_count}' \
      "$EVIDENCE/worker-after-maintenance.json" \
      >>"$EVIDENCE/worker-ready-after-maintenance.ndjson"
    if jq -e '.id == 5 and .maintenance.enabled == false and .state == "ready" and
      .unreachable == false and .gpu_count == 2' \
      "$EVIDENCE/worker-after-maintenance.json" >/dev/null; then
      ready_samples=$((ready_samples + 1))
      if [[ "$ready_samples" -ge 2 ]]; then
        schedulable_ok=1
        break
      fi
    else
      ready_samples=0
    fi
  else
    ready_samples=0
  fi
  sleep 1
done
require_equal "$schedulable_ok" '1' 'worker_did_not_become_stably_ready'
event 'gate_passed' 'worker_stably_ready_after_maintenance_disable'

"$BINARY" --timeout 600 --context "$OPERATOR_CONTEXT" --output json \
  model deploy "$TARGET_MODEL" \
  --worker "$WORKER_NAME" \
  --local-path "$MODEL_PATH" \
  --image "$RUNNER_IMAGE" \
  --gpu 0 \
  --tensor-parallel-size 1 \
  --max-model-len 2048 \
  --gpu-memory-utilization 0.5 \
  --dtype auto \
  --enforce-eager \
  --wait \
  >"$EVIDENCE/model-deploy.json" \
  2> >(while IFS= read -r line; do
    printf '%s\t%s\n' "$(timestamp)" "$line"
  done >>"$EVIDENCE/model-deploy-transitions.log")

model_id="$(jq -r '.model.id' "$EVIDENCE/model-deploy.json")"
instance_id="$(jq -r '.instances[0].id' "$EVIDENCE/model-deploy.json")"
assigned_port="$(jq -r '.instances[0].port' "$EVIDENCE/model-deploy.json")"
[[ "$model_id" =~ ^[1-9][0-9]*$ ]] || fail 'model_id_invalid'
[[ "$instance_id" =~ ^[1-9][0-9]*$ ]] || fail 'instance_id_invalid'
[[ "$assigned_port" -ge 40000 && "$assigned_port" -le 40063 ]] ||
  fail 'assigned_port_outside_managed_pool'
mapfile -t observed_states < <(jq -r '.transitions[].state' "$EVIDENCE/model-deploy.json")
required_states=(pending analyzing scheduled initializing downloading starting running)
required_index=0
for observed_state in "${observed_states[@]}"; do
  if [[ "$required_index" -lt "${#required_states[@]}" ]] &&
    [[ "$observed_state" == "${required_states[$required_index]}" ]]; then
    required_index=$((required_index + 1))
  fi
done
require_equal "$required_index" "${#required_states[@]}" 'complete_state_chain_missing'
jq -e '(.instances | length) == 1 and .instances[0].state == "running" and
  .instances[0].worker_id == 5' \
  "$EVIDENCE/model-deploy.json" >/dev/null || fail 'running_instance_contract_mismatch'
state_chain_complete=1

"$BINARY" --context "$OPERATOR_CONTEXT" --output json model file list \
  --worker "$WORKER_ID" >"$EVIDENCE/model-files-running.json"
model_file_id="$(jq -r --arg path "$MODEL_PATH" \
  '[.[] | select(.worker_id == 5 and .source == "local_path" and .local_path == $path)] |
   if length == 1 then .[0].id else empty end' "$EVIDENCE/model-files-running.json")"
[[ "$model_file_id" =~ ^[1-9][0-9]*$ ]] || fail 'model_file_identity_missing'
jq -e --arg path "$MODEL_PATH" --argjson bytes "$EXPECTED_MODEL_BYTES" '
  [.[] | select(.worker_id == 5 and .source == "local_path" and
    .local_path == $path and .state == "ready" and .download_progress == 100 and
    .size == $bytes and .resolved_paths == [$path] and
    ((.cleanup_on_delete == null) or (.cleanup_on_delete == false)))] | length == 1
' "$EVIDENCE/model-files-running.json" >/dev/null || fail 'model_file_ready_contract_mismatch'

require_equal "$(owned_container_count)" '1' 'owned_container_count_not_one'
owned_container_id="$(owned_container_ids)"
docker inspect "$owned_container_id" >"$EVIDENCE/owned-container-running.json"
owned_spec_hash="$(docker inspect --format \
  '{{ index .Config.Labels "io.gpustack.spec-hash" }}' "$owned_container_id")"
[[ "$owned_spec_hash" =~ ^[0-9a-f]{64}$ ]] || fail 'owned_spec_hash_invalid'
jq -e --arg uuid "$WORKER_UUID" --arg instance "$instance_id" --arg hash "$owned_spec_hash" '
  .[0].Config.Labels as $labels |
  $labels["io.gpustack.implementation"] == "rust" and
  $labels["io.gpustack.schema-version"] == "2" and
  $labels["io.gpustack.mode"] == "managed" and
  $labels["io.gpustack.owner-id"] == $uuid and
  $labels["io.gpustack.component"] == "vllm" and
  $labels["io.gpustack.worker-uuid"] == $uuid and
  $labels["io.gpustack.model-instance-id"] == $instance and
  $labels["io.gpustack.spec-hash"] == $hash
' "$EVIDENCE/owned-container-running.json" >/dev/null || fail 'owned_label_tuple_mismatch'
require_equal "$(docker inspect --format '{{.Image}}' "$owned_container_id")" \
  "$RUNNER_IMAGE_ID" 'owned_container_image_mismatch'
require_equal "$(docker inspect --format '{{.State.Running}}' "$owned_container_id")" \
  'true' 'owned_container_not_running'
require_equal "$(listener_count "$assigned_port")" '1' 'managed_listener_missing'

runtime_running_line="$(grep 'outcome=running' "$EVIDENCE/rust-events.log" | head -n 1)"
health_ready_unix_nanos="$(sed -n \
  's/.*health_ready_at_unix_nanos=\([0-9][0-9]*\).*/\1/p' <<<"$runtime_running_line")"
server_running_ts="$(grep 'state=running' "$EVIDENCE/model-deploy-transitions.log" |
  head -n 1 | cut -f1)"
[[ -n "$health_ready_unix_nanos" && -n "$server_running_ts" ]] ||
  fail 'running_timestamps_missing'
server_running_unix_nanos="$(date -d "$server_running_ts" +%s%N)"
[[ "$health_ready_unix_nanos" -le "$server_running_unix_nanos" ]] ||
  fail 'server_running_preceded_runtime_health'
{
  printf 'runtime_health_ready_unix_nanos=%s\nserver_running_observed=%s\n' \
    "$health_ready_unix_nanos" "$server_running_ts"
  printf 'server_running_observed_unix_nanos=%s\n' "$server_running_unix_nanos"
} >"$EVIDENCE/health-before-running.txt"
event 'state_chain_complete' "model_id=$model_id instance_id=$instance_id port=$assigned_port"

phase='server_only_inference_and_logs'
"$BINARY" --context "$OPERATOR_CONTEXT" --output json inference models \
  >"$EVIDENCE/inference-models.json"
jq -e --arg name "$TARGET_MODEL" \
  '[.data[] | select(.id == $name)] | length == 1' \
  "$EVIDENCE/inference-models.json" >/dev/null || fail 'inference_model_route_missing'

"$BINARY" --timeout 120 --context "$OPERATOR_CONTEXT" --output json \
  inference chat --model "$TARGET_MODEL" --prompt 'Reply with exactly: rust worker ready' \
  --max-tokens 16 >"$EVIDENCE/chat-buffered.json"
jq -e '.choices[0].message.content | type == "string" and length > 0' \
  "$EVIDENCE/chat-buffered.json" >/dev/null || fail 'buffered_chat_empty'

"$BINARY" --timeout 120 --context "$OPERATOR_CONTEXT" --output json \
  inference chat --model "$TARGET_MODEL" --prompt 'Count from one to three.' \
  --max-tokens 24 --stream >"$EVIDENCE/chat-stream.sse"
grep -Eq '^data: \{.*"choices"' "$EVIDENCE/chat-stream.sse" || fail 'stream_chat_data_missing'
grep -Eq '^data: \[DONE\]' "$EVIDENCE/chat-stream.sse" || fail 'stream_chat_done_missing'

"$BINARY" --timeout 120 --context "$OPERATOR_CONTEXT" --output json \
  inference completion --model "$TARGET_MODEL" --prompt 'Rust is' --max-tokens 16 \
  >"$EVIDENCE/completion.json"
jq -e '.choices[0].text | type == "string" and length > 0' \
  "$EVIDENCE/completion.json" >/dev/null || fail 'completion_empty'

set +o errexit
"$BINARY" --timeout 30 --context "$OPERATOR_CONTEXT" --output json \
  inference chat --model m6b5-nonexistent-model --prompt denied --max-tokens 1 \
  >"$EVIDENCE/invalid-model.stdout" 2>"$EVIDENCE/invalid-model.stderr"
invalid_model_exit=$?
set -o errexit
[[ "$invalid_model_exit" -ne 0 ]] || fail 'invalid_model_was_not_rejected'
printf 'exit_code=%s\n' "$invalid_model_exit" >"$EVIDENCE/invalid-model-result.txt"

"$BINARY" --timeout 60 --context "$OPERATOR_CONTEXT" model logs "$model_id" --tail 40 \
  >"$EVIDENCE/model-logs-tail.txt"
[[ -s "$EVIDENCE/model-logs-tail.txt" ]] || fail 'model_logs_tail_empty'
set +o errexit
timeout --signal=TERM --kill-after=10 8 \
  "$BINARY" --timeout 60 --context "$OPERATOR_CONTEXT" model logs "$model_id" \
  --tail 2 --follow >"$EVIDENCE/model-logs-follow.txt" \
  2>"$EVIDENCE/model-logs-follow.stderr"
logs_follow_exit=$?
set -o errexit
require_equal "$logs_follow_exit" '124' 'model_logs_follow_not_bounded_by_timeout'
[[ -s "$EVIDENCE/model-logs-follow.txt" ]] || fail 'model_logs_follow_empty'
inference_passed=1
event 'gate_passed' 'server_only_inference_and_logs'

phase='same_process_idempotency'
for sample in 1 2 3; do
  sleep 2
  kill -0 "$rust_pid" 2>/dev/null || fail 'rust_worker_exited_during_idempotency'
  "$BINARY" --context "$OPERATOR_CONTEXT" --output json model status "$model_id" \
    >"$EVIDENCE/idempotency-status-${sample}.json"
  jq -e --argjson instance "$instance_id" --argjson port "$assigned_port" '
    (.instances | length) == 1 and .instances[0].id == $instance and
    .instances[0].state == "running" and .instances[0].port == $port
  ' "$EVIDENCE/idempotency-status-${sample}.json" >/dev/null ||
    fail "idempotency_server_identity_drift_${sample}"
  require_equal "$(owned_container_count)" '1' "idempotency_container_count_${sample}"
  require_equal "$(owned_container_ids)" "$owned_container_id" \
    "idempotency_container_identity_${sample}"
  require_equal "$(docker inspect --format \
    '{{ index .Config.Labels "io.gpustack.spec-hash" }}' "$owned_container_id")" \
    "$owned_spec_hash" "idempotency_spec_hash_${sample}"
  require_equal "$(listener_count "$assigned_port")" '1' "idempotency_listener_${sample}"
done
require_equal "$(find "$LOG_DIR/serve" -maxdepth 1 -type f \
  -name "${instance_id}.*.log" 2>/dev/null | wc -l)" '1' 'duplicate_log_session'
"$BINARY" --timeout 120 --context "$OPERATOR_CONTEXT" --output json \
  inference chat --model "$TARGET_MODEL" --prompt 'Reply: stable' --max-tokens 8 \
  >"$EVIDENCE/chat-after-idempotency.json"
jq -e '.choices[0].message.content | type == "string" and length > 0' \
  "$EVIDENCE/chat-after-idempotency.json" >/dev/null || fail 'route_failed_after_idempotency'
idempotency_passed=1
smoke_passed=1
failure_reason=''
event 'smoke_gate_passed' 'managed_runtime_server_inference_and_idempotency'
phase='smoke_passed_pending_cleanup'
exit 0
