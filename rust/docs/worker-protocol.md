# Rust Worker 协议与 HTTP 兼容性

## 兼容基线

R1 以 90 服务器上已经运行的 GPUStack `v2.2.3` 为准确目标，源码 commit 固定为 `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`。部署 image tag、digest 与环境身份见 `environment-baseline.md`。R1 实现和验证期间不升级 Server。

M3 的准确 endpoint、schema、认证、分页、watch framing、错误和 fixture 见 `server-contract-v2.2.3.md` 与 `rust/fixtures/server-v2.2.3/`。设计阶段检查过的其他 commit 不再是兼容性证据。

## Worker 到 Server 的操作

类型化 Rust client 必须支持已部署 Server 中与下列功能对应的接口：

- Server 版本兼容性检查；
- `POST /workers` 注册；
- 获取已认证 Worker 身份；
- `POST /worker-heartbeat`；
- `POST /worker-status`；
- Model、ModelInstance 与 ModelFile 的 list/watch 操作；
- 不分页的权威 Model/ModelInstance 全量列表，以及按当前 `worker_id` 过滤的 ModelFile 全量列表；
- 按 ID 读取最新 ModelFile，作为完整更新的基准；
- 更新 ModelInstance observed state；
- 更新分配给当前 Worker 的完整 ModelFile；
- R1 CLI 使用的运维侧 model CRUD 与日志 API；
- R1 CLI 使用的 OpenAI-compatible 推理请求。

准确 prefix、schema、认证 header、null 行为、enum 拼写与错误响应体已经在 M3-A 从固定源码、公开只读响应和同版本 Pydantic 序列化中形成契约 fixture。第一次隔离身份集成测试必须再以真实认证响应交叉验证 synthetic 样本。

## M3-D 真实协议验证子集

第一次真实集成只允许 `control-plane-only` 子集：版本检查、注册或 `me` 身份恢复、heartbeat、status，以及三类资源的 list/watch/full-sync。注册请求的 maintenance 必须类型化为 `enabled=true`，status 必须携带空 GPU 列表；M3-D 不调用 ModelInstance 或 ModelFile update，也不启动任何工作负载。

Rust Worker 在独立的 `10.213.80.90:20150` 提供最小 `GET /healthz`，供 Server 探测真实进程。该响应只证明 liveness；M3-D 不宣称 `/readyz`、metrics 或其他 Worker API 已实现。真实运行时必须记录 Server 可见状态、心跳推进、watch/full-sync、重启身份恢复和无副作用证据，但不得记录任何 token 或 Authorization header。

## Worker HTTP API

M4 在显式 `--resource-api-only` 配置中扩展 `20150` 上的 HTTP API，并在 `20151` 提供 metrics。该配置继续保持 maintenance，不构造 Docker、ModelFile manager、ModelInstance reconciler 或 vLLM。M3-D `--control-plane-only` 仍只提供 `/healthz`，不能隐式获得这些能力。

固定 v2.2.3 Python Worker 是 schema、查询参数和错误形状的兼容 oracle；下述安全和 readiness 差异是已接受的有意增强，必须由契约测试固定，不能被实现时静默改回。

### 公开探针

| Method | Path | 契约 |
|---|---|---|
| GET | `/healthz` | 进程存活；不代表已经具备 Server 工作就绪条件。 |
| GET | `/readyz` | resource-api-only 固定返回 503；未来完整 Managed 只有先决条件满足且第一次权威协调完成后才返回成功。 |
| GET | `/version` | 返回 Rust 版本、Git commit、build target 与协议基线。 |

### 需要 Worker 认证的 endpoint

