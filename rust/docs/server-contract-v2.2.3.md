# GPUStack Server v2.2.3 控制面契约

## 状态与边界

- 状态：`accepted`；2026-10-20 以 M3-C.1 修正注册身份语义
- 兼容版本：GPUStack `v2.2.3`
- 源码 commit：`1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`
- 目标部署：90 主机当前运行的固定 Server；R1 期间不升级
- 本阶段限制：只读检查和 synthetic schema 序列化；没有注册 Worker、发送
  heartbeat/status、更新 ModelInstance 或读取任何 token

本文件冻结 M3 Rust client 必须实现的最小 Server 契约。运维 model CRUD、日志和
OpenAI-compatible inference 属于后续 CLI/Worker 里程碑，不在本次 M3-A fixture
集合中展开。

## 证据优先级

1. 固定 commit 的 route、schema、client 与事件实现；
2. 同一版本部署的公开只读响应；
3. 同一版本容器内 Pydantic/FastAPI 序列化出的 synthetic fixture；
4. OpenAPI 只作交叉验证，不覆盖前三级证据。

部署 OpenAPI 的 SHA-256 为
`acb247255add4a162bb4d886a1bd659b38d265f8f03c827e16076cd417f3314a`。
它不公开 `worker-status` 和 `worker-heartbeat`，并且重复 router 挂载使
`POST /v2/workers` 的成功响应 schema 显示为空。注册成功响应实际为
`WorkerRegistrationPublic`，由 route 装饰器和 Python client 同时证明。

## Endpoint 矩阵

除 `/version` 外，以下路径都在 Server URL 后追加 `/v2`。Worker 系统请求使用
`Authorization: Bearer <token>`；Rust 日志、错误和测试输出绝不打印 header 或
token 值。

| 阶段 | Method 与 path | 凭据 | 请求 | 成功响应 |
|---|---|---|---|---|
| 版本检查 | `GET /version` | 无 | 无 | `200` JSON：`version`、`git_commit` |
| 注册/重注册 | `POST /v2/workers` | cluster registration token | `WorkerCreate` | `200 WorkerRegistrationPublic`，含新 Worker token、UUID 和可选配置 |
| 解析自身身份 | `GET /v2/workers?me=true` | Worker token | 无 | `200 PaginatedList<WorkerPublic>`；必须恰好一项 |
| heartbeat | `POST /v2/worker-heartbeat` | Worker token | `{}` | `204`，无 body |
| 状态上报 | `POST /v2/worker-status` | Worker token | `WorkerStatusPublic` 兼容 JSON | `204`，无 body |
| Model watch | `GET /v2/models?watch=true` | Worker token | query | 长连接事件流 |
| Model 全量同步 | `GET /v2/models?page=-1` | Worker token | query | `200 PaginatedList<ModelPublic>` |
| ModelInstance watch | `GET /v2/model-instances?watch=true` | Worker token | query | 长连接事件流 |
| ModelInstance 全量同步 | `GET /v2/model-instances?page=-1` | Worker token | query | `200 PaginatedList<ModelInstancePublic>` |
| 更新 observed state | `PUT /v2/model-instances/{id}` | Worker token | 完整 `ModelInstanceUpdate` | `200 ModelInstancePublic` |

Python Worker 注册时总是重新请求注册，并保存 Server 返回的 Worker name、UUID 和
token，再用 Worker token 建立正式 client。随后以 `me=true` 查询自身，结果不是
恰好一项就视为注册失败。Rust Worker 采用相同的 Server 权威身份语义，但使用本地
身份元数据和 token 的两文件状态机保证重启与部分写入恢复；两类文件都必须原子发布、
不可覆盖且仅 owner 可读。

注册请求的 `worker_version` 是 Server 兼容身份；R1 固定发送 `v2.2.3`。Rust
artifact 版本、Git commit 和 build target 仍由 Rust `/version` 与 CLI `version`
单独报告，不能用 `worker_version` 代替构建身份。

