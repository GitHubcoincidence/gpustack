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
| D077 | 共享 GPU 主机隔离修订 | M5-D.1/D.2 继续不申请任何 GPU；预检只要求未来目标 GPU0 `GPU-52e4ed55-...` 无计算进程并保持空闲，允许 GPU1 上已识别的外部任务继续运行。GPU1 的 UUID、进程 identity 和资源快照须前后保持可归因，本项目不得信号、停止或修改它。M5-E 只允许按 GPU0 精确 UUID 分配，仍需独立维护授权和 Python Worker drain | 把隔离边界收紧到稳定 GPU UUID，使共享主机上的无 GPU验证不被无关 GPU1 任务永久阻断，同时防止索引漂移、`all` GPU 分配或干扰其他用户。 |

用户接受 D073–D075 只授权 M5-D.0 的离线源码、fake/mock 测试和 Rust 门禁，不授权
连接真实 Docker socket、运行 `worker runtime info`、inspect 真实镜像、创建诊断容器或
读取 GPU。M5-D.1 和 M5-D.2 必须分别获得新的明确授权；M5-E 维护窗口边界不变。

M5-D.0 接受后，用户另行接受 D076 并明确授权执行 M5-D.1 的只读 Docker、容器元数据和
GPU 预检。该授权不包含 `m5d_docker_probe --execute`、镜像 pull、容器 create/start/
stop/restart/remove、`docker exec`、服务变更、模型加载或推理；M5-D.2 仍须单独授权。

首次 M5-D.1 因 GPU1 上已识别的外部 vLLM 任务在原“全卡空闲”门禁停止后，用户接受
D077，明确把未来目标固定为 GPU0，并允许 GPU1 外部任务保持运行时重试 M5-D.1。
该修订不授权当前无 GPU阶段使用 GPU，也不授权 M5-D.2 或 M5-E。

## 2026-10-22 M5-E 单 GPU Lab smoke 设计冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D078 | M5-E 分段门禁 | M5-E.0 在 Python Worker 运行时只读验证源码/二进制、模型 identity、固定 Runner、Server 资源、服务、GPU0、GPU1 外部 principal、端口和历史残留；M5-E.1 另获明确维护授权后才停止 Python Worker、创建 vLLM container、挂载模型和申请 GPU0 | 把会改变服务和 GPU 状态的正式 smoke 与可提前发现阻断条件的只读检查分离，避免在维护窗口内临时修订输入。 |
| D079 | 固定 smoke 输入 | 使用 `/home/wujiaxing2/flowscale-system-validation/models/Qwen-7B`、固定 Runner digest、GPU0 完整 UUID、`127.0.0.1:42000`、served name `qwen-7b-m5e`、tensor parallel 1、maximum model length 2048、GPU memory utilization 0.5、dtype auto、trust remote code 和 eager execution；模型只读、独立 cache 可写，禁止 pull、GPU1、privileged 和 force remove | 固定最小单卡配置和可复现实验 identity，降低显存、图编译与网络变量，同时仍验证真实 Qwen-7B/vLLM 路径。 |
| D080 | 正式 CLI 与验收序列 | M5-E.1 依次验证 deploy/list/status/logs、`/v1/models` readiness、同 spec 幂等、异 spec 冲突、completion 与 chat 非空响应、stop/remove、cleanup dry-run/execute；前后 Server Model/ModelInstance/ModelFile 不增加，最终恢复 Python Worker 且无 Rust container、端口、cache、进程或 GPU0 残留 | 让 smoke 覆盖用户可观察的完整 Lab CLI，而不把单次容器启动误当成 M5 完成。 |
| D081 | 失败关闭与回滚 | 正常和失败清理都只操作可重新证明完整 ownership tuple 的资源，不在同一正式 run 中换镜像、模型、GPU、放宽权限或使用强制删除。独立 watchdog 负责有界回滚；若所有权无法证明或 GPU0 残留不可归因，则停止自动处置且不让 Python Worker 与未知 GPU workload 并行，保留 Server 并请求人工决策 | 在优先恢复现有 Worker 的同时，避免为了自动回滚而删除未知资源或制造 GPU 争用。 |

用户接受 D078–D081 并授权执行 M5-E.0。该授权允许只读连接现有 Server、Docker 与
NVML，并允许在 Git 外建立受保护的配置和证据目录；不授权停止或重启 Python Worker、
创建/启动/停止/删除 container、挂载模型、申请 GPU、运行 vLLM 或推理。M5-E.1 必须在
M5-E.0 通过并交付审阅后获得新的明确维护窗口授权。

