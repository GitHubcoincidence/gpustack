#!/usr/bin/env bash
# Builds one source-identified Rust Worker image without running it.
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# shellcheck source=common.sh
source "$SCRIPT_DIR/common.sh"

require_env GPUSTACK_RS_IMAGE
require_env GPUSTACK_RS_EXPECTED_HEAD

readonly IMAGE="$GPUSTACK_RS_IMAGE"
readonly EXPECTED_HEAD="$GPUSTACK_RS_EXPECTED_HEAD"
readonly DOCKER_BIN="${GPUSTACK_DOCKER_BIN:-/usr/bin/docker}"
readonly REPO="$(readlink -f -- "$SCRIPT_DIR/../../..")"
readonly DOCKERFILE="$REPO/rust/docker/worker/Dockerfile"

require_safe_image_reference "$IMAGE"
require_lower_hex_commit "$EXPECTED_HEAD"
require_executable "$DOCKER_BIN"
[[ "$(git -C "$REPO" rev-parse HEAD)" == "$EXPECTED_HEAD" ]] || fail 'repository HEAD does not match GPUSTACK_RS_EXPECTED_HEAD'
[[ -z "$(git -C "$REPO" status --porcelain=v1 --untracked-files=normal)" ]] || fail 'repository worktree must be clean before an image build'

"$DOCKER_BIN" build \
  --platform linux/amd64 \
  --file "$DOCKERFILE" \
  --build-arg "GPUSTACK_GIT_COMMIT=$EXPECTED_HEAD" \
  --tag "$IMAGE" \
  "$REPO/rust"

image_identity="$(
  "$DOCKER_BIN" image inspect \
    --format '{{.Id}}|{{index .Config.Labels "org.opencontainers.image.revision"}}|{{index .Config.Labels "io.gpustack.component"}}|{{index .Config.Labels "io.gpustack.implementation"}}|{{json .Config.Entrypoint}}|{{json .Config.Cmd}}' \
    "$IMAGE"
)" || fail 'built image cannot be inspected'

IFS='|' read -r image_id revision component implementation entrypoint command <<<"$image_identity"
require_image_id "$image_id"
[[ "$revision" == "$EXPECTED_HEAD" ]] || fail 'built image revision label does not match the source HEAD'
[[ "$component" == "$M8A6_COMPONENT" ]] || fail 'built image component label is invalid'
[[ "$implementation" == "$M8A6_IMPLEMENTATION" ]] || fail 'built image implementation label is invalid'
[[ "$entrypoint" == '["/usr/local/bin/gpustack-rs"]' ]] || fail 'built image entrypoint is invalid'
[[ "$command" == '["worker","run"]' ]] || fail 'built image command is invalid'

printf 'image=%s\nimage_id=%s\nsource_commit=%s\n' "$IMAGE" "$image_id" "$revision"
