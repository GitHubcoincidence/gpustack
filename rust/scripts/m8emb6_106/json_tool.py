#!/usr/bin/env python3
"""Strict JSON and model identity checks for the M8-EMB.6 106 runbook."""

import argparse
import hashlib
import json
import os
import re
from pathlib import Path
from typing import Any, Dict, List


def fail(message: str) -> None:
    raise SystemExit(f'STOP: {message}')


def load_json(path: str) -> Any:
    with open(path, encoding='utf-8') as stream:
        return json.load(stream)


def items(value: Any) -> List[Dict[str, Any]]:
    if isinstance(value, list):
        return value
    if isinstance(value, dict):
        for key in ('items', 'data'):
            found = value.get(key)
            if isinstance(found, list):
                return found
    fail('JSON response does not contain an item list')


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: str, value: Any) -> None:
    output = Path(path)
    output.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + '\n', encoding='utf-8'
    )
    output.chmod(0o600)


def maintenance_enabled(worker: Dict[str, Any]) -> bool:
    maintenance = worker.get('maintenance')
    if isinstance(maintenance, dict):
        return bool(maintenance.get('enabled', False))
    return bool(worker.get('maintenance_mode', False))


def command_verify_model(args: argparse.Namespace) -> None:
    root = Path(args.model_path)
    if not root.is_dir() or root.is_symlink():
        fail('fixed model directory is unavailable or is a symlink')
    manifest_path = Path(args.manifest)
    metadata_path = Path(args.metadata)
    if not manifest_path.is_file() or not metadata_path.is_file():
        fail('model identity files are unavailable')

    expected: Dict[str, str] = {}
    for line in manifest_path.read_text(encoding='utf-8').splitlines():
        match = re.fullmatch(r'([0-9a-f]{64})  (.+)', line)
        if not match:
            fail('model manifest contains an invalid line')
        expected[match.group(2)] = match.group(1)

    actual: Dict[str, Path] = {}
    for current_root, directories, files in os.walk(root, followlinks=False):
        current = Path(current_root)
        for name in directories:
            if (current / name).is_symlink():
                fail('model directory contains a symlink')
        for name in files:
            path = current / name
            if path.is_symlink():
                fail('model directory contains a symlink')
            actual[path.relative_to(root).as_posix()] = path

    if set(actual) != set(expected):
        fail('model file set does not match the manifest')
    for name, expected_sha in expected.items():
        if sha256(actual[name]) != expected_sha:
            fail(f'model SHA-256 mismatch: {name}')

    total_bytes = sum(path.stat().st_size for path in actual.values())
    if len(actual) != args.files or total_bytes != args.bytes:
        fail('model file count or byte count is invalid')
    if (root / 'REVISION').read_text(encoding='utf-8').strip() != args.revision:
        fail('model revision is invalid')

    metadata = load_json(str(metadata_path))
    if not isinstance(metadata, dict):
        fail('model metadata is not an object')
    expected_metadata = {
        'revision': args.revision,
        'total_bytes': args.bytes,
        'embedding_dimension': args.dimension,
        'pooling_type': 'CLS',
    }
    for key, value in expected_metadata.items():
        if metadata.get(key) != value:
            fail(f'model metadata field is invalid: {key}')


def command_check_version(args: argparse.Namespace) -> None:
    value = load_json(args.file)
    if not isinstance(value, dict) or value.get('git_commit') != args.expected[:12]:
        fail('binary does not report the expected governance HEAD')


def command_check_contexts(args: argparse.Namespace) -> None:
    contexts = items(load_json(args.file))
    expected = {
        args.python_name: (args.python_url, args.python_token),
        args.rust_name: (args.rust_url, args.rust_token),
    }
    for name, (server_url, token_file) in expected.items():
        matches = [context for context in contexts if context.get('name') == name]
        if len(matches) != 1:
            fail(f'context identity is not unique: {name}')
        context = matches[0]
        if context.get('server_url') != server_url:
            fail(f'context Server URL is invalid: {name}')
        if context.get('token_file') != token_file:
            fail(f'context token file is invalid: {name}')