## 2026-10-22 M5-E Qwen ChatML 前置条件修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D082 | Qwen ChatML 内容、identity 与传递 | 以固定模型 `qwen_generation_utils.py::make_context` 为语义来源，冻结 `rust/fixtures/m5e/qwen-7b-chatml.jinja`：无显式 system 消息时仍生成空 system 块，随后逐条生成 system/user/assistant ChatML 块，`add_generation_prompt` 时追加 assistant 起始块。fixture 文件 SHA-256 为 `0d06c62751b46ff03bde0e08ae4d351f056e95885b3c258382492686965af15d`；删除唯一结尾 LF 后的有效模板为 381 bytes，SHA-256 为 `74c5628e34e1d05c43a639966d7f5861d01cfd89ebafa39b3976f4d6c12dfd14`。M5-E.1 只能以一个 `--vllm-extra-arg=--chat-template=<template>` CLI 参数传入，Rust 将其保存为结构化 flag/value 并作为两个独立 container argv 元素传给 vLLM；禁止 `eval`、临时修改模型目录或新增模板 mount | 固定模型的 tokenizer 没有 template，而 vLLM 0.25.1 在这种情况下要求显式提供。内联单行模板是 vLLM 的正式输入形式，既不改变只读模型目录和 mount 契约，也避免 shell 在容器内再次解析 Jinja 内容；hash、渲染样例和 argv 回归测试使正式 smoke 输入可复现。 |

D082 是对 D079 固定 smoke 输入和 D080 chat 验收的有意补充，不改变模型、Runner、GPU0、
端口、显存参数、CLI 序列或回滚边界。该 fixture 只声明 string content 与
system/user/assistant 消息，不声明 tool calling、多模态消息或其他 Qwen 版本兼容性。
用户审阅 fixture、CLI 解析、workload argv、Jinja 渲染、Rust 门禁和环境隔离报告后，
已接受 D082 与 M5-E.0.1。该接受只解除 chat template 的设计前置条件，不证明真实模型
加载或推理，也不授权 M5-E.1；维护窗口继续需要新的明确授权。

用户随后明确授权了 M5-E.1 首次维护窗口。尝试 1 在创建任何 deployment、container、
cache 或 GPU request 前因 `127.0.0.1:42000` 瞬时 bind 冲突停止并完成回滚；事后直接
bind 可用，故根因尚不能归于某个持续外部连接。该结果不修改 D079，也不授权自动换端口
或重试。把端口门禁从 LISTEN 检查加强为紧邻 deploy 的真实 bind 检查目前只是待讨论
修订，接受前不得写成新的冻结决策。

## 2026-10-22 M5-E.1 重试端口门禁修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D083 | 固定端口真实 bind 双门禁 | 保持 D079 的 `127.0.0.1:42000` 和全部 workload spec 不变；停止 Python Worker 前执行一次短生命周期真实 bind，drain 后完成服务/GPU 检查，再执行第二次真实 bind。第二次成功后不插入其他网络命令并立即执行同 spec deploy；任一次失败都不得 deploy，drain 后失败必须直接回滚。不得把诊断 socket 保持为监听、杀死未知连接、改端口或借重试调整模型参数 | 尝试 1 证明 LISTEN 快照不能替代应用实际 bind，且预检与 deploy 间存在竞态窗口。双门禁把可提前发现的冲突移到服务切换前，并缩短最终 bind 到 deploy 的窗口，同时保留 CLI 自身的最终失败关闭检查。 |

用户接受 D083，并明确授权用完全相同的固定 spec 和 GPU0 重试 M5-E.1；授权允许再次
停止并恢复 `gpustack-worker`，以及在门禁通过后创建、启动、停止和删除唯一 Lab 容器。
授权不允许更换端口、模型、Runner、GPU、ChatML、显存参数，不允许处置未知 socket，
也不把尝试 1 提升为正式结果。

同 spec 重试已验证 D083：两次真实 bind 均通过，唯一 Lab 容器也取得了精确 GPU0；但
固定 Runner 的 vLLM 0.25.1 明确拒绝 `QWenLMHeadModel`，因此 D079 当前冻结的模型与
Runner 组合不能通过 M5-E。该事实不自动修改 D079；后续须以新的决策在“保留 Qwen-7B、
改用兼容 Runner”和“保留当前 Runner、改用受支持模型”之间取舍。正式失败还证明 Lab
cache ownership 与回滚重入需要代码修复；二者完成无 GPU 验证前，不得请求新的 GPU
维护窗口。

