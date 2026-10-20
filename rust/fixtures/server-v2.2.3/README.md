# GPUStack Server v2.2.3 契约 fixture

本目录固定 Rust Worker 在 M3 使用的 Server 控制面协议样本。兼容目标为
GPUStack `v2.2.3`，源码 commit
`1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`，部署镜像 digest 见
`rust/docs/environment-baseline.md`。

## 证据分类

- `live-version.response.json`：2026-10-19 从 90 主机现有 Server 的公开
  `GET /version` 只读捕获。
- `unauthenticated.response.json`：2026-10-19 未携带凭据请求
  `GET /v2/workers` 的真实错误响应。没有读取或记录任何 token。
- 除下述 ModelFile 样本外的其他 `.json` 与 `.ndjson`：在现有 `gpustack` Server 容器内，使用固定版本
  的 Pydantic 类型和 FastAPI `jsonable_encoder` 构造的 synthetic 样本。所有
  identity、地址、时间和 token 都是保留测试值，不来自数据库或真实凭据。
- `model-files-full-sync.response.json`、`model-file-update.request.json` 与
  `model-file-watch.ndjson`：2026-10-20 直接根据固定 commit 的 ModelFile schema、
  route、Python Worker 更新顺序和事件协议离线构造，没有调用运行中的 Server 或
  Docker。这些样本固定本阶段需要的字段与状态，但不冒充 Pydantic 运行结果。
- `worker-maintenance-me.response.json`：2026-10-20 根据固定 commit 的
  `WorkerPublic` 与 `Maintenance` schema 离线构造，用于证明 M3-D Worker 在启动控制
  循环前会重新确认 Server 保存的 maintenance 门禁；它不是一次真实注册响应。

这些 synthetic 样本固定字段名、默认值、null、enum 拼写和事件编码，但不声称
是一次真实注册或模型部署的捕获。注册请求使用的候选 UUID 以 `...0001` 结尾，
注册响应中的 Server 权威 UUID 以 `...0002` 结尾，随后 status fixture 也使用
`...0002`。这个差异刻意模拟 v2.2.3 路由忽略首次请求 UUID 并生成新 UUID 的行为。
第一次隔离身份集成测试必须把真实响应与这些样本比较；若有差异，先更新契约证据，
再修改 Rust client。

## 文件与用途

| 文件 | 对应操作 |
|---|---|
| `live-version.response.json` | `GET /version` 成功响应 |
| `unauthenticated.response.json` | 通用未认证错误响应 |
| `worker-register.request.json` | `POST /v2/workers` 请求体；首次 UUID 是非权威候选值 |
| `worker-register.response.json` | 注册成功的 `WorkerRegistrationPublic` 响应体；UUID 由 Server 分配 |
| `workers-me.response.json` | `GET /v2/workers?me=true` 响应体 |
| `worker-maintenance-me.response.json` | M3-D maintenance Worker 的 `me=true` synthetic 响应体 |
| `worker-heartbeat.request.json` | `POST /v2/worker-heartbeat` 请求体；成功 body 为空、status 为 204 |
| `worker-status.request.json` | `POST /v2/worker-status` 请求体；成功 body 为空、status 为 204 |
| `models-full-sync.response.json` | `GET /v2/models?page=-1` 响应体 |
| `model-instances-full-sync.response.json` | `GET /v2/model-instances?page=-1` 响应体 |
| `model-instance-update.request.json` | `PUT /v2/model-instances/{id}` 的完整更新请求体 |
| `model-watch.ndjson` | `GET /v2/models?watch=true` 的非空事件行 |
| `model-instance-watch.ndjson` | ModelInstance CREATED 与 ID-only DELETED 事件行 |
| `model-files-full-sync.response.json` | `GET /v2/model-files?page=-1&worker_id=101` 响应体；首项也作为单项 GET 响应 |
| `model-file-update.request.json` | `PUT /v2/model-files/{id}` 的完整更新请求体 |
| `model-file-watch.ndjson` | ModelFile CREATED、UPDATED 与 ID-only DELETED 事件行 |

## 再生依据

主要源码依据均位于上述固定 commit：

- `gpustack/routes/routes.py`：`/v2` prefix、认证路由和 endpoint 挂载；
- `gpustack/routes/workers.py`：注册、status、heartbeat 与 204 语义；
- `gpustack/routes/models.py`、`gpustack/routes/model_instances.py`：list、watch 与更新；
- `gpustack/routes/model_files.py`：ModelFile list、get、watch、worker filter 与完整更新；
- `gpustack/schemas/workers.py`、`gpustack/schemas/models.py`、
  `gpustack/schemas/model_files.py`：schema、enum 与 null；
- `gpustack/worker/model_file_manager.py`：更新前刷新 public 对象以及完整
  `ModelFileUpdate` 的构造顺序；
- `gpustack/client/worker_manager_clients.py` 与 `generated_*_client.py`：Python
  Worker 的实际请求序列化方式；
- `gpustack/mixins/active_record.py`、`gpustack/server/coordinator/base.py`：分页、
  replay、心跳与事件编码。

部署的 `/openapi.json` 在捕获时为 `546580` 字节，SHA-256 为
`acb247255add4a162bb4d886a1bd659b38d265f8f03c827e16076cd417f3314a`。
OpenAPI 不包含 `worker-status` 和 `worker-heartbeat`，且因为 `/v2/workers` POST
被多个 router 挂载，注册成功响应在 OpenAPI 中显示为空 schema。上述三个操作必须
以 route 实现和 Python client 类型为准，不能从 OpenAPI 自动推断。
