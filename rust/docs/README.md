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

- 设计状态：M6-A.2.3 只读预检 CLI、M6-A.3.0/.3.1 方法，以及 D107–D151 operator
  Context、Worker identity、READY、资源保留字段和单 Worker分布式占位门禁均已 `frozen`；完整 R1 架构继续按
  门禁推进。
- 已接受实现：M1～M5 全部阶段、M6-A.0、M6-A.1、M6-A.2、M6-A.2.1、M6-A.2.2 和
  M6-A.2.3。当前 Rust CLI 已具备 Context、Model、Worker 查询/maintenance 和安全 ModelFile
  元数据清理能力，`worker run --managed-file-only` 已接入离线验证的 Managed 文件准备
  协调器；只读 `worker get` 与按 Worker 的 `model file list` 已完成离线验收。
- 当前证据边界：M5 已真实证明宿主机进程形态的单 GPU Lab Qwen2.5/vLLM 全流程；M6-A
  已真实证明 operator Context、当前 Worker ID 5 的持久身份和 M6-A.3.0 全部只读门禁。
  M6-A.3.1 attempt 1 和 attempt 2 都在维护窗口之前因执行器字段读取错误而 fail-closed：
  attempt 1 把 `worker_id` 错读为 `id`，attempt 2 又把 `worker_name` 错读为 `name`。两次均
  未停止 Python Worker、切换 maintenance、创建模型、启动 Rust Worker、创建 Docker/GPU
  workload 或推理。attempt 3 首次进入维护窗口并启动了真实 `--managed-file-only`，但
  readiness 轮询第一次 `curl` 连接拒绝被 `set -e` 提前解释为失败；attempt 4 修复重试语义后
  发现 Rust Worker 只监听冻结的 `10.213.80.90`，而执行器仍探测 `127.0.0.1`，最终按超时门失败；
  attempt 5 修正地址后又发现启动门错误要求 `state=ready`，而 maintenance 开启时 Server 的合法
  状态是 `state=maintenance`；attempt 6 首次通过启动门并解除 maintenance，但 Rust CLI 的
  `ModelCreate` 未携带手工 GPU 选择所需的目标 Worker `cluster_id`，Server 以 HTTP 400 拒绝。
  attempt 7 已证明 D141 修复能创建正确集群的 Model/Instance，但目标 Worker 的健康上报仍携带
  非空受限模式消息，Server 将其保留为 `not_ready`。attempt 8 进一步证明 D143 已使 Worker
  通过 READY 门，但调度器随后因 Rust 上报 `system_reserved=null` 在访问 `.vram` 时失败，实例
  仍停在 `pending`。attempt 9 已把实例调度到 Worker ID 5，但 Rust 把 Server 的默认
  `subordinate_workers=null` 占位对象误判为跨 Worker推理，实例停在 `scheduled`。attempt 10
  已真实证明完整 Managed ModelInstance/ModelFile 状态链可到达 `starting`，且通过 60 秒无
  runtime/Docker/GPU 隔离门；但验收脚本错误拒绝 Server 合法的 `cleanup_on_delete=null`，正式
  cleanup 失败，因此本阶段仍未正式通过，也未进入 M6-B。
- 已接受修复：代码检查点 `208ac76dfd78beca4c697afd05235e0c974bdd8d` 让 deploy 先唯一
  解析目标 Worker、要求其具有集群归属，并将 `cluster_id` 写入 `ModelCreate`；CLI 参数和
  调度范围不变。90 上 `cargo test --locked --offline --workspace` 的 150 个测试全部通过，相关
  crate 的全 target Clippy 以 `-D warnings` 通过。
- 当前停止位置：attempt 7 为 `failed_during_window`，证据目录的 `manifest.sha256` 文件
  SHA-256 为 `81e34f127d853eaf8bd5e326e071156a3fd539f958c08ab82371aecc48d0a43e`；清理与恢复
  审计通过，没有 ModelFile、端口、Docker/GPU workload 或推理残留。
- 已接受修复：代码检查点 `85d6b8f478de2b096e35d0c22ddaa8ab1768fa39` 让
  `managed-file-only` 的健康报告清空显式 NOT_READY 消息，同时保持 `resource-api-only` 不可
  调度和注册 maintenance 门。90 上 150 个 workspace 测试、全 workspace/all-targets Clippy
  和格式门全部通过。
- attempt 8 的只读预检与预窗口故障注入已通过，清单文件 SHA-256 分别为
  `2acf1400baea3948a5e823ea91100c8728431d500794cd7822053f104b5ff777` 和
  `e94f833beceb96bf3ba881cc4619803d31ba9cf42dc35650f07b8679fa732262`；现有服务与资源未改变。