## 2026-10-22 M5-E 模型兼容性修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D084 | 新单 GPU smoke 模型 | 保留 D079 的固定 Runner digest，模型改为 `Qwen/Qwen2.5-0.5B-Instruct` revision `7ae557604adf67be50417f59c2c2f167def9a775`，目标路径为 `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct`。Lab name 使用 `m5e-qwen25-05b-gpu0`，served name 使用 `qwen2.5-0.5b-m5e`；GPU0、42000、TP=1、max model length 2048、GPU memory utilization 0.5、dtype auto 与 eager 不变。使用模型内置 chat template，不传旧 Qwen-7B ChatML 或 `--trust-remote-code` | 该公开非 gated 模型仅约 1 GB，架构为当前 vLLM 支持的 `Qwen2ForCausalLM`，能同时覆盖 models、completion 和 chat 验收，并避免为了旧 `QWenLMHeadModel` 降级 Runner。固定 revision 和本地 manifest 防止上游 main 漂移。 |

D084 模型 revision 由 10 个顶层文件组成，总计 999,604,126 bytes；
`model.safetensors` 为 988,097,824 bytes，其上游 LFS SHA-256 为
`fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe`。正式下载须固定
revision，并对全部文件生成本地 SHA-256 manifest。优先经本机 VPN 下载后通过局域网 SSH
传输；下载、传输和完整性检查均不授权停止 Python Worker、创建 Lab 容器或使用 GPU。

## 2026-10-22 M5-E 清理与回滚修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D085 | 容器运行身份与回滚重入 | 共享 workload 契约升级为 `WorkloadSpecV2` 和 schema label `2`；把 Worker 进程的有效数字 `UID:GID` 作为 `container_user` 写入规范化 spec、spec hash 和 Docker `user`，并由 Worker 固定 `USER=gpustack`、`LOGNAME=gpustack`、`HOME=<deployment-cache>/home` 及四个 cache 环境变量。`lab stop` 在记录的容器已经缺失或没有 container ID 时直接收敛为 `STOPPED`，不执行 Docker 变更；容器仍存在时继续要求完整 ownership tuple 后才能 stop | 让 Runner 写入的 cache 与宿主 Rust Worker 属主一致，消除需要 root 修复容器的清理路径；同时使 `status → stop → remove` 能在中断后安全重入，而不降低对仍存在容器的失败关闭边界。运行身份会改变容器行为，因此必须进入 spec hash 并显式升级 schema。 |

D085 实施时 Lab 中没有 deployment、container 或 cache，只有可继续使用的本地 owner 记录，
因此没有迁移活动 workload。旧 schema manifest 在读取时继续失败关闭，不做静默转换。
该修订只授权无 GPU、无网络、无模型 mount 的 Runner 探针；不授权停止 Python Worker、
申请 GPU 或重新运行 M5-E.1。

## 2026-10-23 M6-A Managed 文件准备设计冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D086 | M6-A 状态边界 | M6-A 只实现并验证 `SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING` 与 ModelFile `downloading -> ready/error`；`--managed-file-only` 到 `STARTING` 即停止协调动作，不连接 Docker、不创建容器、不运行 vLLM、不把实例写为 `RUNNING` | 先独立证明 Server/Worker 状态所有权和文件准备协议，避免把真实 Server 写入与 Docker/GPU runtime 风险混在同一实现门禁。 |
| D087 | Managed 端口责任纠正 | 以固定 v2.2.3 源码为准：Worker 在处理 `SCHEDULED` 时从 `40000-40063` 分配或复用服务端口，并把 `port`、`ports` 与 `INITIALIZING` 一起完整写回；分配必须排除权威实例已占端口、执行 Worker IP 真实 bind 检查、拒绝处置未知占用者 | 旧设计中“Server 分配 Managed 端口”的表述与兼容 oracle 不符；Python Worker 的 `_assign_ports` 明确在 Worker 侧、写 `INITIALIZING` 前完成分配。 |
| D088 | 本地 ModelFile 解析 | R1 仅支持允许根目录内的绝对 `local_path`。解析必须 canonicalize symlink、拒绝越界或不存在路径；目录和普通文件返回 canonical path，分片文件验证编号完整、总数一致并稳定排序；递归计算大小。成功写 `resolved_paths`、`size`、`download_progress=100`、`ready`，失败写脱敏有界消息与 `error`，不进行网络下载 | 让本地模型与远程模型一样经过 ModelFile 控制链，同时把路径安全和不完整分片阻断在容器启动之前。 |
| D089 | 协调与全量更新 | watch/full-sync 只触发按 ID 去重的协调。每次 ModelInstance/ModelFile PUT 前必须重新 GET 最新 public 对象，复核 Worker assignment、状态、source 与关键输入，再从最新对象构造完整 update；状态已经前进、assignment 改变或不在 R1 范围时不写入 | v2.2.3 使用完整替换且没有 ETag；刷新不能消除所有竞态，但能避免从陈旧 watch cache 主动覆盖已知字段，并使重复事件保持幂等。 |
| D090 | M6-A CLI 前移与补充 | 把 `context add/use/list/remove` 和 `model deploy/list/get/status/delete` 的控制面实现前移到 M6-A；`model logs` 与 `inference models/chat/completion` 仍留在 M6-B。新增 `worker maintenance enable/disable <worker>`，所有命令通过 Context 的运维凭据调用 Server，Model CLI 不直接访问 Docker | M6-A 真实验证不能依赖 Web UI 或未文档化的 curl；显式 maintenance CLI 补齐了让保留 Rust Worker 临时可调度并安全恢复的运维缺口。 |
| D091 | M6-A 分段门禁 | M6-A.0 冻结契约；M6-A.1 离线实现端口、文件解析和协调器；M6-A.2 离线实现 Context/最小 Model/maintenance CLI；M6-A.3 另获授权后执行真实 Server、无 Docker 的文件准备 smoke。真实 smoke 精确调度到 Rust Worker GPU0，停在 `STARTING`，随后删除测试资源、恢复 maintenance 和原 Python Worker | 把源码正确性、CLI 对等、真实控制面写入及服务维护拆开授权；任一阶段失败都能停在没有 vLLM/GPU workload 的明确边界。 |

