# GPUStack Rust 重写：已接受决策

除明确标为 pending 的项目外，下列决策均已接受。架构变更必须先讨论，再新增带日期的修订记录；不得为了迁就实现困难而静默修改。

## 2026-10-17 设计冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D001 | 第一阶段重写边界 | 先重写 Worker，保留现有 Server | 提供可测试的兼容边界，避免同时重写控制面。 |
| D002 | R1 后端 | 只支持 vLLM | 先打通一条完整路径，再增加后端变量。 |
| D003 | R1 runtime | Linux/NVIDIA 上的本地 Docker | 匹配验证主机，并把 Kubernetes、Podman 与其他厂商排除在第一次验证之外。 |
| D004 | 分布式推理 | 只支持同一 Worker 的单/双 GPU | 在不引入 Ray 或跨 Worker 协调的情况下验证 tensor parallelism。 |
| D005 | Runner | 复用现有 `gpustack/runner` 镜像 | vLLM 仍是 Python/CUDA 软件；重建镜像载荷与 Worker 控制逻辑无关。 |
| D006 | Runner 选择 | 模型显式镜像，其次配置默认值，否则报错 | 避免隐藏 catalog 选择与静默版本不匹配。 |
| D007 | Runtime 包 | 原生 Rust 子集，不依赖 Python subprocess、PyO3 或 FFI | 在限制 R1 runtime 范围的同时，产出可独立部署的 Worker。 |
| D008 | 模型来源 | 只支持本地模型目录 | 将 Worker 执行与下载、缓存复杂度分开。 |
| D009 | 开发拓扑 | 先以宿主机进程 bring-up，R1 强制使用 Worker 容器 | 简化早期调试，但不降低生产部署门禁。 |
| D010 | 生产拓扑 | Worker 容器通过宿主机 Docker Engine 管理同级 vLLM 容器 | 符合预期 GPUStack 部署模型，并支持独立于进程的工作负载恢复。 |
| D011 | 网络路径 | R1 中 Server/gateway 直接访问 vLLM | 在核心生命周期打通前，不实现 Worker proxy 与 tunnel。 |
| D012 | 源码管理 | 可写 fork `GitHubcoincidence/gpustack`；上游 `gpustack/gpustack`；分支 `codex/rust-worker` | 保持历史可审计，并把项目工作与 upstream main 分离。 |
| D013 | 开发主机 | 90 是主要 Linux/GPU 工作树与验证主机，但不是唯一源码副本 | 硬件相关工作靠近目标环境，同时让 Git 保持权威。 |
| D014 | Git 认证 | 根据用户明确选择，在 90 上使用账号级 SSH key | 使用用户 GitHub 账号的权限；通过专用 key、文件权限、脱敏与后续撤销来控制较宽权限风险。 |
| D015 | 现有部署 | 保留 Server；只在批准的 GPU 测试窗口 drain 并停止 Python Worker | 防止 GPU 重复计算，并支持立即回滚。 |
| D016 | 兼容基线 | 固定 M1 发现的实际 Server 部署，R1 期间不升级 | 让协议证据可归因、可复现。 |
| D017 | CLI 产品原则 | CLI-first；每项已支持的用户/运维能力都具有稳定 CLI | 使重写结果可脚本化，并杜绝只能从 Web 操作的流程。 |
| D018 | 二进制命名 | 迁移阶段为 `gpustack-rs`，最终为 `gpustack` | 避免与已安装 Python 命令冲突，同时保留最终公开接口。 |
| D019 | 运行模式 | Managed 与 Lab adapter 复用同一 reconciler/runtime | 支持早期 CLI 演示，同时不创建第二套生产控制面。 |
| D020 | Managed 部署 | Model CLI 调用 Server；Server 仍是期望状态权威来源 | 保留调度、授权、数据库状态、gateway 行为与恢复语义。 |
| D021 | Lab 证据 | Lab 仅用于隔离诊断/冒烟，绝不能作为正式 Managed 证据 | 防止把本地容器成功运行误报为 Server 兼容。 |
| D022 | Worker API | 只实现 R1 probe、文件检查、模型日志、配置读取、token 验证与 metrics | 支持本地路径部署和 CLI 日志，而不引入 proxy、K8s、benchmark、cache 或 GGUF 范围。 |
| D023 | 协调 | 事件加启动/重连/周期性权威 full sync | 从事件丢失中恢复，并防止基于临时 cache 做破坏性决策。 |
| D024 | 所有权 | 每次变更前用完整 mode/Worker/instance/spec label 校验 | 保护未知、Python 所有、Lab 以及其他 Worker 的容器。 |
| D025 | 退出行为 | 不删除健康的 Managed 模型容器 | 允许 Worker 进程/容器重启，并在不中断模型的情况下重新接管。 |
| D026 | Workspace | 四个 crate：CLI、client、Worker 与 runtime | 这些边界由 CLI-first 范围所需的依赖和职责支持。 |
| D027 | 配置 | CLI > environment > YAML > defaults；secret 只记录文件引用 | 支持自动化，同时避免 secret 值进入配置历史。 |
| D028 | 性能 | R1 必须测量性能，但通过/失败门禁是兼容性和可靠性 | 在建立受控基线前，不作缺乏证据的性能优势声明。 |
| D029 | 治理文档语言 | `rust/docs/` 中文为权威版本；技术标识保留英文；对外 MkDocs 另行编写英文版 | 便于当前协作与长期实验记录阅读，同时遵守顶层 `docs/` 的 English-only 约束并避免双语副本漂移。 |
| D030 | 日常开发工作流 | 90 主机工作树优先直接修改、验证、提交并推送；本地仓库作为备用与必要审阅副本 | 避免每次在本地和 90 间手工同步，并让构建与部署验证靠近目标环境。 |

