# Rust Worker：实现与验证计划

## 里程碑

| 里程碑 | 范围 | 先决条件 | 产出 | 验收证据 |
|---|---|---|---|---|
| M0 冻结设计 | Charter、架构、CLI、协议、计划、决策与交接记录 | 用户接受设计 | `codex/rust-worker` 上的本套文档 | 审阅通过，并以只包含目标文档的 Git commit 保存 |
| M1 环境基线 | 只读盘点 90 服务器、现有 Server/Worker、GPU、Docker、网络、模型、Runner 镜像与 Git 连通性 | M0 已提交 | 已验证的基线报告和完整环境身份信息 | 命令、版本、镜像摘要、源码映射、网络探测，且无部署变更 |
| M2 Rust workspace 与 CLI 骨架 | 四 crate workspace、配置、context、输出、错误分类、signal 与本地测试 | 已有 M1 基线和工具链 | 支持 `version`、`validate-config` 与 `doctor` 的 `gpustack-rs` | format、lint、unit test、Linux build、secret redaction test |
| M3-A–C.1 Server 控制面基础 | 版本检查、类型化 client、注册身份持久化、heartbeat/status、Model/ModelInstance watch 与 full sync | 已冻结 Server fixture | 已接受的离线控制循环与 Server 权威 Worker 身份 | fixture、mock transport、恢复和重连测试 |
| M3-C.2 ModelFile 控制协议 | ModelFile schema/fixture、按 Worker list/watch、按 ID 刷新、完整 update、desired state 与 full-sync 恢复 | M3-C.1 与固定 v2.2.3 源码复核 | 不启动模型的完整 Managed 控制资源缓存 | 所有权过滤、字段保留、断线恢复、状态值和 ID-only delete 离线测试 |
| M3-D 隔离真实 Server 集成 | 隔离身份注册、身份落盘、heartbeat/status、Model/ModelInstance/ModelFile list/watch/full-sync | M3-C.2 接受且真实写路径另行批准 | Rust Worker 在 Server 可见且稳定，但不可调度、不启动模型 | 真实响应与 fixture 对比、运行时长、Server 可见性、清理与回滚证据 |
| M4-A 资源与 Worker API 契约 | 冻结 schema、endpoint、认证、错误、fixture、依赖与 MSRV/cache 证据 | M3-D | 可离线实现的固定 v2.2.3 契约 | Python 源码/响应 oracle、静态 fixture、依赖审计；无真实写入 |
| M4-B 离线实现 | NVML/系统抽象、Worker HTTP API、文件/日志/配置读取、metrics、CLI | M4-A | 可用 mock/fake 验证的 resource-api-only 实现 | format、Clippy、locked/offline test/build、契约与安全测试、详细实现报告 |
| M4-C 宿主机只读 GPU 冒烟 | CLI 读取真实 NVML 并与 `nvidia-smi` 交叉验证 | M4-B | 不注册 Server 的两张 A100 本地盘点证据 | index、UUID、名称、显存逐项一致；Python Worker 保持运行 |
| M4-D 维护窗口正式集成 | 停止 Python Worker 后，以保留身份运行 resource-api-only，验证 Server 资源状态和完整 Worker API | M4-C 与单独维护窗口授权 | Server 可见两张 A100、metrics 及 API，且仍不可调度、不启动模型 | Server 行、认证/API/路径/日志/metrics、无 Docker/vLLM/GPU 任务、Python Worker 回滚证据 |
| M5-A～E Docker runtime 与 Lab | Docker 原语、所有权标签、WorkloadSpec、vLLM builder、本地 Lab 状态、Lab 命令、日志、健康与清理 | M4；真实运行另需对应授权 | 完全通过 CLI 完成单 GPU Lab 推理，不创建 Server ModelInstance/ModelFile | unit plan、Docker smoke、vLLM response、log stream、清理与非所有权测试 |
| M6-A Managed 文件准备 | Worker 端口分配、`SCHEDULED -> INITIALIZING`、ModelFile 本地路径校验/分片解析、完整结果上报、Context/最小 Model/maintenance CLI，并等待 Server `STARTING` | M5 与 M3-C.2 | Managed 实例按原状态所有权完成本地模型准备，不提前启动容器，真实验证不依赖 Web UI | 状态所有权、端口、ModelFile ready/error、路径安全、CLI 对等和“STARTING 时无容器”证据 |
| M6-B Managed 单 GPU runtime | `STARTING` 后 reconciler 启动/接管 vLLM，健康后上报 `RUNNING`，并补齐 `model logs`、Managed 推理和删除后的 runtime 清理 | M6-A | 完全通过 CLI 完成 Server 托管单 GPU 推理 | 完整命令记录、全状态链、响应、日志、健康门禁和删除后无孤儿资源 |
| M7 恢复与双 GPU 路径 | 接管、事件/full-sync 恢复、重启策略、Server/Docker 故障、双 A100 tensor parallelism | M6-B | 宿主机进程形式的 Worker 行为可靠 | 重启、断连、故障、未知容器与双 GPU 证据 |
| M8 Worker 容器与 R1 candidate | Worker 镜像、socket/NVML/network/path 契约、持久化状态、容器重启恢复与回滚 | M7 | 容器化 Rust Worker release candidate | 重跑单/双 GPU 套件、镜像摘要、回滚到 Python Worker、最终审计 |

## M1 门禁

M1 从只读检查开始，必须确认：

- 现有 Server 与 Python Worker 的部署机制、镜像、端口、network、volume、数据目录、名称与当前工作负载；
- 已部署 Server 的版本、镜像摘要与源码 commit 映射；
- OS、架构、driver、Docker、NVIDIA Container Toolkit、CUDA 可见性与两张 A100 的 UUID；
- 模型目录以及适合现有 vLLM 的验证模型；
- Server 容器到候选 direct inference port 的连通性；
- 可用 Runner 镜像 reference 与 digest；
- GitHub HTTPS/SSH 连通性和账号级 Git SSH 认证；
- 安全的维护、drain、测试与回滚流程。

只读盘点期间，不修改任何现有容器、服务、配置、端口分配或模型。如果不确定项会改变部署拓扑，项目返回设计讨论，不直接进入实现。

## M3-D 隔离真实控制面门禁

M3-D 使用固定隔离身份 `rust-m3d-r760xa`，在 `10.213.80.90:20150` 提供最小 `GET /healthz`。数据目录固定为 `/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d`，注册令牌只允许从 `/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token` 读取。Server 返回的 Worker ID、名称和 UUID 仍按 D035 作为权威身份持久化。

固定 v2.2.3 的 registration token 是现有默认 Cluster 的共享 bootstrap secret，不是预签发的 per-Worker token。M3-D 为它使用独立的 `0600` 文件，但只保留到首次运行确认 `worker-identity.json` 和 `worker-token` 完整落盘。之后删除该本地副本，第二次运行必须只用 per-Worker token 恢复；首次注册或 metadata-only 恢复缺少 Cluster token 时必须 fail closed。

`gpustack-rs worker run --control-plane-only` 必须以类型化 maintenance 配置注册 Worker，且满足以下硬约束：

- `maintenance.enabled=true`，维护信息说明这是 Rust M3-D control-plane validation；
- `gpu_devices=[]`，不提供 metrics 端口；
- 不初始化 GPU/NVML、Docker client、vLLM、ModelFile manager 或 ModelInstance reconciler；
- 不发送 ModelFile 或 ModelInstance 状态更新；
- 只运行版本检查、注册/身份恢复、heartbeat/status、Model/ModelInstance/ModelFile list/watch/full-sync 和 liveness；
- 日志、CLI 输出、测试证据和 Git diff 均不得出现注册 token、Worker token 或 Authorization header。

M3-D 分成三个独立门禁：

1. **M3-D.1 离线实现**：实现 CLI run 路径、最小 health listener、类型化 maintenance、安全关闭和可观测摘要；使用 fixture、mock transport 和临时目录验证，不读取真实 token，不向真实 Server 写入。完成后直接在聊天中交付详细实现报告并等待审阅。
2. **M3-D.1.1 凭据生命周期加固**：registration token 对 supervisor 可选；完整持久身份无需 Cluster token，首次注册和 metadata-only 恢复仍严格要求。只进行离线实现、测试和报告，不读取真实 token 或运行真实 Worker。
3. **M3-D.2 真实集成**：只有另获用户明确批准后才准备凭据并注册。首次运行至少 6 分钟，覆盖多轮 heartbeat/status、watch 保活和一次周期 full sync；确认身份完整后删除 Cluster token 本地副本，再用相同数据目录运行约 2 分钟，验证只靠 Worker token 恢复同一身份且不产生重复 Worker。

真实集成通过条件包括：Server 只有一个隔离身份、状态始终为 maintenance、GPU 列表为空、Server 能访问 Rust `/healthz`、心跳持续推进、三资源同步稳定、重启复用同一 ID/UUID、没有 ModelInstance/ModelFile 写入、没有新增 Docker/vLLM/GPU 进程，且 Python Server/Worker 全程健康。停止后 Rust 进程和端口必须消失。正常验收保留 maintenance identity 与本地状态供 M4 复用；精确删除 Server Worker 记录属于另一个破坏性门禁。

