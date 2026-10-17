# Rust Worker：架构与接口设计

## 职责与边界

Rust Worker 负责节点资源盘点、连接 Server、提供 Worker HTTP API、协调期望状态与实际状态、管理 Docker 工作负载生命周期、检查 vLLM 健康状态、提供日志以及故障恢复。它不负责模型业务状态、调度策略、网关路由、用户授权或 vLLM 本身的实现。

现有 Server 仍是 Managed 模型规格、调度位置和期望生命周期的权威来源。Runner 镜像继续承载 Python/CUDA/vLLM。Rust runtime 只替换 R1 所需的 Docker、NVML 与系统功能。

## Workspace 边界

已接受的初始 workspace 为：

```text
rust/
  Cargo.toml
  Cargo.lock
  crates/
    gpustack-cli/
    gpustack-client/
    gpustack-worker/
    gpustack-runtime/
  docs/
```

四个 crate 是稳定的职责边界：

- `gpustack-cli`：命令解析、context、输出、退出码与命令路由。
- `gpustack-client`：类型化 Server REST、watch stream、Worker 注册与 OpenAI-compatible 推理客户端。
- `gpustack-worker`：supervisor、HTTP API、期望状态来源、reconciler、vLLM 规划、状态上报、本地状态与日志服务。
- `gpustack-runtime`：Docker Engine API、NVML、宿主机资源信息、工作负载状态与日志原语。

在出现明确且有证据支持的依赖边界前，不增加 `common` 或 `types` crate。

## 运行模式

### Managed 模式

```text
CLI 或 Web UI
  -> GPUStack Server
  -> scheduler 和 ModelInstance 期望状态
  -> Rust Worker watch/full-sync client
  -> reconciler
  -> Docker Engine
  -> gpustack/runner vLLM container
  -> direct inference endpoint
```

Managed 模式由 `gpustack-rs worker run` 启动。模型命令调用 Server，绝不直接创建生产容器。

### Lab 模式

```text
gpustack-rs worker lab ...
  -> Lab desired-state adapter
  -> 同一个 reconciler 和 WorkloadSpec
  -> 同一个 Docker runtime 和 health checker
  -> 隔离的 Lab container 和本地状态存储
```

Lab 模式不向 Server 注册，不创建 Server 数据库记录，不使用 Worker token，也不创建 gateway route。Lab 证据只属于诊断或冒烟证据，不能代替 Managed 验收证据。

## 状态所有权

| 状态 | 权威来源 |
|---|---|
| 模型规格、副本数、放置位置、Worker/GPU 分配 | Server |
| Worker 身份与系统凭据 | Server |
| 容器是否存在、运行状态、退出码与镜像摘要 | Docker Engine |
| GPU 身份与实时利用率 | NVML，并与 Docker 信息关联 |
| 端口预留、重启次数、日志与本地操作元数据 | Rust Worker |
| 模型是否就绪 | Rust Worker 对 vLLM 健康端点的解释 |

本地 manifest 只用于辅助恢复，不是权威来源。必须能根据 Server 的权威快照和 Docker 所有权标签重建它们。

## 启动生命周期

1. 解析配置，并获取数据目录的排他进程锁。
2. 启动 HTTP listener；liveness 可用，但 readiness 为 false。
3. 验证 Docker、NVML、目录、模型根目录与服务端口范围。
4. 验证与已冻结 Server 基线的兼容性。
5. 注册 Worker，并原子保存返回的身份与凭据。
6. 发布初始系统与 GPU 状态。
7. 获取不分页的权威 ModelInstance 快照。
8. 枚举带有 Rust 所有权标签的 Docker 容器。
9. 执行第一次全量协调，并接管匹配且健康的容器。
10. 启动 heartbeat、状态上报、watch stream、健康检查与定期 full sync 循环。
11. 只有在第一次权威收敛完成后，readiness 才变为 true。

## 协调模型

事件只用于加速，而不是唯一事实来源。按 ModelInstance key 去重的工作队列保证每个实例的协调过程串行执行。启动时、watch 重连后以及可配置的周期到达时，都必须触发权威 full sync。

```text
desired ModelInstance + actual Docker workload + local recovery record
  -> normalize WorkloadSpec
  -> compare identity and spec hash
  -> create, adopt, replace, stop, or report
  -> persist local record
  -> report observed state
```

只有收到明确且权威的删除/取消分配事件，或者成功取得 Server 全量快照后，才允许执行删除。绝不能把空 watch cache 或失败的 API 请求解释为删除许可。

Managed 容器至少携带以下标签：

