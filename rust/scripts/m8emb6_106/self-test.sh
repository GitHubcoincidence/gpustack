#!/usr/bin/env bash
# Offline refusal and identity tests for the 106 preparation/preflight tools.
# shellcheck disable=SC1091
set -o errexit -o errtrace -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
source "$SCRIPT_DIR/common.sh"

readonly TEMP_ROOT="$(mktemp -d /tmp/m8emb6-106-self-test.XXXXXX)"
cleanup() {
  if [[ -d "$TEMP_ROOT" && ! -L "$TEMP_ROOT" ]]; then
    find "$TEMP_ROOT" -type f -exec unlink {} \; 2>/dev/null || true
    find "$TEMP_ROOT" -depth -type d -exec rmdir {} + 2>/dev/null || true
  fi
}
trap cleanup EXIT

printf '[1/7] shell and Python syntax\n'
source /home/wujiaxing/.cargo/env
command -v cargo >/dev/null
for script in "$SCRIPT_DIR"/*.sh; do
  bash -n "$script"
done
python3 - "$M8EMB6106_JSON_TOOL" "$SCRIPT_DIR/runtime_tool.py" <<'PY'
import sys

for path in sys.argv[1:]:
    with open(path, encoding='utf-8') as stream:
        compile(stream.read(), path, 'exec')
PY

printf '[2/7] preparation tools contain no service mutation\n'
if grep -REn -- \
  'docker (rm|stop|start)|worker maintenance|model deploy|kill -|--privileged|rm -rf' \
  "$SCRIPT_DIR/common.sh" "$SCRIPT_DIR/prepare-model.sh" \
  "$SCRIPT_DIR/prepare-contexts.sh" "$SCRIPT_DIR/preflight.sh"; then
  printf 'STOP: preparation/preflight contains a forbidden mutation\n' >&2
  exit 1
fi

printf '[3/7] process identity accepts deleted executable and rejects drift\n'
install -m 0700 /bin/sleep "$TEMP_ROOT/deleted-sleep"
"$TEMP_ROOT/deleted-sleep" 30 &
test_pid=$!
unlink "$TEMP_ROOT/deleted-sleep"
m8emb6106_record_process_identity "$test_pid" deleted_executable \
  "$TEMP_ROOT/process.txt"
m8emb6106_process_identity_matches "$TEMP_ROOT/process.txt"
sed 's/^starttime=.*/starttime=1/' "$TEMP_ROOT/process.txt" \
  >"$TEMP_ROOT/process-invalid.txt"
if m8emb6106_process_identity_matches "$TEMP_ROOT/process-invalid.txt"; then
  printf 'STOP: invalid process identity was accepted\n' >&2
  exit 1
fi
kill -TERM "$test_pid"
wait "$test_pid" 2>/dev/null || true

printf '[4/7] argv sanitization removes only obsolete model-root\n'
printf '%s\0' /old/gpustack-rs --output json worker --server-url \
  http://127.0.0.1:18091/ --model-root /home/wujiaxing/models run \
  >"$TEMP_ROOT/cmdline.bin"
python3 "$M8EMB6106_JSON_TOOL" sanitize-argv \
  --file "$TEMP_ROOT/cmdline.bin" --binary "$M8EMB6106_BINARY" \
  --output "$TEMP_ROOT/argv.json"
python3 - "$TEMP_ROOT/argv.json" "$M8EMB6106_BINARY" <<'PY'
import json
import sys

with open(sys.argv[1], encoding='utf-8') as stream:
    value = json.load(stream)
assert value['argv'][0] == sys.argv[2]
assert '--model-root' not in value['argv']
assert value['argv'][-1] == 'run'
assert value['removed_model_root'] == 1
PY

printf '[5/7] missing maintenance authorization is rejected\n'
python3 - "$SCRIPT_DIR/runtime_tool.py" "$TEMP_ROOT/missing.json" <<'PY'
import importlib.util
import sys
from pathlib import Path

spec = importlib.util.spec_from_file_location('runtime_tool', sys.argv[1])
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
try:
    module.validate_authorization(Path(sys.argv[2]), '0' * 40, '0' * 64)
except module.GateError:
    pass
else:
    raise SystemExit('missing authorization was accepted')
PY

printf '[6/7] formal tools contain no broad destructive command\n'
if grep -REn -- 'docker system prune|docker volume prune|--privileged|rm -rf' \
  "$SCRIPT_DIR/runtime_tool.py" "$SCRIPT_DIR/executor.sh" \
  "$SCRIPT_DIR/cleanup.sh" "$SCRIPT_DIR/watchdog.sh"; then
  printf 'STOP: formal tools contain a broad destructive command\n' >&2
  exit 1
fi

printf '[7/7] fixed model and evidence identity\n'
m8emb6106_verify_model
install -d -m 0700 "$TEMP_ROOT/evidence"
printf 'payload\n' >"$TEMP_ROOT/evidence/value.txt"
m8emb6106_seal_directory "$TEMP_ROOT/evidence"
(cd "$TEMP_ROOT/evidence" && sha256sum --check manifest.sha256) >/dev/null
printf 'm8emb6_106_toolchain_self_test=PASS\n'
