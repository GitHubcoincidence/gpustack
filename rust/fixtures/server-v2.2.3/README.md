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
- 其他 `.json` 与 `.ndjson`：在现有 `gpustack` Server 容器内，使用固定版本
  的 Pydantic 类型和 FastAPI `jsonable_encoder` 构造的 synthetic 样本。所有
  identity、地址、时间和 token 都是保留测试值，不来自数据库或真实凭据。

这些 synthetic 样本固定字段名、默认值、null、enum 拼写和事件编码，但不声称
是一次真实注册或模型部署的捕获。第一次隔离身份集成测试必须把真实响应与这些
样本比较；若有差异，先更新契约证据，再修改 Rust client。

## 文件与用途

| 文件 | 对应操作 |
|---|---|
| `live-version.response.json` | `GET /version` 成功响应 |
| `unauthenticated.response.json` | 通用未认证错误响应 |
| `worker-register.request.json` | `POST /v2/workers` 请求体 |
| `worker-register.response.json` | 注册成功的 `WorkerRegistrationPublic` 响应体 |
| `workers-me.response.json` | `GET /v2/workers?me=true` 响应体 |
| `worker-heartbeat.request.json` | `POST /v2/worker-heartbeat` 请求体；成功 body 为空、status 为 204 |
| `worker-status.request.json` | `POST /v2/worker-status` 请求体；成功 body 为空、status 为 204 |
| `models-full-sync.response.json` | `GET /v2/models?page=-1` 响应体 |
| `model-instances-full-sync.response.json` | `GET /v2/model-instances?page=-1` 响应体 |
| `model-instance-update.request.json` | `PUT /v2/model-instances/{id}` 的完整更新请求体 |
| `model-watch.ndjson` | `GET /v2/models?watch=true` 的非空事件行 |
| `model-instance-watch.ndjson` | ModelInstance CREATED 与 ID-only DELETED 事件行 |

## 再生依据

主要源码依据均位于上述固定 commit：

- `gpustack/routes/routes.py`：`/v2` prefix、认证路由和 endpoint 挂载；
- `gpustack/routes/workers.py`：注册、status、heartbeat 与 204 语义；
- `gpustack/routes/models.py`、`gpustack/routes/model_instances.py`：list、watch 与更新；
- `gpustack/schemas/workers.py`、`gpustack/schemas/models.py`：schema、enum 与 null；
- `gpustack/client/worker_manager_clients.py` 与 `generated_*_client.py`：Python
  Worker 的实际请求序列化方式；
- `gpustack/mixins/active_record.py`、`gpustack/server/coordinator/base.py`：分页、
  replay、心跳与事件编码。

部署的 `/openapi.json` 在捕获时为 `546580` 字节，SHA-256 为
`acb247255add4a162bb4d886a1bd659b38d265f8f03c827e16076cd417f3314a`。
OpenAPI 不包含 `worker-status` 和 `worker-heartbeat`，且因为 `/v2/workers` POST
被多个 router 挂载，注册成功响应在 OpenAPI 中显示为空 schema。上述三个操作必须
以 route 实现和 Python client 类型为准，不能从 OpenAPI 自动推断。
