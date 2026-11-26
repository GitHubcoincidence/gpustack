#!/usr/bin/env python3
"""Formal execution, exact cleanup, and audit for M8-EMB.6 on host 106."""

import argparse
import base64
import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple


class GateError(RuntimeError):
    """A fail-closed gate failure with a user-readable reason."""


AUTHORIZATION_STAGE = 'M8-EMB.6-106-maintenance-window'
MAINTENANCE_MESSAGE = 'M8-EMB.6 106 compatibility window'
FORMAL_STAGE = 'M8-EMB.6C-106-formal'
CLEANUP_STAGE = 'M8-EMB.6C-106-cleanup'
AUDIT_STAGE = 'M8-EMB.6D-106-post-audit'


def require(condition: bool, message: str) -> None:
    if not condition:
        raise GateError(message)


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def load_json(path: Path) -> Any:
    with path.open(encoding='utf-8') as stream:
        return json.load(stream)


def write_json(path: Path, value: Any) -> None:
    path.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8'
    )
    path.chmod(0o600)


def private_file(path: Path) -> None:
    require(path.is_file() and not path.is_symlink(), f'protected file missing: {path}')
    metadata = path.stat()
    require(metadata.st_uid == os.getuid(), f'protected file owner invalid: {path}')
    require(stat.S_IMODE(metadata.st_mode) == 0o600, f'protected mode invalid: {path}')
    require(metadata.st_nlink == 1, f'protected file link count invalid: {path}')


def private_text(path: Path) -> str:
    private_file(path)
    value = path.read_text(encoding='utf-8').strip()
    require(bool(value), f'protected file is empty: {path}')
    return value


class Config:
    """Frozen values exported by common.sh."""

    def __init__(self) -> None:
        keys = [
            'REPO',
            'BINARY',
            'PYTHON_SERVER_URL',
            'RUST_SERVER_URL',
            'ORIGINAL_WORKER_NAME',
            'ORIGINAL_WORKER_ID',
            'ORIGINAL_MODEL_NAME',
            'ORIGINAL_MODEL_ID',
            'ORIGINAL_INSTANCE_ID',
            'ORIGINAL_WORKER_PORT',
            'ORIGINAL_METRICS_PORT',
            'ORIGINAL_MODEL_PORT',
            'WORKER_NAME',
            'WORKER_HOSTNAME',
            'WORKER_IP',
            'WORKER_IFNAME',
            'WORKER_PORT',
            'METRICS_PORT',
            'TARGET_MODEL',
            'MODEL_PATH',
            'EMBEDDING_DIMENSION',
            'RUNNER_IMAGE',
            'RUNNER_IMAGE_ID',
            'GPU0_UUID',
            'CONTEXTS_FILE',
            'PYTHON_CONTEXT',
            'RUST_CONTEXT',
            'PYTHON_OPERATOR_TOKEN',
            'RUST_OPERATOR_TOKEN',
            'CLUSTER_TOKEN',
            'DATA_DIR',
            'LOG_DIR',
        ]
        for key in keys:
            environment_key = f'M8EMB6106_{key}'
            value = os.environ.get(environment_key)
            require(bool(value), f'missing environment value: {environment_key}')
            setattr(self, key.lower(), value)

        self.repo = Path(self.repo)
        self.binary = Path(self.binary)
        self.contexts_file = Path(self.contexts_file)
        self.python_operator_token = Path(self.python_operator_token)
        self.rust_operator_token = Path(self.rust_operator_token)
        self.cluster_token = Path(self.cluster_token)
        self.data_dir = Path(self.data_dir)
        self.log_dir = Path(self.log_dir)
        self.original_worker_id = int(self.original_worker_id)
        self.original_model_id = int(self.original_model_id)
        self.original_instance_id = int(self.original_instance_id)
        self.embedding_dimension = int(self.embedding_dimension)

    def worker_command(self) -> List[str]:
        return [
            str(self.binary),
            '--output',
            'json',
            'worker',
            '--server-url',
            self.python_server_url,
            '--worker-name',
            self.worker_name,
            '--worker-hostname',
            self.worker_hostname,
            '--worker-ip',
            self.worker_ip,
            '--worker-ifname',
            self.worker_ifname,
            '--worker-port',
            self.worker_port,
            '--metrics-port',
            self.metrics_port,
            '--token-file',
            str(self.cluster_token),
            '--data-dir',
            str(self.data_dir),
            '--log-dir',
            str(self.log_dir),
            '--docker-socket',
            '/var/run/docker.sock',
            'run',
        ]


class Evidence:
    """Protected evidence directory with mirrored console output."""

    def __init__(self, path: Path) -> None:
        require(not path.exists() and not path.is_symlink(), f'evidence exists: {path}')
        path.mkdir(parents=True, mode=0o700)
        path.chmod(0o700)
        self.path = path
        self.console = path / 'console.log'

    def log(self, message: str) -> None:
        print(message, flush=True)
        with self.console.open('a', encoding='utf-8') as stream:
            stream.write(message + '\n')

    def json(self, name: str, value: Any) -> None:
        write_json(self.path / name, value)

    def text(self, name: str, value: str) -> None:
        path = self.path / name
        path.write_text(value, encoding='utf-8')
        path.chmod(0o600)

    def run(
        self,
        name: str,
        command: List[str],
        timeout: int = 120,
        check: bool = True,
        input_data: Optional[bytes] = None,
    ) -> subprocess.CompletedProcess[bytes]:
        result = subprocess.run(
            command,
            cwd=os.environ['M8EMB6106_REPO'],
            env=os.environ.copy(),
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            input=input_data,
            timeout=timeout,
            check=False,
        )
        (self.path / f'{name}.stdout').write_bytes(result.stdout)
        (self.path / f'{name}.stderr').write_bytes(result.stderr)
        if check and result.returncode != 0:
            raise GateError(f'command failed: {name}, exit={result.returncode}')
        return result

    def seal(self) -> str:
        matches = []
        patterns = [
            re.compile(rb'gpustack_[A-Za-z0-9_]{20,}'),
            re.compile(rb'Authorization:\s*Bearer'),
            re.compile(rb'BEGIN [A-Z ]*PRIVATE KEY'),
        ]
        for path in self.path.rglob('*'):
            if not path.is_file() or path.name == 'manifest.sha256':
                continue
            data = path.read_bytes()
            if any(pattern.search(data) for pattern in patterns):
                matches.append(str(path.relative_to(self.path)))
            path.chmod(0o600)
        require(not matches, f'evidence secret scan failed: {matches}')
        manifest = self.path / 'manifest.sha256'
        lines = []
        for path in sorted(self.path.rglob('*')):
            if path.is_file() and path != manifest:
                relative = path.relative_to(self.path).as_posix()
                lines.append(f'{file_sha256(path)}  {relative}\n')
        manifest.write_text(''.join(lines), encoding='utf-8')
        manifest.chmod(0o600)
        return file_sha256(manifest)


