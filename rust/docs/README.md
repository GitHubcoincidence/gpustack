# Rust GPUStack 重写文档

本目录是 GPUStack Rust 重写项目的长期设计、治理、验证与交接记录。首个发布里程碑是实现一个与现有 Python Server 兼容的 Rust Worker。

项目先通过讨论冻结设计，再进入实现。聊天仍是主要协作渠道；这些文件用于持久保存决策和证据，使后续工作无需依赖聊天记录也能准确恢复。

## 文档索引

- [charter.md](charter.md)：目标、R1 范围、非目标、不变量与发布标准。
- [architecture.md](architecture.md)：组件边界、状态所有权、协调循环、运行时、配置与部署拓扑。
- [cli-design.md](cli-design.md)：CLI-first 产品契约，以及 Managed 和 Lab 两种命令界面。
- [worker-protocol.md](worker-protocol.md)：Server/Worker 兼容协议、Worker HTTP API、认证与模型生命周期。
- [server-contract-v2.2.3.md](server-contract-v2.2.3.md)：M3 固定的 Server endpoint、schema、认证、分页、watch 与错误契约。
- [implementation-plan.md](implementation-plan.md)：带门禁的里程碑、验证矩阵、证据要求与回滚约束。
- [environment-baseline.md](environment-baseline.md)：M1 验证过的 90 主机、GPU、Docker、部署、镜像、模型与网络身份。
- [decisions.md](decisions.md)：已接受的架构决策与被否决的替代方案。
- [execution-log.md](execution-log.md)：关键检查点、当前停止位置与下一项允许执行的工作。

## 文档语言

- `rust/docs/` 中的内部治理和实验文档以中文为权威版本。
- 命令、API 路径、类型名、配置键、状态名与代码标识符保留英文。
- 代码注释遵循仓库约定使用英文。
- 若内容以后进入顶层 MkDocs `docs/`，应另行编写英文发布版本；不并行维护一套中英文内部治理文档，以免发生内容漂移。

## 当前状态

- 设计状态：`frozen`
- 实现状态：M2、M3-B、M3-C、M3-C.1、M3-C.2 与 M3-D.1 control-plane-only 离线实现均为 `accepted`；M3-A 注册 fixture 的 UUID 等值语义和 M3-C 对应身份断言保持 `invalidated`；M3-D.2 真实 Server 集成尚未开始。
- 环境基线：M1 已接受；M2 固定 Rust 1.99.1，详见 `environment-baseline.md`。
- 当前停止位置：用户已审阅并接受 M3-D.1 详细实现报告，正在讨论 M3-D.2 真实集成运行手册；没有执行 Server 写请求，现有 GPUStack 服务未受影响。
- 下一项允许执行的工作：共同冻结 M3-D.2 的凭据、预检、运行、证据、停止和失败处置步骤；第一次真实注册仍需用户再次明确批准。

当前检查点包含 Rust CLI/Worker 基础源码、已接受的离线契约 fixture、类型化 Server client、身份持久化、控制循环、显式 control-plane-only supervisor 和最小 `/healthz`；不包含实际 Worker 注册、真实 Server 写操作、Docker 工作负载变更、GPU 使用或 Python Worker 变更。
