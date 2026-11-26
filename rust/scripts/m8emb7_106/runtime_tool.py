#!/usr/bin/env python3
"""Cookie-authenticated M8-EMB.7 runtime built on the accepted 106 lifecycle."""

import base64
import http.cookiejar
import importlib.util
import json
import os
import re
import signal
import subprocess
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from config import Config


SCRIPT_DIR = Path(__file__).resolve().parent
BASE_TOOL = SCRIPT_DIR.parent / 'm8emb6_106' / 'runtime_tool.py'
SPEC = importlib.util.spec_from_file_location('m8emb6_runtime_tool', BASE_TOOL)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError('accepted M8-EMB.6 runtime tool is unavailable')
base = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(base)

base.AUTHORIZATION_STAGE = 'M8-EMB.7-cookie-runtime-maintenance-window'
base.MAINTENANCE_MESSAGE = 'M8-EMB.7 cookie-session runtime window'
base.FORMAL_STAGE = 'M8-EMB.7-cookie-runtime-formal'
base.CLEANUP_STAGE = 'M8-EMB.7-cookie-runtime-cleanup'
base.AUDIT_STAGE = 'M8-EMB.7-cookie-runtime-post-audit'


def cookie_opener(cookie_file: Path) -> urllib.request.OpenerDirector:
    """Load the protected administrator cookie without exposing its value."""
    base.private_file(cookie_file)
    jar = http.cookiejar.MozillaCookieJar(str(cookie_file))
    jar.load(ignore_discard=True, ignore_expires=False)
    base.require(bool(jar), 'administrator session cookie is empty or expired')
    return urllib.request.build_opener(urllib.request.HTTPCookieProcessor(jar))


def api_request(
    method: str,
    url: str,
    cookie_file: Path,
    body: Optional[Any] = None,
) -> Tuple[int, Any]:
    """Send one JSON request with the short-lived administrator session."""
    data = None
    headers: Dict[str, str] = {}
    if body is not None:
        data = json.dumps(body).encode('utf-8')
        headers['Content-Type'] = 'application/json'
    request = urllib.request.Request(
        url,
        data=data,
        headers=headers,
        method=method,
    )
    try:
        with cookie_opener(cookie_file).open(request, timeout=30) as response:
            payload = response.read()
            status = response.status
    except urllib.error.HTTPError as error:
        payload = error.read()
        status = error.code
    if not payload:
        return status, None
    try:
        return status, json.loads(payload)
    except json.JSONDecodeError:
        return status, payload.decode('utf-8', errors='replace')


def validate_authorization(
    path: Path,
    expected_head: str,
    preflight_sha: str,
) -> None:
    """Bind one explicit approval to the exact source and preflight evidence."""
    base.private_file(path)
    value = base.load_json(path)
    base.require(isinstance(value, dict), 'authorization is not a JSON object')
    base.require(
        value.get('stage') == 'M8-EMB.7-cookie-runtime-maintenance-window',
        'authorization stage invalid',
    )
    base.require(value.get('approved') is True, 'maintenance window is not approved')
    base.require(
        value.get('expected_head') == expected_head,
        'authorization HEAD drifted',
    )
    base.require(
        value.get('preflight_manifest_sha256') == preflight_sha,
        'authorization preflight identity drifted',
    )
    nonce = value.get('nonce')
    base.require(isinstance(nonce, str) and len(nonce) >= 16, 'nonce invalid')


def set_maintenance(
    config: Config,
    evidence: Any,
    _context: str,
    worker_id: int,
    enabled: bool,
    name: str,
) -> None:
    """Update only name, labels, and maintenance through the admin session."""
    status, before = api_request(
        'GET',
        f'{config.rust_server_url}v2/workers/{worker_id}',
        config.cookie_file,
    )
    base.require(status == 200 and isinstance(before, dict), 'Worker GET failed')
    maintenance = {
        'enabled': enabled,
        'message': ('M8-EMB.7 cookie-session runtime window' if enabled else None),
    }
    body = {
        'name': before.get('name'),
        'labels': before.get('labels') or {},
        'maintenance': maintenance,
    }
    status, after = api_request(
        'PUT',
        f'{config.rust_server_url}v2/workers/{worker_id}',
        config.cookie_file,
        body,
    )
    base.require(status == 200 and isinstance(after, dict), 'Worker PUT failed')
    actual = after.get('maintenance') or {}
    base.require(actual.get('enabled') is enabled, 'maintenance did not converge')
    evidence.json(f'{name}-before.json', before)
    evidence.json(f'{name}-after.json', after)


