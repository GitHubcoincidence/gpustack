#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail

umask 077

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
readonly REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd -P)"
# shellcheck disable=SC1091
source "$REPO_ROOT/hack/lib/init.sh"

readonly TEMP_ROOT="$(mktemp -d /tmp/m8emb7-106-self-test.XXXXXX)"
cleanup() {
  if [[ -d "$TEMP_ROOT" && ! -L "$TEMP_ROOT" ]]; then
    find "$TEMP_ROOT" -type f -exec unlink {} \;
    find "$TEMP_ROOT" -depth -type d -exec rmdir {} +
  fi
}
trap cleanup EXIT

printf '[1/6] shell and Python syntax\n'
for script in "$SCRIPT_DIR"/*.sh; do
  bash -n "$script"
done
python3 - "$SCRIPT_DIR" <<'PY'
import sys
from pathlib import Path

for path in Path(sys.argv[1]).glob('*.py'):
    compile(path.read_text(encoding='utf-8'), str(path), 'exec')
PY

printf '[2/6] authorization is bound to the exact stage and evidence\n'
python3 - "$SCRIPT_DIR" "$TEMP_ROOT" <<'PY'
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import runtime_tool

root = Path(sys.argv[2])
authorization = root / 'authorization.json'
value = {
    'stage': 'M8-EMB.7-worker-embedding-diagnostic-window',
    'approved': True,
    'expected_head': '1' * 40,
    'preflight_manifest_sha256': '2' * 64,
    'nonce': 'offline-self-test',
}
authorization.write_text(json.dumps(value), encoding='utf-8')
authorization.chmod(0o600)
runtime_tool.validate_authorization(
    authorization,
    value['expected_head'],
    value['preflight_manifest_sha256'],
)
value['stage'] = 'wrong-stage'
authorization.write_text(json.dumps(value), encoding='utf-8')
try:
    runtime_tool.validate_authorization(
        authorization,
        value['expected_head'],
        value['preflight_manifest_sha256'],
    )
except runtime_tool.base.GateError:
    pass
else:
    raise SystemExit('wrong authorization stage was accepted')
PY

printf '[3/6] Server contract and Worker diagnostic sequences remain separate\n'
python3 - "$SCRIPT_DIR" "$TEMP_ROOT" <<'PY'
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[1])
import runtime_tool

path = Path(sys.argv[2]) / 'watch.body'
states = [
    'pending', 'analyzing', 'pending', 'scheduled', 'initializing',
    'downloading', 'starting', 'running',
]
lines = []
for state in states:
    lines.append('data: ' + json.dumps({'data': {'model_id': 7, 'state': state}}))
path.write_text('\n'.join(lines) + '\n', encoding='utf-8')
observed = runtime_tool.collect_instance_states(path, 7)
assert observed == states
classification = runtime_tool.require_diagnostic_state_sequence(observed)
assert classification['server_contract_complete'] is True
assert classification['worker_sequence_complete'] is True

known_gap = ['pending', 'scheduled', 'initializing', 'starting', 'running']
classification = runtime_tool.require_diagnostic_state_sequence(known_gap)
assert classification['worker_sequence_complete'] is True
assert classification['server_contract_complete'] is False
assert classification['missing_server_states'] == ['analyzing', 'downloading']
try:
    runtime_tool.require_diagnostic_state_sequence(
        ['pending', 'scheduled', 'starting', 'running']
    )
except runtime_tool.base.GateError:
    pass
else:
    raise SystemExit('incomplete Worker state sequence was accepted')
PY

printf '[4/6] exact orphan ModelFile database fallback\n'
python3 - "$SCRIPT_DIR" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import runtime_tool
from config import Config

config = Config()
model_file = {
    'id': 23,
    'worker_id': 418,
    'source': 'local_path',
    'local_path': config.model_path,
    'cleanup_on_delete': False,
    'owner_principal_id': config.database_owner_principal_id,
}
sql = runtime_tool.build_orphan_model_file_delete_sql(config, model_file)
assert 'id = 23' in sql
assert 'worker_id = 418' in sql
assert 'modelinstancemodelfilelink' in sql
assert 'modelinstancedraftmodelfilelink' in sql
assert "SELECT 'deleted_count=' || count(*)" in sql

model_file['local_path'] = '/unexpected/path'
try:
    runtime_tool.build_orphan_model_file_delete_sql(config, model_file)
except runtime_tool.base.GateError:
    pass
else:
    raise SystemExit('unexpected ModelFile path was accepted')
PY

printf '[5/6] frozen command and stage-specific hooks\n'
python3 - "$SCRIPT_DIR" <<'PY'
import sys

sys.path.insert(0, sys.argv[1])
import runtime_tool
from config import Config

config = Config()
command = config.worker_command()
assert command[-1] == 'run'
assert '--worker-name' in command
assert config.worker_name in command
assert config.target_model == 'm8emb7-bge-small-zh-106-attempt3'
preflight_source = (runtime_tool.SCRIPT_DIR / 'preflight.py').read_text(
    encoding='utf-8'
)
assert 'target_model_rows=0' in preflight_source
assert runtime_tool.os.environ['M8EMB6106_REPO'] == str(config.repo)
assert runtime_tool.base.api_request is runtime_tool.api_request
assert runtime_tool.base.set_maintenance is runtime_tool.set_maintenance
assert runtime_tool.base.deploy_and_validate is runtime_tool.deploy_and_validate
assert runtime_tool.base.delete_model_file_metadata is (
    runtime_tool.delete_model_file_metadata
)
assert runtime_tool.base.post_audit_extra is runtime_tool.post_audit_extra
assert runtime_tool.base.AUTHORIZATION_STAGE == (
    'M8-EMB.7-worker-embedding-diagnostic-window'
)
assert runtime_tool.base.FORMAL_STAGE == 'M8-EMB.7-worker-embedding-diagnostic'
assert runtime_tool.base.CLEANUP_STAGE == (
    'M8-EMB.7-worker-embedding-diagnostic-cleanup'
)
assert runtime_tool.base.AUDIT_STAGE == (
    'M8-EMB.7-worker-embedding-diagnostic-post-audit'
)
parser = runtime_tool.base.build_parser()
arguments = parser.parse_args(
    [
        'post-audit', '--preflight', 'p', '--preflight-sha', 's',
        '--expected-head', 'h', '--evidence', 'e', '--formal-evidence', 'f',
    ]
)
assert arguments.formal_evidence == 'f'
PY

printf '[6/6] no broad destructive operation\n'
if grep -REn --exclude='self-test.sh' -- \
  'docker system prune|docker volume prune|--privileged|rm -rf' "$SCRIPT_DIR"; then
  printf 'STOP: tool contains a broad destructive operation\n' >&2
  exit 1
fi
grep -q 'bash "$SCRIPT_DIR/executor.sh"' "$SCRIPT_DIR/watchdog.sh"
grep -q 'bash "$SCRIPT_DIR/cleanup.sh"' "$SCRIPT_DIR/watchdog.sh"
printf 'm8emb7_106_toolchain_self_test=PASS\n'