def command_python_safety(args: argparse.Namespace) -> None:
    workers = items(load_json(args.workers))
    models = items(load_json(args.models))
    if any(worker.get('name') == args.target_worker for worker in workers):
        fail('experimental Worker name already exists on Python Server')
    if any(model.get('name') == args.target_model for model in models):
        fail('experimental Model name already exists on Python Server')

    active = []
    for model in models:
        replicas = model.get('replicas') or 0
        if not isinstance(replicas, int) or replicas <= 0:
            continue
        selector = model.get('worker_selector')
        if not isinstance(selector, dict):
            fail(f'active Model {model.get("id")} has no exact Worker selector')
        worker_name = selector.get('worker-name')
        if not isinstance(worker_name, str) or not worker_name:
            fail(f'active Model {model.get("id")} has no exact Worker name')
        if worker_name == args.target_worker:
            fail('an active Model already targets the experimental Worker')
        active.append(
            {
                'id': model.get('id'),
                'name': model.get('name'),
                'replicas': replicas,
                'worker_name': worker_name,
                'gpu_selector': model.get('gpu_selector'),
            }
        )
    print(json.dumps({'active_models': active}, ensure_ascii=False, indent=2))


def find_one(values: List[Dict[str, Any]], key: str, expected: Any) -> Dict[str, Any]:
    matches = [value for value in values if value.get(key) == expected]
    if len(matches) != 1:
        fail(f'expected exactly one resource with {key}={expected}')
    return matches[0]


def command_sanitize_argv(args: argparse.Namespace) -> None:
    raw = Path(args.file).read_bytes()
    values = [item.decode('utf-8') for item in raw.split(b'\0') if item]
    if len(values) < 3 or 'worker' not in values or values[-1] != 'run':
        fail('captured Worker argv does not match the unified command')
    sanitized = [args.binary]
    index = 1
    removed = 0
    while index < len(values):
        value = values[index]
        if value == '--model-root':
            if index + 1 >= len(values):
                fail('captured --model-root has no value')
            index += 2
            removed += 1
            continue
        if value.startswith('--model-root='):
            index += 1
            removed += 1
            continue
        sanitized.append(value)
        index += 1
    if removed > 1:
        fail('captured Worker argv contains repeated --model-root')
    write_json(args.output, {'argv': sanitized, 'removed_model_root': removed})