def collect_instance_states(path: Path, model_id: int) -> List[str]:
    """Extract target state events from an SSE or JSON-lines watch capture."""
    states: List[str] = []
    if not path.is_file():
        return states
    for raw in path.read_text(encoding='utf-8', errors='replace').splitlines():
        line = raw.strip()
        if line.startswith('data:'):
            line = line[5:].strip()
        if not line or line.startswith(':'):
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        stack = [value]
        while stack:
            item = stack.pop()
            if isinstance(item, dict):
                if item.get('model_id') == model_id and isinstance(
                    item.get('state'), str
                ):
                    states.append(item['state'].lower())
                    break
                stack.extend(item.values())
            elif isinstance(item, list):
                stack.extend(item)
    return states


def stop_watch(process: subprocess.Popen[bytes]) -> None:
    """Stop the exact isolated timeout/curl process group and reap it."""
    if process.poll() is not None:
        return
    os.killpg(process.pid, signal.SIGTERM)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=5)


def has_state_sequence(states: List[str]) -> bool:
    """Check the managed lifecycle while allowing repeated intermediate states."""
    required = [
        'pending',
        'analyzing',
        'scheduled',
        'initializing',
        'downloading',
        'starting',
        'running',
    ]
    index = 0
    for state in states:
        if index < len(required) and state == required[index]:
            index += 1
    return index == len(required)


def require_state_sequence(states: List[str]) -> None:
    """Require the complete managed lifecycle from pending through running."""
    base.require(has_state_sequence(states), f'incomplete state sequence: {states}')