D087 仅取代 D068 中“Managed 使用 Server 分配端口”的责任表述；D068 关于 Lab 使用
loopback、`42000-42063` 端口池及其隔离理由继续有效。Managed 端口以 D087 的 Worker
侧 `40000-40063` 分配规则为准。

用户接受 D086–D091 只解锁 M6-A.1 的离线实现、fixture 和 fake/mock 测试。它不授权读取
运维凭据、改变 Worker maintenance、创建或删除真实 Model/ModelInstance/ModelFile、停止
Python Worker、连接 Docker、创建容器、申请 GPU 或运行推理。M6-A.2 仍是离线 CLI
实现；M6-A.3 的真实 Server 写入和维护窗口必须在前两段分别报告并获接受后另行授权。

## 2026-10-24 M6-A.2.1 Managed 生产入口接线修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D092 | 增设离线接线门禁 | 在 M6-A.2 与 M6-A.3 之间增设 M6-A.2.1，只实现并离线验证 `worker run --managed-file-only` 的正式 CLI 与 supervisor 接线；M6-A.3 继续作为单独授权的真实环境门禁 | M6-A.1 已实现协调器、M6-A.2 已实现控制面 CLI，但正式 `worker run` 尚不能启动协调器。若直接进入真实 smoke，测试命令与生产代码路径不闭合。 |
| D093 | Managed file-only runtime 组成 | `--managed-file-only` 与其他 Worker mode 互斥；复用 resource-api-only 的持久 Worker identity、真实 system/NVML 资源、Worker API、metrics 和 control plane，并接入 `ManagedCoordinator`。启动时要求目标 Worker 已处于 maintenance，但进程不得自动修改 maintenance；模式内不得构造 Docker client、WorkloadSpec、容器或 vLLM runtime | 该模式只负责把 Server 托管状态推进到文件准备边界，同时保留 M4 已验证的只读资源面和显式运维所有权，避免离线接线阶段扩大外部副作用。 |
| D094 | 观察性与接受边界 | supervisor 以脱敏结构化事件记录 ModelInstance/ModelFile ID、观察状态、分配或复用端口、文件结果和 `STARTING` no-op；优雅退出输出有界摘要。实现只运行 fixture/fake/mock、fmt、check、Clippy、test、doc-test 和 build，完成后状态为 `ready_for_review` 并交付 D033 详细报告 | 真实 smoke 前必须能审计 Worker 看到了什么、做了什么和为何停止；离线证据不能被误写成真实 Server、Docker 或 GPU 证据。 |

D092–D094 是对 M6-A 门禁拆分的纠正，不改变 D086 的状态所有权和 `STARTING` 边界，
也不扩大 R1 能力范围。用户接受该修订只授权 M6-A.2.1 的文档、源码和离线测试；不授权
读取真实 token、连接真实 Server、改变 maintenance、停止 Python Worker、访问 Docker
socket、创建容器、申请 GPU 或运行推理。M6-A.3 仍须在 M6-A.2.1 报告获接受后另行授权。