def command_baseline(args: argparse.Namespace) -> None:
    worker = load_json(args.worker)
    model = load_json(args.model)
    instance = load_json(args.instance)
    inspect_value = load_json(args.container)
    if not all(isinstance(value, dict) for value in (worker, model, instance)):
        fail('Server resource baseline is malformed')
    if not isinstance(inspect_value, list) or len(inspect_value) != 1:
        fail('container inspect response is malformed')
    container = inspect_value[0]

    if worker.get('id') != args.worker_id or worker.get('name') != args.worker_name:
        fail('original Worker identity drifted')
    worker_uuid = worker.get('uuid')
    if not isinstance(worker_uuid, str) or not re.fullmatch(
        r'[0-9a-fA-F-]{36}', worker_uuid
    ):
        fail('original Worker UUID is invalid')
    if worker.get('unreachable') is True:
        fail('original Worker is unreachable')
    if model.get('id') != args.model_id or model.get('name') != args.model_name:
        fail('original Model identity drifted')
    if instance.get('id') != args.instance_id:
        fail('original ModelInstance identity drifted')
    if instance.get('model_id') != args.model_id:
        fail('original ModelInstance has the wrong Model')
    if instance.get('worker_id') != args.worker_id:
        fail('original ModelInstance has the wrong Worker')

    labels = container.get('Config', {}).get('Labels') or {}
    if labels.get('io.gpustack.implementation') != 'rust':
        fail('original container is not owned by Rust Worker')
    if labels.get('io.gpustack.mode') != 'managed':
        fail('original container is not a managed runtime')
    if labels.get('io.gpustack.worker-uuid') != worker_uuid:
        fail('original container Worker UUID is invalid')
    if labels.get('io.gpustack.model-instance-id') != str(args.instance_id):
        fail('original container ModelInstance ID is invalid')
    if container.get('Image') != args.image_id:
        fail('original container Runner image identity drifted')
    requests = container.get('HostConfig', {}).get('DeviceRequests') or []
    if len(requests) != 1 or requests[0].get('DeviceIDs') != [args.gpu_uuid]:
        fail('original container GPU assignment drifted')
    state = container.get('State') or {}
    if not state.get('Running'):
        fail('original container is not running')
    if container.get('RestartCount') != 0:
        fail('original container restart count is not zero')
    spec_hash = labels.get('io.gpustack.spec-hash')
    if not isinstance(spec_hash, str) or not re.fullmatch(r'[0-9a-f]{64}', spec_hash):
        fail('original container spec hash is invalid')

    sanitized = load_json(args.sanitized_argv)
    if not isinstance(sanitized, dict) or not isinstance(sanitized.get('argv'), list):
        fail('sanitized Worker argv is malformed')
    process = {}
    for line in Path(args.process).read_text(encoding='utf-8').splitlines():
        key, separator, value = line.partition('=')
        if not separator:
            fail('process identity file is malformed')
        process[key] = value

    recovery = {
        'worker_id': args.worker_id,
        'worker_uuid': worker_uuid,
        'worker_name': args.worker_name,
        'model_id': args.model_id,
        'model_name': args.model_name,
        'instance_id': args.instance_id,
        'container_id': container.get('Id'),
        'container_name': container.get('Name'),
        'container_image_id': container.get('Image'),
        'container_spec_hash': spec_hash,
        'container_restart_count': container.get('RestartCount'),
        'maintenance_enabled': maintenance_enabled(worker),
        'gpu_uuid': args.gpu_uuid,
        'process': process,
        'restore_argv': sanitized['argv'],
        'removed_model_root': sanitized.get('removed_model_root'),
        'latest_binary_sha256': sha256(Path(args.binary)),
    }
    write_json(args.output, recovery)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest='command', required=True)

    verify = commands.add_parser('verify-model')
    verify.add_argument('--model-path', required=True)
    verify.add_argument('--manifest', required=True)
    verify.add_argument('--metadata', required=True)
    verify.add_argument('--revision', required=True)
    verify.add_argument('--files', required=True, type=int)
    verify.add_argument('--bytes', required=True, type=int)
    verify.add_argument('--dimension', required=True, type=int)
    verify.set_defaults(handler=command_verify_model)

    version = commands.add_parser('check-version')
    version.add_argument('--file', required=True)
    version.add_argument('--expected', required=True)
    version.set_defaults(handler=command_check_version)

    contexts = commands.add_parser('check-contexts')
    contexts.add_argument('--file', required=True)
    contexts.add_argument('--python-name', required=True)
    contexts.add_argument('--python-url', required=True)
    contexts.add_argument('--python-token', required=True)
    contexts.add_argument('--rust-name', required=True)
    contexts.add_argument('--rust-url', required=True)
    contexts.add_argument('--rust-token', required=True)
    contexts.set_defaults(handler=command_check_contexts)

    safety = commands.add_parser('python-safety')
    safety.add_argument('--workers', required=True)
    safety.add_argument('--models', required=True)
    safety.add_argument('--target-worker', required=True)
    safety.add_argument('--target-model', required=True)
    safety.set_defaults(handler=command_python_safety)

    sanitize = commands.add_parser('sanitize-argv')
    sanitize.add_argument('--file', required=True)
    sanitize.add_argument('--binary', required=True)
    sanitize.add_argument('--output', required=True)
    sanitize.set_defaults(handler=command_sanitize_argv)

    baseline = commands.add_parser('baseline')
    baseline.add_argument('--worker', required=True)
    baseline.add_argument('--model', required=True)
    baseline.add_argument('--instance', required=True)
    baseline.add_argument('--container', required=True)
    baseline.add_argument('--process', required=True)
    baseline.add_argument('--sanitized-argv', required=True)
    baseline.add_argument('--binary', required=True)
    baseline.add_argument('--output', required=True)
    baseline.add_argument('--worker-id', required=True, type=int)
    baseline.add_argument('--worker-name', required=True)
    baseline.add_argument('--model-id', required=True, type=int)
    baseline.add_argument('--model-name', required=True)
    baseline.add_argument('--instance-id', required=True, type=int)
    baseline.add_argument('--image-id', required=True)
    baseline.add_argument('--gpu-uuid', required=True)
    baseline.set_defaults(handler=command_baseline)
    return parser


def main() -> None:
    args = build_parser().parse_args()
    args.handler(args)


if __name__ == '__main__':
    main()