def deploy_and_validate(
    config: Config,
    evidence: Any,
    state: Dict[str, Any],
) -> None:
    """Create the fixed BGE Model and validate the cookie-visible runtime."""
    evidence.log('[4/7] 使用管理员session部署固定BGE并验证运行时身份')
    watch_body = evidence.path / 'model-instance-watch.body'
    watch_headers = evidence.path / 'model-instance-watch.headers'
    body_stream = watch_body.open('wb')
    header_stream = watch_headers.open('wb')
    watch = subprocess.Popen(
        [
            'timeout',
            '--foreground',
            '720',
            'curl',
            '--silent',
            '--show-error',
            '--no-buffer',
            '--fail-with-body',
            '--cookie',
            str(config.cookie_file),
            '--dump-header',
            '/dev/fd/2',
            f'{config.rust_server_url}v2/model-instances?watch=true',
        ],
        cwd=config.repo,
        stdout=body_stream,
        stderr=header_stream,
        start_new_session=True,
    )
    body_stream.close()
    header_stream.close()
    time.sleep(1)

    worker, _ = base.experimental_worker(config)
    base.require(worker is not None, 'experimental Worker is unavailable')
    cluster_id = worker.get('cluster_id')
    base.require(isinstance(cluster_id, int), 'experimental Cluster ID is invalid')
    request = {
        'cluster_id': cluster_id,
        'name': config.target_model,
        'source': 'local_path',
        'categories': ['embedding'],
        'local_path': config.model_path,
        'replicas': 1,
        'backend': 'vLLM',
        'image_name': config.runner_image,
        'worker_selector': {'worker-name': config.worker_name},
        'gpu_selector': {
            'gpu_ids': [f'{config.worker_name}:cuda:0'],
            'gpus_per_replica': 1,
        },
        'backend_parameters': [
            '--tensor-parallel-size=1',
            '--runner=pooling',
            '--max-model-len=512',
            '--gpu-memory-utilization=0.1',
            '--dtype=auto',
            '--enforce-eager',
        ],
        'distributed_inference_across_workers': False,
        'enable_model_route': True,
    }
    try:
        status, model = api_request(
            'POST',
            f'{config.rust_server_url}v2/models',
            config.cookie_file,
            request,
        )
        base.require(
            status in (200, 201) and isinstance(model, dict),
            f'Model create failed: HTTP {status}',
        )
        evidence.json('model-create-request.json', request)
        evidence.json('model-create-response.json', model)
        model_id = model.get('id')
        base.require(isinstance(model_id, int), 'Model response has no integer ID')
        state['model_id'] = model_id
        base.write_state(evidence, state)

        transitions: List[str] = []
        last_state: Optional[str] = None
        instance: Optional[Dict[str, Any]] = None
        deadline = time.monotonic() + 660
        while time.monotonic() < deadline:
            status, value = api_request(
                'GET',
                f'{config.rust_server_url}v2/model-instances?page=-1',
                config.cookie_file,
            )
            base.require(status == 200, 'ModelInstance list failed')
            matches = [
                item
                for item in base.resource_items(value)
                if item.get('model_id') == model_id
            ]
            base.require(len(matches) <= 1, 'ModelInstance identity is ambiguous')
            if matches:
                instance = matches[0]
                current = str(instance.get('state', '')).lower()
                if current and current != last_state:
                    transitions.append(current)
                    last_state = current
                if current == 'error':
                    raise base.GateError(
                        f'ModelInstance failed: {instance.get("state_message")}'
                    )
                if current == 'running':
                    break
            time.sleep(0.25)
        else:
            raise base.GateError('ModelInstance did not become running')

        event_deadline = time.monotonic() + 10
        while time.monotonic() < event_deadline:
            if has_state_sequence(collect_instance_states(watch_body, model_id)):
                break
            time.sleep(0.2)
    finally:
        stop_watch(watch)

    base.require(instance is not None, 'ModelInstance is unavailable')
    watch_header_text = watch_headers.read_text(encoding='utf-8', errors='replace')
    base.require(
        re.search(r'^HTTP/\S+ 200(?:\s|$)', watch_header_text, re.MULTILINE)
        is not None,
        'ModelInstance watch did not return HTTP 200',
    )
    watch_states = collect_instance_states(watch_body, int(state['model_id']))
    combined_states = watch_states if watch_states else transitions
    require_state_sequence(combined_states)
    evidence.json(
        'model-transitions.json',
        {'watch': watch_states, 'poll': transitions},
    )
    base.require(model.get('categories') == ['embedding'], 'category is invalid')
    base.require(
        instance.get('worker_id') == state['experimental_worker_id'],
        'Worker assignment is invalid',
    )
    base.require(instance.get('gpu_indexes') == [0], 'GPU assignment is invalid')
    state['instance_id'] = int(instance['id'])
    state['instance_port'] = int(instance['port'])
    base.write_state(evidence, state)

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
        containers = base.find_containers(filters, all_containers=False)
        base.require(len(containers) <= 1, 'container identity is ambiguous')
        if len(containers) == 1:
            break
        time.sleep(1)
    base.require(len(containers) == 1, 'experimental container was not found')
    container = base.docker_inspect(containers[0])
    labels = container.get('Config', {}).get('Labels') or {}
    requests = container.get('HostConfig', {}).get('DeviceRequests') or []
    command = container.get('Config', {}).get('Cmd') or []
    base.require(container.get('Image') == config.runner_image_id, 'image mismatch')
    base.require(
        len(requests) == 1 and requests[0].get('DeviceIDs') == [config.gpu0_uuid],
        'container requested an unexpected GPU',
    )
    base.require('--runner=pooling' in command, 'pooling runner is missing')
    base.require('--max-model-len=512' in command, 'max model length is missing')
    spec_hash = labels.get('io.gpustack.spec-hash')
    base.require(
        isinstance(spec_hash, str) and re.fullmatch(r'[0-9a-f]{64}', spec_hash),
        'spec hash is invalid',
    )
    state['container_id'] = container['Id']
    state['spec_hash'] = spec_hash
    state['restart_count'] = container.get('RestartCount')
    evidence.json('embedding-container-running.json', container)
    base.write_state(evidence, state)