## 2026-10-18 M1 已确认身份信息

已部署兼容基线固定为 GPUStack `v2.2.3`、commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` 和 image digest `sha256:55ecc762950a32641b6a4d96195c0662ef72998eaeeda31bba0e8e218d41afef`。R1 首选 Runner 固定为 `quay.io/gpustack/runner:cuda13.0-vllm0.25.1` 的 digest `sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af`。首选验证模型目录为 `/home/wujiaxing2/flowscale-system-validation/models/Qwen-7B`。完整环境、端口、网络与模型 identity 见 `environment-baseline.md`。

这些 identity 的确认不等于 Runner/模型或 Rust Worker 已通过。实际加载、单/双 GPU、Managed 协议、容器化 Worker 与回滚仍按后续门禁验证。

## 2026-10-18 M2 工具链固定

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D031 | Rust toolchain | R1 开发固定 Rust `1.99.1-x86_64-unknown-linux-gnu`，使用 rustup minimal profile，并安装 `rustfmt` 与 `clippy` | 使用已验证的明确版本，避免开发主机、后续容器与 CI 因浮动 stable 产生不可归因差异；仓库级 pin 在 M2 workspace 检查点提交。 |

该决策只固定开发工具链，不改变 R1 scope、Server 兼容基线、Runner、模型或部署拓扑。

## 2026-10-19 实现可解释性与阶段汇报门禁

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D032 | 代码注释硬门禁 | 最初采用 `missing_docs = "deny"` 并要求全部公开项具有 Rustdoc；该方案因产生过量模板化注释被 D034 修订 | 保留决策演进记录；该方案不再作为当前门禁。 |
| D033 | 阶段详细实现报告 | 每个实现阶段在请求验收前，必须由执行者直接在聊天中说明功能、实现机制、数据流、模块、依赖取舍、证据、安全边界、限制、未实现项和下一步；长期文档只保存检查点摘要，不能代替直接报告 | 让用户能在不了解代码细节的情况下准确判断每一阶段实际完成了什么，并避免只看到“测试通过”却无法审阅实现内容。 |

这两项是所有后续阶段的通用质量门禁，也追溯应用于尚未接受的 M3-B。它们不改变 R1 产品范围、Server 协议或部署拓扑。

## 2026-10-19 注释密度修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D034 | 关键必要注释 | 取消 `missing_docs = "deny"` 和逐项公开 API 文档要求；只对安全边界、状态转移、协议偏差、恢复逻辑、所有权检查、关键数据流和非直观算法要求英文注释；缺少关键注释与注释过量均视为门禁失败 | 用户要求的是帮助理解和维护的关键注释，而不是覆盖每个字段的模板化说明；注释过多会遮挡代码并增加同步成本。 |

D034 只修订 D032 的注释密度和检查方式。D033 的阶段详细实现报告要求保持不变，R1 产品范围、协议、依赖与运行行为均未改变。

## 2026-10-20 Server 权威 Worker 身份纠偏

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D035 | Worker 注册身份权威与恢复 | 首次注册以后以 Server 返回的 Worker `id`、`name` 和 `worker_uuid` 为权威，并将三者写入 `<data_dir>/managed/worker-identity.json`；身份元数据必须先于 Worker token 持久化。重启时由 token 调用 `me` 核验 `id` 和 `name`，由受保护的本地元数据恢复 `worker_uuid`。若只有元数据而没有 token，则使用注册凭据和已持久化的 Server UUID 重注册同一 Worker；若只有 token 而没有元数据，则拒绝启动。 | 固定 v2.2.3 的注册路由会忽略客户端首次提交的 `worker_uuid` 并生成 Server UUID，注册响应包含该 UUID，但 `me` 响应不包含。先写元数据使注册后崩溃能够恢复同一 Worker；token 无元数据时无法安全确定 UUID，必须 fail closed。 |

M3-A 中“注册请求 UUID 与响应 UUID 相同”的 synthetic fixture 语义，以及 M3-C 中依赖该等值关系的身份断言，自本决策起标记为 `invalidated`，不得继续作为兼容性证据。M3-A 的其余协议形状、M3-B client 与 M3-C 控制循环结论保持有效。该修订不授权真实 Server 注册或其他写请求。

## 2026-10-20 Managed 状态机与 Lab 边界纠偏

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D036 | Managed ModelInstance 状态机 | R1 完整保留 `PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING -> RUNNING`，并按固定 v2.2.3 的 Scheduler、Server Controller 与 Worker 职责推进；`ERROR`、`UNREACHABLE` 保留原旁路语义 | 状态词汇相同但跳过控制步骤不等于协议兼容；状态所有权会直接影响容器启动、恢复与 Server 可见性。 |
| D037 | 本地模型与 ModelFile | `local_path` 仍通过 ModelFile 控制链；Worker 不进行网络下载，但负责安全校验、分片解析并上报完整 ModelFile `ready/error` 结果，Server 据此推进 ModelInstance 的 `downloading/starting` | ModelFile 是原 Server 协调 ModelInstance 的必要资源，而不只是远程模型下载器。 |
| D038 | Lab 状态 | Lab 不创建 ModelInstance/ModelFile，也不写 Server 状态；使用本地 `CREATED/PREPARING/STARTING/RUNNING/FAILED/STOPPED`，与 Managed 复用模型路径解析、planner、Docker runtime、日志和健康检查 | 允许 CLI 独立演示真实执行能力，同时不伪造 Managed 协议证据或引入第二套生产控制面。 |
| D039 | 里程碑顺序 | 在 M3-D 前加入 M3-C.2，离线补齐 ModelFile schema/fixture/client/list/watch/update 与控制缓存；M6 拆为文件准备状态协调和 `STARTING` 后 runtime 两部分 | 第一次真实控制面集成应覆盖 Worker 所需的三类资源，并在启动容器前固定状态所有权门禁。 |

D036–D039 修订了最初设计中“本地模型不经过下载阶段”和只同步 Model/ModelInstance 的假设。修订时 Rust Worker 尚未实现 Docker workload、ModelFile manager 或 Managed reconciler，因此没有需要作废的运行时或 GPU 证据；M3-C 已接受的身份、heartbeat/status、Model/ModelInstance watch/full-sync 结论继续有效。M3-D 的真实写路径仍需单独批准。

## 2026-10-20 M3-D 隔离真实控制面设计冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D040 | M3-D 运行模式 | 新增显式 `gpustack-rs worker run --control-plane-only`；只启动身份、探针、heartbeat/status 和三类控制资源同步，不初始化 GPU、Docker、vLLM、ModelFile 准备或 ModelInstance reconciler | 让第一次真实集成只验证控制面协议，不能因默认初始化路径意外执行工作负载。 |
| D041 | 不可调度双重门禁 | 注册时提交 `maintenance.enabled=true`，并上报空 `gpu_devices`；maintenance 是主要调度隔离，空 GPU 是独立防御 | 固定 v2.2.3 调度器只接受 `READY` Worker；只依赖空 GPU 不能覆盖未来 CPU/offload 等候选路径。 |
| D042 | 最小 Worker HTTP 能力 | 从 M4 提前实现独立端口上的 `GET /healthz`，只表示 Rust 进程存活；M3-D 不提供 readiness、metrics、文件、日志或推理 API | Server 必须能够探测真实 Rust Worker，不能借用 Python Worker 端口，也不能把存活误报为完整工作就绪。 |
| D043 | 隔离身份与目录 | 名称 `rust-m3d-r760xa`、IP `10.213.80.90`、网卡 `eno8403`、health 端口 `20150`、无 metrics 端口、数据目录 `/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d`、注册令牌文件 `/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token` | 与现有 Python Worker 的 `10150/10151`、身份和数据完全隔离，并给重启恢复提供固定位置。 |
| D044 | 凭据策略 | 优先使用专用 cluster registration token；若创建不便，可在另获明确批准后把现有 Python Worker 的 `GPUSTACK_TOKEN` 直接写入权限为 `0600` 的文件，整个过程不得在终端、日志、Git 或证据中回显 | 保持凭据来源可撤销、可审计，同时为当前部署保留安全的实际执行路径。 |
| D045 | 阶段与外部写入门禁 | M3-D.1 只做代码、fixture/mock 测试和实现报告；M3-D.2 才读取获批凭据并对真实 Server 注册、heartbeat/status。接受本设计不授权 M3-D.2 | 把实现审阅与真实系统变更分开，使用户能在任何外部写入前检查完整行为。 |
| D046 | 验收后身份处理 | 验收后停止 Rust 进程，但保留 maintenance Worker identity 和本地状态供 M4 复用；完整删除只在最终清理、身份失效或用户针对精确 Worker ID 另行批准时执行 | Worker identity 本应跨重启持久；保留不可调度身份既能验证恢复，也避免各阶段重复注册和危险级联清理。 |

M3-D.2 的第一次运行至少持续 6 分钟，以覆盖多轮 heartbeat/status、watch 保活和一次五分钟周期 full sync；正常停止后使用相同数据目录再次运行约 2 分钟，证明恢复同一 Server 身份且不产生重复 Worker。该阶段不要求 drain 或停止 Python Worker，因为 Rust Worker 处于 maintenance、上报空 GPU 且没有 runtime 路径；任何后续 GPU、Docker 或 vLLM 测试仍必须使用单独维护窗口。

## 2026-10-21 M3-D 凭据生命周期修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D047 | M3-D 注册凭据语义与生命周期 | 固定 v2.2.3 不预签发 per-Worker registration token；首次注册使用现有默认 Cluster 的共享 registration token，由用户通过隐藏输入写入 M3-D 专用 `0600` 文件。首次运行确认 Worker identity 和 per-Worker token 完整落盘后删除该 Cluster token 本地副本；不使用 admin API key、不读取 Python Worker 环境，也不额外创建 Cluster | “专用”只指隔离文件而不是新的注册凭据。共享 Cluster token 能创建 Worker，必须缩短落盘时间；Server 返回的 per-Worker token 才是后续恢复所需的最小权限身份。 |
| D048 | 完整身份恢复不依赖 Cluster token | M3-D.1.1 把 supervisor 的 registration token 读取改为可选：完整 identity 加 Worker token 可以独立恢复；首次注册或 metadata-only 恢复仍要求 Cluster token，缺失时 fail closed。完成离线门禁和详细报告前不得进入 M3-D.2 | 身份状态机本已区分注册凭据和 Worker 凭据，外层强制每次读取 Cluster token 会扩大共享 secret 驻留时间，也无法证明真正的持久身份恢复。 |

D047 修订 D044 中“专用 cluster registration token”和读取 Python Worker `GPUSTACK_TOKEN` 的备选表述：固定 v2.2.3 的 Cluster token 是共享 bootstrap secret，本阶段只为它创建专用文件；不再把读取现有 Python Worker 环境作为 M3-D.2 的正常或备选路径。

修订后的 M3-D.2 在首次正常停止并确认两份 Worker 身份文件后删除本地 Cluster token 副本，再使用相同数据目录运行约 2 分钟，证明只靠 per-Worker token 恢复同一 Server 身份且不产生重复 Worker。其余运行时长、隔离和维护窗口边界不变。

## 2026-10-21 M3-D.2 真实集成运行手册冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D049 | 操作责任与分段门禁 | M3-D.2 的全部命令由用户在 90 主机亲自执行；助手逐段提供命令与目的说明，只审阅用户返回的非敏感输出。每一段必须验收后才提供下一段，registration token 和 Worker token 永不发送到聊天 | 让真实外部写入保持在用户直接控制下，同时避免凭据进入对话记录或因一次性交付整套命令而绕过中间门禁。 |
| D050 | 证据目录与 Server 可达性 | 正式证据写入 Git 外固定 `0700` 目录 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m3d2-formal`；证据不得保存 token 或 Authorization header。Server 到 Rust health 的方向使用一次 `docker exec gpustack` 内的只读 HTTP probe 验证 | 将运行证据与源码提交分离，并直接验证 Server 容器实际使用的网络方向；临时 probe 不改变容器或工作负载状态。 |
| D051 | 首次注册凭据失败处置 | 首次注册尝试一旦开始，无论成功或失败，都删除精确路径上的本地 Cluster token 副本；需要重试时由用户重新隐藏输入。失败时不自动删除身份文件、Server Worker row 或其他状态 | 缩短共享 bootstrap secret 的驻留时间，并避免在结果不明时用级联清理破坏可审计现场或误删已签发身份。 |
| D052 | 正式运行时序 | 正式二进制必须从冻结后干净 HEAD 重新构建并记录 commit 与 SHA-256。首次运行用 SIGINT 超时边界持续 370 秒，在约 60 秒和 330 秒观测；第二次在确认身份完整且已删除 Cluster token 副本后持续 130 秒。正常结束保留 maintenance Worker 和本地身份 | 370 秒覆盖多轮 30 秒 heartbeat/status 和一次五分钟 full sync；第二次明确验证只靠 per-Worker token 恢复同一 ID/UUID。固定构建 identity 使证据可归因。 |

