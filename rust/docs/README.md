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
- [m6a3-managed-file-runbook.md](m6a3-managed-file-runbook.md)：M6-A.3 只读预检、真实无 Docker 文件准备 smoke、证据与回滚边界。

## 文档语言

- `rust/docs/` 中的内部治理和实验文档以中文为权威版本。
- 命令、API 路径、类型名、配置键、状态名与代码标识符保留英文。
- 代码注释遵循仓库约定使用英文。
- 若内容以后进入顶层 MkDocs `docs/`，应另行编写英文发布版本；不并行维护一套中英文内部治理文档，以免发生内容漂移。

## 当前状态

- 设计状态：M6-A.2.3 只读预检 CLI、M6-A.3.0/.3.1 方法，以及 D107–D119 operator
  Context/API Key 准备和 Worker identity 恢复门禁均已 `frozen`；完整 R1 架构继续按门禁
  推进。
- 已接受实现：M1～M5 全部阶段、M6-A.0、M6-A.1、M6-A.2、M6-A.2.1、M6-A.2.2 和
  M6-A.2.3。当前 Rust CLI 已具备 Context、Model、Worker 查询/maintenance 和安全 ModelFile
  元数据清理能力，`worker run --managed-file-only` 已接入离线验证的 Managed 文件准备
  协调器；只读 `worker get` 与按 Worker 的 `model file list` 已完成离线验收。
- 当前证据边界：M5 已真实证明宿主机进程形态的单 GPU Lab Qwen2.5/vLLM 全流程；M6-A
  已真实证明 7 天、仅 `management` 的 operator API Key、active `lab90` Context 和
  `worker get rust-m3d-r760xa` 只读管理链路。第二次 M6-A.3.0 在 Worker identity 硬门禁
  按协议停止；随后已验证 `/1` 中的 per-Worker token 对应当前唯一 ID 5，并将该目录权限
  收紧为 0700。第三次执行的构建通过，但因无效 `context list` 参数在本地 CLI 检查阶段
  终止并按非评分 `INVALIDATED` 封存。第四次执行通过身份、资源和模型门禁，但因 GPU0
  上既有 Rust Lab workload 按协议停止；该 Lab 随后由用户停止并删除，且已通过只读
  复核确认 GPU0 恢复空闲。尚未证明真实 Server 上的 Managed
  ModelInstance/ModelFile 状态链，也未进入 M6-B Docker/vLLM runtime。
- 当前停止位置：M6-A.3 已按 D114–D116 把当前持久身份固定为 `/1` 中的 Worker ID 5，
  名称 `rust-m3d-r760xa`、UUID `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`。旧 `m3d` 中 ID 2、
  前两次停止证据和身份恢复证据均保持不变；attempt 3 已封存无效调用证据，attempt 4 已
  封存 `gpu0_not_idle`。该 Lab 不是 Server ModelInstance 或本次预检资源；用户完成清理后，
  只读复核确认精确容器和部署状态文件均不存在、GPU0 无 compute process，固定 Qwen2.5
  模型目录仍保留，Server 与 Python Worker 健康。
- 下一项允许执行的工作：attempt 5 因 Cargo working directory 调用错误在构建前按非评分
  `INVALIDATED` 封存。只允许讨论并冻结新的证据路径、修正脚本及新的 M6-A.3.0 只读执行
  授权；attempt 4 仍是有效 `STOP/gpu0_not_idle`，且仍不授权 M6-A.3.1、maintenance、
  模型写入或 Python Worker 维护窗口。

环境基线见 `environment-baseline.md`，实时停止位置与准确 commit/证据 identity 以
`execution-log.md` 为准。
