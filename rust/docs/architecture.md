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

Lab 使用独立的本地部署状态，不复用或伪造 Server 的 `ModelInstance` 状态：

```text
CREATED -> PREPARING -> STARTING -> RUNNING
              |            |           |
              +----------> FAILED <----+

RUNNING -> STOPPED
```

这些状态只描述一次本地实验部署。Lab 可以直接解析本地模型路径并继续启动容器，但必须复用 Managed 路径将使用的模型路径校验、分片解析、Docker planner、runtime 和健康检查；它不创建 `ModelFile`，也不向 Server 上报任何模型生命周期状态。

### M3-D control-plane-only 验证模式

`gpustack-rs worker run --control-plane-only` 是完整 Managed 模式之前的受限启动配置，不是第二套生产模式。它只运行最小 `/healthz`、Server 版本检查、Worker 注册/恢复、heartbeat/status 和三资源 list/watch/full-sync。注册请求固定携带 `maintenance.enabled=true`，状态固定上报空 GPU；进程不构造 runtime、GPU inventory、ModelFile manager 或 ModelInstance reconciler，因此不能接受或执行模型任务。

该模式用于把真实 Server 协议证据与 Docker/GPU/runtime 证据分离。M4 以后逐步启用的能力不能在 M3-D 中被隐式探测或自动开启；完整 Managed 启动路径仍以下述生命周期为目标。

### M4 resource-api-only 验证模式

`gpustack-rs worker run --resource-api-only` 是 M4 的显式受限启动配置。它复用已持久化的 Server Worker 身份和 per-Worker token，继续注册或恢复为 maintenance；启用 NVML/系统资源盘点、完整 M4 Worker HTTP API 和独立 metrics listener，但不构造 Docker client、ModelFile manager、ModelInstance reconciler 或 vLLM runtime，也不发送任何模型状态更新。

五个运行配置的能力边界如下：

| 启动配置 | 资源上报 | Worker API | metrics | Docker/vLLM | 调度状态 |
|---|---|---|---|---|---|
| `--control-plane-only` | 空 GPU | 仅 `/healthz` | 无 | 禁用 | maintenance |
| `--resource-api-only` | 真实系统/NVML | M4 完整只读 API | `20151` | 禁用 | maintenance |
| `--managed-file-only` | 真实系统/NVML | M4 完整只读 API | `20151` | 禁用 | 运维显式控制 |
| `--managed-runtime` | 真实系统/NVML | 完整 R1 API 与推理代理 | `20151` | 单 GPU vLLM | 运维显式控制 |
| 未来普通 `worker run` | 真实系统/NVML | 完整 R1 API | 启用 | 启用 | 首次收敛后按 Managed 状态 |

resource-api-only 使用 Worker API `10.213.80.90:20150`、metrics `10.213.80.90:20151` 和模型根目录 `/home/wujiaxing2/flowscale-system-validation/models`。`/healthz` 只证明 listener 存活；该模式缺少 Docker 和协调器，因此 `/readyz` 固定返回 503。完整 Managed 的 readiness 仍遵循下述生命周期。M4 不能修改 control-plane-only 的空 GPU、无 metrics 和最小 API 语义。

### M6-A managed-file-only 验证模式

`gpustack-rs worker run --managed-file-only` 复用 resource-api-only 的真实资源、Worker API、
metrics、持久身份和三资源控制面，并增加 Managed 端口分配、ModelInstance 初始化与
ModelFile 本地路径准备协调器。进程启动时要求 Rust Worker 仍处于 maintenance；是否解除
maintenance 只能由运维 Context 的显式 CLI 决定，Worker 不得自行改变该状态。

该模式不构造 Docker client、WorkloadSpec 或 vLLM runtime。观察到 Server 权威
`STARTING` 后只记录状态，不创建或接管容器，也不把实例推进为 `RUNNING`。它是 M6-A
真实协议验证的有界安全模式，不是最终生产运行模式。

### M6-B managed-runtime 验证模式

`gpustack-rs worker run --managed-runtime` 是 M6-B 唯一启用 Managed Docker/vLLM 的显式
模式。它继承 `managed-file-only` 的资源、Worker API、metrics、持久身份、文件准备和
maintenance 启动门，再增加 runtime reconciler、动态 readiness、当前 session 日志和
Worker 推理代理。它与前三种受限模式互斥，不改变它们已经接受的行为。

`managed-runtime` 的 `/readyz` 只有在资源采集、Docker 先决条件和首次权威 runtime 收敛
完成后返回 200；单个模型未部署或进入 ERROR 不等同于 Worker 未就绪。运维人员仍须通过
CLI 显式解除 maintenance，Worker不能为了获取任务自行切换该字段。

