# GPUStack Worker v2.2.3 资源与 HTTP 契约

## 目的与边界

本文固定 M4-B 实现所依据的 Worker 端契约。兼容目标为 GPUStack
`v2.2.3`，源码 commit
`1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`。本检查点只包含源码审计、
只读现场响应、静态 fixture 和依赖可构建性证据，不包含 Rust Worker API、
NVML、metrics 或资源上报实现。

M4 只允许显式 `--resource-api-only` 模式。它复用已经持久化的 per-Worker
身份，启动资源采集、Worker HTTP API 和 metrics，但保持 maintenance，并禁止
Docker、ModelFile/ModelInstance reconciler、vLLM 和模型状态写入。M3-D 的
`--control-plane-only` 行为不得因 M4 实现而扩大。

## 证据身份

- 固定源码：GPUStack `v2.2.3` /
  `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`。
- 现场 Server：`GET /version` 返回 `v2.2.3` / `1cfcbbf`。
- 现场 Python Worker：`10150` 上的 `/healthz`、`/readyz`、`/version` 和受保护
  endpoint 只读响应；捕获时 OpenAPI SHA-256 为
  `a02743d483120ccd52179cdfb329fbc47d05503be6f2b455b341aeb31f5aa455`。
- 现场 Python metrics：`10151/metrics` 的捕获 SHA-256 为
  `bb5f0bb0271f2969d8ceda5904ac30b64735d30a10fe692886e25767ae02f4d4`。
- M4 拓扑：Worker API `10.213.80.90:20150`，metrics
  `10.213.80.90:20151`，模型根目录
  `/home/wujiaxing2/flowscale-system-validation/models`。
- 现场模型样本：`Qwen-7B` 根目录下固定扩展名权重共 8 个、
  `15442677288` 字节；该值只作为环境基线，不写死到实现。

固定源码文件的 Git blob identity：

| 文件 | blob |
|---|---|
| `gpustack/worker/worker.py` | `8ac22bc31c16b511385e57b508128f52142bb0bc` |
| `gpustack/routes/probes.py` | `219bafb5ec98e99eda8bfa37ee6823d64914e150` |
| `gpustack/routes/config.py` | `3768f365dd79d10f4e01076f779241ee2527f480` |
| `gpustack/routes/worker/filesystem.py` | `af3dd3374f3b5fd89f3c49a9d07a1c399da95933` |
| `gpustack/routes/worker/logs.py` | `1153fe5fe47b77f75e7328ad0afa71f81597000d` |
| `gpustack/api/auth.py` | `126f3fc70ad71934abda25c49295c212e1f11fc7` |
| `gpustack/api/exceptions.py` | `253657d513f548d37ea1af06267928679153f07a` |
| `gpustack/schemas/filesystem.py` | `805b627c681cb013c8ecbf90b8fdaee541958f0f` |
| `gpustack/schemas/models.py` | `c6229b086c2cfe5a40c4204f3bef482a3b83670e` |
| `gpustack/schemas/workers.py` | `6d76ed12971f924ea5c360818a8bdf348c9a53ac` |
| `gpustack/utils/config.py` | `eb420ad13bfc0903a011c34f811e251b896c2cf5` |
| `gpustack/worker/logs.py` | `3d6c619877b29596484273f91bfd618c7a9a21ab` |
| `gpustack/worker/collector.py` | `f296f8365fbab2494d3de711676aca1d145e0fa0` |
| `gpustack/worker/exporter.py` | `c7b81f7d4adade9174b38f91f9dd33dbbe4cb162` |
| `gpustack/scheduler/calculator.py` | `2d2b3415ed6d4d5c73681fd62103b32f64557724` |
| `gpustack/server/worker_request.py` | `5bef3ef0e03e045a9b9bfdbcbe568c2f28c2bbb3` |

## HTTP 挂载、认证与响应

Worker 端 route prefix 和 method 固定如下。`public` 表示无需凭据；`worker`
表示必须通过下述 per-Worker token 校验。

