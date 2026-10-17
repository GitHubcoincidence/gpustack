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

## M1 待确认身份信息

下列内容是有意留待确认的值，不属于尚未决定的架构问题：已部署 Server version/commit/digest、当前部署拓扑、driver 与 Docker 版本、准确端口和路径、验证模型 identity、Runner image digest、direct-network 连通性，以及最终容器 user/group 参数。M1 必须用实测证据替换这些占位信息，之后实现才能依赖它们。
