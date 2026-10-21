# Rust GPUStack 重写文档

本目录是 GPUStack Rust 重写项目的长期设计、治理、验证与交接记录。首个发布里程碑是实现一个与现有 Python Server 兼容的 Rust Worker。

项目先通过讨论冻结设计，再进入实现。聊天仍是主要协作渠道；这些文件用于持久保存决策和证据，使后续工作无需依赖聊天记录也能准确恢复。

## 文档索引

- [charter.md](charter.md)：目标、R1 范围、非目标、不变量与发布标准。
- [architecture.md](architecture.md)：组件边界、状态所有权、协调循环、运行时、配置与部署拓扑。
- [cli-design.md](cli-design.md)：CLI-first 产品契约，以及 Managed 和 Lab 两种命令界面。
- [worker-protocol.md](worker-protocol.md)：Server/Worker 兼容协议、Worker HTTP API、认证与模型生命周期。
- [server-contract-v2.2.3.md](server-contract-v2.2.3.md)：M3 固定的 Server endpoint、schema、认证、分页、watch 与错误契约。
- [worker-contract-v2.2.3.md](worker-contract-v2.2.3.md)：M4 固定的 Worker HTTP、认证、文件/日志/配置、资源 schema、metrics 与依赖版本契约。
- [implementation-plan.md](implementation-plan.md)：带门禁的里程碑、验证矩阵、证据要求与回滚约束。
- [environment-baseline.md](environment-baseline.md)：M1 验证过的 90 主机、GPU、Docker、部署、镜像、模型与网络身份。
- [decisions.md](decisions.md)：已接受的架构决策与被否决的替代方案。
- [execution-log.md](execution-log.md)：关键检查点、当前停止位置与下一项允许执行的工作。
- [m3d-integration-runbook.md](m3d-integration-runbook.md)：M3-D.2 真实控制面测试的分组命令、判定标准与安全边界。

## 文档语言

- `rust/docs/` 中的内部治理和实验文档以中文为权威版本。
- 命令、API 路径、类型名、配置键、状态名与代码标识符保留英文。
- 代码注释遵循仓库约定使用英文。
- 若内容以后进入顶层 MkDocs `docs/`，应另行编写英文发布版本；不并行维护一套中英文内部治理文档，以免发生内容漂移。

## 当前状态

- 设计状态：`frozen`
- 实现状态：M2、M3-B、M3-C、M3-C.1、M3-C.2、M3-D.1、M3-D.1.1 与 M3-D.2 均为 `accepted`；M4 设计和门禁为 `frozen`，M4-A 精确契约、fixture 与依赖审计为 `ready_for_review`，M4-B 尚未授权。M3-D.1.1 代码检查点为 `99e9fdb9`，M3-D.2 正式二进制来自 `e30e7bd4`；M3-A 注册 fixture 的 UUID 等值语义和 M3-C 对应身份断言保持 `invalidated`。
- 环境基线：M1 已接受；M2 固定 Rust 1.99.1，详见 `environment-baseline.md`。
- 当前停止位置：M4-A 已完成固定 v2.2.3 的精确 Worker API/resource schema、fixture 和依赖可构建性审计，等待详细报告和共同验收；maintenance Worker `rust-m3d-r760xa`（ID 2）和本地 per-Worker 身份保留供 M4-D 复用，Cluster registration token 本地副本已删除，Rust 进程与 `20150/20151` 监听均应保持停止。
- 下一项允许执行的工作：只审阅并验收 M4-A。用户接受后才能进入 M4-B 的 mock/fake 驱动离线实现；M4-C 真实 NVML 冒烟、M4-D 停止 Python Worker，以及任何 Docker/vLLM/模型任务都尚未授权。

当前检查点包含 Rust CLI/Worker 基础源码、已接受的离线契约 fixture、类型化 Server client、身份持久化、控制循环、显式 control-plane-only supervisor、最小 `/healthz`，以及真实 v2.2.3 Server 上的注册、持续控制循环和无 Cluster token 身份恢复证据；M4 的 resource-api-only/API/metrics/资源采集仍只有设计，不包含实现或运行证据，也不包含 Docker workload、GPU 任务、vLLM 推理或 Python Worker 变更。
