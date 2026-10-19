# GPUStack Rust 重写：目标与原则

## 一句话目标

以渐进方式使用 Rust 重写 GPUStack。第一步实现一个 CLI-first 的 Worker：它兼容现有 GPUStack Server，并能在 NVIDIA 宿主机上通过本地 Docker 容器可靠运行 vLLM。

## 动机与用户

本项目需要一个更精简、行为明确且可恢复的 Worker 实现。其运行时行为必须能通过命令行测试，并能在真实 GPU 上完成部署验证。对于某个版本已经支持的每项能力，运维人员都应能在不依赖 Web UI 的情况下完成部署、检查、调用与清理。

第一批用户是使用一台配有两张 NVIDIA A100 的 Linux 主机验证 Rust Worker 的 GPUStack 开发者和运维人员。长期用户则是需要一个可脚本化、可观测 GPU 集群管理器的所有 GPUStack 运维人员。

## 长期产品原则

GPUStack 坚持 CLI-first：

- 一个版本支持的每项面向用户或运维人员的能力，都必须有稳定、非交互式的 CLI 入口。
- Web UI 与 CLI 都是同一组 Server API 的客户端，二者不各自拥有独立的业务流程。
- 心跳、控制器回调和事件流等内部机制，不为追求表面上的 CLI 对等而暴露成人工命令。
- 迁移期间 Rust 二进制命名为 `gpustack-rs`，避免与已安装的 Python `gpustack` 命令冲突；最终公开名称仍为 `gpustack`。

## R1 范围

R1 包含：

- Linux x86_64 与 NVIDIA GPU，并在一台双 A100 主机上完成正式验证。
- 一个兼容该主机现有 Server 版本的 Rust Worker。M1 用证据冻结准确版本、镜像摘要与 commit。
- 原生 Rust 实现的 `gpustack-runtime` 子集，负责系统信息、NVML 与 Docker Engine 工作负载生命周期。
- vLLM 是唯一推理后端。
- Docker 是唯一工作负载运行时。
- 本地模型目录是唯一模型来源。
- 一个 Worker 使用一张或两张本地 GPU，包括跨两张 GPU 的 tensor parallelism。
- 复用现有 `gpustack/runner` 镜像作为 vLLM 执行环境。
- 优先使用模型请求显式指定的镜像，否则使用配置中的默认 vLLM 镜像；缺失或不兼容时直接报错。
- Server 托管的生产模式与相互隔离的本地 Lab 模式；两者共享相同的工作负载规划器、运行时、健康检查、日志和协调器。
- 统一的 Rust CLI，覆盖 Worker 运维、Lab 演示、Server 托管模型操作与推理请求。
- 最小化且经过认证的 Worker HTTP API，用于健康、就绪、版本、本地模型检查、模型日志、配置查看、令牌验证和指标。
- Worker 容器化部署是 R1 发布门禁；宿主机进程方式仅作为中间开发模式。
- 使用 Git 管理源码、文档和里程碑检查点。

## R1 非目标

R1 不包含：

- Kubernetes、Podman 或远程容器运行时。
- AMD、Ascend、MUSA 或其他加速器厂商。
- SGLang、VoxBox、MindIE、自定义后端或任意命令。
- 跨 Worker 分布式推理或 Ray 集群。
- Hugging Face 或 ModelScope 下载、缓存管理或下载进度。
- LoRA、GGUF 解析、speculative decoding 或扩展 KV cache 服务。
- Benchmark Manager 或 Cache Service Manager。
- Worker 推理代理、tunnel 模式或 cluster proxy。
- 自动选择 Runner catalog 或 Runner override 条目。
- 使用 Rust 重写 vLLM 或 Runner 容器镜像中的载荷。
- 在 R1 重写全部 Server 管理能力；CLI 覆盖面随以后被重写的模块逐步扩展。

## 不变量与约束