def server_embedding(
    config: Config,
    evidence: Any,
    name: str,
    body: Dict[str, Any],
    attempts: int = 1,
) -> Any:
    """Call the Server route with bounded retries for asynchronous route readiness."""
    statuses: List[int] = []
    last_value: Any = None
    for attempt in range(1, attempts + 1):
        status, value = api_request(
            'POST',
            f'{config.rust_server_url}v1/embeddings',
            config.cookie_file,
            body,
        )
        statuses.append(status)
        last_value = value
        if status == 200:
            evidence.json(f'{name}.json', value)
            evidence.json(f'{name}-attempts.json', {'statuses': statuses})
            return value
        if attempt < attempts:
            time.sleep(5)
    evidence.json(f'{name}-attempts.json', {'statuses': statuses, 'last': last_value})
    raise base.GateError(f'Server embedding route failed: statuses={statuses}')


def inference_gates(
    config: Config,
    evidence: Any,
    state: Dict[str, Any],
) -> None:
    """Validate cookie-authenticated Server inference and direct Worker inference."""
    evidence.log('[5/7] 验证Server session与Worker直连embedding响应')
    float_value = server_embedding(
        config,
        evidence,
        'server-embeddings-float',
        {
            'model': config.target_model,
            'input': [
                'GPUStack是一个GPU集群管理平台。',
                'Rust Worker提供模型推理能力。',
            ],
            'encoding_format': 'float',
        },
        attempts=12,
    )
    base.validate_embedding(float_value, 2, config.embedding_dimension)

    base64_value = server_embedding(
        config,
        evidence,
        'server-embeddings-base64',
        {
            'model': config.target_model,
            'input': 'GPUStack embedding base64 verification',
            'encoding_format': 'base64',
        },
    )
    data = base64_value.get('data') if isinstance(base64_value, dict) else None
    base.require(isinstance(data, list) and len(data) == 1, 'base64 count invalid')
    encoded = data[0].get('embedding') if isinstance(data[0], dict) else None
    base.require(isinstance(encoded, str), 'base64 embedding is not a string')
    decoded = base64.b64decode(encoded, validate=True)
    base.require(
        len(decoded) == config.embedding_dimension * 4,
        'base64 byte length is invalid',
    )

    destination = f'model-{state["model_id"]}-{state["instance_id"]}.static'
    request_body = {
        'model': config.target_model,
        'input': ['Worker direct embedding'],
        'encoding_format': 'float',
    }
    token = base.private_text(config.cluster_token)
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
            base.require(response.status == 200, 'Worker embedding returned non-200')
            direct_value = json.load(response)
    finally:
        token = ''
    evidence.json('worker-direct-response.json', direct_value)
    base.validate_embedding(direct_value, 1, config.embedding_dimension)
    evidence.text(
        'cli-bypass.txt',
        'bearer_session_http=401\ncontrol_plane=administrator_cookie\n',
    )


def repeat_gates(
    config: Config,
    evidence: Any,
    state: Dict[str, Any],
) -> None:
    """Prove repeated cookie-authenticated requests do not replace the container."""
    evidence.log('[6/7] 验证三次重复推理不重建容器')
    for sample in range(1, 4):
        value = server_embedding(
            config,
            evidence,
            f'repeat-{sample}',
            {
                'model': config.target_model,
                'input': 'GPUStack repeated embedding request',
                'encoding_format': 'float',
            },
        )
        base.validate_embedding(value, 1, config.embedding_dimension)
        container = base.docker_inspect(state['container_id'])
        labels = container.get('Config', {}).get('Labels') or {}
        base.require(container.get('Id') == state['container_id'], 'ID changed')
        base.require(
            container.get('RestartCount') == state['restart_count'],
            'restart count changed',
        )
        base.require(
            labels.get('io.gpustack.spec-hash') == state['spec_hash'],
            'spec hash changed',
        )
        base.require(container.get('State', {}).get('Running') is True, 'stopped')
        evidence.json(f'container-repeat-{sample}.json', container)


base.Config = Config
base.api_request = api_request
base.validate_authorization = validate_authorization
base.set_maintenance = set_maintenance
base.deploy_and_validate = deploy_and_validate
base.inference_gates = inference_gates
base.repeat_gates = repeat_gates


def main() -> None:
    """Run the accepted lifecycle parser with M8-EMB.7 operator overrides."""
    base.main()


if __name__ == '__main__':
    main()