def verify_manifest(root: Path, expected_sha: str) -> None:
    require(root.is_dir() and not root.is_symlink(), 'preflight directory unavailable')
    manifest = root / 'manifest.sha256'
    require(file_sha256(manifest) == expected_sha, 'preflight manifest SHA drifted')
    for line in manifest.read_text(encoding='utf-8').splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  (.+)', line)
        require(match is not None, 'preflight manifest line is invalid')
        path = root / match.group(2)
        require(path.is_file() and not path.is_symlink(), 'preflight file unavailable')
        require(file_sha256(path) == match.group(1), 'preflight evidence drifted')


def api_request(
    method: str,
    url: str,
    token_file: Path,
    body: Optional[Any] = None,
) -> Tuple[int, Any]:
    token = private_text(token_file)
    data = None
    headers = {'Authorization': f'Bearer {token}'}
    if body is not None:
        data = json.dumps(body).encode('utf-8')
        headers['Content-Type'] = 'application/json'
    request = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = response.read()
            status = response.status
    except urllib.error.HTTPError as error:
        payload = error.read()
        status = error.code
    finally:
        token = ''
    if not payload:
        return status, None
    try:
        return status, json.loads(payload)
    except json.JSONDecodeError:
        return status, payload.decode('utf-8', errors='replace')


def resource_items(value: Any) -> List[Dict[str, Any]]:
    if isinstance(value, list):
        return value
    if isinstance(value, dict):
        for key in ('items', 'data'):
            found = value.get(key)
            if isinstance(found, list):
                return found
    raise GateError('resource list response is malformed')


def cli_json(
    config: Config,
    evidence: Evidence,
    name: str,
    context: str,
    arguments: List[str],
    timeout: int = 120,
) -> Any:
    command = [
        str(config.binary),
        '--timeout',
        str(timeout),
        '--context',
        context,
        '--output',
        'json',
        *arguments,
    ]
    result = evidence.run(name, command, timeout=timeout + 10)
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise GateError(f'CLI output is not JSON: {name}') from error


def cli_json_retry(
    config: Config,
    evidence: Evidence,
    name: str,
    context: str,
    arguments: List[str],
    attempts: int = 12,
    request_timeout: int = 20,
    delay: int = 5,
) -> Any:
    command = [
        str(config.binary),
        '--timeout',
        str(request_timeout),
        '--context',
        context,
        '--output',
        'json',
        *arguments,
    ]
    exit_codes = []
    for attempt in range(1, attempts + 1):
        result = evidence.run(
            f'{name}-attempt-{attempt}',
            command,
            timeout=request_timeout + 10,
            check=False,
        )
        exit_codes.append(result.returncode)
        if result.returncode == 0:
            try:
                value = json.loads(result.stdout)
            except json.JSONDecodeError as error:
                raise GateError(f'CLI output is not JSON: {name}') from error
            evidence.json(f'{name}.json', value)
            return value
        if attempt < attempts:
            time.sleep(delay)
    raise GateError(f'CLI route did not become ready: {name}, exits={exit_codes}')


def set_maintenance(
    config: Config,
    evidence: Evidence,
    context: str,
    worker_id: int,
    enabled: bool,
    name: str,
) -> None:
    if enabled:
        arguments = [
            'worker',
            'maintenance',
            'enable',
            str(worker_id),
            '--message',
            MAINTENANCE_MESSAGE,
        ]
    else:
        arguments = [
            'worker',
            'maintenance',
            'disable',
            str(worker_id),
            '--yes',
        ]
    response = cli_json(config, evidence, name, context, arguments)
    after = response.get('after') if isinstance(response, dict) else None
    require(isinstance(after, dict), 'maintenance response is malformed')
    require(after.get('enabled') is enabled, 'maintenance state did not converge')


def process_starttime(pid: int) -> str:
    return Path(f'/proc/{pid}/stat').read_text().split()[21]


def process_state(pid: int) -> str:
    raw = Path(f'/proc/{pid}/stat').read_text(encoding='utf-8')
    command_end = raw.rfind(')')
    require(command_end > 0, f'process stat is malformed: {pid}')
    fields = raw[command_end + 2 :].split()
    require(bool(fields), f'process state is unavailable: {pid}')
    return fields[0]


def process_identity(pid: int) -> Dict[str, Any]:
    proc = Path(f'/proc/{pid}')
    require(proc.exists(), f'process is unavailable: {pid}')
    return {
        'pid': pid,
        'starttime': process_starttime(pid),
        'exe_sha256': file_sha256(proc / 'exe'),
        'cmdline_sha256': file_sha256(proc / 'cmdline'),
    }


def process_matches(expected: Dict[str, Any]) -> bool:
    try:
        actual = process_identity(int(expected['pid']))
    except (GateError, FileNotFoundError, ProcessLookupError):
        return False
    return all(str(actual[key]) == str(expected[key]) for key in actual)


def wait_process_exit(pid: int, timeout: int) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if not Path(f'/proc/{pid}').exists():
            return
        if process_state(pid) == 'Z':
            try:
                os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                pass
            return
        time.sleep(0.5)
    raise GateError(f'process did not exit: {pid}')


