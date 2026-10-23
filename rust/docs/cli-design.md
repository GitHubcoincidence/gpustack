# Rust GPUStack CLI-first 设计

## 契约

一个版本支持的每项面向用户或运维人员的能力，都必须有稳定的非交互式 CLI 命令。对于 Managed 操作，CLI 调用类型化 Server API 或推理 API，不在客户端重复实现 Server 业务逻辑。

开发阶段二进制为 `gpustack-rs`，最终公开二进制为 `gpustack`。临时名称用于避免与验证主机上的 Python 命令冲突。

## R1 命令树

```text
gpustack-rs
  context add|use|list|remove
  worker run
  worker doctor
  worker validate-config
  worker gpu list
  worker runtime info
  worker config get
  worker maintenance enable|disable <worker>
  worker lab deploy|list|status|logs|infer|stop|remove|cleanup
  model deploy|list|get|status|logs|delete
  inference models|chat|completion
  version
```

最终命令树在 `gpustack` 下保留这些子命令，并随着后续 Server 模块的重写继续扩展。

## M3-D control-plane-only 启动

M3-D.1 为 `worker run` 增加显式安全模式：

```text
gpustack-rs worker run --config <path> --control-plane-only
```

该命令只启动最小 `/healthz`、Worker 注册或身份恢复、heartbeat/status，以及 Model、ModelInstance、ModelFile 的 list/watch/full-sync。它注册为 maintenance、上报空 GPU，并且不初始化 NVML、Docker、vLLM、文件准备或模型 reconciler。`control-plane-only` 必须显示在结构化状态和启动日志中，不能静默退化成普通 Managed Worker。

注册凭据仍只能来自权限受限的 token 文件，不能作为命令行值传入。进程收到 `SIGINT` 或 `SIGTERM` 后停止后台循环和 health listener，保留 Server identity 与本地状态供下一次启动核验。M3-D 不提供自助删除 Server Worker 的 CLI；精确身份删除属于单独的破坏性操作门禁。

## M4 resource-api-only 启动与资源 CLI

M4 为 `worker run` 增加第二个显式受限配置：

```text
gpustack-rs worker run --config <path> --resource-api-only
```

该命令复用持久化 per-Worker 身份，启用真实系统/NVML 资源盘点、`20150` 上的 M4 Worker API 和 `20151` 上的 metrics；仍为 maintenance，且不初始化 Docker、ModelFile manager、ModelInstance reconciler 或 vLLM。`--control-plane-only` 与 `--resource-api-only` 互斥，二者都不能静默退化成普通 Managed Worker。

M4 同时实现以下只读 CLI：

```text
gpustack-rs worker gpu list --output table|json
gpustack-rs worker config get --output table|json
```

`worker gpu list` 直接调用与 status 上报相同的 resource collector，稳定 JSON 至少包含 index、UUID、名称、vendor/type、driver/CUDA/compute capability、core utilization、显存和温度；采集失败返回非零退出码，不伪造空列表。`worker config get` 使用与 `GET /v2/config` 相同的非敏感白名单和脱敏规则。

M4-C 只运行 `worker gpu list --output json` 与 `nvidia-smi` 做宿主机只读对比，不读取 Worker token、不连接 Server、不绑定正式 API 端口。启动 `--resource-api-only` 会更新真实 Server status，只能在 M4-D 的独立维护窗口授权后执行。

## Managed 模型命令

M6-A 先实现 `context add/use/list/remove`、`model deploy/list/get/status/delete`、
`model file get/delete` 和 `worker maintenance enable/disable`，用于完成不依赖 Web UI
或原始请求的真实文件准备验证。`model logs` 与 `inference models/chat/completion` 在
M6-B runtime 和推理端点可用后实现；阶段前移不改变最终 R1 命令树。

`model deploy` 使用运维人员身份认证，并通过 Server API 创建模型。它不直接访问 Docker 或 Worker。R1 强制显式指定目标 Worker，避免测试任务被调度到 Python Worker。

R1 部署输入包括：

- 模型名称；
- 目标 Worker 名称；
- 本地模型目录；
- 显式 Runner 镜像或配置的默认镜像；
- 一张或两张本地 GPU 的 index；
- 使用两张 GPU 时的 tensor parallel size；
- 已支持的结构化 vLLM 选项；
- 使用 `--wait` 跟踪 ModelInstance，并显示 `SCHEDULED`、`INITIALIZING`、`DOWNLOADING`、`STARTING` 等中间状态，直至进入 `RUNNING` 或 `ERROR`。

CLI 把友好的 GPU 参数转换为现有 Server selector 形式，例如 `worker-name:cuda:0`。
准确兼容行为以固定 v2.2.3 Worker 为准：服务端口由目标 Worker 从 `40000-40063` 分配，
并随 `INITIALIZING` 写回 Server；CLI 不指定或预留该端口。`worker maintenance` 由 Context
中的运维身份调用 Server，只修改精确目标 Worker，默认输出修改前后状态；Worker 进程
本身不得为获取调度任务而静默解除 maintenance。

`model file get <id>` 读取精确 ModelFile，供运维人员核对 Worker、来源、本地路径、状态、
大小和 resolved paths。`model file delete <id>` 只清理由 M6-A 支持的本地路径元数据：它
要求显式确认，删除前重新读取对象，并固定向 Server 传递 `cleanup=false`。若 ModelFile
仍被任何 ModelInstance 使用，Server 的冲突响应必须原样失败；命令不得删除宿主机模型
文件，也不得把删除失败伪装成成功。