## 状态所有权

| 状态 | 权威来源 |
|---|---|
| 模型规格、副本数、放置位置、Worker/GPU 分配 | Server |
| Worker 身份与系统凭据 | Server |
| `PENDING`、`ANALYZING`、`SCHEDULED` 和不可达解释 | Server/Scheduler |
| `ModelFile` 创建、关联以及据此推进 `DOWNLOADING`、`STARTING` | Server Controller |
| 分配任务的初始化事实、`ModelFile` 准备结果和 vLLM 健康事实 | Rust Worker |
| 容器是否存在、运行状态、退出码与镜像摘要 | Docker Engine |
| GPU 身份与实时利用率 | NVML，并与 Docker 信息关联 |
| 端口预留、重启次数、日志与本地操作元数据 | Rust Worker |
| 模型是否就绪 | Rust Worker 对 vLLM 健康端点的解释 |

本地 manifest 只用于辅助恢复，不是权威来源。必须能根据 Server 的权威快照和 Docker 所有权标签重建它们。

## 完整 Managed 启动生命周期

1. 解析配置，并获取数据目录的排他进程锁。
2. 启动 HTTP listener；liveness 可用，但 readiness 为 false。
3. 验证 Docker、NVML、目录、模型根目录与服务端口范围。
4. 验证与已冻结 Server 基线的兼容性。
5. 注册 Worker，并原子保存返回的身份与凭据。
6. 发布初始系统与 GPU 状态。
7. 获取不分页的权威 Model、ModelInstance 与分配给当前 Worker 的 ModelFile 快照。
8. 枚举带有 Rust 所有权标签的 Docker 容器。
9. 执行第一次全量协调，并接管匹配且健康的容器。
10. 启动 heartbeat、状态上报、watch stream、健康检查与定期 full sync 循环。
11. 只有在第一次权威收敛完成后，readiness 才变为 true。

## 协调模型

事件只用于加速，而不是唯一事实来源。按 ModelInstance key 去重的工作队列保证每个实例的协调过程串行执行。启动时、watch 重连后以及可配置的周期到达时，都必须触发权威 full sync。

```text
desired Model + ModelInstance + ModelFile + actual Docker workload + local recovery record
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
io.gpustack.component=vllm
```

Lab 容器携带 `io.gpustack.mode=lab` 与一个 Lab 标识。每次变更容器前都要验证完整所有权 tuple。

## Managed 模型生命周期

R1 与固定 GPUStack v2.2.3 的完整 `ModelInstance` 状态链保持一致：

```text
PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING
                                      |
                                      v
                                DOWNLOADING -> STARTING -> RUNNING
                                      |            |           |
                                      +----------> ERROR <------+

UNREACHABLE 由 Server 在 Worker 失联时解释。
```

状态迁移按以下所有权执行：

- Server 和 Scheduler 负责 `PENDING`、`ANALYZING`、调度结果与 `SCHEDULED`。
- Worker 只接受分配给自身的 `SCHEDULED` 实例；验证 assignment 和准备参数，从
  `40000-40063` 分配或复用服务端口并执行真实 bind 检查后，把 `port`、`ports` 与
  `INITIALIZING` 一起完整写回。端口不是 Server 预先分配的。
- Server Controller 为 `INITIALIZING` 实例创建或关联 `ModelFile`。当文件处于准备中时，Server 把实例推进为 `DOWNLOADING`。
- Worker 的 ModelFile manager 只处理分配给自身的文件。即使来源是 `local_path`，仍要执行允许根目录校验、symlink canonicalization、分片解析，并把 `resolved_paths`、progress 和 `READY` 或 `ERROR` 写回完整 `ModelFileUpdate`。本地路径不发生网络下载，但不能绕过 ModelFile 控制链。
- Server Controller 聚合关联文件；全部 `READY` 后写入解析结果，并把实例推进为 `STARTING`。Worker 不得自行跳过到该状态。
- Worker 只在观察到 `STARTING` 后创建或接管 vLLM 工作负载。Docker 容器处于 running 不是模型就绪证据；vLLM 健康探针成功后才能把实例更新为 `RUNNING`。
- 文件准备、启动、健康或运行时故障进入 `ERROR`；具体写入者由发生故障的层和固定 Server 协议决定，状态消息必须简洁、可执行且脱敏。

启用 `restart_on_error` 后，重试延迟依次为 0、10、20、40、80 秒等，最大 300 秒。保留重启次数以及当前和上一次日志。Worker 退出时不移除健康的 Managed 容器。

