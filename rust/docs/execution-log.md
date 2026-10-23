# GPUStack Rust 重写：执行与交接记录

## 文档职责

本文记录已接受决策、关键执行证据、当前状态与下一项允许执行的工作。聊天仍是主要交互渠道。大段命令输出与运行时 artifact 只记录其 identity 和路径，不直接嵌入本文。

## 已冻结决策

- 2026-10-17：接受 R1 范围、非目标、CLI-first 原则、Managed/Lab 边界、Worker 协议界面、协调模型、Git 策略、部署拓扑、验证门禁与回滚方法。
- 2026-10-17：Worker 容器化被纳入 R1 发布门禁；宿主机进程运行仍保留为开发阶段。
- 2026-10-17：最终公开 CLI 统一为 `gpustack`，迁移期二进制为 `gpustack-rs`。更早提出的最终独立 `gpustack-worker` 公开命令已被取代。
- 2026-10-17：用户选择让 90 主机使用账号级 GitHub SSH 权限，而不是仓库级 deploy key。
- 2026-10-17：用户选择以中文维护 `rust/docs/` 内部治理与实验文档；技术标识保留英文。顶层 MkDocs `docs/` 若发布相关内容，单独编写英文版。
- 2026-10-19：用户把代码注释设为硬门禁，并要求每个实现阶段在请求验收前直接在聊天中交付详细实现报告；两项要求追溯应用于尚未接受的 M3-B。
- 2026-10-19：用户澄清代码门禁要求的是关键必要注释而非逐项公开文档；取消 `missing_docs = "deny"`，并把注释过量同样列为门禁失败。阶段直接汇报要求不变。
- 2026-10-20：纠正 Managed 状态机设计：本地模型也保留 ModelFile 控制链和 `DOWNLOADING`，Lab 使用独立本地状态；M3-D 前新增 M3-C.2 ModelFile 离线协议阶段。
- 2026-10-20：冻结 M3-D control-plane-only、maintenance 加空 GPU 双重隔离、独立 health 端口、两段式执行门禁与身份保留方案；方案接受不授权真实 Server 写入。
- 2026-10-21：冻结 M3-D.2 操作手册；用户亲自分段执行，助手只提供命令、解释并审阅非敏感输出。证据保存在 Git 外 `0700` 目录，token 永不进入聊天或证据；首次注册一旦尝试，无论结果都删除本地 Cluster token 副本。
- 2026-10-23：冻结 M6-A 状态边界、Worker Managed 端口责任、本地 ModelFile resolver、
  写前刷新、CLI 前移、maintenance 运维命令和四段门禁；接受只解锁离线 M6-A.1。
- 2026-10-24：发现正式 `worker run` 尚未接入 M6-A.1 协调器；冻结 D092–D094，在
  M6-A.2 与真实 smoke 之间增设 M6-A.2.1 离线生产入口接线门禁。

## 当前状态

- 状态：`ready`
- 当前阶段：M5-D.0、M5-D.1、M5-D.2 和整个 M5-D 已接受；M5-E.0、D082 与
  M5-E.0.1 已接受。Qwen-7B 的一次无效端口尝试和一次正式不兼容失败保持原分类；D084
  改选 Qwen2.5-0.5B-Instruct、D085 清理与回滚修复完成后，新模型 M5-E.1 正式 smoke
  已通过全部运行、推理、清理和恢复门禁。用户审阅详细报告后已接受 M5-E.1、M5-E 和
  整个 M5。M6-A.0 设计已接受，D086–D091 已冻结。用户已审阅并接受 M6-A.1 和
  M6-A.2；M6-A.2 的接受范围仍限于离线 CLI 实现、mock 契约和构建门禁。用户已接受
  D092–D094。用户已审阅 D033 详细实现报告并接受 M6-A.2.1；接受范围仅为正式
  managed-file-only 入口、离线 fake/mock 契约和构建门禁。
- 分支：`codex/rust-worker`；M5-D.0 代码检查点为
  `0fbc8d14b1792210e52aab782f92b15bc1fd9a32`，接受前治理基线为
  `3f02c644d5928c7ae54ae8ca697d79a483dfe115`；M6-A.2 实现提交为
  `aff61fe2c7e98c50d3b5d273f6fbc582877a633f`；M6-A.2.1 实现提交为
  `e1e3ee4a662099c1acfc60a8cfae0c8129b7d762`，CLI 帮助补充提交为
  `dc53b9aedd6465a0564ecc07aeda03b1cf0c70fb`。
- 检查点前已存在且与本项目无关的工作树状态：未跟踪目录 `output/`；本项目不修改它。
- 90 主机身份：`wujiaxing2@r760xa`；仓库路径：
  `/home/wujiaxing2/workspaces/gpustack`；账号级 SSH 已认证 fork。
- 实现状态：M1～M4、M5-A、M5-B、M5-C、整个 M5-D、M6-A.1 和 M6-A.2 已接受。
  M6-A.2 提供 Context、最小 Model CRUD/status、精确 Worker maintenance CLI、稳定
  table/JSON、确认、退出码、状态跟踪与凭据文件安全读取；接受依据仍只有离线 mock 证据。
  M6-A.2.1 正式 managed-file-only CLI/supervisor 接线已经用户接受。
- 当前停止位置：本阶段只在 90 主机工作树执行 locked/offline 编译与测试，没有启动 Rust
  Worker、读取真实 Worker token、连接 Server API、改变 maintenance、停止 Python Worker、
  访问 Docker socket、创建容器或使用 GPU。M5 完成时的运行环境结论是历史接受证据，
  本阶段未重新探测或刷新它。
- 下一项允许执行的工作：只讨论并冻结 M6-A.3 的独立只读预检、真实无 Docker smoke、
  维护窗口、证据和回滚 runbook。真实 Server 写入、maintenance 变化、停止 Python
  Worker、Docker 或 GPU 操作仍须另行明确授权，不能因 M6-A.2.1 已接受而自动执行。

## 里程碑

### 2026-10-17：M0 设计冻结并归档

- 目的：在实现前，把已接受讨论整理成可审计、可恢复的项目契约。
- 结果：已记录 charter、架构、CLI、Worker 协议、实现计划、决策与交接状态。Git 检查点后状态为 `ready`，可以进入 M1 只读基线盘点。
- 证据：`rust/docs/`；本地 base commit `76dc841916ff8fed4068a1ab99d178d2e27a4a06`；初始设计 commit `74ef8977`。
- 解释：架构与验收规则已经冻结。这不证明任何远程环境事实或实现能力。
- 无效或非正式 artifact：此前聊天中的示例和未验证命令草案只用于解释；当前没有 Lab 或 Managed 运行结果。
- 新停止位置：没有远程修改，也没有 Rust 实现。
- 下一项允许执行的工作：M1 只读检查和 Git 认证设置；创建 runtime 代码前必须先记录验证过的身份信息。

### 2026-10-17：治理文档切换为中文

- 目的：使长期内部治理与实验记录更适合当前协作语言，同时保持技术接口含义稳定。
- 结果：`rust/docs/` 的八份文档转换为中文，命令、API、类型、状态和代码标识符保留英文；状态为 `ready`。
- 证据：`rust/docs/`；翻译 commit `78d46176`。
- 解释：这是文档表达方式变更，不改变已冻结的 R1 scope、架构、CLI 契约、协议或验收标准。若以后发布到顶层 MkDocs `docs/`，另行编写英文版本。
- 无效或非正式 artifact：被翻译取代的英文工作副本不再是内部权威版本；初始 Git commit 仍用于审计历史。
- 当前停止位置：M0 已完成，Rust 实现与远程部署均未开始。
- 下一项允许执行的工作：建立可写 fork 和 90 主机 Git 连通性，然后执行 M1 只读盘点；任何部署变更仍需等到明确维护窗口。

### 2026-10-18：90 主机 Git 工作树建立

- 目的：在主要开发主机上建立可恢复、可推送且与冻结基线一致的源码工作树，为 M1 只读盘点提供可信起点。
- 结果：90 主机已使用账号级 SSH 密钥认证 fork；工作树位于 `/home/wujiaxing2/workspaces/gpustack`，`origin` 指向 `GitHubcoincidence/gpustack`，`upstream` 指向上游 GPUStack 仓库。分支、HEAD、基线祖先关系、fetch 与 push dry-run 均通过验证。
- 传输证据：bootstrap bundle 大小为 `220242551` 字节，SHA-256 为 `f700d34a01a817d23067ce848bb7d55e8b6cf050d88ef986a6eb24a4e0cc22ff`；Windows 与 90 主机哈希一致。`git bundle verify` 确认它包含完整历史，唯一分支头为 `d3623bfbc927c3ca0be677a6667600494d3821c6 refs/heads/codex/rust-worker`。
- Git 证据：90 主机 `HEAD` 为 `d3623bfbc927c3ca0be677a6667600494d3821c6`；`76dc841916ff8fed4068a1ab99d178d2e27a4a06` 存在且是其祖先；`git fetch origin codex/rust-worker` 成功；`git push --dry-run origin HEAD:refs/heads/codex/rust-worker` 返回 `Everything up-to-date`。
- 身份证据：90 主机为 `wujiaxing2@r760xa`；专用公钥指纹为 `SHA256:cK0QnBbtpAsV+HUOcY3kAjQgOI8+UNiNgxWYvAT9XiU`。记录只包含公钥指纹，不包含私钥或口令。
- 无效或诊断尝试：90 主机直接通过 GitHub SSH clone，以及经本机 VPN 反向隧道进行的 SSH/HTTPS clone，吞吐均不足以作为初始大仓库传输路径；未完成的目标目录在 bundle clone 前已不存在。这些尝试只说明大流量链路性能不足，不说明 Git 身份或仓库内容异常，也不作为功能验收证据。
- 解释：初始大仓库传输改用本地生成、哈希校验后复制到 90 主机的 Git bundle；日常小规模 fetch/push 仍使用 GitHub。该选择只改变源码引导方式，不改变架构、分支策略或发布流程。
- 当前停止位置：可信 Git 工作树已经就绪；Rust 实现、系统盘点和部署变更均未开始。
- 下一项允许执行的工作：分批执行 M1 只读盘点，首先采集 OS、CPU、内存、磁盘、GPU、driver、Docker 与 NVIDIA Container Toolkit 身份。

### 2026-10-18：M1 环境基线接受

- 目的：以不修改现有部署的方式固定 Rust Worker 实现依赖的实际 Server、主机、GPU、Docker、网络、Runner 和模型身份。
- 结果：M1 状态为 `accepted`，允许进入不使用 GPU 的 M2。完整基线见 `rust/docs/environment-baseline.md`。
- 兼容证据：部署版本为 GPUStack `v2.2.3 (1cfcbbf)`，image digest 为 `sha256:55ecc762950a32641b6a4d96195c0662ef72998eaeeda31bba0e8e218d41afef`；上游 tag 与 image revision 均解析到 `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`。
- 硬件证据：两张 NVIDIA A100 80GB PCIe，driver `580.178.04`，拓扑 `NV12`；采集时两卡均无进程和显存占用。
- runtime 证据：Docker `29.1.3`、NVIDIA Container Toolkit `1.20.0`；`gpustack` 和 `gpustack-worker` 已运行三天且 restart count 为 0。Worker 使用 host network、NVIDIA runtime、privileged、共享数据 volume 和 Docker socket。
- 网络证据：Server、Worker `/healthz` 与 Worker metrics 均返回 200；Server 容器访问宿主机 Worker API 返回 200，因此 direct-network 方向成立。
- Runner 与模型证据：固定本地 Runner digest 为 `sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af`；首选模型为当前用户拥有的完整 Qwen-7B 目录，identity hash 已归档。
- 工具链证据：90 主机尚未安装 `rustc`、`cargo` 或 `rustup`；这是 M2 的第一个显式动作，不是隐含依赖。
- 无效或诊断尝试：90 主机直接 fetch `v2.2.3` tag 因 GitHub 大对象链路过慢被主动中止。随后使用只含一个增量 commit 的 2217-byte bundle 导入，双端 SHA-256 一致并通过 `git bundle verify`。历史 vLLM 容器和旧 v0.2.2 smoke 只作为诊断信息，不作为固定 Runner 兼容证据。
- 安全边界：没有读取 token 值，没有输出完整 container env，没有运行原始 `docker inspect`，也没有启动、停止、重启或删除容器。
- 当前停止位置：M1 已接受，Rust workspace 尚未创建，现有 Python Server/Worker 未改变。
- 下一项允许执行的工作：M2 安装 pinned Rust toolchain 并建立 CLI/workspace 骨架；实际 GPU smoke 仍需后续明确维护窗口。

### 2026-10-18：M2-A Rust 工具链门禁接受

- 目的：在创建 Rust workspace 前，为 90 主机建立可复现且可验证的编译、格式化与 lint 工具链，同时确认安装过程不影响现有 GPUStack。
- 结果：状态为 `accepted`。rustup `1.29.1` 已安装，默认 toolchain 为 `1.99.1-x86_64-unknown-linux-gnu`；`cargo`、`rustfmt` 与 Clippy 均可用。M2 整体仍为 `running`，因为 workspace 和 CLI 骨架尚未创建。
- 完整性证据：官方 rustup-init SHA-256 `dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71` 与下载文件一致；rustc 为 `1.99.1 (58cf8f9c3 2026-10-13)`，commit `58cf8f9c35b355199727627d07ae34a4e2846d67`，LLVM `23.1.1`；Cargo 为 `1.99.1 (5f94df478 2026-08-27)`。
- 组件证据：minimal profile 已安装 `cargo`、`clippy`、`rust-std`、`rustc` 与 `rustfmt`；`rustfmt --version` 返回 `1.10.0-stable`，`cargo clippy --version` 返回 `0.1.99`。
- 传输证据：临时 SSH 反向隧道把 90 主机 `127.0.0.1:17897` 转发至开发机本地代理 `127.0.0.1:7897`。1 MiB probe 返回 HTTP 206，平均 `1096104 B/s`；剩余 19.1 MiB 断点续传平均约 `5664 KiB/s`。安装后隧道已关闭，远端 `17897` 无监听。
- 隔离证据：安装后 Git 仍位于 `1df210c1997e536fd4c1d610de3bd3c9cd7e3d03` 且工作树干净；Server `/healthz`、Worker `/healthz` 和 Worker metrics 均返回 200；`gpustack` 与 `gpustack-worker` 容器继续保持运行。
- 解释：工具链门禁已经解锁 M2 的仓库实现部分，但不证明任何 Rust Worker、CLI、Docker runtime、vLLM 或 Managed 能力。工具链下载链路只是环境准备证据，不是产品性能证据。
- 非正式或无效尝试：90 主机直接下载 rustup-init 仅约 20 KiB/s，被主动中止并保留部分文件；随后经已验证代理断点续传。rustup-init 被重复执行一次，但安装器幂等，未改变固定版本或验收结果。
- 当前停止位置：工具链可用，仓库仍无 Rust workspace，现有 Python Server/Worker 未改变。
- 下一项允许执行的工作：M2-B 添加仓库级 `rust-toolchain.toml` 和四 crate workspace，先实现可编译的 `gpustack-rs` CLI 骨架；不得启动 vLLM 或占用 GPU。

### 2026-10-18：M2 Rust workspace 与 CLI 骨架接受

- 目的：建立可重复构建的四 crate Rust workspace 和 CLI-first 基础契约，为后续 Server、Worker 与 runtime 功能提供配置、输出、错误、signal 和测试边界。
- 结果：M2 状态为 `accepted`。实现提交为 `c5bbd01e7ab5e113d90b88c46afcf074d57d52a2`（`feat(worker): add rust workspace and cli foundation`）。workspace 包含 `gpustack-cli`、`gpustack-client`、`gpustack-worker` 与 `gpustack-runtime`，迁移期二进制为 `gpustack-rs`。
- 工具链与依赖证据：`rust/rust-toolchain.toml` 固定 Rust `1.99.1`、Clippy 与 rustfmt；`Cargo.lock` SHA-256 为 `e423a7a819f2f16abb20e968c0d40439f9ce1824c9fb3911322f2b5eed4bf1aa`。YAML 使用维护中的 `serde-saphyr 2.0.0`；首次解析发现 `serde_yaml 0.9.34` 已标记 deprecated 后，在提交前移除，锁文件不再包含它。
- 配置与安全证据：实现 `CLI > environment > YAML > defaults` 合并顺序；只接受 token file 引用，不提供内联 token 字段；配置错误不回显被拒绝的 secret；Server URL 拒绝 user info、query 与 fragment。相关单元测试均通过。
- CLI 证据：`gpustack-rs version` 支持 table/JSON，并在干净提交构建中报告 `git_commit: c5bbd01e7ab5`、target `x86_64-unknown-linux-gnu` 和 toolchain `1.99.1`；`worker validate-config` 验证默认配置；无效 `file://` Server URL 返回稳定配置退出码 10。
- Doctor 证据：只读 smoke 使用 Docker socket、隔离开发目录与首选 Qwen-7B 模型路径，JSON 报告 `healthy: true`；不存在的数据/日志目录及尚未配置的 token file 明确标记为 warning。该命令只检查元数据，没有调用 Docker Engine、访问 GPU、启动容器或连接 Server。
- 自动化门禁：`cargo fmt --all -- --check`、`cargo test --workspace --all-targets --locked --offline`、`cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` 与 `cargo build --workspace --locked --offline` 均通过；共 6 个单元测试通过，0 失败。
- 隔离证据：依赖首次获取临时复用 `127.0.0.1:17897` 反向代理，完成后已关闭且端口无监听。Server `/healthz`、Worker `/healthz` 与 Worker metrics 在实现验证后仍返回 HTTP 200，现有容器未停止或修改。
- 解释：M2 只证明 workspace、CLI 基础和本地只读诊断可用，不证明 Server 协议兼容、Worker 注册、GPU 盘点、Docker runtime、vLLM、Lab 或 Managed 流程。
- 当前停止位置：M2 已完成并接受，Rust Worker 尚未连接真实 Server，现有 Python Worker 保持运行。
- 下一项允许执行的工作：M3-A 从已冻结 GPUStack v2.2.3 源码与只读响应中建立版本、注册、heartbeat、状态、watch/full-sync 和 probe fixture；fixture 经审阅接受后才开始 M3 写路径实现。

### 2026-10-19：M3-A Server 控制面契约待审阅

- 目的：在实现 Rust HTTP client 和任何真实写路径前，固定已部署 GPUStack v2.2.3 的 endpoint、认证、schema、分页、watch framing、事件、错误和兼容身份。
- 状态：`ready_for_review`，尚未标记 `accepted`。契约见 `rust/docs/server-contract-v2.2.3.md`，样本与来源说明见 `rust/fixtures/server-v2.2.3/`。
- 实现检查点：`cff7c1fca8272d02600f32333f6bd0b6aa7c5408`（`test(client): freeze v2.2.3 server contract fixtures`）。
- 源码证据：tag `v2.2.3` 和部署 revision 都解析到 `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`。检查了 routes、schemas、Python Worker manager/client、pagination、event bus 与 streaming 实现；未 checkout 或修改固定源码。
- 真实只读证据：公开 `GET /version` 返回 `{"version":"v2.2.3","git_commit":"1cfcbbf"}`；未认证 `GET /v2/workers` 返回 401 与标准 `ErrorResponse`。部署 OpenAPI 为 `546580` 字节，SHA-256 为 `acb247255add4a162bb4d886a1bd659b38d265f8f03c827e16076cd417f3314a`。
- synthetic 证据：在现有 v2.2.3 Server 容器内只实例化 Pydantic model 并使用 FastAPI encoder，生成 registration、status、identity、model/model-instance full sync、ModelInstance update 与 watch event 样本。样本只含保留测试 identity 和 `<redacted-worker-token>`，没有数据库查询、认证请求或真实 secret。
- 关键契约：prefix 为 `/v2`；register 成功返回 `WorkerRegistrationPublic`；heartbeat/status 成功为 204；`page=-1` 是不分页权威列表；watch 非空行是 JSON event，事件 type 为数字，建连回放 CREATED，15 秒空白 keepalive，DELETED 允许 ID-only data；ModelInstance PUT 是完整对象而不是 patch。
- OpenAPI 偏差：`worker-status` 和 `worker-heartbeat` 被显式排除；`POST /v2/workers` 的重复 router 挂载导致成功响应在 OpenAPI 显示空 schema。注册响应以 route 装饰器和 Python client 类型为准。
- 安全与隔离：没有读取 registration/Worker token 文件，没有发送 POST/PUT/PATCH/DELETE，没有停止或重启容器，没有访问 GPU，也没有启动 vLLM。Server、Python Worker 与 metrics 健康检查在开始时仍为 200。
- 漂移审计：R1 仍为 vLLM、本地 Docker、本地模型目录、单 Worker 单/双 A100、复用 Runner、CLI-first；未引入 K8s、跨 Worker 分布式、Python bridge 或额外推理后端。M3-A 只增加协议证据和离线测试，没有改变架构边界。
- 当前停止位置：fixture 等待共同审阅；它们固定 schema 形状但不冒充真实认证成功流量。
- 下一项允许执行的工作：接受 M3-A 后，在 mock transport 上实现 M3-B Rust Server client。第一次真实注册必须另行使用隔离 Worker identity，并将真实响应与 synthetic fixture 比较。

### 2026-10-19：M3-A Server 控制面契约接受

- 目的：把已共同审阅的 v2.2.3 协议事实升级为 M3-B 的正式实现输入，并明确真实写路径仍受后续门禁约束。
- 结果：用户接受 M3-A，状态由 `ready_for_review` 更新为 `accepted`。本次接受不改变 R1 scope、架构、Server 基线或安全边界。
- 证据：契约文档 `rust/docs/server-contract-v2.2.3.md`；fixture 目录 `rust/fixtures/server-v2.2.3/`；实现检查点 `cff7c1fca8272d02600f32333f6bd0b6aa7c5408`；治理检查点 `760de1df5758b8301cb3c881826ab2c6426edaf1`。契约测试 4 项、workspace 总计 10 项测试以及 fmt、Clippy、离线 locked build 均已通过。
- 解释：M3-B 可以把固定 schema、认证边界、分页和 watch framing 实现为 Rust 类型与客户端行为。synthetic 注册成功样本仍不是实际认证流量，不能作为真实集成通过证据。
- 安全边界：M3-B 首先只使用静态 fixture 和 mock transport；不得读取现有 token、向运行中的 Server 发送写请求、占用 GPU、启动 vLLM 或修改 Python Worker。
- 当前停止位置：M3-A 已接受，M3-B 尚未开始。
- 下一项允许执行的工作：实现 M3-B 类型、错误模型、Bearer 注入与脱敏、endpoint 请求、全量分页和 watch 解析，并以无网络测试验证；第一次真实注册必须另立隔离身份集成门禁。

### 2026-10-19：M3-B Rust Server client 待审阅

- 目的：把已接受的 v2.2.3 wire contract 实现为可替换传输层的类型化 Rust client，在连接真实 Server 前验证序列化、认证边界、错误处理和 watch framing。
- 状态：`ready_for_review`，尚未标记 `accepted`。实现检查点为 `fed728afc82c4f4f5d5f5d21e0800a73a61c3ea3`（`feat(client): add v2.2.3 control plane client`）。
- 实现范围：严格 request、容忍未知响应字段的 response、固定 Server version/commit 门禁、不可在 `Debug`/`Display` 输出明文的 `SecretToken`、Bearer 注入、标准错误映射与 1024 字符未知响应上限、30 秒普通请求 timeout、45 秒 watch idle timeout、32 MiB buffered response 上限、`page=-1` full sync、ID-only delete 和 assignment 自检。
- endpoint 范围：`version`、`register`、`me`、`heartbeat`、`worker-status`、model/model-instance full sync 与 watch、完整 ModelInstance update 均已实现；watch 断开后的 backoff/full-sync 协调和 token 文件生命周期属于 M3-C。
- 依赖证据：新增直接依赖 `reqwest 0.13.5`（关闭 default features，启用 `rustls`、`json`、`stream`）和 `futures-util 0.3.34`；更新后的 `Cargo.lock` SHA-256 为 `b1708691bb817145f3dc5742865b22fc081feb252134de21263e4eaf9ba3b3d8`。这是待本阶段审阅的依赖选择，不改变 Server、Runner 或部署基线。
- 自动化证据：`gpustack-client` 共 12 项测试通过，workspace 共 17 项测试通过；`cargo fmt --all -- --check`、workspace Clippy `-D warnings`、离线 locked build、`git diff --check` 和 staged secret scan 均通过。
- 隔离证据：测试只使用静态 fixture 和 mock transport；没有读取现有 token，没有向 Server 发送真实写请求，没有访问 GPU、启动 vLLM、修改 Docker 工作负载或更改 Python Worker。完成后 Server `/healthz`、Worker `/healthz` 与 Worker metrics 均为 HTTP 200。
- 诊断或非验收 artifact：90 主机直接更新 crates.io 索引长时间无进度后被中止；确认并终止遗留的旧 Cargo 进程后，经临时反向代理完成依赖获取。隧道已关闭，远端 `127.0.0.1:17897` 无监听；下载链路不属于产品性能证据。
- 漂移审计：实现仍只面向 v2.2.3、vLLM、本地 Docker、本地模型目录与单 Worker；未加入 K8s、跨 Worker 分布式推理、Python bridge 或额外后端。真实写入门禁未被放宽。
- 当前停止位置：M3-B 已实现并推送，等待共同审阅；M3-C 与真实集成都未开始。
- 下一项允许执行的工作：接受 M3-B 后实现 M3-C 原子 token 持久化、注册状态机、heartbeat/status/watch/full-sync 控制循环与 mock 重连；第一次真实注册仍需另行批准隔离身份。

### 2026-10-19：代码注释与阶段直接汇报门禁生效

