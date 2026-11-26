"""Frozen host, resource, and model identities for M8-EMB.7 on host 106."""

from pathlib import Path
from typing import List


class Config:
    """Runtime values shared by the cookie-session integration tools."""

    def __init__(self) -> None:
        self.repo = Path('/home/wujiaxing/gpustack')
        self.binary = self.repo / 'rust/target/debug/gpustack-rs'
        self.python_server_url = 'http://127.0.0.1:18091/'
        self.rust_server_url = self.python_server_url
        self.original_worker_name = 'jiaxingHaoyuNB-106'
        self.original_worker_id = 17
        self.original_model_name = 'rust-rs6-106-qwen25-05b-gpu0'
        self.original_model_id = 21
        self.original_instance_id = 87
        self.original_worker_port = '20150'
        self.original_metrics_port = '20151'
        self.original_model_port = '40000'
        self.original_token = Path(
            '/home/wujiaxing/.config/liantiao/registration-token'
        )
        self.original_data_dir = Path(
            '/home/wujiaxing/.local/state/gpustack-rust-worker/liantiao-data'
        )
        self.original_log_dir = Path(
            '/home/wujiaxing/.local/state/gpustack-rust-worker/liantiao-logs'
        )
        self.original_identity = self.original_data_dir / 'managed/worker-identity.json'
        self.original_worker_token = self.original_data_dir / 'managed/worker-token'
        self.original_manifest = self.original_data_dir / 'managed/manifests/87.json'
        self.worker_name = 'rust-m8emb7-106'
        self.worker_hostname = 'lenovo-ThinkStation-P5'
        self.worker_ip = '10.213.80.106'
        self.worker_ifname = 'eno8403'
        self.worker_port = '20250'
        self.metrics_port = '20251'
        self.target_model = 'm8emb7-bge-small-zh-106-attempt4'
        self.model_path = (
            '/home/wujiaxing/.local/share/gpustack-rust-worker/models/'
            'bge-small-zh-v1.5-7999e1d'
        )
        self.model_manifest = (
            self.repo / 'rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.sha256'
        )
        self.model_metadata = (
            self.repo / 'rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.metadata.json'
        )
        self.model_revision = '7999e1d3359715c523056ef9478215996d62a620'
        self.model_files = 11
        self.model_bytes = 96378336
        self.embedding_dimension = 512
        self.runner_image = (
            'quay.io/gpustack/runner@sha256:'
            'f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
        )
        self.runner_image_id = (
            'sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af'
        )
        self.gpu0_uuid = 'GPU-be80b448-dd25-a523-676f-a279bf5c9c9c'
        self.cookie_file = Path(
            '/home/wujiaxing/.config/gpustack-rust/m8emb7-admin-session.cookies'
        )
        # The reused runtime helper passes these paths to the operator transport.
        self.python_operator_token = self.cookie_file
        self.rust_operator_token = self.cookie_file
        self.cluster_token = self.original_token
        self.contexts_file = Path(
            '/home/wujiaxing/.config/gpustack-rust/m8emb7-unused-contexts.json'
        )
        self.python_context = 'm8emb7-cookie-session'
        self.rust_context = self.python_context
        self.data_dir = Path(
            '/home/wujiaxing/.local/state/gpustack-rust-worker/m8emb7-106-data'
        )
        self.log_dir = Path(
            '/home/wujiaxing/.local/state/gpustack-rust-worker/m8emb7-106-logs'
        )
        self.evidence_root = Path(
            '/home/wujiaxing/.local/state/gpustack-rust-worker/evidence/m8emb7-106'
        )
        self.database_container = 'rust-server-db-1'
        self.database_image = 'postgres:17'
        self.database_compose_project = 'rust-server'
        self.database_compose_service = 'db'
        self.database_name = 'gpustack223'
        self.database_cluster_id = 1
        self.database_owner_principal_id = 1
        self.json_tool = self.repo / 'rust/scripts/m8emb6_106/json_tool.py'
        self.product_commit = '92184388070f961e8bbc1fbed4a3ec3c84691829'
        self.cargo_lock_sha = (
            'ce89eb7d47c0666ed35301705302ed260fc4f2d68c9f85b64f5fe8b6416e382a'
        )

    def worker_command(self) -> List[str]:
        """Build the isolated Worker command without exposing credential contents."""
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
