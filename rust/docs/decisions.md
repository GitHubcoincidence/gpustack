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