冻结参数继续使用 Worker `rust-m3d-r760xa`、Server `http://127.0.0.1`、`r760xa`、`10.213.80.90`、`eno8403`、端口 `20150`、D043 的 token/data 路径和无 metrics 配置。现有 Python Worker 在本阶段保持运行；control-plane-only、maintenance 和空 GPU 三项约束意味着无需为此次控制面验证安排 GPU 维护窗口。

## 2026-10-21 M3-D.2 执行授权与失败证据修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D053 | M3-D.2 操作者修订 | 用户在预检、构建、凭据准备门禁完成后明确授权助手代为执行剩余正式命令；该授权仅修订 D049 的操作者，不改变二进制、身份、时序、观测、秘密处理、失败停止和验收条件 | 保留用户对真实外部写入的显式授权，同时使助手可以完成长时间观测和证据归档；授权没有扩展到 Docker workload、GPU、vLLM 或 Python Worker 操作。 |
| D054 | CLI 参数层级错误的处置 | 第一次调用因把 Worker 配置参数放在顶层解析器而以退出码 2 结束；该目录保留为 `invalidated`，不覆盖、不计入正式结果。确认没有 Server 写入或本地身份后，通过官方登录端点重新获取 Cluster token，在 `validate-config` 验证 `gpustack-rs [global] worker [worker options] run --control-plane-only` 后使用新证据目录重试 | 区分运行手册命令错误与 Worker 实现失败，保留失败链，并遵守 D051 在首次尝试退出时删除 Cluster token 的要求。 |