- 目的：把“代码需要注释”和“用户需要知道每阶段具体实现”从协作偏好升级为可审计、可阻止验收的项目规则。
- 状态：`ready_for_review`。该变更由用户明确提出并接受，不属于未授权 scope 漂移；代码与文档构建门禁已经完成，M3-B 的直接实现报告随本次聊天交接提供，M3-B 本身仍等待用户共同审阅。
- 代码门禁：workspace 使用 `missing_docs = "deny"`，crate、module、公开 type/trait/function/method/constant/field 缺少英文 Rustdoc 时编译失败；安全边界、状态转移、协议偏差、恢复和非直观算法还需英文行内注释解释目的与原因。逐行复述、失真、过期或修改历史式注释不合格。
- 验证门禁：阶段检查加入 `cargo doc --workspace --no-deps --locked --offline`，并继续执行 fmt、Clippy `-D warnings`、workspace test、离线 locked build；人工审阅关键私有逻辑注释。功能测试通过不能覆盖注释门禁失败。
- 汇报门禁：每个阶段在请求验收前，必须直接在聊天中报告目标、功能、实现机制、数据流、模块、依赖取舍、证据、安全边界、限制、未实现项和下一步。长期文档只保存恢复所需摘要，不能代替直接报告。
- 漂移审计：R1 后端、Docker runtime、本地模型、单 Worker 单/双 GPU、Server v2.2.3、Runner、Managed/Lab 与容器化发布门禁均未改变；本次只收紧代码质量与验收顺序。
- 验证证据：`cargo fmt --all -- --check`、workspace Clippy `-D warnings`、17 项 workspace test、离线 locked build 和 `cargo doc --workspace --no-deps --locked --offline` 全部通过。公开 API 漏写 Rustdoc 现在会在编译阶段失败。
- 当前停止位置：注释与文档构建门禁已补齐；等待本次聊天交付 M3-B 详细实现报告后由用户共同审阅，M3-C 尚未开始。
- 下一项允许执行的工作：交付 M3-B 详细实现报告并等待审阅；未收到接受结论前不得把 M3-B 标记为 `accepted` 或开始 M3-C。

### 2026-10-19：注释门禁密度校正

- 目的：纠正把“关键必要注释”扩大为“每个公开项都写 Rustdoc”的过度实现，恢复代码本身作为主要表达媒介。
- 决策：D034 修订 D032。取消 workspace 的 `missing_docs = "deny"`；保留模块职责以及安全、协议、状态、所有权、恢复和非直观逻辑的说明，删除简单字段、枚举值、CLI 参数和显然方法上的模板化注释。
- 验收方式：注释门禁改为人工双向审阅——关键位置缺少原因说明不通过，注释数量过多、逐行复述、过期或遮挡代码同样不通过。fmt、Clippy、test 和 locked offline build 自动门禁保持不变。
- 影响：只改变注释密度与门禁实现方式，不改变 M3-B 功能、Server v2.2.3 契约、依赖、测试 fixture、R1 范围或安全边界。D033 阶段直接实现报告继续有效。
- 验证证据：精简后 `cargo fmt --all -- --check`、workspace Clippy `-D warnings`、17 项 workspace test、离线 locked build 和按需 `cargo doc` 均通过；功能代码和 `Cargo.lock` 未改变。
- 当前停止位置：注释密度校正已完成，等待提交推送后继续进行 M3-B 审阅。
- 下一项允许执行的工作：继续等待 M3-B 审阅；不得开始 M3-C 或真实 Server 写路径。

### 2026-10-19：M3-B Rust Server client 接受

- 目的：在进入 Worker 身份和控制循环前，正式确认类型化 v2.2.3 client 的协议覆盖、安全边界、依赖选择、离线证据和注释质量。
- 结果：用户在收到 M3-B 详细实现报告及注释密度校正结果后确认无问题，M3-B 状态由 `ready_for_review` 更新为 `accepted`。
- 实现证据：client 实现检查点 `fed728afc82c4f4f5d5f5d21e0800a73a61c3ea3`；注释与治理校正检查点 `8287ceb0af461d291c1e4faf780715acd862d7d9`；`Cargo.lock` SHA-256 为 `b1708691bb817145f3dc5742865b22fc081feb252134de21263e4eaf9ba3b3d8`。
- 验证证据：`gpustack-client` 12 项、workspace 17 项测试全部通过；fmt、Clippy `-D warnings`、locked offline build、按需 Rustdoc、diff 检查和 secret scan 均通过。Server、Python Worker 和 metrics 健康检查均为 HTTP 200。
- 解释：接受结果只证明固定 fixture/mock transport 下的 v2.2.3 client 行为，不证明 token 文件生命周期、Worker 控制循环、真实认证写流量、Docker、GPU 或 vLLM 能力。
- 安全边界：没有读取现有 token，没有向 Server 发送写请求，没有修改 Docker 工作负载、GPU 状态或 Python Worker。第一次真实注册仍需隔离 Worker identity 和单独批准。
- 漂移审计：M3-B 与 R1 的 vLLM、本地 Docker、本地模型、单 Worker、Server v2.2.3 和 CLI-first 范围一致；D034 只校正注释密度，不改变实现行为或证据解释。
- 当前停止位置：M3-B 已接受，M3-C 尚未开始。
- 下一项允许执行的工作：实现 M3-C 离线身份持久化与控制循环，并以 mock 验证原子写入、恢复、重连、backoff 和 full sync；不得连接真实写路径。

### 2026-10-19：M3-C 身份持久化与控制循环待审阅

- 目的：在不连接真实 Server 的前提下，实现 Worker 注册/恢复身份和长期控制面循环，使下一阶段真实集成不再依赖临时代码。
- 状态：`ready_for_review`，尚未标记 `accepted`。代码检查点为 `5c7c69cc31774cbc1aa0dac8d600ec8971d938b3`（`feat(worker): add identity and control loops`）。
- 身份范围：注册凭据与 Server 签发的 Worker 系统凭据分离；后者固定在 `<data_dir>/managed/worker-token`，使用私有目录、`0600` 文件、同目录临时文件、`fsync` 和 hard-link no-replace 发布。启动时优先恢复持久身份并调用 `me` 验证；已存在凭据无效时失败关闭，不自动重新注册。
- 控制循环范围：heartbeat 与状态上报独立运行；期望状态由启动/重连/周期 full sync 与 model/model-instance watch 共同维护。仅保留当前 Worker 的 ModelInstance；断流、未知事件或请求失败时保留最后一次权威状态，以指数 backoff 重连并在重新 watch 前 full sync。
- 依赖证据：Worker 新增 workspace 内 `gpustack-client` 依赖；Unix 目标直接声明已在锁文件中的 `libc` 以使用 `O_NOFOLLOW`；Tokio 启用 `sync`；`serde_json` 仅用于测试。`Cargo.lock` SHA-256 为 `af566acb9215d5601dd8bec1282eb93c133b521c4b946870e6762b88409d5019`。
- 自动化证据：workspace 共 25 项测试通过，其中 Worker 11 项；fmt、all-features Clippy `-D warnings`、locked/offline test、locked/offline build、Rustdoc `-D warnings`、diff check 和 staged secret scan 均通过。新增测试覆盖原子凭据往返、权限与覆盖拒绝、身份恢复、首次注册、持久凭据失败关闭、慢状态请求不阻塞 heartbeat、断线 full-sync 恢复和 assignment 移出。
- 隔离证据：全部测试使用临时目录、静态 fixture 和 mock API；没有读取现有 token，没有真实注册、heartbeat 或状态写入，没有操作 Docker、GPU、vLLM 或 Python Worker。
- 环境核验：完成后 Server `/healthz`、Python Worker `/healthz` 与 metrics 均为 HTTP 200，`gpustack` 和 `gpustack-worker` 容器保持运行；此前未归因的远端 `17897` 监听此时已不存在，本阶段没有停止或修改它。
- 漂移审计：实现继续固定 Server v2.2.3，未加入 K8s、跨 Worker 分布式推理、其他推理后端、Python bridge 或 runtime 容器操作。注释仅保留在凭据暴露边界、协议偏差和失败恢复等关键位置。
- 当前停止位置：M3-C 代码和治理摘要已完成，等待聊天中的详细实现报告与用户共同审阅。
- 下一项允许执行的工作：M3-C 接受后讨论并审批隔离 identity 的真实 Server 集成；在获得单独批准前不得发送真实写请求。

### 2026-10-19：M3-C 身份持久化与控制循环接受

- 目的：在进入真实 Server 写路径前，正式确认 M3-C 的身份生命周期、控制循环、安全不变量、依赖选择和离线验证证据。
- 结果：用户在收到逐项实现报告后确认无问题，M3-C 状态由 `ready_for_review` 更新为 `accepted`。
- 实现证据：代码检查点 `5c7c69cc31774cbc1aa0dac8d600ec8971d938b3`；待审阅治理检查点 `0b0689b6f7e21f2d60113e37a693857223803c8c`；`Cargo.lock` SHA-256 为 `af566acb9215d5601dd8bec1282eb93c133b521c4b946870e6762b88409d5019`。
- 验证证据：workspace 25 项测试全部通过，其中 Worker 11 项；fmt、all-features Clippy `-D warnings`、locked/offline test、locked/offline build、Rustdoc `-D warnings`、diff check 和 secret scan 均通过。完成后的 Server、Python Worker 与 metrics 健康检查均为 HTTP 200。
- 解释：本次接受确认的是临时文件、静态 fixture 和 mock API 下的身份及控制状态机，不证明真实注册、真实 heartbeat/status、GPU 状态采集、Worker API、Docker、vLLM 或 Managed 模型流程。
- 安全边界：没有读取现有 token，没有向真实 Server 发送写请求，没有操作 Docker、GPU、vLLM 或 Python Worker。M3-C 接受不会自动授权 M3-D 的外部写操作。
- 当前停止位置：M3-C 已接受，M3-D 尚未开始。
- 下一项允许执行的工作：先共同确定隔离 Worker name/UUID、端口、数据目录、注册凭据来源、Server 可见性、清理和回滚步骤，再单独决定是否进入真实 Server 集成。

### 2026-10-20：M3-C.1 Server 权威身份纠偏启动

- 触发事实：复核固定 GPUStack v2.2.3 的 `gpustack/routes/workers.py` 后确认，新 Worker 注册会排除请求中的 `worker_uuid`、先以空值创建，再由 Server 生成唯一 UUID；`WorkerRegistrationPublic` 返回 Server UUID，而 `WorkerPublic`/`me` 不包含 UUID。Python Worker 在注册后保存响应中的 name 和 UUID。
- 漂移结论：M3-A synthetic 注册 fixture 直接序列化模型，未经过真实路由，因此错误地让请求与响应 UUID 相同；M3-C 随后把该等值关系当作安全断言，并在 token 恢复时使用本地请求 UUID。真实首次注册会安全失败，若移除断言又会在重启时恢复错误 UUID。
- 历史处理：M3-A 的 UUID 等值 fixture 语义和 M3-C 的对应身份断言标记为 `invalidated`；其余协议形状、M3-B client、M3-C heartbeat/status/full-sync/watch/reconnect 结论不受影响。此前接受记录保留，不静默改写历史。
- 已接受修正：新增权限受限的 `worker-identity.json`，持久化 Server 返回的 `id`、`name`、`worker_uuid`；先写身份元数据再写 token。元数据存在但 token 缺失时，用已保存的 Server UUID 重注册同一 Worker；token 存在但元数据缺失时 fail closed；二者都存在时以 `me` 核验 ID/name，并从元数据恢复 UUID。
- 门禁：先修正 synthetic fixture 和离线状态机，覆盖首次注册、正常重启、元数据先写后的恢复、token-only 拒绝、元数据损坏和身份不匹配；不执行真实注册，不读取现有 token，不操作 Docker、GPU、vLLM 或 Python Worker。
- 用户结论：用户已接受 M3-C.1 纠偏方案并授权离线实现；该授权不包含 M3-D 真实 Server 写路径。
- 当前停止位置：M3-C.1 状态为 `running`，M3-D 暂停。
- 下一项允许执行的工作：提交本纠偏检查点后实现 M3-C.1，并在聊天中交付详细实现报告供重新验收。

### 2026-10-20：M3-C.1 Server 权威身份修正待审阅

- 结果：代码检查点 `e04fda6562c1065491b7eb7478bb617f33d9d4f7` 已实现并推送。首次注册接受并持久化 Server 返回的 `id`、`name`、`worker_uuid`；正常重启用 token 调用 `me` 核验 ID/name；身份元数据存在但 token 缺失时，以保存的 Server name/UUID 重注册；token 存在但身份元数据缺失时在任何网络 I/O 前 fail closed。
- 磁盘语义：新增 `<data_dir>/managed/worker-identity.json`，格式版本为 1；身份元数据与 `worker-token` 共用拒绝符号链接、拒绝宽权限、16 KiB 上限、0600 文件、0700 目录、临时文件 fsync、hard-link no-replace 发布与目录 fsync。首次注册固定先发布身份元数据再发布 token，使两次写入之间的崩溃可恢复。
- 契约修正：注册 request fixture UUID 为 `...0001`，response 的 Server UUID 为 `...0002`，后续 status fixture 使用 `...0002`；client 和契约测试明确断言请求/响应 UUID 不同以及 status 采用响应 UUID。
- 自动化证据：Rust 1.99.1 下 `cargo fmt --all -- --check`、Clippy 全 workspace/all targets/all features `-D warnings`、全 workspace 30 个测试、离线 build 和 `RUSTDOCFLAGS='-D warnings' cargo doc` 全部通过。Worker 测试从 11 个增加到 16 个。`Cargo.lock` SHA-256 仍为 `af566acb9215d5601dd8bec1282eb93c133b521c4b946870e6762b88409d5019`。
- 依赖与注释审阅：`serde_json` 从该 crate 的 dev dependency 移至已有 workspace production dependency，没有新增第三方 crate 或 lockfile 变化。关键注释只覆盖 token-only 安全原因、两文件写入顺序和 atomic no-replace 发布，没有加入逐字段或逐行模板化注释。
- 环境证据：提交前 Server `/healthz`、Python Worker `/healthz` 和 metrics 均为 200；`gpustack` 与 `gpustack-worker` 容器继续为 Up 5 days。staged diff secret scan 与 `git diff --check` 通过。
- 安全边界：全部测试使用临时目录、静态 fixture 和 mock transport；没有读取真实 token，没有注册真实 Worker，没有发送 heartbeat/status 或其他真实写请求，没有操作 Docker 容器、GPU、vLLM 或 Python Worker。
- 当前停止位置：M3-C.1 状态为 `ready_for_review`，等待直接实现报告和用户共同验收；M3-D 继续暂停。
- 下一项允许执行的工作：完成详细报告与审阅；若用户接受，再提交验收治理检查点并恢复 M3-D 的独立审批流程。

### 2026-10-20：M3-C.1 Server 权威身份修正接受

- 目的：在恢复真实 Server 集成讨论前，正式确认纠偏后的身份权威、两文件状态机、安全不变量、fixture 语义和离线证据。
- 结果：用户在收到逐项实现报告后确认无问题，M3-C.1 状态由 `ready_for_review` 更新为 `accepted`。
- 接受范围：Server 返回的 Worker ID/name/UUID 为权威；身份元数据先于 token 持久化；正常重启核验、metadata-only 恢复、token-only fail closed、持久 token 被拒绝时不重新注册，以及相应离线回归测试。
- 保留的失效记录：M3-A 注册请求/响应 UUID 等值 fixture 语义与 M3-C 对应断言继续标记为 `invalidated`，不得重新作为兼容性证据；M3-C 控制循环和其他已验证契约不受影响。
- 安全边界：本次接受只确认离线实现，不授权真实 Worker 注册、heartbeat/status、Docker、GPU、vLLM 或 Python Worker 操作。
- 当前停止位置：M3-C.1 已接受，M3-D 尚未开始。
- 下一项允许执行的工作：恢复 M3-D 执行前门禁讨论；第一次真实 Server 写请求必须再次获得用户明确批准。

### 2026-10-20：Managed 状态机与 Lab 边界纠偏接受

- 目的：在 M3-D 前核对当前设计是否完整遵循固定 GPUStack v2.2.3 的 ModelInstance 生命周期，并明确 Lab 是否需要进入相同状态链。
- 触发事实：现有 `architecture.md` 和 `worker-protocol.md` 明确写成 `SCHEDULED -> INITIALIZING -> STARTING -> RUNNING`，并声称本地模型不进入 `DOWNLOADING`；当前 Rust desired state 也只包含 Model 和 ModelInstance，没有 ModelFile。
- 固定源码证据：在 commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` 核对了 `gpustack/schemas/models.py`、`gpustack/scheduler/scheduler.py`、`gpustack/worker/serve_manager.py`、`gpustack/worker/backends/base.py`、`gpustack/server/controllers.py`、`gpustack/schemas/model_files.py`、`gpustack/routes/model_files.py`、`gpustack/worker/model_file_manager.py` 和 `gpustack/worker/downloaders.py`。
- 协议结论：完整链路为 `PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING -> RUNNING`。Worker 接受自身 `SCHEDULED` 后写 `INITIALIZING`；Server Controller 根据 ModelFile 写 `DOWNLOADING/STARTING`；Worker 只在 `STARTING` 后启动 vLLM，并在健康检查成功后写 `RUNNING`。
- 本地路径结论：`local_path` 不执行网络下载，但仍由 ModelFile manager 验证路径、解析分片并写回 `resolved_paths`、progress 与 `ready/error`；因此不能绕过 ModelFile 控制链。
- Lab 结论：Lab 不创建 ModelInstance/ModelFile，也不写 Server 状态；使用隔离的本地 `CREATED/PREPARING/STARTING/RUNNING/FAILED/STOPPED`，同时与 Managed 复用模型路径解析、Docker planner/runtime、日志和健康检查。Lab 结果继续只属于诊断或冒烟证据。
- 漂移结论：这是已发现并经用户确认修正的设计偏差。偏差尚未进入 Docker workload、ModelFile manager 或 Managed reconciler，因此没有错误运行时、GPU 或集成 artifact 需要作废；M3-C 已接受的身份、heartbeat/status、Model/ModelInstance watch/full-sync 证据继续有效。
- 已接受修订：新增 D036–D039；M3-D 前插入 M3-C.2，离线实现 ModelFile fixture/schema/client、按 Worker list/watch、完整 update、desired state 和 full-sync 恢复；M6 拆为文件准备协调与 `STARTING` 后 runtime。
- 安全边界：本次只检查固定 Git 对象并修改治理文档；没有读取 token、连接真实 Server 写路径、操作 Docker/GPU/vLLM，也没有修改 Python Worker。
- 当前停止位置：M3-C.2 设计已冻结、实现未开始；M3-D 继续暂停。
- 下一项允许执行的工作：在聊天中交付 M3-C.2 的文件级实现计划、数据流、测试矩阵和验收边界，得到确认后才开始离线代码实现。

### 2026-10-20：M3-C.2 ModelFile 控制协议实现启动

- 目的：在第一次真实 Server 集成前，把固定 v2.2.3 的 ModelFile list/get/watch/update 契约加入 Rust client 和 Worker 权威控制缓存，但不执行文件准备或运行时动作。
- 用户确认：用户已审阅并接受文件级实现方案、数据流、测试矩阵和验收边界，并明确同意把 `GET /v2/model-files/{id}` 纳入本阶段。
- 单项刷新理由：ModelFile update 是完整 PUT；固定 Python Worker 每次更新前先获取最新 public 对象。Rust 保留该顺序，避免用可能过期的 watch cache 主动覆盖已知字段。该读取不改变固定协议缺少 ETag/条件更新的事实。
- 实现边界：新增 synthetic ModelFile fixture、public/update 类型、list/get/watch/update client、三资源 full sync/watch 和离线回归测试；不实现 ModelFile manager、本地路径访问、分片解析、实际状态写入、ModelInstance 推进、Docker、GPU 或 vLLM。
- 证据等级：本阶段 fixture 和 mock 结果属于离线协议证据，不冒充真实认证流量或 Managed 部署结果。
- 当前停止位置：M3-C.2 状态为 `running`，M3-D 继续暂停。
- 下一项允许执行的工作：提交本启动检查点后进行离线实现，完成门禁与详细报告前不得请求验收。

### 2026-10-20：M3-C.2 ModelFile 控制协议待审阅

- 结果：代码检查点 `b8a9b126770dc963518c839557d348d62eca03cd` 已实现并推送，M3-C.2 状态为 `ready_for_review`，尚未标记 `accepted`。
- client 范围：新增 tolerant `ModelFilePublic`、完整 `ModelFileUpdate`、`downloading/ready/error` 写入状态校验，以及按 Worker 的 list/watch、按 ID 刷新和完整 PUT。写请求在传输前拒绝错误 Worker、非 `local_path`、空路径和未知状态；调用方必须先 GET 最新 public 对象再构造完整 update。
- Worker 范围：desired state、full sync 和 watch session 已扩展为 Model、ModelInstance、ModelFile 三资源；服务端过滤后再次按当前 Worker 防御性过滤。任一 watch 断开、报错或收到未知事件都会保留最后权威快照并重连、full sync；ModelFile delete 只移除控制缓存，不触碰文件系统。
- fixture 证据：新增 full-sync、完整 update 和 CREATED/UPDATED/ID-only DELETED synthetic fixture，均由固定 Server commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` 的 schema、route、Python Worker 顺序与事件协议离线构造；未冒充真实 Server 或 Pydantic 运行结果。
- 自动化证据：Rust 1.99.1 下 workspace 共 35 项测试通过，其中 `gpustack-client` 13 项、`gpustack-worker` 20 项、CLI 2 项；`cargo fmt --check`、全 workspace/all targets/all features Clippy `-D warnings`、locked/offline test、locked/offline build、Rustdoc `-D warnings`、diff check 和 secret scan 全部通过。
- 依赖证据：没有修改 `Cargo.toml` 或 `Cargo.lock`，锁文件 SHA-256 仍为 `af566acb9215d5601dd8bec1282eb93c133b521c4b946870e6762b88409d5019`。
- 环境证据：提交前 Server `/healthz`、Python Worker `/healthz` 与 metrics 均为 HTTP 200；`gpustack` 和 `gpustack-worker` 容器继续为 Up 5 days。
- 安全与漂移审计：实现只使用 static fixture、mock transport 和内存状态；没有读取真实 token，没有向真实 Server 发送写请求，没有访问模型路径，也没有操作 Docker、GPU、vLLM 或 Python Worker。实现与已接受的 Managed 状态所有权、Lab 隔离和 M3-C.2 边界一致。
- 解释：本阶段只证明固定 v2.2.3 下 ModelFile wire contract、控制缓存和断线恢复的离线兼容性，不证明真实认证、并发 PUT 保护、文件准备、ModelInstance 状态推进或模型运行能力。
- 当前停止位置：M3-C.2 等待聊天中的详细实现报告和用户共同验收；M3-D 继续暂停。
- 下一项允许执行的工作：完成详细报告与审阅；若用户接受，再提交验收治理检查点。任何真实注册或写路径仍需另行明确批准。

### 2026-10-20：M3-C.2 ModelFile 控制协议接受

- 结果：用户在收到详细实现报告，并确认“支持完整状态协议与控制缓存”不等于“已经执行完整调度流程”后表示无问题；M3-C.2 状态由 `ready_for_review` 更新为 `accepted`。
- 接受范围：固定 v2.2.3 的 ModelFile tolerant response、完整 update、list/get/watch client，Model/ModelInstance/ModelFile 三资源 desired state、full sync、watch 重连，以及所有权、字段保留、状态值和 ID-only delete 离线回归证据。
- 明确非结论：本次接受不代表 Rust Worker 已执行 `SCHEDULED -> INITIALIZING`、ModelFile 准备与状态写回、Server Controller 推进或 `STARTING -> RUNNING`；这些能力仍分别属于 M6-A、Server 和 M6-B。
- 证据：代码检查点 `b8a9b126770dc963518c839557d348d62eca03cd`；待审阅治理检查点 `5633e15f98047ee74454edf146ec0567cb54ead9`；workspace 35 项测试及格式、Clippy、build、Rustdoc、diff 和 secret 门禁均通过。
- 安全边界：验收不授权读取现有 token、注册真实 Worker、发送 heartbeat/status/ModelFile/ModelInstance 写请求，或操作 Docker、GPU、vLLM 与 Python Worker。
- 当前停止位置：M3-C.2 已接受，M3-D 尚未开始。
- 下一项允许执行的工作：共同冻结 M3-D 的隔离身份和真实集成运行手册；只有用户随后明确批准真实写路径，才能执行 M3-D。

### 2026-10-20：M3-D 隔离真实控制面设计冻结

- 目的：在第一次真实 Server 注册前固定 Rust Worker 的运行范围、不可调度保护、身份、端口、凭据处理、运行时长、证据和清理边界。
- 用户确认：用户接受 `control-plane-only` 设计，并同意提前实现最小 `/healthz`、优先使用专用注册 token 且保留安全复用现有 token 的备选路径、验收后保留 maintenance identity 供 M4 使用。
- 隔离身份：Worker 名称 `rust-m3d-r760xa`，IP `10.213.80.90`，网卡 `eno8403`，health 端口 `20150`，无 metrics 端口；数据目录 `/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d`，注册令牌文件 `/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token`。
- 调度保护：注册固定 `maintenance.enabled=true`，状态上报固定空 GPU；M3-D 不初始化 NVML、Docker、vLLM、ModelFile manager 或 ModelInstance reconciler，也不发送 ModelFile/ModelInstance update。
- 分阶段门禁：M3-D.1 只实现代码并用 fixture/mock 验证，完成后必须直接交付详细实现报告；M3-D.2 才能在再次明确批准后读取凭据并对真实 Server 写入。
- 真实运行计划：首次连续运行至少 6 分钟，正常停止后以相同数据目录再次运行约 2 分钟；证明 heartbeat/status、三资源 watch/full-sync、Server health 探测和同一身份恢复，同时验证无新容器、无 GPU 进程、无模型状态写入且 Python Server/Worker 健康。
- 清理与延续：每次运行后停止 Rust 进程并确认 `20150` 关闭；正常验收保留 maintenance Worker row 和本地状态供 M4 复用。删除精确 Worker identity 需要单独破坏性审批。
- 证据等级：当前仅冻结设计，不是实际注册、认证成功、调度兼容或模型运行证据；支持完整状态协议仍不等于已经执行完整调度状态链。
- 安全边界：本次只修改治理文档，不读取 token，不连接真实 Server 写路径，不操作 Docker、GPU、vLLM 或 Python Worker。
- 当前停止位置：M3-D 设计已冻结，M3-D.1 尚未开始。
- 下一项允许执行的工作：提交本治理检查点后，等待用户授权 M3-D.1 离线实现；M3-D.2 仍未授权。

### 2026-10-20：M3-D.1 control-plane-only 离线实现启动