| Method | Path | Auth | 成功响应 | M4 语义 |
|---|---|---|---|---|
| GET | `/healthz` | public | 200 JSON 字符串 `"ok"` | 只表示进程存活。 |
| GET | `/readyz` | public | 不适用 | resource-api-only 固定 503。 |
| GET | `/version` | public | 200 JSON object | 返回 Rust build 与 Server baseline identity。 |
| GET | `/token-auth` | worker | 200 | 只验证本地 per-Worker token。 |
| GET | `/files/file-exists` | worker | 200 JSON object | 安全根目录内存在性和文件类型。 |
| GET | `/files/model-config` | worker | 200 JSON value | 读取白名单配置文件，最多 16 MiB。 |
| GET | `/files/model-weight-size` | worker | 200 `{"size": <u64>}` | 固定扩展名的权重字节数。 |
| GET | `/serveLogOptions/{id}` | worker | 200 JSON object | 列出受控 serve log restart。 |
| GET | `/serveLogs/{id}` | worker | 200 byte stream | `application/octet-stream`。 |
| GET | `/v2/config` | worker | 200 JSON object | 只读非敏感白名单。 |
| GET | `/metrics` | public | 200 Prometheus text | 只在 `20151` 端口挂载。 |

认证兼容 `Authorization: Bearer <token>` 和 `X-API-Key: <token>`，两者同时出现时
Bearer 优先。合法值只有 `<data_dir>/managed/worker-token` 中的 per-Worker token；
Cluster registration token 和 Server fallback 都不接受。比较使用 constant-time
算法，失败统一返回：

```json
{"code":401,"reason":"Unauthorized","message":"Invalid authentication credentials"}
```

`X-Higress-Llm-Model` 可以存在但不得触发 Python Worker 的 Server `/token-auth`
fallback。任何日志、错误、fixture 和 metrics 都不得输出 token、header 或 secret
文件内容。

## 错误契约

所有 JSON 错误均使用 `ErrorResponse`：

```json
{"code":400,"reason":"BadRequest","message":"Invalid request"}
```

M4-B 必须固定下列分类和通用消息；错误不得回显用户提供的完整路径或凭据：

| 场景 | HTTP | reason | message |
|---|---:|---|---|
| 缺少或无效 Worker token | 401 | `Unauthorized` | `Invalid authentication credentials` |
| 相对路径、无效 ID/名称或格式错误 | 400 | `BadRequest` | `Invalid request` |
| 根目录外路径或 symlink 越界 | 403 | `Forbidden` | `Access denied` |
| 安全根目录内目标不存在 | 404 | `NotFound` | `Resource not found` |
| model config 超过 16 MiB | 413 | `PayloadTooLarge` | `Model config file is too large` |
| query/body 校验失败 | 422 | `Invalid` | `Request validation failed` |
| resource-api-only readiness | 503 | `ServiceUnavailable` | `Worker runtime is not ready in resource-api-only mode` |
| 容器日志请求 | 501 | `NotSupported` | `Container logs are unavailable in resource-api-only mode` |
| `PUT /v2/config` | 405 | `MethodNotAllowed` | `Method not allowed` |

`file-exists` 是例外：安全根目录内尚不存在的路径返回 200，并将 `exists`、
`is_file` 和 `is_dir` 全部设为 false。未实现的 `/files/parse-gguf`、benchmark、
cache、debug、proxy 和 cluster proxy route 返回稳定 404；不提供“成功但无效果”的
stub。

## 文件 API

所有 path query 必须是绝对路径。启动时先 canonicalize 每个配置的模型根目录；
对已存在目标 canonicalize 完整目标，对未存在的 `file-exists` 目标 canonicalize
最近的已存在祖先，再把剩余 lexical component 安全拼接。任何现存 symlink 祖先、
最终目标或拼接结果落到允许根目录外都返回 403。不能依赖字符串 prefix 判断，也不能
跟随越界 symlink 后再读取。

`model-config` 只允许下列 basename，并要求目标是普通文件：

```text
config.json
model_index.json
tokenizer.json
tokenizer_config.json
special_tokens_map.json
generation_config.json
adapter_config.json
preprocessor_config.json
```

读取上限固定为 `16777216` 字节。返回值保持 JSON 原始类型，不强制必须是 object。
非法 JSON 返回 400，权限拒绝返回 403，I/O 内部错误返回 500，错误消息不得包含文件
内容。

`model-weight-size` 只接受目录，扩展名区分大小写并固定为 `.safetensors`、`.bin`、
`.pt`、`.pth`。普通 LLM 只统计模型根目录的直接子文件，不递归。若根目录存在
`model_index.json`，则按固定 Python 算法读取其中不以下划线开头的顶层 key，把每个
key 视作一个直接组件目录，并只统计这些组件目录的直接子文件；无关目录和更深层级
不计入。总和使用 `u64` 并检查溢出。

