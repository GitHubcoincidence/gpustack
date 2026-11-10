#!/usr/bin/env bash
# Starts the uniquely identified Rust Worker container with the frozen host contract.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

for required in \
  GPUSTACK_RS_IMAGE \
  GPUSTACK_RS_EXPECTED_IMAGE_ID \
  GPUSTACK_RS_EXPECTED_GIT_COMMIT \
  GPUSTACK_RS_CONFIG_PATH \
  GPUSTACK_RS_TOKEN_FILE \
  GPUSTACK_RS_DATA_DIR \
  GPUSTACK_RS_LOG_DIR \
  GPUSTACK_RS_MODEL_ROOT; do
  require_env "$required"
done

readonly IMAGE="$GPUSTACK_RS_IMAGE"
readonly EXPECTED_IMAGE_ID="$GPUSTACK_RS_EXPECTED_IMAGE_ID"
readonly EXPECTED_COMMIT="$GPUSTACK_RS_EXPECTED_GIT_COMMIT"
readonly DOCKER_BIN="${GPUSTACK_DOCKER_BIN:-/usr/bin/docker}"
readonly DOCKER_SOCKET="${GPUSTACK_RS_DOCKER_SOCKET:-/var/run/docker.sock}"
readonly HOST_UID="$(id -u)"
readonly HOST_GID="$(id -g)"

require_safe_image_reference "$IMAGE"
require_image_id "$EXPECTED_IMAGE_ID"
require_lower_hex_commit "$EXPECTED_COMMIT"
require_executable "$DOCKER_BIN"
[[ "$HOST_UID" != '0' ]] || fail 'the Rust Worker container must be launched by a non-root host user'

readonly CONFIG_PATH="$(canonical_path 'configuration file' "$GPUSTACK_RS_CONFIG_PATH")"
readonly TOKEN_FILE="$(canonical_path 'registration token file' "$GPUSTACK_RS_TOKEN_FILE")"
readonly DATA_DIR="$(canonical_path 'data directory' "$GPUSTACK_RS_DATA_DIR")"
readonly LOG_DIR="$(canonical_path 'log directory' "$GPUSTACK_RS_LOG_DIR")"
readonly MODEL_ROOT="$(canonical_path 'model root' "$GPUSTACK_RS_MODEL_ROOT")"
readonly SOCKET_PATH="$(canonical_path 'Docker socket' "$DOCKER_SOCKET")"

require_private_file 'configuration file' "$CONFIG_PATH" "$HOST_UID"
require_private_file 'registration token file' "$TOKEN_FILE" "$HOST_UID"
require_private_directory 'data directory' "$DATA_DIR" "$HOST_UID"
require_private_directory 'log directory' "$LOG_DIR" "$HOST_UID"
[[ -d "$MODEL_ROOT" && -r "$MODEL_ROOT" && -x "$MODEL_ROOT" ]] || fail 'model root must be a readable directory'
[[ -S "$SOCKET_PATH" ]] || fail 'Docker socket must be a Unix socket'
readonly SOCKET_GID="$(stat -c '%g' -- "$SOCKET_PATH")"

image_identity="$(
  "$DOCKER_BIN" image inspect \
    --format '{{.Id}}|{{index .Config.Labels "org.opencontainers.image.revision"}}|{{index .Config.Labels "io.gpustack.component"}}|{{index .Config.Labels "io.gpustack.implementation"}}|{{json .Config.Entrypoint}}|{{json .Config.Cmd}}' \
    "$IMAGE"
)" || fail 'candidate image cannot be inspected'
IFS='|' read -r actual_image_id revision component implementation entrypoint command <<<"$image_identity"
[[ "$actual_image_id" == "$EXPECTED_IMAGE_ID" ]] || fail 'candidate image ID does not match the expected image'
[[ "$revision" == "$EXPECTED_COMMIT" ]] || fail 'candidate image revision does not match the expected source commit'
[[ "$component" == "$M8A6_COMPONENT" && "$implementation" == "$M8A6_IMPLEMENTATION" ]] || fail 'candidate image labels are invalid'
[[ "$entrypoint" == '["/usr/local/bin/gpustack-rs"]' && "$command" == '["worker","run"]' ]] || fail 'candidate image startup contract is invalid'

existing="$(
  "$DOCKER_BIN" container ls --all \
    --filter "name=^/${M8A6_CONTAINER_NAME}$" \
    --format '{{.ID}}'
)" || fail 'existing container check failed'
[[ -z "$existing" ]] || fail "container already exists: $M8A6_CONTAINER_NAME"

container_id="$(
  "$DOCKER_BIN" run --detach \
    --name "$M8A6_CONTAINER_NAME" \
    --network host \
    --restart unless-stopped \
    --runtime nvidia \
    --gpus all \
    --user "$HOST_UID:$HOST_GID" \
    --group-add "$SOCKET_GID" \
    --read-only \
    --tmpfs '/tmp:rw,noexec,nosuid,nodev,mode=1777' \
    --cap-drop ALL \
    --security-opt no-new-privileges \
    --label "io.gpustack.component=$M8A6_COMPONENT" \
    --label "io.gpustack.implementation=$M8A6_IMPLEMENTATION" \
    --label "io.gpustack.worker.container-name=$M8A6_CONTAINER_NAME" \
    --label "org.opencontainers.image.revision=$EXPECTED_COMMIT" \
    --env 'NVIDIA_VISIBLE_DEVICES=all' \
    --env 'NVIDIA_DRIVER_CAPABILITIES=compute,utility' \
    --mount "type=bind,src=$CONFIG_PATH,dst=$CONFIG_PATH,readonly" \
    --mount "type=bind,src=$TOKEN_FILE,dst=$TOKEN_FILE,readonly" \
    --mount "type=bind,src=$DATA_DIR,dst=$DATA_DIR" \
    --mount "type=bind,src=$LOG_DIR,dst=$LOG_DIR" \
    --mount "type=bind,src=$MODEL_ROOT,dst=$MODEL_ROOT,readonly" \
    --mount "type=bind,src=$SOCKET_PATH,dst=$SOCKET_PATH" \
    "$IMAGE" \
    --config "$CONFIG_PATH" worker run
)" || fail 'Rust Worker container failed to start'

[[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || fail 'Docker did not return a full container ID'
printf 'container=%s\ncontainer_id=%s\nimage_id=%s\nsource_commit=%s\n' \
  "$M8A6_CONTAINER_NAME" "$container_id" "$EXPECTED_IMAGE_ID" "$EXPECTED_COMMIT"