- 目的：把已冻结的隔离控制面设计实现为正式 CLI 和 Worker supervisor，在任何真实凭据或 Server 写入前验证完整启动、身份、控制循环和关闭路径。
- 用户确认：用户已授权开始 M3-D.1；该授权只包含代码、静态 fixture、mock transport、临时目录和离线构建测试，不包含 M3-D.2。
- 实现切面：类型化 maintenance；hostname/ifname/port 配置；`worker run --control-plane-only`；最小 `/healthz`；registration/status 安全 profile；身份 bootstrap、三资源控制循环和 shutdown 编排；脱敏事件与退出摘要。
- 依赖边界：不新增 HTTP framework 或其他第三方 crate；只启用工作区现有 Tokio 的 network/I/O feature。最小 health listener 属于 M3-D liveness 子集，M4 再扩展完整 Worker API。
- 安全边界：测试不得读取 `/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token` 或现有 Python Worker 环境，不得运行新的 `worker run` 连接真实 Server，不得操作 Docker、GPU、vLLM 或 Python Worker。
- 当前停止位置：M3-D.1 状态为 `running`，代码尚未修改。
- 下一项允许执行的工作：完成离线实现、自动化门禁、注释审阅和环境无副作用核验，然后提交代码检查点并在聊天中交付详细实现报告。

### 2026-10-20：M3-D.1 control-plane-only 离线实现待审阅

- 结果：代码检查点 `49235477b7d436343fded38c5c72bc16d31e0823` 已实现并推送，M3-D.1 状态为 `ready_for_review`，尚未标记 `accepted`。
- CLI 与配置：新增显式 `gpustack-rs worker run --control-plane-only`，未提供该安全开关时以配置退出码 10 拒绝运行；补齐 hostname、ifname 和 Worker port 的 YAML、环境变量、CLI 与默认值合并。最终摘要支持 table/JSON，过程事件只输出非敏感身份和控制状态。
- supervisor 与安全 profile：启动顺序固定为先绑定 health、再安全读取注册凭据、建立或恢复 Server 权威身份、重新读取 `me` 确认 `maintenance.enabled=true`，最后启动 heartbeat/status 和三资源 full-sync/watch。注册与状态固定为空 GPU、无 metrics，不初始化 NVML、Docker、vLLM、ModelFile manager 或 ModelInstance reconciler，也不发送 ModelFile/ModelInstance update。
- health 与关闭：Tokio 最小监听器只对 `GET /healthz` 返回 200，限制 8 KiB 请求和 2 秒读取时间；端口冲突会在任何凭据或 Server 操作前失败。SIGINT/SIGTERM 会通知 health 和控制循环停止，并输出包含身份来源、desired revision、各健康位和重连计数的摘要。
- 身份与契约证据：首次 mock 运行保存 Server 返回的 ID/name/UUID 和 Worker token；同一临时数据目录第二次运行恢复同一身份且不重复注册。新增 maintenance Worker synthetic fixture，并将其纳入固定 v2.2.3 契约解析与脱敏检查。
- 自动化证据：Rust 1.99.1 下 workspace 共 40 项测试通过，其中 CLI 3 项、client 13 项、Worker 24 项；`cargo fmt --check`、全 workspace/all targets/all features Clippy `-D warnings`、locked/offline test、locked/offline build、Rustdoc `-D warnings`、diff check 和 secret scan 全部通过。
- 依赖证据：没有新增第三方 crate 或版本；Tokio 只启用已有依赖的 `net`/`io-util` feature，CLI 直接声明已有 Tokio，Worker 测试直接声明锁文件中已有 `futures-util`。`Cargo.lock` SHA-256 为 `b6c0de0cbc158818868a749558fe9ca1aa4ca633c2eb3ef367929fe9d34b0eae`。
- CLI 离线证据：`worker run --help` 展示 safety flag 和完整 identity 参数；固定 M3-D 参数的 `validate-config` 返回 `valid: true`；不带 flag 的 `worker run` 返回 10、stdout 为空，并明确提示必须指定 `--control-plane-only`。没有执行带 flag 的真实 run。
- 环境与隔离证据：完成后 Server、Python Worker 和 metrics 均为 HTTP 200；`gpustack` 与 `gpustack-worker` 容器继续运行；20150 端口空闲、没有 `gpustack-rs` 进程，两张 A100 仍可见。本阶段没有读取真实 token、注册真实 Worker、发送真实 heartbeat/status 或操作 Docker、GPU、vLLM 与 Python Worker。
- 漂移审计：实现继续固定 Server v2.2.3、maintenance 加空 GPU 双重隔离、单 Worker、本地 Docker 和 vLLM-only R1 边界；没有提前实现 M4 Worker API、M6 状态写回或 runtime。关键注释只解释端口先绑定、Server UUID 权威和探针错误隔离等非直观不变量。
- 当前停止位置：M3-D.1 等待聊天中的详细实现报告和用户共同验收，M3-D.2 仍未授权。
- 下一项允许执行的工作：完成报告与审阅；若用户接受，再记录 M3-D.1 接受检查点并单独讨论是否授权 M3-D.2 真实集成运行手册。

### 2026-10-20：M3-D.1 control-plane-only 离线实现接受

- 结果：用户在收到目标、功能、启动顺序、安全 profile、模块、依赖、测试、隔离证据和限制的详细报告后确认无问题；M3-D.1 状态由 `ready_for_review` 更新为 `accepted`。
- 接受范围：显式 `worker run --control-plane-only`、缺少安全开关时拒绝运行、maintenance 加空 GPU 双重隔离、最小 `/healthz`、Server 权威身份注册或恢复、maintenance 二次确认、heartbeat/status、三资源 full-sync/watch、优雅停止和脱敏摘要。
- 证据：代码检查点 `49235477b7d436343fded38c5c72bc16d31e0823`；待审阅治理检查点 `8d7f2ebeb7278065452e39854fa8b316d30be107`；workspace 40 项测试及 fmt、Clippy、locked/offline build、Rustdoc、diff 和 secret 门禁均通过。
- 明确非结论：接受结果只证明 fixture/mock/临时目录下的离线实现，不证明真实注册、真实 heartbeat/status、Server 探针、持续运行、重启恢复、模型状态推进、Docker、GPU 或 vLLM 能力。
- 安全边界：本次接受不授权读取真实 token、注册真实 Worker、向 Server 写入、启动 `gpustack-rs worker run --control-plane-only`，也不授权操作 Docker、GPU、vLLM 或 Python Worker。
- 当前停止位置：M3-D.1 已接受，M3-D.2 运行手册进入讨论状态。
- 下一项允许执行的工作：共同冻结 M3-D.2 的凭据、预检、首次运行、第二次恢复、观测、失败处置和证据要求；执行仍需用户再次明确批准。

### 2026-10-21：M3-D.1.1 凭据生命周期加固启动

- 触发事实：固定 v2.2.3 只有 Cluster 级 bootstrap registration token；Server 在注册成功后才返回 per-Worker token。当前 supervisor 在每次启动时强制读取 Cluster token，虽然身份状态机已支持完整 identity 加 Worker token 独立恢复。
- 用户决定：首次注册使用现有默认 Cluster token，由用户通过隐藏输入写入 M3-D 专用 `0600` 文件；不使用 admin API key、不读取 Python Worker 环境、不创建新 Cluster。首次运行确认两份 Worker 身份文件后删除本地 Cluster token 副本，第二次只靠 Worker token 恢复。
- 修订范围：supervisor 改用可选 registration token；完整 identity 加 Worker token 可以恢复，首次注册或 metadata-only 恢复缺少 Cluster token 时继续 fail closed。CLI 参数、身份格式、Server v2.2.3、maintenance/空 GPU、控制循环和 M3-D.2 验收标准不变。
- 实现门禁：只使用 fixture、mock transport 和临时目录，验证首次缺凭据拒绝、完整身份无 Cluster token 恢复、metadata-only 无 Cluster token 拒绝、不会重复注册以及秘密脱敏；运行全部 Rust 自动化和隔离检查。
- 安全边界：本阶段不读取真实 token，不启动真实 Rust Worker，不注册或写入 Server，不操作 Docker、GPU、vLLM 或 Python Worker。M3-D.2 仍需完成详细报告和再次明确批准。
- 当前停止位置：M3-D.1.1 状态为 `running`。
- 下一项允许执行的工作：提交本治理检查点后进行离线代码实现、门禁和详细报告。

### 2026-10-21：M3-D.1.1 凭据生命周期加固待审阅

- 结果：代码检查点 `99e9fdb9038f65c480371005fb62fe1c1d6b1c72` 已实现并推送，M3-D.1.1 状态为 `ready_for_review`，尚未标记 `accepted`。
- supervisor 行为：Cluster registration token 从强制读取改为可选读取，并以 `Option<SecretToken>` 传入既有身份状态机。完整 identity 加 per-Worker token 不需要 Cluster token；首次注册和 metadata-only 恢复缺少 Cluster token 时返回 `RegistrationTokenRequired`，不会发送 `POST /v2/workers`。
- 恢复证据：原有首次注册加第二次恢复测试现在在第二次会话显式不提供 Cluster token，仍恢复相同 Worker ID/UUID，身份来源为 `persisted`，且没有重复注册或任何 PUT 请求。新增两项 supervisor 测试分别覆盖空状态和 metadata-only 状态缺少 Cluster token 的 fail-closed 行为，并确认 liveness 正常收尾。
- 自动化证据：Rust 1.99.1 下 workspace 共 42 项测试通过，其中 CLI 3 项、client 13 项、Worker 26 项；`cargo fmt --check`、全 workspace/all targets/all features Clippy `-D warnings`、locked/offline test、locked/offline build、Rustdoc `-D warnings`、diff check 和 staged secret scan 全部通过。
- 依赖与注释：没有修改 `Cargo.toml` 或 `Cargo.lock`，锁文件 SHA-256 仍为 `b6c0de0cbc158818868a749558fe9ca1aa4ca633c2eb3ef367929fe9d34b0eae`。只新增一处关键注释，说明完整身份恢复不应保留 Cluster 级 bootstrap secret。
- CLI 与隔离证据：`worker run --help` 仍展示 control-plane-only、token file 和 data dir 参数；不带 safety flag 的 run 仍以 10 退出且 stdout 为空。Server、Python Worker 和 metrics 均为 HTTP 200；20150 端口空闲、精确进程名检查无 `gpustack-rs`、两个 GPUStack 容器持续运行、两张 A100 可见。
- 安全边界：所有新测试只使用临时目录、静态 fixture 和 mock transport；没有读取或生成真实 token，没有注册真实 Worker、发送真实 Server 写请求、启动真实 Rust Worker，也没有操作 Docker、GPU、vLLM 或 Python Worker。
- 漂移审计：未改变固定 Server v2.2.3、CLI 参数、身份文件格式、maintenance 加空 GPU、三资源控制循环或 R1 范围；只收紧 D047–D048 已接受的凭据驻留边界。
- 当前停止位置：M3-D.1.1 等待聊天中的详细实现报告和用户共同验收，M3-D.2 仍未授权。
- 下一项允许执行的工作：完成报告与审阅；若用户接受，再记录 M3-D.1.1 接受检查点并继续冻结 M3-D.2 的精确命令，真实执行仍需单独明确批准。

### 2026-10-21：M3-D.1.1 凭据生命周期加固接受

- 结果：用户在收到 supervisor 行为、身份状态组合、测试、依赖、隔离证据和限制的详细报告后确认无问题；M3-D.1.1 状态由 `ready_for_review` 更新为 `accepted`。
- 接受范围：registration token 可选读取和传递；完整 identity 加 per-Worker token 可在没有 Cluster token 时恢复；首次注册和 metadata-only 恢复缺少 Cluster token 时 fail closed，且不发送 `POST /v2/workers`；Cluster token 文件的删除保留为首次真实运行确认身份完整后的显式受控步骤。
- 证据：设计检查点 `eb5a9c2d80d1e191f9030808e1dcc80e7a217445`；代码检查点 `99e9fdb9038f65c480371005fb62fe1c1d6b1c72`；待审阅治理检查点 `19ae17d9461e9db8f09c9bfa7a121379b5f8c680`；workspace 42 项测试及 fmt、Clippy、locked/offline build、Rustdoc、diff 和 secret 门禁均通过。
- 明确非结论：本次接受只证明 fixture、mock transport 和临时目录下的凭据生命周期；不证明真实 Cluster token、真实注册、真实 heartbeat/status、Server 探针、持续运行、重启恢复、Docker、GPU 或 vLLM 能力。
- 安全边界：接受结果不授权读取真实 token、注册真实 Worker、向 Server 写入或启动 `gpustack-rs worker run --control-plane-only`，也不授权操作 Docker、GPU、vLLM 或 Python Worker。
- 当前停止位置：M3-D.1.1 已接受，M3-D.2 运行手册进入讨论状态。
- 下一项允许执行的工作：共同冻结 M3-D.2 的凭据写入、预检、首次运行、身份确认、Cluster token 副本删除、第二次恢复、观测、停止和失败处置命令；真实执行仍需用户在完整命令审阅后再次明确批准。

### 2026-10-21：M3-D.2 真实集成运行手册冻结

- 目的：在任何真实凭据读取或 Server 写入前，固定操作者、分段门禁、参数、证据位置、运行时序和失败处置，使首次真实控制面集成可以停止、审计和恢复。
- 用户确认：用户接受运行手册，并选择亲自在 90 主机执行全部命令；助手逐段提供详细命令与目的说明，只审阅返回的非敏感输出。任何 token 都不得发送到聊天。
- 固定身份与路径：Worker `rust-m3d-r760xa`，Server `http://127.0.0.1`，hostname `r760xa`，IP `10.213.80.90`，网卡 `eno8403`，health `20150`；token/data 继续使用 D043 路径；证据目录为 Git 外 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m3d2-formal`，权限 `0700`。
- 运行与观测：从冻结后干净 HEAD 重建 binary 并记录 commit/SHA-256；首次运行 370 秒并在约 60 秒和 330 秒观测，第二次在 token file 缺失的情况下运行 130 秒；用一次 Server 容器内只读 HTTP probe 证明 `10.213.80.90:20150/healthz` 可达。
- 凭据处置：首次注册一旦尝试，无论成功失败，都删除精确路径上的本地 Cluster token 副本；重试需要用户重新隐藏输入。失败不自动删除身份文件、Server Worker row 或其他状态。
- 保留与非目标：正常结果保留 maintenance Worker 和本地身份供 M4；Python Worker 保持运行。本阶段不停止或修改容器，不使用 GPU，不启动 vLLM，不执行 ModelInstance/ModelFile 状态写入。
- 证据等级：当前只证明运行手册和责任边界已经冻结，不证明真实 token、注册、heartbeat/status、health 可达、持续运行或身份恢复。
- 当前停止位置：尚未执行第 0 段预检，尚未创建证据目录、构建正式 binary、读取 token 或连接真实写路径。
- 下一项允许执行的工作：用户只执行第 0 段只读预检并返回完整非敏感输出；通过审阅前不得进入后续段。

### 2026-10-21：M3-D.2 真实控制面集成接受

- 结果：M3-D.2 状态为 `accepted`。正式二进制来自干净提交 `e30e7bd40d0d0b12746b9d1cdd5c3a4e9a62af6d`，SHA-256 为 `4272530f2dde55f4679c955544bb64104c8cbff16ee7cc8ca8e32d0b875ca713`，固定 Server 为 v2.2.3 / `1cfcbbf`。
- 操作者修订：用户在门禁通过后明确要求助手代为执行，形成 D053；只改变 D049 的操作者，不改变正式参数、运行时长、秘密边界、验收口径或非目标。
- 无效尝试：`m3d2-formal/first-run/` 中的调用把 `--server-url` 等 Worker 参数放在顶层解析器，CLI 在网络和身份初始化前以退出码 2 拒绝。Cluster token 按 D051 删除，数据目录未创建、Server 无同名 Worker；证据保留并标记 `invalidated`。
- 修正与重试：CLI 帮助和 `validate-config` 确认正确层级为 `gpustack-rs [global options] worker [worker options] run --control-plane-only`。通过官方 `/auth/login` 重新获取默认 Cluster token，二次认证和等值校验后以 `0600` 原子安装；登录 cookie、响应和比较文件均在退出时删除。
- 首次正式运行：`m3d2-formal/first-run-retry-1/` 持续 370 秒，60 秒和 330 秒观测均只有一个 `rust-m3d-r760xa`，maintenance 开启、GPU 列表为空、metrics 端口为空；heartbeat 从 `2026-10-21T04:14:54Z` 推进到 `04:19:24Z`，宿主机和 Server 容器访问 `10.213.80.90:20150/healthz` 均为 200。
- 首次摘要：优雅 `interrupt` 退出，`identity_source=registered`；heartbeat、status、full sync、watch 全部为 true，重连次数为 0。Server 分配 Worker ID 2，名称 `rust-m3d-r760xa`，UUID `f38b7c2c-90f7-4a76-a2c5-dfe1ff63fea0`，与本地 `worker-identity.json` 三方一致。
- 凭据生命周期：首次正式运行结束后 Cluster registration token 固定副本已删除；`managed/` 为 `0700`，`worker-identity.json` 和 per-Worker `worker-token` 均为 `0600`。per-Worker token 可独立查询 `GET /v2/workers?me=true`，证据目录秘密扫描通过。
- 第二次恢复：`m3d2-formal/second-run/` 在 Cluster token 文件缺失时持续 130 秒，60 秒和 110 秒仍为 Worker ID 2，heartbeat 从 `04:22:34Z` 推进到 `04:23:34Z`；最终摘要为 `identity_source=persisted`，ID、UUID 和名称与首次运行及本地身份完全一致，各控制健康位为 true，重连次数为 0。
- 管理员独立复核：全局按名称查询只有一个 Worker ID 2；该 Worker 关联 ModelInstance 和 ModelFile 数量均为 0。Worker 保持 maintenance、空 GPU、无 metrics 端口，没有重复注册或模型状态写入。
- 隔离结果：两次运行后 Server、Python Worker 和 Python metrics 均为 HTTP 200；Rust 进程数和 `20150` 监听均为 0；`gpustack`、`gpustack-worker` 和原有 `open-webui` 容器继续运行；两张 A100 显存使用和利用率均为 0。未启动 Docker workload、vLLM 或 GPU 任务，也未停止或修改 Python Worker。
- 证据：正式根目录 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m3d2-formal` 为 `0700`；有效运行目录与文件为 `0600`，包含 timing、脱敏观测、health、events、summary、token cleanup 和管理员复核。任何 token 与 Authorization header 均未写入 Git 或正式证据。
- 解释：真实 Server 已验证 Rust control-plane-only Worker 的首次注册、Server 权威身份持久化、持续 heartbeat/status、三资源 full-sync/watch、Server 到 Worker health 可达、安全停止，以及不依赖 Cluster token 的同一身份恢复。该结论不证明完整 Worker API、Docker runtime、GPU、vLLM、模型准备或推理能力。
- 当前停止位置：M3-D.2 已接受；Rust Worker 已停止，但 maintenance Worker ID 2 和本地 per-Worker 身份按 D046 保留，Cluster token 本地副本保持不存在。
- 下一项允许执行的工作：讨论并冻结 M4 Worker HTTP API 的范围、端口、认证、离线测试和真实验证门禁。任何 Docker、GPU、vLLM 或 Python Worker 维护操作仍需单独授权并安排维护窗口。

### 2026-10-21：M3-D.2 测试手册整理

- 目的：在进入 M4 讨论前，把已接受的真实集成测试整理为可人工复用的分组命令、通过标准和停止条件，避免后续只能从聊天或长命令块恢复测试方法。
- 分级：A 级只读复核现有证据；B 级使用 Worker ID 2 的完整 per-Worker identity 做 130 秒恢复复测；C 级使用全新隔离身份做首次注册复现。B 级是日常回归首选，C 级仍需单独真实写入批准。
- 安全边界：B 级会更新真实 Server 的 heartbeat/status，但不使用 Cluster token、不创建 Worker、不启动 Docker、GPU 或 vLLM；C 级禁止清空或复用现有 Worker ID 2、名称和数据目录，也不自动删除新建 Server row。
- 命令纠偏：手册固定顶层选项 → `worker` → Worker 配置选项 → `run --control-plane-only` 的层级，明确禁止再次把 `--server-url` 等参数放在 `worker` 之前。
- 核验：正式 run 命令已通过 CLI parser 的只读 `--help` 检查；per-Worker token 经标准输入传给 curl 的 `me=true` 查询已返回唯一 maintenance Worker，token 未进入命令参数或输出；没有启动 Rust Worker 或执行新的 Server 写入。
- 结果：新增 `m3d-integration-runbook.md` 并加入文档索引。该手册整理既不新增 M3-D.2 证据，也不扩大 M4、Docker、GPU、vLLM 或 Python Worker 的授权范围。
- 当前停止位置：M3-D.2 保持 `accepted`，M4 尚未开始。
- 下一项允许执行的工作：共同讨论并冻结 M4 Worker HTTP API 的范围和门禁。

### 2026-10-21：M4 资源盘点与 Worker API 设计冻结

- 结果：用户接受 M4 的模式、端口、认证、安全边界和 M4-A～M4-D 分段门禁；本检查点只冻结设计，没有修改 Rust 实现、读取 Worker token、启动 Rust Worker、更新 Server 或操作现有服务。
- 运行模式：新增显式 `worker run --resource-api-only` 设计；复用保留的 Worker ID 2 和 per-Worker token，启用系统/NVML、完整 M4 Worker API 与 metrics，继续 maintenance，并严格禁止 Docker、ModelFile/ModelInstance reconciler、vLLM 和模型状态写入。M3-D `--control-plane-only` 继续为空 GPU、无 metrics、仅最小 `/healthz`。
- 固定拓扑：Rust Worker API `10.213.80.90:20150`，metrics `10.213.80.90:20151`，模型根目录 `/home/wujiaxing2/flowscale-system-validation/models`；现有 Python Worker 继续使用 `10150/10151`。此前只读检查确认 `20150`～`20159` 无监听。
- API 范围：公开 health/readiness/version；per-Worker token 保护 token-auth、文件检查、模型配置、权重大小、日志选项/流与只读 config；`20151/metrics` 无认证。PUT config、GGUF、benchmark/cache/debug、inference/cluster proxy 和动态写配置明确延后。
- 安全与兼容差异：不接受 Cluster registration token；constant-time 比较、通用 401、绝对路径、canonicalization、symlink 和允许根目录 containment、配置/日志白名单为硬门禁。resource-api-only 的 `/readyz` 固定 503，而固定 Python v2.2.3 为常量 200；这是防止把无 Docker/reconciler 的 M4 误报为完整 Worker 的有意增强，Server reachability 仍使用 `/healthz`。
- 资源契约：类型化上报 CPU、memory/swap、filesystem、OS/kernel/uptime/machine ID，以及两张 GPU 的 index/UUID/name/vendor/type、driver/CUDA/compute capability、core/memory/temperature，并提交 `metrics_port=20151`。NVML 失败时 fail closed，不伪造空 GPU。
- 依赖方向：M4-A 审计 `axum`、`nvml-wrapper`、`sysinfo`、`prometheus-client` 和 `subtle`；准确版本只有在 Cargo cache、Rust 1.99.1/MSRV 与 locked/offline build 通过后冻结。
- 分段门禁：M4-A 只做契约/fixture/依赖审计；M4-B 才做 mock/fake 驱动的离线实现和详细报告；M4-C 只用资源 CLI 与 `nvidia-smi` 做宿主机只读 GPU smoke；M4-D 才在另获授权的维护窗口停止 Python Worker、运行 resource-api-only、验证 Server/API/metrics，并恢复 Python Worker。
- 验收边界：M4-C 是 diagnostic/smoke，不能代替正式验收；只有 M4-D 同时证明 Server 精确可见两张 A100、maintenance、metrics、API、安全路径、无 Docker/vLLM/GPU 任务以及 Python Worker 回滚，M4 才能标记为 accepted。
- 当前停止位置：M4 设计为 `frozen`，实现尚未开始；Rust 进程和 `20150/20151` 必须保持停止，Python Server/Worker 保持原状。
- 下一项允许执行的工作：只进入 M4-A 的固定 v2.2.3 源码/响应契约、静态 fixture 和依赖版本审计；M4-B、M4-C、M4-D 分别需要完成前一门禁和相应授权。

### 2026-10-21：M4-A Worker 资源/API 契约与依赖审计待审阅

- 结果：M4-A 状态为 `ready_for_review`。新增固定 v2.2.3 的 Worker 精确契约、Worker API/错误/metrics fixture 和类型化双 A100 status request；没有修改 Rust 源码、crate manifest 或 workspace `Cargo.lock`。
- 固定源码与现场只读证据：兼容源码仍为 `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`；现场 Python Worker 的 health/ready/version/通用 401 已捕获，Worker OpenAPI SHA-256 为 `a02743d483120ccd52179cdfb329fbc47d05503be6f2b455b341aeb31f5aa455`，metrics SHA-256 为 `bb5f0bb0271f2969d8ceda5904ac30b64735d30a10fe692886e25767ae02f4d4`。
- HTTP 与认证：精确固定 public probe、per-Worker token 保护 route、Bearer 与 `X-API-Key` 传输、Bearer 优先、无 Cluster token/Server fallback、通用 401、`application/octet-stream` 日志、read-only config 和延后 route 的 404/405/501 行为。
- 文件与日志安全：模型根目录固定为 `/home/wujiaxing2/flowscale-system-validation/models`；绝对路径、canonicalization、未存在目标祖先检查、symlink containment、八个配置文件 basename、16 MiB 上限、四种权重扩展、普通 LLM 与 diffusion 组件目录规则，以及 serve log 文件名/restart/tail/follow/previous 语义均已冻结。
- 资源与 metrics：status fixture 覆盖 96 逻辑 CPU、memory/swap、`/home` 文件系统、OS/kernel/uptime、双 A100 UUID/name/80 GiB/core/driver/CUDA/compute capability，并固定 `metrics_port=20151`。资源 metric 名称/label 对齐 Python Worker，新增 Rust mode/readiness/control-loop 指标；rate 使用百分比。
- 依赖审计：scratch crate 固定 `axum 0.8.9`、`nvml-wrapper 0.13.0`、`sysinfo 0.39.6`、`prometheus-client 0.25.1`、`subtle 2.6.1`，解析 89 个 package；Rust 1.99.1 下 online `cargo check --locked` 和关闭临时代理后的 `CARGO_NET_OFFLINE=true cargo check --locked --offline` 均通过。scratch lock SHA-256 为 `957d7f5b36b82a086c327c26f8ace641201578fd1f1fed7fa12839c8c9715d67`，workspace lock 保持 `b6c0de0cbc158818868a749558fe9ca1aa4ca633c2eb3ef367929fe9d34b0eae`。
- 网络与隔离：仅为依赖下载临时建立 `127.0.0.1:17897` 反向代理，完成后已关闭并确认无监听。没有读取 per-Worker token、启动 Rust Worker、发送 Server 写请求、停止或修改 Python Worker，也没有创建 Docker workload、vLLM 或 GPU 任务。
- 漂移审计：实现范围仍是 resource-api-only；M3-D control-plane-only、maintenance Worker ID 2、状态所有权、M4-C/M4-D 门禁和 R1 范围均未改变。对 Python v2.2.3 的差异仅为已经接受的严格路径 containment、per-Worker-only auth 和 readiness 503。
- 解释：本阶段证明契约可追溯、fixture 可用于离线实现且候选依赖可在固定工具链上离线构建；不证明 Rust API、资源采集、NVML、metrics、真实 status 或 Server 可见双 A100 已实现。
- 当前停止位置：M4-A 等待聊天中的详细报告和用户共同验收；M4-B 尚未授权。
- 下一项允许执行的工作：完成详细报告与审阅；若用户接受，再把 D061–D063 和 M4-A 标记为 `accepted` 并进入 M4-B mock/fake 驱动离线实现。M4-C、M4-D、服务维护、Docker、vLLM 和 GPU 工作负载仍不允许。