M6-B 只实现同一 Worker 进程内的重复事件幂等和精确单容器接管。零个完整匹配容器时创建，
恰好一个完整 tuple/spec 匹配时接管；多个匹配、标签缺失或 spec 冲突都 fail closed。Worker
crash/restart 后的完整恢复、restart-on-error、current/previous 日志轮转属于 M7，不能由
M6-B 的接管原语推断已经支持。

## Managed 推理与日志链路

固定 v2.2.3 的推理链路为：

```text
CLI/user -> Server /v1/chat/completions 或 /v1/completions
         -> Server 鉴权和 Model route
         -> Worker 20150 + X-GPUStack-Model-Instance + Worker token
         -> Rust Worker 健康 route table
         -> 本机 vLLM 40000-40063
```

Worker只代理明确支持的 OpenAI 路径。它用严格 parser 从
`X-GPUStack-Model-Instance` 解析 ModelInstance ID，并从内部健康 route table 查找端口；
请求不能指定任意 upstream。转发前删除 Worker `Authorization` 和 hop-by-hop headers，
buffered 与 SSE streaming 都不得把系统凭据传给 vLLM。实例退出健康集合或开始清理时，
先从 route table 删除再停止 runtime。

runtime stdout/stderr 保存为 `<log_dir>/serve/<instance-id>.<restart-count>.log`，文件权限
0600；M6-B 的 restart count 固定为 0。Server 的 model-instance log API继续代理既有 Worker
`/serveLogOptions/{id}` 与 `/serveLogs/{id}`，因此用户日志 CLI 不直接读取 Docker socket。

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

### M5 WorkloadSpecV2 精确契约

M5 把语义计划和 Docker 实现分开。`gpustack-worker` 生成版本化的
`WorkloadSpecV2`，`gpustack-runtime` 只把该计划转换成 Docker Engine API 操作。
Lab 与 Managed 使用同一结构；两者只有期望状态来源和状态回写目标不同。

规范化结构至少包含：

```text
schema_version
mode and owner identity
deployment identity
canonical model path and served model name
requested image and resolved immutable digest
ordered GPU UUIDs and host indexes
bind address and service port
tensor parallel size
container numeric UID:GID
structured and controlled raw vLLM arguments
sorted environment and mounts
shared-memory and network settings
health path, interval and deadline
ownership labels
spec hash
```

M5 只允许单 GPU 和 tensor parallel size 1，但 GPU 字段保持有序数组，为后续双卡
扩展保留稳定接口。用户 raw arguments 不能覆盖模型路径、监听地址、端口、GPU、容器
名称、所有权标签、network 或 mount。

`spec_hash` 是规范化 JSON 的 SHA-256。结构字段顺序固定；语义无序的 map 和集合在
计算前排序，GPU 和 vLLM 参数等语义有序字段保留顺序。resolved image digest、模型
路径、GPU UUID、端口、容器 UID:GID、参数、环境、mount、network 和安全设置参与计算；时间戳、
container ID、实时状态、错误文本以及 `spec-hash` 标签本身不参与计算。

### Docker runtime 边界

M5 的 `gpustack-runtime` 通过 `/var/run/docker.sock` 直接调用 Docker Engine API，
不执行 `docker` CLI，也不依赖 Python `gpustack-runtime`、subprocess、PyO3 或 FFI。
其 R1 子集包括 daemon/version 探测、image inspect/resolve、按标签枚举、container
create/start/inspect/stop/remove、stdout/stderr log stream，以及把 Docker 状态转换为
稳定内部类型。镜像拉取策略支持 `never`、`if-missing` 和 `always`，正式 M5 smoke
固定为 `never` 并要求冻结 digest 已存在。

Python Worker 和已安装的第三方 `gpustack-runtime` 只作为行为 oracle。M5 不复制其
Kubernetes、Podman、pause container、多组件 workload、CDI 抽象或不健康辅助容器；
单个 vLLM 容器由 Rust reconciler 负责健康判断和生命周期。

### M5 容器契约与所有权

所有 Rust 容器都携带：

```text
io.gpustack.implementation=rust
io.gpustack.schema-version=2
io.gpustack.mode=lab|managed
io.gpustack.owner-id=<local-owner-id>
io.gpustack.component=vllm
io.gpustack.spec-hash=<sha256>
```

Lab 增加 `io.gpustack.lab-id=<uuid>`；Managed 增加既有的
`io.gpustack.worker-uuid` 和 `io.gpustack.model-instance-id`。Lab 的 owner ID 在数据
根目录首次初始化时生成并以 `0600` 原子持久化，不依赖 Server Worker identity 或
任何 token。