D053 只覆盖本次 M3-D.2 剩余执行；后续真实部署、维护窗口和破坏性操作仍需按各自门禁单独授权。D054 没有改变 370/130 秒时序或验收口径，旧 `first-run/` 证据不得作为成功证据，正式首次结果位于 `first-run-retry-1/`。

## 2026-10-21 M4 资源盘点与 Worker API 设计冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D055 | M4 运行模式 | 新增显式 `worker run --resource-api-only`；启用真实系统/NVML、M4 Worker API 和 metrics，继续 maintenance，并禁止 Docker、ModelFile/ModelInstance reconciler 与 vLLM；M3-D control-plane-only 语义保持不变 | 把资源/API 兼容证据与模型 runtime 风险分开，并避免已有安全模式随实现扩展而漂移。 |
| D056 | 固定端口与模型根目录 | Worker API 使用 `10.213.80.90:20150`，metrics 使用 `10.213.80.90:20151`，允许模型根目录为 `/home/wujiaxing2/flowscale-system-validation/models`；Python Worker 继续使用 `10150/10151` | 已只读确认候选端口空闲，并与现有服务隔离；固定根目录使文件 API 可以执行严格 containment。 |
| D057 | Worker API 认证与安全增强 | 注册完成后的 API 只接受持久化 per-Worker token，使用 constant-time 比较和通用 401；不接受 Cluster registration token。文件路径必须为绝对路径并 canonicalize 到允许根目录内，配置和日志只暴露受控白名单 | Cluster token 是共享 bootstrap secret，已按生命周期删除；Python v2.2.3 缺少 base-path containment，Rust 有意收紧而不影响合法路径。 |
| D058 | Readiness 兼容偏差 | resource-api-only 的 `/readyz` 固定返回 503；未来完整 Managed 只有 runtime 先决条件和首次权威收敛后才返回 200。`/healthz` 继续用于 Server reachability | 固定 Python v2.2.3 的 readyz 常量 200 不能表达 M4 尚无 Docker/reconciler；有意义的 readiness 能防止把资源/API 测试误报为完整 Worker。 |
| D059 | M4 分段验收与维护窗口 | M4-A 冻结契约/fixture/依赖，M4-B 离线实现，M4-C 宿主机只读 NVML smoke，M4-D 停止 Python Worker 后做正式 Server 集成并恢复；只有 M4-D 通过才接受 M4 | 只读采集可与 Python Worker 共存，但两个 Worker 同时向 Server 声明相同物理 GPU 会造成资源重复；正式上报必须使用维护窗口。 |
| D060 | M4 依赖策略 | 依赖类别冻结为 `axum`、`nvml-wrapper`、`sysinfo`、`prometheus-client` 和 `subtle`；准确版本只有在 Cargo cache、Rust 1.99.1/MSRV 和 locked/offline build 审计通过后才能写入锁文件 | 先固定职责而不猜测版本，避免在 90 网络受限环境引入不可离线复现或不兼容工具链的依赖。 |

