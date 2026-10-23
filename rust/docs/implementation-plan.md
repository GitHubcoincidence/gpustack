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

M6-A 分成五个独立门禁：

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
5. **M6-A.3 真实无 Docker smoke**：另获授权后停止 Python Worker，启动
   `--managed-file-only`，由 CLI 临时解除 Rust Worker maintenance，并把唯一固定 Qwen2.5
   模型精确调度到 Rust GPU0。捕获完整状态序列和 ModelFile READY 后，实例只能停在
   `STARTING`；期间 Docker client/container、inference listener、GPU process 和显存使用
   必须为 0。随后用 CLI 删除测试模型、恢复 maintenance、停止 Rust Worker 并恢复原
   Python Worker，复核 Server 资源集合与全部零残留门禁。

本地路径只允许固定模型根目录内的绝对路径，canonical symlink 不得越界；目录和普通
文件返回 canonical path，分片必须编号连续、声明总数一致且稳定排序，并递归计算字节数。
成功完整写回 `resolved_paths`、`size`、`download_progress=100`、`ready`；失败写
`error` 和脱敏有界消息。M6-A 不支持网络下载、LoRA、draft model、跨 Worker 或多 GPU
runtime。

M6-A.1、M6-A.2 和 M6-A.2.1 都必须分别交付 D033 详细实现报告并获用户接受；当前只
授权 M6-A.2.1 的离线源码与测试。M6-A.3 的真实 Server 写入、maintenance 变化和停止
Python Worker 必须在 M6-A.2.1 获接受后另获明确授权。

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
`ready_for_review`。下一项只允许交付并审阅 D033 详细实现报告；M6-A.3 的真实 Server
写入、maintenance 变化、停止 Python Worker、Docker 或 GPU 操作仍须另行明确授权。