### 2026-10-21：M4-A Worker 资源/API 契约与依赖审计接受

- 结果：用户在收到 HTTP/auth、文件和日志安全、类型化资源、Prometheus、依赖版本、验证结果和安全边界的详细报告后确认无问题；M4-A 与 D061–D063 由 `ready_for_review` 更新为 `accepted`。
- 接受范围：固定 v2.2.3 Worker route/query/response/error、per-Worker-only auth、严格路径 containment、日志和配置白名单、双 A100 status schema、Python-compatible 资源 metric 和五项准确依赖版本。
- 证据：M4-A 契约与 fixture 提交 `e5362c573c1f004cb3aab50f8f026fdff95c09bb`；类型化 status fixture 通过固定 v2.2.3 Pydantic schema，Prometheus fixture 可解析且 Python 资源 metric 名称集合一致，Rust workspace 42 项测试以及 fmt、Clippy、locked/offline build 通过。
- 已知门禁限制：顶层 `make lint` 因 90 主机缺少 `uv` 在启动 Python pre-commit 前退出；这不是 lint 失败。M4-B 仍须运行 Rust 全门禁和适用于新增 fixture/文档的检查。
- 明确非结论：接受 M4-A 只证明契约、fixture 和候选依赖可用，不证明 Rust Worker API、资源采集、NVML、metrics 或真实 Server 双 A100 上报已经实现。
- 安全边界：接受结果只授权 M4-B 的离线代码与 mock/fake 测试；M4-C 真实 NVML、M4-D 服务切换、真实 Worker token、Server 写入、Docker、vLLM 和 GPU 工作负载仍未授权。
- 当前停止位置：M4-A 已接受，M4-B 尚未产生代码。
- 下一项允许执行的工作：实现 M4-B，并在提交代码和详细实现报告后停在 `ready_for_review`；用户再次接受前不得进入 M4-C。

### 2026-10-21：M4-B resource-api-only 离线实现待审阅

- 结果：M4-B 状态为 `ready_for_review`。代码检查点
  `994b820cba795b0bbfa25d23a8d55e2e2f470915` 已实现资源采集、Worker API、
  metrics、显式 resource-api-only supervisor 和两项只读 CLI；尚未标记为
  `accepted`。
- 资源采集：新增可替换的 system/GPU probe 和固定 v2.2.3 类型。生产 probe
  使用 `sysinfo` 两次 CPU 采样与单例 NVML 句柄，输出 bytes 和百分比；M4
  supervisor 对 machine ID、至少两张 GPU、关键 GPU identity 和模型根目录 mount
  coverage 执行 fail-closed 校验。
- HTTP 与认证：`axum` router 实现 public health/ready/version、per-Worker token
  保护的 token-auth/files/logs/config，以及独立 metrics router。Bearer 优先于
  `X-API-Key`，使用 `subtle` constant-time 比较；错误体、503 readiness、501
  container log 和 405 config PUT 对齐已接受 fixture。
- 文件、日志和配置：路径解析要求绝对路径，并以 canonical root、最近存在祖先和
  symlink containment 阻止越界；配置文件限制八个 basename 和 16 MiB；权重统计区分
  LLM 直接文件与 diffusion 组件目录；日志支持 restart/current/previous/tail/follow，
  只读取受控 serve 文件名；配置输出固定为非敏感白名单。
- metrics 与控制面：使用 `prometheus-client` 输出 19 个 Python-compatible 资源指标和
  7 类 Rust mode/readiness/control 指标；resource-api-only readiness 始终为 0。
  supervisor 只恢复已经持久化的 Worker identity，验证 Server maintenance，提交类型化
  status，并运行既有 heartbeat/status 与三资源 full-sync/watch；没有 workload 或模型
  状态写入路径。
- CLI：新增 `worker gpu list`、`worker config get` 和互斥的
  `worker run --resource-api-only`。GPU CLI 与 status 共用 NVML 类型；config CLI 与
  HTTP 使用同一白名单。实际执行只验证了无副作用的 `worker config get` 和帮助输出，
  没有运行真实 GPU list 或 resource-api-only。
- 依赖与锁文件：实际 workspace 写入 D062 的五项准确依赖和 feature，更新后的
  `Cargo.lock` SHA-256 为
  `ad5fabd70e55fcbee197c3a644b8acf530ba943ca385c9344e7e6cfb06cb19c3`；全部构建与测试
  使用固定 Rust 1.99.1 和 `--locked --offline`。
- 自动化证据：workspace 62 项测试通过，其中 CLI 5 项、client 13 项、Worker 44
  项；`cargo fmt --check`、locked/offline check、test、build、全 workspace/all targets
  Clippy `-D warnings`、diff check 和 secret scan 均通过。新增测试覆盖 typed 双 GPU、
  auth header 优先级、错误 fixture、路径 traversal/symlink、未存在路径、16 MiB 边界、
  LLM/diffusion 权重、restart/previous/tail/follow、container 501、配置脱敏和 metrics。
- 现场无副作用核验：Server、Python Worker health 和 Python metrics 均为 HTTP 200；
  `gpustack` 与 `gpustack-worker` 容器保持 Up 5 days；没有 `gpustack-rs` 进程，
  `20150/20151` 无监听。本阶段没有读取真实 Worker token、访问真实 NVML、启动 Rust
  Worker、更新 Server、停止 Python Worker 或执行 Docker/vLLM/GPU 工作负载。
- 漂移审计：实现保持固定 v2.2.3、maintenance、单 Worker、本地 Docker/vLLM-only
  R1 和完整 Managed 状态所有权；M3-D control-plane-only 未扩展。实际实现只到 M4-B
  离线边界，没有提前执行 M4-C 或 M4-D。
- 当前停止位置：M4-B 等待聊天中的详细实现报告和用户共同验收；代码与治理检查点
  均已提交并推送。M4-C 真实 NVML smoke、M4-D 维护窗口和任何服务/GPU 操作仍未
  授权。
- 下一项允许执行的工作：直接交付 M4-B 详细实现报告；若用户接受，再记录 M4-B
  `accepted` 并单独讨论 M4-C 的只读 GPU 命令。用户接受前不得运行 `worker gpu list`
  或 `--resource-api-only`。

### 2026-10-21：M4-B resource-api-only 离线实现接受

- 结果：用户审阅资源采集、类型化状态、Worker API、路径安全、日志、metrics、
  resource-api-only supervisor、CLI、自动化门禁和环境隔离的详细实现报告后确认无问题；
  M4-B 由 `ready_for_review` 更新为 `accepted`。
- 接受证据：代码检查点
  `994b820cba795b0bbfa25d23a8d55e2e2f470915`；workspace `Cargo.lock`
  SHA-256 为
  `ad5fabd70e55fcbee197c3a644b8acf530ba943ca385c9344e7e6cfb06cb19c3`；
  62 项 workspace 测试、fmt、locked/offline check/build/test 和全 target Clippy
  `-D warnings` 均通过。
- 接受边界：该结果接受离线实现及 mock/fake 证据，不证明真实 NVML、Server 资源上报、
  在线 Worker API/metrics、Docker 或 vLLM。用户的接受只解锁 M4-C 宿主机只读 GPU
  冒烟，不授权 M4-D、停止 Python Worker 或启动 resource-api-only。
- 当前停止位置：M4-B 已接受，M4-C 可按冻结门禁读取真实 NVML 并与
  `nvidia-smi` 对比。
- 下一项允许执行的工作：只运行 `worker gpu list --output json` 和
  `nvidia-smi`，记录 diagnostic/smoke 证据；Python Worker 必须保持运行。

### 2026-10-21：M4-C 宿主机只读 GPU 冒烟待审阅

- 结果：M4-C 状态为 `ready_for_review`。有效重试精确枚举两张
  `NVIDIA A100 80GB PCIe`；Rust 与 `nvidia-smi` 的 index、UUID、名称、显存总量和
  driver version 规范化结果逐行一致，空 diff 与附加固定 A100 断言均通过。
- 构建身份：从干净 HEAD `c76fa3903b3ba81010b22cf17cd272a053dde2cb`
  离线重建 `gpustack-cli`；二进制 SHA-256 为
  `fa645ae457c656039e2a75fbf28cce96888e7503de15d4007946a7fde09b3ece`，
  内嵌 commit 为 `c76fa3903b3b`，Rust 为 1.99.1；`Cargo.lock` 保持冻结 SHA-256
  `ad5fabd70e55fcbee197c3a644b8acf530ba943ca385c9344e7e6cfb06cb19c3`。
- GPU 0：index `0`，UUID
  `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3`，名称
  `NVIDIA A100 80GB PCIe`，Rust total `85899345920` bytes，对应
  `nvidia-smi` 的 `81920` MiB，driver `580.178.04`。
- GPU 1：index `1`，UUID
  `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708`，名称
  `NVIDIA A100 80GB PCIe`，Rust total `85899345920` bytes，对应
  `nvidia-smi` 的 `81920` MiB，driver `580.178.04`。
- 附加观测：Rust 对两卡报告 NVIDIA/CUDA、Ampere、runtime `13.0`、compute
  capability `8.0`、6912 cores、0 bytes used 和 0% utilization；温度分别为
  33°C 与 32°C。这些字段是 smoke 观测，不扩展本阶段的通过门槛。
- 有效证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4c-gpu-smoke-c76fa390-retry-1`
  为 `0700`，文件为 `0600`。`rust-gpu-list.json` SHA-256 为
  `9e6a8779bf63e29fcf5f7f01ae6ada5fd822b1f5568cdf360427bd8d5f1dfeb5`，
  `nvidia-smi.csv` 为
  `5aa3c26084d35fdfe2c27854afae9c9cea87e8d9af4e2f8ad5b3b4d81aecd1a5`，
  `comparison.txt` 为
  `207f500d72880c5fa336bfdff7677438d162707fe99347f824fbc5d6e748b22c`，
  规范化 diff 为空文件。
- 无效和非计分证据：预检发现旧 binary 内嵌 `d602c1baaf21-dirty`，仅运行
  `version` 后即拒绝使用。随后一条跨 PowerShell/Bash 的长命令发生变量提前展开，
  把后续单词错误传给 `tee`，截断生成 binary、污染 `Cargo.lock` 并创建 8 个零字节
  根目录文件；第一次脚本因此只得到有效 `nvidia-smi` 与空 Rust 输出，目录
  `m4c-gpu-smoke-c76fa390/` 已写入 `INVALIDATED.txt`，不得作为 GPU collector 证据。
  精确恢复冻结 lock、删除本次误建文件、清理 CLI 构建产物并离线重建后，工作树重新
  干净。比较脚本首次附加 jq 断言作用域错误也不计分；规范化 diff 当时已一致，修正
  jq 后只复用已捕获文件，没有再次读取 GPU。
- 隔离结果：有效比较后 Server、Python Worker health 与 Python metrics 均为 HTTP
  200；`gpustack-rs` 精确进程数为 0，`20150/20151` 无监听；`gpustack` 和
  `gpustack-worker` 容器继续 Up 6 days。没有读取 Worker token、注册或更新 Server、
  启动 Rust Worker、停止 Python Worker、创建 Docker/vLLM workload 或运行 GPU 任务。
- 解释：本阶段证明生产 NVML collector 能在 90 宿主机准确读取两张 A100 的关键身份
  和容量字段；它只属于 diagnostic/smoke，不证明 resource-api-only 的真实 Server
  status、Worker HTTP API、metrics 或维护窗口回滚，也不能使整个 M4 标记为接受。
- 当前停止位置：M4-C 等待聊天中的详细结果报告和用户共同验收；Rust Worker 保持
  停止，Python Server/Worker 保持原状。
- 下一项允许执行的工作：交付并审阅 M4-C 报告。若用户接受，只能开始讨论 M4-D
  精确维护窗口、预检、stop/start 和回滚命令；未获得单独明确授权前不得停止 Python
  Worker 或运行 `--resource-api-only`。

### 2026-10-21：M4-C 宿主机只读 GPU 冒烟接受

- 结果：用户审阅双 A100 逐字段对比、构建与证据身份、无效尝试恢复链、服务隔离结果
  和 M4-C 非正式集成边界后确认无问题；M4-C 由 `ready_for_review` 更新为
  `accepted`。
- 接受范围：生产 NVML collector 能在 90 宿主机准确读取两张 A100 的 index、UUID、
  名称、显存和 driver，并与同一时点的 `nvidia-smi` 一致；无效证据继续保持
  `invalidated`，不提升为结论。
- 明确非结论：M4-C 不证明 resource-api-only 已与真实 Server 集成，也不证明在线
  Worker API、metrics、maintenance 保持、服务切换或回滚；整个 M4 仍须通过 M4-D
  才能接受。
- 当前停止位置：Rust Worker 已停止，`20150/20151` 无监听，Python Server/Worker
  保持健康；M4-D 尚未获得执行授权。
- 下一项允许执行的工作：只讨论并冻结 M4-D 的精确维护窗口、预检、Python Worker
  stop/start、resource-api-only 启停、验证与回滚步骤。讨论通过后仍须由用户单独明确
  授权实际维护操作。

### 2026-10-21：M4-D.0 只读预检通过

- 结果：M4-D.0 状态为 `passed`。本阶段只验证正式维护集成的前置条件，没有停止或
  启动任何服务，也没有向 Server 写入状态；M4-D 本身仍为未执行、未授权。
- 源码与锁文件：分支 `codex/rust-worker` 的 HEAD 与 origin 均为
  `fb6412611c5ebd371e7848baea8f8fdccec9fae0`，工作树干净；`Cargo.lock`
  SHA-256 保持
  `ad5fabd70e55fcbee197c3a644b8acf530ba943ca385c9344e7e6cfb06cb19c3`。
- Server 与身份：真实 Server 为 v2.2.3、commit `1cfcbbf`；本地持久化 Worker
  identity 为 ID 2、UUID `f38b7c2c-90f7-4a76-a2c5-dfe1ff63fea0`、名称
  `rust-m3d-r760xa`。Cluster registration token 文件保持不存在；per-Worker token
  文件权限为 `0600`，只在内存中用于认证只读 GET，没有打印、复制到证据或写入 Git。
- 控制面：认证 `GET /v2/workers?me=true` 只返回目标 Worker，maintenance 为 true，
  当前 Server 记录仍是 M3-D control-plane-only 的 `gpu_count=0` 和空
  `metrics_port`。目标 Worker 的 ModelInstance 与 ModelFile 数量均为 0。
- 服务隔离：Server、Python Worker health 和 Python metrics 均为 HTTP 200；
  `gpustack` 与 `gpustack-worker` 容器继续运行。精确 `gpustack-rs` 进程数为 0，
  `20150/20151` 监听数均为 0。
- GPU 与主机：两张 `NVIDIA A100 80GB PCIe` 的 UUID 分别为
  `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` 和
  `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708`；两卡显存占用和利用率均为 0，
  没有 compute process。模型根目录
  `/home/wujiaxing2/flowscale-system-validation/models` 可 canonicalize；接口
  `eno8403` 为 UP，地址为 `10.213.80.90/20`。
- 有效证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d0-preflight-fb641261-retry-2`
  为 `0700`，文件为 `0600`。`gates.txt` SHA-256 为
  `021d4821f08736ab9cc264c897128aad5652252633f95aece416f771e21c7142`，
  `result.txt` 为
  `a59452548d4f0f6bfb001780082a5786c30c2077cd68cd2e4a9d6854a3ca5738`，
  `gpu-baseline.csv` 为
  `36d92ef6cf851b1ec44b755a4c78298e6b8f6ea6e333954fc5edeee122049c4e`，
  `worker-me-summary.json` 为
  `5b1b3f42135961c849b52cbf5a916def6c9fa0f64fadcd1356c9709f0e8b4e8a`，
  `container-baseline.txt` 为
  `d89be5d2a7c98162562d899f41a4a826fff443d953fd774af54b130b379d05a7`。
- 安全检查：证据中未发现真实 token 值或 Authorization header，临时认证配置已删除；
  预检仅执行 health、version、Worker/ModelInstance/ModelFile 查询和宿主机只读命令。
- 无效尝试：`m4d0-preflight-fb641261/` 使用了错误的 `/v2/version` 路径，在收到
  404 后即停止；`m4d0-preflight-fb641261-retry-1/` 将 `pgrep` 的正常“零进程”退出码
  与 `pipefail` 组合误判为失败。两目录均写入 `INVALIDATED.txt`，不得作为通过证据；
  两次尝试都没有改变 Server、Worker、容器、端口或 GPU 状态。
- 二进制边界：当前构建产物 SHA-256 为
  `fa645ae457c656039e2a75fbf28cce96888e7503de15d4007946a7fde09b3ece`，
  内嵌 M4-C commit `c76fa3903b3b`，不是当前文档 HEAD。正式 M4-D 必须在最终干净
  HEAD 上重新离线构建并重新记录 binary identity，随后进行即时复检，不能把本次已有
  binary 当作正式维护集成产物。
- 当前停止位置：Python Server/Worker 保持运行，Rust Worker 保持停止，
  `20150/20151` 空闲。M4-D 没有因预检通过而自动获得授权。
- 下一项允许执行的工作：讨论并冻结最终构建、即时复检、精确
  `gpustack-worker` stop/start、resource-api-only 启停、在线 API/metrics、Server 双
  A100 上报、失败恢复和结束复核命令。实际执行须由用户另行明确批准。

### 2026-10-21：M4-D 正式集成尝试 1 失败并完成恢复

- 结果：状态为 `failed`，不是 `invalidated`。用户明确授权维护后，脚本通过最终预检，
  只停止 `gpustack-worker`，并以保留的 Worker ID 2 成功启动 resource-api-only；
  Rust 绑定 `20150/20151`、恢复持久化身份、枚举两张 GPU，heartbeat、status、
  full sync 和三资源 watch 均成功。在线 API 的 `/version` 身份门禁随后发现
  `git_commit="unknown"`，因此在进入 190 秒观察前停止。
- 失败原因：`GPUSTACK_GIT_COMMIT` 和 `GPUSTACK_BUILD_TARGET` 只由
  `gpustack-cli/build.rs` 注入 CLI package。Cargo build-script 环境变量不会传递给
  依赖 package，`gpustack-worker` 中的 `option_env!` 因而退回 `unknown`。CLI
  `version` 正确报告 `a80922abd0ed`，但 Worker HTTP `/version` 没有获得同一身份。
- 恢复结果：退出 trap 向 Rust 发送 SIGINT，Rust 摘要报告 Worker ID 2、同一 UUID、
  两张 GPU、四个控制面健康位为 true、重连次数 0 和 `shutdown_signal=interrupt`；
  随后恢复原 `gpustack-worker`。复核 Server、Python Worker health 和 Python metrics
  均为 HTTP 200，Rust 进程为 0，`20150/20151` 无监听。独立 480 秒 watchdog 观察到
  主流程已完成，没有触发第二次恢复。
- 失败证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d-formal-a80922ab`
  为 `0700`，文件为 `0600`。`FAILED.txt` SHA-256 为
  `f1c1ea6fc72edf73d40e9cd5fffdd4f72351d8a333b9afbc4f9190b44ef0b443`，
  `run-summary.json` 为
  `4cf99582bfa9f5a7821a5d818e03e819aad5258cddf4064c40fffc8f6c67766b`，
  `cleanup-result.txt` 为
  `281d5755036ccfa2e330ebc1b389ce5026be6ae3f6949e75659286fef8c513cf`。
- 修复：为 `gpustack-worker` 增加自身 build script，并在 Worker API 中使用强制存在的
  package build identity；新增测试验证 HTTP version 使用 Worker package 的 commit、
  target 和固定 Server baseline。关键注释说明 build-script 变量的 package 作用域，
  没有增加逐行或历史叙述注释。
- 修复验证：Rust 1.99.1 下 fmt、全 workspace/all targets/all features Clippy
  `-D warnings`、全部 63 项测试、locked/offline build 和 diff check 均通过；其中
  `gpustack-worker` 从 44 项增加到 45 项。修复尚未获得第二次真实维护集成证据，
  不能据此把 M4-D 标记为通过。
- 当前停止位置：Python Server/Worker 已恢复，Rust Worker 已停止，修复待提交；
  M4-D 仍为未接受。
- 下一项允许执行的工作：提交并推送修复，从新的干净 HEAD 生成新二进制和证据目录；
  再次停止 Python Worker 前确认维护授权仍适用于重试。

### 2026-10-21：M4-D 正式集成重试通过并待审阅

- 结果：状态为 `ready_for_review`。用户单独授权第二次维护后，修复后的
  resource-api-only 在真实 v2.2.3 Server 上持续运行 231 秒，全部冻结门禁和独立结束
  复核通过；M4-D 和整个 M4 尚待聊天中的详细报告与用户共同验收，暂不标记
  `accepted`。
- 输入身份：源码与 origin 均为
  `a3283b665a63f7cd46e1f0c5d270cbd42f05be95`，工作树干净；CLI/Worker 二进制
  SHA-256 为
  `b8695eab4aecd38e98c1dc99e292a5e688738fd2a067667376361e4c533a7468`，
  内嵌 commit `a3283b665a63`；`Cargo.lock` SHA-256 为
  `ad5fabd70e55fcbee197c3a644b8acf530ba943ca385c9344e7e6cfb06cb19c3`。
- 服务切换：最终预检通过后只停止容器 `gpustack-worker`，Server 始终为 HTTP 200，
  Python `10150/10151` 停止后才启动 Rust。独立 480 秒 watchdog 与退出 trap 同时
  生效；正式流程完成后 watchdog 未触发兜底恢复。
- Worker API：宿主机和 Server 容器到 Rust `/healthz` 均为 200；`/readyz` 按设计为
  503；修复后的 `/version` 返回 `a3283b665a63` 和固定 Server baseline。无凭据
  `/token-auth` 为 401，per-Worker token 为 200；模型目录 exists、`config.json` 和
  权重大小接口均为 200，Qwen-7B 固定权重结果为 `15442677288` bytes；含 `..` 的路径
  为 400，根目录外路径为 403。日志 options 为 200、缺失 serve log 为 404；只读配置
  为 200 且无 secret/Docker socket，PUT 为 405。
- 资源与 metrics：Server 三次快照均保持 Worker ID 2、name/UUID 不变、maintenance
  true、`metrics_port=20151`、两张 A100 的固定 UUID、名称、`85899345920` bytes 显存、
  compute capability 8.0 和 6912 cores。heartbeat 依次为
  `13:00:47Z`、`13:01:48Z`、`13:03:48Z`。最终 metrics 同时证明 mode、ready=0、
  Server connected、heartbeat/status success 和三资源 watch connected。
- 隔离与状态：运行前、运行中和恢复后两张 GPU 的显存占用与利用率均为 0，没有
  compute process；目标 Worker 的 ModelInstance 和 ModelFile 始终为 0；容器 ID/name/
  image 的前中后 diff 均为空，没有创建 Docker/vLLM workload。Rust SIGINT 摘要报告
  heartbeat、status、full sync、watch 均成功，重连次数为 0。
- 恢复：Rust 优雅停止后 `gpustack-rs` 进程为 0，`20150/20151` 无监听；原
  `gpustack-worker` 以相同 container ID、image 和 `unless-stopped` policy 恢复运行。
  Server、Python Worker health 和 Python metrics 均为 200；恢复后两张 GPU 仍为空闲。
- 证据安全：证据目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d-formal-a3283b66-retry-1`
  为 `0700`，全部文件为 `0600`，完整 manifest 校验通过；真实 per-Worker token 和
  Authorization/X-API-Key header 扫描均未命中。
- 关键证据哈希：`integration-result.txt` 为
  `6a5356cb01688b4e91cda78d13b6b45ae96cd1616a9c328250e0bac389fbb137`，
  `run-summary.json` 为
  `4cf99582bfa9f5a7821a5d818e03e819aad5258cddf4064c40fffc8f6c67766b`，
  `worker-final.json` 为
  `9ecbde4bc4cedcb23c8e45f69efc7607fc84669c284a26822f22eed4e279aed0`，
  `metrics-final.prom` 为
  `83a5857cd0d476d4dfb56bd4dc1ae1510ea646a882f240ab9799732da6d01371`，
  `final-isolation.txt` 为
  `aab26c52c0cc7f9ff1051ec0b33134d60935b61b266b26f45d477354ca341a69`。
- 当前停止位置：Python Server/Worker 已恢复，Rust Worker 已停止，`20150/20151`
  空闲；M4-D 等待详细报告和用户审阅。
- 下一项允许执行的工作：直接交付 M4-D 详细实现与验证报告。用户接受后记录
  M4-D/M4 `accepted`，再讨论下一里程碑；不得把本结果解释为 Docker/vLLM 或模型推理
  能力已经实现。

### 2026-10-21：M4-D.1 文件系统空间语义修正待审阅

- 触发原因：M4-D 审阅时，Web 节点列表中 Rust Worker 的磁盘为 5%，Python Worker
  为 56%。UI 代码固定查找 `mount_point == "/"` 并显示 `used / total` 的四舍五入
  百分比。M4-D 保存的 Rust 根挂载快照为 `4.891248%`，Python Worker 当前根挂载
  metrics 为 `55.7049%`，两者与页面精确对应。
