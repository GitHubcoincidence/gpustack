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
- model 与 ModelInstance 的 list/watch 操作；
- 不分页的权威 ModelInstance 全量列表；
- 更新 ModelInstance observed state；
- R1 CLI 使用的运维侧 model CRUD 与日志 API；
- R1 CLI 使用的 OpenAI-compatible 推理请求。

准确 prefix、schema、认证 header、null 行为、enum 拼写与错误响应体已经在 M3-A 从固定源码、公开只读响应和同版本 Pydantic 序列化中形成契约 fixture。第一次隔离身份集成测试必须再以真实认证响应交叉验证 synthetic 样本。

## Worker HTTP API

### 公开探针

| Method | Path | 契约 |
|---|---|---|
| GET | `/healthz` | 进程存活；不代表已经具备 Server 工作就绪条件。 |
| GET | `/readyz` | 只有先决条件满足且第一次权威协调完成后才返回成功。 |
| GET | `/version` | 返回 Rust 版本、Git commit、build target 与协议基线。 |

### 需要 Worker 认证的 endpoint

| Method | Path | 契约 |
|---|---|---|
| GET | `/token-auth` | 验证 Worker 或注册凭据，不暴露凭据值。 |
| GET | `/files/file-exists` | 报告允许模型根目录下某路径是否存在及其类型。 |
| GET | `/files/model-config` | 只针对允许的模型配置文件名返回解析后的 JSON。 |
| GET | `/files/model-weight-size` | 返回允许的本地模型目录中的权重字节数。 |
| GET | `/serveLogOptions/{id}` | 返回兼容的当前/上次重启以及容器日志选项。 |
| GET | `/serveLogs/{id}` | 流式返回兼容模型日志，并支持已部署版本的查询语义。 |
| GET | `/v2/config` | 返回非敏感的 Worker 配置视图。 |

兼容基线可能要求不同的带版本 config prefix；此时 M1 证据优先于这里通过源码检查得到的默认值。

### 指标

`GET /metrics` 暴露 Prometheus 指标，覆盖 Worker readiness、Server 连通性、heartbeat、状态上报、事件重连、协调、Docker 调用、工作负载健康、模型数量与 GPU 资源信息。准确端口在 M1 后冻结。

### 延后实现的 endpoint

R1 有意不实现动态写配置、推理代理、cluster proxy、GGUF 解析、benchmark 日志、cache-service 日志和 Python debug 界面。对于这些功能应明确返回不支持，而不是静默模拟。

## 认证与授权

探针 endpoint 无需认证，且不返回敏感信息。其他 Worker HTTP endpoint 根据冻结的 Server 契约，要求 Worker 凭据或注册凭据。适用时使用 constant-time 凭据比较，错误信息保持通用，日志中只保留脱敏标识。

Managed CLI 使用运维人员凭据请求 Server。Server 在代理模型日志请求前继续执行现有 tenant 与授权检查。Worker 系统凭据绝不用来创建用户资源。

## Readiness 定义

只有 Docker 与 NVML 可用、注册成功、已发布初始状态、已获得权威模型快照且第一次协调完成，readiness 才为 true。持续的 Server 失联或不可恢复的 runtime 故障会使 readiness 变为 false，但 liveness 可以保持 true。

## 事件与协调契约

Watch event 把 ModelInstance 标记为 dirty，并触发按 key 协调。每次重连都触发一次权威全量 list，周期性权威 sync 作为最后兜底。清理决策绝不能来自临时空 cache、失败请求或不完整分页。

Worker 只处理分配给自身 Worker identity 的实例。R1 拒绝跨 Worker 分布式放置；同一 Worker 的 assignment 可以包含一张或两张 GPU 的 index。

## 模型状态上报

R1 上报现有状态词汇：

```text
SCHEDULED -> INITIALIZING -> STARTING -> RUNNING
                                |           |
                                +-> ERROR <-+
```

本地模型部署不进入 `DOWNLOADING`。状态消息必须简洁、可执行且已脱敏。Server 继续负责 `PENDING`、`ANALYZING`、调度和 unreachable Worker 的解释。

## 文件系统安全

所有请求路径都必须 canonicalize 并解析 symlink，结果必须仍位于配置的模型根目录之下。配置读取采用文件名白名单。这些 endpoint 不提供通用文件读取能力。访问失败、JSON 格式错误、文件类型错误与越界路径，都有由契约测试固定的稳定错误类别。