| Method | Path | 契约 |
|---|---|---|
| GET | `/token-auth` | 只验证持久化 per-Worker token，不暴露凭据值；保留固定版本对 `X-Higress-Llm-Model` header 的兼容行为。 |
| GET | `/files/file-exists` | 报告允许模型根目录下某路径是否存在及其类型。 |
| GET | `/files/model-config` | 只针对允许的模型配置文件名返回解析后的 JSON。 |
| GET | `/files/model-weight-size` | 返回允许的本地模型目录中的权重字节数。 |
| GET | `/serveLogOptions/{id}` | 返回兼容的当前/上次重启以及容器日志选项。 |
| GET | `/serveLogs/{id}` | 流式返回兼容模型日志，并支持已部署版本的查询语义。 |
| GET | `/v2/config` | 返回非敏感的 Worker 配置视图。 |

`/serveLogs/{id}` 保留固定版本的 `tail`、`follow`、`previous`、`model_instance_name`、`model_file_id` 和 `container_name` 查询语义。M4 没有 Docker runtime；需要容器日志的请求必须明确返回不支持，不能伪造日志。文件和 Worker 自身日志只允许映射到受控日志目录。

兼容基线可能要求不同的带版本 config prefix；此时 M1 证据优先于这里通过源码检查得到的默认值。

### 指标

`GET /metrics` 在独立的 `10.213.80.90:20151` 上无需认证地暴露 Prometheus 格式。M4 覆盖 Worker mode/readiness、Server 连通性、heartbeat、状态上报、事件重连以及 CPU、内存、文件系统和 GPU 资源；Docker、工作负载健康与模型数量指标只在后续相应组件启用后出现。

### 延后实现的 endpoint

M4 有意不实现 `PUT /v2/config`、`/files/parse-gguf`、benchmark 日志、cache-service 日志、Python debug 界面、推理代理、cluster proxy 和动态配置写入。对于已知但不支持的路由或 method，返回稳定的 404、405 或明确不支持错误，不得静默模拟成功。

## 认证与授权

探针和 metrics 无需认证，且不返回敏感信息。M4 其他 Worker HTTP endpoint 只接受注册后持久化的 per-Worker token，不接受 Cluster registration token。凭据比较必须 constant-time，失败统一返回通用 401，日志、CLI 和测试证据中只保留脱敏标识。

Managed CLI 使用运维人员凭据请求 Server。Server 在代理模型日志请求前继续执行现有 tenant 与授权检查。Worker 系统凭据绝不用来创建用户资源。

## 资源上报契约

M4 将 Worker status 的 GPU 与系统资源字段改为固定 v2.2.3 对应的类型化结构，不用通用 JSON 值代替已知 schema。盘点至少包含 CPU 数量/利用率、内存/swap、文件系统、OS/kernel、boot time、uptime、machine ID，以及 GPU index/device index/chip index、UUID、名称、NVIDIA vendor、CUDA 类型、driver、CUDA runtime、compute capability、core utilization、显存 total/used/utilization、`unified=false` 和 temperature。status 同时报告 `metrics_port=20151` 并继续保持 maintenance。

NVML 初始化失败时 resource-api-only 启动失败或保持不可用，不得用空 GPU 冒充成功。可选瞬时 metric 读取失败可以显式表示缺失或降级；正式 M4 证据仍要求两张 A100 的 index、UUID、名称和关键容量字段与 `nvidia-smi` 一致。

## Readiness 定义

resource-api-only 没有 Docker 和 reconciler，所以 `/readyz` 固定为 503，即使 NVML、Server 连接和 HTTP API 正常也不能宣称完整工作就绪。固定 Python v2.2.3 的 `/readyz` 始终返回 200；Rust 的 503 是有意收紧，因为 Server reachability 只依赖 `/healthz`。

未来完整 Managed 只有 Docker 与 NVML 可用、注册成功、已发布初始状态、已获得权威 Model、ModelInstance 和当前 Worker ModelFile 快照且第一次协调完成，readiness 才为 true。持续的 Server 失联或不可恢复的 runtime 故障会使 readiness 变为 false，但 liveness 可以保持 true。

## 事件与协调契约