- 解释边界：Rust 作为宿主机进程时，`/` 是 15 TB 的系统逻辑卷；Python Worker
  容器中的 `/` 是位于 `/dev/sda1` 上的 Docker overlay。该差异来自第一阶段冻结的
  部署方式和 Linux mount namespace，不把宿主机 `/var` 或数据盘伪装成 `/`。未来
  Rust Worker 容器化后再按相同容器挂载验证展示一致性。
- 发现的兼容缺陷：Rust 原实现把 sysinfo 的 `available_space` 同时填入 `free` 和
  `available`，并用 `total - available` 计算 `used`；这会把 ext4 为特权用户保留的
  空间计为已用，与 Python/Fastfetch 的 `free`、`available` 和 `used` 语义不一致。
  因此旧 M4-D 文件系统数值不能作为字段兼容性通过证据。
- 修正：Unix 平台通过安全的 `rustix::fs::statvfs` 分别读取 `f_bfree` 和
  `f_bavail`，按 `used = total - free` 生成四个容量字段；零 block size 或无法查询的
  异常挂载点保留 sysinfo 回退，避免单个失效 overlay 路径阻断整次资源采集。`rustix`
  已存在于冻结锁文件中，本次仅将同一版本提升为 Worker 的直接 `fs` 依赖，没有升级
  或下载依赖。
- 回归验证：新增预留块语义和失败回退两项测试；`gpustack-worker` 47 项、全 workspace
  65 项测试全部通过，fmt、全 workspace/all targets/all features Clippy
  `-D warnings`、locked/offline build 和 diff check 均通过。
- 实现身份：代码提交为
  `ad4ebfa39234de22690f8a28ee79b273eb3acc7d`；更新后的 `Cargo.lock` SHA-256 为
  `09d6abfb78214378deeb274ba6dc0f1e0b71e994759f6c13e2bfc7f30308849e`。
- 隔离：本次只修改、编译和测试 90 主工作树，没有启动 Rust Worker、停止容器、调用
  Server 写接口或使用 GPU。现有 Python Server/Worker 不属于本次变更对象。
- 结果：M4-D.1 状态为 `ready_for_review`。原 M4-D 的身份、API、GPU、控制面、隔离和
  恢复证据继续有效，但文件系统字段门禁被本次发现阻止；在新二进制完成一次在线资源
  快照复核前，M4-D 和整个 M4 都不得标记 `accepted`。
- 当前停止位置：实现和离线验证完成，等待详细报告与用户审阅；Rust Worker 保持停止。
- 下一项允许执行的工作：提交并推送该修正。用户接受离线实现后，单独讨论最小维护
  复核；再次停止 `gpustack-worker` 或启动 Rust Worker 仍须新的明确维护授权。

### 2026-10-21：M4-D.2 文件系统在线复核只读预检通过

- 结果：只读预检状态为 `passed`，M4-D.2 在线运行尚未执行。源码、HEAD 与 origin
  均为治理检查点 `91cf21df79225e4d7b11dacf60c72c20fd5bd387`，工作树干净；从该
  HEAD locked/offline 重建的 binary 内嵌 commit 为 `91cf21df7922`，SHA-256 为
  `27604b4f9e47bd274a2356bde32368c0f835aae34ef5507829167b018f9a47c0`。
- 身份与任务隔离：per-Worker 身份仍唯一映射到 Worker ID 2
  `rust-m3d-r760xa`，maintenance 为 true；目标 Worker 的 ModelInstance 和
  ModelFile 数量均为 0。token 文件保持普通文件、单硬链接和 `0600`，只通过 curl
  标准输入构造内存配置，未进入参数、输出或证据。
- 服务与设备：Server、Python Worker health 和 Python metrics 均为 HTTP 200；Rust
  进程为 0，`20150/20151` 无监听；`gpustack` 与 `gpustack-worker` 容器继续运行。
  两张 A100 的显存占用和利用率均为 0，没有 compute process。
- 文件系统基线：同一时点宿主机 `/` 的 total、used、free、available 分别为
  `15590638252032`、`127894986752`、`15462743265280`、`14828061691904`
  bytes；新口径利用率为 `0.820331950%`，UI 四舍五入预期为 `1%`。
- 有效证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d2-filesystem-preflight-91cf21df`
  为 `0700`，文件为 `0600`；`result.txt` SHA-256 为
  `11f9af98a6bd8dc7d7ee17c3abdcb1b4a4e6123f3b1cf1a88a9dfde66e557e23`，
  `build-identity.txt` 为
  `3a0aa8ffadfbdee5dd7fa59d8d6533e16dcd8e4ab4172407e3232f14ac3c01aa`，
  `root-statvfs.txt` 为
  `b34c2921940bc69d2cadcb6b2efebf4e14bb741e0af6e78a58a757bc360f26b6`；
  header/secret marker 扫描为 0。
- 安全边界：本次只重建二进制并执行只读查询，没有启动 Rust Worker、停止或重启
  Python Worker、调用 Server 写接口、创建容器或使用 GPU。
- 当前停止位置：M4-D.2 已通过启动前门禁，Rust Worker 仍停止；M4-D/M4 仍未接受。
- 下一项允许执行的工作：用户明确授权后，短时启动 maintenance
  `resource-api-only`，捕获 Server 根挂载快照与 Rust metrics，并与同一时点
  `statvfs/df` 比较后优雅停止。未经授权不得启动 Rust Worker；若选择再次停止
  `gpustack-worker`，还须明确包含该操作的维护授权。

### 2026-10-21：M4-D.2 文件系统语义在线复核通过并待审阅

- 结果：状态为 `ready_for_review`。在 Python Worker 保持运行的前提下，短时启动
  maintenance `resource-api-only`，成功捕获 Server Worker 快照、Rust metrics 与同一
  时点宿主机 `df/statvfs` 结果，随后优雅停止 Rust Worker。M4-D.1、M4-D 和整个 M4
  仍等待聊天中的详细报告与用户共同验收，暂不标记 `accepted`。
- 输入身份：正式运行源码与 origin 均为
  `f6faa63e9c03dba91f25b14f10fa1a4eeb53a52c`，工作树干净；locked/offline 重建
  binary 内嵌 commit `f6faa63e9c03`，SHA-256 为
  `00de77e7943a0a1a6396ec4ab98d91f4394e291c23e11b38212083bbdaa37a15`；
  `Cargo.lock` SHA-256 为
  `09d6abfb78214378deeb274ba6dc0f1e0b71e994759f6c13e2bfc7f30308849e`。
  文件系统语义修正对应代码提交
  `ad4ebfa39234de22690f8a28ee79b273eb3acc7d`。
- 在线资源结果：Server 根挂载的 total、used、free、available 分别为
  `15590638252032`、`127895011328`、`15462743240704`、
  `14828061667328` bytes，满足 `total = used + free`；利用率为
  `0.820332107%`，Web UI 按现有显示逻辑四舍五入为 `1%`。同一时点宿主机值仅有
  `12288` bytes 的采样期间写入差，Rust metrics 的 total 与 used 也与宿主机采样一致。
- 差异结论：Rust Worker 的根挂载继续表示宿主机 15 TB 逻辑卷，因此修正后的页面值
  应为约 `1%`；Python Worker 容器的 `/` 是 Docker overlay，页面约 `56%`。本次修正
  对齐的是 `free`、`available`、`used` 字段语义，不跨 mount namespace 伪造相同容量。
- 控制面：Worker ID 2、名称 `rust-m3d-r760xa` 和既有 UUID 均从持久化身份恢复；两张
  A100 被正常枚举，heartbeat、status、full sync 和 watch 均成功，重连次数为 0。
  运行期间目标 Worker 的 ModelInstance 与 ModelFile 数量均为 0，没有调度或模型工作负载。
- 共存与隔离：本次没有停止或重启 Python Worker；Rust 运行期间 Server、Python Worker
  health 和 Python metrics 均为 HTTP 200。容器 ID/name/image 前后 diff 为空，没有创建
  Docker/vLLM 容器；两张 A100 前后均为 0 MiB、0%，没有 compute process。
- 结束状态：Rust 收到 SIGINT 后优雅停止，`gpustack-rs` 进程为 0，`20150/20151`
  无监听；Server、Python Worker health 和 Python metrics 继续为 HTTP 200。
- 凭据边界：注册 bootstrap token 文件当前不存在；正式运行使用
  `data_dir/managed/worker-token` 中权限为 `0600` 的持久化 per-Worker 凭据，运行事件
  报告 `identity_source=persisted`。真实 token 值和 Authorization/X-API-Key header
  对正式证据目录的扫描均为 0 命中。
- 有效证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d2-filesystem-online-f6faa63e-retry-2`
  为 `0700`，全部文件为 `0600`，完整 `SHA256SUMS` 校验通过。`result.txt` SHA-256 为
  `52c149afda586f1530405dc770fd68c88d58c72acc228b0f9a53f7e9d893d2d9`，
  `filesystem-comparison.txt` 为
  `f5d84ef2fcc937333148536b9b8c654feaa553eb43a9f492ca8ecbde9a722697`，
  `worker-online.json` 为
  `8f4600bff21159956040f192b0a047b9e1eee331bcc1de7743652c33a45b4f0e`，
  `rust-metrics.prom` 为
  `ab051f29fc028679879936ed0338b85b2f9c84f9985a96353046f2413c2ee01f`，
  `final-isolation.txt` 为
  `7fa837729a8fbc05865092f526f5d494f3502b1d3bbddd7231f716ad778d58f7`。
- 无效尝试：首次脚本错误轮询 `127.0.0.1`，而 Rust 仅绑定
  `10.213.80.90`；第二次脚本把顶层 `metrics_port` 错写成
  `status.metrics_port`。两次均在 Rust 成功启动并优雅停止后留下
  `INVALIDATED.txt`，Server/Python 服务保持 200 且没有残留进程或端口；这些目录只用于
  说明测试工具错误，不作为正式通过证据。
- 当前停止位置：Python Server/Worker 正常运行，Rust Worker 已停止，`20150/20151`
  空闲；M4-D.2、M4-D.1、M4-D 与 M4 等待详细报告和用户审阅。
- 下一项允许执行的工作：直接交付本次实现与验证报告。用户接受后记录
  M4-D.1、M4-D.2、M4-D 和 M4 为 `accepted`，再讨论下一里程碑；不得把本次结果解释为
  Docker/vLLM 或模型推理能力已经实现。

### 2026-10-21：M4-D 文件系统闭环与 M4 正式接受

- 结果：用户审阅 M4-D.1 文件系统语义修正、M4-D.2 在线复核、正式证据和服务隔离
  报告后确认无问题；M4-D.1、M4-D.2、M4-D 与整个 M4 均由
  `ready_for_review` 更新为 `accepted`，M4 阶段完成。
- 接受范围：Rust Worker 已通过固定 v2.2.3 Server 的 maintenance
  resource-api-only 集成，包含双 A100 资源上报、Worker API、per-Worker token 认证、
  文件/日志/配置读取、Prometheus metrics、控制面保活、路径与脱敏边界以及 Python
  Worker 回滚。根文件系统的 `free`、`available` 和 `used = total - free` 已与 Linux
  `statvfs` 语义对齐，并由 Server、metrics 与宿主机同点采样交叉验证。
- 接受证据：主要 M4-D 正式证据目录为
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d-formal-a3283b66-retry-1`；
  文件系统闭环证据目录为
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m4d2-filesystem-online-f6faa63e-retry-2`。
  文件系统修正代码提交为
  `ad4ebfa39234de22690f8a28ee79b273eb3acc7d`，在线复核记录提交为
  `61c760a07c73975b3182604e557106d6c488bf94`；无效和失败尝试继续保持原状态，
  不提升为接受证据。
- 明确非结论：M4 没有实现或验证 Docker workload、vLLM 模型启动、Lab 推理、Managed
  ModelInstance 状态推进、跨 Worker 分布式推理或 Rust Worker 容器化；M4 的接受不授权
  任何新的维护窗口、停止 Python Worker、创建容器或使用 GPU。
- 当前停止位置：Python Server/Worker 正常运行，Rust Worker 已停止，`20150/20151`
  空闲；远端分支和 origin 同步，工作树干净。
- 下一项允许执行的工作：进入 M5 Docker runtime 与 Lab 的设计讨论，细化 Docker
  原语、所有权标签、WorkloadSpec、vLLM builder、本地状态、CLI、健康、日志与清理门禁。
  设计经用户冻结后才能开始离线实现；真实 Docker/vLLM/GPU 验证仍须单独维护授权。

### 2026-10-21：M5 Lab 与 Managed 核心边界确认

- 结果：用户确认继续采用既有 D019、D020、D021 与 D038；M5 保留
  `worker lab`，但它只是共享生产 runtime 的薄适配层，不发展为第二套模型管理或生产
  控制面。该核心边界状态为 `accepted`，没有修改 M4 结论。
- CLI 边界：`worker lab deploy` 直接驱动隔离的本地期望状态，用于演示和诊断 Rust
  Worker 的 Docker/vLLM 执行能力；`model deploy` 必须调用 Server API，由 Server
  创建 Model/ModelInstance/ModelFile 并保持期望状态权威。两种命令不得自动回退或
  静默切换模式，Lab 推理继续使用显式 `worker lab infer`。
- 复用约束：Lab 与 Managed 必须共用模型路径校验、分片解析、WorkloadSpec、planner、
  Docker runtime、日志、健康检查和清理原语。Lab 只增加本地状态 adapter、`mode=lab`
  所有权标签及 CLI 参数转换；不注册 Server、不使用 Worker/user token、不创建
  Server 数据库记录或 gateway route。
- 证据边界：M5 Lab 证据只证明 runtime 的 diagnostic/smoke 能力；M6 仍须通过
  `model deploy`、完整 ModelFile/ModelInstance 状态链和 Managed 推理单独验收。Lab
  成功不得提升为 Managed 兼容结论。
- 当前停止位置：M5 的 Lab/Managed 角色、共享实现原则和 CLI 分流已确认，具体
  Docker/runtime 契约与实现分段尚未冻结，也尚未开始编码。
- 下一项允许执行的工作：继续讨论并冻结 M5 的 WorkloadSpec、所有权标签、Runner/vLLM
  参数、GPU/端口/路径分配、本地状态、CLI 行为、健康与日志、幂等清理以及离线和真实
  Docker 验收门禁。当前确认不授权创建容器、停止 Python Worker 或使用 GPU。

### 2026-10-21：M5-A Docker runtime 与 Lab 契约接受

- 结果：用户审阅并接受 M5 Docker/runtime 详细设计；M5-A 状态为 `accepted`，允许
  进入 M5-B 离线 runtime 实现，但没有授予任何真实 Docker、GPU 或服务切换权限。
- 冻结架构：Lab 与 Managed 共用 `WorkloadSpecV1`、planner、reconciler、Docker
  runtime、健康、日志和清理，只保留不同的 desired-state adapter 与状态回写目标。
  Rust 通过 Docker Engine API 原生实现 R1 子集，不依赖 Python runtime 或调用
  `docker` CLI。
- 工作负载契约：normalized spec 使用规范化 JSON SHA-256；所有容器带 Rust、schema、
  mode、local owner、component、deployment 和 spec hash 标签。每次 stop/remove/
  cleanup 前重新验证完整 tuple，任何 Python、Managed、未知或冲突资源均不得修改。
- Lab 契约：单 GPU、tensor parallel size 1、loopback `127.0.0.1`、自动端口范围
  `42000-42063`。同名同 spec 幂等，同名异 spec 拒绝；Docker running 不等于就绪，
  `/v1/models` 必须包含 served model 才进入 RUNNING；失败保留现场，stop 不删除，
  cleanup 默认 dry-run。
- Runner 安全基线：使用 M1 固定 digest 和 `pull=never`，host network、10 GiB shm、
  NVIDIA DeviceRequest、只读模型 mount、独立 cache、restart policy `no`、
  `no-new-privileges`，默认非 privileged。若真实 smoke 证明需要放宽，必须作为设计
  修订先讨论，不能在实现或测试中静默改变。
- 只读 oracle：固定 Python Server/Worker commit
  `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` 的 vLLM 路径使用 `vllm serve`、host
  network、无 Docker restart 和 `/v1/models` 健康判定；固定 Runner digest
  `sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af`
  的 entrypoint 为 `tini --`。Python `gpustack-runtime 0.2.1` 和
  `gpustack-runner 0.1.27.post4` 只作为 oracle，不成为 Rust 依赖。
- 分段门禁：M5-B 是 fake/mock Docker runtime 离线实现；M5-C 是 Lab adapter/CLI
  离线实现；M5-D 需另行授权无 GPU Docker 集成；M5-E 需单独维护窗口完成 Qwen-7B
  单 GPU smoke 并恢复 Python Worker。M5-D 不能替代 M5-E，Lab smoke 不能替代 M6
  Managed 验收。
- 本次安全边界：只进行源码、安装包和 image metadata 的只读检查以及治理文档更新，
  没有连接 Docker socket 执行变更、创建或删除 container、停止 Python Worker、
  使用 GPU、加载模型或调用 Server 写接口。
- 当前停止位置：M5-A 契约已经冻结，M5 尚未开始代码实现；现有 Python Server/Worker
  保持原运行状态，Rust Worker 停止。
- 下一项允许执行的工作：进入 M5-B，先审计 Rust Docker client 的准确版本、feature、
  transitive dependency、Rust 1.99.1/MSRV 和 90 主机 Cargo cache/offline 可用性；
  审计通过后只实现 fake/mock 可验证的 runtime 基础。真实 Docker 仍未授权。

### 2026-10-22：M5-B Docker runtime 离线实现

- 结果：M5-B 状态为 `ready_for_review`。代码检查点
  `fbaf1a0426c02b82028aa1b836d42ea0138340cd` 实现 `WorkloadSpecV1`、规范化
  SHA-256、完整所有权标签核验、受控 vLLM command/container plan，以及可替换的
  Docker Engine API runtime 边界；尚未进入 M5-C。
- 依赖审计：独立 scratch crate 固定 `bollard 0.21.1`，关闭 default features 并只启用
  `pipe`，同时固定 `sha2 0.10.9`。Rust 1.99.1 的 online check、完整 offline
  metadata 和 clean target locked/offline check 均通过；可选 feature 图未启用 TLS、
  SSH、BuildKit 或 websocket。实际 workspace `Cargo.lock` SHA-256 为
  `fff13ee7507e6dc5874fad87931e5a5c4da98782ca8c6c9fa3dd825108799afe`。
- runtime 实现：`gpustack-runtime::docker` 定义 daemon/image/container/log 类型和
  object-safe `ContainerRuntime`，并用 Bollard 映射 ping/version、image inspect、
  label-filtered list、create/start/inspect/stop/remove 和 stdout/stderr log stream。
  create body 在发出请求前验证必要字段和绝对 mount，并显式映射 host network、bind
  mounts、NVIDIA DeviceRequest、restart `no`、非 privileged、security options、shm
  与 stop timeout。
- workload 实现：`gpustack-worker::workload` 只允许 M5 单 GPU 和 TP=1，要求稳定 NVIDIA
  UUID 与不可变 image digest，区分 Lab/Managed 标签和 bind address，固定模型只读、
  cache 可写、10 GiB shm、`no-new-privileges` 与 vLLM 健康契约。受保护的 endpoint、
  并行度、模型、cache、TLS 与认证参数不能通过 extra args 覆盖。
- 身份不变量：spec hash 对显式版本化、确定性排序后的结构计算，并排除 hash 自身；
  hash 随端口等身份输入变化。危险操作的共享前置检查要求实际容器逐项匹配 implementation、
  schema、mode、owner、component、deployment 和 spec hash，名称或本地记录不能替代标签。
- 自动化：`cargo fmt --all --check`、全 workspace/all-targets/all-features 的离线 locked
  Clippy（`-D warnings`）、78 个测试和 release build 全部通过；本阶段新增 13 个测试，
  覆盖 Docker 请求映射、日志、失败关闭、hash、命令、参数保护、所有权和安全默认值。
  仓库级 `make lint` 已尝试，但 90 未安装 `uv`，在启动 pre-commit 前以 127 退出且没有
  修改文件；因此本检查点不宣称 Python/pre-commit 全仓 lint 已通过。
- 注释审阅：只保留 3 条解释可替换 I/O 边界、完整标签核验和 hash 自排除原因的关键注释；
  没有逐字段、逐行或修改历史式注释。
- 安全边界：没有连接真实 Docker socket、创建或删除 container、停止 Python Worker、
  使用 GPU、加载 Runner/模型或调用 Server 写接口。M5-D/M5-E 授权边界保持不变。
- 隔离复核：Server `/healthz`、Python Worker `/healthz` 与 metrics 均返回 200；
  `gpustack-rs` 进程为 0，`20150/20151` 仍无监听。
- 当前停止位置：M5-B 等待详细实现报告和用户共同验收；D072 同为
  `ready_for_review`。用户接受前不得实现 M5-C。

### 2026-10-22：M5-B Docker runtime 离线实现接受

- 结果：用户审阅 M5-B 的依赖、runtime、workload planner、所有权、测试、隔离与限制
  详细报告后确认无问题；M5-B 与 D072 由 `ready_for_review` 更新为 `accepted`。
- 接受证据：代码检查点为 `fbaf1a0426c02b82028aa1b836d42ea0138340cd`，接受前
  治理 HEAD 为 `fc0b5408093efe39cddb60f567e9520670fd8730`，`Cargo.lock`
  SHA-256 为 `fff13ee7507e6dc5874fad87931e5a5c4da98782ca8c6c9fa3dd825108799afe`；
  78 个测试、离线 locked Clippy 和 release build 结果继续作为本阶段证据。
- 接受范围：M5-B 只证明 `WorkloadSpecV1`、Docker API 类型与调用边界、vLLM container
  plan、规范化 hash 和所有权失败关闭的离线正确性。没有把编译通过提升为真实 Docker、
  Runner、模型健康、推理或 Managed 兼容证据。
- 漂移审计：目标、v2.2.3 Server、单 GPU M5 范围、固定 Runner、Lab/Managed 分离、
  完整所有权标签和分段授权均未改变；结论为 `aligned`。
- 当前停止位置：M5-B 已接受，M5-C 尚未开始。Python Server/Worker 保持运行，Rust
  Worker 停止。
- 下一项允许执行的工作：进入 M5-C Lab adapter 与 CLI 的 fake/mock 离线实现；不得
  连接真实 Docker socket、创建或删除 container、停止 Python Worker、访问 GPU 或
  运行模型。

### 2026-10-22：M5-C Lab adapter 与 CLI 离线实现

- 结果：M5-C 状态为 `ready_for_review`。代码检查点
  `4cfd7d86af0eff7b36ac615266a7d3a7c98ac7f6` 实现隔离的 Lab 本地状态、完整
  CLI、可注入协调服务与失败关闭的资源操作；尚未进入 M5-D，也未把离线结果提升为
  真实 Docker、vLLM 或 GPU 证据。
- 状态与协调：`data_dir/lab` 使用 0700 目录、0600 文件、非阻塞排他锁和
  temp/fsync/rename 原子持久化 owner、deployment 与 cache；持久化
  `WorkloadSpecV1` 每次读取均重算 hash 并核验完整身份标签。Lab 在同一 data dir
  检测到 Managed lock 时拒绝 deploy，同名同 spec 幂等恢复，同名异 spec 冲突。
- 资源与运行边界：模型路径 canonicalize 后必须位于配置根目录，镜像必须是本地存在的
  Linux amd64 不可变 digest，GPU 只接受稳定 UUID/index 且在新建或重启前不得存在
  compute process，端口只绑定 `127.0.0.1` 并在 `42000-42063` 分配。HTTP 客户端显式
  禁用环境代理，健康只在 `/v1/models` 出现期望模型后进入 `RUNNING`。
- CLI 与安全行为：实现 `worker lab deploy|list|status|logs|infer|stop|remove|cleanup`；
  vLLM 参数经过结构化白名单和共享 workload builder，日志支持增量 follow，推理只访问
  owned loopback deployment。remove 仅允许 `STOPPED/FAILED`，cleanup 默认 dry-run，
  执行需要 `--execute --yes`；stop/remove/cleanup 均重新核验实际 Docker 标签，未知、
  冲突、运行中或缺少 manifest 的资源一律阻断。
- 故障与诊断：状态机完整保存 `CREATED -> PREPARING -> STARTING -> RUNNING` 以及
  `FAILED/STOPPED`；启动失败保留容器现场，记录有界、脱敏诊断和最多 20 行日志尾部。
  Docker 404 映射为 typed not-found，用于只重建已确认缺失的 owned container。
- 自动化：最终 `cargo fmt --all -- --check`、workspace/all-targets 的 locked/offline
  check、Clippy（`-D warnings`）、release build 与 100 个测试全部通过；M5-C 新增测试
  覆盖 CLI、原子存储/权限/锁、路径逃逸、GPU/端口冲突、状态推进、幂等、所有权、
  日志/推理、失败现场和两阶段清理。`Cargo.lock` SHA-256 为
  `8f86dfe7559d710f2abb5ec950cc047b19f383738961a0c839902406041362b2`。
- 仓库门禁限制：已运行 `make lint`，但 90 主机没有 `uv`，命令在 pre-commit 启动前
  以 127 退出且未修改文件；因此不宣称 Python/pre-commit 全仓 lint 已通过。变更没有
  修改 Python 文件、数据库 schema、route、依赖服务或 generated client。
- 注释审阅：关键注释只解释可替换 I/O 边界、持久化 identity 校验、完整标签核验与
  hash 自排除原因；没有逐行复述、修改历史式或无必要注释。
- 安全边界：本阶段没有连接真实 Docker socket、执行真实 NVML/HTTP 请求、创建或删除
  container、调用 Server 写接口、停止 Python Worker、使用 GPU、加载模型或发起推理。
- 当前停止位置：M5-C 等待详细实现报告和用户共同验收。代码已推送，90 工作树在代码
  检查点后干净；M5-D/M5-E 仍未授权。
- 下一项允许执行的工作：直接交付 M5-C 详细实现与验证报告。用户接受后记录 M5-C 为
  `accepted`，再单独讨论并授权 M5-D 无 GPU Docker 集成；不得自动进入真实 Docker
  或 GPU 测试。

### 2026-10-22：M5-C Lab adapter 与 CLI 离线实现接受

- 结果：用户审阅 M5-C 的本地状态、协调服务、CLI、状态机、幂等恢复、所有权保护、
  故障诊断、测试、隔离与限制报告后确认无问题；M5-C 由 `ready_for_review` 更新为
  `accepted`。
