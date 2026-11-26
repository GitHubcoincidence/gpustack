#!/usr/bin/env python3
"""Read-only preflight for the cookie-authenticated M8-EMB.7 window."""

import argparse
import json
import os
import socket
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

import runtime_tool
from config import Config


base = runtime_tool.base


def run_text(config: Config, arguments: List[str]) -> str:
    """Run one read-only command and return stripped UTF-8 output."""
    result = subprocess.run(
        arguments,
        cwd=config.repo,
        env=os.environ.copy(),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
        text=True,
    )
    return result.stdout.strip()


def http_status(url: str) -> int:
    """Return one unauthenticated health status without response content."""
    try:
        with runtime_tool.urllib.request.urlopen(url, timeout=10) as response:
            return response.status
    except runtime_tool.urllib.error.HTTPError as error:
        return error.code
    except runtime_tool.urllib.error.URLError:
        return 0


def port_is_free(port: int) -> bool:
    """Check one loopback TCP port without starting a listener."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    try:
        return sock.connect_ex(('127.0.0.1', port)) != 0
    finally:
        sock.close()


def find_original_worker_pid(config: Config) -> int:
    """Find the single exact original Worker process by its command line."""
    result = subprocess.run(
        [
            'pgrep',
            '-f',
            '[g]pustack-rs.*worker.*--worker-name '
            f'{config.original_worker_name}.*run',
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    values = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    base.require(len(values) == 1, 'expected exactly one original Worker process')
    return int(values[0])


def docker_json(arguments: List[str]) -> Any:
    """Run a Docker inspection command and decode its JSON response."""
    result = subprocess.run(
        arguments,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=True,
    )
    return json.loads(result.stdout)


def write_process_identity(path: Path, pid: int) -> None:
    """Write the identity format consumed by the accepted recovery planner."""
    identity = base.process_identity(pid)
    value = (
        'role=original_rust_worker\n'
        f'pid={pid}\n'
        f'starttime={identity["starttime"]}\n'
        f'exe_sha256={identity["exe_sha256"]}\n'
        f'cmdline_sha256={identity["cmdline_sha256"]}\n'
    )
    path.write_text(value, encoding='utf-8')
    path.chmod(0o600)


def gpu_compute_count(gpu_uuid: str) -> int:
    """Count active compute processes on the exact preserved GPU."""
    result = subprocess.run(
        [
            'nvidia-smi',
            '--query-compute-apps=gpu_uuid',
            '--format=csv,noheader',
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
        text=True,
    )
    return sum(line.strip() == gpu_uuid for line in result.stdout.splitlines())


def preflight(expected_head: str, evidence_path: Path) -> int:
    """Build, freeze, and seal the exact recovery plan without service writes."""
    config = Config()
    evidence = base.Evidence(evidence_path)
    status = 'STOP'
    failed_step = 'initialization'
    return_code = 1
    try:
        failed_step = 'protected_inputs'
        evidence.log('[1/7] 验证管理员session、registration token和固定模型')
        base.private_file(config.cookie_file)
        base.private_file(config.cluster_token)
        base.private_file(config.original_identity)
        base.private_file(config.original_worker_token)
        base.private_file(config.original_manifest)
        verify = evidence.run(
            'verify-model',
            [
                'python3',
                str(config.json_tool),
                'verify-model',
                '--model-path',
                config.model_path,
                '--manifest',
                str(config.model_manifest),
                '--metadata',
                str(config.model_metadata),
                '--revision',
                config.model_revision,
                '--files',
                str(config.model_files),
                '--bytes',
                str(config.model_bytes),
                '--dimension',
                str(config.embedding_dimension),
            ],
        )
        base.require(verify.returncode == 0, 'fixed model verification failed')

        failed_step = 'source_identity'
        evidence.log('[2/7] 固定源码并离线构建最新Worker')
        base.require(
            run_text(config, ['git', 'branch', '--show-current'])
            == 'codex/rust-worker',
            'branch drifted',
        )
        base.require(
            run_text(config, ['git', 'rev-parse', 'HEAD']) == expected_head,
            'source HEAD drifted',
        )
        base.require(
            run_text(config, ['git', 'rev-parse', 'origin/codex/rust-worker'])
            == expected_head,
            'origin tracking reference drifted',
        )
        base.require(
            run_text(config, ['git', 'status', '--porcelain']) == '',
            'source worktree is dirty',
        )
        base.require(
            run_text(
                config,
                ['git', 'merge-base', '--is-ancestor', config.product_commit, 'HEAD'],
            )
            == '',
            'product commit is not an ancestor',
        )
        lock_sha = base.file_sha256(config.repo / 'rust/Cargo.lock')
        base.require(lock_sha == config.cargo_lock_sha, 'Cargo.lock drifted')
        build = evidence.run(
            'cargo-build',
            [
                '/home/wujiaxing/.cargo/bin/cargo',
                'build',
                '--manifest-path',
                'rust/Cargo.toml',
                '--workspace',
                '--locked',
                '--offline',
            ],
            timeout=900,
        )
        base.require(build.returncode == 0, 'workspace build failed')
        version = evidence.run(
            'version',
            [str(config.binary), '--output', 'json', 'version'],
        )
        version_value = json.loads(version.stdout)
        base.require(
            version_value.get('git_commit') == expected_head[:12],
            'binary commit drifted',
        )
        evidence.json(
            'source-identity.json',
            {
                'head': expected_head,
                'binary_sha256': base.file_sha256(config.binary),
                'cargo_lock_sha256': lock_sha,
                'model_manifest_sha256': base.file_sha256(config.model_manifest),
            },
        )

        failed_step = 'services'
        evidence.log('[3/7] 验证Server、原Worker和Qwen健康')
        health = {
            'rust_server': http_status(f'{config.rust_server_url}healthz'),
            'worker_health': http_status(
                f'http://{config.worker_ip}:{config.original_worker_port}/healthz'
            ),
            'worker_ready': http_status(
                f'http://{config.worker_ip}:{config.original_worker_port}/readyz'
            ),
            'worker_metrics': http_status(
                f'http://{config.worker_ip}:{config.original_metrics_port}/metrics'
            ),
            'original_model': http_status(
                f'http://{config.worker_ip}:{config.original_model_port}/health'
            ),
        }
        base.require(all(value == 200 for value in health.values()), 'health failed')
        evidence.json('health.json', health)

        failed_step = 'control_plane'
        evidence.log('[4/7] 验证session可见性和实验身份隔离')
        status_code, workers = runtime_tool.api_request(
            'GET',
            f'{config.rust_server_url}v2/workers?page=-1',
            config.cookie_file,
        )
        base.require(status_code == 200, 'Worker list failed')
        status_code, models = runtime_tool.api_request(
            'GET',
            f'{config.rust_server_url}v2/models?page=-1',
            config.cookie_file,
        )
        base.require(status_code == 200, 'Model list failed')
        base.require(
            not any(
                item.get('name') == config.worker_name
                for item in base.resource_items(workers)
            ),
            'experimental Worker already exists',
        )
        base.require(
            not any(
                item.get('name') == config.target_model
                for item in base.resource_items(models)
            ),
            'experimental Model already exists',
        )
        evidence.json('workers.json', workers)
        evidence.json('models.json', models)
        for path in (config.data_dir, config.log_dir, config.contexts_file):
            base.require(not path.exists() and not path.is_symlink(), 'path occupied')
        for port in (int(config.worker_port), int(config.metrics_port)):
            base.require(port_is_free(port), f'port is occupied: {port}')

        failed_step = 'original_process'
        evidence.log('[5/7] 固定原Worker进程、资源和同一容器')
        pid = find_original_worker_pid(config)
        process_path = evidence.path / 'original-worker-process.txt'
        write_process_identity(process_path, pid)
        cmdline = Path(f'/proc/{pid}/cmdline').read_bytes()
        cmdline_path = evidence.path / 'original-worker-cmdline.bin'
        cmdline_path.write_bytes(cmdline)
        cmdline_path.chmod(0o600)
        sanitized_path = evidence.path / 'restore-worker-argv.json'
        evidence.run(
            'sanitize-argv',
            [
                'python3',
                str(config.json_tool),
                'sanitize-argv',
                '--file',
                str(cmdline_path),
                '--binary',
                str(config.binary),
                '--output',
                str(sanitized_path),
            ],
        )
        for name, path in (
            ('original-worker-identity.json', config.original_identity),
            ('original-instance-manifest.json', config.original_manifest),
        ):
            target = evidence.path / name
            target.write_bytes(path.read_bytes())
            target.chmod(0o600)

        resources: Dict[str, Any] = {}
        for resource_name, resource_path in (
            ('worker', f'v2/workers/{config.original_worker_id}'),
            ('model', f'v2/models/{config.original_model_id}'),
            ('instance', f'v2/model-instances/{config.original_instance_id}'),
        ):
            status_code, value = runtime_tool.api_request(
                'GET',
                f'{config.rust_server_url}{resource_path}',
                config.cookie_file,
            )
            base.require(status_code == 200 and isinstance(value, dict), 'GET failed')
            resources[resource_name] = value
            evidence.json(f'original-{resource_name}.json', value)

        filters = [
            'label=io.gpustack.implementation=rust',
            'label=io.gpustack.mode=managed',
            'label=io.gpustack.component=vllm',
            f'label=io.gpustack.model-instance-id={config.original_instance_id}',
        ]
        containers = base.find_containers(filters, all_containers=False)
        base.require(len(containers) == 1, 'original container identity is invalid')
        container = docker_json(['docker', 'inspect', containers[0]])
        evidence.json('original-container.json', container)
        evidence.json(
            'runner-image.json',
            docker_json(['docker', 'image', 'inspect', config.runner_image_id]),
        )
        recovery_path = evidence.path / 'recovery-plan.json'
        evidence.run(
            'build-recovery-plan',
            [
                'python3',
                str(config.json_tool),
                'baseline',
                '--worker',
                str(evidence.path / 'original-worker.json'),
                '--identity',
                str(evidence.path / 'original-worker-identity.json'),
                '--model',
                str(evidence.path / 'original-model.json'),
                '--instance',
                str(evidence.path / 'original-instance.json'),
                '--container',
                str(evidence.path / 'original-container.json'),
                '--process',
                str(process_path),
                '--sanitized-argv',
                str(sanitized_path),
                '--binary',
                str(config.binary),
                '--output',
                str(recovery_path),
                '--worker-id',
                str(config.original_worker_id),
                '--worker-name',
                config.original_worker_name,
                '--model-id',
                str(config.original_model_id),
                '--model-name',
                config.original_model_name,
                '--instance-id',
                str(config.original_instance_id),
                '--image-id',
                config.runner_image_id,
                '--gpu-uuid',
                config.gpu0_uuid,
            ],
        )

        failed_step = 'gpu'
        evidence.log('[6/7] 固定唯一GPU和原计算进程基线')
        gpu_uuids = run_text(
            config,
            ['nvidia-smi', '--query-gpu=uuid', '--format=csv,noheader'],
        ).splitlines()
        base.require(gpu_uuids == [config.gpu0_uuid], 'GPU inventory drifted')
        base.require(
            gpu_compute_count(config.gpu0_uuid) > 0,
            'original GPU task missing',
        )
        evidence.run(
            'gpu-baseline',
            [
                'nvidia-smi',
                '--query-gpu=index,uuid,name,memory.total,memory.used,utilization.gpu',
                '--format=csv,noheader,nounits',
            ],
        )

        failed_step = 'seal'
        evidence.log('[7/7] 封存只读preflight')
        base.require(
            base.process_matches(base.load_json(recovery_path)['process']),
            'original process drifted',
        )
        status = 'PASS'
        return_code = 0
    except Exception as error:
        evidence.log(f'preflight failure at {failed_step}: {error}')
    finally:
        evidence.text(
            'result.txt',
            (
                f'status={status}\n'
                'stage=M8-EMB.7-cookie-runtime-preflight\n'
                f'failed_step={"none" if status == "PASS" else failed_step}\n'
                f'governance_head={expected_head}\n'
            ),
        )
        try:
            manifest_sha = evidence.seal()
            print(
                f'preflight_status={status}\n'
                f'evidence={evidence.path}\n'
                f'manifest_sha256={manifest_sha}',
                flush=True,
            )
        except Exception as error:
            print(f'evidence sealing failed: {error}', file=sys.stderr)
            return_code = 1
    return return_code


def build_parser() -> argparse.ArgumentParser:
    """Build the standalone read-only preflight CLI."""
    parser = argparse.ArgumentParser()
    parser.add_argument('--expected-head', required=True)
    parser.add_argument('--evidence', required=True)
    return parser


def main() -> None:
    """Run preflight and preserve a nonzero fail-closed exit."""
    arguments = build_parser().parse_args()
    raise SystemExit(preflight(arguments.expected_head, Path(arguments.evidence)))


if __name__ == '__main__':
    main()