- attempt 8 已作为有效 `failed_during_window` 封存：Model 11/Instance 26 未被分配 Worker，
  没有 ModelFile、端口、容器、GPU workload 或推理；`cleanup_ok=1`，全部运行状态恢复。证据在
  安全脱敏后重新封存，清单文件 SHA-256 为
  `efdf1c89566cf9936bdb8bf066b66c262367e04065e2246b9eb487ea4a7bb735`。
- 已接受修复：代码检查点 `8b06832440f222600721171f8d1ede7e9bab4004` 新增强类型
  `SystemReserved`，让注册和周期状态都明确上报 `{ram: 0, vram: 0}`。90 上 153 个 all-target
  测试、all-features/all-targets Clippy、rustfmt 和 workspace build 全部通过。
- attempt 9 的只读预检和预窗口故障注入已通过，清单文件 SHA-256 分别为
  `c1e7fcbdea35d2a142c8259a5abc724e8a35e414fcfaf0ac8907d270d6d2fb1d` 和
  `1cdb433cf52ba63d681b79ed39cffabd371511eefbb6deb79d1095fa86bf4743`；`Config.Env` 未进入
  故障注入证据，现有服务与资源未改变。
- attempt 9 已作为有效 `failed_during_window` 封存：Model 12/Instance 27 在 Worker ID 5 上
  达到 `scheduled`，但没有 ModelFile、端口、容器、GPU workload 或推理；`cleanup_ok=1` 且独立
  恢复审计通过。正式/Watchdog 清单文件 SHA-256 分别为
  `b582cea03604a926b480e6714c400564345feb13ed86e8aafd9152929683f5de` 和
  `1de3bd24e1cada13fbf2978c21cdb43b230cb3a7f32b6f2972a1a10652eb06a0`。
- 已接受修复：代码检查点 `50bc840b5b00cdedbde4cd3e32676054f1cc91e6` 只在
  `subordinate_workers` 非空时判定跨 Worker推理，默认 null/空列表仍按单 Worker处理，畸形结构
  保守拒绝。90 上 156 个 all-target 测试、all-features/all-targets Clippy、rustfmt 和 workspace
  build 全部通过。
- attempt 10 的只读预检与预窗口故障注入已通过，清单文件 SHA-256 分别为
  `9cd18a8398d0e4b940456df7b2b4405a11020149d1c8a065ce6840f3015bfde7` 和
  `cc7403475b5646bf09f30b01ce0647df09559a4f5c85434f13bb61ad26f89086`；`Config.Env` 未进入
  注入证据，现有服务与资源未改变。
- attempt 10 的核心 smoke 通过但正式结果为 FAIL：正式/Watchdog 清单文件 SHA-256 分别为
  `fb4c1a8412498e9dbd31484f542d64605293101c2f52b6a92aa7404a091c9c76` 和
  `44671e99ac99883e8b8489d52a89be2f0f92a6b20ed75b1213a5512c2e912ad8`。独立恢复审计清单
  SHA-256 为 `f8b95648a83601d3b34dee451e8e752f5c5b9b40a8df3cd966bb6fb71e190466`，证明唯一残留
  ModelFile 已精确删除且服务、资源、Docker、端口和 GPU 均恢复基线。
- attempt 11 的只读预检与预窗口故障注入已通过，清单文件 SHA-256 分别为
  `ec5cb0c271285f79e243bc8b8ac699d149dd86f1e50d7f667bb936925b24b1d7` 和
  `c4d7841b520c4823bf056c56085b0496a2d9d080cb9355d66b489874a182a2f2`；现有服务、资源、
  maintenance、端口与 GPU 未改变。
- attempt 11 已正式通过并由独立审计确认恢复：Model 14/Instance 29/ModelFile 8 完成
  `starting` + ready 复合状态门和 60 秒无 runtime 隔离，`smoke_passed=1`、`cleanup_ok=1`。
  正式、Watchdog 和独立审计清单文件 SHA-256 分别为
  `737d35976a595cadc3f21fdff8e67e603656e43ef5bd95cb6d5ab046f2e64a3a`、
  `768cd133fbf7d026c7d684f36c5dfd3ae165e4c826f05ca96ab9472eebebaa0b` 和
  `2dffa63f45943d0b630d5f4e1d41a5fcc8988d5a9b7ddcccdaf7c39aae930613`。测试资源、Docker ID、
  maintenance、Python Worker、端口和 GPU 全部恢复基线，M6-A.3.1 更新为 `accepted`。
- 当前停止位置：M6-B 尚未开始。下一项允许执行的工作是共同讨论并冻结 Docker/vLLM runtime
  启动、GPU 隔离、推理验收与回滚方案；本次授权不能复用于创建 workload 或执行推理。

环境基线见 `environment-baseline.md`，实时停止位置与准确 commit/证据 identity 以
`execution-log.md` 为准。