## 日志 API

日志只允许位于 `<log_dir>/serve`，实例 ID 为正整数。文件名固定为：

- 主日志：`{id}.{restart_count}.log`；
- 默认容器日志：`{id}.container.{restart_count}.log`；
- sidecar 日志：`{id}.container.{name}.{restart_count}.log`。

`serveLogOptions` 按 restart count 降序；最高一项 `previous=false`，其余历史项均为
`previous=true`。每项为
`{"previous": bool, "started_at": RFC3339|null, "containers": [string]}`。
存在默认容器日志时 containers 含 `default`，sidecar 名称排序且去重。

`serveLogs` 查询参数保持固定 Python 签名：

| 参数 | 类型 | 默认值 |
|---|---|---|
| `model_instance_name` | string | `""` |
| `model_file_id` | optional integer | null |
| `container_name` | optional string | null |
| `tail` | integer | `-1` |
| `follow` | boolean | false |
| `previous` | boolean | false |

`previous=false` 解析到最高 restart count；`previous=true` 且至少有两组日志时解析到
第二高，否则仍用当前最高。`tail > 0` 只返回目标组最近日志文件的最后 N 行；其他值
按顺序流式返回全部。`follow=true` 只 follow 最后一个文件。M4 没有 Docker，任何非空
`container_name` 均返回上述 501；不得读取 Python Worker 容器日志。没有可读主日志时
返回 404。

## 配置 API

`GET /v2/config` 只返回下列键；值来自 M4 的有效配置或 null，不能返回 token、
Docker socket、凭据内容或任意环境变量：

```text
debug
system_default_container_registry
server_external_url
grafana_url
disable_builtin_observability
grafana_worker_dashboard_uid
grafana_model_dashboard_uid
```

`PUT /v2/config` 固定返回 405，动态配置写入延后。

## 资源上报 schema

M4 把当前通用 JSON 字段改成固定 v2.2.3 对应的类型。所有 byte 字段使用 bytes，
utilization rate 使用 `0..=100` 百分比而不是 `0..=1` 比例。

- CPU：`total: int`、`utilization_rate: float|null`；总数采用逻辑 CPU 数。
- Memory：`total`、`used`、`utilization_rate`、`is_unified_memory=false`；
  `allocated` 保持 null，由 Server 计算。
- Swap：`total`、`used`、`utilization_rate`。
- Filesystem：`name`、`mount_point`、`mount_from`、`total`、`used`、`free`、
  `available`；至少包含承载配置模型根目录的 mount。
- OS：`name`、`version`；kernel：`name`、`release`、`version`、`architecture`。
- Uptime：`uptime` 秒数和 `boot_time` 字符串。
- GPU：`vendor`、`type`、`index`、`device_index`、`device_chip_index`、
  `arch_family`、`name`、`uuid`、`driver_version`、`runtime_version`、
  `compute_capability`、`core`、`memory`、`temperature`、`network`。

90 主机期望为两张 `NVIDIA A100 80GB PCIe`，UUID 分别是
`GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` 和
`GPU-b1f89e65-e9c4-26f5-a306-8a520df07708`，单卡总显存
`85899345920` bytes，compute capability `8.0`，core total `6912`，driver
`580.178.04`，CUDA compatibility/runtime baseline `13.0`。这些字段在 M4-C 必须与
`nvidia-smi` 交叉验证；利用率、温度、used memory 和 uptime 是瞬时值，fixture
只固定类型和范围。

status 顶层必须继续携带持久化 `worker_uuid`、`machine_id`、`port=20150`、
`metrics_port=20151`，并保持现有 hostname/IP/ifname。NVML 初始化失败、枚举少于两张
预期 GPU 或关键 GPU identity 缺失时 resource-api-only fail closed，不发送伪造的空
GPU 成功状态。

## Prometheus metrics

为保持现有 dashboard 兼容，资源指标名称和 label 对齐固定 Python Worker：

