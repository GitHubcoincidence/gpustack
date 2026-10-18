# M1：90 主机环境基线

## 状态与证据边界

- 采集时间：2026-10-18 19:30 CST 起。
- 状态：`accepted`，允许进入 M2 Rust workspace 与 CLI 骨架。
- 主机：`wujiaxing2@r760xa`，Dell PowerEdge R760xa。
- 工作树：`/home/wujiaxing2/workspaces/gpustack`，分支 `codex/rust-worker`。
- 证据边界：本阶段只执行只读查询、Git object 导入和用户级 SSH 公钥配置；没有停止、重启、创建或删除任何 GPUStack/vLLM 容器，也没有修改 Server、Worker、数据库、端口或模型。
- M1 接受仅证明实现输入已经固定，不证明 Rust Worker、Runner/模型组合或 Managed 流程已经通过。

## 主机资源

| 项目 | 已验证值 |
|---|---|
| OS | Ubuntu 24.04.4 LTS |
| Kernel | Linux `6.8.0-147-generic` |
| Architecture | `x86_64` |
| CPU | 2 × Intel Xeon Gold 6542Y，48 cores / 96 threads |
| NUMA | 2 nodes；两张 GPU 均报告 NUMA affinity 0 |
| Memory | 754 GiB；采集时约 734 GiB available |
| Swap | 8 GiB；采集时未使用 |
| `/home` | ext4，14 TiB，总计约 5.5 TiB available |

## GPU 与驱动

| Index | UUID | 型号 | 显存 | PCI bus | 拓扑 |
|---|---|---|---|---|---|
| 0 | `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` | NVIDIA A100 80GB PCIe | 81920 MiB | `00000000:4A:00.0` | 与 GPU 1 为 `NV12` |
| 1 | `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` | NVIDIA A100 80GB PCIe | 81920 MiB | `00000000:61:00.0` | 与 GPU 0 为 `NV12` |

- NVIDIA driver：`580.178.04`。
- `nvidia-smi` 报告 CUDA compatibility version `13.0`。
- MIG：两张卡均 disabled。
- 采集时显存占用均为 0 MiB，没有 GPU process。该事实只代表采集时快照，不代表后续维护窗口仍为空闲。

## Docker 与 NVIDIA runtime

| 项目 | 已验证值 |
|---|---|
| Docker client/server | `29.1.3` / API `1.52` |
| containerd | `2.2.1` |
| runc | `1.3.4-0ubuntu1~24.04.1` |
| Storage driver | `overlayfs` |
| Cgroup | systemd，v2 |
| Default runtime | `runc` |
| Registered GPU runtime | `nvidia` -> `nvidia-container-runtime` |
| NVIDIA Container Toolkit | `1.20.0-1` |
| Docker Compose | `docker compose` 不可用 |
| Docker socket | `/var/run/docker.sock`，`root:docker`，mode `srw-rw----` |
| 开发用户 | `wujiaxing2` 属于 `docker` group |

Docker Compose plugin 不可用，现有容器没有 Compose labels，也没有 GPUStack systemd unit。当前部署应按独立 Docker 容器管理，不得假设存在 Compose project。

## 已部署 GPUStack

### 固定兼容基线

- 版本：`v2.2.3 (1cfcbbf)`。
- image：`gpustack/gpustack@sha256:55ecc762950a32641b6a4d96195c0662ef72998eaeeda31bba0e8e218d41afef`。
- image label revision：`1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`。
- 上游 tag：`v2.2.3` 精确指向上述 commit。
- 90 工作树通过 2217-byte 增量 Git bundle 导入该 tag；bundle SHA-256 为 `1c09bf12bfda3914dc46a92d43d56b0f909c863c181dc376e64e9a0541101a5f`。

### 运行拓扑

| 容器 | 状态 | 网络 | runtime/权限 | 挂载 | 重启策略 |
|---|---|---|---|---|---|
| `gpustack` | running | bridge；host `80` -> container `80` | `runc`；非 privileged | volume `gpustack-data` -> `/var/lib/gpustack` | `unless-stopped` |
| `gpustack-worker` | running | host | `nvidia`；privileged | `gpustack-data` 和 `/var/run/docker.sock` | `unless-stopped` |
| `open-webui` | running/healthy | bridge；host `3000` -> container `8080` | `runc`；非 privileged | `/opt/ai/open-webui/data` | `always` |