M3-D.2 按 D049–D052 分段执行。用户是唯一命令执行者，助手只提供当前已解锁的一段命令、解释并审阅非敏感输出；token 值不得进入聊天。正式证据目录固定为 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m3d2-formal`，权限为 `0700`，位于 Git 工作树外，文件不得包含 token 或 Authorization header。Server 容器到 `10.213.80.90:20150/healthz` 使用一次只读 `docker exec gpustack` HTTP probe 验证。

执行顺序固定为：只读预检；从干净冻结 HEAD 重新构建并记录 binary identity；隐藏输入 Cluster token；首次 370 秒运行并在约 60 秒和 330 秒观测；确认两份身份文件；无论首次注册尝试成功或失败都删除本地 Cluster token 副本；查询并保存脱敏 Server 证据；缺少 token file 的第二次 130 秒恢复运行；最终核验和保留状态。任一门禁失败都停止，不自动删除身份文件或 Server row；重试首次注册需要重新隐藏输入。

## M4 资源盘点与 Worker API 门禁

M4 新增显式 `gpustack-rs worker run --resource-api-only`。该模式复用 M3-D 保留的 Worker ID 2 和 per-Worker token，继续提交 `maintenance.enabled=true`，但启用真实 NVML/系统资源盘点、完整 M4 Worker HTTP API 和 metrics。它不初始化 Docker client、ModelFile manager、ModelInstance reconciler 或 vLLM，也不执行模型状态写入。M3-D 的 `--control-plane-only` 语义保持不变，不能因 M4 实现而隐式获得 GPU、metrics 或其他 API。

固定运行参数为：Worker API `10.213.80.90:20150`、metrics `10.213.80.90:20151`、允许模型根目录 `/home/wujiaxing2/flowscale-system-validation/models`。现有 Python Worker 继续使用 `10150/10151`。端口在每次启动前仍须检查空闲；模型根目录必须 canonicalize，任何请求解析后的路径都不得越界。

M4 分成四个独立门禁：

1. **M4-A 契约与 fixture 冻结**：从固定 v2.2.3 Python 源码和只读响应冻结 endpoint、schema、认证、错误和静态 fixture；完成依赖、离线缓存与 Rust 1.99.1 兼容性审计。不得读取真实 Worker token、向 Server 写入或启动真实 Worker。
2. **M4-B 离线实现**：实现 HTTP/auth/files/logs/config/metrics、类型化资源结构、collector 抽象以及 fake/mock NVML 和系统数据；提供 `worker gpu list` 与 `worker config get`。只允许 fixture、临时目录和 mock transport，完成全部 Rust 门禁并直接交付 D033 规定的详细实现报告。
3. **M4-C 宿主机只读冒烟**：只运行 `worker gpu list --output json`，把真实 NVML 结果与 `nvidia-smi` 的两张 A100 index、UUID、名称和显存交叉验证。不注册 Worker、不更新 Server、不绑定正式 Worker API 端口；Python Worker 保持运行。该结果只属于 diagnostic/smoke，不能满足 M4 正式验收。
4. **M4-D 维护窗口正式集成**：必须另获用户对精确窗口和 stop/start 命令的明确授权。先确认无 ModelInstance、GPU 空闲和无工作负载，再停止 Python Worker；以保留的 Worker ID 2 启动 `--resource-api-only`，验证 Server 可见两张 A100、maintenance 保持开启、`metrics_port=20151`，并验证 Server 到 Worker 的认证、文件、日志、配置、probe 和 metrics。结束时停止 Rust Worker、确认 `20150/20151` 关闭，随后恢复 Python Worker 并复核 `10150/10151` 与 Server 状态。

资源上报至少使用类型化结构覆盖：

- CPU 数量和利用率，内存与 swap，文件系统；
- OS、kernel、boot time、uptime 与 machine ID；
- GPU index、device index、chip index、UUID、名称、NVIDIA vendor 与 CUDA 类型；
- driver、CUDA runtime、compute capability、GPU core utilization；
- GPU memory total/used/utilization、`unified=false` 与 temperature；
- `metrics_port=20151`，且 Worker 继续处于 maintenance。

NVML 初始化失败时 resource-api-only 必须 fail closed，不得伪造空 GPU 或健康状态。单个瞬时可选指标可以显式表示缺失或降级，但 M4 正式验收要求两张 GPU 的身份和关键容量字段完整且与 `nvidia-smi` 一致。

M4 Worker API 范围固定为：

- 公开 `GET /healthz`、`GET /readyz`、`GET /version`；
- 只接受持久化 per-Worker token 的 `GET /token-auth`、`GET /files/file-exists`、`GET /files/model-config`、`GET /files/model-weight-size`、`GET /serveLogOptions/{id}`、`GET /serveLogs/{id}` 和 `GET /v2/config`；
- 独立 `20151` 端口上无需认证的 Prometheus `GET /metrics`；
- 明确不实现 `PUT /v2/config`、`/files/parse-gguf`、benchmark/cache 日志、debug API、inference proxy、cluster proxy 和动态配置写入。

resource-api-only 因没有 Docker 和 reconciler，`/readyz` 固定返回 503；未来完整 Managed 只有在 runtime 先决条件和首次权威收敛完成后才返回 200。该行为有意强于固定 Python v2.2.3 始终返回 200 的实现，但不影响 Server 的 `/healthz` reachability 检查。

认证只接受注册后持久化的 per-Worker token，不接受已经删除的 Cluster registration token；凭据比较必须 constant-time，401 信息保持通用，日志和证据不得包含 secret。文件接口必须拒绝相对路径、symlink 越界和允许根目录外路径；model config 使用文件名白名单和大小上限；权重统计遵循固定版本的 `.safetensors`、`.bin`、`.pt`、`.pth` 本地目录语义；日志 ID 和名称必须约束在 Worker 日志目录内；配置接口只返回非敏感白名单字段。

M4-A 只冻结依赖类别，不提前锁定未经审计的版本：HTTP router/middleware 使用 `axum`，NVML 使用动态初始化的 `nvml-wrapper`，系统信息使用 `sysinfo`，metrics 使用 `prometheus-client`，constant-time 比较使用 `subtle`。准确版本必须先证明 Cargo cache 可用、支持固定 Rust 1.99.1，并写入 `Cargo.lock`；离线构建通过前不得接受 M4-B。

M4 正式接受必须同时具备：精确 Rust/Server/Cargo.lock identity；Python/Rust fixture 对比；认证与通用 401；路径 traversal、symlink 和越界测试；配置脱敏；日志查询/流语义；Prometheus 格式；全部 Rust 自动化；M4-C 的 NVML 对比；以及 M4-D 的 Server 双 A100、maintenance、metrics、API、无 Docker/vLLM/GPU 任务和 Python Worker 恢复证据。接受本设计不授权停止 Python Worker 或执行 M4-D。

## M5 Docker runtime 与 Lab 门禁

M5 以 `WorkloadSpecV2` 为 Lab 和后续 Managed 的共享运行时契约。M5 只完成单 GPU
Lab 路径，不创建或更新 Server Model、ModelInstance、ModelFile 或 gateway route，
也不能把 Lab 结果解释为 Managed 兼容证据。

M5 分成五个独立门禁：

1. **M5-A 契约与依赖类别冻结**：冻结 WorkloadSpec、spec hash、所有权标签、Docker
   原语、Runner/vLLM 参数、GPU/端口/路径、本地状态、CLI、健康、日志和清理语义；
   对固定 Python Worker、第三方 runtime 和 Runner image 做只读 oracle 审计。只允许
   文档和只读检查，不连接 Docker socket 或执行模型。
2. **M5-B Docker runtime 离线实现**：实现版本化 spec、规范化 hash、Docker client
   抽象、image/container/log 类型、所有权验证和 vLLM plan builder。先审计准确 Rust
   Docker client 版本，再更新 lock；测试只使用 fake transport、fixture 和临时目录，
   不访问真实 Docker、网络或 GPU。
3. **M5-C Lab adapter 与 CLI 离线实现**：实现 owner/deployment 原子存储、排他锁、
   Lab 状态机、幂等 deploy、list/status/logs/infer/stop/remove/cleanup、端口分配、健康
   检查与故障诊断。所有 Docker、HTTP、clock、GPU 和 filesystem 边界可注入 fake；
   完成全部 Rust 门禁和 D033 详细实现报告。
4. **M5-D 无 GPU Docker 集成**：M5-D.0 先离线补齐 `worker runtime info`、共享 label
   校验和受显式执行门禁保护的 Rust example probe；它必须通过 fake/mock 测试和全部
   Rust 门禁，不连接 Docker。M5-D.1 另获授权后，在不停止 Python Worker 的前提下
   只读验证 daemon/version、固定 image inspect、服务/GPU 基线和无历史残留。
   M5-D.2 再获授权后使用固定缓存 Runner 创建 network-none、无 mount、无 GPU、无端口
   的有界诊断容器，验证 owned label create/inspect/log/stop/remove、错误 owner 和现有
   Python 容器拒绝、服务前中后不变以及最终零残留。任何预检失败均停止，不 pull 镜像，
   不运行 vLLM 或模型，不使用名称或 force 作为正常清理依据。
5. **M5-E 单 GPU Lab smoke**：M5-E.0 先在 Python Worker 保持运行时只读验证固定
   source/binary、固定 Qwen2.5-0.5B-Instruct identity、缓存 Runner、Server 资源、服务、GPU0、GPU1 外部
   principal、`127.0.0.1:42000` 和历史残留，并建立 Git 外受保护的运行配置与证据目录。
   M5-E.1 另获维护窗口授权后，只停止 `gpustack-worker`；使用固定 Runner digest、本地
   Qwen2.5-0.5B-Instruct、GPU0 完整 UUID、tensor parallel 1、2048 context、0.5 GPU memory、dtype
   auto 和 eager execution，并使用模型内置 chat template
   完成 deploy/list/status/logs、readiness、同 spec 幂等、异 spec 冲突、completion/chat、
   stop/remove 与 cleanup。不传旧 Qwen-7B ChatML、`--trust-remote-code` 或其他
   `--vllm-extra-arg`，也不使用 `eval`、可写模型目录或额外 mount。无论成功失败都只清理完整
   owned tuple，恢复原 Python Worker 并独立验证 Server 资源、服务、端口、
   容器、cache 和 GPU 无残留；所有权或 GPU0 残留不可归因时失败关闭并请求人工处置。

M5-B 和 M5-C 的每个实现阶段都必须分别交付详细实现报告并由用户接受，不能因为最终
计划做真实 smoke 就降低离线测试。准确 Docker client 依赖只有在固定 Rust 1.99.1、
90 Cargo cache 和 workspace locked/offline 构建全部通过后才可接受；M5-A 不预先猜测
crate 版本。

M5-D 的通过只证明 Docker 原语和所有权防护；M5-E 的通过才证明 Lab 单 GPU vLLM
smoke。M5 正式接受还要求：`/v1/models` 含期望模型后才报告 RUNNING；OpenAI-
compatible 推理返回有效内容；日志流可读；同 spec 幂等、异 spec 冲突；失败现场
保留；stop/remove/cleanup 行为符合所有权 tuple；Server 没有新增 Lab ModelInstance
或 ModelFile；Python Worker 恢复；Rust container、端口、GPU process 和 cache 清理
结果均有证据。

## M6-A Managed 文件准备门禁

M6-A 只证明 Server 托管状态链、Worker 服务端口分配和本地 ModelFile 准备，不连接
Docker、不创建 vLLM 容器、不运行推理。固定状态边界是
`SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING`；`STARTING -> RUNNING` 属于 M6-B。

M6-A 分成九个独立门禁：

1. **M6-A.0 契约冻结**：从部署 commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`
   复核 Python Worker、Server Controller、Model/ModelInstance/ModelFile 路由和 scheduler。
   冻结 Worker 端口责任、状态所有权、完整 PUT、路径/分片规则、CLI 与真实回滚边界。
2. **M6-A.1 离线 Worker 实现**：实现 `40000-40063` Managed 端口分配、ModelInstance
   初始化、ModelFile resolver 和按 ID 去重的协调器。每次写入前重新 GET 并复核归属；
   fake/mock 覆盖 ready/error、重复事件、assignment 漂移、越界 symlink、不完整分片、
   端口冲突和 `STARTING` 不调用 runtime。不得读取真实 token 或连接 Server/Docker/NVML。
3. **M6-A.2 离线 CLI 实现**：实现 Context、最小 Model CRUD/status 与精确 Worker
   maintenance 命令；凭据只引用权限受限 token 文件，稳定 table/JSON、退出码、确认与
   脱敏规则全部用 mock transport 验证。`model logs` 和 inference 命令仍不在本阶段实现。
4. **M6-A.2.1 离线生产入口接线**：实现 `worker run --managed-file-only`，与其他模式
   互斥，并复用持久 Worker identity、system/NVML、Worker API、metrics 和 control plane；
   接入 M6-A.1 协调器，启动时要求 Worker 已处于 maintenance，但不自动改变 maintenance。
   结构化事件记录实例/文件 ID、观察状态、端口、文件结果和 `STARTING` no-op；该模式
   不得构造 Docker client、WorkloadSpec、容器或 vLLM runtime。全部验证使用 fake/mock。
5. **M6-A.2.2 离线 ModelFile 清理 CLI**：实现 `model file get/delete`。删除前读取最新
   对象并限制为精确 Worker 上具有非空本地路径的 `local_path` ModelFile；危险操作要求
   确认并固定 `cleanup=false`，只删除 Server 元数据，不删除宿主机模型数据。mock transport
   覆盖非法 ID、非本地来源、Server 引用冲突和 secret 脱敏。
6. **M6-A.2.3 只读预检 CLI 补全**：实现 `worker get <worker-id-or-exact-name>` 和
   `model file list --worker <worker-id-or-exact-name>`。Worker 输出使用安全白名单，不透传
   extensions/provider config/principal；ModelFile 查询强制 Worker scope 并按 ID 稳定排序。
   数字 ID、精确名称、无结果、重名、URL 编码、空列表、稳定 table/JSON、错误映射和脱敏
   全部通过 mock/fixture 验证，不连接真实 Server。
7. **M6-A.3.P operator Context 准备**：正式预检需要独立 operator API Key。通过本地
   管理员的隐藏式登录创建唯一、7 天有效、仅 `management` scope 的阶段凭据，保存到
   mode 0600 token 文件并建立 active `lab90` Context；登录 cookie 和完整创建响应立即
   清除。该准备只允许创建这一条 API Key 和本地 Context，不授权任何 Worker/模型变化。
8. **M6-A.3.0 独立只读预检**：另获授权后核对固定源码/二进制/Server/Worker/Context、
   唯一测试 name、无同路径 ModelFile、Qwen2.5 manifest、GPU0、受管端口、Docker 与原
   Python Worker 的精确恢复材料。允许从干净冻结提交构建二进制并在 Git 外建立受保护
   证据目录；不启动 Rust Worker、不修改 Server 或 maintenance、不停止 Python Worker。