```text
gpustack:worker_node_os_info
gpustack:worker_node_kernel_info
gpustack:worker_node_uptime_seconds
gpustack:worker_node_cpu_cores
gpustack:worker_node_cpu_utilization_rate
gpustack:worker_node_memory_total_bytes
gpustack:worker_node_memory_used_bytes
gpustack:worker_node_memory_utilization_rate
worker_node_gpu_info
gpustack:worker_node_gpu_cores
gpustack:worker_node_gpu_utilization_rate
gpustack:worker_node_gpu_temperature_celsius
gpustack:worker_node_gram_total_bytes
gpustack:worker_node_gram_allocated_bytes
gpustack:worker_node_gram_used_bytes
gpustack:worker_node_gram_utilization_rate
gpustack:worker_node_filesystem_total_bytes
gpustack:worker_node_filesystem_used_bytes
gpustack:worker_node_filesystem_utilization_rate
```

所有资源指标带 `instance`、`worker_id` 和 `worker_name`，其中 `instance` 是 Worker
IP，不含 metrics 端口；GPU 额外带
`gpu_chip_index`、`gpu_index`、`gpu_name`，filesystem 带 `mountpoint`，OS/kernel
带各自 identity label。`rate` 的值继续使用百分比。为证明 Rust 控制循环自身状态，
M4 另固定以下指标：

```text
gpustack:rust_worker_mode_info{mode="resource_api_only"}
gpustack:rust_worker_ready
gpustack:rust_worker_server_connected
gpustack:rust_worker_heartbeat_success
gpustack:rust_worker_status_success
gpustack:rust_worker_watch_connected{resource="models|model_instances|model_files"}
gpustack:rust_worker_watch_reconnects_total{resource="models|model_instances|model_files"}
```

resource-api-only 的 `gpustack:rust_worker_ready` 固定为 0。M4 不输出 Docker、
workload、vLLM 或模型数量指标。

## 依赖审计与锁定输入

M4-A 在 90 主机用独立 scratch crate 审计了下列准确版本；这些版本支持固定
Rust 1.99.1，并已在网络断开后通过 `cargo check --locked --offline`：

| 依赖 | 版本 | features | 声明 MSRV |
|---|---:|---|---|
| `axum` | `0.8.9` | `http1,json,query,tokio` | 1.80 |
| `nvml-wrapper` | `0.13.0` | 无 default feature | 1.60 |
| `sysinfo` | `0.39.6` | `disk,system` | 1.95 |
| `prometheus-client` | `0.25.1` | 无 default feature | 未声明；1.99.1 实编译通过 |
| `subtle` | `2.6.1` | 无 default feature | 1.41 |

scratch `Cargo.lock` SHA-256 为
`957d7f5b36b82a086c327c26f8ace641201578fd1f1fed7fa12839c8c9715d67`，
解析 89 个 package。它只是依赖可用性证据，不替代 workspace lock；M4-B 必须把依赖
写入实际 crate、更新 workspace `Cargo.lock`，再重复全部 locked/offline 门禁。
M4-A 前后 workspace `Cargo.lock` SHA-256 保持
`b6c0de0cbc158818868a749558fe9ca1aa4ca633c2eb3ef367929fe9d34b0eae`。

`sysinfo` CPU 利用率实现必须尊重两次采样与最小刷新间隔；`nvml-wrapper` 只初始化
一次并共享句柄，不能在每次 status/metrics scrape 重复加载 NVML。

## Fixture 与 M4-B 门禁

Worker HTTP fixture 位于 `rust/fixtures/worker-v2.2.3/`；类型化资源上报样本位于
`rust/fixtures/server-v2.2.3/worker-status-resource.request.json`。前者混合现场只读
捕获和明确标注的 synthetic 预期，后者固定 Server request schema。

M4-B 必须用 mock/fake 驱动覆盖：两种认证 header 和优先级、constant-time 比较调用
边界、通用 401、全部错误分类、相对路径、`..`、根目录外路径、最终 symlink 与祖先
symlink 越界、未存在安全路径、16 MiB 边界、配置白名单、LLM/diffusion 权重规则、
日志 restart/tail/follow/previous、容器日志 501、配置脱敏、metrics 文本和类型化双 GPU
序列化。自动化测试不得访问真实网络、Docker、数据库或 GPU。

M4-A 状态只能为 `ready_for_review`。只有用户接受本契约和依赖审计后，才允许进入
M4-B；M4-C、M4-D 与任何服务/GPU 操作仍未授权。