stop、remove、replace 和 cleanup 在每次变更前必须重新 inspect，并同时匹配
implementation、schema version、mode、owner ID、deployment ID 和 spec hash。缺失、
冲突或属于 Python/未知 scope 的资源只能报告，不能修改。容器名称由 mode 和不可变
deployment ID 派生；用户显示名称保存在 manifest，不作为唯一所有权依据。

Runner 容器固定使用 host network、10 GiB shared memory、Docker restart policy
`no`、NVIDIA DeviceRequest 和只读模型 mount。默认不启用 privileged，并设置
`no-new-privileges`；只有获批 smoke 证明固定 Runner 必须增加权限时，才能先讨论再
修订。容器使用启动 Rust Worker 的有效数字 `UID:GID`；Worker 固定容器内的
`USER`、`LOGNAME`、`HOME` 和 cache 环境，避免镜像中没有宿主 UID 的 passwd 项时身份
解析失败。模型之外的编译和运行缓存写入每个 deployment 的独立可写 cache 目录，且由
宿主 Worker 用户直接拥有和清理。

Lab 默认只监听 `127.0.0.1`，自动端口范围为 `42000-42063`；它只服务宿主机上的
`worker lab infer`，不创建 gateway route。Managed 监听配置的 Worker IP，由 Worker
从 `40000-40063` 分配端口并写回 Server。端口分配在数据目录排他锁下完成，排除权威
ModelInstance 已记录端口，分配时和 container start 前都进行真实 bind 检查；发现占用
时失败，不能终止占用者。已有实例端口在重试和恢复时保持不变。

M6-A 的本地 ModelFile resolver 只接受允许模型根目录内的绝对路径。所有输入先解析
symlink 并验证 canonical path 不越界；目录和普通文件返回 canonical path，分片文件要求
同一目录中的编号从 1 到声明总数完整、无重复且总数一致，再按编号稳定排序。resolver
递归计算实际字节数；不存在、不可读、越界或不完整分片均在 Docker 之前失败。

GPU 的用户输入可以是宿主机 index，但 planner 必须先解析并记录稳定 UUID，Docker
DeviceRequest 也使用 UUID。M5 拒绝多于一张 GPU、活跃 compute process、已被 Rust
记录占用或无法可靠确认空闲的目标 GPU，并且不提供强制抢占选项。

### M5 vLLM、健康与日志契约

固定 Runner 的 entrypoint 为 `tini --`，M5 生成的 command 形如：

```text
vllm serve <canonical-model-path>
  --host 127.0.0.1
  --port <allocated-port>
  --served-model-name <name>
  --tensor-parallel-size 1
```

maximum model length、GPU memory utilization、dtype、trust remote code 和 eager
execution 通过结构化选项加入。模型路径以宿主机和容器内相同的绝对路径只读挂载。

container running 只表示进程存在。Lab 必须在 deadline 内轮询 `/v1/models`，只有
HTTP 200 且响应包含期望 served model name 时才能进入 `RUNNING`。容器提前退出时
进入 `FAILED`，保存 exit code、脱敏错误和有限日志尾部；超时同样进入 `FAILED`，且
默认保留容器供诊断。`worker lab logs` 直接读取 Docker stdout/stderr，并支持 tail、
since 和 follow；M5 不把整份容器日志复制进 manifest。

### Lab 本地状态与幂等性

Lab 数据根目录为 `0700`，owner、deployment manifest 和锁文件为 `0600`，通过临时
文件、fsync 和原子 rename 更新。目录结构为：

```text
lab/
  owner.json
  deployments/<lab-id>.json
  cache/<lab-id>/
  lab.lock
```

manifest 保存 normalized spec、spec hash、container ID、resolved image digest、GPU
UUID、port、状态、脱敏错误和时间戳。manifest 是恢复辅助；Docker 标签仍是实际工作
负载所有权证据。`status` 每次重新 inspect Docker 并执行健康检查，不能只回显记录。

同名且相同 spec 的 deploy 是幂等操作：运行中的 workload 直接返回当前状态，已停止
的 owned workload 可以重新启动，container 丢失时可以按相同 spec 重建。同名但
spec 不同必须报告冲突并要求先 stop/remove，不能自动替换。stop 使用 30 秒优雅期限
并保留容器；remove 只接受 STOPPED 或 FAILED 的完整 owned tuple。cleanup 默认
dry-run，只列出当前 owner 的 Lab 容器、记录和 cache；执行清理必须显式确认，且不能
触及 Managed、Python 或未知资源。

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