9. **M6-A.3.1 真实无 Docker smoke**：M6-A.3.0 通过并共同审阅后另获维护授权，停止
   精确 Python Worker，启动 `--managed-file-only`，由 CLI 临时解除 Rust Worker
   maintenance，并把唯一固定 Qwen2.5 模型精确调度到 Rust GPU0。部署不得使用 `--wait`。
   在 300 秒内捕获完整状态序列和 ModelFile ready 后，实例只能停在 `STARTING` 并保持
   观察 60 秒；期间 Docker client/container、inference listener、GPU compute process 和
   可归因显存增长必须为 0。随后先恢复 maintenance，再用 CLI 删除精确
   Model/ModelInstance 和 ModelFile 元数据、核对本地模型不变，停止 Rust Worker 并恢复
   原 Python Worker，复核 Server 资源集合与全部零残留门禁。完整协议见
   `m6a3-managed-file-runbook.md`。

静态 CLI 核对发现的只读 Worker/ModelFile 查询缺口已经通过 D103–D106 冻结为
M6-A.2.3。该阶段完成离线实现、D033 报告和用户接受前，M6-A.3.0 仍为 `not_ready`；D090
继续禁止用 Web UI 或未文档化 curl 绕过该门禁。

本地路径只允许固定模型根目录内的绝对路径，canonical symlink 不得越界；目录和普通
文件返回 canonical path，分片必须编号连续、声明总数一致且稳定排序，并递归计算字节数。
成功完整写回 `resolved_paths`、`size`、`download_progress=100`、`ready`；失败写
`error` 和脱敏有界消息。M6-A 不支持网络下载、LoRA、draft model、跨 Worker 或多 GPU
runtime。

M6-A.1、M6-A.2、M6-A.2.1 和 M6-A.2.2 均已在 D033 详细实现报告后获用户接受。
M6-A.2.3 的接口、源码、离线质量门禁和 D033 详细报告已经用户接受。M6-A.3.0 的前五次
尝试分别保留缺少 Context、Worker identity 漂移、无效 Context 调用、GPU0 被既有 Lab
占用和无效 Cargo working directory 的停止或失效证据。D107–D125 完成 operator Context、
当前 ID 5 identity、证据滚动与执行器修正；attempt 6 已从头通过全部只读门禁，状态为
`passed_preflight`。该结果不自动开始 M6-A.3.1。真实 Server 写入、maintenance 变化、
停止 Python Worker 和启动 `--managed-file-only` 必须在预检结果共同审阅后另行明确授权；
M6-A.3.1 也不授权 Docker workload、GPU workload 或推理。

## M6-B Managed 单 GPU runtime 门禁

M6-B 把 M6-A 已证明的 `STARTING` 终点闭合到真实 `RUNNING`、Server 路由推理、日志和
权威删除后的精确 runtime 清理。只支持固定本地 Qwen2.5-0.5B、固定 digest Runner、单
Worker/单 GPU/TP=1、host network 和 `40000-40063`。跨 Worker、多 GPU、LoRA、draft
model、Kubernetes、完整 crash recovery、restart-on-error 和日志轮转均不是本阶段结论。

M6-B 分成六个独立门禁：

1. **M6-B.0 设计冻结**：冻结 `--managed-runtime`、状态所有权、写前刷新、共享 runtime
   核心、精确容器标签、健康门、Worker 推理代理、日志/CLI、删除和恢复边界。该门禁只改
   治理文档，不执行真实 Server、Docker 或 GPU 操作。
2. **M6-B.1 共享 runtime 与 planner 离线实现**：从 Lab 提取 Docker lifecycle、ownership、
   vLLM health、诊断和日志读取原语；实现 Managed `STARTING` planner、resolved path/GPU
   UUID/固定镜像/端口和参数 allowlist。fake runtime 覆盖创建、单容器接管、重复事件、
   多容器/标签/spec 冲突、越界路径、未知参数和 assignment 漂移。不得连接真实 Docker。
3. **M6-B.2 Managed reconciler 离线实现**：把 runtime 协调接到 `--managed-runtime`，实现
   最新权威对象写回、`/v1/models` readiness 后 `RUNNING`、有界 `ERROR`、mode 0600 当前
   session 日志、辅助 manifest 和权威删除后的精确 cleanup。测试只用 fake Server/runtime/
   filesystem；前三种受限模式的行为必须保持不变。
4. **M6-B.3 推理代理与 CLI 离线实现**：在 Worker API 增加严格实例 header parser、健康
   route table、受限 OpenAI 路径、系统认证 header/hop-by-hop 清理、buffered 与 SSE 转发；
   实现动态 readiness、`model logs` 和 `inference models/chat/completion`。`model deploy`
   启用 Server model route。全部使用 mock transport，不连接真实 endpoint。
5. **M6-B.4 90 只读预检与故障注入**：从冻结干净提交构建；核对 Server/Python Worker、
   Docker daemon/固定镜像、Qwen manifest、GPU0、端口、模型/容器冲突、回滚材料和凭据
   准备边界。用 fake 或窗口前硬失败证明未进入时不会改变服务、Server、Docker 或 GPU。
   该阶段不得停止 Python Worker或创建真实 workload。
6. **M6-B.5 单 GPU 正式 smoke**：在单独明确授权的维护窗口中停止并最终恢复精确 Python
   Worker，以 `--managed-runtime` 启动 Rust Worker、显式解除 maintenance，并通过 CLI
   部署唯一模型。必须证明完整状态链、健康先于 `RUNNING`、Server `/v1/models`、chat、
   completion、Server model logs、重复事件幂等以及模型删除后 owned container、route、
   port、cache、manifest、ModelFile/Instance 元数据和 GPU0 workload 零残留。GPU1、未知
   容器和其他 Server 资源不得改变；阶段凭据验收后退役。

M6-B.1～B.3 每个实现阶段都执行完整 Rust workspace fmt、Clippy、test 和 build 门禁，并在
聊天中交付 D033 详细实现报告。M6-B.4 的只读通过不自动授权 B.5；停止 Python Worker、
切换 maintenance、创建 Server 模型、创建 Docker/GPU workload 和真实推理必须由 B.5 的
精确维护窗口授权覆盖。

## M7 恢复与双 GPU 门禁

M7 把 M6-B 已接受的宿主机进程 Managed 单 GPU 路径扩展为可恢复、可退避并支持本机双 GPU
tensor parallelism 的 Worker。它不改变 R1 的后端和运行时边界：仍然只支持本地 Docker、
vLLM、`local_path`、单 Worker 和固定 Runner，不加入 Kubernetes、其他推理后端或跨 Worker
分布式推理。Rust Worker 在 M7 继续作为宿主机进程运行；Worker 镜像、容器重启接管和最终
R1 candidate 属于 M8。

M7 的宿主机恢复和双 GPU 结果是阶段验收证据。架构要求的最终 R1 恢复与双 GPU 证据必须在
M8 使用容器化 Rust Worker 重跑，不能把 M7 的宿主机结果直接提升为发布证据。

M7 分为七个独立门禁：

1. **M7-0 设计冻结**：冻结接管判定、重启退避、日志代际、故障模型、宿主机证据角色、
   单/双 GPU 窗口、授权边界和 M8 重跑要求。该门禁只修改治理文档，不执行 Server、Docker、
   服务切换或 GPU 操作。
2. **M7-A 启动接管离线实现**：启动 full sync 后联合 Server 权威快照、Docker 实际状态和
   本地恢复记录进行协调。唯一完整 ownership tuple 且 spec hash 匹配的健康容器必须原位接管，
   恢复端口预留、route、manifest 和日志跟踪，不改变 container ID、端口或 workload；零匹配
   只在权威期望仍要求运行时创建；多匹配、标签缺失和 spec 冲突都 fail closed。Worker异常
   退出不得删除健康 Managed 容器。
3. **M7-B restart-on-error 与日志代际离线实现**：与固定 Python Worker 对齐，首次重试延迟
   0 秒，连续失败随后为 10、20、40、80 秒并以 300 秒封顶；进程内连续失败计数在 workload
   再次健康并进入 `RUNNING` 后清零。每次重启把 Server 持久 `restart_count` 加一并写
   `last_restart_time`，重新经过 `SCHEDULED -> STARTING -> RUNNING` 的权威链；
   `restart_on_error=false` 时保持 `ERROR`。日志保留当前和上一 restart generation，
   `model logs --previous` 读取上一代，永久删除时清除全部代际。
4. **M7-C 离线故障矩阵**：使用 fake Server/runtime/filesystem 验证 Worker crash 后接管、
   watch 断线与 full sync、Server 暂时不可达、Docker API 暂时不可达、vLLM 异常退出、启动
   失败、健康超时、端口冲突、退避期间权威删除、未知容器和 ownership/spec 冲突。Server 或
   watch 失败不能授权删除；Docker 短暂失败不能立刻把全部实例写为失败；未知、Python、Lab
   或不同 scope 的容器始终不修改。
5. **M7-D 90 只读预检**：从干净冻结提交构建，固定 Server、Rust binary、`Cargo.lock`、
   Runner digest、Qwen2.5-0.5B revision/path、Worker身份、端口和两张 A100 identity；验证
   Python Worker恢复材料、代理故障注入边界和阶段凭据准备条件。该门禁不得停止服务、切换
   maintenance、创建 workload 或使用 GPU。
6. **M7-E 单 GPU 恢复阶段验证**：使用 GPU0 和固定 TP=1 workload，分三个独立维护窗口证明：
   Rust Worker被精确强制终止并重启后原 vLLM 容器、端口和 spec 不变且无重复容器；精确终止
   owned vLLM 后出现 `ERROR`、持久 restart count、冻结退避、新容器、当前/上一代日志和推理
   恢复；仅中断 Rust Worker 自己的 Server/Docker 代理连接后健康 workload 被保留，并在恢复
   连接后通过 full sync/重新枚举收敛。真实 Server 和宿主机 Docker daemon 不作为故障注入
   对象，不得停止。
7. **M7-F 双 GPU TP=2 阶段验证**：继续使用固定 Qwen2.5-0.5B 和 Runner，把有序 GPU
   assignment 固定为 GPU0/GPU1、`tensor_parallel_size=2`。必须证明 Server assignment、
   WorkloadSpec、Docker DeviceRequest、两张 GPU 的可归因显存/计算活动、完整状态链、
   Server-only models/chat/completion/logs、重复协调幂等和删除后零残留。

M7-E 的三个窗口和 M7-F 必须分别获得精确授权；任一窗口都要 drain 并停止精确 Python
Worker、使用独立 watchdog、只修改完整 Rust ownership tuple，并在结束或失败后恢复同一
Python Worker。短期 operator credential 只在对应真实阶段需要时创建，用户审阅通过后删除
Key、Context 和 token file，并验证旧 token 失效。M7 的任一真实通过都不能自动授权下一
窗口，也不能自动进入 M8。

M7-A～C 每个实现阶段继续执行完整 workspace fmt、Clippy、test 和 locked/offline build，
并在请求用户验收前按 D033 在聊天中直接交付详细实现报告。M7-E/F 的接受必须同时具备业务
结果、watchdog、独立事后审计、凭据退役和用户审阅；部分状态链、仅恢复成功或仅 cleanup
成功不能替代阶段通过。

## 验证策略

### 本地自动化检查

仓库内测试使用 mock 与 fixture，不访问真实网络、Docker、数据库或 GPU。测试覆盖配置优先级、脱敏、CLI 输出与退出码、Server 协议序列化、Model/ModelInstance/ModelFile watch 重连与 full sync、状态所有权、镜像解析、GPU 映射、端口分配、WorkloadSpec 生成、所有权检查、幂等协调和故障策略。