## 认证与授权

- Server 同时认识 Basic、Bearer、`X-API-Key` 和 session cookie；Rust Worker 只实现
  Bearer。
- registration token 只能从 token file 读取，用于注册；注册响应里的 Worker token
  持久化到独立文件，后续控制面请求只使用 Worker token。
- 注册 route 接受 platform admin 或 SYSTEM principal。正常 Worker bring-up 使用
  cluster registration token，不借用人类管理员凭据。
- Worker token 对应 SYSTEM principal。共享 model/model-instance router 对 SYSTEM
  principal 绕过组织 owner 门禁，并按 cluster scope 限制可见数据。
- Server 的 `PUT /model-instances/{id}` 不验证“该实例一定分配给当前 Worker”；Rust
  reconciler 在发起更新前必须自行验证 `worker_id` 或 subordinate assignment，绝不
  更新其他 Worker 的实例。

## 版本检查

公开只读响应已捕获为：

```json
{"git_commit":"1cfcbbf","version":"v2.2.3"}
```

Python 兼容算法除开发版 `0.0.0` 外要求版本字符串完全相等。Rust R1 更严格地把
`v2.2.3` 和固定 commit 作为协议基线：

- `version` 不等于 `v2.2.3`：拒绝进入 Managed ready；
- `git_commit` 缺失：记录可执行警告并拒绝正式验收；
- `git_commit` 与 `1cfcbbf` 不一致：拒绝正式验收；
- `/version` 404 只作为旧 Server 兼容现象记录，R1 固定环境中视为门禁失败。

## Schema 与 null 规则

### WorkerCreate

OpenAPI required 字段为：`name`、`hostname`、`ip`、`ifname`、`port`、`status`、
`worker_uuid`。Python client 的 `model_dump_json()` 会同时发送 optional/default
字段，因此 Rust 请求 serializer 也固定输出 fixture 中的完整稳定形状。

关键 enum/默认值：

- `proxy_mode`: `worker | direct | delegated | tunnel`，默认 `worker`；
- `labels`: 默认 `{}`；
- `cluster_id`: 可为 null；使用 cluster registration token 时由 Server principal
  推导，隔离 M3 身份应省略或发送 null；
- `status.gpu_devices`: M3 可以为空数组；真实 A100 资源状态在 M4 冻结。

### WorkerRegistrationPublic 与 WorkerPublic

注册响应在 `WorkerPublic` 基础上增加 required `token`、required `worker_uuid` 和
optional `worker_config`。常规 `GET /workers` 明确排除 `worker_uuid` 和
`machine_id`，也绝不返回 token。Rust response deserializer 必须允许新增未知字段，
并把所有 optional 字段的缺失与显式 null 都映射为 `None`。

固定 v2.2.3 的新 Worker 注册路由不会保留首次请求中的 `worker_uuid`：路由先排除该
字段并以空值创建 Worker，再由 Server 生成唯一 UUID。注册响应中的 UUID 因而是权威
值，不能要求它等于请求 UUID。`me` 响应又不包含 UUID，所以 Rust Worker 必须把注册
响应的 `id`、`name` 和 `worker_uuid` 持久化到私有身份元数据；正常重启用 `me` 核验
ID/name，并从元数据恢复 UUID。若身份元数据已落盘而 token 尚未落盘，则以保存的
Server UUID 重注册同一 Worker；token 存在而身份元数据缺失时必须拒绝启动。

### Worker status

Python client 发送 `WorkerStatusPublic`，其中比 Server input `WorkerStatusStored` 多出
optional `gateway_endpoint`。当前 Server 允许并忽略这一额外字段。Rust R1 不依赖
该宽松行为，可省略 `gateway_endpoint`；fixture 保留它以记录 Python wire shape。

### Model 与 ModelInstance