`model logs` 调用 Server 日志 API；Server 在向 Worker 请求日志之前继续执行现有用户授权与 tenant 可见性检查。

## 推理命令

Managed `inference` 命令调用 Server 或 gateway 的 OpenAI-compatible endpoint。Lab 推理调用隔离的本地 vLLM endpoint，并在输出中明确标记为 Lab。两种形式绝不静默切换 endpoint。

流式命令保留增量输出，并在处理中断时确保认证 header 不会泄露。

## Lab 命令

Lab 模式用于在完成 Server 集成前演示真实 Worker runtime。它复用生产路径中的 workload planner、runtime、health check、logging 和 reconciler，但不进行 Server 注册，不使用 Worker credential，不创建 Server 数据库记录或 gateway route。

Lab 状态只保存在隔离的本地记录中：`CREATED -> PREPARING -> STARTING -> RUNNING`，并包含 `FAILED`、`STOPPED`。它们不是 `ModelInstanceStateEnum`，不得通过任何 Server API 上报。Lab status 的机器可读输出必须明确标记 `mode=lab`。

Lab 对 `local_path` 直接执行允许根目录校验、symlink canonicalization 和模型分片解析，不创建 ModelFile；这些解析原语必须与后续 Managed ModelFile manager 复用。Docker 容器 running 只表示进程存在，只有 vLLM 健康检查成功后 Lab 才能进入本地 `RUNNING`。

出现以下任一情况时，Lab deploy 拒绝执行：Rust Managed 数据目录锁处于活动状态、目标 GPU 已被占用、目标端口已占用或存在同名 Lab 工作负载。正式测试还要求先 drain 并停止 Python Worker；Rust 自身的锁无法证明它独占物理 GPU。

Lab cleanup 只处理标签匹配的资源，默认使用 `--dry-run`。R1 不提供强制抢占 GPU 或删除未知容器的参数。

### M5 Lab 命令精确语义

`worker lab deploy` 接收部署名称、本地模型目录、固定 digest 的 Runner image、一张
GPU、可选端口和受支持的结构化 vLLM 参数。省略端口时只从 `42000-42063` 分配；Lab
服务只绑定 `127.0.0.1`。命令必须依次完成路径、镜像、GPU、端口和所有权预检，原子
保存本地期望状态，创建并启动容器，并等待 `/v1/models` 返回期望模型后才报告
`RUNNING`。启动失败保留 owned container 和脱敏诊断，不自动删除现场。

同名且 normalized spec 相同的 deploy 是幂等操作；运行中的 workload 返回当前状态，
已停止的 workload 可以重新启动，丢失的 container 可以按同一 spec 重建。同名但
spec 不同返回冲突，要求用户先 stop/remove，不提供隐式替换。

`worker lab list` 枚举当前本地 owner 的记录，并与带完整所有权标签的 Docker 实际
状态合并；默认不采用未知容器。`worker lab status` 每次重新 inspect container 并
执行 vLLM 健康检查，稳定输出至少包含 mode、Lab ID、名称、Lab 状态、container
状态与 ID、spec hash、image digest、GPU UUID、endpoint 和最近错误。

`worker lab logs` 直接读取 owned container 的 stdout/stderr，并支持 `--tail`、
`--since` 和 `--follow`。`worker lab infer` 只调用该部署记录的 loopback OpenAI-
compatible endpoint，支持 chat 和 completion 请求；它不读取 context 的 Server
endpoint，也不自动回退到 Managed inference。

`worker lab stop` 在重新验证完整所有权 tuple 后执行 30 秒 graceful stop，保留
container、记录和日志。`worker lab remove` 只删除 STOPPED 或 FAILED 的 owned
container 及对应本地记录和 deployment cache；RUNNING 必须先 stop。交互式删除要求
确认，自动化使用 `--yes`。

`worker lab cleanup` 默认只输出 dry-run 清单。执行模式要求显式确认，并且只处理
当前 owner 的 Lab 资源；标签缺失、spec hash 冲突、Managed、Python 或未知 scope
全部拒绝修改。所有 Lab JSON 输出明确包含 `mode=lab`，诊断信息写 stderr。

## 认证

- `worker run` 使用注册凭据以及注册后获得的 Worker 系统凭据。
- `model` 和 Managed `inference` 命令使用运维人员或用户 API 凭据。
- Lab 命令不使用上述任何凭据。
- 凭据来自权限受限的 token 文件或获准的环境变量来源。普通命令行参数不得包含 token 值。
- Context 配置可以记录 Server URL 与 token 文件路径，但绝不记录 token 本身。

## 输出与自动化

所有适用命令都支持稳定的机器可读输出与可预测的进程行为：

```text
--output table|json
--quiet
--no-color
--timeout
--context
```

结果数据写入 stdout，诊断和进度写入 stderr。成功退出码为 0；配置、认证、兼容性、调度、运行时、健康与超时错误使用可区分的非零退出码。JSON 字段名作为带版本的兼容界面管理。

破坏性用户命令在交互式终端中要求确认，并为自动化提供显式 `--yes`。Dry-run 输出包含规范化请求或按所有权限定的清理计划，且不得包含敏感信息。

## CLI 对等门禁

如果某项已支持的用户或运维流程必须依赖 Web UI 或未文档化的原始请求，该功能就不能通过对应版本的验收。可以为诊断保留原始 API escape hatch，但它不能满足 CLI 对等要求。