def docker_inspect(container_id: str) -> Dict[str, Any]:
    result = subprocess.run(
        ['docker', 'inspect', container_id],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    require(result.returncode == 0, f'container unavailable: {container_id}')
    value = json.loads(result.stdout)
    require(isinstance(value, list) and len(value) == 1, 'Docker inspect is malformed')
    return value[0]


def validate_original_container(
    container: Dict[str, Any], plan: Dict[str, Any]
) -> None:
    require(
        container.get('Id') == plan['container_id'], 'original container ID drifted'
    )
    require(
        container.get('Image') == plan['container_image_id'], 'Runner image drifted'
    )
    labels = container.get('Config', {}).get('Labels') or {}
    require(
        labels.get('io.gpustack.worker-uuid') == plan['worker_uuid'],
        'original container Worker UUID drifted',
    )
    require(
        labels.get('io.gpustack.model-instance-id') == str(plan['instance_id']),
        'original container ModelInstance drifted',
    )
    require(
        labels.get('io.gpustack.spec-hash') == plan['container_spec_hash'],
        'original container spec hash drifted',
    )
    requests = container.get('HostConfig', {}).get('DeviceRequests') or []
    require(
        len(requests) == 1 and requests[0].get('DeviceIDs') == [plan['gpu_uuid']],
        'original container GPU assignment drifted',
    )


def health_status(url: str, timeout: int = 10) -> int:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            response.read(1)
            return response.status
    except (urllib.error.URLError, TimeoutError):
        return 0


def wait_health(url: str, expected: int, timeout: int) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if health_status(url, 3) == expected:
            return
        time.sleep(1)
    raise GateError(f'health endpoint did not converge: {url}')


def gpu_process_count(gpu_uuid: str) -> int:
    result = subprocess.run(
        [
            'nvidia-smi',
            '--query-compute-apps=gpu_uuid',
            '--format=csv,noheader',
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
        text=True,
    )
    return sum(line.strip() == gpu_uuid for line in result.stdout.splitlines())


def find_exact_process(command: List[str]) -> Optional[int]:
    expected = b'\0'.join(value.encode('utf-8') for value in command) + b'\0'
    matches = []
    for path in Path('/proc').iterdir():
        if not path.name.isdigit():
            continue
        try:
            if (path / 'cmdline').read_bytes() == expected:
                matches.append(int(path.name))
        except (FileNotFoundError, PermissionError, ProcessLookupError):
            continue
    require(len(matches) <= 1, 'exact Worker process identity is ambiguous')
    return matches[0] if matches else None


def remove_runtime_tree(path: Path) -> None:
    allowed = Path('/home/wujiaxing/.local/state/gpustack-rust-worker').resolve()
    require(not path.is_symlink(), f'runtime path is a symlink: {path}')
    resolved = path.resolve(strict=False)
    require(resolved == path, f'runtime path is not canonical: {path}')
    require(
        allowed in resolved.parents, f'runtime path is outside allowed root: {path}'
    )
    if not path.exists():
        return
    require(path.is_dir(), f'runtime path is not a directory: {path}')
    require(path.stat().st_uid == os.getuid(), f'runtime path owner invalid: {path}')
    shutil.rmtree(path)


def validate_repo(config: Config, expected_head: str) -> None:
    commands = [
        ['git', 'rev-parse', 'HEAD'],
        ['git', 'rev-parse', 'origin/codex/rust-worker'],
        ['git', 'status', '--porcelain'],
    ]
    values = []
    for command in commands:
        result = subprocess.run(
            command,
            cwd=config.repo,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=True,
            text=True,
        )
        values.append(result.stdout.strip())
    require(values[0] == expected_head, 'source HEAD drifted')
    require(values[1] == expected_head, 'origin tracking reference drifted')
    require(values[2] == '', 'source worktree is dirty')
    version = subprocess.run(
        [str(config.binary), '--output', 'json', 'version'],
        cwd=config.repo,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
        text=True,
    )
    value = json.loads(version.stdout)
    require(value.get('git_commit') == expected_head[:12], 'binary commit drifted')


def validate_authorization(path: Path, expected_head: str, preflight_sha: str) -> None:
    private_file(path)
    value = load_json(path)
    require(isinstance(value, dict), 'authorization is not a JSON object')
    require(
        value.get('stage') == AUTHORIZATION_STAGE,
        'stage invalid',
    )
    require(value.get('approved') is True, 'maintenance window is not approved')
    require(value.get('expected_head') == expected_head, 'authorization HEAD drifted')
    require(
        value.get('preflight_manifest_sha256') == preflight_sha,
        'authorization preflight identity drifted',
    )
    nonce = value.get('nonce')
    require(isinstance(nonce, str) and len(nonce) >= 16, 'authorization nonce invalid')


def write_state(evidence: Evidence, state: Dict[str, Any]) -> None:
    evidence.json('runtime-state.json', state)


def experimental_worker(config: Config) -> Tuple[Optional[Dict[str, Any]], Any]:
    status, response = api_request(
        'GET',
        f'{config.python_server_url}v2/workers?page=-1',
        config.python_operator_token,
    )
    require(status == 200, 'cannot list Python Server Workers')
    matches = [
        worker
        for worker in resource_items(response)
        if worker.get('name') == config.worker_name
    ]
    require(len(matches) <= 1, 'experimental Worker identity is ambiguous')
    return (matches[0] if matches else None), response


def wait_experimental_worker(config: Config, timeout: int) -> Dict[str, Any]:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        worker, _ = experimental_worker(config)
        if worker is not None:
            return worker
        time.sleep(0.2)
    raise GateError('experimental Worker registration timed out')


def find_containers(filters: List[str], all_containers: bool = True) -> List[str]:
    command = ['docker', 'ps', '-q', '--no-trunc']
    if all_containers:
        command.insert(2, '-a')
    for value in filters:
        command.extend(['--filter', value])
    result = subprocess.run(
        command,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
        text=True,
    )
    return [line for line in result.stdout.splitlines() if line]


def deploy_and_validate(
    config: Config,
    evidence: Evidence,
    state: Dict[str, Any],
) -> None:
    evidence.log('[4/7] 部署固定BGE模型并验证运行时身份')
    arguments = [
        'model',
        'deploy',
        config.target_model,
        '--category',
        'embedding',
        '--runner',
        'pooling',
        '--worker',
        config.worker_name,
        '--local-path',
        config.model_path,
        '--image',
        config.runner_image,
        '--gpu',
        '0',
        '--max-model-len',
        '512',
        '--gpu-memory-utilization',
        '0.10',
        '--dtype',
        'auto',
        '--enforce-eager',
        '--wait',
    ]
    value = cli_json(
        config,
        evidence,
        'model-deploy',
        config.python_context,
        arguments,
        timeout=600,
    )
    require(isinstance(value, dict), 'model deploy response is malformed')
    model = value.get('model')
    instances = value.get('instances')
    transitions = value.get('transitions')
    require(isinstance(model, dict), 'model deploy response has no Model')
    require(
        isinstance(instances, list) and len(instances) == 1, 'instance count invalid'
    )
    require(isinstance(transitions, list), 'model transition list is unavailable')
    instance = instances[0]
    require(model.get('categories') == ['embedding'], 'Model category is invalid')
    require(instance.get('state') == 'running', 'ModelInstance is not running')
    require(
        instance.get('worker_id') == state['experimental_worker_id'], 'Worker mismatch'
    )
    require(instance.get('gpu_indexes') == [0], 'GPU assignment is not GPU0')
    observed = [item.get('state') for item in transitions if isinstance(item, dict)]
    required_states = [
        'pending',
        'analyzing',
        'scheduled',
        'initializing',
        'downloading',
        'starting',
        'running',
    ]
    index = 0
    for observed_state in observed:
        if index < len(required_states) and observed_state == required_states[index]:
            index += 1
    require(index == len(required_states), 'complete scheduling state chain is missing')

    state['model_id'] = int(model['id'])
    state['instance_id'] = int(instance['id'])
    write_state(evidence, state)
    filters = [
        'label=io.gpustack.implementation=rust',
        'label=io.gpustack.mode=managed',
        'label=io.gpustack.component=vllm',
        f'label=io.gpustack.worker-uuid={state["experimental_worker_uuid"]}',
        f'label=io.gpustack.model-instance-id={state["instance_id"]}',
    ]
    deadline = time.monotonic() + 60
    containers: List[str] = []
    while time.monotonic() < deadline:
        containers = find_containers(filters, all_containers=False)
        if len(containers) == 1:
            break
        require(len(containers) <= 1, 'experimental container identity is ambiguous')
        time.sleep(1)
    require(len(containers) == 1, 'experimental container was not found')
    container = docker_inspect(containers[0])
    labels = container.get('Config', {}).get('Labels') or {}
    requests = container.get('HostConfig', {}).get('DeviceRequests') or []
    command = container.get('Config', {}).get('Cmd') or []
    require(container.get('Image') == config.runner_image_id, 'Runner image mismatch')
    require(
        len(requests) == 1 and requests[0].get('DeviceIDs') == [config.gpu0_uuid],
        'experimental container requested an unexpected GPU',
    )
    require('--runner=pooling' in command, 'vLLM pooling runner is missing')
    max_len_joined = '--max-model-len=512' in command
    max_len_split = any(
        command[index : index + 2] == ['--max-model-len', '512']
        for index in range(len(command) - 1)
    )
    require(max_len_joined or max_len_split, 'vLLM max model length is missing')
    spec_hash = labels.get('io.gpustack.spec-hash')
    require(
        isinstance(spec_hash, str) and re.fullmatch(r'[0-9a-f]{64}', spec_hash),
        'experimental spec hash is invalid',
    )
    state['container_id'] = container['Id']
    state['spec_hash'] = spec_hash
    state['restart_count'] = container.get('RestartCount')
    state['instance_port'] = int(instance['port'])
    evidence.json('embedding-container-running.json', container)
    write_state(evidence, state)


def validate_embedding(value: Any, count: int, dimension: int) -> None:
    require(isinstance(value, dict), 'embedding response is not an object')
    data = value.get('data')
    require(isinstance(data, list) and len(data) == count, 'embedding count is invalid')
    for item in data:
        require(isinstance(item, dict), 'embedding item is malformed')
        embedding = item.get('embedding')
        require(isinstance(embedding, list), 'float embedding is not an array')
        require(len(embedding) == dimension, 'float embedding dimension is invalid')
        require(
            all(isinstance(number, (int, float)) for number in embedding),
            'non-number',
        )


def inference_gates(
    config: Config,
    evidence: Evidence,
    state: Dict[str, Any],
) -> None:
    evidence.log('[5/7] 验证Server CLI、Worker直连及float/base64响应')
    float_value = cli_json_retry(
        config,
        evidence,
        'server-embeddings-float',
        config.python_context,
        [
            'inference',
            'embeddings',
            '--model',
            config.target_model,
            '--input',
            'GPUStack是一个GPU集群管理平台。',
            '--input',
            'Rust Worker提供模型推理能力。',
            '--encoding-format',
            'float',
        ],
    )
    validate_embedding(float_value, 2, config.embedding_dimension)

    base64_value = cli_json(
        config,
        evidence,
        'server-embeddings-base64',
        config.python_context,
        [
            'inference',
            'embeddings',
            '--model',
            config.target_model,
            '--input',
            'GPUStack embedding base64 verification',
            '--encoding-format',
            'base64',
        ],
    )
    data = base64_value.get('data') if isinstance(base64_value, dict) else None
    require(isinstance(data, list) and len(data) == 1, 'base64 response count invalid')
    encoded = data[0].get('embedding') if isinstance(data[0], dict) else None
    require(isinstance(encoded, str), 'base64 embedding is not a string')
    try:
        decoded = base64.b64decode(encoded, validate=True)
    except ValueError as error:
        raise GateError('base64 embedding is invalid') from error
    require(
        len(decoded) == config.embedding_dimension * 4,
        'base64 embedding byte length is invalid',
    )

    table = evidence.run(
        'server-embeddings-table',
        [
            str(config.binary),
            '--timeout',
            '120',
            '--context',
            config.python_context,
            'inference',
            'embeddings',
            '--model',
            config.target_model,
            '--input',
            'GPUStack table verification',
            '--encoding-format',
            'float',
        ],
    )
    require(
        re.search(rb'0\s+512\s+float\s+', table.stdout) is not None,
        'table output does not report dimension 512',
    )

    destination = f'model-{state["model_id"]}-{state["instance_id"]}.static'
    request_body = {
        'model': config.target_model,
        'input': ['Worker direct embedding'],
        'encoding_format': 'float',
    }
    token = private_text(config.cluster_token)
    request = urllib.request.Request(
        f'http://{config.worker_ip}:{config.worker_port}/v1/embeddings',
        data=json.dumps(request_body).encode('utf-8'),
        headers={
            'Authorization': f'Bearer {token}',
            'Content-Type': 'application/json',
            'X-GPUStack-Model-Instance': destination,
        },
        method='POST',
    )
    try:
        with urllib.request.urlopen(request, timeout=120) as response:
            require(response.status == 200, 'Worker direct embedding returned non-200')
            direct_value = json.load(response)
    finally:
        token = ''
    evidence.json('worker-direct-response.json', direct_value)
    validate_embedding(direct_value, 1, config.embedding_dimension)


def repeat_gates(
    config: Config,
    evidence: Evidence,
    state: Dict[str, Any],
) -> None:
    evidence.log('[6/7] 验证三次重复推理不重建容器')
    for sample in range(1, 4):
        value = cli_json(
            config,
            evidence,
            f'repeat-{sample}',
            config.python_context,
            [
                'inference',
                'embeddings',
                '--model',
                config.target_model,
                '--input',
                'GPUStack repeated embedding request',
                '--encoding-format',
                'float',
            ],
        )
        validate_embedding(value, 1, config.embedding_dimension)
        container = docker_inspect(state['container_id'])
        labels = container.get('Config', {}).get('Labels') or {}
        require(container.get('Id') == state['container_id'], 'container ID changed')
        require(
            container.get('RestartCount') == state['restart_count'], 'restart changed'
        )
        require(
            labels.get('io.gpustack.spec-hash') == state['spec_hash'], 'spec drifted'
        )
        require(container.get('State', {}).get('Running') is True, 'container stopped')
        evidence.json(f'container-repeat-{sample}.json', container)


def model_files_for_worker(value: Any, worker_id: int) -> List[Dict[str, Any]]:
    """Apply the Worker filter locally when a Server ignores the query filter."""
    return [
        model_file
        for model_file in resource_items(value)
        if model_file.get('worker_id') == worker_id
    ]


def experimental_container_filters(
    worker_uuid: str,
    instance_id: Optional[int],
) -> List[str]:
    """Keep cleanup possible when failure precedes Instance ID persistence."""
    filters = [f'label=io.gpustack.worker-uuid={worker_uuid}']
    if instance_id is not None:
        filters.append(f'label=io.gpustack.model-instance-id={instance_id}')
    return filters


def stop_experimental_worker(
    config: Config,
    evidence: Evidence,
    state: Dict[str, Any],
) -> None:
    process = state.get('experimental_process')
    pid: Optional[int] = None
    if isinstance(process, dict) and process_matches(process):
        pid = int(process['pid'])
    else:
        pid = find_exact_process(config.worker_command())
    if pid is None:
        return
    os.kill(pid, signal.SIGTERM)
    wait_process_exit(pid, 60)
    evidence.text('experimental-worker-stop.txt', f'pid={pid}\nstatus=stopped\n')


def remove_experimental_resources(
    config: Config,
    evidence: Evidence,
    state: Dict[str, Any],
) -> None:
    status, models_value = api_request(
        'GET',
        f'{config.python_server_url}v2/models?page=-1',
        config.python_operator_token,
    )
    require(status == 200, 'cannot list experimental Models during cleanup')
    models = [
        model
        for model in resource_items(models_value)
        if model.get('name') == config.target_model
    ]
    require(len(models) <= 1, 'experimental Model identity is ambiguous')
    if models:
        model_id = int(models[0]['id'])
        recorded = state.get('model_id')
        require(recorded in (None, model_id), 'experimental Model ID conflicts')
        status, response = api_request(
            'DELETE',
            f'{config.python_server_url}v2/models/{model_id}',
            config.python_operator_token,
        )
        require(status in (200, 204), 'experimental Model delete failed')
        evidence.json('model-delete.json', response)

    deadline = time.monotonic() + 180
    while time.monotonic() < deadline:
        status, value = api_request(
            'GET',
            f'{config.python_server_url}v2/models?page=-1',
            config.python_operator_token,
        )
        require(status == 200, 'cannot verify Model deletion')
        if not any(
            model.get('name') == config.target_model for model in resource_items(value)
        ):
            break
        time.sleep(1)
    else:
        raise GateError('experimental Model deletion timed out')

    worker, worker_list = experimental_worker(config)
    evidence.json('workers-before-delete.json', worker_list)
    worker_id = (
        int(worker['id']) if worker is not None else state.get('experimental_worker_id')
    )
    worker_uuid = state.get('experimental_worker_uuid')
    instance_id = state.get('instance_id')
    if worker_id is not None:
        status, files_value = api_request(
            'GET',
            f'{config.python_server_url}v2/model-files?page=-1&worker_id={worker_id}',
            config.python_operator_token,
        )
        require(status == 200, 'cannot list experimental ModelFiles')
        model_files = model_files_for_worker(files_value, worker_id)
        evidence.json('experimental-model-files.json', model_files)
        for model_file in model_files:
            local_path = model_file.get('local_path')
            resolved = model_file.get('resolved_paths') or []
            require(
                local_path == config.model_path or config.model_path in resolved,
                'unexpected ModelFile belongs to experimental Worker',
            )
            delete_model_file_metadata(config, evidence, model_file)

    if isinstance(worker_uuid, str):
        filters = experimental_container_filters(worker_uuid, instance_id)
        containers = find_containers(filters)
        require(len(containers) <= 1, 'experimental container identity is ambiguous')
        if containers:
            container = docker_inspect(containers[0])
            evidence.json('experimental-container-before-remove.json', container)
            result = evidence.run(
                'experimental-container-remove',
                ['docker', 'rm', '--force', containers[0]],
                timeout=180,
            )
            require(result.returncode == 0, 'experimental container removal failed')

    worker, _ = experimental_worker(config)
    if worker is not None:
        worker_id = int(worker['id'])
        recorded = state.get('experimental_worker_id')
        require(recorded in (None, worker_id), 'experimental Worker ID conflicts')
        status, response = api_request(
            'DELETE',
            f'{config.python_server_url}v2/workers/{worker_id}',
            config.python_operator_token,
        )
        require(status in (200, 204), 'experimental Worker delete failed')
        evidence.json('worker-delete.json', response)
    remove_runtime_tree(config.data_dir)
    remove_runtime_tree(config.log_dir)


def delete_model_file_metadata(
    config: Config,
    _evidence: Evidence,
    model_file: Dict[str, Any],
) -> None:
    """Delete one exact experimental ModelFile through the public API."""
    model_file_id = int(model_file['id'])
    status, _ = api_request(
        'DELETE',
        (
            f'{config.python_server_url}v2/model-files/'
            f'{model_file_id}?cleanup=false'
        ),
        config.python_operator_token,
    )
    require(status in (200, 204), 'experimental ModelFile delete failed')


def restore_original(
    config: Config,
    evidence: Evidence,
    plan: Dict[str, Any],
) -> Dict[str, Any]:
    evidence.log('[cleanup] 恢复原容器和最新Worker')
    container = docker_inspect(plan['container_id'])
    validate_original_container(container, plan)
    if not container.get('State', {}).get('Running'):
        require(
            gpu_process_count(plan['gpu_uuid']) == 0, 'GPU0 is not empty for restore'
        )
        evidence.run(
            'original-container-start',
            ['docker', 'start', plan['container_id']],
            timeout=180,
        )

    restore_argv = plan.get('restore_argv')
    require(isinstance(restore_argv, list) and restore_argv, 'restore argv is invalid')
    require(restore_argv[0] == str(config.binary), 'restore binary path is invalid')
    original_survived = process_matches(plan['process'])
    existing_pid = find_exact_process(restore_argv)
    if original_survived:
        restored_pid = int(plan['process']['pid'])
    elif existing_pid is None:
        process = subprocess.Popen(
            restore_argv,
            cwd=config.repo,
            env=os.environ.copy(),
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        restored_pid = process.pid
    else:
        restored_pid = existing_pid

    deadline = time.monotonic() + 240
    worker_value: Optional[Dict[str, Any]] = None
    while time.monotonic() < deadline:
        require(Path(f'/proc/{restored_pid}').exists(), 'restored Worker exited')
        status, value = api_request(
            'GET',
            f'{config.rust_server_url}v2/workers/{config.original_worker_id}',
            config.rust_operator_token,
        )
        if status == 200 and isinstance(value, dict):
            if value.get('name') == config.original_worker_name and not value.get(
                'unreachable', False
            ):
                worker_value = value
                break
        time.sleep(1)
    require(worker_value is not None, 'restored Worker did not reconnect')

    maintenance = worker_value.get('maintenance') or {}
    current_maintenance = bool(maintenance.get('enabled', False))
    expected_maintenance = bool(plan.get('maintenance_enabled', False))
    if current_maintenance != expected_maintenance:
        set_maintenance(
            config,
            evidence,
            config.rust_context,
            config.original_worker_id,
            expected_maintenance,
            'original-maintenance-restore',
        )
    wait_health(
        f'http://{config.worker_ip}:{config.original_worker_port}/healthz', 200, 180
    )
    wait_health(
        f'http://{config.worker_ip}:{config.original_worker_port}/readyz', 200, 180
    )
    wait_health(
        f'http://{config.worker_ip}:{config.original_metrics_port}/metrics', 200, 180
    )
    wait_health(
        f'http://{config.worker_ip}:{config.original_model_port}/health', 200, 240
    )
    restored_identity = process_identity(restored_pid)
    if not original_survived:
        require(
            restored_identity['exe_sha256'] == plan['latest_binary_sha256'],
            'restored Worker is not running the latest binary',
        )
    container = docker_inspect(plan['container_id'])
    validate_original_container(container, plan)
    require(container.get('State', {}).get('Running') is True, 'container not running')
    require(
        container.get('RestartCount') == plan['container_restart_count'],
        'original container restart count drifted',
    )
    deadline = time.monotonic() + 240
    model: Any = None
    instance: Any = None
    while time.monotonic() < deadline:
        model_status, model = api_request(
            'GET',
            f'{config.rust_server_url}v2/models/{config.original_model_id}',
            config.rust_operator_token,
        )
        instance_status, instance = api_request(
            'GET',
            (
                f'{config.rust_server_url}v2/model-instances/'
                f'{config.original_instance_id}'
            ),
            config.rust_operator_token,
        )
        model_ready = (
            model_status == 200
            and isinstance(model, dict)
            and model.get('ready_replicas') == 1
        )
        instance_ready = (
            instance_status == 200
            and isinstance(instance, dict)
            and instance.get('state') == 'running'
        )
        if model_ready and instance_ready:
            break
        time.sleep(1)
    require(model_ready, 'original Model did not become ready')
    require(instance_ready, 'original ModelInstance did not become running')
    status, worker_value = api_request(
        'GET',
        f'{config.rust_server_url}v2/workers/{config.original_worker_id}',
        config.rust_operator_token,
    )
    require(status == 200 and isinstance(worker_value, dict), 'Worker refetch failed')
    evidence.json('restored-worker.json', worker_value)
    evidence.json('restored-model.json', model)
    evidence.json('restored-instance.json', instance)
    evidence.json('restored-container.json', container)
    evidence.json('restored-process.json', restored_identity)
    return restored_identity


def perform_cleanup(
    config: Config,
    preflight: Path,
    preflight_sha: str,
    formal: Path,
    cleanup_path: Path,
) -> bool:
    verify_manifest(preflight, preflight_sha)
    plan = load_json(preflight / 'recovery-plan.json')
    require(isinstance(plan, dict), 'recovery plan is malformed')
    evidence = Evidence(cleanup_path)
    cleanup_succeeded = False
    failed_step = 'initialization'
    try:
        state_path = formal / 'runtime-state.json'
        state = load_json(state_path) if state_path.is_file() else {}
        require(isinstance(state, dict), 'runtime state is malformed')
        failed_step = 'stop_experimental_worker'
        stop_experimental_worker(config, evidence, state)
        failed_step = 'remove_experimental_resources'
        remove_experimental_resources(config, evidence, state)
        failed_step = 'restore_original'
        restore_original(config, evidence, plan)
        failed_step = 'final_health'
        require(
            health_status(f'{config.python_server_url}healthz') == 200,
            'Python Server health failed after cleanup',
        )
        require(
            health_status(f'{config.rust_server_url}healthz') == 200,
            'Rust Server health failed after cleanup',
        )
        cleanup_succeeded = True
    except Exception as error:  # cleanup must always seal its failure evidence
        evidence.log(f'cleanup failure at {failed_step}: {error}')
    finally:
        status = 'PASS' if cleanup_succeeded else 'FAIL'
        reported_step = 'none' if cleanup_succeeded else failed_step
        evidence.text(
            'result.txt',
            (
                f'status={status}\n'
                f'stage={CLEANUP_STAGE}\n'
                f'failed_step={reported_step}\n'
            ),
        )
        try:
            manifest_sha = evidence.seal()
            print(
                f'cleanup_evidence={cleanup_path}\n'
                f'cleanup_manifest_sha256={manifest_sha}',
                flush=True,
            )
        except Exception as error:
            print(f'cleanup evidence sealing failed: {error}', file=sys.stderr)
            cleanup_succeeded = False
            evidence.text(
                'result.txt',
                (
                    'status=FAIL\n'
                    f'stage={CLEANUP_STAGE}\n'
                    'failed_step=evidence_seal\n'
                ),
            )
    return cleanup_succeeded


def execute(args: argparse.Namespace) -> int:
    config = Config()
    expected_head = args.expected_head
    preflight = Path(args.preflight)
    preflight_sha = args.preflight_sha
    formal_path = Path(args.evidence)
    cleanup_path = Path(args.cleanup_evidence)
    authorization = Path(args.authorization)
    validate_repo(config, expected_head)
    verify_manifest(preflight, preflight_sha)
    validate_authorization(authorization, expected_head, preflight_sha)
    require(not cleanup_path.exists(), 'cleanup evidence already exists')
    evidence = Evidence(formal_path)
    plan = load_json(preflight / 'recovery-plan.json')
    require(isinstance(plan, dict), 'recovery plan is malformed')
    state: Dict[str, Any] = {
        'window_entered': False,
        'smoke_passed': False,
        'cleanup_ok': False,
    }
    write_state(evidence, state)
    reason = 'executor_incomplete'
    phase = 'revalidate_preflight'
    cleanup_ok = False
    try:
        evidence.log('[1/7] 复核preflight、源码、进程和原容器')
        require(process_matches(plan['process']), 'original Worker process drifted')
        container = docker_inspect(plan['container_id'])
        validate_original_container(container, plan)
        require(container.get('State', {}).get('Running') is True, 'container stopped')
        require(gpu_process_count(plan['gpu_uuid']) > 0, 'GPU0 workload disappeared')

        phase = 'enter_window'
        evidence.log('[2/7] 原Worker进入maintenance并精确释放GPU0')
        set_maintenance(
            config,
            evidence,
            config.rust_context,
            config.original_worker_id,
            True,
            'original-maintenance-enable',
        )
        state['window_entered'] = True
        write_state(evidence, state)
        pid = int(plan['process']['pid'])
        require(process_matches(plan['process']), 'original Worker changed before stop')
        os.kill(pid, signal.SIGTERM)
        wait_process_exit(pid, 90)
        evidence.run(
            'original-container-stop',
            ['docker', 'stop', '--time', '120', plan['container_id']],
            timeout=150,
        )
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and gpu_process_count(plan['gpu_uuid']) != 0:
            time.sleep(1)
        require(gpu_process_count(plan['gpu_uuid']) == 0, 'GPU0 did not become empty')

        phase = 'start_experimental_worker'
        evidence.log('[3/7] 启动实验Worker并建立maintenance注册屏障')
        stdout = (evidence.path / 'experimental-worker.stdout').open('wb')
        stderr = (evidence.path / 'experimental-worker.stderr').open('wb')
        process = subprocess.Popen(
            config.worker_command(),
            cwd=config.repo,
            env=os.environ.copy(),
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        stdout.close()
        stderr.close()
        time.sleep(0.2)
        state['experimental_process'] = process_identity(process.pid)
        write_state(evidence, state)
        worker = wait_experimental_worker(config, 60)
        worker_id = int(worker['id'])
        set_maintenance(
            config,
            evidence,
            config.python_context,
            worker_id,
            True,
            'experimental-maintenance-enable',
        )
        identity_path = config.data_dir / 'managed' / 'worker-identity.json'
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline and not identity_path.is_file():
            time.sleep(0.2)
        require(
            identity_path.is_file(), 'experimental Worker identity file unavailable'
        )
        identity = load_json(identity_path)
        require(
            identity.get('worker_id') == worker_id, 'experimental Worker ID mismatch'
        )
        require(
            identity.get('worker_name') == config.worker_name, 'Worker name mismatch'
        )
        worker_uuid = identity.get('worker_uuid')
        require(
            isinstance(worker_uuid, str)
            and re.fullmatch(r'[0-9a-f-]{36}', worker_uuid),
            'experimental Worker UUID is invalid',
        )
        state['experimental_worker_id'] = worker_id
        state['experimental_worker_uuid'] = worker_uuid
        write_state(evidence, state)
        wait_health(f'http://{config.worker_ip}:{config.worker_port}/healthz', 200, 120)
        wait_health(f'http://{config.worker_ip}:{config.worker_port}/readyz', 200, 120)
        wait_health(
            f'http://{config.worker_ip}:{config.metrics_port}/metrics', 200, 120
        )
        set_maintenance(
            config,
            evidence,
            config.python_context,
            worker_id,
            False,
            'experimental-maintenance-disable',
        )

        phase = 'deploy_embedding_model'
        deploy_and_validate(config, evidence, state)
        phase = 'embedding_inference'
        inference_gates(config, evidence, state)
        phase = 'repeat_idempotency'
        repeat_gates(config, evidence, state)
        state['smoke_passed'] = True
        write_state(evidence, state)
        reason = 'smoke_passed'
    except Exception as error:
        reason = str(error)
        evidence.log(f'formal failure in {phase}: {error}')
    finally:
        evidence.log('[7/7] 清理实验资源并恢复原模型实例')
        cleanup_ok = perform_cleanup(
            config, preflight, preflight_sha, formal_path, cleanup_path
        )
        state['cleanup_ok'] = cleanup_ok
        write_state(evidence, state)
        status = 'PASS' if state['smoke_passed'] and cleanup_ok else 'FAIL'
        if status == 'PASS':
            reason = 'accepted_gates_passed'
            phase = 'complete'
        evidence.text(
            'result.txt',
            (
                f'status={status}\n'
                f'stage={FORMAL_STAGE}\n'
                f'reason={reason}\nphase={phase}\n'
                f'smoke_passed={str(state["smoke_passed"]).lower()}\n'
                f'cleanup_ok={str(cleanup_ok).lower()}\n'
            ),
        )
        try:
            manifest_sha = evidence.seal()
            print(
                f'formal_evidence={formal_path}\n'
                f'formal_manifest_sha256={manifest_sha}',
                flush=True,
            )
        except Exception as error:
            print(f'formal evidence sealing failed: {error}', file=sys.stderr)
            return 1
    return 0 if state['smoke_passed'] and cleanup_ok else 1


def post_audit(args: argparse.Namespace) -> int:
    config = Config()
    validate_repo(config, args.expected_head)
    preflight = Path(args.preflight)
    verify_manifest(preflight, args.preflight_sha)
    plan = load_json(preflight / 'recovery-plan.json')
    evidence = Evidence(Path(args.evidence))
    status = 'FAIL'
    try:
        evidence.log('[1/4] 验证实验控制面资源和本地运行残留归零')
        worker, workers = experimental_worker(config)
        evidence.json('python-workers.json', workers)
        require(worker is None, 'experimental Worker remains on Python Server')
        api_status, models_value = api_request(
            'GET',
            f'{config.python_server_url}v2/models?page=-1',
            config.python_operator_token,
        )
        require(api_status == 200, 'cannot list Python Server Models')
        evidence.json('python-models.json', models_value)
        require(
            not any(
                model.get('name') == config.target_model
                for model in resource_items(models_value)
            ),
            'experimental Model remains on Python Server',
        )
        require(not config.data_dir.exists(), 'experimental data directory remains')
        require(not config.log_dir.exists(), 'experimental log directory remains')
        require(find_exact_process(config.worker_command()) is None, 'process remains')
        require(
            health_status(
                f'http://{config.worker_ip}:{config.worker_port}/healthz'
            )
            == 0,
            'experimental Worker port remains reachable',
        )
        filters = [
            'label=io.gpustack.implementation=rust',
            f'label=io.gpustack.model-instance-id={args.experimental_instance_id}',
        ]
        if args.experimental_instance_id != 'none':
            require(not find_containers(filters), 'experimental container remains')

        evidence.log('[2/4] 验证原容器、Worker、Model和Instance完整恢复')
        container = docker_inspect(plan['container_id'])
        validate_original_container(container, plan)
        require(container.get('State', {}).get('Running') is True, 'container stopped')
        require(
            container.get('RestartCount') == plan['container_restart_count'],
            'original container restart count drifted',
        )
        status_code, worker_value = api_request(
            'GET',
            f'{config.rust_server_url}v2/workers/{config.original_worker_id}',
            config.rust_operator_token,
        )
        require(status_code == 200 and isinstance(worker_value, dict), 'Worker missing')
        require(
            worker_value.get('name') == config.original_worker_name, 'Worker drifted'
        )
        require(worker_value.get('unreachable') is False, 'Worker is unreachable')
        maintenance = worker_value.get('maintenance') or {}
        require(
            bool(maintenance.get('enabled', False)) == plan['maintenance_enabled'],
            'original maintenance state drifted',
        )
        status_code, model = api_request(
            'GET',
            f'{config.rust_server_url}v2/models/{config.original_model_id}',
            config.rust_operator_token,
        )
        require(
            status_code == 200 and model.get('ready_replicas') == 1,
            'Model not ready',
        )
        status_code, instance = api_request(
            'GET',
            f'{config.rust_server_url}v2/model-instances/{config.original_instance_id}',
            config.rust_operator_token,
        )
        require(
            status_code == 200 and instance.get('state') == 'running',
            'instance down',
        )
        evidence.json('original-worker.json', worker_value)
        evidence.json('original-model.json', model)
        evidence.json('original-instance.json', instance)
        evidence.json('original-container.json', container)

        evidence.log('[3/4] 验证恢复进程运行最新二进制和全部健康端点')
        restore_argv = plan['restore_argv']
        restored_pid = find_exact_process(restore_argv)
        require(restored_pid is not None, 'restored latest Worker process not found')
        identity = process_identity(restored_pid)
        require(
            identity['exe_sha256'] == plan['latest_binary_sha256'],
            'restored Worker binary identity drifted',
        )
        evidence.json('restored-process.json', identity)
        endpoints = {
            'python_server': f'{config.python_server_url}healthz',
            'rust_server': f'{config.rust_server_url}healthz',
            'original_worker_health': (
                f'http://{config.worker_ip}:{config.original_worker_port}/healthz'
            ),
            'original_worker_ready': (
                f'http://{config.worker_ip}:{config.original_worker_port}/readyz'
            ),
            'original_metrics': (
                f'http://{config.worker_ip}:{config.original_metrics_port}/metrics'
            ),
            'original_model': (
                f'http://{config.worker_ip}:{config.original_model_port}/health'
            ),
        }
        health = {name: health_status(url) for name, url in endpoints.items()}
        require(all(value == 200 for value in health.values()), 'health audit failed')
        evidence.json('health.json', health)

        post_audit_extra(config, evidence, plan, args)

        evidence.log('[4/4] 封存独立post-audit')
        status = 'PASS'
        return_code = 0
    except Exception as error:
        evidence.log(f'post-audit failure: {error}')
        return_code = 1
    finally:
        evidence.text(
            'result.txt',
            f'status={status}\nstage={AUDIT_STAGE}\n',
        )
        try:
            manifest_sha = evidence.seal()
            print(
                f'post_audit_evidence={evidence.path}\n'
                f'post_audit_manifest_sha256={manifest_sha}',
                flush=True,
            )
        except Exception as error:
            print(f'post-audit evidence sealing failed: {error}', file=sys.stderr)
            return_code = 1
    return return_code


def post_audit_extra(
    _config: Config,
    _evidence: Evidence,
    _plan: Dict[str, Any],
    _args: argparse.Namespace,
) -> None:
    """Run stage-specific recovery checks before sealing the audit."""


def cleanup_command(args: argparse.Namespace) -> int:
    config = Config()
    ok = perform_cleanup(
        config,
        Path(args.preflight),
        args.preflight_sha,
        Path(args.formal_evidence),
        Path(args.evidence),
    )
    return 0 if ok else 1


def common_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument('--preflight', required=True)
    parser.add_argument('--preflight-sha', required=True)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest='command', required=True)

    execute_parser = commands.add_parser('execute')
    common_arguments(execute_parser)
    execute_parser.add_argument('--expected-head', required=True)
    execute_parser.add_argument('--evidence', required=True)
    execute_parser.add_argument('--cleanup-evidence', required=True)
    execute_parser.add_argument('--authorization', required=True)
    execute_parser.set_defaults(handler=execute)

    cleanup_parser = commands.add_parser('cleanup')
    common_arguments(cleanup_parser)
    cleanup_parser.add_argument('--formal-evidence', required=True)
    cleanup_parser.add_argument('--evidence', required=True)
    cleanup_parser.set_defaults(handler=cleanup_command)

    audit_parser = commands.add_parser('post-audit')
    common_arguments(audit_parser)
    audit_parser.add_argument('--expected-head', required=True)
    audit_parser.add_argument('--evidence', required=True)
    audit_parser.add_argument('--experimental-instance-id', default='none')
    audit_parser.add_argument('--formal-evidence')
    audit_parser.set_defaults(handler=post_audit)
    return parser


def main() -> None:
    try:
        arguments = build_parser().parse_args()
        code = arguments.handler(arguments)
    except Exception as error:
        print(f'STOP: {error}', file=sys.stderr)
        code = 1
    raise SystemExit(code)


if __name__ == '__main__':
    main()