ModelInstance watch event 把实例标记为 dirty，并触发按 key 协调；ModelFile watch event 更新当前 Worker 的文件准备队列。ModelFile 准备完成后由 Server 推进 ModelInstance，并通过独立的 ModelInstance watch 通知 Worker。每次重连都触发一次权威全量 list，周期性权威 sync 作为最后兜底。清理决策绝不能来自临时空 cache、失败请求或不完整分页。

Worker 只处理分配给自身 Worker identity 的实例和 `worker_id` 等于当前 Worker ID 的 ModelFile。R1 拒绝跨 Worker 分布式放置；同一 Worker 的 assignment 可以包含一张或两张 GPU 的 index。

## 模型状态上报

R1 保留并遵守固定 v2.2.3 的完整状态词汇和状态所有权：

```text
PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING
                                      |
                                      v
                                DOWNLOADING -> STARTING -> RUNNING
                                      |            |           |
                                      +----------> ERROR <------+
```

Server/Scheduler 负责 `PENDING`、`ANALYZING`、调度和 `SCHEDULED`。Worker 接受分配给自身的 `SCHEDULED` 后，可以在完成任务与端口准备时更新为 `INITIALIZING`。Server Controller 根据关联 ModelFile 把实例推进为 `DOWNLOADING`，并在所有文件 `ready` 后推进为 `STARTING`。Worker 不得自行跳过这两个 Server 状态；只有观察到 `STARTING` 才能创建或接管 vLLM，且只有 vLLM 健康检查成功后才能上报 `RUNNING`。Server 继续负责 unreachable Worker 的解释。

本地模型仍进入 `DOWNLOADING` 协调阶段，但 `local_path` 的文件准备不进行网络下载。状态消息必须简洁、可执行且已脱敏。

## ModelFile 协议

ModelFile 状态值为 `downloading | ready | error`。Server 为 `INITIALIZING` 实例创建或关联 ModelFile；Worker 通过 `GET /model-files?page=-1&worker_id=<id>` 和对应 watch 流只接收属于自身的文件。

Worker 对 `local_path` 至少执行允许根目录检查、symlink canonicalization、文件/目录存在性和模型分片解析。成功时写回 `download_progress=100`、`resolved_paths` 和 `state=ready`；失败时写回 `state=error` 与脱敏的 `state_message`。

`PUT /model-files/{id}` 使用完整 `ModelFileUpdate`，不是 merge patch。每次更新前先用 `GET /model-files/{id}` 读取最新 public 对象，再转换为 update 并只修改目标字段；随后必须再次核对 `worker_id`，并保留来源、路径、清理策略、LoRA、大小和其他已知 schema 字段，不能用仅含状态或可能过期的 watch cache 覆盖 Server 数据。ModelFile watch/full-sync 的断线、未知事件和删除安全规则与 ModelInstance 相同。

## Lab 状态边界

Lab 不注册 Server，不创建 ModelInstance 或 ModelFile，也不写入上述状态。它使用本地 `CREATED -> PREPARING -> STARTING -> RUNNING` 状态以及 `FAILED`、`STOPPED` 旁路；本地路径解析、Docker runtime 和 vLLM 健康检查与 Managed 复用实现，但 Lab 结果只能作为诊断或冒烟证据。

## 文件系统安全

所有请求路径必须是绝对路径，并在 canonicalize 和解析 symlink 后仍位于 `/home/wujiaxing2/flowscale-system-validation/models`。这项限制有意强于固定 Python v2.2.3 只做一般路径安全检查而没有传入 base path 的实现；Server 提交的合法本地模型路径仍保持兼容。

model config 采用文件名白名单和读取大小上限，不提供通用文件读取。权重统计只覆盖固定版本支持的 `.safetensors`、`.bin`、`.pt` 和 `.pth` 本地目录语义。日志 ID、模型名称和容器名称必须映射到受控日志目录，不能变成任意路径。`GET /v2/config` 只输出非敏感白名单，不返回 token、凭据内容或其他 secret。

访问失败、JSON 格式错误、文件类型错误、相对路径、symlink 越界与根目录外路径，都有由契约测试固定的稳定错误类别。