## 2026-10-24 M6-A.2.2 ModelFile 清理 CLI 修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D095 | 增设 ModelFile 清理门禁 | 在 M6-A.2.1 与 M6-A.3 之间增设 M6-A.2.2，增加 `model file get <id>` 和 `model file delete <id> --yes`，并先以 mock transport 和离线质量门禁验证 | Server 删除 Model 时会级联删除 ModelInstance，但独立 ModelFile 缓存记录不会自动删除；若直接执行真实 smoke，现有 CLI 无法满足 D091 的测试资源清理要求。 |
| D096 | 删除安全语义 | `model file delete` 只接受正整数 ID，删除前 GET 最新对象，只允许有明确 Worker 归属和非空本地路径的 `local_path` ModelFile；必须显式确认，并固定调用 `DELETE /v2/model-files/{id}?cleanup=false`。Server 仍被 ModelInstance 引用时的冲突门禁保持权威 | 精确 ID、最新对象检查和 Server 引用检查共同限制误删；`cleanup=false` 只删除 Server 元数据，绝不删除 90 主机上的共享模型文件。 |
| D097 | 授权和证据边界 | M6-A.2.2 只修改 Rust client/CLI、契约测试和文档；通过后状态为 `ready_for_review`，交付详细实现报告并等待用户接受。它不读取真实凭据、不连接真实 Server、不改变 maintenance、不停止 Python Worker，也不访问 Docker 或 GPU | 清理能力本身先在离线边界内验收，不能因为它服务于 M6-A.3 就提前获得真实环境权限。 |

D095–D097 是对 D091 回滚闭环的必要补充，不改变 M6-A 的状态边界、模型、GPU、Runner、
Worker 身份或正式 smoke 解释。M6-A.3 仍拆为独立只读预检和另行授权的维护窗口。

## 2026-10-24 M6-A.3 真实文件准备验证手册冻结

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D098 | 两级执行授权 | 把 M6-A.3 明确拆为 M6-A.3.0 独立只读预检和 M6-A.3.1 真实维护窗口。预检可以核对 Git/二进制/Server/Worker/模型/GPU/端口/Docker/Python Worker 基线并在 Git 外准备受保护证据，但不启动 Rust Worker、不修改 Server 资源或 maintenance、不停止 Python Worker。预检通过和共同审阅后，M6-A.3.1 仍须新的明确授权 | 可提前发现 identity、冲突和回滚材料问题，又不让“方案已接受”或“预检通过”被误解为允许真实写入和服务切换。 |
| D099 | 固定 smoke 输入和正确终点 | 唯一测试 Model name 为 `m6a3-qwen25-05b-gpu0`，固定 Qwen2.5 revision/local path、Runner digest、Rust Worker、GPU0、TP=1、max model length 2048、GPU memory utilization 0.5、dtype auto 和 eager。部署不得使用 `--wait`；通过终点是 Server 权威 `STARTING` 保持 60 秒和 Rust no-op，不是 `RUNNING` | `model deploy --wait` 只在 `RUNNING` 成功，而 M6-A.3 按 D086 有意不启动 runtime；禁止 `--wait` 防止把正确的 `STARTING` 误判为失败或诱导实现越界。固定 name 和 spec 让清理与证据能按精确 ID 归因。 |
| D100 | 状态链和零 runtime 门禁 | 300 秒内用 Rust 结构化事件、CLI 与 Server 快照共同证明 `SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING` 和 ModelFile `downloading -> ready`；同时证明没有 Docker client/container、vLLM、inference listener、GPU compute process、可归因显存增长或 `RUNNING` 写入 | 最终状态本身不能证明端口和 ModelFile 责任正确；组合时间线验证各层所有权。零 runtime 证据确保 M6-A 和 M6-B 的风险边界没有漂移。 |
| D101 | Python Worker 隔离和有界时限 | 即使 GPU0 预检空闲，正式 M6-A.3.1 仍只在维护窗口内停止并最终恢复精确 `gpustack-worker` 容器；Server 始终运行。Rust 启动、状态链、`STARTING` 保持和总 watchdog 分别使用 120、300、60 和 900 秒的有界时限，失败不在同一 run 内临时换输入或无限延长 | Python 与 Rust Worker 同时报同一物理 GPU 会削弱调度和状态证据的归因，即使没有即时 compute process。固定时限让失败可判定，避免观察结果后调整协议。 |
| D102 | 清理、恢复和证据 | 清理顺序固定为先恢复 Rust maintenance，再删除精确 Model/ModelInstance，随后重新 GET 并以 `cleanup=false` 删除精确 ModelFile 元数据，核对本地模型未变，停止精确 Rust PID，最后恢复原 Python Worker 并做资源 ID/容器/端口/GPU 零残留差分。任何恢复或清理失败都使正式 run 失败。Git 外证据不得包含 secret | Server 删除 Model 不自动删除独立 ModelFile；先隔离调度再窄范围删除可避免新任务和误删。`cleanup=false` 保留共享模型数据，最终恢复门禁确保成功状态链不能掩盖环境破坏。 |