M4-D 的维护窗口必须另获用户对精确 stop/start 命令的明确授权。接受这些决策只允许进入 M4-A；不授权 M4-B 代码实现、M4-C 真实 NVML 读取、停止 Python Worker、启动 resource-api-only 或任何 Docker/vLLM/GPU 工作负载。

## 2026-10-21 M4-A 精确契约与依赖版本冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D061 | Worker API 精确兼容边界 | 固定 route、query、响应 schema、日志文件规则、配置白名单和 Python 资源 metric 名称；接受 Bearer 与 `X-API-Key` 两种传输但只校验 per-Worker token。相对路径为 400，根目录或 symlink 越界为 403，安全根目录内不存在为 404，`file-exists` 的安全未存在目标为 200/false；model config 上限固定 16 MiB。容器日志在 resource-api-only 返回 501，PUT config 返回 405，其他延后 route 返回 404 | 把兼容性、安全增强和不支持边界变成可测试契约，避免 M4-B 在实现时从模糊设计自行选择行为。 |
| D062 | M4 依赖准确版本 | M4-B 输入固定为 `axum 0.8.9`（`http1,json,query,tokio`）、`nvml-wrapper 0.13.0`、`sysinfo 0.39.6`（`disk,system`）、`prometheus-client 0.25.1` 和 `subtle 2.6.1`，不启用未列出的 default feature。版本已在独立 scratch crate 中用 Rust 1.99.1 完成 online locked check 和断网 locked/offline check；M4-B 仍须在实际 workspace 更新 lock 后重复全部门禁 | 证明准确版本在 90 的 cache、固定工具链和离线模式可用，同时不让一次 scratch 结果替代真实 workspace 依赖集成。 |
| D063 | 资源和 readiness 失败语义 | 资源 rate 固定为 `0..=100` 百分比；GPU 与文件系统改为固定 v2.2.3 类型，GPU memory 使用 bytes。NVML 初始化、双 A100 枚举或关键 identity 失败时 resource-api-only fail closed；`/healthz` 仍可表示存活，但 `/readyz` 和 `gpustack:rust_worker_ready` 固定不可用/0 | 空 GPU 上报会把资源采集失败伪装成成功，且与 M3-D 的 control-plane-only 隔离语义混淆；明确单位可避免 dashboard 和 Server 资源值出现百倍偏差。 |