- 接受证据：代码检查点为 `4cfd7d86af0eff7b36ac615266a7d3a7c98ac7f6`，接受前
  治理 HEAD 为 `0a4642a6bd91adebd3d45bde5b7a59ac2af080d1`，`Cargo.lock`
  SHA-256 为 `8f86dfe7559d710f2abb5ec950cc047b19f383738961a0c839902406041362b2`；
  100 个测试、locked/offline check、Clippy（`-D warnings`）、格式和 release build
  结果继续作为本阶段证据。
- 接受范围：M5-C 只接受 Lab adapter/CLI 和共享 runtime 边界的离线实现。它不证明
  Docker daemon/image/container 原语在真实环境可用，也不证明 Runner、vLLM、模型
  健康、GPU 推理或 Managed 工作流兼容。
- 漂移审计：M5 单 GPU 范围、固定 Runner、Lab/Managed 分离、loopback、完整所有权
  tuple、两阶段清理与分段授权均未改变；结论为 `aligned`。
- 当前停止位置：M5-C 已接受。Python Server/Worker 保持运行，Rust Worker 停止，
  没有本阶段创建的容器、GPU 进程或 `127.0.0.1` Lab 端口监听。
- 下一项允许执行的工作：讨论并冻结 M5-D 无 GPU Docker 集成的准确命令、临时资源、
  所有权标签、失败 trap、清理证据和停止条件；连接真实 Docker socket 或创建容器前
  仍须获得用户的单独明确授权。M5-E GPU smoke 继续需要更严格的独立维护窗口授权。

### 2026-10-22：M5-D 无 GPU Docker 集成设计冻结

- 结果：用户接受 M5-D.0/M5-D.1/M5-D.2 分段、诊断容器契约、所有权拒绝、失败清理和
  证据方案；D073–D075 状态为 `accepted`。本次接受只授权 M5-D.0 离线准备。
- M5-D.0：实现冻结 CLI 中缺失的 `worker runtime info`，抽取生产 WorkloadSpec 与诊断
  probe 共用的完整 label 校验函数，并新增不随正式 CLI 发布、默认拒绝执行的 Rust
  example probe。所有行为通过 fake/mock 和静态门禁验证，不连接 Docker socket。
- M5-D.1：另获授权后只读连接 Docker，验证 daemon/API、固定缓存 Runner identity、
  Server/Python Worker/GPU 基线和无历史 M5-D 残留；失败时不进入容器创建。
- M5-D.2：再获授权后只创建一个唯一身份、network-none、无 GPU、无 mount、无端口、
  非 privileged 且最多运行 120 秒的固定 Runner 诊断容器；通过 Rust runtime 验证完整
  生命周期、日志、正确 tuple 和错误 owner 拒绝，并只读证明现有 Python 容器不归属。
- 清理与证据：正常路径不使用 force，任何 stop/remove 前都重新核验完整 tuple；失败时
  只有 tuple 完整匹配才自动清理。证据位于 Git 外 0700/0600 目录，必须记录源码、
  Cargo.lock、二进制、Docker/image、步骤、服务和最终零残留 identity，且不保存 secret。
- 解释边界：M5-D 只证明 Docker 原语和所有权防护，不能证明 vLLM、模型、GPU 或
  Managed 兼容；M5-E/M6 验收边界不变。漂移审计结论为 `aligned`。
- 当前停止位置：M5-D 设计已冻结，尚未开始 M5-D.0 源码实现；没有连接 Docker、读取
  GPU、创建容器或改变服务。
- 下一项允许执行的工作：实施并验证 M5-D.0，交付详细实现报告。M5-D.1 的 Docker
  只读访问和 M5-D.2 的容器变更仍未授权。

### 2026-10-22：M5-D.0 Docker 集成离线准备

- 结果：M5-D.0 状态为 `ready_for_review`。代码检查点
  `0fbc8d14b1792210e52aab782f92b15bc1fd9a32` 实现产品 CLI 的
  `worker runtime info`、生产 WorkloadSpec 与诊断 probe 共用的完整 label 校验，以及
  不随正式 CLI 发布的受保护 Rust example probe；尚未进入真实 Docker 读取或变更。
- CLI 与 runtime：`worker runtime info` 从有效配置取得 Docker socket 和 CLI timeout，
  通过现有 `ContainerRuntime::daemon_info` 返回 ping、Engine/API 最低版本、OS 和架构，
  支持稳定 table/JSON 输出；连接与 daemon 错误继续映射为 runtime 退出码。
- 诊断 probe：example 要求不可变 `repository@sha256` 镜像、受限 run ID 和显式
  `--execute`。执行门禁在 runtime 初始化前失败关闭；冻结 container spec 为
  network-none、无 mount、无 GPU、无端口、无环境变量、非 privileged、
  `no-new-privileges`、restart no 和 120 秒有界 shell。
- 所有权与清理：共享校验逐项要求完整 expected tuple。probe 在每次 stop/remove 和
  失败清理前重新 inspect 并核验完整标签，名称或 container ID 不作为所有权证明；正常
  清理不使用 force，冲突、未知或 transitional 状态均失败关闭。错误 owner 拒绝、现有
  Python 容器拒绝和最终 typed not-found/零过滤残留已编码为后续集成断言。
- 自动化：`cargo fmt --all -- --check`、workspace/all-targets 的 locked/offline check、
  两组 Clippy（含 all-features，均 `-D warnings`）、release build、example release build
  和 104 个测试全部通过。首次增量 check 因传输保留时间戳复用了旧 package artifact；
  仅清理 `gpustack-worker`/`gpustack-cli` 构建产物后，完整 locked/offline 门禁重跑通过。
  `Cargo.lock` SHA-256 为
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`。
- 默认拒绝证据：只运行了 CLI help 和未携带 `--execute` 的 example；后者在连接 runtime
  前以明确错误退出。没有运行 `worker runtime info`，也没有执行带 `--execute` 的 probe。
- 仓库门禁限制：已运行 `make lint`，但 90 主机没有 `uv`，命令在 pre-commit 启动前
  退出且未修改文件；因此不宣称 Python/pre-commit 全仓 lint 已通过。变更没有修改
  Python、数据库、route、generated client 或服务配置。
- 隔离复核：Server、Python Worker health 和 metrics 均返回 200；`gpustack-rs` 进程为
  0，`20150/20151` 无监听。阶段内未访问 Docker socket、daemon/image/container API、
  NVML/GPU、模型或 Server 写接口，未停止或重启任何现有服务。
- 当前停止位置：M5-D.0 等待详细实现报告和用户共同验收。代码已推送，90 工作树在代码
  检查点后干净；M5-D.1 的 Docker 只读访问、M5-D.2 的容器变更和 M5-E 的 GPU smoke
  均未授权。
- 下一项允许执行的工作：直接交付 M5-D.0 详细实现与验证报告。用户接受后才能讨论并
  单独授权 M5-D.1 只读 Docker 预检；不得自动连接 Docker 或进入容器生命周期测试。

### 2026-10-22：M5-D.0 Docker 集成离线准备接受

- 结果：用户审阅 `worker runtime info`、共享完整 label 校验、受保护 example probe、
  生命周期不变量、失败关闭清理、测试和隔离报告后确认无问题；M5-D.0 由
  `ready_for_review` 更新为 `accepted`。
- 接受证据：代码检查点为 `0fbc8d14b1792210e52aab782f92b15bc1fd9a32`，接受前
  治理 HEAD 为 `3f02c644d5928c7ae54ae8ca697d79a483dfe115`，`Cargo.lock`
  SHA-256 为 `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`；
  104 个测试、locked/offline check、两组 Clippy、release build 和默认拒绝执行结果继续
  作为本阶段证据。
- 接受范围：M5-D.0 只接受 CLI/runtime 接线、共享所有权检查和诊断 probe 的离线实现。
  它不证明真实 daemon/image/container API、Runner、vLLM、模型、GPU 或 Managed 兼容。
- 漂移审计：M5 单 GPU 范围、固定 Runner、Lab/Managed 分离、完整 ownership tuple、
  分段授权和 M5-E/M6 解释边界均未改变；结论为 `aligned`。
- 当前停止位置：M5-D.0 已接受。Python Server/Worker 保持运行，Rust Worker 停止；
  本阶段没有连接 Docker、创建容器或产生 GPU/端口残留。
- 下一项允许执行的工作：讨论并冻结 M5-D.1 只读 Docker 预检；实际连接 Docker socket
  前仍须用户单独明确授权。M5-D.2 容器变更和 M5-E GPU smoke 均继续禁止。

### 2026-10-22：M5-D.1 只读 Docker 预检冻结与授权

- 结果：用户接受 D076，并明确授权读取真实 Docker daemon、固定 Runner image、受保护
  容器元数据和 GPU 状态；M5-D.1 状态为 `ready`，尚未产生运行结果。
- 固定输入：从干净 `codex/rust-worker` HEAD 构建 release 二进制，Docker socket 为
  `/var/run/docker.sock`，Runner 固定为 M1 的 `quay.io/gpustack/runner@sha256:` identity，
  受保护容器为 `gpustack` 和 `gpustack-worker`，GPU UUID 仍为 M1 的两张 A100。
- 允许操作：运行 `worker runtime info`，使用精确格式的 image/container 只读查询，
  检查历史 M5-D 名称/标签残留，并在前后采集 Server、Python Worker、GPU、Rust 进程和
  端口基线；证据写入 Git 外 0700/0600 目录且不得包含 secret。
- 禁止操作：不得运行带 `--execute` 的 probe，不得 pull、create、start、stop、restart、
  remove、exec、读取容器日志或完整 inspect，不得停止服务、加载模型、运行推理或调用
  Server 写接口。任何 identity、健康、残留、GPU 或隔离门禁失败均停止且不现场修复。
- 解释边界：通过只证明 Rust daemon 读取路径和 M5-D.2 环境前置条件，不证明容器生命
  周期、Runner shell、vLLM、模型、GPU 推理或 Managed 兼容；漂移审计结论为 `aligned`。
- 当前停止位置：M5-D.1 已授权但尚未执行；Python Server/Worker 保持运行。
- 下一项允许执行的工作：执行 M5-D.1 只读预检，完成后停在 `ready_for_review` 并交付
  详细报告。M5-D.2 继续未授权。

### 2026-10-22：M5-D.1 首次只读预检被 GPU 前置条件阻断

- 结果：M5-D.1 在运行 Rust daemon 查询和 Runner image inspect 前以
  `stopped_precondition` 停止，不是 Rust runtime 失败，也不是通过结果。
- 输入 identity：源码与 origin 均为
  `861c6f47032f8d09726aa11023f2f8be12212357`，release 二进制 SHA-256 为
  `cea24c586d2688e4ce740ba8b62b16af721b3b8879fc99294a53ced85435a56b`，内嵌 commit
  为 `861c6f47032f`，`Cargo.lock` SHA-256 仍为
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`。
- 阻断事实：GPU 1 `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` 在执行前占用约
  48034 MiB、利用率 100%，存在 PID 2844996、用户 `liuwang` 的
  `VLLM::EngineCore`；执行后同一进程仍存在。项目没有停止、信号或修改该进程。
- 已完成只读检查：Server、Python Worker health 和 metrics 前后均为 200；受保护容器
  identity、running 状态、started-at 和 restart count 前后完全一致；完整受限容器清单
  一致；M5-D 名称/标签残留、Rust 进程和 `20150/20151` 监听均为 0。
- 未执行操作：没有运行 `worker runtime info`，没有 inspect 固定 Runner，没有执行
  probe、pull、create/start/stop/restart/remove/exec、容器日志、服务变更、Server 写入
  或模型推理；Docker 只读访问仅限容器列表和受保护容器的精确字段。
- 证据：有效停止记录位于 Git 外
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5d1-readonly-20261022T115722+0800/`，
  目录/文件权限为 0700/0600，24 个文件，secret scan 通过；`manifest.sha256` 自身
  SHA-256 为 `dbc90643583833801418fe89704e864ce925fbd19b18a85754290c30cf0bdac5`。
- 无效尝试：此前目录 `m5d1-readonly-20261022T115635+0800/` 因在仓库根目录而不是
  `rust/` 调用 Cargo，随后读取到旧二进制，被标记为 `invalidated`；该尝试没有访问
  Docker，不能作为 M5-D.1 证据。
- 解释：空闲 GPU 是 M5-D.1 进入 daemon/image 查询的冻结前置条件。外部用户工作负载
  使本次停止，不能据此评价 Rust Docker runtime、Runner identity 或 M5-D.2 可行性；
  漂移审计结论仍为 `aligned`。
- 当前停止位置：M5-D.1 尚未接受，Python Server/Worker 与外部 GPU 工作负载均保持原状。
- 下一项允许执行的工作：向用户交付阻断报告。只有在用户确认 GPU 1 工作负载已合法
  结束并允许重试后，才可重新执行 M5-D.1；不得由本项目清理未知进程或容器。

### 2026-10-22：M5-D.1 共享 GPU 主机隔离修订

- 结果：用户确认 GPU0 空闲并接受继续方案；D077 状态为 `accepted`，允许按修订门禁
  重试 M5-D.1。首次 `stopped_precondition` 记录继续有效，不被改写为失败或通过。
- 修订内容：M5-D.1 和 M5-D.2 都不得申请 GPU，只要求未来目标 GPU0
  `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` 无计算进程并保持空闲。GPU1 上已识别的
  外部任务可以继续运行，但必须记录前后 UUID、PID/进程 identity 和资源快照；本项目
  不得停止、信号或修改它，快照发生不可归因变化时本次结果不通过。
- 后续 GPU 边界：M5-E 仍需独立维护窗口，只能按 GPU0 精确 UUID 分配；禁止使用 GPU
  index 范围、`all` 或同时申请 GPU1，且 Python Worker drain 与恢复门禁保持不变。
- 解释：这是基于稳定设备 UUID 的隔离修订，不把无 GPU M5-D 结果提升为 GPU 兼容证据，
  也不降低容器所有权、服务不变和分段授权要求；漂移审计结论为 `intentional change`
  且已获用户接受。
- 当前停止位置：M5-D.1 可按修订门禁重试，M5-D.2/M5-E 仍未授权。
- 下一项允许执行的工作：记录干净源码和二进制 identity，确认 GPU0 空闲及 GPU1 外部
  任务可归因后，完成 M5-D.1 的 Rust daemon 与固定 Runner 只读查询和前后对照。

### 2026-10-22：M5-D.1 修订后只读预检通过并待审阅

- 结果：修订后的 M5-D.1 只读门禁通过，状态为 `ready_for_review`，尚未标记
  `accepted`。源码与 origin 均为
  `34ac580699013d0cd06719cff21296e2969c96d3`；release 二进制 SHA-256 为
  `9b52f7f2dc55fa00ccc20a39d139a7c6a2915e5ddfdab8cad0f518a2733501c0`，内嵌 commit
  为 `34ac58069901`；`Cargo.lock` SHA-256 为
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`。
- Docker 与 Runner：`worker runtime info` 成功完成 daemon ping，报告 Engine
  `29.1.3`、API `1.52`、最低 API `1.44`、`linux/amd64`。缓存 Runner
  `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af`
  的 image ID、repo digest、平台和 `tini --` 入口与固定 identity 一致；没有 pull。
- GPU 隔离：GPU0 `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` 前后均为 0 MiB、
  0% 且无计算进程。GPU1
  `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` 的外部任务在采样期间由 PID 变化且显存
  从 47988 MiB 变为 15994 MiB，但稳定 principal 均为 `liuwang` 的
  `VLLM::EngineCore`；容器清单完全一致且本项目只执行 daemon/version 与 image inspect
  读取，因此变化可归因于外部任务自身，不是本项目操作。
- 服务与残留：Server、Python Worker health 和 metrics 前后均为 200；完整受限容器清单
  前后相同，`gpustack` 与 `gpustack-worker` 的 identity、运行状态、restart count 和
  started-at 完全一致。M5-D 历史容器残留、`gpustack-rs` 进程及 `20150/20151` 监听
  前后均为 0；Rust Worker 保持停止。
- 执行校正：首次 CLI 调用把 `--docker-socket` 放在顶层，被 Clap 在初始化 runtime 前
  拒绝；随后按 `worker --docker-socket ... runtime info` 正确执行。首次 `jq` 断言把
  image inspect 对象误当数组，正确断言后通过。最初要求 GPU1 PID 完全相同的断言比
  D077 的“可归因且本项目不得修改”门禁更严格，因此保留为无效断言；按 UUID、用户和
  进程类型组成的稳定外部 principal 复核通过。这三项均保存在证据中，不计作通过步骤。
- 安全边界：没有执行 example probe，没有 pull、create、start、stop、restart、remove、
  exec、容器日志或完整 inspect，没有停止服务、发送 Server 写请求、加载模型或申请
  GPU。结果只证明 Rust Docker daemon 读取路径、固定 Runner identity 和 M5-D.2 的环境
  前置条件，不证明容器生命周期、Runner shell、vLLM、模型、GPU 推理或 Managed 兼容。
- 证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5d1-readonly-retry-20261022T121520+0800/`
  为 0700，30 个文件均为 0600，secret scan 通过；`manifest.sha256` 自身 SHA-256 为
  `805aa4425e1113a2b8d8d463d36ef7cd2648a94605b9fb278c09a3249059e012`，`result.txt`
  SHA-256 为 `a76fa70e28c746e06a097030d612d36f00c2258806f1bc4cbe68fa6ac16c5685`。
- 当前停止位置：M5-D.1 等待详细报告和用户共同验收；Python Server/Worker 正常运行，
  Rust Worker 停止，GPU0 空闲，GPU1 外部工作负载未被本项目修改。
- 下一项允许执行的工作：直接交付 M5-D.1 详细报告并等待审阅。M5-D.2 的无 GPU 容器
  生命周期变更和 M5-E 的 GPU0 smoke 仍需分别讨论并获得明确授权。

### 2026-10-22：M5-D.1 修订后只读预检接受

- 结果：用户审阅 Docker daemon、固定 Runner、服务、容器、GPU 隔离、零残留、执行
  校正和解释边界报告后确认无问题；M5-D.1 由 `ready_for_review` 更新为 `accepted`。
- 接受证据：正式结果记录检查点为
  `f561204852cf5fbd0628462bb090f8b6e031ded6`；正式证据目录、identity 和哈希保持上一
  里程碑记录，不因接受而重写首次阻断或无效断言。
- 接受范围：只证明真实 Docker daemon 读取路径、固定缓存 Runner identity、受保护
  服务不变、共享 GPU 主机隔离和 M5-D.2 环境前置条件；不证明 container lifecycle、
  Runner shell、vLLM、模型、GPU 推理或 Managed 兼容。
- 漂移审计：固定 Runner、无 GPU M5-D、完整 ownership tuple、Python 服务保护、分段
  授权和 M5-E/M6 解释边界均未改变；结论为 `aligned`。
- 当前停止位置：M5-D.1 已接受。Python Server/Worker 正常运行，Rust Worker 停止；
  没有本项目创建的容器、GPU 工作负载或端口残留。
- 下一项允许执行的工作：讨论并冻结 M5-D.2 的精确 preflight、唯一 run identity、
  probe 执行、失败关闭清理、前中后服务/GPU 对照和证据封存步骤。实际运行带
  `--execute` 的 probe 前仍须用户单独明确授权；M5-E 继续未授权。

### 2026-10-22：M5-D.2 无 GPU Docker 生命周期诊断待审阅

- 结果：用户明确授权只创建、启动、停止并删除一个无 GPU 诊断容器且不得停止 Python
  Worker；正式门禁通过，状态为 `ready_for_review`，尚未标记 `accepted`。run ID 为
  `d2-20261022-123342`，container ID 为
  `f8e31cc2a4478e8aaa5b82160aab2141af79a993699753960e0a70729fed83f1`。
- 输入 identity：源码与 origin 均为
  `25df8bbbd2a95ba7b1b9144ced30ebe24167104d`，工作树干净；locked/offline release
  example 二进制 SHA-256 为
  `7aa48fb60e56f21cdf9c602a4aa0374927a168702392f0fae68750aa0249f613`，
  `Cargo.lock` SHA-256 为
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`；固定 Runner
  digest 与 M1/M5-D.1 一致且前后未变，没有 pull。
- 生命周期：probe 在约 0.5 秒内通过 Rust runtime 完成 `created → running → exited →
  removed`，读取到 `m5d-ready`，Docker events 为
  `create,start,kill,stop,die,destroy`；其中 `kill` 是正常 stop 发出的 SIGTERM，不是
  force remove。最终按 container ID inspect 返回 not found，名称和 ownership tuple
  查询均为零残留。
- 安全契约：运行中受限 inspect 证明 `network=none`、mount/bind/device request/port
  binding 均为空、`privileged=false`、`no-new-privileges=true`、auto-remove false、
  restart policy `no`，命令只输出 readiness marker 并 sleep 120 秒。正确完整 tuple
  通过；错误 owner 被共享校验拒绝后诊断容器仍为 running。
- Python 服务保护：probe 在诊断容器 running 后、stop 前验证 `gpustack-worker` 不匹配
  M5-D ownership，随后按 ID/state 复核未变化；旁路观察也捕获两台受保护容器的
  ID、running、restart count 和 started-at 与前后完全相同。Server、Python Worker 和
  metrics 执行前、观察点和执行后均为 200；连续 45 组采样全部为 200。
- GPU 隔离：GPU0 前中后均为 0 MiB、0% 且无计算进程，实际诊断容器
  `device_requests=[]`。GPU1 在执行前空闲，执行中由外部用户 `liuwang` 启动
  `VLLM::EngineCore`；执行后该 principal 可识别。由于诊断容器没有 GPU request、GPU0
  保持空闲且所有项目操作只针对唯一 Docker tuple，该变化按 D077 归因于外部任务，本
  项目没有停止、信号或修改它。
- 执行校正：首次 Runner 只读格式化命令假设镜像存在 `Config.Cmd` 键，被 Docker Go
  template 拒绝，发生在任何容器变更前；空输出和校正说明已保留，随后用可选字段 `jq`
  复核通过。另一个要求 HTTP 样本时间戳必须落在 164 ms 容器存活区间的断言得到 0 个
  样本；该要求不是冻结门禁且比“服务前中后状态不变”更严格，保留为
  `invalid_non_scoring`。正式中段证据使用 Rust probe 的有序 protected-container inspect，
  不用该时间重叠断言替代或提升结果。
- 证据：Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5d2-lifecycle-20261022T123342+0800/`
  为 0700，66 个文件均为 0600，secret scan 通过；`manifest.sha256` 自身 SHA-256 为
  `71f3f6200b2ab7e073d333dcfff753855e8b07a749de6e0a8b8036a64af25759`，`result.txt`
  SHA-256 为 `36fe4783251c928ef873afcd7e3e78e048b6e261b139ce5df6b45f1d564f2425`。
- 解释边界：该 diagnostic 只证明固定 Runner 下的 Docker 生命周期、日志、实际隔离字段、
  ownership 拒绝、服务保护和清理，不证明 Runner vLLM entrypoint、模型加载、GPU 推理或
  Managed 控制链。漂移审计结论为 `aligned`。
- 当前停止位置：M5-D.2 等待详细报告和用户共同验收。Python Server/Worker 正常运行，
  Rust Worker 停止，诊断容器已删除，GPU0 空闲，GPU1 外部任务未被本项目修改。
- 下一项允许执行的工作：直接交付 M5-D.2 详细报告并等待审阅。用户接受后才能把
  M5-D.2 和整个 M5-D 标记为 `accepted` 并开始讨论 M5-E；不得自动进入 GPU smoke。

### 2026-10-22：M5-D.2 与整个 M5-D 正式接受

- 结果：用户审阅 M5-D.2 的生命周期、安全字段、所有权拒绝、Python 服务保护、共享
  GPU 隔离、清理和证据说明后确认无问题；M5-D.2 与整个 M5-D 由
  `ready_for_review` 更新为 `accepted`。
- 接受证据：正式结果记录检查点为
  `f4d2c6b488e9fddfab75838395a411f17be9b919`；测试源码、二进制、Runner、run ID、
  container ID、证据目录及哈希保持上一里程碑记录，不因接受而改写首次阻断、命令校正
  或 `invalid_non_scoring` 断言。
- 接受范围：M5-D 只证明 Rust Docker daemon 读取、固定 Runner identity、无 GPU
  container create/inspect/start/log/stop/remove、完整 ownership tuple、错误所有者和受
  保护容器拒绝、真实隔离字段、服务保护与零残留。它不证明 Runner vLLM entrypoint、
  Qwen-7B 加载、GPU 推理、Lab 完整 CLI smoke 或 Managed 控制链。
- 漂移审计：M5-A 的共享 runtime、固定 digest、单 GPU、loopback、Lab 状态机、失败
  保留与所有权清理契约，以及 D077 的 GPU0 精确 UUID/GPU1 外部任务隔离均未改变；
  结论为 `aligned`。
- 当前停止位置：Python Server/Worker 正常运行，Rust Worker 停止；M5-D.2 诊断容器已
  删除，没有本项目容器、进程、端口或 GPU0 残留。M5-E 尚未获得执行授权。
- 下一项允许执行的工作：只讨论并冻结 M5-E 的模型 identity、Runner/vLLM 参数、GPU0
  与端口门禁、Lab 命令序列、证据、失败清理和 Python Worker 回滚方案。实际停止服务、
  创建模型容器、挂载模型或申请 GPU0 前必须另获明确维护窗口授权。

### 2026-10-22：M5-E 单 GPU Lab smoke 设计冻结与 M5-E.0 授权

- 结果：用户接受 D078–D081。M5-E 分为不改变 workload 的 M5-E.0 只读就绪检查和需
  另行维护授权的 M5-E.1 正式 smoke；当前只授权执行 M5-E.0。
- 固定输入：Qwen-7B 路径、固定 Runner digest、GPU0 完整 UUID、loopback 端口 42000、
  served name `qwen-7b-m5e`、tensor parallel 1、2048 context、0.5 GPU memory、dtype
  auto、trust remote code 和 eager execution。禁止 pull、GPU1、privileged、force remove
  和正式 run 中临时换输入。
- 正式验收序列：M5-E.1 将覆盖 deploy/list/status/logs、`/v1/models` readiness、同 spec
  幂等、异 spec 冲突、completion/chat、stop/remove 与 cleanup，并证明 Server 没有新增
  Lab 资源、Python Worker 恢复和最终零残留。
- 失败策略：只处置可重新证明完整 ownership tuple 的资源；ownership 或 GPU0 残留不可
  归因时失败关闭，不让 Python Worker 与未知 GPU workload 并行，保留 Server 并请求人工
  决策。正常路径使用独立 watchdog 提供有界回滚。
- 当前停止位置：Python Server/Worker 正常运行，Rust Worker 停止；尚未创建 M5-E 配置、
  证据、容器或 GPU workload。
- 下一项允许执行的工作：执行 M5-E.0，只读复核 source/binary/model/Runner/Server/
  service/GPU/port/residue identity，并建立 Git 外 0700/0600 配置与证据。M5-E.1 仍未
  授权。

### 2026-10-22：M5-E.0 只读就绪检查在 chat template 前置条件停止

- 目的：在不停止 Python Worker、不创建或改变容器、不申请 GPU 和不写 Server 资源的
  条件下，确认 M5-E.1 的固定输入、隔离条件、回滚材料与证据链是否完整。
- 结果：状态为 `stopped_precondition`。source/binary、模型 identity、固定 Runner、
  Server 资源、现有服务、GPU、端口、历史残留、回滚脚本和证据安全门禁均通过；但
  Qwen-7B 的 `tokenizer_config.json` 中 `chat_template` 为 `null`，顶层也没有 template
  文件。vLLM 0.25.1 要求 tokenizer 自带或启动时显式提供 chat template，因此当前冻结
  输入不能满足 D080 的 chat 非空响应门禁。
- 输入 identity：源码与 origin 均为
  `d19b784b296b192983180e4f40bf6f556f132916`；locked/offline release 二进制 SHA-256
  为 `3788a39ca59b38be97d0ae976b4f7e8c39b6faa574c2e4947608da2b47c49bfe`，
  `Cargo.lock` SHA-256 为
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`。
  Qwen-7B 顶层文件名/大小 manifest SHA-256 为
  `03f9cb2ad053674b076b5f93ef5389bfb482c01b5de1a603dd2909ba8bc9bc62`；固定
  Runner digest、内嵌 vLLM `v0.25.1`、CUDA `13.0.1` 与 M1/D079 一致。