```text
io.gpustack.implementation=rust
io.gpustack.mode=managed
io.gpustack.worker-uuid=<uuid>
io.gpustack.model-instance-id=<id>
io.gpustack.spec-hash=<hash>
```

Lab 容器携带 `io.gpustack.mode=lab` 与一个 Lab 标识。每次变更容器前都要验证完整所有权 tuple。

## 模型生命周期

R1 沿用现有状态词汇，但不经过下载阶段：

```text
SCHEDULED -> INITIALIZING -> STARTING -> RUNNING
                                |           |
                                +-> ERROR <-+
```

- `INITIALIZING`：验证路径、镜像、GPU 分配、端口与规范化工作负载计划。
- `STARTING`：拉取或检查镜像，创建并启动容器，持久化日志，并等待健康检查 deadline。
- `RUNNING`：vLLM 健康探针成功。
- `ERROR`：启动、健康或运行时故障导致当前无法收敛。

启用 `restart_on_error` 后，重试延迟依次为 0、10、20、40、80 秒等，最大 300 秒。保留重启次数以及当前和上一次日志。Worker 退出时不移除健康的 Managed 容器。

## 故障行为

- Server 不可用：保留健康工作负载、停止破坏性协调、重试连接；超过配置阈值后将 readiness 设为 false，并在恢复后执行 full sync。
- Watch stream 不可用：保持心跳与工作负载运行，退避重连，随后执行 full sync。
- Docker 暂时不可用：报告 Worker 健康降级，经过宽限期后重新枚举工作负载；不得立刻把全部实例标记为失败。
- vLLM 启动失败：保存已脱敏诊断信息，设置 `ERROR`，并执行配置的重启策略。
- 未知容器：可以报告，但绝不修改。

## R1 vLLM WorkloadSpec

规范化内部计划包含：

```text
instance identity
model path
image reference and resolved digest
GPU UUIDs and runtime indexes
service port
tensor parallel size
vLLM arguments
environment
mounts and shared-memory settings
health deadline
ownership labels
spec hash
```

镜像选择顺序为：模型显式指定镜像、配置中的默认 vLLM 镜像、硬失败。R1 实现不导入也不调用 Python Runner catalog。

结构化选项包括 tensor parallel size、maximum model length、GPU memory utilization、dtype、served model name、trust remote code 与 eager execution。允许重复传入受控的原始 vLLM 参数，但这些参数不能覆盖由 Worker 管理的模型路径、服务端口、GPU 分配、容器身份或 mount。

## 路径契约

Docker bind source 由宿主机 Docker daemon 解释，而不是由 Worker 容器解释。因此，R1 要求配置的模型根目录在宿主机和 Worker 容器中使用相同路径，例如：

```text
host /data/models -> Worker /data/models -> vLLM /data/models
```

每个文件系统 API 都必须解析 symlink，并确保结果仍位于允许的模型根目录之下。禁止任意访问宿主机文件系统。

## 配置与敏感信息

配置优先级为：

```text
CLI > environment > YAML file > defaults
```

预期 Linux 路径为：

```text
/etc/gpustack-rust/config.yaml
/var/lib/gpustack-rust-worker/managed/
/var/lib/gpustack-rust-worker/lab/
/var/log/gpustack-rust-worker/
```

准确运行时路径在 M1 期间验证。配置中记录 secret 文件引用，而不是 secret 值。Worker 系统凭据、运维 API 凭据与 Git 凭据彼此分离。未来只有少量非敏感运行配置白名单可以支持热更新；身份、Docker endpoint、根目录与数据目录的变更都要求重启。

## R1 容器化拓扑

正式 R1 拓扑为：Rust Worker 运行在容器中，通过宿主机 Docker Engine 管理同级 Runner 容器。预期基线包括 host network、Docker socket、NVML 访问、持久化 Worker 状态与日志 volume，以及路径相同的模型根目录 mount。准确的 Compose 参数与 user/group 权限在 M1 盘点后冻结。

在第一次单 GPU 冒烟测试完成前，允许把 Worker 作为宿主机进程运行。所有正式 R1 恢复测试与双 GPU 证据都必须使用 Worker 容器重新执行。

## 可观测性与恢复记录

Worker 输出结构化且脱敏的日志，并提供 Prometheus 指标，覆盖 readiness、Server 连通性、watch 重连、协调过程、Docker 调用、工作负载状态、健康检查失败和 GPU 资源信息。每个实例使用原子写入的 manifest 记录 container ID、spec hash、image digest、GPU UUID、port、restart count 与 last observed state。即使 manifest 丢失，也不能阻止恢复。