D061–D063 已随 M4-A 获用户接受。该接受允许进入 M4-B 的 mock/fake 驱动离线实现，但不授权读取真实 Worker token、启动 resource-api-only、访问真实 NVML、更新 Server、停止 Python Worker 或执行 Docker/vLLM/GPU 工作负载。

## 2026-10-21 M5-A Docker runtime 与 Lab 契约冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D064 | 共享部署契约 | `gpustack-worker` 生成版本化 `WorkloadSpecV1`，Lab 与 Managed 共用 planner、reconciler、Docker runtime、健康、日志和清理；只有期望状态 adapter 与状态回写目标不同 | 让 Lab 验证的执行实现可以直接进入 M6，而不形成第二套 Worker 或生产控制面。 |
| D065 | Docker 实现边界 | Rust 通过 Unix socket 原生调用 Docker Engine API，不执行 `docker` CLI，也不依赖 Python `gpustack-runtime`；M5 只实现单 vLLM container 所需子集 | 保持独立 Rust Worker，同时避免复制第三方 runtime 的 Kubernetes、Podman、pause container 和多组件抽象。 |
| D066 | Workload identity | normalized spec 使用规范化 JSON 的 SHA-256；所有变更前必须重新匹配 implementation、schema、mode、local owner、deployment 与 spec hash。Lab owner 独立原子持久化，不依赖 Server identity 或 token | 使幂等接管和危险操作有不可混淆的身份边界，并保护 Python、Managed 和未知容器。 |
| D067 | M5 Runner 安全基线 | 正式 smoke 使用 M1 固定 Runner digest、`pull=never`、host network、10 GiB shared memory、NVIDIA DeviceRequest、只读模型 mount、独立可写 cache、restart policy `no`、`no-new-privileges` 且默认非 privileged | 对齐固定 Runner 的必要运行方式，同时坚持最小权限；若 smoke 证明需要放宽，必须先讨论并记录修订。 |
| D068 | Lab 网络与端口 | Lab vLLM 只绑定 `127.0.0.1`，自动端口范围为 `42000-42063`；Managed 后续绑定 Worker IP 并使用 Server 分配端口 | Lab 只由本机 CLI 验证，不应暴露到局域网；独立端口范围降低与 Python/Managed 分配冲突。 |
| D069 | Lab 生命周期与幂等性 | 状态保持 `CREATED/PREPARING/STARTING/RUNNING/FAILED/STOPPED`；只有 `/v1/models` 返回期望模型才进入 RUNNING。同名同 spec 可返回、重启或重建，同名异 spec 必须冲突；失败保留现场，stop 不删除，cleanup 默认 dry-run | Docker running 不是模型可用证据；保留现场和拒绝隐式替换使诊断、恢复和清理可审计。 |
| D070 | M5 单 GPU 约束 | M5 只允许一张 GPU 和 tensor parallel size 1；CLI index 必须解析为稳定 UUID，活跃或不能可靠确认空闲的 GPU 拒绝使用，不提供强制抢占 | 先验证最小完整 vLLM 路径，并防止与现有 Worker 或其他任务争用物理 GPU。 |
| D071 | M5 分段门禁 | M5-A 冻结契约；M5-B 离线 runtime；M5-C 离线 Lab CLI/reconciler；M5-D 获批后做无 GPU Docker 集成；M5-E 单独维护窗口完成 Qwen-7B 单 GPU Lab smoke 并恢复 Python Worker | 把代码正确性、Docker 变更和 GPU 风险分离，任何阶段失败都能停在明确边界。 |