- 环境与隔离：Rust CLI 和 `nvidia-smi` 均识别两张固定 UUID 的 A100；GPU0 为 0 MiB、
  0% 且无 compute process，GPU1 采样时同样空闲。Server、Python Worker health 与
  metrics 均为 200；两个受保护容器保持 running、restart count 0。Server 资源前后为
  Models 6、ModelInstances 0、ModelFiles 6；端口 42000 空闲，Rust container/process
  均为 0，预留 data/log 路径不存在。
- 回滚准备：Git 外证据中保存 ownership 失败关闭 rollback 与 1200 秒 watchdog；其
  SHA-256 分别为
  `27b9a1a2448c9638153fa24fb0f754e91366151a65b587747f812352b56644d7` 和
  `53648f837ef71a4db9074ff67875051c7f248193d963577571435a7e240013c3`。本阶段没有
  启动 watchdog，也没有执行任何 stop/remove/cleanup。
- 无效与非正式尝试：错误 Cargo 工作目录导致的 stale binary、管道子 shell 丢变量的
  source gate、递归模型 manifest、错误 `/v1/version` 与未处理空 `pgrep`、未加 `/v2`
  的资源路径、把现有 6 个 ModelFile 错判为必须为零、已删除历史 token、交互登录空输入、
  origin 展示污染和审计输出自检查权限均已分类为 `invalidated` 或
  `invalid_non_scoring`，未被提升为通过证据；正式替代文件均保存在同一证据目录。
- 证据：run ID 为 `e0-20261022-125241`；Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5e0-readiness-20261022-125241/`
  为 0700，共 58 个文件且均为 0600，临时登录文件已删除，secret scan 和 manifest
  自校验通过。`manifest.sha256` 自身 SHA-256 为
  `fcc3e8c34fc929319dd4851f787a6a757e9c48eb907a4e202dfd430dcc8032cc`，
  `result.txt` SHA-256 为
  `a6a4291baa9186f1457dbdba6307200c468bc046e9d66f74ddae76ed566a7601`。
- 漂移裁定：D079 未冻结 template，而 D080 要求 chat 成功；继续使用原命令并把 chat
  失败留到维护窗口会降低已冻结验收门禁。结论为 `intentional change pending`，不得
  静默增加参数或自动进入 M5-E.1。
- 当前停止位置：M5-E.0 已完整执行并封存，但未通过进入维护窗口的总体前置门禁。Python
  Server/Worker 保持原状态，Rust Worker 停止，系统无本阶段 workload 或 GPU0 残留。
- 下一项允许执行的工作：向用户报告并讨论是否以模型原生 ChatML 语义冻结一份显式
  template，通过现有 `--vllm-extra-arg` 传给 `--chat-template`。只有修订被接受并完成
  对应只读复核后，才可单独请求 M5-E.1 维护窗口授权。

### 2026-10-22：M5-E.0.1 Qwen ChatML 冻结与传递复核待审阅

- 结果：D082 修订方向已获用户同意，fixture、CLI 解析和 workload argv 回归已实现并
  推送；M5-E.0.1 状态为 `ready_for_review`。本阶段没有进入 M5-E.1，也没有把离线结果
  提升为模型加载或推理证据。
- 模型语义来源：90 主机固定 Qwen-7B 的 `qwen_generation_utils.py::make_context` 明确
  使用 `<|im_start|>/<|im_end|>` ChatML，并在默认 system 为空时仍生成 system 块；该
  文件 SHA-256 为 `f223e0f1bb987fdb93b7cf80e046d96e9e910650103ef2f136eb04887324de34`。
  `tokenizer_config.json` 的 `chat_template` 为 null，文件 SHA-256 为
  `9d4a57f708b43924364068ff3ae5e4c0aa9cf5bdab23d971a0c8f83137b81f52`。
- template identity：新增 `rust/fixtures/m5e/qwen-7b-chatml.jinja`，只覆盖 string
  content、system/user/assistant 与 `add_generation_prompt`。文件为 UTF-8 单行、唯一 LF
  结尾，共 382 bytes，SHA-256 为
  `0d06c62751b46ff03bde0e08ae4d351f056e95885b3c258382492686965af15d`；正式传递时删除
  结尾 LF，有效值为 381 bytes，SHA-256 为
  `74c5628e34e1d05c43a639966d7f5861d01cfd89ebafa39b3976f4d6c12dfd14`。
- 语义复核：90 主机 Jinja2 3.1.2 对单 user、显式 system 加 user、user/assistant 多轮
  三个样例逐字渲染均通过，并与固定 `make_context` 的空 system、逐轮换行和 assistant
  generation prompt 结构一致。vLLM 0.25.1 官方参数契约允许 `--chat-template` 接受单行
  template，并要求 tokenizer 未定义时显式提供。
- 传递复核：正式 Bash 命令只允许把有效值组成一个
  `--vllm-extra-arg=--chat-template=<template>` 参数，禁止 `eval`。现有 CLI 只按第一个
  `=` 拆分为 `--chat-template` 和完整 template；workload builder 再把二者作为两个独立
  container argv 元素传入，因此 Jinja 内容不经过容器 shell 二次解析，也不需要改变
  只读模型 mount。
- 自动化：代码检查点为 `77b402c41ecdcea377d4c4e8dfab962ff30daa6e`；新增 CLI 与
  workload 两项回归测试。固定 Rust 1.99.1 下 fmt、workspace/all-targets/all-features
  locked/offline check、Clippy `-D warnings`、106 项测试、release build 与 Rustdoc
  `-D warnings` 全部通过。没有新增依赖，`Cargo.lock` SHA-256 保持
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`；release binary
  SHA-256 为 `5578b43b774264bc2747367190f4776c09256226319345cb8a9c5e182b7a51f5`。
- 注释与实现边界：本次没有修改生产逻辑，只增加固定 fixture、说明和行为回归测试；
  测试名称已表达不变量，因此没有添加冗余逐行注释或修改历史式注释。
- 结束复核：source、origin 均为代码检查点且工作树干净；Server、Python Worker health
  与 metrics 均为 200，两个既有 GPUStack 容器保持运行；Rust 进程和
  `20150/20151/42000` 监听均为 0，两张 A100 均为 0 MiB、0%。没有连接 Docker 变更
  路径、创建容器、停止服务、挂载模型、申请 GPU 或运行推理。
- 漂移裁定：这是对 D079/D080 的 `intentional change accepted`，只补足固定 chat 输入，
  不改变 Runner、模型、GPU0、端口、资源参数、验收序列和 D081 回滚边界。
- 当前停止位置：M5-E.0.1 等待本聊天中的详细报告与用户共同验收；M5-E.1 仍未授权。
- 下一项允许执行的工作：完成报告与审阅。用户接受后才能单独讨论 M5-E.1 的即时预检、
  维护窗口和精确命令；不得自动停止 Python Worker、创建 vLLM 容器或申请 GPU0。

### 2026-10-22：M5-E.0.1 Qwen ChatML 冻结与传递复核接受

- 结果：用户审阅模板语义、双重 SHA-256 identity、CLI 到 container argv 的逐字传递、
  两项回归测试、106 项 Rust 门禁和服务/GPU 隔离结果后确认无问题；D082 与
  M5-E.0.1 由 `ready_for_review` 更新为 `accepted`。
- 接受证据：代码检查点为 `77b402c41ecdcea377d4c4e8dfab962ff30daa6e`，待审阅治理
  检查点为 `40cc0811cda8184f65e381572e0237cc2a1ba3f2`；fixture、有效模板、模型源文件、
  `Cargo.lock` 和 release binary identity 保持上一里程碑记录。
- 接受范围：只接受固定 Qwen ChatML 内容、identity、单个 CLI 参数和独立 container
  argv 的离线可复现性。它不证明 Runner 能加载 Qwen-7B、chat/completion 能返回内容、
  GPU0 runtime 可用或 Python Worker 回滚已在本阶段执行。
- 漂移审计：D082 是已接受的有意补充；Runner、模型、GPU0、端口、资源参数、Lab CLI
  序列、Server 资源不变和 D081 失败关闭边界均未改变。
- 当前停止位置：Python Server/Worker 正常运行，Rust Worker 停止；没有本阶段容器、
  端口、cache、模型 mount 或 GPU0 残留。M5-E.1 尚未授权。
- 下一项允许执行的工作：共同冻结 M5-E.1 的即时预检、服务切换、Lab 命令序列、证据、
  watchdog 与回滚步骤；实际维护操作仍须新的明确授权。

### 2026-10-22：M5-E.1 尝试 1 在容器创建前因瞬时端口冲突停止

- 目的：按 D078–D082 和用户明确维护授权，在 GPU0 上完成固定 Qwen-7B/Runner 的完整
  Lab CLI smoke，并验证服务切换、失败关闭和恢复证据链。
- 前置门禁：源码与 origin 均为
  `1602f3e38b41cb270dfb68af047572f25f27155c`，工作树已跟踪文件干净；Rust fmt、
  workspace Clippy `-D warnings`、locked/offline 测试和 release build 通过。release
  binary SHA-256 为
  `b99a924311265786028813e5f16ecd1c2db4298defdef083582f5ad0a19f2193`；模型、
  Runner digest、GPU0 UUID、ChatML 双 hash、服务、Server 资源和零残留门禁均通过。
- 维护动作：独立 1200 秒 watchdog 启动后，只停止了受保护的 `gpustack-worker`；Server
  保持 200，Worker health/metrics 按预期关闭，GPU0 保持空闲。Python Worker 容器 ID
  为 `68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165`。
- 停止事实：固定 spec 的 `worker lab deploy` 在约 12 秒后以退出码 50 返回
  `Lab port 127.0.0.1:42000 is already in use`。失败发生在 deployment、cache 和
  container 创建之前；没有模型 mount、GPU request 或推理。事后没有 42000 listener，
  但观测到本机 `192.168.122.1:42000` 到 `192.168.122.162:3389` 的 ESTABLISHED
  连接；稍后的直接 `bind(127.0.0.1:42000)` 又成功，因此不能把该连接认定为直接根因，
  只能确认部署时 Rust 真实收到瞬时端口冲突。
- 裁定：尝试 1 为 `invalidated_non_scoring`，M5-E.1 未通过。原只读预检只检查 LISTEN，
  没有执行真实 bind，且与 deploy 之间存在时间窗；不能因事后端口可绑定而把失败改判为
  成功，也不能静默改端口重试。D079–D082 仍保持不变。
- 回滚与最终隔离：接受的 rollback 执行成功，cleanup 删除数均为 0；Python Worker 以
  原容器 ID 恢复，Server/Worker/metrics 均为 200。Server 资源前后均为 Models 6、
  ModelInstances 0、ModelFiles 6；最终 Rust container/process/42000 listener、GPU0
  compute process、cache 和 deployment 均为 0。watchdog 租约已撤销，临时登录文件已
  逐个删除。
- 证据：run ID `e1-20261022-163941`；Git 外 0700 目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5e1-formal-20261022-163941/`
  共 39 个 0600 文件，known-secret 与 credential-pattern 扫描通过。`result.txt`
  SHA-256 为 `b9bd36f3eece12c58ab41e71b8a9f111e05b008ea95157e6c6b93ac2e53aeeaf`，
  `manifest.sha256` SHA-256 为
  `8c4a2b7422b965fa39920ed019efe96476fae8255dd1a87d9d3bdb88db247793`。
- 当前停止位置：现有 Python 服务完全恢复，系统无本阶段 workload 或 GPU0 残留；正式
  smoke 尚未产生任何模型加载、readiness、幂等、冲突或推理结论。
- 下一项允许执行的工作：讨论是否增加“停止 Worker 前和 deploy 紧邻时均以真实 bind
  验证固定端口”的操作门禁。该修订获接受后，仍须重新获得维护授权才可用同一固定 spec
  重试；当前不得自动重开维护窗口。

### 2026-10-22：D083 bind 双门禁与 M5-E.1 同 spec 重试获授权

- 决策：用户接受 D083。固定端口保持 `127.0.0.1:42000`；停止 Python Worker 前和
  drain 后各执行一次短生命周期真实 bind，第二次成功后不插入其他网络操作并立即 deploy。
- 不变项：模型、Runner digest、GPU0 UUID、served name、context、GPU memory、dtype、
  trust remote code、eager、ChatML、CLI 验收序列、Server 资源对照和 ownership 回滚均不变。
- 授权：用户明确允许用相同 spec 重试，允许再次停止和恢复 `gpustack-worker`，并在全部
  门禁通过后创建、启动、停止和删除唯一 Lab 容器；不得改端口或处置未知 socket。
- 当前停止位置：Server/Python Worker 正常，GPU0 空闲，无 Rust workload；尝试 1 证据
  保持非评分且不可覆盖。
- 下一项允许执行的工作：提交并同步本修订，从该干净 HEAD 重建 binary，建立新的 Git 外
  证据目录和 watchdog，然后严格按 D083 执行重试。

### 2026-10-22：M5-E.1 同 spec 正式重试确认模型与 Runner 不兼容

- 目的：在不改变 D079–D083 任何 workload 输入的前提下，排除尝试 1 的瞬时端口因素，
  真实验证固定 Qwen-7B、Runner、GPU0 与 Lab CLI 启动路径。
- 身份与门禁：从干净治理 HEAD `649e248ad9262435017fe5f95d7620c61b00d8a5`
  强制重建 release binary，SHA-256 为
  `a1c1c3b645120ac4f49ede20b6496697311a2b05dd7c794c9b463fb393756fe2`；停止
  Python Worker 前和 drain 后的 `127.0.0.1:42000` 真实 bind 均通过，第二次 bind 后
  立即执行固定 deploy。
- 实际覆盖：Lab `m5e-qwen7b-gpu0` 创建了 ID
  `eeae6832-cd30-418c-a554-0214122ab96a` 和唯一容器
  `4c41415f02c719c1a2c3256cb1175ed601080befae556b39e7eeb6442f801888`；容器的
  NVIDIA DeviceRequest 只包含固定 GPU0 UUID，spec hash 为
  `45471aaa9f92c4a3111413c1ec42617979737a7b7936b2b78dc70118939b2094`。
- 正式失败：固定 Runner label 证明其镜像为 `vllm/vllm-openai:v0.25.1-ubuntu2404`；
  vLLM 启动校验明确报告 `QWenLMHeadModel` 只支持到 v0.23.0，当前版本已不再支持。
  deploy 以 50 退出，容器以 1 退出，未达到 readiness，未执行 models、幂等/冲突、
  completion 或 chat 验收。该结果是固定输入的真实兼容性失败，不是端口或基础设施失效。
- 回滚缺陷：首次回滚已停止并删除容器，但 Runner 以 root 写入 cache，导致非 root Rust
  CLI 删除 14 个 root-owned 条目时收到 `Permission denied`。原回滚脚本再次执行时又因
  deployment 指向已删除容器而提前退出，证明 cache ownership 和中断后幂等重入都需修复。
- 受控恢复：账号没有 sudo 权限；改用已有 `gpustack/gpustack` 镜像创建一次性
  `--network none`、无 GPU 的恢复容器，只 bind 本次精确 cache 路径并恢复属主，容器随后
  自动删除。精确 `lab remove --yes` 与 cleanup 完成后，以受保护的原容器 ID 恢复 Python
  Worker，并撤销 watchdog 租约。
- 最终不变式：Server、Python Worker health 与 metrics 均为 200；Server 资源前后均为
  Models 6、ModelInstances 0、ModelFiles 6、Workers 2，Python Worker 为 `ready`，Rust
  Worker 保持 `maintenance`。42000 listener、Rust process、Lab manifest/cache/container、
  GPU0 compute process、repair container、watchdog 与临时登录文件均为 0。
- 裁定：本次为 `formal_failure`，M5-E 未通过；尝试 1 仍独立保持
  `invalidated_non_scoring`。不得在同一 run 内换模型、Runner 或端口，也不得自动发起第三次
  GPU 运行。
- 证据：run ID `e1r-20261022-170212`；Git 外 0700 目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5e1-retry-20261022-170212/`
  含 55 个被清单覆盖的 0600 文件和 `SHA256SUMS`；逐文件校验 0 失败，known-secret 与
  credential-pattern 扫描 0 命中。`27-result.txt` SHA-256 为
  `5833258b7f62e4d18479fa7ffb57330f36e10a21d2e3b992e42684dd606522bd`，清单
  SHA-256 为 `1f425b28c887999686f8d0a9b2a57110c735053964d921377e23cc74e86241e2`。
- 下一项允许执行的工作：先对两个清理/回滚缺陷做无 GPU 实现与回归，再共同接受新的
  模型/Runner 兼容性决策；任何新维护窗口均须重新明确授权。

### 2026-10-22：D084 选择 Qwen2.5-0.5B-Instruct 作为新 smoke 模型

- 决策：用户选择保留当前固定 Runner，并把 M5-E 模型改为公开、非 gated 的
  `Qwen/Qwen2.5-0.5B-Instruct`。固定 Hugging Face revision 为
  `7ae557604adf67be50417f59c2c2f167def9a775`，模型架构为 `Qwen2ForCausalLM`。
- 下载 identity：官方 revision 有 10 个顶层文件，总计 999,604,126 bytes；其中
  `model.safetensors` 为 988,097,824 bytes，LFS SHA-256 为
  `fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe`。正式落盘后仍须
  为全部文件生成本地 SHA-256 manifest，不能只验证权重。
- 新 spec：目标路径固定为
  `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct`，Lab name
  为 `m5e-qwen25-05b-gpu0`，served name 为 `qwen2.5-0.5b-m5e`。Runner digest、GPU0
  UUID、`127.0.0.1:42000`、tensor parallel 1、maximum model length 2048、GPU memory
  utilization 0.5、dtype auto 和 eager execution 保持不变。
- 模板与代码信任：使用模型 revision 自带的 `tokenizer_config.json` chat template，不再
  传 D082 的 Qwen-7B ChatML extra arg，也不传 `--trust-remote-code`。D082 fixture 和测试
  作为旧模型输入的历史证据保留，不删除、不改写为 Qwen2.5 语义。
- 传输门禁：为避免 90 主机外网吞吐问题，先在本机 VPN 链路下载精确 revision，再通过
  局域网 SSH 传入 90 的唯一 staging 目录；两端核对文件列表、总字节数和 SHA-256 后才
  原子落到最终路径。下载和传输不授权停止 Python Worker 或运行 GPU workload。
- 当前停止位置：D084 只冻结输入，没有下载模型、修改服务或使用 GPU。
- 下一项允许执行的工作：先实现 Lab cache ownership 清理和回滚重入修复，完成离线及
  无 GPU Docker 回归后建立代码检查点；随后执行模型下载和完整性验证。

### 2026-10-22：D085 cache ownership 与回滚重入修复通过

- 目的：消除正式 M5-E.1 重试暴露的 root-owned cache 和“容器已删、manifest 尚存”时
  rollback 不能重入的两个缺陷，且在再次申请 GPU 维护窗口前完成无 GPU 证明。
- 实现：共享契约升级到 `WorkloadSpecV2`/schema `2`。LabEnvironment 用安全的 Rustix API
  读取 Worker 有效 UID:GID，workload 把它写入规范化 spec、spec hash 与 Docker create
  body 的 `user`；同时强制 `USER=gpustack`、`LOGNAME=gpustack`、deployment cache 下的
  `HOME` 和四个 cache 根。用户环境不能覆盖这些 Worker 控制项。
- 回滚：`lab stop` 改用 optional inspect。资源存在时仍先核验完整 ownership labels；资源
  已缺失或没有 container ID 时不调用 Docker，直接把 manifest 收敛为 `STOPPED`，随后
  `remove` 可清理 cache 与 manifest。新增测试覆盖 `deploy → 外部容器缺失 → status FAILED
  → stop STOPPED → remove`，并断言没有不安全的 remove 调用。
- 自动化：固定 Rust 1.99.1 下 fmt、104 项 workspace locked/offline 测试、workspace
  all-targets Clippy `-D warnings`、Rustdoc `-D warnings` 和 release build 全部通过。代码
  检查点为 `cadbc826a40e8dd8766f9d1f797ed4741e621170`；release binary SHA-256 为
  `108f10af170bc268feadc293a2409c5867525b8c5b49d9908d52b5865df4e309`，`Cargo.lock`
  SHA-256 保持 `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`。
- 真实无 GPU 探针：固定 Runner digest 以 `1004:1004`、network none、无 GPU、无端口、
  无模型 mount 启动；vLLM 0.25.1 导入成功，`USER=gpustack` 和 cache 下 HOME 生效，所有
  新建条目 `foreign_owner_entries=0`。唯一探针容器与临时目录已删除。
- 隔离：探针前后未停止 Python Worker、未申请 GPU、未运行 vLLM server 或模型；最终
  Server、Python Worker health/metrics 均为 200，远程 Git 工作树干净且无探针容器残留。
- 当前停止位置：D085 无 GPU 修复已通过；Qwen2.5 模型尚未下载，M5-E 仍未通过。
- 下一项允许执行的工作：按 D084 在本机下载固定 revision，生成完整 manifest，经局域网
  传输到 90 并复核；任何 GPU workload 或 Python Worker 维护仍须新的明确授权。

### 2026-10-22：Qwen2.5-0.5B-Instruct 固定 revision 已安装并验证

- 下载：本机直连 Hugging Face 在 TLS 阶段被重置，改用已知本机 VPN HTTP proxy
  `127.0.0.1:7897` 后完成固定 revision 下载；下载器支持断点续传，未使用 90 主机低速
  外网链路。解析出的 commit 与 D084 完全一致。
- 本机门禁：模型目录只含 10 个预期普通文件、总计 999,604,126 bytes；完整 SHA-256
  清单保存为 `rust/fixtures/m5e/qwen2.5-0.5b-instruct-7ae557604adf.sha256`。权重 hash 为
  `fdf756fa7fcbe7404d5c60e26bff1a0c8b8aa1f72ced49e7dd0210fe288fb7fe`，与冻结的上游
  LFS identity 一致；配置声明 `Qwen2ForCausalLM`，tokenizer chat template 非空。
- 传输与安装：经 `10.213.80.90` 局域网 SSH 以约 85 MiB/s 传入唯一 staging 目录，
  manifest 独立传输。90 主机复核文件数 10、子目录数 0、总字节数、manifest 自身 hash
  `645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6` 和全部 10 项
  SHA-256 后，才原子重命名为
  `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct`。
- 权限与隔离：最终目录为 `0700`、模型文件为 `0600`，与将运行容器的 `1004:1004`
  身份一致。下载、传输和安装期间未停止 Python Worker、未创建 Lab 容器、未申请 GPU；
  完成后 Server、Python Worker health/metrics 均为 200。
- 当前停止位置：固定模型已就绪，M5-E 仍未通过，也没有新的 GPU 运行结论。
- 下一项允许执行的工作：执行不改变服务或 GPU 的新模型 M5-E.1 只读预检并准备正式
  runbook；停止 Python Worker、创建 vLLM container 和申请 GPU0 仍须新的明确授权。

### 2026-10-22：Qwen2.5 M5-E.1 只读即时预检通过

- 源码与二进制：本地、origin 和 90 均为干净 HEAD
  `56c632472116e50c064ba9c89627a0b3a5dfb60a`；从该 HEAD 强制重建 release binary，内嵌
  commit `56c632472116`，SHA-256 为
  `bfa6ab9b21e99d50144c4b1f13579b5d389dd89c71e27e7ed76eebf8c8a90312`。
  `Cargo.lock` SHA-256 仍为
  `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`。
- 配置与 runtime：`worker validate-config` 返回 `valid=true`，`worker doctor` 返回
  `healthy=true`；唯一 warning 是 log dir 尚未创建，Worker 启动时会创建。Docker 为
  29.1.3、API 1.52，固定 Runner digest 解析为同一 image ID、Linux amd64、entrypoint
  `tini --`。最终模型目录再次逐项通过仓库 fixture 的 10 项 SHA-256。
- 隔离与端口：Server、Python Worker health/metrics 均为 200；Python Worker 原容器
  `68cd9a516eb2` 正常运行。Rust container、Rust process、Lab deployment 和 Lab cache
  均为 0。`127.0.0.1:42000` 无 listener，短生命周期真实 bind 成功。
- GPU：Rust `worker gpu list` 和 `nvidia-smi` 均识别 GPU0
  `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` 与 GPU1
  `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` 为 A100 80GB；两卡当前 used memory 和
  utilization 均为 0，compute process 列表为空。正式 run 仍只允许请求 GPU0 完整 UUID。