- 现有 Server 是生产环境期望状态的权威来源。
- Docker 是容器实际状态的权威来源；Worker 负责协调两者差异，绝不把本地 manifest 当成权威状态。
- 只能修改或删除带有完整且匹配的 Rust 所有权标签的容器。
- Lab 与 Managed 工作负载使用不同的身份、目录、标签、端口和状态接收端。
- Server 或事件流的短暂故障不得停止或删除健康的推理容器。
- Worker 退出不得删除健康的 Managed 推理容器。
- 破坏性清理必须以权威 Server 状态为依据，或者来自明确且仅作用于 Lab 所有权标签的命令。
- 正式 GPU 测试期间，Python Worker 与 Rust Worker 不得同时声明或使用相同的物理 GPU。
- Worker 系统凭据与运维人员凭据必须分离。
- token、Git 私钥、模型凭据和敏感环境变量不得进入 Git、命令输出、日志或证据归档。
- R1 兼容开发期间不升级已部署的 Server。
- 可写源码仓库为 fork `GitHubcoincidence/gpustack`，上游仓库为 `gpustack/gpustack`。
- Rust 代码必须在安全边界、状态转移、协议偏差、恢复逻辑、所有权校验和非直观算法等关键位置使用英文注释解释“做什么”和“为什么”。简单字段、显然的参数和能由代码直接表达的逻辑不要求注释；缺少关键说明、失真、逐行复述、描述修改历史或注释密度干扰阅读均不合格。
- 每个实现阶段在请求验收前，必须先在聊天中直接给出完整实现报告，说明阶段目标、已实现功能、实现机制、关键数据流、变更模块、依赖与取舍、验证证据、安全边界、已知限制、未实现项和下一步。长期文档可以只保存检查点摘要，但不能代替这份直接报告。

## 成功标准

只有证据同时证明以下事项，R1 才能验收：

- Rust CLI 可以从已记录的 commit 和 `Cargo.lock` 可重复构建。
- Worker 能针对已冻结的 Server 基线完成注册、认证、心跳、状态上报与事件流恢复。
- Server 能准确看到两张 A100。
- Lab 模式可以完全通过 CLI 部署、检查、调用、查看日志并移除一个 vLLM 模型。
- Managed 模式可以完全通过 CLI 和 Server 部署、检查、调用、查看日志并移除同类模型。
- 单 GPU 和双 GPU tensor-parallel 部署都能进入 `RUNNING` 并返回有效推理结果。
- 进程重启和 Worker 容器重启后，能够接管已有的健康工作负载，而不是重复创建或误删。
- Server 与事件流恢复连接后，能通过一次权威全量协调重新收敛。
- 删除模型后不遗留任何归属本 Worker 的孤儿容器或保留端口。
- 未知容器及 Python Worker 所有的容器不受影响。
- 正式部署形态是 Rust Worker 运行在容器中，vLLM 运行在同级 Runner 容器中。
- 按文档执行回滚能够成功恢复 Python Worker。
- 单元、契约、集成、安全与恢复证据均记录准确的 commit、镜像摘要、环境版本和命令。
- Rust 工作区通过格式化、Clippy、测试和离线构建门禁；人工审阅同时确认关键逻辑具有必要且不过量的有效注释。
- 每个阶段的直接实现报告已经在请求验收前交付，且其描述与提交、测试证据和当前限制一致。

性能测量是强制证据，但 R1 不预设 Rust 必须比 Python 快某个百分比。功能兼容性、运行安全与恢复能力才是硬门禁。

## 结果失效条件

若 R1 结果依赖未记录的 Server 升级、使用的代码或镜像版本与证据记录不符、允许 Python 与 Rust Worker 重复计算同一 GPU、在声称为 Managed 的测试中绕过 Server、把 Lab 冒烟证据提升为正式 Managed 证据、泄露敏感信息，或在无法证明所有权时清理容器，则该结果无效。

## 预期最终能力

完成 R1 后，GPUStack 可以声明：选定的 Server 版本能够使用容器化 Rust Worker，在一张或两张 NVIDIA GPU 上管理基于本地 Docker 的 vLLM 推理；完整 R1 流程可以通过稳定 CLI 操作，无需 Web UI。
