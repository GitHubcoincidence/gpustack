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
| M4 资源盘点与 Worker API | NVML/系统上报、文件系统 API、日志、读取配置、metrics 与认证 | M3-D | Server 能看到两张 A100，并能调用所需 Worker API | Python/Rust fixture 对比、路径安全测试、HTTP auth test、A100 证据 |
| M5 Docker runtime 与 Lab | Docker 原语、所有权标签、WorkloadSpec、vLLM builder、本地 Lab 状态、Lab 命令、日志、健康与清理 | M4 与维护窗口 | 完全通过 CLI 完成单 GPU Lab 推理，不创建 Server ModelInstance/ModelFile | unit plan、Docker smoke、vLLM response、log stream、清理与非所有权测试 |
| M6-A Managed 文件准备 | `SCHEDULED -> INITIALIZING`、ModelFile 本地路径校验/分片解析、完整结果上报，并等待 Server `STARTING` | M5 与 M3-C.2 | Managed 实例按原状态所有权完成本地模型准备，不提前启动容器 | 状态所有权、ModelFile ready/error、路径安全和“STARTING 前无容器”测试 |
| M6-B Managed 单 GPU runtime | `STARTING` 后 reconciler 启动/接管 vLLM，健康后上报 `RUNNING`，并支持 model CLI、推理与删除 | M6-A | 完全通过 CLI 完成 Server 托管单 GPU 推理 | 完整命令记录、全状态链、响应、日志、健康门禁和删除后无孤儿资源 |
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

M3-D.1 已接受，用户已进一步接受 D047–D048 的凭据生命周期修订。当前只允许实现 M3-D.1.1 并以 fixture/mock/临时目录验证完整身份不依赖 Cluster token；在完成详细报告且用户对冻结运行手册再次明确授权前，不得读取真实 token、注册 Worker 或发送真实 Server 写请求，也不得操作 Docker、GPU、vLLM 或 Python Worker。