D098–D102 的完整检查组、停止条件、执行顺序、回滚和证据契约见
`m6a3-managed-file-runbook.md`。固定 ModelFile 冲突检查是硬门禁：若 Rust Worker 已有同一
`local_path` 的 ready 记录，正式 run 会复用缓存而不能证明 `downloading -> ready`，因此
预检必须停止且不得自行删除该记录。

用户接受本手册只把 M6-A.3.0 标记为 `frozen`，不授权实际连接 90 环境执行预检。
M6-A.3.0 和 M6-A.3.1 的执行必须分别获得后续明确授权；M6-A.3.1 的授权还必须明确包含
停止和恢复 Python Worker、改变精确 Rust Worker maintenance 以及创建和删除唯一测试
Model/ModelInstance/ModelFile。两段均不授权 Docker workload、GPU workload 或推理。

## 2026-10-24 M6-A.2.3 只读预检 CLI 补全

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D103 | 增设只读 CLI 前置门禁 | 在 M6-A.2.2 与 M6-A.3.0 之间增设 M6-A.2.3，实现 `worker get <worker-id-or-exact-name>` 和 `model file list --worker <worker-id-or-exact-name>`，只使用 Context 的运维身份和类型化 Server client | M6-A.3.0 必须用产品 CLI 核对精确 Worker 和同 Worker 的 ModelFile 冲突；Web UI 或临时 curl 会破坏 D090 的 CLI-first 证据链。 |
| D104 | Worker 安全投影 | `worker get` 的数字 selector 按 ID 查询，其他 selector 只接受精确名称；无结果和重名均失败关闭。JSON 与 table 只输出明确白名单字段，不透传 Worker `extensions`、provider config、principal ID 或未知扩展；固定 v2.2.3 的 public 响应没有 `worker_uuid` 时不伪造该字段 | Worker public schema 会展平服务端扩展，直接序列化完整对象可能把与预检无关的敏感或不稳定字段带入终端和证据。精确解析避免因模糊匹配检查错误节点。 |
| D105 | 按 Worker 的 ModelFile 列表 | `model file list` 必须显式提供 `--worker`，先按 D104 解析唯一 Worker，再请求 `/v2/model-files?page=-1&worker_id=<id>`；结果按 ModelFile ID 稳定排序。JSON 返回与 `model file get` 相同的 `ModelFilePublic` 数组，table 固定显示 ID、Worker ID、source、state、local path、size、progress 和更新时间；空列表是成功 | 强制 Worker scope 可避免全局枚举和错误归因；稳定顺序和一致 schema 便于预检比较及自动化取证。 |
| D106 | 实现与授权边界 | M6-A.2.3 只修改 Rust CLI、mock/fixture 测试和治理文档，不新增依赖，也不实现 ModelInstance list。通过离线质量门禁后状态为 `ready_for_review`，必须交付 D033 详细报告并等待用户接受；M6-A.3.0 仍需单独明确授权 | 只读命令的实现不能被解释为允许读取真实 token、连接真实 Server 或开始预检，更不能授权维护窗口、Docker、GPU 或推理操作。 |

D103–D106 只关闭 M6-A.3 手册已经识别的产品 CLI 缺口，不改变 D086–D091、D098–D102
的状态链、回滚和分段授权。用户接受本设计只授权文档、源码和离线测试；不授权读取真实
凭据、连接真实 Server、改变 maintenance、停止 Python Worker、访问 Docker 或使用 GPU。

