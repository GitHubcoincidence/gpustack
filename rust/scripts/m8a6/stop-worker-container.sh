#!/usr/bin/env bash
# Stops and removes only the exact Rust Worker container identity.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

require_env GPUSTACK_RS_EXPECTED_IMAGE_ID
require_env GPUSTACK_RS_EXPECTED_GIT_COMMIT

readonly EXPECTED_IMAGE_ID="$GPUSTACK_RS_EXPECTED_IMAGE_ID"
readonly EXPECTED_COMMIT="$GPUSTACK_RS_EXPECTED_GIT_COMMIT"
readonly DOCKER_BIN="${GPUSTACK_DOCKER_BIN:-/usr/bin/docker}"

require_image_id "$EXPECTED_IMAGE_ID"
require_lower_hex_commit "$EXPECTED_COMMIT"
require_executable "$DOCKER_BIN"

container_ids="$(
  "$DOCKER_BIN" container ls --all \
    --filter "name=^/${M8A6_CONTAINER_NAME}$" \
    --format '{{.ID}}'
)" || fail 'container lookup failed'

if [[ -z "$container_ids" ]]; then
  printf 'container=%s\nstatus=absent\n' "$M8A6_CONTAINER_NAME"
  exit 0
fi
[[ "$container_ids" != *$'\n'* ]] || fail 'more than one container matched the protected name'

identity="$(
  "$DOCKER_BIN" container inspect \
    --format '{{.Id}}|{{.Image}}|{{.Name}}|{{index .Config.Labels "io.gpustack.component"}}|{{index .Config.Labels "io.gpustack.implementation"}}|{{index .Config.Labels "org.opencontainers.image.revision"}}|{{index .Config.Labels "io.gpustack.worker.container-name"}}|{{index .Config.Labels "io.gpustack.mode"}}|{{index .Config.Labels "io.gpustack.worker-uuid"}}|{{index .Config.Labels "io.gpustack.model-instance-id"}}|{{index .Config.Labels "io.gpustack.spec-hash"}}' \
    "$container_ids"
)" || fail 'candidate container cannot be inspected'

IFS='|' read -r container_id image_id container_name component implementation revision name_label mode worker_uuid instance_id spec_hash <<<"$identity"
mode="$(normalize_optional_label "$mode")"
worker_uuid="$(normalize_optional_label "$worker_uuid")"
instance_id="$(normalize_optional_label "$instance_id")"
spec_hash="$(normalize_optional_label "$spec_hash")"

[[ "$container_id" =~ ^[0-9a-f]{64}$ ]] || fail 'candidate container ID is invalid'
[[ "$image_id" == "$EXPECTED_IMAGE_ID" ]] || fail 'candidate container image ID does not match'
[[ "$container_name" == "/$M8A6_CONTAINER_NAME" ]] || fail 'candidate container name does not match'
[[ "$component" == "$M8A6_COMPONENT" && "$implementation" == "$M8A6_IMPLEMENTATION" ]] || fail 'candidate container component labels do not match'
[[ "$revision" == "$EXPECTED_COMMIT" ]] || fail 'candidate container revision does not match'
[[ "$name_label" == "$M8A6_CONTAINER_NAME" ]] || fail 'candidate container name label does not match'
[[ -z "$mode" && -z "$worker_uuid" && -z "$instance_id" && -z "$spec_hash" ]] || fail 'candidate carries Managed Runner ownership labels'

"$DOCKER_BIN" container stop --time 30 "$container_id" >/dev/null
running="$("$DOCKER_BIN" container inspect --format '{{.State.Running}}' "$container_id")" || fail 'stopped container cannot be inspected'
[[ "$running" == 'false' ]] || fail 'container remained running after graceful stop'
"$DOCKER_BIN" container rm "$container_id" >/dev/null

printf 'container=%s\ncontainer_id=%s\nstatus=removed\n' "$M8A6_CONTAINER_NAME" "$container_id"