- `source`: `huggingface | model_scope | local_path`；R1 只接受 `local_path`；
- `backend` 对 vLLM 的大小写为准确字符串 `vLLM`；
- ModelInstance state：`initializing | pending | starting | running | scheduled |
  error | downloading | analyzing | unreachable`；
- 数组字段可能为空数组或 null，optional 标量可能缺失或 null；
- Rust response 类型必须容忍 R1 不使用的字段，如 LoRA、draft model、分布式和扩展
  KV cache，但 R1 reconciler 不得据此启动范围外能力。

`PUT /v2/model-instances/{id}` 是完整 `ModelInstanceUpdate`，不是 JSON Merge Patch。
至少 `source`、`name` 和 `model_name` required；Python Worker 通常从完整 public
对象构造 update 后修改 state/runtime 字段。Rust client 必须保留未知于业务逻辑但
属于已知 schema 的字段，防止全量更新把 Server 期望状态清空。

## 分页与权威 full sync

普通 list 默认 `page=1&perPage=100`。`page=-1` 时 Server 不加 SQL offset/limit，
返回全部可见条目；pagination metadata 仍保留 `page=-1`、`perPage=100`，且
`totalPage` 仍按 `ceil(total/100)` 计算。Rust 不得把 `perPage=100` 解读为 items
被截断。

ModelInstance full sync 至少带 `page=-1`，并在本地只保留分配给自身 Worker identity
的主实例或 subordinate assignment。每次 watch 建连/重连后都执行一次权威 full
sync；周期性 full sync 继续作为兜底。只有成功解析的完整响应可以驱动清理。

## Watch framing 与事件

Server 响应 `Content-Type: text/event-stream`，但数据不是带 `data:` 前缀的标准 SSE
frame，而是 JSON 后跟两个换行。client 应按行处理：忽略空行，每个非空行解析成
一个 JSON event。

事件 shape：

```json
{"type":1,"data":{"id":301},"changed_fields":{},"id":301}
```

`type` 是数字：`1=CREATED`、`2=UPDATED`、`3=DELETED`、`4=UNKNOWN`、
`5=HEARTBEAT`。实际流的 keepalive 每 15 秒发送空白行而不是 type 5 JSON。

建立 watch 时 Server 先把现存记录作为 `CREATED` 回放；这批回放与后续事件一起
构成连接时刻的初始快照。跨 Server 实例的 delete 可能只有 `{"id": ...}`，因此
`DELETED` 必须支持 ID-only data。CREATED/UPDATED 的 ID-only 事件由 Server 丢弃。

读超时必须大于 15 秒，并允许调度抖动；Python client 使用 45 秒。断流、超时或
解析失败都使 cache 失去权威性，清空 watch cache、执行带 backoff 的重连和新的
full sync，不能把空 cache 当作删除全部的依据。

## 错误契约

通用错误 JSON 为：

```json
{"code":401,"reason":"Unauthorized","message":"Invalid authentication credentials"}
```

已定义状态包括 400、401、403、404、409、422、500、503、504。Rust client 保留
HTTP status、`code`、`reason` 和已脱敏 `message`；响应不是该 shape 时使用稳定的
`UnknownResponse` 类别并限制 body 长度。认证错误不得记录所用 token。

## M3-B 实现门禁

在本文件与 fixture 经共同审阅接受后，M3-B 才能开始：

1. 在 `gpustack-client` 定义 tolerant response 类型、严格 request 类型、错误映射、
   bearer 注入与 token redaction；
2. 用本目录全部 fixture 做离线反序列化、序列化和 watch framing 测试；
3. 实现 version、register、me、heartbeat、status、model/model-instance list/watch 和
   ModelInstance update；
4. 先用 mock transport 验证，不访问真实 Server；
5. 只有另行批准隔离身份后才进行真实注册写入，并将真实响应与 synthetic fixture
   对比。

fixture 审阅不授权停止 Python Worker、占用 GPU、启动 vLLM、修改现有容器或写入
真实 Server。