- Server 基线：Models 8、ModelInstances 0、ModelFiles 6、Workers 2；Python Worker
  `r760xa=ready`，Rust Worker `rust-m3d-r760xa=maintenance`。后续验收必须以前后相同基线
  为准，不能沿用早期 Models 6 的历史快照。
- 固定 deploy：Lab name `m5e-qwen25-05b-gpu0`，served name
  `qwen2.5-0.5b-m5e`，模型为已验证最终路径；Runner、GPU0、42000、TP=1、max model
  length 2048、GPU memory utilization 0.5、dtype auto、eager 均按 D084。不传
  `--trust-remote-code`，不传 D082 ChatML，也不传任何 `--vllm-extra-arg`。
- 维护与验收序列：继续采用 D081/D083 的独立 watchdog、停止前与 drain 后真实 bind、
  第二次 bind 后立即 deploy，以及完整 ownership rollback。成功路径依次验证
  deploy/list/status/logs、`/v1/models`、同 spec 幂等、异 spec 冲突、completion、chat、
  stop/remove、cleanup dry-run/execute，最后恢复原 Python Worker 并复核 Server 基线与
  container/process/port/cache/GPU0 零残留。
- 当前停止位置：只读门禁通过，但没有停止服务、创建 container、挂载模型、申请 GPU 或
  运行推理。新的正式 run 必须获得用户对本次 Qwen2.5 维护窗口的明确授权。

### 2026-10-22：Qwen2.5 M5-E.1 正式 smoke 待审阅

- 目的与授权：用户明确授权本次维护窗口，只允许停止并恢复受保护的
  `gpustack-worker`，以及在固定 GPU0 UUID 上创建、启动、停止并删除唯一 Qwen2.5 Lab
  容器；GPU1、Runner、模型 revision、42000 端口和资源参数均不得改变。
- 无效预检：首个 Git 外目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5e1-qwen25-formal-20261022-214717-4zQEa2/`
  因运行手册遗漏 `version` 的 `--output json`，在 `preflight_identity` 阶段立即停止。
  Python Worker 从未停止，没有 container 或 GPU request；分类为 `invalidated_preflight`。
  该目录 13 个文件均为 `0600`，清单 SHA-256 为
  `c276f242f7cb787cd6229d5849df9fd5343c08c504506f257156cc1fa42be13e`。
- 正式身份：有效运行使用治理 HEAD
  `81aab8e7aaf10e5caa36a84d6878151d80a8697d`，并证明从二进制代码检查点
  `56c632472116e50c064ba9c89627a0b3a5dfb60a` 到治理 HEAD 没有 Rust 代码漂移。release
  binary SHA-256 为
  `bfa6ab9b21e99d50144c4b1f13579b5d389dd89c71e27e7ed76eebf8c8a90312`，`Cargo.lock`
  SHA-256 为 `9093caad63df8eab5b0a59801e3f92dc0f4f145e1bc066bf9854a3647d049a56`；
  10 项模型 manifest 再次全部通过。
- 维护与启动：独立 1800 秒 watchdog 就绪后，按完整容器 ID
  `68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165` 停止 Python
  Worker；Server 保持健康。停止前和 drain 后两次短生命周期真实 bind 均通过，第二次
  bind 后立即执行固定 deploy。vLLM 在约 75 秒后达到 `/v1/models` readiness。
- 工作负载证据：Lab ID 为 `8f2cd296-41ac-4b6c-9b45-12355bb73b54`，container ID 为
  `96d9e1676ab21723640d3fbc9f5b21013702c4cb3cdb040674c78d3a67a4c6f4`，spec hash 为
  `09cf2988422f6148ac2fe0ad4bfffdee5c15da343e227f0707cac60f5db7dd7c`。捕获的 Docker
  契约证明只请求 GPU0 完整 UUID，容器用户为 `1004:1004`，固定 cache HOME/环境生效，
  模型 mount 只读，host network、10 GiB shm、非 privileged 和
  `no-new-privileges` 生效；argv 不含 `--trust-remote-code` 或旧 ChatML 参数。
- CLI 验收：deploy/list/status/logs 与 `/v1/models` 通过；同 spec 再次 deploy 返回同一
  Lab ID、container ID 和 spec hash。把 GPU memory utilization 改为 0.51 的异 spec
  请求以退出码 50 和 `different normalized spec` 拒绝，运行实例身份未变。completion
  返回非空文本，chat 对固定提示返回 `测试成功`。
- 清理与恢复：`lab stop`、`lab remove --yes`、cleanup dry-run/execute 均通过，证明
  D085 的真实 vLLM cache ownership 正常，不再需要 root 修复。恢复原 Python Worker 后，
  Server、Worker health 和 metrics 均为 200；Models、ModelInstances、ModelFiles、Workers
  前后为 8/0/6/2，前三类资源 ID 集不变，Worker 状态恢复为
  `r760xa=ready`、`rust-m3d-r760xa=maintenance`。
- 最终隔离：独立语法检查后的事后审计确认 Rust container、Lab deployment/cache、42000
  listener、watchdog、GPU0 compute process 和显存使用均为 0。事后审计前两条交互命令
  分别因宽泛进程匹配和 Bash 括号拼写被标记为 `invalidated_diagnostic`；它们只读且没有
  改变正式运行，最终 `post-audit.sh` 完整通过。
- 有效证据：Git 外 `0700` 目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5e1-qwen25-formal-20261022-215207-Mxm3Ak/`
  含 105 个 `0600` 文件；known Worker token 与 credential-pattern 扫描均为 0 命中，
  `result.txt` SHA-256 为
  `679050d4665b61e566ea4801671095a3aa6f016782ff9ae2cc145950adccad5d`，`SHA256SUMS`
  SHA-256 为 `7126ec45f421e01d144030a8561d3e4cad6016df5e839423e37441cd71a77cb5`。
- 解释与状态：该结果证明 M5 的宿主机进程形态、单 GPU Lab、固定 Runner/Qwen2.5、CLI
  推理、幂等、冲突保护、所有权清理和 Python Worker 回滚路径，状态为
  `ready_for_review`。它仍不是 Managed、双 GPU 或容器化 Rust Worker 证据。
- 当前停止位置与下一步：系统已回到维护前状态；先在聊天中完成详细报告和用户审阅。
  用户接受后才记录 M5-E/M5 `accepted` 并讨论 M6-A，不自动执行 Managed 写入或新维护窗口。

### 2026-10-22：Qwen2.5 M5-E.1 与整个 M5 接受

- 验收：用户在收到正式 smoke 的功能、实现、运行证据、失败分类、清理和恢复报告后明确
  接受结果。M5-E.1、M5-E 与整个 M5 状态由 `ready_for_review` 更新为 `accepted`。
- 接受范围：宿主机进程形态的 Rust CLI 已证明单 GPU Lab 能以固定 Runner 和固定
  Qwen2.5 revision 完成部署、查询、日志、OpenAI-compatible completion/chat、同 spec
  幂等、异 spec 冲突保护、停止、删除与 cleanup；D085 的非 root cache ownership 和正常
  清理路径也已由真实 vLLM workload 验证。
- 证据身份：有效证据目录为
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m5e1-qwen25-formal-20261022-215207-Mxm3Ak/`；
  `result.txt` SHA-256 为
  `679050d4665b61e566ea4801671095a3aa6f016782ff9ae2cc145950adccad5d`，`SHA256SUMS`
  SHA-256 为 `7126ec45f421e01d144030a8561d3e4cad6016df5e839423e37441cd71a77cb5`。
  待审结果检查点为 `bf86ded9a8560104b36015df727c4a5a9136e7e2`。
- 分类保留：遗漏 JSON 输出的首次预检继续为 `invalidated_preflight`；两条只读事后审计
  交互命令继续为 `invalidated_diagnostic`；Qwen-7B 的端口无效尝试和 Runner 不兼容
  `formal_failure` 也不因本次成功而改写。
- 非接受范围：本次不证明 Managed Model/ModelInstance/ModelFile 控制链、跨 Worker 或
  双 GPU 分布式推理、Rust Worker 容器化、故障中断后的 GPU 回滚重入或生产发布门禁。
- 文档校验：待审检查点的 `git diff --check` 通过；仓库级 `make lint` 因 90 主机没有
  `uv`、`pre-commit` 或 Python `pre_commit` 模块而无法启动。该检查点只改治理文档，
  没有生产代码变化。
- 当前停止位置：Python Server/Worker 健康并以原身份运行，Rust Worker 停止，正式 Lab、
  container、port、cache、watchdog 和 GPU0 计算进程均无残留。
- 下一项允许执行的工作：仅讨论并冻结 M6-A Managed 本地模型文件准备、状态所有权与
  验收设计。M6-A 设计和实现门禁获接受且真实写入另获授权前，不执行 Server Managed
  资源写入或新的 GPU 维护窗口。

### 2026-10-23：M6-A Managed 文件准备设计接受

- 目的：在 M5 Lab runtime 已接受后，把第一次 Managed 实现限制为 Server 状态所有权、
  Worker 端口分配和本地 ModelFile 准备，不提前引入 Docker/vLLM/GPU runtime。
- 源码纠正：固定 v2.2.3 commit
  `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` 证明服务端口由 Worker 的
  `_assign_ports` 在写 `INITIALIZING` 前从 `40000-40063` 分配，而不是 Server 预先分配；
  旧文档表述已按代码优先原则纠正。
- 决策：用户接受 D086–D091。M6-A 停在 Server 权威 `STARTING`，`--managed-file-only`
  不构造 Docker client；本地 resolver 强制根目录、canonical symlink、完整分片、大小和
  ready/error 结果；每次完整 PUT 前按 ID 刷新并复核归属。
- CLI：Context、`model deploy/list/get/status/delete` 前移到 M6-A，新增
  `worker maintenance enable/disable`；logs 与 inference 留在 M6-B。真实流程不能依赖
  Web UI 或未文档化原始请求。
- 分段：M6-A.1 为离线 Worker 实现，M6-A.2 为离线 CLI 实现，M6-A.3 才是另获授权的
  真实 Server 无 Docker smoke。M6-A.3 将精确调度固定 Qwen2.5 到 Rust GPU0，但不得
  创建 container、申请 Docker GPU DeviceRequest 或运行推理，并须恢复原 Python Worker。
- 当前停止位置：只完成并接受设计，代码、Server、maintenance、容器、服务和 GPU 均未
  改变；本地既有 `output/` 继续不属于项目变更。
- 下一项允许执行的工作：只实施 M6-A.1 离线代码、fixture 和 fake/mock 测试，完成后
  在聊天中交付 D033 详细实现报告并等待验收。M6-A.2 和 M6-A.3 不自动开始。

### 2026-10-23：M6-A.1 离线 Worker 实现完成

- 状态：`ready_for_review`，不是 `accepted`。
- 实现：`gpustack-client` 增加单个 ModelInstance GET；Worker 新增 Managed 端口分配、
  允许根目录内的本地路径/完整分片解析、递归大小计算，以及由 desired-state snapshot
  触发、按资源 ID 去重的协调器。每次完整 PUT 前重新 GET 并复核 assignment、状态与
  source 关键字段；`STARTING` 和 READY 资源只产生 no-op。
- 安全边界：resolver canonicalize 输入和目录中的 symlink，拒绝根目录逃逸、相对路径、
  缺失或总数不一致的分片；错误消息不含原始路径且限制为 512 字符。端口分配排除同
  Worker 权威实例占用并通过可替换 bind probe 检查未知监听，不处置监听进程。
- 离线证据：固定 Rust 1.99.1、`Cargo.lock` 和 `--offline` 下，Client 契约测试 13 项、
  Managed 定向测试 17 项、全 workspace `--all-targets` 124 项及全部 doc-test 通过；
  `cargo fmt --check`、`cargo check --workspace --all-targets` 和
  `cargo clippy --workspace --all-targets -- -D warnings` 通过。90 主机没有 `uv`，因此依赖
  `uv run pre-commit` 的仓库级 `make lint` 未执行，未把缺失工具记为成功证据。
- 非执行项：没有读取真实 token，没有发出 Server 请求，没有连接 Docker socket/NVML，
  没有启动 Worker、容器或 vLLM，没有修改 maintenance、Python Worker、端口监听或 GPU。
- 当前停止位置：实现和离线门禁完成，等待聊天中的 D033 详细报告与用户验收。
- 下一项允许执行的工作：只交付并审阅 M6-A.1 报告；接受后才能记录验收检查点并开始
  M6-A.2 离线 CLI，实现之外的真实 Server/维护窗口仍须到 M6-A.3 单独授权。

### 2026-10-23：M6-A.1 离线 Worker 实现接受

- 决策：用户审阅详细实现报告后明确接受 M6-A.1；状态从 `ready_for_review` 变为
  `accepted`。
- 证据：实现提交 `976e80617bb9919949d276a4034970ea07a357e4`；Client 契约测试
  13 项、Managed 定向测试 17 项、全 workspace 124 项、doc-test、fmt、check 和
  `clippy -D warnings` 均通过。90 主机缺少 `uv`，仓库级 `make lint` 未执行的限制继续
  保留，不被本次接受改写。
- 接受范围：Managed 端口选择、本地 ModelFile 安全解析、完整写前刷新、按资源 ID 去重、
  assignment 漂移保护、ready/error 构造和 `STARTING` no-op 的离线实现与 fake/mock 证据。
- 明确非结论：没有真实 Server 请求，没有正式 `worker run --managed-file-only` 接线，
  没有 maintenance 变化、Docker/vLLM 容器、推理或 GPU 证据；M6-A.3 仍未授权。
- 当前停止位置：M6-A.1 已接受，源码、GitHub 与 90 主机均以实现提交为共同基线。
- 下一项允许执行的工作：只实施 M6-A.2 离线 Context、最小 Model CRUD/status 和 Worker
  maintenance CLI，完成后交付新的详细实现报告；M6-A.3 真实验证仍须另行明确授权。

### 2026-10-23：M6-A.2 离线 CLI 实现完成

- 状态：`ready_for_review`，不是 `accepted`。
- 实现：增加 `context add/use/list/remove`、`model deploy/list/get/status/delete` 和
  `worker maintenance enable/disable`。Context 只保存 Server URL 与 token 文件引用；
  Model 命令只调用类型化 Server client，不接触 Docker；maintenance 完整 PUT 会保留
  Server 返回的 Worker name 和 labels，并且 disable 与 delete/remove 均有确认门禁。
- 输出与等待：table/JSON 输出稳定并转义终端控制字符；`model deploy --wait` 在 stderr
  逐次报告观察到的 ModelInstance 状态转换，最终 JSON 不混入进度文本。配置错误、控制面
  错误和内部序列化错误继续分别使用退出码 10、30 和 1。
- 凭据边界：Context store 采用版本化 JSON、`0700` 父目录、`0600` 原子替换文件；token
  读取拒绝 symlink、组/其他用户可读模式、超限文件、非 UTF-8、首尾空白与控制字符。
  token 值不进入 Context、table/JSON、Debug、错误信息或请求调试输出。
- 离线证据：实现提交
  `aff61fe2c7e98c50d3b5d273f6fbc582877a633f`；固定 Rust 1.99.1 与 `--locked --offline`
  下，全 workspace `--all-targets` 140 项测试、全部 doc-test、fmt、check、Clippy
  `-D warnings` 和 build 通过。`Cargo.lock` SHA-256 为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`；从干净实现提交
  重建的 `gpustack-rs` SHA-256 为
  `a6dfb727935e4b33c13795a845670977104d701850fa017b9cbc702cf4cc0106`，内嵌 commit
  `aff61fe2c7e9`。90 主机仍缺少 `uv`，所以依赖它的仓库级 `make lint` 未执行，未把缺失
  工具记作成功证据。
- 非执行项：没有读取真实运维 token，没有连接真实 Server、Docker socket 或 NVML，
  没有启动 Rust Worker、创建/删除 Model 或容器、改变真实 maintenance、停止 Python
  Worker、申请 GPU 或运行推理。M6-A.3 仍未授权。
- 当前停止位置：实现、离线门禁、Git 提交和 fork 推送完成，等待聊天中的 D033 详细报告
  与用户验收。
- 下一项允许执行的工作：只交付并审阅 M6-A.2 报告；用户接受后才能记录接受检查点并
  讨论 M6-A.3 的独立只读预检与真实无 Docker smoke，不能自动执行。

### 2026-10-23：M6-A.2 离线 CLI 实现接受

- 结果与状态：用户审阅 D033 详细实现报告后确认无问题；M6-A.2 由
  `ready_for_review` 更新为 `accepted`。
- 接受证据：实现提交为 `aff61fe2c7e98c50d3b5d273f6fbc582877a633f`，待审治理检查点
  为 `6a55c8d98ea5a6d0bfdd9dea35e8fa12bef6c329`。固定 Rust 1.99.1 下 140 项 workspace
  测试、全部 doc-test、fmt、locked/offline check、Clippy `-D warnings` 和 build 均通过；
  `Cargo.lock` 与干净实现提交二进制 SHA-256 分别为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b` 和
  `a6dfb727935e4b33c13795a845670977104d701850fa017b9cbc702cf4cc0106`。
- 接受范围：版本化且受权限保护的 Context store、类型化最小 Model CRUD/status、保留
  Worker name/labels 的完整 maintenance PUT、危险操作确认、稳定 table/JSON 和退出码、
  `model deploy --wait` 状态转换，以及 mock transport 契约测试。
- 明确非结论：本次接受不证明真实凭据、真实 Server 写入、真实 maintenance、Managed
  Worker 联调、Python Worker drain、Docker/vLLM、推理或 GPU 能力；M6-A.3 未获执行授权。
- 当前停止位置：M6-A.2 已接受，代码、治理文档、fork 与 90 主机工作树保持同一 Git
  基线；运行中的 Python Server/Worker 和 GPU 环境没有因本阶段发生变化。
- 下一项允许执行的工作：只讨论并冻结 M6-A.3 的独立预检、真实无 Docker smoke、维护
  窗口和回滚 runbook；任何真实环境写操作仍须用户另行明确授权。

### 2026-10-24：M6-A.2.1 离线生产入口接线修订接受

- 发现：M6-A.1 已实现 `ManagedCoordinator`，M6-A.2 已实现控制面 CLI，但正式
  `WorkerRunArgs` 只有 control-plane-only 和 resource-api-only；文档中的
  `worker run --managed-file-only` 尚无生产调用方，不能直接作为 M6-A.3 的验证入口。
- 决策：用户接受 D092–D094，在真实 smoke 前增设 M6-A.2.1。该阶段增加互斥 CLI mode，
  复用持久 Worker identity、system/NVML、Worker API、metrics 和 control plane，接入
  Managed coordinator，并以结构化脱敏事件记录状态观察和协调结果。
- 安全边界：启动时要求 Worker 已处于 maintenance，但 Rust 进程不得自动修改它；不得
  读取真实 token、连接真实 Server、停止 Python Worker、访问 Docker socket、构造
  WorkloadSpec、创建容器、申请 GPU 或运行推理。全部门禁只使用 fixture/fake/mock。
- 状态：修订已冻结，M6-A.2.1 为 `running`；M6-A.3 仍未授权。
- 下一项允许执行的工作：只完成 M6-A.2.1 源码、离线测试、质量门禁和 D033 详细实现
  报告。报告获用户接受前，不进入真实预检或维护窗口。

### 2026-10-24：M6-A.2.1 离线生产入口接线完成

- 结果与状态：M6-A.2.1 为 `ready_for_review`。实现提交
  `e1e3ee4a662099c1acfc60a8cfae0c8129b7d762` 把正式
  `worker run --managed-file-only` 接到共享资源 supervisor 和 M6-A.1
  `ManagedCoordinator`；`dc53b9aedd6465a0564ecc07aeda03b1cf0c70fb` 补齐三种受限
  Worker mode 的 CLI help。两项均已推送 fork。
- CLI 与生命周期：三种 mode 两两互斥且必须显式选择。managed-file-only 复用持久身份、
  system/NVML 资源、只读 Worker API、metrics、heartbeat/status 与三资源 full-sync/watch；
  启动先验证 Server Worker maintenance 已启用，再把 control-plane desired-state revision
  交给 coordinator。进程本身不修改 maintenance。
- 协调与观察：协调结果携带 watch 快照中的观察状态。结构化事件记录资源类型、数字 ID、
  观察状态、端口分配/复用、ModelFile 路径数量与字节数、有界错误或 no-op 理由；因此
  `STARTING` 会明确记录为 no-op。优雅退出摘要保存最终 desired 集合/版本、各结果计数和
  control-plane health，不记录 token 或模型路径。
- 安全边界：model roots 在创建网络 client 前 canonicalize 并失败关闭；启动要求持久
  per-Worker identity 和 maintenance。生产 resource supervisor 源码静态审计没有
  `gpustack_runtime`、Docker client、`WorkloadSpec` 或 `docker_socket` 引用，且本阶段没有
  新依赖，`Cargo.lock` SHA-256 仍为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`。
- 离线验证：固定 Rust 1.99.1 下 `cargo fmt --check`、workspace all-targets locked/offline
  check、workspace locked/offline Clippy `-D warnings`、test、doc-test 和 build 全部通过；
  workspace 共 142 项测试。新增覆盖 mode 互斥、maintenance profile、观察状态随 revision
  进入协调器、`STARTING` no-op 事件、结果计数和旧 resource metrics mode 标签兼容。
- 构建证据：从干净提交 `dc53b9ae` 重建的 debug CLI 报告
  `git_commit=dc53b9aedd64`、`rust_toolchain=1.99.1`，SHA-256 为
  `9009d9486f39e6aca9f648aae5c92a11c7b08c70c500d3a09bd5492ecde9e619`；帮助输出列出并解释
  `--control-plane-only`、`--resource-api-only` 和 `--managed-file-only`。
- 外部副作用：没有启动 Rust Worker、读取真实 token、连接真实 Server API、改变
  maintenance、停止 Python Worker、访问 Docker socket、创建容器、申请 GPU 或运行推理。
- 漂移审计：目标、固定 Server v2.2.3 契约、R1 单 Worker/单 GPU 范围、D086
  `STARTING` 边界、D087–D090 状态/端口/文件/CLI 所有权和 M6-A.3 单独授权均未改变；
  本次只修复阶段分解与生产入口缺口，结论为 `aligned`。
- 当前停止位置：实现、离线证据、代码提交和 fork 推送完成，等待聊天中的 D033 详细报告
  与用户验收；这不证明真实 Server 状态链、真实 maintenance 或部署能力。
- 下一项允许执行的工作：只交付并审阅 M6-A.2.1 报告。用户接受后才能记录验收检查点
  并讨论 M6-A.3；真实预检或维护窗口不自动开始。

### 2026-10-24：M6-A.2.1 离线生产入口接线接受

- 结果与状态：用户审阅 D033 详细实现报告后确认无问题；M6-A.2.1 从
  `ready_for_review` 更新为 `accepted`。
- 接受证据：实现提交为 `e1e3ee4a662099c1acfc60a8cfae0c8129b7d762`，CLI 帮助补充
  提交为 `dc53b9aedd6465a0564ecc07aeda03b1cf0c70fb`，待审治理检查点为
  `9b923f5838c4126152a9edec30a2a75192839195`。固定 Rust 1.99.1 下 142 项 workspace
  测试、doc-test、fmt、locked/offline all-targets check、Clippy `-D warnings` 和 build
  全部通过；干净二进制 SHA-256 为
  `9009d9486f39e6aca9f648aae5c92a11c7b08c70c500d3a09bd5492ecde9e619`。
- 接受范围：显式且互斥的 `--managed-file-only` CLI、共享资源 supervisor、持久身份和
  maintenance 启动门禁、control-plane desired-state 到 `ManagedCoordinator` 的生产接线、
  结构化脱敏事件、退出摘要和旧 resource-only 行为兼容。
- 明确非结论：本次接受不证明真实 Worker token、真实 Server 状态写入、maintenance
  切换、Python Worker drain、Docker/vLLM、推理或 GPU 能力；M6-A.3 尚未获执行授权。
- 漂移审计：接受结论只使用冻结离线证据，没有把未执行的真实 smoke 提升为通过；目标、
  D086 `STARTING` 边界和 M6-A.3 单独授权继续有效，结论为 `aligned`。
- 当前停止位置：M6-A.2.1 已接受，代码、治理文档、fork 与 90 主机工作树保持同一 Git
  基线；本次验收检查点不读取或改变远程运行服务状态。
- 下一项允许执行的工作：只讨论并冻结 M6-A.3 的独立只读预检、真实无 Docker smoke、
  维护窗口、证据和回滚 runbook；任何真实环境操作仍须用户另行明确授权。

## 未解决风险与待确认事项

- 固定 Runner v0.25.1 与 Qwen-7B 的不兼容失败继续保留；D084 选择的
  `Qwen2.5-0.5B-Instruct` 已完成固定 revision 下载、全文件完整性验证和真实 vLLM smoke。
- Lab cache ownership 与回滚重入已由 D085 修复，并在无 GPU Runner 探针和本次真实
  vLLM 正常 stop/remove 中通过；故障中断后的真实 GPU 回滚重入仍留给 M7 恢复阶段覆盖。
- Rust Worker API 与 metrics 已分别冻结为 `20150` 和 `20151`；Lab 自动 inference
  端口为 `42000-42063`，Managed 由 Worker 从 `40000-40063` 分配并写回 Server。分配时
  和启动容器前都须检查实际端口冲突。
- M4-D 已真实执行 `gpustack-worker` stop/start、失败 trap、独立 watchdog 和正常恢复；该证据只证明 M4 资源/API 维护切换，不替代后续 Docker/vLLM 工作负载阶段各自的 drain 与回滚门禁。
- Rust 容器最终 user/group 仍需在 M8 根据 Docker socket、NVML 与持久化目录要求验证；宿主机开发用户已确认具备 `docker` group 权限。
- 90 主机到 GitHub 的小规模 fetch/push 可用，但大对象传输吞吐过低；bootstrap 和发布 tag 增量 bundle 是当前已验证的补充路径。
- 90 主机 `127.0.0.1:17897` 与 `[::1]:17897` 曾存在未归因监听，M3-C 完成核验时已无监听；本项目没有停止它。未来复用或处置同端口监听前仍须确认所有者，自动化 SSH 命令继续显式使用 `ClearAllForwardings=yes`，避免自动创建额外转发。