M5-A 接受只允许进入 M5-B 的离线代码实现和 mock/fake 测试。准确 Docker Rust client
版本必须先通过 Cargo cache、Rust 1.99.1/MSRV、locked/offline 与依赖范围审计后才能
写入 workspace。M5-A 不授权连接真实 Docker socket、创建或删除 container、停止
Python Worker、加载 Runner、访问 GPU 或运行模型；M5-D 和 M5-E 分别需要新的明确
授权。

## 2026-10-22 M5-B 精确依赖与离线实现决策

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D072 | Docker client 与 hash 依赖 | 固定 `bollard 0.21.1`，关闭 default features，仅启用本地 Unix socket 所需的 `pipe`；固定 `sha2 0.10.9` 生成 normalized spec SHA-256。两个版本均写入 workspace `Cargo.lock` | `bollard` 直接提供类型化 Docker Engine API，避免执行 `docker` CLI；最小 feature 集不引入 TLS、SSH、BuildKit 或 websocket。`sha2` 只承担本地规范化身份计算，不改变网络或运行时权限。 |

D072 已在独立 scratch crate 和实际 workspace 中通过固定 Rust 1.99.1 的 locked/offline
检查，并在 M5-B 详细实现报告后获用户接受。该决策只覆盖 M5-B 源码和离线测试，不授权
连接真实 Docker socket、创建或删除 container、停止 Python Worker、访问 GPU 或运行
模型；接受结果只解锁 M5-C 离线实现。