- Worker 公开参数只包含 `--server-url http://10.213.80.90` 与 `--worker-ip 10.213.80.90`。
- Worker 使用 `GPUSTACK_TOKEN` 环境变量；盘点只记录变量名，没有读取或输出值。
- `gpustack-data` 是 local Docker volume，mountpoint 为 `/var/lib/docker/volumes/gpustack-data/_data`。
- 采集时没有运行中的 vLLM 容器。历史 vLLM 测试容器保持 stopped 状态，本项目未清理或修改它们。

## 端口与直接网络路径

| 端口 | 当前用途 |
|---|---|
| 80 | GPUStack Server/UI |
| 3000 | Open WebUI |
| 10150 | Python Worker API |
| 10151 | Python Worker metrics |

- `http://127.0.0.1/healthz`、`http://127.0.0.1:10150/healthz` 和 `http://127.0.0.1:10151/metrics` 均返回 HTTP 200。
- 从 `gpustack` Server 容器访问 `http://10.213.80.90:10150/healthz` 返回 HTTP 200，证明 bridge 中的 Server 容器可以直接访问宿主机公开端口。后续 vLLM 使用相同 direct-network 方向，但准确测试端口仍必须在启动前检查冲突。
- R1 Rust Worker 不得占用 10150/10151；Rust Worker API、metrics 与 Lab/Managed inference 使用独立端口。

## Runner 与 vLLM 镜像

R1 的固定首选 Runner 为：

```text
quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af
tag: cuda13.0-vllm0.25.1
embedded VLLM_IMAGE_TAG: vllm/vllm-openai:v0.25.1-ubuntu2404
embedded VLLM_BUILD_COMMIT: 752a3a504485790a2e8491cacbb35c137339ad34
embedded CUDA_VERSION: 13.0.1
```

该镜像已经存在于主机，但本阶段未启动它。服务器上另有 `vllm/vllm-openai:v0.26.0` 和可变 `latest`（采集时实际为 v0.31.0）；正式证据不得使用可变 `latest`。

## 验证模型候选

### 首选单卡与双卡 smoke model

- path：`/home/wujiaxing2/flowscale-system-validation/models/Qwen-7B`。
- size：约 15 GiB。
- architecture：`QWenLMHeadModel`；32 layers、32 attention heads、hidden size 4096。
- 文件名/大小 manifest SHA-256：`03f9cb2ad053674b076b5f93ef5389bfb482c01b5de1a603dd2909ba8bc9bc62`。
- `config.json` SHA-256：`69dc0b62b70422fcb2c14df80852bae544ff8087a390817108753d2fc194f27c`。
- `model.safetensors.index.json` SHA-256：`0399fe41ec7ca1ec5ac449c80a2a9cd469eccfdace61d3d8c5200766d0bf2bbf`。
- `tokenizer_config.json` SHA-256：`9d4a57f708b43924364068ff3ae5e4c0aa9cf5bdab23d971a0c8f83137b81f52`。

该目录属于当前用户且模型权重完整。历史容器曾使用 vLLM v0.2.2 成功退出，只能作为模型目录曾可加载的诊断证据；它不证明固定 Runner v0.25.1 已通过。正式兼容性将在批准的 GPU 窗口内验证。

### 备用模型

- `/home/wujiaxing2/flowscale-system-validation/models/Qwen-14B`，约 29 GiB。
- 文件名/大小 manifest SHA-256：`b7d4ac9ac87f97716e558446af2c9b4fb55659a3046540322095df3de5c17cbe`。
- `/data/models/Qwen3-0.6B` 采集时只有空目录大小，不能作为有效模型。
- `/home/yanghaoyu/models/...` 对当前用户不可访问，不能作为本项目依赖。

## 开发工具链

- Git `2.43.0`、OpenSSH `9.6p1`。
- `rustc`、`cargo`、`rustup` 均未安装。
- 已有 `pkg-config 1.8.1`、CMake `3.28.3`、GCC `13.3.0`、GNU Make `4.3`；`clang` 未安装。
- M2 必须先安装并固定 Rust toolchain，再创建 workspace；工具链安装本身不是 M1 证据。

## 维护与回滚边界

当前 Python Worker 是 `gpustack-worker`，Server 是独立的 `gpustack` 容器。后续 GPU 测试窗口必须：

1. 再次确认 Server 中无活动模型实例、两个 GPU 空闲且没有未知推理容器；
2. 保持 `gpustack` Server 运行，只停止 `gpustack-worker`；
3. 使用不同的 Rust Worker identity、目录、端口和所有权 labels；
4. 测试后只清理 Rust 所有的资源；
5. 通过重新启动原 `gpustack-worker` 并验证 10150 `/healthz` 恢复完成回滚。

M1 没有实际执行 stop/start。真实 drain、停止和回滚演练仍需明确维护窗口，并作为后续集成门禁记录。