Rust 代码只在对维护者有实际价值的位置添加英文注释。状态机、安全与所有权检查、协议兼容偏差、错误恢复、关键数据流和非直观实现必须解释目的和原因；简单 getter、字段、枚举值、CLI 参数以及代码已能清楚表达的行为不要求注释。每次阶段门禁至少执行：

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`
- `cargo test --workspace --all-targets --locked --offline`
- `cargo build --workspace --locked --offline`

注释门禁由人工审阅负责，因为编译器无法判断某段逻辑是否关键。审阅同时拒绝两类问题：关键逻辑缺少“为什么”，以及过期、逐行复述、修改历史叙述或数量过多而遮挡代码的注释。测试通过不能覆盖注释门禁失败。保留的 Rustdoc 可按需运行 `cargo doc --workspace --no-deps --locked --offline` 验证，但不要求每个公开字段都写文档。

### 阶段实现报告与验收顺序

每个实现阶段必须按“实现与验证 → 在聊天中直接交付详细实现报告 → 用户共同审阅与验收”的顺序推进。详细报告至少包含：阶段目标与范围、逐项功能、每项功能的实现方式、关键数据流与不变量、文件/模块映射、新依赖与取舍、自动化和环境证据、明确未执行的风险操作、已知限制与未实现项、当前停止位置及下一项允许工作。

长期治理文档只需保存能够恢复工作的检查点摘要和证据 identity；不得把用户需要阅读的详细实现说明仅写入文档或用文件链接代替。未直接交付详细报告时，阶段不得请求或记录为 `accepted`。

### 契约检查

从已冻结 Python Server 捕获的 fixture 覆盖 request、response、enum、null、错误响应体、认证、分页和逐行 watch event。不编辑 generated Python client。Python Worker 与 runtime 只作为兼容性 oracle，不成为 Rust runtime 依赖。

### 90 服务器集成检查

真实 Docker、NVML、Runner、vLLM 和 GPU 测试只在获批维护窗口内于 90 服务器执行。任何 Rust Lab 或 Managed 工作负载使用 GPU 前，先 drain 并停止 Python Worker；Server 保持运行。

### 正式证据身份

每个获接受的集成结果都要记录：

- Rust 与 Server commit；
- `Cargo.lock` identity；
- Worker 与 Runner 镜像名称和 digest；
- 模型路径以及稳定模型 identity 或 hash 信息；
- OS、driver、Docker、toolkit 与相关 runtime 版本；
- 准确的命令与配置 identity；
- 开始/结束时间、结果、日志和清理验证。

Lab 证据必须标记为 diagnostic 或 smoke。只有完整 Managed 流程可以满足 Managed 验收。

## R1 验收矩阵

- CLI help、稳定 JSON 输出、脱敏、确认机制与退出码。
- 注册、认证、heartbeat、状态上报与版本兼容性。
- 准确的双 A100 资源盘点和稳定 UUID/index 映射。
- Worker probe、文件系统、日志、配置、metrics 与认证 API。
- 在一张 GPU 上完成 Lab deploy、status、logs、inference、stop 与 remove。
- 在一张 GPU 上完成 Managed deploy、status、logs、inference 与 delete。
- Managed 流程完整经过 `SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING -> RUNNING`，且每次推进由约定责任方完成。
- `local_path` 通过 ModelFile ready/error 控制链完成解析，`STARTING` 前无 vLLM 容器，健康探针成功前不报告 `RUNNING`。
- Managed 双 GPU tensor parallel inference。
- Worker 进程和 Worker 容器重启后接管已有工作负载。
- Server 与 watch stream 重连后通过权威状态收敛。
- vLLM 异常退出时记录重启和 backoff 行为。
- Docker daemon 短暂故障时行为正确。
- 处理端口冲突、无效模型路径、不兼容镜像与健康检查超时。
- 删除后不遗留归属本 Worker 的孤儿工作负载。
- 不修改未知、Python 所有或不同 scope 的容器。
- Git、日志、CLI 输出与证据中不出现凭据。
- 成功回滚到保留的 Python Worker。

性能测量包括 idle memory、startup time、reconciliation latency 和 control-plane overhead。这些数据是特征描述证据，而不是 R1 中宣称性能优于 Python 的通过/失败标准。

## 风险控制

- GPU 重复计算：绝不让 Python 与 Rust Worker 同时成为相同 GPU 的可调度 owner；使用维护窗口并显式指定目标 Worker。
- 破坏现有部署：使用独立 identity、目录、端口、Compose project 与 label；保留 Python 配置和数据。
- 错误清理：要求完整所有权 label tuple 和权威期望状态；Lab cleanup 默认 dry-run。
- 协议漂移：固定已部署 Server，并基于它构建 fixture。
- 状态所有权漂移：以固定 v2.2.3 的 ModelInstance Controller、ModelFile Controller 和 Python Worker 行为为兼容 oracle；Lab 本地状态不得冒充 Managed 状态证据。
- 路径 namespace 不匹配：宿主机、Worker 与 vLLM 使用相同模型根目录，模型启动前验证 canonical mapping。
- 敏感信息泄露：使用 token file、脱敏测试、精确路径 Git staging，以及日志/证据复查。
- Git 数据丢失：把关键检查点推送至可写 fork；90 工作树不是唯一副本。
- 网络或 GitHub 故障：在实现前停止，先建立安全、可审计的同步路径，避免远程工作没有可推送副本。

## 回滚与切换

R1 全程保留 Python Worker。测试使用全新的 Rust Worker name、UUID、token、目录、端口、label 与部署定义。GPU 测试时先 drain 并停止 Python Worker，验证 GPU 与工作负载状态，运行 Rust 测试，只删除 Rust 所有的测试工作负载；若门禁失败则恢复 Python Worker。

在容器化 candidate、恢复套件与回滚通过验收前，不进行永久身份迁移；该迁移不属于 R1。

## 当前停止位置

M1 环境基线、M2 Rust workspace/CLI 骨架、M3-B client、M3-C 控制循环和 M3-C.1 Server 权威身份修正均已接受。固定 v2.2.3 源码复核发现的 M3-A 注册 fixture UUID 等值语义与 M3-C 对应身份断言保持 `invalidated`，不得重新作为兼容性证据。

2026-10-20 的状态机复核确认原设计错误地让本地模型跳过 `DOWNLOADING`，并遗漏 ModelFile 控制资源。该设计偏差尚未进入 Docker/runtime 实现；已接受修订要求 Managed 完整遵循 ModelInstance/ModelFile 状态所有权，同时保留 Lab 独立本地状态。

M3-D.1 与 D047–D048 的 M3-D.1.1 离线实现均已接受，M3-D.1.1 代码检查点为 `99e9fdb9`。M3-D.2 已使用来自干净 `e30e7bd4` 的固定二进制在真实 v2.2.3 Server 上接受：370 秒首次注册和 130 秒无 Cluster token 恢复均保持 maintenance、空 GPU、持续 heartbeat/status/full-sync/watch，并复用同一 Worker ID/UUID。正式证据位于 Git 外 `m3d2-formal/`，其中参数层级错误且未触发网络写入的首次 CLI 调用保留为 `invalidated`。

M4 设计和 M4-A～M4-D 门禁已经冻结。M4-A 契约/fixture/依赖审计与 M4-B resource-api-only 离线实现均已在详细报告后获用户接受；M4-B 代码检查点为 `994b820cba795b0bbfa25d23a8d55e2e2f470915`，workspace `Cargo.lock` SHA-256 为 `ad5fabd70e55fcbee197c3a644b8acf530ba943ca385c9344e7e6cfb06cb19c3`。

M4-C 已使用来自干净 `c76fa3903b3b` 的 CLI 真实读取 NVML，并与 `nvidia-smi` 对两张 A100 的 index、UUID、名称、显存和驱动逐项一致，经用户审阅后标记为 `accepted`。该 smoke 没有启动 Worker、连接 Server 或绑定端口，不能替代 M4-D。

M4-D.0 只读预检已在 `fb6412611c5e` 上通过：Server 与 Python Worker 健康，保留的 Worker ID 2 唯一且处于 maintenance，目标 Worker 没有 ModelInstance 或 ModelFile，两张 A100 空闲，Rust 进程和 `20150/20151` 监听均不存在，模型根目录与网络身份符合冻结配置。预检只使用 per-Worker token 发起认证 GET，未输出 token、写入 Server 或改变服务。

M4-D 尝试 1 在 `a80922abd0ed` 上真实发现 Worker HTTP `/version` 未获得 build identity，状态保持 `failed`；退出 trap 已恢复 Python Worker。修复 commit `a3283b665a63` 为 Worker crate 增加独立 build script 和回归测试。尝试 2 使用来自该干净 commit 的二进制完成 231 秒正式运行：Server 持续看到同一 maintenance Worker ID 2、两张 A100 和 `metrics_port=20151`，完整 Worker API/认证/文件/日志/配置/metrics 门禁通过，三次 heartbeat 前进，ModelInstance 与 ModelFile 保持为 0，容器身份无变化，GPU 无计算负载；Rust 优雅停止后 Python Worker 恢复并通过独立复核。

M4-D 审阅中发现的根文件系统字段语义已在 commit `ad4ebfa39234de22690f8a28ee79b273eb3acc7d` 修正：Unix 使用 `statvfs` 区分 `free` 与 `available`，并按 `used = total - free` 上报。M4-D.2 从干净 `f6faa63e9c03` 构建并在线复核，Server、Rust metrics 与宿主机同点采样一致；Python Worker 全程运行，测试后无 Rust 进程、端口、容器或 GPU 负载残留。用户已审阅详细报告并接受 M4-D.1、M4-D.2、M4-D 和整个 M4。

当前停止位置为 M5-C `accepted`：M4、M5-A 与 M5-B 结论保持有效，Python
Server/Worker 正常运行，Rust Worker 停止。代码检查点
`4cfd7d86af0eff7b36ac615266a7d3a7c98ac7f6` 的 Lab 本地原子状态、排他锁、
可注入协调服务、完整 CLI、状态机、幂等恢复、loopback 健康/推理、日志和所有权失败
关闭已经完成离线验证、详细报告与用户验收。

当前 M5-D 设计和 D073–D075 已获用户接受。M5-D.0 已在代码检查点
`0fbc8d14b1792210e52aab782f92b15bc1fd9a32` 完成 `worker runtime info`、共享完整
label 校验和受显式执行门禁保护的 Rust example probe，经详细报告和用户审阅后状态为
`accepted`。该阶段仅完成离线构建、测试和静态执行门禁验证，没有连接真实 Docker
socket、读取 daemon/image、创建或删除 container。M5-D.1 只读预检方案已经冻结并获
明确执行授权：保持 Python Worker 运行，使用现有 CLI 和受限格式查询读取 daemon、
固定 Runner、受保护容器、历史残留、GPU、进程和端口，前后基线必须一致。M5-D.1
禁止 probe、pull、exec、日志、完整 inspect 和任何容器变更；结果在详细报告和用户验收
前只能是 `ready_for_review`。首次预检在外部用户的 GPU 1 `VLLM::EngineCore` 计算进程
门禁处以 `stopped_precondition` 停止，尚未运行 `worker runtime info` 或 inspect Runner。
随后用户接受共享主机隔离修订：M5-D.1/D.2 不申请任何 GPU，只要求未来目标 GPU0 按
稳定 UUID 保持空闲；允许已识别的 GPU1 外部任务运行，但其前后 identity 和资源快照
必须可归因且本项目不得修改。修订后的 M5-D.1 已从干净源码
`34ac580699013d0cd06719cff21296e2969c96d3` 完成：Rust CLI daemon 查询、固定 Runner
identity、受保护容器和服务不变、GPU0 空闲、GPU1 外部 principal 可归因以及最终零残留
门禁均通过；用户审阅详细报告后已接受 M5-D.1。随后用户明确授权 M5-D.2，只允许创建、
启动、停止并删除一个无 GPU 诊断容器，且不得停止 Python Worker。M5-D.2 已从干净源码
`25df8bbbd2a95ba7b1b9144ced30ebe24167104d` 完成：固定 Runner 的 Rust runtime
create/inspect/start/log/stop/remove、完整 ownership tuple、错误 owner 和受保护容器拒绝、
真实安全字段、服务前中后不变以及最终零残留门禁均通过。用户审阅详细报告后已接受
M5-D.2 和整个 M5-D；接受记录检查点为
`f4d2c6b488e9fddfab75838395a411f17be9b919`。该结果没有 pull、mount、网络、端口或 GPU
request，也不证明 vLLM、模型、GPU 推理或 Managed 兼容。下一步只允许讨论并冻结
M5-E；实际 smoke 仍须另获维护窗口授权，只允许精确分配 GPU0，且仍须停止 Python
Worker。

M5-E 的 D078–D081 已获用户接受。M5-E.0 的只读 Server/Docker/NVML/文件/端口检查与
Git 外证据准备已完成；用户在审阅 M5-E.0.1 后另行明确授权了 M5-E.1 首次维护窗口，
授权范围只包括停止和恢复 `gpustack-worker`，以及在 GPU0 上创建、启动、停止并删除唯一
固定 Qwen-7B Lab 容器。

M5-E.0 已执行：基础设施、模型文件 identity、Runner、Server 资源、服务、GPU0、端口、
零残留与回滚材料门禁均通过；唯一阻断项是固定 Qwen-7B 没有 `chat_template`。用户随后
接受 D082 修订方向，仓库已冻结模型原生 ChatML 语义的单行 fixture，并新增 CLI 解析和
workload argv 逐字传递回归测试。固定 Rust 1.99.1 下 106 项测试、fmt、locked/offline
check、Clippy、release build 和 Rustdoc 均通过，现有服务与 GPU 保持不变。M5-E.0.1
状态为 `accepted`。

M5-E.1 尝试 1 从干净 `1602f3e38b41` 完成最终预检、独立 watchdog 和 Python Worker
drain，但固定 deploy 在任何 deployment/container/cache/GPU request 创建前因
`127.0.0.1:42000` 瞬时 bind 冲突停止。事后直接 bind 可用，根因不能归于仍可观察的
ESTABLISHED 连接，因此该尝试分类为 `invalidated_non_scoring`，不改变 D079 固定端口，
也不产生模型加载或推理结论。rollback 已恢复同一 Python Worker；服务、Server 资源、
GPU0 和 Rust 残留门禁通过。下一步只允许讨论并冻结真实 bind 前置门禁；同 spec 重试仍
需新的明确维护窗口授权。

用户已接受 D083 并明确授权同 spec 重试。重试保持 D079–D082 全部输入和验收规则不变；
在停止 Python Worker 前执行真实短生命周期 bind，drain 后完成服务/GPU 检查并再次真实
bind，第二次成功后不插入其他网络命令而立即 deploy。任一 bind 失败均不得继续；drain
后的失败直接运行已验证 rollback。尝试 1 继续保留为 `invalidated_non_scoring`，不得因
重试结果覆盖或升级。

同 spec 正式重试已执行。D083 两次真实 bind、容器创建和 GPU0 精确 DeviceRequest 均
通过，但固定 Runner 内 vLLM 0.25.1 在启动阶段明确拒绝固定 Qwen-7B 的
`QWenLMHeadModel`，因此本次分类为 `formal_failure`，没有进入 readiness 或推理验收。
回滚同时暴露两个必须先修复的问题：root 容器写入的 Lab cache 不能由宿主机非 root CLI
删除，以及回滚脚本不能在容器已删而 deployment 尚存时幂等重入。受控恢复后 Python
Worker、Server 资源、GPU0、端口和全部 Lab 残留门禁均通过。

M5-E 下一步拆为两个前置工作：先在无 GPU 条件下修复 cache ownership/清理和回滚重入，
完成离线与无 GPU Docker 回归；再共同冻结 D079 的兼容性修订，可选方向为保留 Qwen-7B
并选用兼容 Runner，或保留当前 Runner 并选择受支持模型。未完成这两项并重新获得维护
授权前，不得自动发起第三次 GPU smoke。

用户已通过 D084 选择保留当前 Runner，并把新 smoke 模型固定为
`Qwen/Qwen2.5-0.5B-Instruct` revision
`7ae557604adf67be50417f59c2c2f167def9a775`。新 spec 不再传旧 Qwen-7B ChatML 或
`--trust-remote-code`，其余 GPU、端口和资源边界保持不变。实现顺序固定为：先修复 cache
ownership 与回滚重入并完成无 GPU 回归，再经本机 VPN 下载约 1 GB 的精确 revision、通过
局域网 SSH 传到 90、核对全文件 manifest，最后才讨论新的 GPU0 维护授权。

Qwen2.5 M5-E.1 正式 smoke 已完成全部冻结门禁。有效运行
证明固定模型/Runner 在 GPU0 上达到 readiness，Lab CLI 的 list/status/logs、同 spec
幂等、异 spec 冲突、completion/chat、stop/remove/cleanup 全部通过；原 Python Worker
已恢复，Server 资源 ID 集保持不变，最终容器、端口、cache、watchdog 和 GPU0 均无残留。
用户审阅详细报告后已接受 M5-E.1、M5-E 和整个 M5。接受范围是宿主机进程形态的单 GPU
Lab 与固定 Runner/Qwen2.5 路径，不扩展到 Managed、跨 Worker/双 GPU、Rust Worker
容器化或生产发布门禁。M6-A 的状态边界、Worker 端口责任、本地 ModelFile resolver、
写前刷新和 CLI 前移已经共同冻结。M6-A.1 离线 Worker 代码、fake/mock
测试和详细实现报告已经用户接受。M6-A.2 离线 Context、最小 Model CRUD/status 和
Worker maintenance CLI 已完成实现、mock 测试和 Git 推送。用户审阅 D033 详细实现报告
后已接受 M6-A.2；接受范围仍仅是离线 CLI、mock 契约和构建门禁。源码复核发现文档中的
`--managed-file-only` 尚未接到 M6-A.1 协调器，因此增设并已授权 M6-A.2.1 离线生产入口
接线门禁。该门禁的源码、fake/mock 测试、离线质量门禁和 Git 推送已经完成，目前为
`accepted`。讨论 M6-A.3 回滚时确认 Server 删除 Model 不会自动删除独立 ModelFile，用户
因此接受 D095–D097，并在真实 smoke 前增设 M6-A.2.2。该门禁只实现
`model file get/delete` 的类型化运维 CLI、安全删除语义和离线测试；完成详细报告并获接受
后，才能进入 M6-A.3 的独立只读预检。真实 Server 写入、maintenance 变化、停止 Python
Worker、Docker 或 GPU 操作仍须另行明确授权。

M6-A.2.2 的源码、mock 契约、完整 Rust workspace 质量门禁、干净构建、Git 提交和推送
已经完成。用户审阅详细实现报告后已接受该门禁，目前为 `accepted`。D098–D102 和
`m6a3-managed-file-runbook.md` 已冻结 M6-A.3.0 只读预检以及 M6-A.3.1 真实 smoke 的
输入、检查项、停止条件、证据和回滚。M6-A.2.3 的只读 Worker/ModelFile CLI 已完成
离线实现、质量门禁、D033 详细报告和用户验收，目前为 `accepted`。第一次 M6-A.3.0 因
operator Context 缺失停止，第二次在固定 `m3d` identity ID 2 与 Server ID 5 不一致时停止；
两次证据均已封存。D107–D116 已完成 Context 准备、当前 ID 5 token 只读验证、`/1` 数据
目录选择和权限收紧，并把第三次证据滚动到尚未创建的 `m6a3-formal-attempt-3`。
第三次执行的冻结构建通过，但 `context list` 因错误携带全局 `--context` 在 CLI 参数检查
阶段失败，未读取 token 或发送该 CLI 请求；attempt 3 已按非评分 `INVALIDATED` 封存。
M6-A.3.0 因此一度为 `waiting_for_retry_amendment`，当时新的证据路径和执行授权尚未冻结。
后续真实 Server 写入、maintenance 切换和 Python Worker 维护窗口仍须 M6-A.3.1 的独立
明确授权。

D117–D119 已把第四次只读预检证据冻结为尚未创建的 `m6a3-formal-attempt-4`，并把 Context
枚举修正为不带 `--context` 的本地命令；真实 Worker/Model/ModelFile 查询仍使用
`--context lab90`。用户已授权执行第四次 M6-A.3.0，当前为
`authorized_pending_execution`。该授权仍不包含 M6-A.3.1 或任何 Server/Docker/GPU 写操作。

第四次预检已执行并在 GPU0 空闲硬门禁停止。此前源码/构建、Context、当前 ID 5 identity、
目标 Model/ModelInstance/ModelFile 零冲突及固定 Qwen2.5 manifest 均通过；GPU0 的唯一
compute process 属于既有 Rust Lab `qwen2.5-0.5b-m5e`，不是本次尝试创建的资源。
M6-A.3.0 attempt 4 保持 `stopped_preflight`。用户随后自行停止并删除该测试 Lab；只读
复核确认精确容器和部署状态文件消失、GPU0 空闲、固定模型文件保留且现有服务健康。
当前状态为 `ready_for_retry_amendment`：下一步只能讨论并冻结第五次独立证据路径和新的
M6-A.3.0 只读执行授权。M6-A.3.1 仍未授权。

D120–D122 随后把第五次证据固定为尚未创建的 `m6a3-formal-attempt-5`，要求从头执行所有
预检门禁，并获得冻结构建、真实只读查询和 Git 外证据写入授权。M6-A.3.0 当前为
`authorized_pending_execution`；M6-A.3.1 和全部写路径仍未授权。

attempt 5 在源码 identity 门禁后因脚本从仓库根目录而非 `rust/` workspace 执行 Cargo
命令，以退出码 101 停止；未进入构建或真实查询。该尝试按非评分
`INVALIDATED/invalid_cargo_working_directory` 封存，M6-A.3.0 回到
`waiting_for_retry_amendment`。新的证据路径、修正脚本和执行授权尚未冻结；M6-A.3.1 仍
未授权。

D123–D125 随后把第六次证据固定为尚未创建的 `m6a3-formal-attempt-6`，要求 Cargo 显式
使用 `rust/Cargo.toml`，并在正式目录创建前通过脚本语法、workspace metadata 和窄范围
差异审计。用户已授权审计、冻结构建、真实只读查询和 Git 外证据写入，M6-A.3.0 当前为
`authorized_pending_execution`；M6-A.3.1 与全部写路径仍未授权。

第六次静态审计和完整预检随后全部通过。冻结源码、二进制、Server、Context、Worker ID 5、
目标资源零冲突、固定 Qwen2.5 manifest、GPU0 空闲、受管端口、Python Worker 精确恢复材料、
回滚 CLI 和全局资源 ID 基线均形成完整证据，且没有产生运行状态副作用。M6-A.3.0 当前为
`passed_preflight`，M6-A.3.1 为 `ready_for_authorization`；下一步只能共同审阅并取得独立
维护窗口授权，不能把只读预检通过解释为 Managed 状态链已经验收。

用户随后接受 D126–D128，授权 M6-A.3.1 attempt 1，并把唯一证据路径固定为
`m6a3-managed-file-smoke-attempt-1`。当前状态为 `authorized_pending_execution`。正式执行
仍须先通过外部漂移门禁；授权只覆盖无 Docker 文件准备状态链和完整回滚，不覆盖 M6-B
runtime、GPU workload 或推理。

attempt 1 的静态审计和外部漂移门禁通过，但正式执行在维护窗口前停止。执行器对持久
identity 使用 `.id`，而冻结文件契约字段为 `worker_id`，因此得到
`STOP/persisted_worker_id_mismatch`；源码构建已完成，后续资源基线尚未开始。Python Worker
未停止、Worker ID 5 maintenance 未改变、没有创建 Model/ModelInstance/ModelFile、没有
启动 Rust Worker，也没有 Docker、GPU 或推理副作用。证据以 0700/0600 权限封存，秘密
扫描为 0，独立恢复审计通过。M6-A.3.1 当前为 `stopped_before_window`，不能把 attempt 1
修正为通过或在原目录续跑。下一步只允许修订 pre-window identity/cleanup 门禁、冻结新的
attempt 2 证据路径并重新取得维护窗口授权；M6-B 仍未开始。

用户随后接受 D129–D131：attempt 2 的唯一正式路径固定为
`m6a3-managed-file-smoke-attempt-2`；持久 identity 改用权威 `worker_id` 字段；执行器引入
`baseline_ready`，在基线完成前的 STOP 只做无写安全复核，不再调用依赖缺失 baseline 的
资源 cleanup 或集合比较，基线完成后仍执行原完整回滚。当前阶段只授权执行器准备、静态
审计和只读漂移检查，不授权停止 Python Worker、切换 maintenance、创建测试资源或启动
Rust Worker。静态门禁通过后状态才能成为 `ready_for_authorization`，随后仍需单独维护窗口
授权；固定输入、状态链和 M6-B 边界不变。

attempt 2 的执行器、pre-window failure injection 和只读漂移门禁随后通过，用户再按 D132
明确授权一次真实维护窗口。正式执行完成冻结构建后，在 identity 门禁把权威
`worker_name` 错读为 `.name`，因此以 `STOP/persisted_worker_name_mismatch` 在运行基线和
维护窗口之前停止。`baseline_ready=0`、`window_entered=0`、`cleanup_ok=1`；Python Worker
未停止，maintenance 未改变，没有创建测试资源、Rust Worker、Docker/GPU workload 或推理。
正式与 watchdog 证据清单均校验通过，事后零残留和服务恢复审计通过。M6-A.3.1 当前仍为
`stopped_before_window`；下一步只能共同审阅并冻结 attempt 3 的新路径和 `.worker_name`
窄修正，再完成静态/只读门禁并重新取得维护窗口授权。不得自动重试或进入 M6-B。

用户随后接受 D133–D135，将 attempt 3 正式目录固定为
`m6a3-managed-file-smoke-attempt-3`，仅允许把持久名称读取修正为 `.worker_name`，并授权在
静态、pre-window failure injection 和完整只读漂移门禁全部通过后执行一次真实维护窗口。
当前状态为 `authorized_pending_execution`；任何准备门禁失败都不得创建正式证据目录，正式
运行失败则必须恢复并封存，且不能自动进入后续 attempt 或 M6-B。

attempt 3 随后进入真实维护窗口并通过 Python Worker stop 门禁，但 readiness 循环中的两个
`curl` 命令替换在首次连接拒绝时以退出码 7 触发顶层 `set -e`，没有执行原定的 120 次重试。
Rust Worker 实际稍后完成 ID 5 持久身份、双 GPU 和 maintenance-required 启动，但 cleanup
已经开始；执行器按精确 PID 停止 Rust Worker、恢复原 Python Worker并通过全部前后基线与
零残留检查。没有 maintenance disable、模型资源、Docker/GPU workload 或推理。M6-A.3.1
当前为 `failed_during_window`；下一步只能讨论并冻结 attempt 4 的新路径及两个 readiness
probe 的 `errexit` 窄修正，再重新完成准备门禁和维护窗口授权，不得自动重试或进入 M6-B。

用户随后接受 D136–D138：attempt 4 固定使用 `m6a3-managed-file-smoke-attempt-4`，只把两个
readiness curl 改为不触发 `errexit` 的可重试采样，并授权在增强静态、三步 synthetic
readiness fixture、pre-window failure injection 和完整只读漂移门禁通过后执行一次真实
维护窗口。对不改变产品代码、固定输入、资源范围、超时、成功标准或回滚协议的后续同类
执行器缺陷，可在封存和恢复审计通过后按新目录/最小差异/全门禁自主处理；超出该边界仍须
询问。当前 attempt 4 为 `authorized_pending_execution`。

attempt 4 随后通过全部准备门和 Python Worker stop 门，Rust Worker 也在冻结 Worker IP
`10.213.80.90:20150/20151` 正常启动；但执行器的两个 readiness probe 仍访问 loopback，
该地址返回 000，最终触发 `rust_worker_startup_gate_timeout`。统一 cleanup 已停止 Rust Worker、
恢复 Python Worker并通过全部零残留检查，且没有模型资源、Docker/GPU workload 或推理。
按 D138–D139，attempt 5 只把这两个 probe 改为 Worker IP，使用新证据目录并重新完成全部
准备门；当前状态为 `authorized_pending_execution`，M6-B 仍未开始。

attempt 5 证明 Worker IP 探针正常，但 readiness 错把 maintenance 开启时的 Server 状态断言为
`ready`；权威响应和既有协议均要求 `maintenance`。cleanup 再次完整恢复且没有创建模型资源。
D140 将 attempt 6 的唯一行为变化限定为断言 `state=maintenance`，并同时检查
`maintenance.enabled=true`、`unreachable=false` 和双 GPU。为避免再次逐点暴露执行器假设，
后续 CLI、协调器、事件与正式 jq/cleanup 表达式已整体排练通过；attempt 6 仍须重跑全部
准备门，当前状态为 `authorized_pending_execution`。

attempt 6 首次通过启动门并解除 maintenance，但 `model deploy` 被真实 Server 以 HTTP 400
拒绝：手工 GPU 选择没有 cluster context。目标 Worker 的真实 `cluster_id=1`，而 Rust
`ModelCreate` 和 deploy 请求尚未携带该字段。这属于产品代码与固定 v2.2.3 Server 的契约缺口，
不在 D138 的自主执行器修正范围内；cleanup 已完整恢复且 Model 未创建。当前状态为
`product_fix_decision_pending`：需共同决定是否让 CLI 唯一解析目标 Worker、要求其 cluster ID
非空并写入 ModelCreate，同时增加回归测试；接受前不得开始 attempt 7。

D141 已按上述方案完成并在代码检查点
`208ac76dfd78beca4c697afd05235e0c974bdd8d` 通过整个 Rust workspace 150 个离线测试及相关
Clippy 门禁。当前状态更新为 `attempt_7_preparation_ready`：可建立新的不可复用证据目录并完成
静态、failure-injection 和只读漂移门禁；真实维护窗口仍需单独授权，且本阶段仍只验证
Managed ModelFile 准备链，不进入 M6-B Docker/vLLM runtime。

D142 已冻结 attempt 7 路径和相对 attempt 6 的最小差异；只读预检、预窗口故障注入和完整
外部漂移门均通过，正式目录保持不存在。绑定最终治理 HEAD并复跑外部门后，状态转为
`ready_for_maintenance_authorization`；维护窗口不因准备门通过而自动获准。

attempt 7 在授权后成功验证 D141：Server 接受 `cluster_id=1` 的 Model 10 并创建 Instance 25。
但实例在 300 秒内保持 `pending`，调度器明确只在 READY 门排除了目标 Worker。maintenance
解除后 Rust Worker仍为 `not_ready`，因为 `managed-file-only` 的健康状态报告持续发送非空
受限模式 `state_message`，触发固定 Server 的显式 NOT_READY 保留规则。cleanup 已删除测试
资源、恢复 maintenance/Python Worker并通过全部零残留比较；没有 ModelFile、端口、Docker、
GPU compute、推理或 `RUNNING`。M6-A.3.1 仍未通过。

D143 将下一实现范围冻结为：仅在 `managed-file-only` 健康状态报告中发送空
`state_message`，保留启动 maintenance 门和 operator 显式切换；`resource-api-only` 继续保持
不可调度。完成两种模式回归、workspace 测试、格式和 Clippy 后，才能讨论 attempt 8 的新
证据路径和维护窗口；不得直接复用 attempt 7 或进入 M6-B。

D143 已在 `85d6b8f478de2b096e35d0c22ddaa8ab1768fa39` 完成并通过 150 个 workspace
测试、全 workspace/all-targets Clippy 和格式门。D144 已把 attempt 8 固定为新的预检、正式和
watchdog 路径；下一步允许执行脚本审计、冻结构建和完整只读漂移门。即使准备通过，也不得
自动进入真实维护窗口或 M6-B。

attempt 8 的只读预检和预窗口故障注入已经通过，现有服务、maintenance、资源、端口与 GPU
保持未变。提交准备检查点并把 executor 绑定到最终治理 HEAD后，只须重跑最终无写漂移门即可
进入 `ready_for_maintenance_authorization`；这仍不构成真实维护窗口授权。

attempt 8 随后在明确授权下进入维护窗口。D143 修复生效，目标 Worker 通过 READY 门；但固定
Server 在计算候选资源时因 Rust 上报 `system_reserved=null` 无法读取 `.vram`，Model 11/
Instance 26 始终停在 `pending`。正式结果为 `FAIL/state_chain_did_not_reach_starting`、
`cleanup_ok=1`：没有 ModelFile、端口、容器、GPU workload 或推理，测试资源、maintenance、
Python Worker、服务、端口、Docker、GPU、模型 manifest 和仓库状态均恢复到基线。

D145 已在 `8b06832440f222600721171f8d1ede7e9bab4004` 让注册和周期状态明确发送
`SystemReserved { ram: 0, vram: 0 }`，与 Python Worker 的未配置语义对齐。153 个
all-target 测试、all-features/all-targets Clippy、格式和 workspace build 均通过。当前停止位置
为 `attempt_9_preparation_ready`：下一步只允许冻结新证据路径、修正正式脚本使容器环境在落盘
前脱敏，并完成静态、故障注入和只读漂移门。新的真实维护窗口仍需单独明确授权，M6-B 未开始。

D146 已将 attempt 9 的预检、故障注入、正式和 watchdog 路径冻结，并规定所有容器 inspect
证据在写盘前删除 `Config.Env`。当前状态为 `attempt_9_preparation_authorized`：可执行脚本
最小差异审计、冻结构建、故障注入和完整只读漂移门；不得进入维护窗口。准备结果必须另行记录，
只有全部通过后才能请求真实 attempt 9 授权。

attempt 9 只读预检和预窗口故障注入已通过，且证明 `Config.Env` 不进入新证据。当前停止位置
为 `attempt_9_post_checkpoint_binding_pending`：提交准备记录后，只允许把最终 executor 和
supervisor 绑定到新的治理 HEAD并执行最后一次无写漂移门。门禁通过后状态才能变为
`ready_for_maintenance_authorization`；真实窗口和 M6-B 仍未授权。

attempt 9 在授权后通过 READY 和调度门：Model 12/Instance 27 被分配到 Worker ID 5，证明
D145 的资源保留字段修复有效。固定 Server 同时为单 Worker实例写入
`distributed_servers.mode=initialize_later` 和 `subordinate_workers=null`；Rust 将对象非空误判为
跨 Worker范围并 no-op，实例停在 `scheduled`。正式结果为
`FAIL/state_chain_did_not_reach_starting`、`cleanup_ok=1`，没有 ModelFile、端口、容器、GPU
workload 或推理，完整恢复审计通过。

D147 已在 `50bc840b5b00cdedbde4cd3e32676054f1cc91e6` 改为只在
`subordinate_workers` 非空时判定跨 Worker推理，并对畸形结构 fail closed。156 个 all-target
测试、all-features/all-targets Clippy、格式和 workspace build 全部通过。D148 将当前状态设为
`attempt_10_preparation_authorized`：允许建立新证据路径并完成静态、failure-injection、冻结构建
和只读漂移门；真实维护窗口仍需单独授权，M6-B 未开始。

attempt 10 的只读预检和预窗口故障注入已通过，且再次证明完整容器环境不会写入证据。当前
状态为 `attempt_10_post_checkpoint_binding_pending`：提交准备记录后，只允许把最终 executor 和
supervisor 绑定到新的治理 HEAD并执行最后一次无写漂移门。门禁通过后才能请求真实 attempt 10
授权；M6-B 未开始。

attempt 10 随后在明确授权下把真实状态链推进到 `starting`，并通过 60 秒无 runtime/Docker/GPU
隔离门；但固定 Server 为本地 ModelFile 返回合法 `cleanup_on_delete=null`，正式执行器错误要求
false，导致状态链循环多等 300 秒并拒绝清理该元数据。正式结果因此保持 FAIL，独立窄范围审计
已删除唯一残留 ModelFile并证明全部运行状态恢复。该结果不能提升为 M6-A.3.1 通过。

D149-D150 已冻结 attempt 11：ready 和 cleanup 都接受 null/false 两种不删除本地文件语义；
状态链使用独立完整条件标志，300 秒后若未同时满足 Instance starting 与 ModelFile 的全部字段则
硬失败。下一步可完成 attempt 11 的全新路径、静态/最小差异审计、故障注入、冻结构建和只读
漂移门。准备通过后状态最多为 `ready_for_maintenance_authorization`；不得自动进入维护窗口或
M6-B。

attempt 11 的静态/语义审计、冻结构建、完整只读预检和预窗口故障注入均已通过，服务、资源、
maintenance、Python Worker、端口与 GPU 未改变。当前状态为
`attempt_11_post_checkpoint_binding_pending`：提交准备记录后，只允许把最终 executor/supervisor
绑定到新的治理 HEAD并重跑无写漂移门。通过后才能请求新的维护窗口授权；M6-B 仍未开始。

attempt 11 随后在明确维护授权下通过最终只读漂移门和全部正式门禁：Model 14/Instance 29
被调度到 Worker ID 5 的 GPU0/端口 40000，ModelFile 8 的 ready、100% 进度、精确大小、规范
路径和 null/false 保留语义与同一快照的 Instance `starting` 共同满足 D150；60 秒内没有
inference listener、Docker/GPU workload、推理、`RUNNING` 或 `ERROR`。统一 cleanup 和独立
事后审计均通过，全部服务与资源恢复基线。M6-A.3.1 状态更新为 `accepted`；下一步只允许讨论
并冻结 M6-B runtime 启动方案，尚未授权创建 Docker/vLLM/GPU workload 或执行推理。

接受后已按 D108 删除 M6-A.3.1 专用 Server API Key、`lab90` Context 和本地 token 文件，
并封存独立 PASS 证据。M6-B 不能依赖或复用该凭据；若正式验证需要管理权限，必须把新的最小
权限凭据及其销毁时点纳入 M6-B 设计和授权门禁。

M6-B.0 设计已接受，M6-B.1 已在产品代码检查点
`db6550311075c9eac5f0472fccc8029f4348bf09` 完成共享 runtime 原语和 Managed
workload planner 的离线实现。它已通过固定 Rust 1.99.1 下的 workspace
格式、all-features/all-targets Clippy、162 项测试、locked/offline build 和
`git diff --check`；本阶段未增加 Cargo 依赖，也未修改 `Cargo.lock`。当前状态为
`accepted`：未连接真实 Docker socket，未写入 Server，未变更 maintenance，
未停止 Python Worker，未创建 GPU workload 或执行真实推理。用户已审阅并接受详细
实现报告；下一项允许执行 M6-B.2 Managed reconciler 离线实现。

M6-B.2 已在源码检查点 `6550ac6a419b0253f2219b0b3f0456a3c5e42bf0` 完成
`--managed-runtime`、Server 权威写前刷新、单容器创建/接管、vLLM 健康后 `RUNNING`、
有界脱敏 `ERROR`、0600 当前 session 日志、辅助 manifest、Lab/Managed 排他锁和权威
404 后精确 cleanup 的离线实现。90 服务器在该干净提交上通过格式、all-targets/
all-features Clippy、176 项 locked/offline workspace 测试、build 和差异门；未新增依赖，
也未连接真实 Docker、写 Server、切 maintenance、停止 Python Worker、占用 GPU 或推理。
用户已接受 D033 详细实现报告，M6-B.2 状态更新为 `accepted`。下一项允许执行 M6-B.3
推理代理与 CLI 离线实现；只允许 mock transport、fake runtime 和 locked/offline 质量门，
不授权任何真实 Docker、Server、maintenance、Python Worker、GPU workload 或推理操作。

M6-B.3 已在产品代码检查点 `c78cfc5652808ee5c16b40b11df411dadc7d6b6d`
完成内部健康路由表、动态 readiness、安全 Worker 推理代理、Server-only 日志/推理 CLI 和
`enable_model_route=true`。精确 HEAD 通过 181 项 locked/offline workspace 测试、格式、
all-targets/all-features Clippy、doc-tests 与 build；真实 Docker、Server、maintenance、
Python Worker、GPU 和推理均未触碰。用户已接受 D033 详细实现报告，当前状态为
`accepted`；下一项允许执行 M6-B.4 只读预检与离线故障注入。该授权不包括停止 Python
Worker、写入 Server、改变真实 Docker 资源、申请 GPU、启动 vLLM 或执行推理，也不得据此
自动进入 M6-B.5。

M6-B.4 已在治理 HEAD `a4e2aa2620efab7bb09667fa3640c8b358a374ad` 完成。attempt 1
因证据清单生成后继续追加 console 输出而被独立复核判为 `invalidated`；修正封存顺序后的
attempt 2 通过固定环境只读门、locked/offline build、六项 fake/mock 故障注入、全部正式
before/after 不变性、秘密扫描和独立证据清单复核。Server、Python Worker、Docker、GPU、
端口和 Server 资源没有改变；用户已接受详细验证报告，当前状态为 `accepted`。M6-B.5 的
新短期最小权限 operator Context、正式资源 identity、watchdog、回滚方案和维护窗口仍未
冻结或授权；下一项只允许讨论并冻结这些边界。

M6-B.5.0 方案和 D160–D165 已获用户接受。正式阶段拆为 B.5.1 短期 operator 凭据与最终
只读准备，以及另行明确授权的 B.5.2 维护窗口；唯一 workload 固定为本地
`Qwen2.5-0.5B-Instruct`、GPU0、固定 Runner digest、TP=1 和既有参数。B.5.2 必须证明完整
状态链、Server-only buffered/streaming 推理、日志、至少三轮同进程幂等、权威删除后零残留、
Python Worker恢复和凭据退役。当前只允许准备新 API Key并执行 B.5.1，不授权停止服务、
改变 maintenance、创建模型/容器、申请 GPU 或推理。

D166 已修订 operator 凭据：首次 Web Key ID 20 的单一 `management` scope 和 7 天有效期不
满足正式推理与生命周期门，已在用户明确授权下替换为 ID 21、名称
`rust-worker-m6b5-operator-20261028-v2`、scope `management,inference`、有效期 86400 秒；
新 Key 的双 scope GET 均为 200，旧 Key 已删除且返回 401。当前状态为
`operator_credential_ready`，下一项只允许在新治理 HEAD 上执行 B.5.1 Context 和只读准备门。

M6-B.5.1 的 Git 管理正式执行器、预检器和独立 watchdog 已实现。预检 attempt 1 因 deployed
Server API Key 路径版本写错而在凭据门安全停止；attempt 2 的全部技术门通过且外部状态不变，
但结果文件的 attempt 字段仍硬编码为 1，因此不采纳为最终授权证据。该字段已改为强制显式
输入；下一步只允许在最终干净治理 HEAD 上运行 attempt 3。attempt 3 通过后状态最多为
`ready_for_maintenance_authorization`，B.5.2 仍需用户新的明确授权。

M6-B.5.1 attempt 3 随后在治理 HEAD `145437bc852cd30fae318aceeb9dea1e07f84adc`
通过。M6-B.5.2 attempt 1 在明确授权下进入窗口，但因解除 maintenance 后未等待 Worker
稳定 READY，时间线出现 `ANALYZING -> PENDING` 且 CLI 漏掉初始 `PENDING`，正式状态链门
失败。实例随后仍完成真实 ModelFile、Docker/vLLM 和 `RUNNING`；失败 finalizer 删除模型时，
固定 Server 未及时送达删除 watch，而 5 分钟 full sync 超过 180 秒 cleanup 门，随后 Rust
进程又未在 SIGINT/SIGTERM 后有界退出。精确 fallback 清理和独立事后审计证明 Server 资源、
Python Worker、maintenance、容器、端口、GPU0、本地状态和模型文件全部恢复。

D167-D170 已把 attempt 1 保持为正式 FAIL，并实现创建前 watch、稳定 READY 门、30 秒权威
full sync、可中断协调、15 秒顶层退出上界、精确 watchdog 信号升级和强制 attempt 参数。
代码 HEAD `e66634c523ca37c2e08d50a7656710cb6b16480b` 已通过 188 项 locked/offline
workspace 测试、格式、all-features/all-targets Clippy、build 和 doc-tests。preflight attempt 4
随后在第一组 `source_and_scripts` 门安全停止：脚本仍固定旧产品提交，尚未承认上述已验证的
crate 变化；它未执行凭据、服务、Docker 或 GPU 检查。D171 将 retry 产品检查点显式滚动到
`e66634c523ca37c2e08d50a7656710cb6b16480b`，并要求使用全新的 attempt 5 准备和无写注入
目录。当前状态为 `retry_checkpoint_rollover_pending_preflight`；即使 attempt 5 通过，正式
attempt 2 仍须新的明确维护窗口授权。

preflight attempt 5 又在同一第一组门安全停止；产品 ancestry/diff、工具链和锁文件均已通过，
唯一不一致是由本机完成 GitHub 推送后，90 的本地 `origin/codex/rust-worker` 缓存仍停在旧
提交。90 已通过精确 fetch 把该引用同步到治理 HEAD，attempt 5 未进入任何运行态检查。
D172 要求全新的 attempt 6 准备和无写注入目录；通过后仍只能进入
`ready_for_maintenance_authorization`。

preflight attempt 6 已在治理 HEAD `cdb2a5eeaf7f8f7ffe11fb4eff34ec6259867562` 通过全部九组
门禁。预检和无写注入清单文件 SHA-256 分别为
`bb394acc499cf03169ea74547554a76f44d77e7cd414b7001e7b762abe26dab2` 和
`6a071b9d646687d7a6a0c8c1da9af4982126223193bd12eee579b1b23f78e0cc`；独立复核证明
权限、清单、秘密扫描、服务、资源、Docker、GPU、端口、进程和源码前后不变。当前状态为
`ready_for_maintenance_authorization`，下一步只能请求正式 B.5.2 attempt 2 的新维护窗口授权。

用户随后明确授权 attempt 2。该次运行真实完成稳定 READY 和完整七状态链，但在推理前因
`server_running_preceded_runtime_health` 失败：健康探针通过后 Rust 先写 Server RUNNING，
再输出 `outcome=running`，Server watch 因而比日志时间早约 20 ms。统一 cleanup 自动删除
Model 16、Instance 31、ModelFile 10 和唯一 owned 容器，Rust 优雅退出并恢复精确 Python
Worker。正式、watchdog 和有效独立事后审计清单 SHA-256 分别为
`f32b8f86380c438236af1f97fa884b601bef1f7c0aaddbd7aa9b1a5537e01de6`、
`fa137abae7d87ef75a3633cbc5aa3f9eab7981ae622c733a4c89df9664a72843` 和
`313313059d5174d675461212b86435a8373ba2dbd41c890fc2bba3adaaab8c24`。attempt 2 保持 FAIL。

D175 在产品提交 `799351f1068adabfded57ab2a6cd2a56749352e6` 增加写回前健康纳秒时间并让
executor 比较统一时间轴；两个针对性测试、真实 shell 时间 fixture、189 项 locked/offline
workspace 测试、格式、Clippy、build 和 doc-tests 全部通过。下一步只允许在新的治理 HEAD
上运行 preflight attempt 7 和无写注入；正式 attempt 3 仍须新的明确维护窗口授权。

preflight attempt 7 随后在治理 HEAD `aa691887fb9caedff0fb7e7dec9bcdd53a5fe735` 通过。
预检和无写注入清单文件 SHA-256 分别为
`8f3eb6a3e5443013daeb11f49f0a03f0b51c42e2fef08bcf98b796665b0b4a58` 和
`b7522c7465e5add88f2b069d17e2bb3919158fac441018c1bdac8db6d3f9b8ce`；独立复核证明
55/9 个条目、0700/0600 权限、秘密扫描和外部不变式全部通过。当前状态为
`ready_for_maintenance_authorization`，下一步只能请求 formal attempt 3 的新维护窗口授权。

formal attempt 3 完成七状态链和 `inference models`，但 Server ai-proxy 使用 Cluster
registration token，Rust 推理路由只接受 per-Worker token，buffered chat 返回 401。D178 将
双 token 兼容严格限定到 Managed chat/completion；attempt 4 随后因 executor 未传新的必需
`--token-file` 在 Rust Worker启动前失败。执行器和 preflight 修正后，attempt 5 完成 Rust
Worker、七状态链、健康时序、真实 vLLM 和模型路由，但 buffered chat 返回 400。

attempt 5 根因是 Server传入 `model-<model_id>-<instance_id>.static` 复合 destination，而 Rust
只接受纯数字 Instance ID。产品提交 `c725436bb2e117228866441f59715c332bfa042a` 已按原
GPUStack/Python Worker协议解析该头，并通过 workspace 190 个测试和全部离线质量门。attempt 5
统一 cleanup、watchdog 和独立事后审计均证明完整恢复，但该次仍为正式 FAIL。当前状态为
`retry_6_preflight_pending`：下一步只能把 preflight/injection attempt 10 绑定到新提交并运行
离线/只读门；formal attempt 6 仍须新的明确维护授权。

preflight attempt 10 随后在治理 HEAD `255f87c916462b485e4ddb69bd4bf9d65de262a9` 通过。
预检和无写注入清单文件 SHA-256 分别为
`1c08a998d040a2a264d29194c1ecb3aeccb0c798299a62e39eeeccd88f9b06d2` 和
`6c9f520e16bad891aaffa24a83e61f3d037d604592fb318bcc8b9c29e0bfae2f`；独立复核证明
57/9 个条目、凭据和秘密扫描、服务、Server 资源、Docker、GPU、端口、进程与源码前后不变。
当前状态为 `ready_for_maintenance_authorization`，下一步只能请求 formal attempt 6 的新授权。

formal attempt 6 随后通过复合 destination 解析、完整七状态链、健康时序和 Server模型列表，
但 buffered chat 在 Rust Worker到 vLLM 的连接层失败：vLLM 按 workload 监听
`10.213.80.90:40000`，代理却固定请求 `127.0.0.1:40000`。该次统一 cleanup、watchdog 和
22 项独立事后审计均证明完整恢复，但正式结果仍为 FAIL。产品提交
`d4920e71b89d6507f8e82fb2aff7dfbb24490103` 已把 upstream 改为已验证的
`worker_ip + assigned_port`，缺少 Worker IP 时 fail closed，并通过 workspace 190 个测试和
全部离线质量门。当前状态为 `retry_7_preflight_pending`：下一步只能运行全新的
preflight/injection attempt 11；formal attempt 7 仍须新的明确维护授权。

preflight attempt 11 随后在治理 HEAD `76c0f5d855de3878fefb0a826da64053d070bd76` 通过。
预检和无写注入清单文件 SHA-256 分别为
`1c76b57e12b6540fc835ca81eec340de22e003a3b3805497d5cafc43b1ea10c9` 和
`1a9fe515f3f6c3ff39961802b6c70020b2c709d68e0b3b283a6e75b13df5e6eb`；独立复核证明
57/9 个条目、凭据与秘密扫描、服务、Server 资源、Docker、GPU、端口、进程和源码前后不变。
当前状态为 `ready_for_maintenance_authorization`，下一步只能请求 formal attempt 7 的新授权。

formal attempt 7 随后在独立授权和 watchdog 保护下通过。Model 20、Instance 35、ModelFile 14、
端口 40000 和唯一 GPU0 vLLM 容器完成完整七状态链；Server-only models、buffered/streaming
chat、completion、错误模型拒绝、tail/follow logs、三轮同进程幂等和幂等后 chat 全部通过。
精确 cleanup、watchdog、22 项独立事后审计及最终现场复核证明 Python Worker、maintenance、
Server 资源、Docker、端口、两张 GPU、固定模型和源码恢复基线。正式/watchdog/post-audit 清单
文件 SHA-256 分别为 `c81af217bf80a4ca568de8c45e5a602c624fe87507967c6ce1f6f4dbe9fc9eb3`、
`cabb29165c85b5a8b68cbe1023b8863ab2d12bc1d4385eed1eb271899c947fa7` 和
`8d60613d2086ec708261368a126079f896bd49f96bba067ffac931ee3a6482da`。当前状态为
`ready_for_review`；用户接受详细报告后只先执行阶段凭据退役，完成 401 失效证明后才能标记
M6-B.5 `accepted`，不得自动进入 M7。

用户随后接受详细报告。API Key ID 21 删除返回 200，旧 token 返回 401，`m6b5-operator`
Context 和 operator token 文件已删除，Cluster registration token 未改变；15 个退役清单条目
和独立服务/隔离复核全部通过，清单文件 SHA-256 为
`87e34cef088ee9506a8ced0157d2bd1eadb5f55f99292d5469089c473a04e6a0`。M6-B.5 状态更新为
`accepted`。下一项只允许讨论和冻结 M7 的范围、故障模型、验收与授权边界，不自动进入真实
执行。

用户随后接受 M7 设计。M7-0 冻结宿主机进程接管、Python 对齐的 restart-on-error、当前/上一
日志代际、只影响 Rust Worker 的 Server/Docker 故障注入、分离的单 GPU 恢复窗口、固定
Qwen2.5-0.5B 双 A100 TP=2 阶段验证，以及 M8 容器化重跑边界。M7-0 状态为 `accepted`；
下一项只允许进入 M7-A 启动接管的离线实现。该设计接受不授权连接真实 Docker socket、写真实
Server、停止 Python Worker、切换 maintenance、创建 workload、使用 GPU 或创建阶段凭据。

M7-A 离线实现现已固定在产品提交
`852e16c87be2f6dfb6b0d0b9bc1c925399bb04a5`，并经用户审阅更新为 `accepted`。实现只在收到
首次权威 full sync 后协调 `STARTING`/`RUNNING`；对唯一完整 tuple/spec 匹配且健康的
`RUNNING` 容器重建 route、manifest、cache 和当前日志跟踪，重复调和保持 `Noop`。缺失、
不健康、多匹配或冲突的 `RUNNING` runtime 均 fail closed，不创建、启动、停止、删除容器或
写回 Server；缺失 runtime 的自动重启、非零 restart generation 和连续健康监测仍属于 M7-B/C。
90 上 workspace fmt、Clippy `-D warnings`、197 项 all-target 测试、doc-tests 和
locked/offline build 全部通过。下一项只允许进入 M7-B restart-on-error 与日志代际的离线
源码复核、实现、fake/mock 测试和详细报告；不得连接真实 Docker/Server、停止服务或使用 GPU。

M7-B 离线实现现已固定在产品提交
`421e5d3232a1890064729fedc8163b2dd9db3d0c`，并经用户审阅更新为 `accepted`。
实现对齐固定 Python Worker：受管理 runtime 从权威 `RUNNING` 进入终止或缺失事实时先写
`ERROR`；启用 `restart_on_error` 后按进程内连续失败计数执行 0、10、20、40、80 秒并以
300 秒封顶的退避，重新确认 Server、spec 和完整 ownership 后只清理精确失败容器，再把持久
`restart_count` 加一、写 `last_restart_time` 并返回 `SCHEDULED`。健康 `RUNNING` 会清零暂态
退避，关闭策略则保持 `ERROR`。manifest、日志捕获和启动接管均支持非零 restart generation；
只保留当前和上一代日志，权威永久删除清除全部代际。90 上 workspace fmt、Clippy
`-D warnings`、203 项 all-target 测试、doc-tests 和 locked/offline build 全部通过，本地目标
文件 pre-commit 与 `git diff --check` 通过。下一项只允许进入 M7-C 离线故障矩阵的源码复核、
实现、fake/mock 测试和详细报告；任何真实 Docker、Server、服务或 GPU 操作仍未获得授权。

M7-C 离线故障矩阵现已固定在产品提交
`0d861a4c9d5502341cbc78eefc60e30bb32c24fd`，并经用户审阅更新为 `accepted`。
矩阵联合既有与新增 fake/mock 测试覆盖 Worker crash 后接管、watch 断线/full sync、周期 full
sync 失败、Server 和 Docker 暂时不可达、vLLM 异常退出、Docker start 不确定失败、健康超时、
端口冲突、退避期间权威删除、删除期间 Server/Docker 故障、未知容器和 ownership/spec 冲突。
产品修正把 Server、Docker、NVML、本地 store/registry 等基础设施不确定错误保持为本地失败，
不再从 `STARTING` 擅自写 `ERROR`；确定性的 invalid spec、planner、ownership 或 workload
失败仍按既有权威状态链报告。90 上 workspace fmt、Clippy `-D warnings`、212 项 all-target
测试、doc-tests 和 locked/offline build 全部通过，本地目标文件 pre-commit 与
`git diff --check` 通过。下一项只允许执行 M7-D 90 只读预检；它不得停止服务、切换
maintenance、创建 workload、使用 GPU、创建凭据或执行 M7-E。任何真实维护窗口仍须后续
逐项获得明确授权。

M7-D attempt 3 已在治理 HEAD `d6bba026d818370cae3ed0b46e6958ec3af693b0` 通过。固定
Rust binary、`Cargo.lock`、Server v2.2.3、Worker ID 5/UUID、Python Worker恢复对象、Runner
digest、Qwen2.5-0.5B revision/manifest、两张 A100 identity、Managed 端口和 M7 短期凭据准备
条件；`127.0.0.1:18080` Server TCP 代理与独立 Docker Unix socket 只记录边界，没有启动。
49 个 `0600` 证据文件、秘密扫描、manifest 复算以及服务、容器、GPU、端口、进程和 Server
查询的 13 组前后不变式全部通过。证据清单文件 SHA-256 为
`6a2fcd779b51f20c0e31fc62c8c00c22d227912cadcfd57e521c7a8e323957c2`。当前状态为
`ready_for_maintenance_authorization`；下一步只能在新授权下准备 M7-E.1，不能自动执行或把
该授权扩展到 M7-E.2、M7-E.3 或 M7-F。

M7-E.1 维护协议已经用户接受。它把唯一目标限定为：在 GPU0 上运行固定 TP=1 workload，
精确 `SIGKILL` 宿主机 Rust Worker，并证明缺席期间和重启接管后仍是同一健康 vLLM container
ID、`StartedAt`、端口、spec hash、ownership、Model/Instance/File identity、restart count 和
generation 0 日志。正式 PASS 还要求 Docker event 窗口没有 create/start/die/stop/kill/restart/
destroy，缺席期间直连 vLLM 推理成功，恢复后 Server-only CLI 推理/日志和三轮幂等通过。

协议同时冻结 1200 秒独立 watchdog、精确 PID guard、唯一资源 identity、失败回滚顺序、
attempt 专用证据路径、秘密排除、正常 cleanup、独立 post-audit 和用户审阅后的凭据退役门。
五份 attempt 1 脚本已实现并在提交 `d549825c1a0069ee75ea56895eda9d58f0e87551`
通过本机/90 `bash -n`、90 ShellCheck 0.9.0、必需/禁止模式和只读现场隔离门：shared lib、
preflight、executor、独立 session watchdog 和 post-audit 当前固定。状态为 `credential_pending`；
脚本通过不授权准备凭据、停止 Python Worker、切换 maintenance、创建 workload、使用 GPU 或
注入故障。仍须单独授权凭据与只读 preflight/无写注入，再单独请求 M7-E.1 attempt 1 维护授权。

preflight attempt 3 后续通过，formal attempt 1 在明确授权下完成模型运行、Server/直连推理、
精确 SIGKILL 和 Worker缺席连续性，但重启 Worker因 maintenance=false 被启动安全不变量拒绝，
因此固定为 `failed_during_window`，尚未验证同容器接管。Docker stop 的“不返回 exit event”又使
容器稍后变为 stopped，原 fallback 只接受 running 容器，formal/watchdog recovery 均失败。
精确补救和独立审计已恢复同一 Python Worker并清空目标资源、容器、端口和两卡 workload。

retry 2 工具链在 crash 前重新 enable maintenance并再次验证 runtime/推理不变；fallback 按完整
identity同时支持 running 和 stopped 容器，watchdog 模型删除幂等，pre/post 允许宿主机 Rust
Worker停止后的 unreachable maintenance 对象。下一步先完成质量门和全新 preflight/injection
attempt 4；formal/watchdog/post-audit attempt 2 必须使用新路径并重新取得明确维护授权。