## 2026-10-24 M6-A.3 operator Context 准备门禁

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D107 | operator 凭据类型 | Context 必须使用固定 v2.2.3 Server 创建的正式 API Key，不使用 `/auth/login` 产生的 session JWT，也不复用 Cluster registration token 或 per-Worker token | 源码核对确认 `/auth/login` 只把 120 分钟 JWT 写入 cookie；Rust CLI 固定发送 Bearer，而 Server 会把 Bearer 解析为 API Key。把 session JWT 写入 token 文件会稳定认证失败，其他系统凭据则具有错误身份和权限。 |
| D108 | 最小权限与生命周期 | 通过本地管理员的隐藏式交互登录创建唯一 `rust-worker-m6a3-operator-20261024` API Key，scope 仅为 `management`，有效期固定 7 天。M6-A.3.1 完成、项目中止或凭据不再需要时，删除该 Key 和本地 Context/token；过期不得被解释为预检失败，必须重新走凭据准备门禁 | M6-A.3 只需要 Worker、Model 和 ModelFile 管理 API，不需要 inference scope。短期、阶段专用凭据限制泄露窗口，又为预检审阅与维护窗口授权保留合理时间。 |
| D109 | secret 输入与落盘 | 用户只在 90 主机终端以无回显方式输入用户名和密码；密码、cookie、API Key 完整创建响应只进入 mode `0600` 的临时文件，且不进入参数、shell history、聊天、Git 或证据。最终 token 文件固定为 `/home/wujiaxing2/.config/gpustack-rs/credentials/m6a3-operator.token`，Context 固定名 `lab90`，Server URL 为 `http://127.0.0.1/`；临时 secret 在 Context 只读验证后逐文件清除 | Rust Context 只保存 token 文件引用；受限文件和最小暴露面符合既有 credential 契约，并避免再次把密码或 token 带入聊天及进程参数。 |
| D110 | 失败证据保留与重跑路径 | 已停止的 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal` 永久保留为 attempt 1，不删除、不改名、不覆盖。下一次获授权的完整预检固定使用尚未创建的 `m6a3-formal-attempt-2`。Context 准备成功只解除前置条件，不自动重跑 M6-A.3.0，也不授权 M6-A.3.1 | 证据路径是原协议的一部分；显式滚动到新目录既保存失败链，也避免通过覆盖历史制造看似首次通过的结果。重新执行仍需外部漂移检查和新的明确授权。 |

用户已明确授权 D108 所述的一次 API Key 创建写入和 D109 所述的本地 Context/token
准备；授权不包含改变 Worker maintenance、创建 Model/ModelInstance/ModelFile、停止 Python
Worker、启动 Rust Worker、访问 Docker、使用 GPU 或重跑 M6-A.3.0。由于密码必须仅在用户
终端隐藏输入，该门禁在交互命令成功前保持 `authorized_pending_human_secret_entry`。

## 2026-10-26 M6-A.3 operator Context 认证恢复

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D111 | 已认证 Web 会话恢复路径 | 用户确认此前 401 来自密码误输后，不再猜测本地密码；允许使用用户当前已认证的 Web 管理会话调用正式 `/v2/api-keys` 创建 endpoint。完整 Key 不进入聊天或工具参数，只由用户从一次性创建响应直接无回显写入 90 主机 mode 0600 文件 | 仍通过正式 Server API 创建用户拥有的 API Key，同时避免继续处理或恢复管理员密码；浏览器 session cookie 不导出、不写入 Rust Context。 |
| D112 | API 权威元数据与最小权限修复 | 创建后必须以 `/v2/api-keys` 返回的 `scope` 和 `expires_at` 为准，不以 UI 标签为验收证据。若 Key 为永久或包含 `inference`，使用短期、仅 `management` 的桥接 Key 先保留管理能力，再删除不合规 Key、创建同名 7 天仅 `management` Key、原子替换本地 token，最后删除桥接 Key 和恢复文件 | UI 的“平台管理”和过期显示不能替代 API 契约；桥接顺序避免先删除唯一凭据导致无法完成收紧，也保证任何中断都有 mode 0600 恢复凭据。 |
| D113 | Context 就绪不扩展执行授权 | 正式 Key 仅允许用 `context list` 和 `worker get rust-m3d-r760xa` 验收。成功状态为 `operator_context_ready`；第二次 M6-A.3.0 仍须新的明确授权并使用 `m6a3-formal-attempt-2`，M6-A.3.1 仍须预检审阅后的独立维护授权 | 凭据准备是预检前置条件，不是预检本身；不能把一次 Server API Key 写入扩展为模型、maintenance、Worker 或 workload 操作授权。 |

D111–D113 是 D107–D110 的受限恢复路径，不改变最终 Key 名称、scope、7 天生命周期、token
文件或 Context 契约，也不改变 D098–D102 的两级执行边界。

## 2026-10-26 M6-A.3 Worker identity 恢复

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D114 | 当前 Worker identity 选择 | M6-A.3 后续固定使用 `/home/wujiaxing2/.local/state/gpustack-rust-worker/1`。其中持久 identity 为 Worker ID 5、名称 `rust-m3d-r760xa`、UUID `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`；其 per-Worker token 必须通过 `GET /v2/workers?me=true` 只读认证为唯一 ID 5/name。`me` 不返回 UUID 时继续按 D035 使用受保护本地元数据，不伪造 Server 字段 | 第二次预检证明旧 `m3d` 目录的 ID 2 已与当前 Server 漂移；只读诊断和独立 token 验证则证明 `/1` 是当前 ID 5 的完整持久身份。直接采用现有目录不复制凭据，也不引入新的 Worker 注册或 Server 写入。 |
| D115 | 权限收紧与历史保留 | 将 `/1` 顶层目录从 mode 0775 收紧为 0700；`managed/` 保持 0700，identity/token 保持 0600 和单链接。旧 `m3d` 目录及 ID 2 identity/token 不移动、不覆盖、不删除，只作为历史恢复证据保留 | 顶层目录收紧消除不必要的遍历权限；保留旧身份与前两次停止证据，可解释身份漂移并避免通过改写历史制造通过结果。 |
| D116 | 第三次预检证据与授权 | 第三次 M6-A.3.0 若另获明确授权，固定使用尚未创建的 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-3`。`m6a3-formal` 和 `m6a3-formal-attempt-2` 均保持不可变；身份恢复完成不自动开始第三次预检，更不授权 M6-A.3.1 | 每次正式尝试使用唯一证据目录，保持失败链可审计。冻结恢复输入与授权执行仍是两个独立门禁。 |