## 2026-10-22 M5-D 无 GPU Docker 集成设计冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D073 | M5-D 分段与工具边界 | M5-D.0 只离线实现 `worker runtime info`、共享完整 label 校验函数和不随正式 CLI 发布的 Rust example probe；M5-D.1 另获授权后只读连接 Docker 并完成 daemon/image/service/GPU 预检；M5-D.2 再获授权后才创建诊断容器 | 把产品需要的只读 CLI、可审计验证工具、外部读取和 Docker 变更分开；不向公共 CLI 暴露任意 create/remove 原语。 |
| D074 | 无 GPU 诊断容器契约 | 使用 M1 固定且已缓存的 Runner digest，禁止 pull、GPU request、mount、端口和网络；容器非 privileged、启用 `no-new-privileges`、restart `no`、auto-remove false，只执行输出 readiness 标记并最多 sleep 120 秒的 `/bin/sh` 命令 | 在复用真实固定镜像和真实 Docker API 的同时排除模型、GPU、网络和宿主机文件变量；有界命令在验证进程异常时也会自行停止。 |
| D075 | 所有权、失败与证据 | 每个 run 使用唯一 name、owner、run-id 和 spec hash；create/start/inspect/log/stop/remove 全部走 Rust runtime。错误 owner 必须由共享 label 校验拒绝，现有 Python 容器只做只读拒绝验证。失败时仅在完整 tuple 匹配时清理，不使用名称或 `docker rm -f` 正常路径；正式证据写入 Git 外 0700/0600 目录并要求最终零残留和 Python 服务前中后不变 | 让真实变更仍受与生产 workload 相同的失败关闭原则约束，并使成功、失败和清理证据可归因且不泄露凭据。 |
| D076 | M5-D.1 只读预检门禁 | 使用现有 `worker runtime info` 验证 Rust 到 Docker 的 ping/version 路径，并使用精确格式化的 Docker 只读查询验证固定 Runner、受保护容器和历史残留；前后复核服务、GPU、进程与端口。禁止 probe、pull、exec、日志、完整 inspect 和任何容器变更，Python Worker 全程运行 | 在 M5-D.2 变更前验证真实环境，同时避免扩大公共 CLI 或把只读诊断提升为容器、vLLM、模型和 GPU 兼容证据。 |

用户接受 D073–D075 只授权 M5-D.0 的离线源码、fake/mock 测试和 Rust 门禁，不授权
连接真实 Docker socket、运行 `worker runtime info`、inspect 真实镜像、创建诊断容器或
读取 GPU。M5-D.1 和 M5-D.2 必须分别获得新的明确授权；M5-E 维护窗口边界不变。

M5-D.0 接受后，用户另行接受 D076 并明确授权执行 M5-D.1 的只读 Docker、容器元数据和
GPU 预检。该授权不包含 `m5d_docker_probe --execute`、镜像 pull、容器 create/start/
stop/restart/remove、`docker exec`、服务变更、模型加载或推理；M5-D.2 仍须单独授权。