D114–D116 只修订 M6-A.3 的当前持久身份输入和后续证据路径，不改变固定 Worker name、
Server、模型、GPU、状态链、通过标准或回滚顺序。恢复操作不得修改 Server、maintenance、
Python Worker、Docker 或 GPU workload。

D116 冻结的 attempt 3 随后已执行，但因 `context list` 被错误传入全局 `--context` 而在
CLI 本地参数检查阶段拒绝。该目录已作为非评分 `INVALIDATED` 证据封存，不能删除、覆盖或
解释为正式预检失败；新的重试证据路径和执行授权尚未冻结。

## 2026-10-26 M6-A.3 第四次预检修订

| ID | 决策项 | 已接受方案 | 理由 |
|---|---|---|---|
| D117 | 第四次证据路径 | 第四次 M6-A.3.0 固定使用尚未创建的 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-4`；attempt 1、attempt 2、无效 attempt 3 和 identity 恢复证据均保持不可变 | attempt 3 已被无效调用占用；滚动到唯一新目录可保留完整证据链，避免覆盖后看似首次通过。 |
| D118 | Context 命令边界 | 本地 Context 枚举固定使用不带 selector 的 `gpustack-rs --output json context list`；只有 Worker、Model 和 ModelFile 等 Server 命令使用 `--context lab90`。不得修改 Context 内容来适配预检 | `context list` 管理的是本地 Context store，产品契约明确拒绝 `--context`；Server 命令才需要选择认证上下文。该修正只纠正调用语法，不改变凭据、Server 输入或验收标准。 |
| D119 | 第四次执行授权 | 用户接受 D117–D118 并授权第四次 M6-A.3.0 的冻结构建、真实只读查询和 Git 外证据写入；仍不授权 Server 写入、maintenance 变化、停止 Python Worker、启动 Rust Worker、Docker/GPU workload 或推理 | 将无效尝试后的方法修订与新一次执行范围明确绑定，同时保持 M6-A.3.1 独立维护授权门禁。 |

D117–D119 不改变 D098–D102、D114–D116 的 Worker identity、模型、GPU、状态链、通过条件
或回滚方案。任一硬门禁失败仍立即停止，不能在同一 attempt 中通过换输入或改命令继续。

D117 冻结的 attempt 4 随后在 GPU0 空闲硬门禁返回 `STOP/gpu0_not_idle`。占用来自既有
Rust Lab `qwen2.5-0.5b-m5e` 及其唯一容器，不是本次预检创建的资源；该证据按正式
`stopped_preflight` 保存。停止/删除既有 Lab、新证据路径和再次执行授权均尚未冻结。

### 第四次预检后的外部状态变化

用户确认 `qwen2.5-0.5b-m5e` 是此前测试部署，并自行停止和删除。后续只读复核确认精确
Lab 容器和部署状态文件均不存在，GPU0 显存占用与利用率为 0、没有 compute process；固定
Qwen2.5 模型目录仍存在，Server、Python Worker health/metrics 均为 200，Rust Worker
进程和 `20150/20151` 监听均为 0。该事实只解除 attempt 4 当时观察到的外部 GPU blocker，
不改变 attempt 4 的 `STOP/gpu0_not_idle` 历史判定，也不冻结第五次证据路径或授予新的
M6-A.3.0/M6-A.3.1 执行权限。
