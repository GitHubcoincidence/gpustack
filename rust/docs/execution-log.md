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
- 2026-10-24：M6-A.2.2 ModelFile 清理 CLI 已接受；随后冻结 D098–D102，把 M6-A.3
  拆为须分别授权的 M6-A.3.0 只读预检和 M6-A.3.1 真实无 Docker 维护窗口，并固定
  `STARTING` 终点、零 runtime、精确清理、Python Worker 恢复与证据边界。

## 当前状态

- 状态：`accepted`
- 当前阶段：M1～M5、M6-A 和 M6-B.1～M6-B.4 已完成既定验收。M6-B.5.2 formal
  attempt 7 已通过冻结的完整状态链、Server-only 推理与日志、同进程三轮幂等、精确清理、
  watchdog 和独立事后审计。用户已接受详细报告，阶段专用凭据也已退役，M6-B.5 正式完成；
  该接受不自动授权执行 M7。
- 最新产品检查点：`d4920e71b89d6507f8e82fb2aff7dfbb24490103`。Managed 推理代理现使用
  已验证的 `worker_ip + assigned_port`，与 runtime 健康检查及 Python Worker代理语义一致；
  复合 destination、认证、内部路由表和敏感头剥离边界不变。本地、GitHub 与 90 已同步。
- 检查点前已存在且与本项目无关的工作树状态：未跟踪目录 `output/`；本项目不修改它。
- 90 主机身份：`wujiaxing2@r760xa`；仓库路径：
  `/home/wujiaxing2/workspaces/gpustack`；账号级 SSH 已认证 fork。
- attempt 7 正式、watchdog 和独立事后审计清单文件 SHA-256 分别为
  `c81af217bf80a4ca568de8c45e5a602c624fe87507967c6ce1f6f4dbe9fc9eb3`、
  `cabb29165c85b5a8b68cbe1023b8863ab2d12bc1d4385eed1eb271899c947fa7` 和
  `8d60613d2086ec708261368a126079f896bd49f96bba067ffac931ee3a6482da`。三套清单共
  87/5/22 个条目全部通过，秘密扫描无发现；最终现场复核再次证明 Python Worker、maintenance、
  Server 资源、Docker、GPU、端口、模型文件和仓库状态恢复基线。
- formal attempt 7 使用 Model 20、Instance 35、ModelFile 14、端口 40000 和唯一 owned
  container `c63e726ffb484825f6a25fa731f610f2b3d1b225fcd08904e68e4d9e0930c106`；全部已按
  精确 identity 删除。用户接受报告后，阶段专用 API Key ID 21、`m6b5-operator` Context 和
  operator token 文件已按 D161 退役，旧 token 返回 401；Cluster registration token 保持不变。
  退役证据含 15 个清单条目，清单文件 SHA-256 为
  `87e34cef088ee9506a8ced0157d2bd1eadb5f55f99292d5469089c473a04e6a0`。

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

### 2026-10-24：M6-A.2.2 ModelFile 清理 CLI 修订冻结

- 发现：源码核对确认 Model 删除会级联删除 ModelInstance，但独立 ModelFile 缓存记录不会
  自动删除；仅使用现有 `model delete` 无法证明 M6-A.3 测试资源零残留。
- 决策：用户接受 D095–D097，在真实 smoke 前增设 M6-A.2.2，增加
  `model file get <id>` 与 `model file delete <id> --yes`。删除只针对正整数 ID，先读取最新
  对象并限制为有 Worker 归属和非空本地路径的 `local_path` ModelFile，随后固定以
  `cleanup=false` 删除 Server 元数据；本地 Qwen2.5 文件必须保留。
- 状态：M6-A.2.2 为 `running`。本机、fork 和 90 主机在开始时共同位于
  `3f7dbb2c159548bf73c1c3a771dd7969d2d02bb4`；90 工作树干净，本地只有既有且无关的
  `output/`。
- 漂移审计：该修订补全 D091 已冻结的测试资源清理闭环，不改变 D086 `STARTING` 边界、
  固定模型/Worker/GPU/Runner 或真实 smoke 的解释，结论为 `aligned`。
- 外部副作用：本次修订只授权文档、Rust 源码和离线测试；不读取真实 token、不连接真实
  Server、不改变 maintenance、不停止 Python Worker，也不访问 Docker 或 GPU。
- 当前停止位置：设计已冻结，开始实现和离线验证；尚无实现或真实环境结论。
- 下一项允许执行的工作：只完成 M6-A.2.2 实现、mock/CLI 测试、固定 Rust 质量门禁、提交
  与详细报告。报告获用户接受前，不进入 M6-A.3 预检或维护窗口。

### 2026-10-24：M6-A.2.2 ModelFile 清理 CLI 实现完成

- 结果与状态：M6-A.2.2 为 `ready_for_review`，不是 `accepted`。设计检查点提交为
  `3fd938abb34e043b7ef31002b68d9429534d1e64`，实现提交为
  `f19a8fcd41ed8368931509ae3120a6b0d7098043`，均已推送 fork。
- 实现：在类型化 client 中增加固定 `cleanup=false` 的 ModelFile 元数据删除请求；CLI 增加
  `model file get <id>` 和 `model file delete <id> --yes`。删除只接受正整数 ID，确认后重新
  GET 最新对象，限制为有 Worker 归属和非空本地路径的 `local_path` ModelFile，再删除
  Server 记录；输出明确报告 `local_files_preserved=true`。
- 安全与测试：确认缺失和非法 ID 均在网络请求前失败；非本地来源只允许 GET 校验且不得
  发出 DELETE；正式删除请求精确验证为
  `/v2/model-files/{id}?cleanup=false`；mock 请求和输出均不包含 operator token。
- 质量证据：固定 Rust 1.99.1 下 144 项 workspace 测试、全部 doc-test、`cargo fmt`
  check、locked/offline all-targets check、Clippy `-D warnings` 和 locked/offline build 全部
  通过。`make lint` 已执行但 90 主机缺少 `uv`，在 pre-commit 启动前以 status 127 退出；
  该环境限制不被记为通过，也没有触发新的依赖安装。
- 构建证据：从干净提交 `f19a8fcd` 重建的 debug CLI 报告
  `git_commit=f19a8fcd41ed`、`rust_toolchain=1.99.1`，SHA-256 为
  `4e601e9675852515652dc55bde272a906958f143db30b987a95f0ef7754e7a82`；Cargo.lock SHA-256
  保持 `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`。
- 外部副作用：没有读取真实 token、连接真实 Server、创建或删除真实 ModelFile、改变
  maintenance、停止 Python Worker、访问 Docker socket、创建容器或使用 GPU。
- 漂移审计：实现与 D095–D097 对齐，只补全 D091 的清理能力；D086 `STARTING` 边界和
  M6-A.3 单独授权继续有效，结论为 `aligned`。
- 当前停止位置：实现、离线证据、代码提交和 fork 推送完成，等待聊天中的详细实现报告与
  用户验收。
- 下一项允许执行的工作：只交付并审阅 M6-A.2.2 报告。接受后才能记录验收检查点并讨论
  M6-A.3.0 只读预检；真实 Server 写入和维护窗口仍未授权。

### 2026-10-24：M6-A.2.2 ModelFile 清理 CLI 接受

- 结果与状态：用户审阅详细实现报告后确认无问题；M6-A.2.2 从 `ready_for_review` 更新为
  `accepted`。
- 接受证据：设计检查点为 `3fd938abb34e043b7ef31002b68d9429534d1e64`，实现提交为
  `f19a8fcd41ed8368931509ae3120a6b0d7098043`，待审治理检查点为
  `64b56abaa6f9cd39c8e2b8790f4010256108024e`。固定 Rust 1.99.1 下 144 项 workspace
  测试、doc-test、fmt、locked/offline all-targets check、Clippy `-D warnings`、build 和
  干净二进制身份继续作为接受依据。
- 接受范围：类型化 `model file get/delete`、正整数与显式确认门禁、删除前最新 GET、
  `local_path`/Worker/路径安全检查、Server 引用冲突和固定 `cleanup=false` 元数据删除语义。
  宿主机模型文件删除不在该命令能力范围内。
- 已知限制：仓库级 `make lint` 因 90 主机缺少 `uv` 未能启动 pre-commit；该限制继续保留，
  不因用户接受而改写为通过。
- 明确非结论：本次接受不证明真实 operator token、真实 ModelFile 查询或删除、真实 Server
  状态链、maintenance 切换、Python Worker drain、Docker/vLLM、推理或 GPU 能力。
- 漂移审计：接受只使用已冻结的离线证据，没有把未执行的 M6-A.3 提升为通过；D086
  `STARTING` 边界、D091 清理要求和真实环境分段授权均未改变，结论为 `aligned`。
- 当前停止位置：M6-A.2.2 已接受；本次验收只核对 Git 基线，没有重新探测或改变 90 主机
  上的 Server、Worker、Docker、端口或 GPU 运行状态。
- 下一项允许执行的工作：只讨论并冻结 M6-A.3.0 只读预检和 M6-A.3.1 维护窗口 runbook；
  M6-A.3.0 的实际远程检查与 M6-A.3.1 的任何真实写操作仍须分别明确授权。

### 2026-10-24：M6-A.3 真实文件准备验证手册冻结

- 目的：在第一次真实 Managed 文件准备写入前，固定可证伪主张、输入、两级授权、状态
  证据、停止条件、清理和 Python Worker 恢复顺序。
- 决策与状态：用户接受 D098–D102。M6-A.3.0 为独立只读预检，M6-A.3.1 为预检通过并
  共同审阅后另获授权的维护窗口；手册状态为 `frozen`。静态核对同时发现当前 CLI 缺少
  预检需要的只读 Server Worker 查询和按 Worker 的 ModelFile 列表，因此 M6-A.3.0 为
  `not_ready`，M6-A.3.1 为 `not_started`。
- 固定输入：Server v2.2.3/`1cfcbbff`、Rust Worker `rust-m3d-r760xa`、固定本地
  Qwen2.5 revision/path、Model name `m6a3-qwen25-05b-gpu0`、固定 Runner digest、GPU0、
  TP=1、2048 context、0.5 GPU memory、dtype auto 和 eager。正式部署禁止 `--wait`。
- 接受边界：必须以连续证据证明 `SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING`
  和 ModelFile `downloading -> ready`，随后在 `STARTING` 观察 60 秒；不得出现 Docker
  client/container、vLLM、inference listener、GPU compute process、可归因显存增长或
  `RUNNING`。
- 隔离与回滚：正式维护窗口仍停止并恢复精确 Python Worker，以避免同一物理 GPU 的两个
  Worker reporter 削弱结果归因。清理先恢复 Rust maintenance，再删除精确 Model/
  ModelInstance 和 `cleanup=false` 的 ModelFile 元数据，核对本地模型不变，停止精确 Rust
  PID，最后恢复原 Python Worker 并验证资源 ID、容器、端口和 GPU 零残留。
- 证据：完整协议见 `rust/docs/m6a3-managed-file-runbook.md`；原始运行证据未来写入 Git 外
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal`，目录在预检时
  必须尚不存在。文档冻结本身没有生成任何运行证据。
- 漂移审计：方案保留 D086 `STARTING` 边界、D087 Worker 端口责任、D088 本地路径规则、
  D089 写前刷新、D090 CLI-first 和 D091 独立授权；没有把 M5 Lab 或 M6 离线 artifact
  提升为 Managed 证据，结论为 `aligned`。
- 新发现前置条件：当前 CLI 只有 `worker maintenance enable|disable` 与
  `model file get/delete`。若直接执行预检，maintenance/身份和同路径 ModelFile 冲突只能
  借助 Web UI 或未文档化原始请求，违反 D090。候选补充接口为
  `worker get <worker>` 和 `model file list --worker <worker>`；接口及新增门禁尚未获得用户
  接受或实现，不能默认为授权。
- 外部副作用：本次只修改治理文档，没有连接真实 Server、读取 token、启动 Rust Worker、
  改变 maintenance、停止 Python Worker、访问 Docker 或使用 GPU。
- 当前停止位置：M6-A.3 方法已冻结，但 CLI 前置缺口未关闭，M6-A.3.0 尚未执行。
- 下一项允许执行的工作：只讨论并冻结最小只读 CLI 前置门禁。该门禁实现、离线验证、
  D033 详细报告和用户接受完成后，才能请求 M6-A.3.0 的单独执行授权。

### 2026-10-24：M6-A.2.3 只读预检 CLI 设计冻结

- 决策与状态：用户接受 D103–D106，将 `worker get <worker-id-or-exact-name>` 和
  `model file list --worker <worker-id-or-exact-name>` 冻结为 M6-A.2.3；阶段进入离线实现，
  尚未达到 `ready_for_review`。
- Worker 契约：数字 selector 按 ID，其他值按精确名称解析；无结果或重名失败关闭。输出
  采用安全白名单，不透传 extensions、provider config、principal ID 或未知字段，也不伪造
  固定 v2.2.3 public schema 没有的 `worker_uuid`。
- ModelFile 契约：强制 `--worker` scope，先解析唯一 Worker，再请求其 ModelFile，按 ID
  稳定排序；JSON 与现有 get schema 一致，table 使用固定摘要列，空列表成功。
- 实现与证据边界：不新增依赖，不实现 ModelInstance list；只允许源码、fixture/mock、
  文档和离线质量门禁。不得读取真实 token、连接真实 Server、改变 maintenance、停止
  Python Worker、访问 Docker 或使用 GPU。
- 漂移审计：新增门禁只关闭 D090 CLI-first 前置缺口，D086 `STARTING` 边界以及
  D098–D102 的预检/维护窗口授权保持不变，结论为 `aligned`。
- 下一项允许执行的工作：完成 M6-A.2.3 源码与离线测试，提交 D033 详细报告。即使该阶段
  获接受，M6-A.3.0 仍须另行明确执行授权。

### 2026-10-24：M6-A.2.3 只读预检 CLI 实现完成

- 结果与状态：`worker get <worker-id-or-exact-name>` 和
  `model file list --worker <worker-id-or-exact-name>` 已完成实现、离线测试、提交和 fork
  推送；阶段为 `ready_for_review`，尚未接受。
- Git 证据：设计检查点为 `775ea707801a2650d1092758c37b2c4a8d55a74a`，实现提交为
  `1130546ca89f99f59dc318d9694a6552a023fd8a`。本机、fork 和 90 工作树同步到实现提交，
  目标 tracked 文件干净。
- 实现证据：Worker 数字 ID/精确名称解析、正整数门禁、无结果/重名失败、安全白名单
  table/JSON 和 GPU 数量摘要已覆盖；ModelFile 列表强制 Worker scope、调用类型化
  `/v2/model-files?page=-1&worker_id=<id>`、按 ID 排序并支持空列表。
- 测试证据：固定 Rust 1.99.1 下 152 项 workspace 测试、全部 doc-test、fmt、
  locked/offline all-targets check、Clippy `-D warnings` 和 locked/offline build 全部通过。
  mock 覆盖 ID/精确名称、URL 编码、无结果、重名、401/403/404/5xx、敏感扩展排除、稳定
  排序、table/JSON 与空列表。
- 构建证据：从干净实现提交重建的 debug CLI 报告
  `git_commit=1130546ca89f`、`rust_toolchain=1.99.1`，SHA-256 为
  `fd20877b521e14558b2e76ab47e716548eb708c0b30044969ed12d7203e4579c`；Cargo.lock SHA-256
  保持 `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`，没有新增依赖。
- 仓库级 lint：`make lint` 已执行，但 90 主机仍缺少 `uv`，在 pre-commit 启动前以
  status 127 退出；该环境限制不记为通过，也没有安装新工具。
- 外部副作用：没有读取真实 Context/token、连接真实 Server、改变 maintenance、停止
  Python Worker、访问 Docker socket、创建容器、申请 GPU 或运行推理。
- 漂移审计：实现与 D103–D106 对齐，只补齐 D090 的只读 CLI 证据入口；D086
  `STARTING` 边界和 D098–D102 的分段授权未改变，结论为 `aligned`。
- 当前停止位置：等待聊天中的 D033 详细实现报告和用户验收；M6-A.3.0 尚未执行且仍为
  `not_ready`。验收本阶段也不会自动授权预检或维护窗口。

### 2026-10-24：M6-A.2.3 只读预检 CLI 接受

- 结果与状态：用户审阅 D033 详细实现报告后确认无问题；M6-A.2.3 从
  `ready_for_review` 更新为 `accepted`。
- 接受证据：设计检查点为 `775ea707801a2650d1092758c37b2c4a8d55a74a`，实现提交为
  `1130546ca89f99f59dc318d9694a6552a023fd8a`，待审治理检查点为
  `1a756a7d7f3be9ead1649dbaf2cbda16ed23ae02`。固定 Rust 1.99.1 下 152 项 workspace
  测试、全部 doc-test、fmt、locked/offline check、Clippy `-D warnings`、build 和干净
  二进制身份继续作为接受依据。
- 接受范围：`worker get` 的 ID/精确名称解析、安全白名单 table/JSON，以及强制 Worker
  scope、按 ModelFile ID 稳定排序的 `model file list`。没有接受全局 Worker/ModelFile
  枚举、ModelInstance list 或新的 Server API。
- 已知限制：`make lint` 仍因 90 主机缺少 `uv` 未能启动 pre-commit；该限制没有因用户
  接受而改写为通过，也没有安装新依赖。
- 明确非结论：本次接受只证明离线 CLI、mock 契约和构建门禁，不证明真实 operator
  token、真实 Worker/ModelFile 响应、Server 状态链、maintenance、Python Worker drain、
  Docker/vLLM、推理或 GPU 能力。
- 漂移审计：接受没有改变 D086 `STARTING` 边界、D098–D102 的验证协议或两级授权；没有
  把离线 artifact 提升为真实 Managed 证据，结论为 `aligned`。
- Git 状态：接受前复核本机、fork 和 90 均为 `1a756a7d`；90 tracked 工作树干净，本机
  仅有既存未跟踪 `output/`，未纳入提交。
- 当前停止位置：M6-A.2.3 已接受；M6-A.3.0 从 CLI 缺口导致的 `not_ready` 更新为
  `ready_for_authorization`，但尚未连接真实 Server 或执行任何预检检查。
- 下一项允许执行的工作：只讨论或由用户单独明确授权 M6-A.3.0。即使预检通过，M6-A.3.1
  的 Server 写入、maintenance 切换和 Python Worker 维护窗口仍须新的明确授权。

### 2026-10-24：M6-A.3.0 因缺少 operator Context 停止

- 授权与范围：用户在 M6-A.2.3 接受后确认继续，授权执行 M6-A.3.0 只读预检。范围允许
  读取既有 Context/token、查询真实 Server/Worker/ModelFile、检查文件/Docker/GPU/端口
  并建立 Git 外证据；禁止 Server 写入、maintenance 变化、停止 Python Worker、启动 Rust
  Worker、容器、GPU workload 或推理。
- 源码门禁：本机、fork 和 90 均为治理提交
  `e0a4d1c0d32dde94be6e6289340931ccbff72623`，90 tracked 工作树干净；M6-A.2.2
  `f19a8fcd` 和 M6-A.2.3 `1130546c` 均为祖先，Cargo.lock SHA-256 保持
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`。
- 无效封装尝试：首次第 1 组命令的检查内容均显示通过，但本机 PowerShell 管道给远端脚本
  末行引入 CRLF，使 `exit 0` 参数解析失败。该尝试发生在证据目录建立前，未改变文件或
  外部状态，分类为 `invalidated_harness`；改用无 CRLF 的 Base64 传输后同一检查组正式
  通过。
- 构建证据：从干净治理 HEAD 使用固定 Rust 1.99.1 和 locked/offline 依赖重建 debug
  CLI；内嵌 commit 为 `e0a4d1c0d32d`，二进制 SHA-256 为
  `813a18e1746585c0766cacfd9fc6adce0f5747714372c592512c691a6de0099c`。
- 停止事实：正式 CLI `context list` 返回空数组，Context 数量和 active Context 数量均为
  0；`~/.config/gpustack-rs` 不存在。第 2 组因此在读取 token、请求 `/version`、查询 Worker
  或模型之前按硬门禁停止。M6-A.3.0 状态为 `stopped_preflight`，不是
  `passed_preflight`。
- 非评分诊断：90 上仍有 mode 0600 的 M3 cluster registration token，以及 mode 0600 的
  Rust Worker identity/per-Worker token；它们分别属于注册和 Worker system principal，不能
  代替 operator Context，未读取其内容。
- 证据：停止证据位于
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal`，目录 mode 0700，
  7 个文件均为 0600，秘密扫描通过；`manifest.sha256` 自身 SHA-256 为
  `d5875fbe699bd7a54a79309ccf154e8376c18c9f39d74685b9f365ee91711eda`。固定证据路径现在
  已被本次停止占用，不得删除或静默覆盖。
- 外部副作用：只发生 Cargo artifact 清理/重建和 Git 外证据写入。没有读取任何 token
  内容，没有发出 Server 请求，没有改变 maintenance、停止 Python Worker、启动 Rust
  Worker、访问 Docker socket、创建容器、申请 GPU 或运行推理。
- 漂移审计：按 D098 的只读边界和手册停止条件执行，没有把缺失前置条件绕过为 curl、
  cluster token 或 Worker token，也没有把部分第 1 组证据提升为预检通过，结论为
  `aligned`。
- 当前停止位置：M6-A.3.0 停在 operator Context 前置条件；M6-A.3.1 仍为 `not_started`
  且未授权。
- 下一项允许执行的工作：只讨论并冻结如何安全获得 operator token、创建 active Context，
  以及如何保留本次证据并为重跑提供未使用路径。完成准备后也必须重新检查外部漂移并获得
  新的 M6-A.3.0 执行授权。

### 2026-10-24：M6-A.3 operator Context 准备门禁冻结并获授权

- 源码核查：固定 Server `1cfcbbff` 的 `POST /auth/login` 接受表单认证但只把默认 120 分钟
  JWT 写入 `gpustack_session` cookie；Rust `ReqwestTransport` 固定使用 Bearer。Server 的
  `authenticate_request` 对 cookie 解码 JWT、对 Bearer/X-API-Key 调用 API Key 校验，因此
  login JWT 不能直接作为 Rust Context token。
- 决策：用户接受 D107–D110。创建唯一 `rust-worker-m6a3-operator-20261024` API Key，
  scope 仅 `management`、有效期 7 天；token 文件固定为
  `/home/wujiaxing2/.config/gpustack-rs/credentials/m6a3-operator.token`，active Context 固定为
  `lab90`。不复用 Cluster 或 Worker system credential。
- 授权：用户明确同意该一次 API Key Server 写入和本地 Context/token 写入。授权不包含
  maintenance、Model/ModelInstance/ModelFile、Python Worker、Rust Worker、Docker、GPU、
  推理或重跑 M6-A.3.0。
- secret 边界：管理员用户名和密码只由用户在 90 终端无回显输入；密码、session cookie、
  完整 Key 响应只允许进入 mode 0600 临时文件，验证后逐文件清除，不进入参数、终端、聊天、
  Git 或证据。此前聊天中出现的密码不得被助手复用。
- 证据滚动：第一次停止证据目录 `m6a3-formal` 保持不可变；下一次获授权的完整预检固定使用
  尚未创建的 `m6a3-formal-attempt-2`。准备完成不自动开始预检，仍需重新检查漂移并获得新
  授权。
- 漂移审计：凭据准备是第一次预检暴露出的真实前置条件；API Key 创建是经用户单独授权的
  intentional change。它不放宽 D098–D102 的只读预检、维护窗口、状态链或回滚门禁，结论为
  `intentional_change_approved`。
- 当前停止位置：设计和授权已完成，状态为 `authorized_pending_human_secret_entry`；等待
  用户在 90 主机执行交互命令并返回脱敏结果。
- 下一项允许执行的工作：只执行 Context 准备脚本、确认临时 secret 已清除，并用正式 CLI
  只读查询精确 Rust Worker。成功后记录准备证据，但不得自动重跑 M6-A.3.0。

### 2026-10-24：第一次 operator 交互登录被拒绝并安全清理

- 用户在 90 主机执行固定辅助脚本并输入本地管理员用户名和隐藏密码；`POST /auth/login`
  返回 401，脚本在调用 API Key 创建 endpoint 前按失败关闭，未重试也未输出凭据。
- 清理复核：最终 token 文件和 `contexts.json` 均不存在，`.m6a3-context.*` 临时秘密目录为
  0；Server health 与既有 Python Worker health 均为 200。因此本次没有创建 API Key、
  Context 或其他 Server 资源，也没有影响 Worker。
- 判定：这是凭据输入/认证失败，不是 M6-A.3.0 预检结果，不能提升为 Context 准备通过。
  D107–D110 和用户对一次专用 API Key 创建的授权保持有效；状态仍为
  `authorized_pending_human_secret_entry`。
- 下一项允许执行的工作：用户先确认当前本地管理员凭据，再运行同一受保护脚本。若再次
  返回 401，则停止重复尝试，转入只读认证配置与账户状态诊断。

### 2026-10-24：第二次 operator 登录被拒绝并转入认证恢复等待

- 用户明确要求助手使用其凭据登录。为避免把密码放入工具参数和审计记录，用户在 90 主机
  将密码无回显写入 mode 0600 临时文件；助手只引用固定路径向同一受审计脚本提供标准输入。
- 结果：临时文件 metadata 检查通过，但正式 `POST /auth/login` 再次返回 401；脚本仍在
  API Key 创建前失败关闭。外层 trap 删除密码文件，内层 trap 删除表单/cookie 临时文件；
  最终 token、Context 和 `.m6a3-context.*` 目录均不存在。
- 只读诊断：公开 `/auth/config` 显示没有 external auth、OIDC 或 SAML；Server 容器没有通过
  environment、command 或 config-file 参数覆盖管理员用户名。Server 和 Python Worker
  health 均为 200，近期容器日志没有提供额外的脱敏认证原因。
- 判定：两种独立输入路径对默认本地管理员身份均得到相同 401，当前实际密码与输入不匹配；
  不能继续猜测或把认证失败归因于 Context。凭据准备状态更新为
  `waiting_for_local_auth_recovery`，不是 M6-A.3.0 预检失败。
- 下一项允许执行的工作：由用户在新的浏览器会话验证当前本地登录；若无法登录，则先讨论
  官方密码恢复的影响并获得单独写授权。认证恢复前不再调用登录 endpoint，不创建 API Key，
  也不开始第二次预检。

### 2026-10-26：operator Context 通过 Web 管理会话恢复并验收

- 认证纠偏：用户随后确认此前多次 401 均包含密码误输。第三次受保护文件尝试仍返回 401，
  外层 trap 删除密码文件；只读数据库核对仅确认 `admin` 为启用的 Local 平台管理员和密码
  行有效，没有读取密码哈希。此后停止调用 `/auth/login`，没有执行密码重置。
- 泄露处置：用户曾把一个完整推理 API Key 发到聊天和截图；该 Key 立即视为泄露，未被助手
  使用，用户从 Web 页面删除。随后创建的临时网页 Key 在 mode 0600 文件中验证管理 API
  返回 403，证明它只有推理权限；本地副本随即删除，Server 记录由用户在 Web 页面删除。
- 正式创建：用户通过既有已认证 Web 管理会话调用正式 `/v2/api-keys`，把完整值直接无回显
  写入 90 主机，不进入聊天、工具参数或终端输出。API 权威核对发现该 Key 实际为永久且
  scope 为 `management,inference`，与冻结契约不符，因此未创建 Context。
- 最小权限修复：用不合规 Key 创建 ID 18、30 分钟、仅 `management` 的桥接 Key 并将恢复
  值保存在 mode 0600 文件；验证桥接管理能力后删除 ID 17，创建 ID 19、同名
  `rust-worker-m6a3-operator-20261024`、仅 `management`、7 天到期
  `2026-11-02T08:49:57.571942Z` 的正式 Key，原子替换 token 后删除桥接 Key 和恢复文件。
- Context 验收：`lab90` 指向 `http://127.0.0.1/` 和固定 token 文件并处于 active；token 与
  `contexts.json` 均为 owner `wujiaxing2`、mode 0600、单链接。正式 CLI
  `worker get rust-m3d-r760xa` 成功读取 ID 5、maintenance Rust Worker，证明只读管理链路
  可用。Server health、Python Worker health/metrics 均为 200，90 Git 工作树保持干净。
- 清理与证据：临时密码、cookie、创建响应、网页引导、桥接和修复文件残留均为 0。脱敏证据
  位于 `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-operator-context-20261026`，
  目录 mode 0700、6 个文件均为 0600，秘密扫描通过；`manifest.sha256` 自身 SHA-256 为
  `910bf0143f09f7388005007a6d8af1329e900dc9055c87bec7e4d357008e10e9`。正式第二次预检目录
  `m6a3-formal-attempt-2` 仍未创建。
- 漂移与授权：最终状态完全符合 D107–D110，恢复路径冻结为 D111–D113。没有改变 Worker、
  maintenance、Model/ModelInstance/ModelFile，没有停止 Python Worker、启动 Rust Worker、
  访问 Docker、创建 workload、使用 GPU 或推理。Context 准备状态为
  `operator_context_ready`，不是 `passed_preflight`。
- 下一项允许执行的工作：只讨论或由用户重新明确授权第二次 M6-A.3.0 只读预检。即使预检
  通过，M6-A.3.1 的 Server 写入、maintenance 切换和 Python Worker 维护窗口仍须新的明确
  授权。

### 2026-10-26：第二次 M6-A.3.0 因 Worker identity 漂移停止

- 授权与范围：用户在 operator Context 验收报告后回复“没问题”，明确允许执行第二次
  M6-A.3.0 只读预检。范围仅包含冻结源码构建、真实 GET/CLI 查询、文件/GPU/端口/Docker
  只读基线和 Git 外证据；不包含任何 Server 写入、maintenance 变化、停止 Python Worker、
  启动 Rust Worker、Docker/GPU workload 或推理。
- 源码门禁：本机、fork 和 90 均为 `e50717982a93645df8a9c1b261199376771c736d`，90 tracked
  工作树干净；M6-A.2.2 `f19a8fcd` 与 M6-A.2.3 `1130546c` 均为祖先，后续 Rust 源码、
  manifest、toolchain 和 `Cargo.lock` 无漂移。`Cargo.lock` SHA-256 为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`。
- 构建证据：使用固定 Rust/Cargo 1.99.1、`--locked --offline` 从冻结提交重建 CLI；内嵌
  commit 为 `e50717982a93`，二进制 SHA-256 为
  `bd3b07fc358c88abc3fd733934cdbc52daa12164c7571238640514ec70544dae`。
- 硬停止事实：`m3d/managed/worker-identity.json` 为 mode 0600，记录 Worker ID 2、名称
  `rust-m3d-r760xa` 和 UUID `f38b7c2c-90f7-4a76-a2c5-dfe1ff63fea0`；正式 CLI 对同名
  Worker 的权威查询返回 ID 5。名称一致但 ID 不一致，触发手册的 identity mismatch 停止
  条件，预检状态为 `stopped_preflight`，不是 `passed_preflight`。
- 非评分只读诊断：当前 `workers` 表只有同名 ID 5，创建时间为
  `2026-10-22 12:22:40.232608`，旧 ID 2 已不存在；当前行没有原持久 UUID。因此该差异是
  Server identity 外部漂移，不能在同一次预检中通过改写本地 identity、删除或复用 ID 5
  绕过。进一步枚举只读 identity metadata 发现 ID 5/UUID
  `7c4e07a3-6b97-4577-9ff0-1fc6c2319596` 的匹配 identity 和 per-Worker token 位于
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/1/managed/`，两文件均为 owner
  `wujiaxing2`、mode 0600、单链接；没有读取 token 内容。冻结的 M6 数据目录 `m3d` 仍保存
  ID 2，因此问题具体收敛为多个持久数据目录与当前 Server 身份的选择漂移，而非 ID 5
  凭据完全丢失。
- 未执行检查：发现硬停止后没有查询或删除目标 Model/ModelInstance/ModelFile，没有验证
  模型 manifest、GPU、Managed 端口、进程或 Docker/Python Worker 恢复基线，也没有创建
  任何 Server 资源。停止后的安全复核显示 Server、Python Worker health/metrics 均为
  200，`gpustack-rs` 进程为 0，`20150/20151` 空闲，`gpustack-worker` 保持 running、restart
  count 0。
- 证据：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-2` 为 mode
  0700，13 个文件均为 0600，秘密扫描通过；`manifest.sha256` 自身 SHA-256 为
  `109d4747dd452e7987046c6c3a43e62a3d9fea845f5c275c0894f6e66087be33`。第一次停止目录和
  operator Context 证据目录均未修改。
- 外部副作用与漂移审计：仅清理/重建 Cargo artifact 并写入新的 Git 外证据目录；没有
  改变 Server、Worker、maintenance、模型、容器、端口或 GPU 状态。按冻结停止条件失败
  关闭，结论为 `aligned`。
- 下一项允许执行的工作：只讨论并冻结是采用 ID 5 的既有 `/1` 数据目录，还是把其身份
  受控迁移到新的明确目录，同时固定第三次证据目录和新的 M6-A.3.0 授权；不得直接覆盖
  `m3d` 中 ID 2 的历史身份。M6-A.3.1 仍为 `not_started` 且未授权。

### 2026-10-26：M6-A.3 当前 Worker identity 恢复已冻结并应用

- 决策：用户接受 D114–D116，选择直接采用现有
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/1`，不复制或迁移 Worker 凭据。该目录
  的 identity 为 ID 5、名称 `rust-m3d-r760xa`、UUID
  `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`；旧 `m3d` 中 ID 2 保持历史只读状态。
- 只读身份验证：使用 `/1/managed/worker-token` 调用 `GET /v2/workers?me=true` 返回 HTTP
  200 和唯一 ID 5/name，与本地 identity 一致。固定 v2.2.3 的 `me` 响应不返回 UUID，
  因此仍按 D035 从受保护本地 identity 恢复 UUID；token 值未输出、未写入证据或 Git。
- 目录审计与修复：`/1` 仅含 mode 0700 的 `managed/` 及两个 mode 0600、单链接身份文件，
  没有锁文件或 Lab 状态；顶层目录从 0775 收紧为 0700。未移动、覆盖或删除 `/1`、`m3d`
  及前两次正式证据中的任何身份文件。
- 安全复核：临时 Authorization header/response 目录残留为 0；Server、Python Worker
  health 和 metrics 均为 200，`gpustack-rs` 进程为 0。没有修改 Server、maintenance、
  Model/ModelInstance/ModelFile，没有停止 Python Worker、启动 Rust Worker、访问 Docker、
  创建 workload 或使用 GPU。
- 证据：脱敏恢复证据位于
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-identity-recovery-20261026`，
  目录 mode 0700、文件 mode 0600，秘密扫描通过；`manifest.sha256` 自身 SHA-256 为
  `955f20e6efc42c903800dbb2b92365ce82cf01bec1e1ea6c894f804c83fd629e`。
- 漂移与状态：只修订当前持久身份路径和证据滚动路径；固定 Server、Worker name、模型、
  GPU、状态链、验收条件及回滚顺序不变。第三次预检目录固定为尚未创建的
  `m6a3-formal-attempt-3`，M6-A.3.0 状态为 `ready_for_authorization`，不是
  `passed_preflight`。
- 下一项允许执行的工作：只讨论或由用户另行明确授权第三次 M6-A.3.0 只读预检。第三次
  预检通过并共同审阅前，M6-A.3.1 仍为 `not_started` 且未授权。

### 2026-10-26：第三次 M6-A.3.0 因无效 CLI 调用不计分

- 授权与范围：用户在 identity 恢复检查点后回复“没问题”，授权第三次 M6-A.3.0 只读
  预检。范围仍只有冻结源码构建、真实只读查询和 Git 外证据；不包含 Server 写入、
  maintenance 变化、停止 Python Worker、启动 Rust Worker、Docker/GPU workload 或推理。
- 外部漂移门禁：本机、fork 和 90 均为
  `813920eb56278a63cad285cdde21b7380c16e2f8`，90 工作树干净；已接受代码提交
  `1130546ca89f99f59dc318d9694a6552a023fd8a` 为祖先且后续无 Rust/Cargo/toolchain 漂移。
  当前 `/1` 为 mode 0700、identity ID 5/name 正确，旧 `m3d` ID 2 保持不变；前两次停止
  证据和 identity 恢复证据的 manifest 文件哈希均未漂移。
- 构建证据：使用 Rust/Cargo 1.99.1、`Cargo.lock` 和 `--locked --offline` 重新构建成功；
  `Cargo.lock` SHA-256 为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`，二进制内嵌 commit
  `813920eb5627`，SHA-256 为
  `1a4b110e20eb352e487f92ff574d93ee6628e8d040972ed6f541f586f3bbdc5e`。
- 无效调用：身份门禁的第一条命令错误使用
  `gpustack-rs --context lab90 --output json context list`。Context 管理命令按产品契约拒绝
  `--context`，CLI 在本地参数检查阶段退出；正确形式应为不带 selector 的
  `gpustack-rs --output json context list`。因此没有读取 Context token、没有发送该 CLI
  Server 请求，Worker/Model/ModelInstance/ModelFile、模型 manifest、GPU、端口和 Docker
  正式门禁均未执行。
- 判定：原 `preflight-result.txt` 保留 `STOP/operator_context_list_failed` 原始结果，另以
  `adjudication.txt` 标记 `INVALIDATED/invalid_context_list_invocation`。该尝试不计分，不能
  解释为系统预检失败，也不能在同一证据目录改命令继续。
- 安全与证据：Server、Python Worker health/metrics 均为 200，`gpustack-rs` 进程为 0；
  没有读取或输出 secret，没有任何 Server/maintenance/Worker/Docker/GPU 副作用。证据目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-3` 为 0700，
  14 个文件均为 0600，秘密扫描通过；`manifest.sha256` 自身 SHA-256 为
  `e0920b96a175989550baaa4562a10732ea0ad7525e29e3f7223826e3fbee36b4`。
- 当前停止位置：M6-A.3.0 为 `waiting_for_retry_amendment`，新的未占用证据路径和重试授权
  尚未冻结。M6-A.3.1 仍为 `not_started` 且未授权。

### 2026-10-26：第四次 M6-A.3.0 修订冻结并获授权

- 决策与授权：用户接受 D117–D119，将新证据路径固定为尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-4`，并授权
  第四次 M6-A.3.0 的冻结构建、真实只读查询和 Git 外证据写入。
- 调用修正：`context list` 固定为不带 selector 的
  `gpustack-rs --output json context list`；只有 Worker、Model 和 ModelFile 等 Server
  命令使用 `--context lab90`。不修改 Context store、token 或任何冻结输入。
- 边界：attempt 1、attempt 2、无效 attempt 3 和 identity 恢复证据保持不可变。授权不包含
  Server 写入、maintenance 变化、停止 Python Worker、启动 Rust Worker、Docker/GPU
  workload 或推理；M6-A.3.1 仍为 `not_started`。
- 下一项允许执行的工作：完成外部漂移复核后执行第四次 M6-A.3.0；任一硬门禁失败立即
  停止并封存 attempt 4。

### 2026-10-26：第四次 M6-A.3.0 因 GPU0 被既有 Lab 占用停止

- 源码和构建：本机、fork 和 90 均为
  `6684a32643d82380a570ce5ebfe94fcd8dbab533`，90 工作树干净；已接受代码提交
  `1130546ca89f99f59dc318d9694a6552a023fd8a` 为祖先且后续无 Rust/Cargo/toolchain 漂移。
  Rust/Cargo 1.99.1、`--locked --offline` 构建通过，`Cargo.lock` SHA-256 为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`；二进制内嵌 commit
  `6684a32643d8`，SHA-256 为
  `5731421f280545f82e92b13cc2b97da687238902965898b2d67f8b8255fa4a1e`。
- 身份与资源门禁：修正后的 Context 枚举、active `lab90`、固定配置均通过。Server Worker
  与 `/1` identity/per-Worker token 均为唯一 ID 5/name `rust-m3d-r760xa`，UUID 为
  `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`，状态为 maintenance。目标 Model count、该 Worker
  的 ModelInstance count 和 ModelFile count 均为 0。
- 模型门禁：固定 Qwen2.5 路径位于允许根目录，10 个顶层普通文件、0 个子目录、
  999,604,126 bytes、完整 manifest、权重 SHA-256、`Qwen2ForCausalLM` 和非空 chat template
  均与已接受 D084 证据一致。
- GPU 硬停止：GPU0 UUID/name 未漂移，但存在 PID 2204755 `VLLM::EngineCore`，占用
  76,034 MiB。cgroup 精确映射到既有容器
  `gpustack-rs-lab-47b11d29-f749-4250-9ca3-8fa8c5bdfa45`；容器运行固定 Runner、restart count
  0。受保护 Lab 状态文件将其标识为 `qwen2.5-0.5b-m5e/RUNNING`，位于
  `m5e-e0-20261022-125241/lab/`。Server 全局 ModelInstance 为 0，证明它是独立 Lab，且
  `created_by_attempt4=false`。
- 未执行检查：发现 GPU0 非空闲后，没有继续执行正式端口/进程门禁、Python Worker Docker
  恢复基线、回滚 CLI help 或全局资源 ID 基线；停止后的容器/cgroup/Server 查询仅为
  非评分归因诊断。没有停止、删除或修改该 Lab。
- 安全与证据：Server、Python Worker health/metrics 均为 200，`gpustack-rs` 进程为 0；
  没有改变 Server、maintenance、Python Worker、Rust Worker、Docker 或 GPU workload。
  正式证据目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-4` 为 0700，
  27 个文件均为 0600，秘密扫描通过；`manifest.sha256` 自身 SHA-256 为
  `03e8ff6c78ce4135e545efd9849d57b3c1aa9aded499eecaaf02581ec30a5d5b`。
- 判定与停止位置：attempt 4 为有效 `STOP/gpu0_not_idle`，M6-A.3.0 状态为
  `stopped_preflight`，不是 `passed_preflight`。下一项只允许讨论并授权该精确 Lab 的处置、
  新证据路径和后续只读预检；M6-A.3.1 仍为 `not_started`。

### 2026-10-26：用户清理既有 Lab，GPU0 blocker 已解除

- 外部动作：用户确认 `qwen2.5-0.5b-m5e` 是此前测试部署，并自行完成停止和删除。本项目
  没有代替用户执行该删除，也没有修改 attempt 4 的证据目录。
- 只读复核：精确容器 ID
  `6afffa063ff89f0747c03f361b94cf753bb1b1f78948155b925752ddbb3c5e99` 的 Docker 记录为 0，
  精确 deployment state 文件不存在；GPU0 保持固定 UUID/name，显存占用和利用率均为 0，
  compute process 数为 0。
- 数据与服务安全：固定 Qwen2.5 目录仍存在且含 10 个顶层普通文件；此前完整 manifest
  复核保持通过。Server、Python Worker health 和 metrics 均为 200，`gpustack-rs` 进程为
  0，`20150/20151` 均无监听。
- 解释：这次清理只改变了 attempt 4 结束后的外部运行状态。attempt 4 仍是有效
  `STOP/gpu0_not_idle`，不能借当前 GPU 空闲追溯改判；未执行的端口、Docker 恢复、回滚
  CLI 和全局 ID 门禁仍然未执行。
- 漂移审计：固定 Server、Worker ID 5、模型、Runner、GPU、状态链和验收标准均未修改；
  当前外部状态与再次执行只读预检的前置条件一致，但没有新的正式证据目录或执行授权。
- 当前停止位置：M6-A.3.0 为 `ready_for_retry_amendment`，M6-A.3.1 仍为 `not_started`。
- 下一项允许执行的工作：讨论并冻结第五次 M6-A.3.0 的唯一未占用证据路径和只读执行
  授权；获得明确授权后必须从头重跑全部门禁，不得从 attempt 4 的中断位置续跑。

### 2026-10-26：第五次 M6-A.3.0 已冻结并获只读授权

- 决策与授权：用户在 GPU blocker 清理报告后回复“没问题”，接受 D120–D122。第五次
  证据目录固定为尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-5`，并授权
  冻结构建、真实只读查询和 Git 外证据写入。
- 重试边界：必须从头执行全部门禁，不能复用 attempt 4 已通过的中间结果或补跑其未执行
  检查。attempt 1～4 和 identity 恢复证据保持不可变。
- 漂移审计：固定 Server、Worker ID 5、模型、Runner、GPU、状态链、通过条件和回滚协议
  均未改变；唯一外部变化是用户删除旧测试 Lab 后 GPU0 恢复空闲，结论为 `aligned`。
- 授权边界：不允许 Server 写入、maintenance 变化、停止 Python Worker、启动 Rust Worker、
  Docker/GPU workload 或推理；M6-A.3.1 仍为 `not_started`。
- 当前停止位置：M6-A.3.0 attempt 5 为 `authorized_pending_execution`。
- 下一项允许执行的工作：从新的未占用目录执行第五次完整只读预检；任一硬门禁失败立即
  停止并封存证据。

### 2026-10-26：第五次 M6-A.3.0 因 Cargo working directory 错误不计分

- 执行事实：attempt 5 建立新证据目录并通过源码 HEAD、origin、干净工作树、已接受代码
  祖先、无 Rust drift、Rust/Cargo 版本和 `Cargo.lock` identity 检查。随后脚本在仓库根
  `/home/wujiaxing2/workspaces/gpustack` 执行 `cargo clean -p gpustack-cli`；Rust workspace
  实际位于其 `rust/` 子目录，Cargo 因找不到 `Cargo.toml` 以退出码 101 停止。
- 判定：这是预检执行器的 working directory 调用错误，不是源码、构建、Server 或环境
  门禁失败。原 `preflight-result.txt` 保留
  `STOP/unexpected_command_failure_exit_101`，治理判定为非评分
  `INVALIDATED/invalid_cargo_working_directory`；不得在同一目录修正后续跑。
- 未执行范围：没有生成正式 binary，没有读取 operator 或 per-Worker token，没有发送
  Worker/Model/ModelInstance/ModelFile 查询，也没有进入模型、GPU、端口、Docker/Python
  Worker、回滚 CLI 或全局 ID 门禁。
- 安全与证据：Server、Python Worker health/metrics 均为 200，`gpustack-rs` 进程为 0；
  没有任何 Server、maintenance、服务、容器或 GPU 副作用。证据目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-5` 为 0700，
  8 个文件均为 0600，秘密扫描为 0；`manifest.sha256` 自身 SHA-256 为
  `2dd631a89066be9ccb5636a547756162fb9e2c390817e080059cc60db02d9cb4`。
- 当前停止位置：M6-A.3.0 为 `waiting_for_retry_amendment`，M6-A.3.1 仍为 `not_started`。
- 下一项允许执行的工作：讨论并冻结新的唯一证据路径，先把 Cargo working directory 修正
  为 `$REPO/rust` 并完成脚本静态/只读审计，再另获新的 M6-A.3.0 执行授权。

### 2026-10-26：第六次 M6-A.3.0 已冻结并获只读授权

- 决策与授权：用户在 attempt 5 无效报告后回复“没问题”，接受 D123–D125。第六次证据
  目录固定为尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-6`，并授权
  脚本审计、冻结构建、真实只读查询和 Git 外证据写入。
- 执行器修正：Cargo `clean/build` 显式指定 `$REPO/rust/Cargo.toml`；正式目录创建前必须
  通过 `bash -n`、`cargo metadata --manifest-path ...` 和与 attempt 5 脚本的窄范围差异
  审计。日志同步写入 evidence，最终摘要使用独立文件描述符，避免 manifest 生成后尾写；
  attempt 5 manifest 加入前序证据检查。
- 漂移审计：固定 Server、Worker ID 5、模型、Runner、GPU、状态链、通过条件和回滚协议
  均未改变；修订只作用于预检执行器，结论为 `aligned`。
- 授权边界：不允许 Server 写入、maintenance 变化、停止 Python Worker、启动 Rust Worker、
  Docker/GPU workload 或推理；M6-A.3.1 仍为 `not_started`。
- 当前停止位置：M6-A.3.0 attempt 6 为 `authorized_pending_execution`。
- 下一项允许执行的工作：通过证据目录外的脚本审计后，从新的未占用目录执行第六次完整
  只读预检；任一硬门禁失败立即停止并封存证据。

### 2026-10-26：第六次 M6-A.3.0 完整只读预检通过

- 目的与授权：按 D123–D125 从头执行 M6-A.3.0，只允许脚本审计、冻结构建、真实只读
  查询和 Git 外证据写入；不允许改变 maintenance、停止 Python Worker、启动 Rust Worker、
  写 Server 资源或创建 Docker/GPU workload。
- 预执行审计：正式证据目录创建前，脚本通过 `bash -n`、显式
  `cargo metadata --manifest-path .../rust/Cargo.toml` 和 attempt 5/6 窄范围差异核对。
  正式脚本 SHA-256 为
  `9b2eba10c581401e4f5e4f902b7bf46cfbdae20faed296510fc911a9f3ad84f0`；差异只包含新
  attempt identity、Cargo manifest 路径、同步 evidence 日志/独立最终摘要和 attempt 5
  前序证据检查。
- 源码与构建：本机、fork 和 90 使用提交
  `61b5aab437d0a9b6c643a4469e5a29fcd34626b8`，90 tracked 工作树干净；已接受代码
  `1130546ca89f99f59dc318d9694a6552a023fd8a` 为祖先且无后续 Rust 漂移。Rust/Cargo
  1.99.1、`--locked --offline` 构建通过；`Cargo.lock` SHA-256 为
  `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`，二进制内嵌
  commit `61b5aab437d0`，mode 0700，SHA-256 为
  `99a73e07efac81107f49f39690365e91406320a1a3c7b66726db6a05af2942d4`。
- 身份与资源：active Context 为 `lab90`；持久 identity、Server Worker 和 per-Worker
  `me` 均指向唯一 ID 5/name `rust-m3d-r760xa`，UUID 为
  `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`，Worker 保持 maintenance。目标 Model、该 Worker
  的 ModelInstance 和 ModelFile 数均为 0；全局基线分别为 Model
  `[1,2,3,4,6,7,8,9]`、ModelInstance `[]`、ModelFile `[1,2,3,4,5,6]`、Worker `[1,5]`。
- 模型与 GPU：固定 Qwen2.5 revision、canonical path、10 个顶层文件、999,604,126 bytes、
  全文件 manifest、权重 SHA-256、架构和 chat template 均通过。GPU0 保持固定 A100 UUID，
  显存占用 0 MiB、compute process 0；GPU1 只记录外部基线。
- 端口、Docker 与恢复：`20150/20151` 和 `40000-40063` 无监听，`gpustack-rs` 进程为 0。
  已记录精确 Python Worker container ID
  `68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165`、固定 image digest、
  `unless-stopped`、host network、命令和两个 mount；Server、Python Worker health/metrics
  均为 200。
- 证据与判定：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-6` 为 0700，
  45 个文件均为 0600，44 个 manifest 条目逐项校验通过，秘密扫描为 0；
  `manifest.sha256` 文件 SHA-256 为
  `984919960979ff8d0df503da0a0355d5b703850ad162bd4f585e0c4a0ea04c59`。结果为
  `PASS/all_read_only_preflight_gates_passed`，M6-A.3.0 状态为 `passed_preflight`。
- 副作用与收尾：所有 mutation 标志均为 false；独立复核确认服务仍健康、GPU0 无 compute
  process、Rust/受管端口无监听，远端临时脚本按精确路径删除。attempt 1～5 和 identity
  恢复证据均未修改；漂移审计结论为 `aligned`。
- 当前停止位置：M6-A.3.0 已通过，M6-A.3.1 为 `ready_for_authorization`，尚未开始且未
  获授权。该结果没有证明 Managed 文件准备状态链。
- 下一项允许执行的工作：共同审阅本检查点，并由用户单独明确授权 M6-A.3.1 的真实无
  Docker 维护窗口。授权必须精确包含停止/恢复原 Python Worker、切换 Rust Worker
  maintenance、创建/清理唯一 Model/ModelInstance/ModelFile 和启动/停止
  `worker run --managed-file-only`；仍不允许 Docker/vLLM workload、GPU 使用或推理。

### 2026-10-26：M6-A.3.1 attempt 1 已冻结并获维护窗口授权

- 授权：用户在 M6-A.3.0 PASS 的详细报告和精确维护范围后回复“没问题”，接受 D126–D128。
  允许停止并恢复预检记录的 `gpustack-worker`、切换 Worker ID 5/name
  `rust-m3d-r760xa` maintenance、创建并清理唯一测试 Model/ModelInstance/ModelFile，
  以及启动并停止 `worker run --managed-file-only`。
- 证据身份：attempt 1 固定使用尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-1`；
  M6-A.3.0 attempt 1～6 和 identity 恢复证据不得修改。
- 授权排除项：不允许 Docker/vLLM workload、GPU compute、推理、实例进入 `RUNNING`，
  也不允许换模型、Worker、GPU、Runner、端口策略、状态链或超时迁就结果。
- 授权前只读复核：新证据路径未占用，90 工作树干净；Server 与 Python Worker
  health/metrics 均为 200，精确 Python Worker container ID 仍为
  `68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165` 且正在运行；GPU0
  compute process 和 `gpustack-rs` 进程均为 0。固定输入、成功标准和回滚协议无漂移，结论
  为 `aligned`。
- 当前停止位置：M6-A.3.1 attempt 1 为 `authorized_pending_execution`，尚未进入维护窗口。
- 下一项允许执行的工作：提交并同步本授权检查点后，完成证据目录外的执行器静态审计和
  全量外部漂移门禁；全部通过后才能执行一次 900 秒内的真实无 Docker smoke。任一失败先
  恢复 Python Worker 和 Rust Worker maintenance，再封存 attempt 1，不能自动重试。

### 2026-10-27：M6-A.3.1 attempt 1 在维护窗口前停止

- 预执行审计：执行器和独立 900 秒 watchdog 均通过 `bash -n`；目录模型的
  `resolved_paths` 断言已在正式执行前按代码契约修正为单个规范化目录。最终 executor
  SHA-256 为 `ff5362b84327f4bf8bd462b45347ea8e8b7bf2d90e500f9179cd5f8143c3bb52`，
  watchdog SHA-256 为
  `a206010455c5d99bbbc4a2084d1fc60c47c252be86459e3759305bcf45761c44`，二者均复制进证据链。
- 外部漂移门禁：源码、origin 和 90 HEAD 均为
  `581c95f632116e14d69b9b1504a865e4a1e8e675`，远端工作树干净；Server/Python Worker
  health/metrics 为 200，Worker ID 5 处于 maintenance，目标 Model/ModelFile 无冲突，
  固定模型 manifest 通过，GPU0、`20150/20151` 和 `40000-40063` 均空闲。
- 停止原因：冻结源码完成 `--locked --offline` 构建后，执行器使用
  `jq -r '.id' managed/worker-identity.json`，而该文件的权威字段是 `worker_id=5`，因此在
  `identity_resource_and_environment_gate` 返回
  `STOP/persisted_worker_id_mismatch`。这是执行器字段错误，不是 Worker identity 漂移。
- 未进入范围：`python-worker-stop.txt`、`rust-worker.pid`、`maintenance-disable.json` 和
  `model-deploy.json` 均未生成；Python Worker 未停止，Rust Worker 未启动，maintenance 未
  解除，没有创建 Model/ModelInstance/ModelFile、Docker/vLLM workload、GPU compute 或
  推理。`model_id`、`model_instance_id`、`model_file_id` 和 assigned port 均为 `none`。
- cleanup 判读：停止发生在 `before-global-ids.json` 建立之前，通用 cleanup/final-check 因
  缺少该基线把 `cleanup_ok` 记录为 0；其窄范围发现逻辑没有匹配到任何可删除资源，也没有
  执行 maintenance enable、Model 删除或 ModelFile 删除。独立事后审计确认 target Model/
  ModelFile 为 0，Worker 仍为 maintenance，Python Worker 与 Server 健康，Rust 进程和端口
  无残留，GPU0 显存与 compute process 为 0，远端工作树仍干净。
- 证据：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-1`
  为 0700，42 个文件均为 0600，41 个 manifest 条目逐项校验通过，秘密扫描为 0；
  `manifest.sha256` 文件 SHA-256 为
  `acca6ea071ebae4ed3c52d7bf32b81fb85ed7dca375671c3bcd19f240283999c`。watchdog 在正式
  seal 后生成的 log/PID/exit 另存于
  `m6a3-managed-file-smoke-attempt-1-watchdog-2`：目录为 0700，7 个文件均为 0600，6 个
  manifest 条目通过，秘密扫描为 0，清单文件 SHA-256 为
  `c16b337f85b0b14a96f22277bb74bb65a200bb94fa2a2e57d3dadbafc487934f`。第一次补充封装目录
  `m6a3-managed-file-smoke-attempt-1-watchdog` 因零匹配 `grep` 在 `pipefail` 下退出而未 seal，
  作为不计分的 incomplete artifact 原样保留并由 `watchdog-2` 记录其文件哈希。
- 判定与停止位置：attempt 1 为有效 `stopped_before_window`，不是 M6-A.3.1 PASS，也不能
  在原目录修正或续跑。下一项只允许共同审阅停止证据、修订 identity 与 pre-window cleanup
  分支并冻结新的 attempt 2 证据路径；再次执行必须重新通过外部漂移门禁并取得新的维护窗口
  授权。M6-B 仍未开始。

### 2026-10-27：M6-A.3.1 attempt 2 修订方案已冻结

- 决策范围：用户审阅 attempt 1 STOP 报告后回复“没问题”，接受 D129–D131。attempt 2
  正式证据固定为尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-2`；
  attempt 1 正式证据和 watchdog 补充 artifact 均不得修改。
- 执行器修正：持久 identity 改为读取 `worker_id`；新增 `baseline_ready`。全局 ID、Docker、
  Worker 和模型基线完成前若 fail-closed，且 `window_entered=0`，finalizer 只做无写安全复核，
  不执行依赖 `before-global-ids.json` 的删除和集合比较。基线完成或进入窗口后继续使用 D128
  的完整 maintenance、资源、Rust PID、Python Worker、模型数据和零残留回滚。
- 不变项：Server v2.2.3、Worker ID 5/name/UUID、Qwen2.5 revision、Runner digest、GPU0、
  模型参数、`20150/20151`、`40000-40063`、状态链、60 秒 `STARTING` 保持和 900 秒 watchdog
  均不改变；仍禁止 Docker/vLLM workload、GPU compute、推理和 `RUNNING`。
- 授权边界：本次“没问题”只授权文档冻结、执行器准备、证据目录外静态审计和真实只读
  漂移检查。不得创建 attempt 2 正式证据目录，不得停止 Python Worker、改变 maintenance、
  创建测试资源或启动 Rust Worker。
- 当前停止位置：attempt 2 为 `executor_preparation_authorized`。下一项允许执行的工作是完成
  修正执行器、语法/fixture/cleanup 分支审计和只读外部门禁；通过后报告精确脚本 identity，
  状态转为 `ready_for_authorization`，再由用户单独授权维护窗口。

### 2026-10-27：M6-A.3.1 attempt 2 在维护窗口前停止

- 授权与准备：attempt 2 executor SHA-256 为
  `0bc46ec6394fa6a7e335323e54a43617c7c2ac5297cecf011f7dc02360ef274c`，watchdog SHA-256 为
  `b879ddb3a078b7af3f9ecf8aad33dee82f83fb3f34c579e1005ee76d5f4e80ad`。两者通过 `bash -n`
  和 ShellCheck；pre-window failure injection 证明 `baseline_ready=0` 时只做无写安全复核。
  完整只读门禁确认源码/origin/90 HEAD 均为
  `a199649f7818a248e74a21f1dcc27e117d226a08`，工作树干净，Server/Python Worker 健康，
  Worker ID 5 处于 maintenance，目标资源为 0，固定模型 manifest 通过，GPU0 和受管端口空闲。
  用户随后在精确维护范围后回复“没问题”，按 D132 授权一次真实 attempt 2。
- 停止原因：正式执行完成冻结构建和 source gate 后，identity 门禁正确读取
  `worker_id=5`，但把同一持久文件的 `worker_name` 错读为 `.name`，因此返回
  `STOP/persisted_worker_name_mismatch`。受保护文件实际为 `worker_id=5`、
  `worker_name=rust-m3d-r760xa` 和固定 UUID，与 Server 当前 Worker 一致；这是执行器错误，
  不是实现或外部身份漂移。
- 范围与恢复：结果记录 `baseline_ready=0`、`window_entered=0`、`cleanup_ok=1`。没有停止
  Python Worker、切换 maintenance、创建 Model/ModelInstance/ModelFile、启动 Rust Worker、
  创建 Docker/vLLM workload、使用 GPU 或推理。事后审计确认 Server、Python Worker
  health/metrics 为 200，精确 Python Worker 容器仍运行；Worker ID 5 仍为 maintenance，
  目标 Model/ModelFile 为 0，Rust 进程、`20150/20151` 和 `40000-40063` 监听均为 0，GPU0
  显存/利用率/compute process 为 0，远端源码仍干净。
- 证据：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-2`
  为 0700，30 个文件均为 0600，29 个 manifest 条目通过，秘密扫描为 0；清单文件 SHA-256
  为 `5777af9731fe639a10840217f02f598d3e0ed03b6b2ff62f5ba006db1cd17b0a`。watchdog 补充目录
  `m6a3-managed-file-smoke-attempt-2-watchdog` 为 0700，7 个文件均为 0600，6 个 manifest
  条目通过，秘密扫描为 0；清单文件 SHA-256 为
  `220c46ec3d01dddc762d2219e0bc00b379d57091e10cb397d1bbed088f48911b`。
- 判定与停止位置：attempt 2 是有效 `stopped_before_window`，不能在原目录修正或续跑，
  M6-A.3.1 仍未通过。下一项只允许共同审阅该 STOP，冻结 attempt 3 新证据路径和
  `.worker_name` 窄修正；随后重新执行静态/只读门禁并取得新的维护窗口授权。不得自动重试，
  M6-B 仍未开始。

### 2026-10-27：M6-A.3.1 attempt 3 已冻结并获维护窗口授权

- 决策与路径：用户审阅 attempt 2 STOP 和零副作用审计后回复“没问题”，接受 D133–D135。
  attempt 3 唯一正式目录固定为尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-3`；
  attempt 1、attempt 2 及 watchdog artifact 均保持不可变。
- 唯一方法修正：从 attempt 2 封存 executor 派生，把持久 identity 名称读取从 `.name` 改为
  `.worker_name`；同时只更新 attempt、证据路径、冻结 HEAD 和脚本 identity。`worker_id`、
  `worker_uuid`、`baseline_ready`、完整回滚、固定 Server/Worker/模型/Runner/GPU/端口、
  状态链、超时和成功标准不变。
- 授权：静态、pre-window failure injection 和完整只读漂移门禁全部通过后，允许执行一次
  attempt 3，包括停止/恢复精确 Python Worker、切换 Worker ID 5 maintenance、创建/清理
  唯一 Model/ModelInstance/ModelFile 和启动/停止 `--managed-file-only`。不允许 Docker/vLLM
  workload、GPU compute、推理或 `RUNNING`。
- 当前停止位置：attempt 3 为 `authorized_pending_execution`。下一项允许执行的工作是提交并
  同步本检查点，准备并审计最小差异执行器；准备门禁失败不得创建正式目录，正式运行失败则
  必须恢复、封存并停止，不能自动重试或进入 M6-B。

### 2026-10-27：M6-A.3.1 attempt 3 在维护窗口内失败并完成恢复

- 准备与输入：授权检查点 HEAD 为
  `8d371da6e37732f6ddcd69c333c54aff52db81c1`；executor SHA-256 为
  `9b9c85846239fd2454e917470e6858981d872919c39d6fa6ad0a30d14b9ed73b`，watchdog SHA-256 为
  `8098c8b2ee911552784e410a1f377d82a3e60e7fb9ad640aeeb1fc5cf8e597cf`。与 attempt 2 的差异只含
  attempt/路径/HEAD/脚本 identity 和 `.worker_name` 修正；`bash -n`、ShellCheck、pre-window
  failure injection 及完整只读漂移门禁通过。
- 执行经过：源码构建和 identity/resource/environment 门禁通过，记录完整 baseline 后停止
  精确 Python Worker，`python_worker_stop_gate` 通过。`rust_worker_start_gate` 启动 Rust
  Worker 后，第一次 API probe 尚未监听；命令替换内的 curl 返回 7，在顶层 `set -e` 下直接
  触发 EXIT trap，未进入设计的 120 次重试。Rust 日志稍后证明 Worker 已使用持久 ID 5 启动、
  上报两张 GPU，并保持 maintenance-required/runtime-disabled。
- 结果与范围：正式结果为 `FAIL/maintenance_window_or_cleanup_failed`，记录
  `baseline_ready=1`、`window_entered=1`、`smoke_passed=0`、`cleanup_ok=1`。maintenance 从未
  disable，`model_id`、ModelInstance、ModelFile 和 assigned port 均为 none；没有 Docker/vLLM
  workload、GPU compute 或推理。
- 恢复与独立审计：cleanup 对精确 Rust PID 先 SIGINT、后 SIGTERM，随后恢复原
  `gpustack-worker`。Server、Python Worker health/metrics 均为 200；Worker ID 5 仍处于
  maintenance；目标 Model/ModelFile、Rust 进程、`20150/20151`、`40000-40063` 监听和 GPU0
  负载均为 0。前后全局资源 ID、Docker ID 集和模型 manifest 一致，远端工作树干净。
- 证据：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-3`
  为 0700，60 个文件均为 0600，59 条 manifest 全部通过，秘密扫描为 0；清单文件 SHA-256
  为 `e7d3d05710ebb5755a363658daf7580fcbc0701065ee49ca9b33873347a6e180`。watchdog 补充目录
  `m6a3-managed-file-smoke-attempt-3-watchdog` 为 0700，7 个文件均为 0600，6 条 manifest 通过，
  秘密扫描为 0；清单文件 SHA-256 为
  `a51a74bbb9e3a1fec29bb512d0527dd3813f0133f6f4446f8b6e111ba879c324`。
- 判定与停止位置：attempt 3 是有效 `failed_during_window`，不能在原目录续跑；M6-A.3.1
  仍未通过。下一项只允许讨论并冻结 attempt 4 新证据路径和 readiness probe 的 `errexit`
  窄修正，随后重新执行静态/failure-injection/只读门禁并取得新的维护窗口授权。不得自动
  重试，M6-B 仍未开始。

### 2026-10-27：M6-A.3.1 attempt 4 已冻结并获维护窗口授权

- 用户决定：用户询问近期问题偏多的原因，并明确要求后续同类问题由执行者直接解决、必要时
  再询问；同时对上一条 attempt 4 精确授权文本回复“没问题”。D136–D138 因此冻结 attempt 4
  和有限自主处理边界。
- 证据路径与唯一修正：正式目录固定为尚未创建的
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-4`。
  executor 从 attempt 3 封存副本派生，只在两个 readiness curl 后增加 `|| true`，并更新
  attempt/路径/HEAD/脚本 identity；其他正式输入、状态链、超时、回滚和排除项不变。
- 增强准备门禁：除 `bash -n`、ShellCheck、最小 diff、pre-window failure injection 和完整
  只读漂移检查外，新增三步 synthetic readiness fixture，必须证明两次 curl 7 后第三次 200
  能在 `set -e` 下继续并成功；同时审计所有等待预期瞬态状态的外部命令。
- 自主处理边界：若后续仍出现纯执行器字段、shell 错误处理、传输或证据封装缺陷，只有在上次
  已封存、恢复审计通过、新证据路径、最小差异和完整门禁同时满足，且不改变产品代码、正式
  输入、资源范围、超时、成功标准或回滚协议时，才可不再逐次询问。任何方法/风险扩大、破坏性
  清理或恢复不确定仍须停止询问。
- 当前停止位置：attempt 4 为 `authorized_pending_execution`。下一项允许执行的工作是提交并
  同步本检查点、准备和审计最小差异执行器，门禁全部通过后执行一次正式维护窗口。

### 2026-10-27：M6-A.3.1 attempt 4 因探针地址错误失败并完成恢复

- 准备与输入：授权检查点 HEAD 为
  `9ec413929492a02d6558b2fe75809f5e59948d5a`；executor SHA-256 为
  `6235fe0b2e84c7e7c29272b1c0695d3fa99293a6a31cb0d4a2fec358c4e40875`，watchdog SHA-256 为
  `6f1dcb6de3dc837eb21add81a03712394f1daacf4e6828f4b88e065fb4328cb7`。与 attempt 3 的行为差异
  仅为两个 readiness curl 增加 `|| true`；静态审计、三步 synthetic retry、真实空端口、
  pre-window failure injection 和完整只读漂移门禁均通过。
- 执行经过：source/build、identity/resource/environment 和 Python Worker stop 门全部通过。
  Rust Worker 使用持久 ID 5 正常启动并监听 `10.213.80.90:20150/20151`，两个 Worker IP 探针
  均返回 200；执行器却仍轮询 `127.0.0.1`，现场该地址返回 000，因此在 120 次重试后以
  `rust_worker_startup_gate_timeout` 进入 cleanup。
- 结果与范围：正式结果为 `FAIL/rust_worker_startup_gate_timeout`，记录
  `baseline_ready=1`、`window_entered=1`、`smoke_passed=0`、`cleanup_ok=1`。maintenance 从未
  disable，Model、ModelInstance、ModelFile、assigned port 均为 none；没有 Docker/vLLM
  workload、GPU compute 或推理。
- 恢复与独立审计：cleanup 精确停止 Rust Worker并恢复原 `gpustack-worker`。Server、Python
  Worker health/metrics 为 200；Worker ID 5 仍处于 maintenance；目标资源、Rust 进程、
  `20150/20151`、`40000-40063` 监听和 GPU0 负载均为 0。前后全局资源 ID、Docker ID 集和
  模型 manifest 一致，远端工作树干净。
- 证据：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-4`
  为 0700，60 个文件均为 0600，manifest 全部通过，秘密扫描为 0；清单文件 SHA-256 为
  `bb259eb9b0162c86f24417f1ae20b28091d6a15d0ac8f0daf4446cc8d7775e5e`。watchdog 补充目录
  `m6a3-managed-file-smoke-attempt-4-watchdog` 为 0700，7 个文件均为 0600，manifest 通过，秘密
  扫描为 0；清单文件 SHA-256 为
  `65495a2d2835f07bf8ff09628035e622ded1e57905e857462538f6f060b601ae`。
- 判定与下一步：attempt 4 是有效 `failed_during_window`，不能在原目录续跑。该问题属于 D138
  允许自主处理的执行器传输地址缺陷；D139 将 attempt 5 固定为新证据目录，并只把两个
  readiness probe 改向冻结 Worker IP `10.213.80.90`。完成最小差异、现场地址、
  failure-injection 和全量漂移门禁后可直接执行 attempt 5；M6-B 仍未开始。

### 2026-10-27：M6-A.3.1 attempt 5 因 Worker 状态断言错误失败并完成恢复

- 准备与输入：检查点 HEAD 为 `5606b09ca8594bead150aa041190d1b6a2947e89`；executor
  SHA-256 为 `f83919bf0409fecf96eb3d67da0ab1c0643a9105a7d94fd63f2496a366484181`，watchdog
  SHA-256 为 `2293a9f73a18a37cec417394062b946c4cab63977e3ff2b7b0605f00ab58d862`。相对
  attempt 4 的行为差异只有两个 readiness URL 改为 Worker IP；地址 fixture、failure
  injection 和全量漂移门禁通过。
- 执行经过：source/build、identity/resource/environment 和 Python Worker stop 门通过；Rust
  Worker 在 `10.213.80.90:20150/20151` 正常监听，探针均为 200。Server 权威响应为
  `state=maintenance`、`maintenance.enabled=true`、`unreachable=false`、`gpu_count=2`；执行器
  却要求 `.state == "ready"`，因此在 120 次循环后以 `rust_worker_startup_gate_timeout` 失败。
- 结果与恢复：正式结果为 `FAIL/rust_worker_startup_gate_timeout`，`baseline_ready=1`、
  `window_entered=1`、`smoke_passed=0`、`cleanup_ok=1`。maintenance 从未 disable，Model、
  ModelInstance、ModelFile 和 assigned port 均为 none；无 Docker/GPU workload 或推理。cleanup
  精确停止 Rust Worker并恢复原 Python Worker；服务、端口、GPU、全局资源、Docker 集和模型
  manifest 的独立审计全部通过。
- 证据：正式目录 `m6a3-managed-file-smoke-attempt-5` 为 0700，62 个文件均为 0600，manifest
  通过、秘密扫描为 0；清单文件 SHA-256 为
  `139821ac350946463a1413da0f455decb8172ab006f02d468004122407ecfb2b`。watchdog 目录为 0700，
  7 个文件均为 0600，manifest 通过、秘密扫描为 0；清单文件 SHA-256 为
  `4ed10c279a5afb982885a64172c3b9805be0b7767e1dae830550c28afead8d60`。
- 整体下游排练：为避免继续逐点暴露执行器假设，随后运行 CLI Managed 17 个测试、Worker
  coordinator 8 个测试和 STARTING no-op 事件测试 1 个，全部通过；另用正式脚本同一组 jq/
  regex/cleanup 表达式演练 STARTING、ModelFile ready、端口和事件链，也全部通过。
- 判定与下一步：attempt 5 是有效 `failed_during_window`。D140 按 D138 冻结 attempt 6 新目录，
  唯一行为修正是 readiness 要求合法的 `state=maintenance`，同时保留 maintenance、reachable
  和双 GPU 收紧条件。全部准备门通过后可直接执行 attempt 6；M6-B 仍未开始。

### 2026-10-27：M6-A.3.1 attempt 6 暴露 Rust CLI cluster context 缺口

- 准备与输入：检查点 HEAD 为 `517255ea8e3fc5ff0d137cea13c4b39149b7d456`；executor
  SHA-256 为 `9f676eddff7343b126acccd0e1da753a7a44470be0deb3a6fff0fb127d9f5ca8`，watchdog
  SHA-256 为 `dfa5557f1bd43d740fd40b68e91675a5fe230430ff9c711c0f629c2f1339ea91`。相对
  attempt 5 的唯一行为修正为合法 maintenance 状态断言；真实状态 fixture、failure injection、
  全量漂移门及此前 26 个下游测试/表达式排练全部通过。
- 执行经过：source/build、identity/resource/environment、Python Worker stop 和
  `rust_worker_start_gate` 首次全部通过，随后 CLI 成功把 Worker ID 5 的 maintenance 关闭。
  不带 `--wait` 的固定 `model deploy` 随即返回退出码 30；Server 报 HTTP 400：手工 GPU 选择
  缺少 cluster context，无法验证 `rust-m3d-r760xa`。执行器在 Model 创建前进入 cleanup。
- 根因：只读 API 核对确认目标 Worker 的 `cluster_id=1`。Python Server 的
  `validate_gpu_ids` 要求手工 `gpu_ids` 同时具有有效 Model `cluster_id`；Rust
  `gpustack_client::ModelCreate` 不包含该字段，CLI deploy 也未先解析目标 Worker 集群，因此
  请求必然缺少上下文。这是产品代码/Server 契约兼容缺口，不是执行器断言。
- 结果与恢复：正式结果为 `FAIL/maintenance_window_or_cleanup_failed`，`cleanup_ok=1`；
  `model_id`、ModelInstance、ModelFile 和 assigned port 均为 none，无 Docker/GPU workload 或
  推理。cleanup 恢复 maintenance、停止 Rust Worker并恢复原 Python Worker；Server/Worker、
  端口、GPU、前后资源 ID、Docker 集和模型 manifest 的独立审计全部通过。
- 证据：正式目录 `m6a3-managed-file-smoke-attempt-6` 为 0700，66 个文件均为 0600，manifest
  通过、秘密扫描为 0；清单文件 SHA-256 为
  `79c532457a5106eb7e81829c19d81fb682cdb2335fc922467da454d74419bd1d`。watchdog 目录为 0700，
  7 个文件均为 0600，manifest 通过、秘密扫描为 0；清单文件 SHA-256 为
  `5c456c399188bbe2a6097831a03ef7f19eb860f7e390c12862edb54cda418b94`。
- 当前停止位置：attempt 6 是有效 `failed_during_window`。下一步必须先共同决定是否修改 Rust
  产品代码：从唯一目标 Worker 推导非空 `cluster_id` 并写入 `ModelCreate`，配套请求体和失败
  回归测试。未经接受不得冻结 attempt 7，M6-B 仍未开始。

### 2026-10-27：D141 cluster context 修复完成离线验收

- 决策与范围：用户授权后续同类范围明确问题由执行者直接解决，仅在风险或设计选择必要时
  询问。D141 保持现有 `model deploy` 参数和固定 Worker/GPU/Runner 输入不变：deploy 先按既有
  规则唯一解析 Worker，读取 typed `cluster_id` 并写入 `ModelCreate`；无集群归属时在 Model
  POST 前 fail closed。
- 实现：`WorkerPublic` 增加可选 `cluster_id`，`ModelCreate` 显式携带可选字段；CLI 从唯一
  Worker 填入非空值。代码仅保留一处解释 Server 集群作用域的关键注释，没有逐行注释或无关
  重构。代码检查点为 `208ac76dfd78beca4c697afd05235e0c974bdd8d`。
- 回归覆盖：部署请求先 GET Worker、随后 POST Model且 body `cluster_id=7`；无集群 Worker 只
  产生一次 GET并返回 typed control-plane error；既有 `--wait` 状态转换测试适配新请求顺序。
  固定 Server fixture 继续验证 `cluster_id=null` 的兼容反序列化。
- 证据：90 上 `cargo fmt --all -- --check` 通过；Server v2.2.3 契约测试 6/6、Managed CLI
  测试 18/18 通过；`cargo clippy --locked --offline -p gpustack-client -p gpustack-cli
  --all-targets -- -D warnings` 通过；`cargo test --locked --offline --workspace` 共 150 个测试
  全部通过。测试未连接网络、数据库、Docker 或真实 GPU。
- 当前停止位置：产品修复已接受并同步到本地、origin 和 90；真实服务状态未因该修复改变。
  下一步允许冻结并完成 attempt 7 的无中断准备门。停止 Python Worker、切换 maintenance 和
  执行真实 smoke 仍需单独明确授权；M6-B 尚未开始。

### 2026-10-27：M6-A.3.1 attempt 7 无中断准备门通过

- 冻结输入：D142 固定 attempt 7 正式/Watchdog 新目录，继续使用 Server v2.2.3、Worker
  ID 5/name/UUID、Qwen2.5 固定 revision、Runner digest、GPU0、`20150/20151`、
  `40000-40063`、60 秒 STARTING hold 和 900 秒 watchdog。产品修复检查点为
  `208ac76dfd78beca4c697afd05235e0c974bdd8d`。
- 只读预检：`m6a3-formal-attempt-7` 在治理 HEAD
  `092e1e20afe04d42ca97e85318d867e5a3dd09ae` 上 `PASS`；45 个文件、目录 0700、文件全 0600、
  秘密扫描为 0，清单通过且清单文件 SHA-256 为
  `f018b990caf44cf2201101d6468aebb2421e2b978eae69d170b70b3c58ef622a`。结果明确记录未改变
  maintenance、未停止 Python Worker、未启动 Rust Worker、未写 Server 资源且无 Docker/GPU
  workload。
- 执行器审计：相对 attempt 6 的差异只有 attempt/路径/identity 常量、接受 D141 的代码
  checkpoint，以及将 cleanup 维护提示从错误的 attempt 2 改为 attempt 7；两个脚本均通过
  `bash -n`。当前 90 环境没有 ShellCheck 命令，因此未声称重新运行；逻辑主体直接继承此前
  已通过 ShellCheck 的 attempt 6 封存脚本，并以最小 diff、故障注入和完整外部门补强。
- 故障注入：非正式目录 `m6a3-managed-file-smoke-attempt-7-pre-window-injection` 故意使用错误
  HEAD，在 source gate 以 `STOP/source_head_mismatch` 退出；`baseline_ready=0`、
  `window_entered=0`、`cleanup_ok=1`，19 个文件 manifest 通过、秘密扫描为 0，Server/Python
  Worker健康且 Rust 进程为 0。清单文件 SHA-256 为
  `e9a1090a3fedd3c5dc27d04a8e97ed656d77182d56ad069d5ca3d724bdd3f398`。
- 外部漂移门：源码/origin/90、binary、预检 manifest、模型 manifest、脚本 identity、Worker
  cluster/maintenance、目标资源、Docker/Python Worker、GPU0、Rust 进程和端口全部通过；正式
  attempt 7/Watchdog 目录保持不存在。
- 当前停止位置：提交本 D142 检查点后，只允许把 executor 的 EXPECTED_HEAD 绑定到最终治理
  commit并重跑无写外部门。通过后状态为 `ready_for_maintenance_authorization`；未获明确授权前
  不得停止 Python Worker、切 maintenance、创建 Server 资源或启动 Rust Worker，M6-B 未开始。

### 2026-10-27：M6-A.3.1 attempt 7 在 Worker READY 门失败并完整恢复

- 执行与结果：用户明确授权固定维护范围后，attempt 7 在治理 HEAD
  `c0bd0d4081964fe0c28d2c941d747edc51080cd3` 进入真实维护窗口。D141 修复生效：`model deploy`
  成功创建 `cluster_id=1` 的 Model 10，Server 创建 ModelInstance 25。
- 失败证据：实例在 300 秒内始终为 `pending`、`worker_id=null`，权威消息为
  `Matched 0/1 workers by READY status`。maintenance 已由 operator 关闭且 heartbeat、Worker
  API、metrics 和双 GPU 均正常，但 Rust Worker 状态为 `not_ready`，消息仍为
  `Rust M6-A managed file preparation; runtime execution disabled`。因此没有进入 `SCHEDULED`，
  也没有创建 ModelFile 或分配端口。
- 根因：`ResourceApiProfile::status_report` 对 `managed-file-only` 的每次健康报告仍写入受限模式
  消息；固定 Server 对 `NOT_READY` 且非空 `state_message` 直接保留显式故障状态，不再计算为
  `READY`。这与已冻结的 operator-controlled 调度语义不一致，D143 将修复范围限制为该字段。
- 恢复：正式结果为 `FAIL/state_chain_did_not_reach_starting`、`cleanup_ok=1`。Model 10、Instance
  25 已删除，Worker ID 5 恢复 maintenance，精确 Python Worker 容器已恢复；Server、Worker
  health/metrics 均为 200，Rust 进程和 `20150/20151`、`40000-40063` 监听为零，两张 GPU 的
  显存、利用率和 compute process 为零。前后全局资源 ID、Docker ID、GPU 摘要与固定模型
  manifest 逐项一致。
- 证据：正式目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-7`
  为 0700，72 个文件均为 0600；71 个 manifest 条目全部通过，秘密扫描为 0；
  `manifest.sha256` 文件 SHA-256 为
  `81e34f127d853eaf8bd5e326e071156a3fd539f958c08ab82371aecc48d0a43e`。executor 与 supervisor
  SHA-256 分别为 `72380a70317ea9df9a32f242e97f7170f5999018a30d47f30ff84b75ab9c22e7` 和
  `4624e61958f62d7fac4a0265f1cfc8c09148b07a867dcf690ee639162db6b5fb`。
- 当前停止位置：attempt 7 已封存为有效 `failed_during_window`，M6-B 未开始。下一项允许工作是
  按 D143 实现健康状态上报修复并完成离线回归；新的真实维护窗口需另行冻结和授权。

### 2026-10-27：D143 Managed 健康状态修复完成离线验收

- 实现：代码检查点 `85d6b8f478de2b096e35d0c22ddaa8ab1768fa39` 为不同受限模式分离
  周期状态消息。`resource-api-only` 继续发送 maintenance 原因，防止误调度；
  `managed-file-only` 的健康报告发送 `state_message=null`，使 operator 解除 maintenance 后
  Server 可按 heartbeat、可达性和资源状态计算 READY。注册请求仍以 maintenance 和原消息启动。
- 回归：两种模式分别断言非空和空状态消息，防止安全模式互相污染。90 上目标测试 6/6；
  `cargo test --locked --offline --workspace` 共 150 个测试通过；rustfmt 与
  `cargo clippy --locked --offline --workspace --all-targets -- -D warnings` 通过。测试没有连接
  Server、Docker 或 GPU，也没有改变现有服务。
- 漂移判断：该修复实现 `architecture.md` 已冻结的 operator-controlled 调度语义，不改变 CLI、
  Server schema、协调器、状态链、固定输入、Docker/runtime 边界或成功标准。
- 当前停止位置：D143 已同步本地、origin 和 90。D144 冻结 attempt 8 的全新证据路径并只允许
  无中断准备门；真实维护窗口仍需单独明确授权，M6-B 未开始。

### 2026-10-27：M6-A.3.1 attempt 8 无中断准备门通过

- 冻结输入：D144 保持 Server v2.2.3、Worker ID 5、Qwen2.5 revision、Runner digest、GPU0、
  端口、状态链、300 秒到 STARTING、60 秒 hold、900 秒 watchdog 和 cleanup 不变；接受代码为
  `85d6b8f478de2b096e35d0c22ddaa8ab1768fa39`。
- 只读预检：`m6a3-formal-attempt-8` 在治理 HEAD
  `af9ea236ddd949ca6718beed7f023718b336f8a5` 上 `PASS`；目录 0700、45 个文件全 0600、秘密
  扫描为 0、manifest 全通过，清单文件 SHA-256 为
  `2acf1400baea3948a5e823ea91100c8728431d500794cd7822053f104b5ff777`。
- 故障注入：`m6a3-managed-file-smoke-attempt-8-pre-window-injection` 使用错误 HEAD，在正式
  baseline 和窗口前以 `STOP/source_head_mismatch` 停止；`baseline_ready=0`、
  `window_entered=0`、`cleanup_ok=1`，19 个文件全 0600、秘密扫描为 0、manifest 全通过，
  清单文件 SHA-256 为 `e94f833beceb96bf3ba881cc4619803d31ba9cf42dc35650f07b8679fa732262`。
- 外部状态：Server/Python Worker health 和 metrics 为 200，Worker ID 5 仍为 maintenance，目标
  Model/ModelFile 为 0，Rust 进程与受管端口为空，两张 GPU 的显存/利用率为 0；正式和 watchdog
  目录保持不存在。准备未执行任何运行态写操作。
- 当前停止位置：提交本检查点后只允许把 executor 的 EXPECTED_HEAD 绑定到最终治理提交并重跑
  无写外部门；通过后为 `ready_for_maintenance_authorization`。真实 attempt 8 和 M6-B 均未开始。

### 2026-10-27：M6-A.3.1 attempt 8 暴露资源保留字段缺口并完整恢复

- 执行与结果：用户明确授权固定维护窗口后，attempt 8 在冻结 HEAD
  `57a47a92cb425152ec455e768fc69c71f723ea86` 通过源码、身份、服务、Python Worker stop、Rust
  Worker 启动和 maintenance disable 门。D143 修复生效，Worker 进入 READY；CLI 创建 Model
  11 和 ModelInstance 26。
- 失败证据：实例在 300 秒观察期内保持 `pending`、`worker_id=null`，权威消息为
  `Failed to find candidate: 'NoneType' object has no attribute 'vram'`。没有 ModelFile、分配端口、
  Docker/vLLM container、GPU workload 或推理。
- 根因：Rust 的注册与周期状态请求发送 `system_reserved=null`；固定 Server 的资源计算直接读取
  `worker.system_reserved.vram`。Python Worker 对未配置值发送 `{ram: 0, vram: 0}`，因此该差异是
  Rust wire contract 缺口，不是调度时序或执行器问题。
- 恢复：正式结果为 `FAIL/state_chain_did_not_reach_starting`，`baseline_ready=1`、
  `window_entered=1`、`cleanup_ok=1`。Model 11/Instance 26 已删除，Worker ID 5 恢复 maintenance，
  精确 Python Worker 容器、health/metrics 恢复；Rust 进程和受管端口为 0，GPU0 为 0 MiB/0%，
  前后全局资源 ID、Docker ID、GPU 摘要和模型 manifest 一致，仓库仍处于冻结提交且干净。
- 证据安全：正式目录原 seal 后发现 `before/after-python-worker-inspect.json` 的容器环境包含
  `GPUSTACK_TOKEN`。文件始终为 0600 且从未进入 Git/聊天；立即只把该字段替换为 `<redacted>`，
  `security-redaction.txt` 记录原 manifest SHA-256
  `ab6865984cf59718295f1c353870e5c18303642162ecd3b016b1a9838f4b7eef`、原/新快照哈希和
  `experiment_result_changed=false`。重新封存后目录含 73 个文件、72 个 manifest 条目，校验
  通过，清单文件 SHA-256 为
  `efdf1c89566cf9936bdb8bf066b66c262367e04065e2246b9eb487ea4a7bb735`。
- 当前停止位置：attempt 8 保留为有效 `failed_during_window`，M6-A.3.1 未通过，M6-B 未开始。

### 2026-10-27：D145 Worker 资源保留字段修复完成离线验收

- 实现：代码检查点 `8b06832440f222600721171f8d1ede7e9bab4004` 在 client wire types 中
  新增 `SystemReserved { ram, vram }`，并让 control-plane-only、resource-api-only 和
  managed-file-only 的注册请求与周期状态都明确发送默认双零对象。没有新增配置、CLI 参数、
  runtime 行为或 Server schema。
- 回归：resource-api-only 与 managed-file-only 测试均断言注册和状态上报为双零对象；既有 frozen
  Server fixture、认证、状态消息和协调器测试继续通过。90 上
  `cargo test --workspace --all-targets --locked --offline` 为 153/153；
  `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings`、rustfmt、
  workspace build 和 `git diff --check` 均通过。
- 漂移判断：D145 只恢复 Python Worker 已实现的固定 v2.2.3 请求语义，不改变正式模型、Worker、
  GPU、Runner、端口、状态链、超时、清理或成功标准；结论为 `aligned`。
- 当前停止位置：代码已同步本地、origin 和 90，工作树仅保留用户已有的本地 `output/`。下一项
  允许工作是冻结 attempt 9 新路径并准备经过预落盘 secret 过滤的 executor；准备门通过后仍需
  新的明确维护窗口授权。

### 2026-10-27：M6-A.3.1 attempt 9 无中断准备方法冻结

- 决策：D146 固定 `m6a3-formal-attempt-9`、`m6a3-managed-file-smoke-attempt-9`、对应
  `-pre-window-injection` 和 `-watchdog` 四个尚未占用的路径。固定输入、状态链、时间限制、通过
  标准和 cleanup 均继承 attempt 8。
- 唯一安全修订：预检、正式执行、cleanup 和 final check 的每份 `docker inspect` JSON 都必须
  在写盘前通过 `jq` 删除整个 `Config.Env`；未过滤环境不得落盘。其余恢复所需字段必须保留。
- 授权边界：当前只允许脚本准备、静态与最小差异审计、冻结构建、故障注入、真实只读查询和
  Git 外受保护证据写入。不得停止 Python Worker、改变 maintenance、写 Server 测试资源、启动
  Rust Worker、创建 Docker/GPU workload 或推理。
- 当前停止位置：执行 attempt 9 无中断准备门；全部通过后状态只能成为
  `ready_for_maintenance_authorization`，不能自动执行真实维护窗口。

### 2026-10-27：M6-A.3.1 attempt 9 无中断准备门通过

- 静态审计：attempt 9 的 preflight、executor 和 supervisor 均从 attempt 8 封存副本派生并
  通过 `bash -n`。除 attempt/路径/HEAD/D145/前序证据 identity 外，行为差异仅为三份完整
  `docker inspect` 在写盘前执行 `jq 'map(del(.Config.Env))'`；状态链、超时和 cleanup 未改变。
- 只读预检：`m6a3-formal-attempt-9` 在治理 HEAD
  `6a61fd2966f34481e8f8bdddf3db77a2c410681c` 上 `PASS`；目录 0700、45 个文件全 0600、
  秘密扫描为 0、44 个 manifest 条目全部通过，清单文件 SHA-256 为
  `c1e7fcbdea35d2a142c8259a5abc724e8a35e414fcfaf0ac8907d270d6d2fb1d`。
- 故障注入：`m6a3-managed-file-smoke-attempt-9-pre-window-injection` 故意使用错误 HEAD，按
  `STOP/source_head_mismatch`、`baseline_ready=0`、`window_entered=0`、`cleanup_ok=1`
  封存；18 个 manifest 条目通过、秘密扫描为 0、`Config.Env` 路径数为 0，清单文件 SHA-256
  为 `1cdb433cf52ba63d681b79ed39cffabd371511eefbb6deb79d1095fa86bf4743`。
- 外部状态：Server/Python Worker health/metrics 为 200，原 Python Worker容器仍运行，Rust
  进程和受管端口为 0，GPU0 为 0 MiB/0%；正式和 watchdog 路径未创建，准备没有运行态写入。
- 当前停止位置：提交本检查点后，把最终 executor/supervisor 绑定到新的治理 HEAD并重跑完整
  无写漂移门。通过后状态为 `ready_for_maintenance_authorization`；真实 attempt 9 和 M6-B
  均未开始。

### 2026-10-27：M6-A.3.1 attempt 9 暴露单 Worker 分布式占位契约并完整恢复

- 执行与结果：用户明确授权后，attempt 9 在冻结 HEAD
  `482e2fa2747021eb3afbcbb355bb39d540771c20` 进入维护窗口。D145 修复生效：Rust Worker
  通过 READY 门，CLI 创建 Model 12，Server 将 ModelInstance 27 调度到 Worker ID 5，并计算
  GPU0 的 40 GiB 显存声明。
- 失败证据：Server 返回的单 Worker 实例包含默认
  `distributed_servers={mode:"initialize_later", download_model_files:true,
  subordinate_workers:null}`。Rust 协调器把任何非空 `distributed_servers` 都视为跨 Worker
  推理，因而对 `scheduled` 实例记录 `outcome=noop`，实例未进入 `initializing`，也没有创建
  ModelFile 或分配端口。300 秒后结果为 `FAIL/state_chain_did_not_reach_starting`。
- 恢复：`baseline_ready=1`、`window_entered=1`、`cleanup_ok=1`。Model 12/Instance 27 已删除，
  Worker ID 5 恢复 maintenance，精确 Python Worker 容器和 health/metrics 恢复；Rust 进程、
  `20150/20151` 与 `40000-40063` 监听为 0，GPU0 为 0 MiB 且没有 compute process。前后全局
  资源 ID 与 Docker ID 一致，仓库回到冻结提交且干净；没有 Docker/GPU workload 或推理。
- 证据：正式目录 `m6a3-managed-file-smoke-attempt-9` 为 0700，73 个文件全为 0600，72 个
  manifest 条目通过、秘密扫描为 0，清单文件 SHA-256 为
  `b582cea03604a926b480e6714c400564345feb13ed86e8aafd9152929683f5de`。watchdog 目录为
  0700，7 个文件全为 0600，6 个 manifest 条目通过、秘密扫描为 0，清单文件 SHA-256 为
  `1de3bd24e1cada13fbf2978c21cdb43b230cb3a7f32b6f2972a1a10652eb06a0`。本轮所有完整
  `docker inspect` 证据在落盘前删除了 `Config.Env`。

### 2026-10-27：D147 单 Worker 分布式占位契约修复完成离线验收

- 实现：代码检查点 `50bc840b5b00cdedbde4cd3e32676054f1cc91e6` 将 R1 Managed 范围判断
  与 Python `ModelInstance.spans_workers` 语义对齐：`distributed_servers` 为 null，或
  `subordinate_workers` 为 null/空数组时仍是单 Worker；只有非空从属 Worker 列表属于当前
  不支持的跨 Worker 推理。非对象或非数组的畸形结构继续保守拒绝。
- 回归：新增真实 Server 默认占位对象可初始化、非空从属 Worker 必须 no-op、客户端 update
  校验与协调器一致三组覆盖。90 上 `cargo test --workspace --all-targets --locked --offline`
  共 156 个测试通过；all-features/all-targets Clippy `-D warnings`、rustfmt、workspace build 和
  diff check 全部通过。
- 漂移判断：D147 不改变 CLI、模型、Worker、GPU、Runner、端口、超时、状态链、Docker/runtime
  边界或成功标准，只纠正固定 Server 已有的单 Worker wire shape 解释；结论为 `aligned`。
- 当前停止位置：D148 冻结 attempt 10 的新预检、故障注入、正式和 watchdog 路径。允许先完成
  无中断准备门；真实维护窗口仍须新的明确授权，M6-B 未开始。

### 2026-10-27：M6-A.3.1 attempt 10 无中断准备门通过

- 静态审计：preflight、executor 与 supervisor 从 attempt 9 封存副本派生；除 attempt/路径、
  HEAD、接受 D147 的代码检查点、前序证据 identity 和恢复提示外无行为变化。三个完整
  `docker inspect` 写盘点继续先删除 `Config.Env`，脚本均通过 `bash -n`。
- 只读预检：`m6a3-formal-attempt-10` 在治理 HEAD
  `e6ca2f074e087a8f2d90ba1d7131b6cbf7e5cf60` 上 `PASS`；目录 0700、45 个文件全 0600、
  44 个 manifest 条目全部通过、秘密扫描为 0，清单文件 SHA-256 为
  `9cd18a8398d0e4b940456df7b2b4405a11020149d1c8a065ce6840f3015bfde7`。
- 故障注入：`m6a3-managed-file-smoke-attempt-10-pre-window-injection` 使用全零错误 HEAD，按
  `STOP/source_head_mismatch`、`baseline_ready=0`、`window_entered=0`、`cleanup_ok=1`
  封存；目录 0700、19 个文件全 0600、18 个 manifest 条目通过、秘密扫描为 0，唯一容器
  inspect 快照中 `Config.Env` 路径数为 0，清单文件 SHA-256 为
  `cc7403475b5646bf09f30b01ce0647df09559a4f5c85434f13bb61ad26f89086`。
- 外部状态：Server/Python Worker health/metrics 为 200，Python Worker 容器仍运行；Rust 进程、
  `20150/20151` 和受管端口为空，GPU0 为 0 MiB，正式和 watchdog 证据路径未创建。
- 当前停止位置：提交本准备记录后，只允许把最终 executor/supervisor 绑定到新的治理 HEAD并
  重跑完整无写漂移门。通过后状态为 `ready_for_maintenance_authorization`；不得自动进入真实
  attempt 10 或 M6-B。

### 2026-10-27：M6-A.3.1 attempt 10 核心 smoke 通过但正式清理失败

- 执行：用户明确授权后，在治理 HEAD `6ca290153af6ebd20fcc76406531ad0bf5e2c01a`
  启动冻结 executor/watchdog。Python Worker按约停止，Rust Worker以持久 Worker ID 5 启动并
  解除 maintenance；CLI 创建 Model 13，Server 创建 Instance 28 并调度到 GPU0、端口 40000。
- 核心证据：D147 生效，Rust 接受 `subordinate_workers=null` 的单 Worker默认对象。实例完成
  `scheduled -> initializing/downloading -> ready -> starting`；ModelFile 7 解析固定本地目录，
  `size=999604126`、`download_progress=100`、单元素规范路径正确。`starting` 连续保持超过
  60 秒，assigned port 没有 listener，Docker 容器 ID 集未变化，GPU0 始终 0 MiB/无 compute，
  没有 runtime、推理、`RUNNING` 或 `ERROR`。核心门记录 `smoke_passed=1`。
- 正式判定：正式结果保持 `FAIL/maintenance_window_or_cleanup_failed`，不是 PASS。固定 Server
  返回 `cleanup_on_delete=null`，执行器却只接受 false；状态链循环因此等待满 300 秒，且超时
  后只检查 `reached_starting` 和非空文件 ID，错误进入保持期。cleanup 同一错误又拒绝删除
  ModelFile 7，最终 `cleanup_ok=0`。
- 恢复：统一 cleanup 已删除 Model 13/Instance 28、恢复 Worker maintenance、停止 Rust Worker
  并启动原 Python Worker。独立事后审计确认 ModelFile 7 不属于基线且严格匹配 Worker ID 5、
  `local_path` source 和固定路径后，只删除该 Server 元数据。最终全局资源 ID、Docker ID 与实验
  前逐项一致；Server/Python Worker health/metrics 为 200，Rust 进程和 `20150/20151`、
  `40000-40063` listener 为 0，GPU0 为 0 MiB/无 compute，模型 manifest 保持通过。
- 证据：正式目录 `m6a3-managed-file-smoke-attempt-10` 为 0700，84 个文件全 0600，83 个
  manifest 条目通过、秘密扫描为 0，清单文件 SHA-256 为
  `fb4c1a8412498e9dbd31484f542d64605293101c2f52b6a92aa7404a091c9c76`。watchdog 目录 7 个
  文件/6 个条目，清单文件 SHA-256 为
  `44671e99ac99883e8b8489d52a89be2f0f92a6b20ed75b1213a5512c2e912ad8`。第一次事后审计因
  jq 作用域错误在变更前 `STOP`，清单 SHA-256 为
  `994e28b445209ac854287b8dcc78862bdd4bd7ff271deaff0db8d978472e48b2`；第二次审计完成精确
  清理和恢复，清单 SHA-256 为
  `f8b95648a83601d3b34dee451e8e752f5c5b9b40a8df3cd966bb6fb71e190466`。全部证据秘密扫描为
  0；两份完整容器快照落盘前删除 `Config.Env`。
- 解释与修订：Python schema 默认该字段为 null，Python Worker只在值为 truthy 时删除本地
  文件；Rust 原样保留 null 与原实现一致。因此无需修改产品代码。D149-D150 显式接受 null/
  false 两种保留语义，并要求 300 秒循环以完整复合条件硬失败。
- 当前停止位置：attempt 10 为有效 `failed_during_cleanup_harness`，不得提升为正式通过。
  attempt 11 的协议修订已冻结，可执行新路径的静态审计、预窗口故障注入、冻结构建和只读
  漂移门；新的维护窗口仍须用户另行明确授权，M6-B 尚未开始。

### 2026-10-27：M6-A.3.1 attempt 11 无中断准备门通过

- 方法与范围：从 attempt 10 封存脚本派生，只更新 attempt/路径/HEAD/前序证据 identity，并按
  D149-D150 把 ready/cleanup 的 `cleanup_on_delete` 谓词修为 null 或 false、增加独立
  `state_chain_complete` 标志及超时后硬断言。Server、Worker、模型、Runner、GPU0、端口、
  300/60/900 秒界限、无 runtime/Docker/GPU workload 终点和统一恢复顺序均未改变。
- 静态与语义审计：preflight、executor、故障注入和 supervisor 均通过远端 `bash -n`；SHA-256
  分别为 `f1c01e491c9d453b11c983540e75bd9de1435fc78ef1c5d933efb615020a1211`、
  `e8d7cdf5738dac251713bb916be56e115bf1fa47840563afc1071f366b6597d0`、
  `a34bd745ec5a7a43cb4aea7fc094edc04fcc52d3eace293588f0db1853485d12` 和
  `7f837c88650d793e44298743dc675b4affc7a0cf0e1b24f5dd752dfdbd4e8427`。独立 jq 探针只选中
  字段存在且值为 null/false 的对象，拒绝 true 与缺失字段。
- 只读预检：治理 HEAD `538cb499563229546de3ea1ab15acad7239b1440` 上结果为 PASS；
  `m6a3-formal-attempt-11` 为 0700、45 个文件全为 0600、44 个 manifest 条目通过、秘密扫描为
  0，清单文件 SHA-256 为
  `ec5cb0c271285f79e243bc8b8ac699d149dd86f1e50d7f667bb936925b24b1d7`。
- 故障注入：全零错误 HEAD 在窗口前按 `STOP/source_head_mismatch`、`baseline_ready=0`、
  `window_entered=0`、`cleanup_ok=1` 封存；目录 0700、19 个文件全 0600、18 个条目通过、
  秘密扫描为 0，唯一容器快照不含 `Config.Env`，清单文件 SHA-256 为
  `c4d7841b520c4823bf056c56085b0496a2d9d080cb9355d66b489874a182a2f2`。
- 外部状态：目标 Model、Worker 5 的 ModelFile 均为 0；Worker保持 maintenance。Server/Python
  Worker health/metrics 为 200，Python Worker容器运行；精确 Rust 进程、`20150/20151`、
  `40000-40063` listener 为 0，两张 GPU 均 0 MiB/0%。正式与 watchdog 证据路径未创建。
- 当前停止位置：提交本准备记录后，把最终 executor/supervisor 绑定到新的治理 HEAD并执行最后
  一次无写漂移门。通过后才能请求 attempt 11 的独立维护授权；不得自动重跑正式 smoke，M6-B
  尚未开始。

### 2026-10-27：M6-A.3.1 attempt 11 正式通过并完成独立恢复审计

- 执行与身份：用户明确授权后，在治理 HEAD
  `0c1ff02ab4b3d30f4cf1fb71ebdacf9e62c2ed12` 通过最终只读漂移门，再启动冻结的
  executor/supervisor。最终脚本 SHA-256 分别为
  `a4dd35aedce9d3682945fae8aaca57d15d4465a85700833b4fec406a9112640f` 和
  `757c55823ab23f8ce6c1eacf9b9c93614205832e9e63a120de7a9168ea592de0`；总 watchdog
  仍为 900 秒，没有修改固定 Server、Worker、模型、Runner、GPU、端口或超时边界。
- 正向状态链：Python Worker按协议停止后，Rust Worker以持久 Worker ID 5 启动并由 operator
  CLI 解除 maintenance。Model 14/Instance 29 被调度到 GPU0 和端口 40000；同一次权威快照
  证明 Instance=`starting`，ModelFile 8=`ready`、`download_progress=100`、
  `size=999604126`、规范本地路径正确且 `cleanup_on_delete=null`，因此 D149-D150 的
  `state_chain_complete` 复合硬门通过。
- 隔离与恢复：`starting` 保持 60 秒期间 assigned port 无 listener，Docker ID 集不变，GPU0
  无 compute process 且显存为 0，没有 runtime、vLLM、推理、`RUNNING` 或 `ERROR`。统一
  cleanup 删除精确 Model/Instance/ModelFile 元数据、保留宿主机模型文件，恢复 Worker
  maintenance，停止 Rust Worker并启动原 Python Worker。
- 正式判定：runner 返回 `exit_code=0`，正式结果为
  `PASS/managed_file_state_chain_and_cleanup_passed`，其中 `smoke_passed=1`、`cleanup_ok=1`、
  `baseline_ready=1`、`window_entered=1`。正式目录
  `m6a3-managed-file-smoke-attempt-11` 为 0700，87 个文件全为 0600，86 个 manifest 条目
  全部通过、秘密扫描为 0，清单文件 SHA-256 为
  `737d35976a595cadc3f21fdff8e67e603656e43ef5bd95cb6d5ab046f2e64a3a`。watchdog 目录
  7 个文件/6 个条目，清单文件 SHA-256 为
  `768cd133fbf7d026c7d684f36c5dfd3ae165e4c826f05ca96ab9472eebebaa0b`。
- 独立审计：`m6a3-managed-file-smoke-attempt-11-post-run-audit` 独立重验正式和 watchdog
  清单、文件权限、秘密扫描、关键状态谓词、源码与脚本 identity，并重新读取实时 Server、
  Docker、端口、进程和 GPU 状态。结果为 PASS；24 个文件/23 个 manifest 条目，清单文件
  SHA-256 为 `2dffa63f45943d0b630d5f4e1d41a5fcc8988d5a9b7ddcccdaf7c39aae930613`。
  审计时全局资源 ID 和 Docker ID 与基线一致，测试资源为 0，Worker 5 保持 maintenance，
  Server/Python Worker health/metrics 为 200，Rust 进程、`20150/20151` 和受管端口 listener
  为 0，两张 GPU 均为 0 MiB/0%。全部持久化 Docker inspect 快照均不含 `Config.Env`。
- 判定与停止位置：M6-A.3.1 状态更新为 `accepted`。该结论只接受 Managed 文件准备状态链和
  无 runtime 隔离，不证明 Docker/vLLM 启动或推理。M6-B 尚未开始；下一项允许工作是共同
  讨论并冻结 M6-B 的 runtime 启动范围、维护窗口、成功门和回滚设计，不得直接复用本次授权
  创建 Docker/GPU workload 或执行推理。

### 2026-10-28：M6-A.3.1 阶段专用凭据已退役

- 目的与范围：按 D108 的完成条件，只清理 M6-A.3.1 专用凭据，不修改模型、Worker、Docker、
  GPU 或服务。删除前只读确认唯一 API Key ID 19、名称
  `rust-worker-m6a3-operator-20261024`、描述和有效期均与阶段记录一致。
- 结果：Server `DELETE /v2/api-keys/19` 返回 200，原 token 随后访问管理 API 返回 401；
  `lab90` Context 已通过 Rust CLI 删除，`m6a3-operator.token` 文件已删除，最终 Context 列表
  为空。Server 和 Python Worker health 仍为 200，Rust 进程与 listener 仍为 0。
- 证据：`m6a3-credential-retirement-attempt-11` 为 0700，15 个文件全为 0600，14 个 manifest
  条目通过、秘密扫描为 0，清单文件 SHA-256 为
  `a30b8840776ea7b7bbd16c4e894208de81f900a4f096813341dce377ade8f605`。
- 当前停止位置：M6-A.3.1 保持 `accepted` 且不再保留管理凭据。若后续 M6-B 真实 Server
  验证需要 operator 权限，必须在其设计冻结后重新创建新的阶段专用最小权限凭据；不得恢复或
  复用已撤销的 Key 19。

### 2026-10-28：M6-B.0 Managed 单 GPU runtime 设计已冻结

- 目的与结论：在 M6-A.3.1 已接受且阶段凭据已退役的基线上，共同冻结 M6-B 的真实
  Docker/vLLM、推理代理、日志、CLI、清理和验证边界。状态为 `accepted_design`，尚未产生
  M6-B 产品代码或真实 runtime 证据。
- 接口与状态：新增显式 `worker run --managed-runtime` 设计；保持 Server/Scheduler、Server
  Controller 和 Rust Worker既有状态所有权。Rust 只在最新权威 `STARTING` 上创建或接管
  workload，vLLM `/v1/models` 返回期望 served model 后才写 `RUNNING`，失败写有界脱敏
  `ERROR`。所有副作用前都刷新并复核 assignment/spec。
- runtime 与隔离：Lab 和 Managed 复用聚焦的 Docker/ownership/health/logging 原语，但保留
  独立状态机。M6-B 只支持固定 digest Runner、本地路径、单 Worker/单 GPU/TP=1、host
  network 和受管端口；Managed 完整标签 tuple 增加 `component=vllm`，多匹配、缺标签或
  spec 冲突均 fail closed。
- 请求链路：固定正式路径为 CLI/user -> Server `/v1/...` -> Worker `20150` -> 内部健康
  route table -> 本机 vLLM。Worker严格解析实例 header，只代理支持的 OpenAI 路径，删除
  Worker认证和 hop-by-hop headers，支持 buffered/SSE。`model logs` 仍走 Server 日志 API。
- 路线与权限：D152–D159 把后续拆成 M6-B.1～B.5。当前只允许 B.1 共享 runtime/planner
  的离线实现与 fake 测试；不授权真实 Docker socket、Server 写入、maintenance 变化、停止
  Python Worker、GPU workload 或真实推理。B.5 需要新的阶段专用最小权限凭据和独立维护
  窗口授权。
- 当前停止位置：M6-B.0 已完成，下一项允许执行 M6-B.1。离线实现完成后必须运行完整 Rust
  workspace 质量门，并按 D033 在聊天中交付详细实现报告再共同验收。

### 2026-10-28：M6-B.1 共享 runtime 与 Managed planner 离线实现完成

- 代码身份：产品代码检查点为
  `db6550311075c9eac5f0472fccc8029f4348bf09`，固定 Rust 1.99.1，`Cargo.lock`
  SHA-256 为 `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`。
- 共享 runtime：新增不依赖 Lab 或 Managed 状态机的 Docker image/container ownership、
  vLLM `/v1/models` 健康观测和有界脱敏诊断原语。Lab 改为复用这些原语，
  同时保留原有独立状态、存储和编排语义。
- Managed planner：对最新 Model/ModelInstance/Worker/GPU 输入执行 fail-closed 校验，
  只允许单 Worker、单空闲 GPU、TP=1、本地受信模型根、固定 digest image、
  `40000-40063` 分配端口和白名单 backend 参数。输出稳定 `WorkloadSpecV2`、
  spec hash 和完整 Managed ownership labels。
- 恢复语义：容器发现只用 ModelInstance ID 做宽查询，随后对完整 owner、
  deployment、spec hash 和 component tuple 做严格检查。零匹配规划为缺失，唯一
  精确匹配可接管，多匹配、缺标签或值不一致全部失败关闭。
- 验证：90 服务器对精确检查点通过 `cargo fmt --all -- --check`、
  all-features/all-targets Clippy `-D warnings`、`cargo test --workspace --all-targets
  --locked --offline`（162 项通过、0 失败）、locked/offline workspace build 和
  `git diff --check`。原有 Lab 测试保持通过；本阶段没有新增 Cargo 依赖或修改
  `Cargo.lock`。
- 安全边界与停止位置：本次全部为 locked/offline 实现和 fake/mock 测试，未连接
  真实 Docker socket、未写入 Server、未变更 maintenance、未停止 Python Worker、
  未占用 GPU 或执行推理。状态为 `ready_for_review`；用户接受详细报告前不得
  进入 M6-B.2。

### 2026-10-28：M6-B.1 详细实现报告已接受

- 验收：用户已审阅 M6-B.1 的功能、实现数据流、文件映射、测试证据、安全
  边界、局限和后续范围，并明确确认无问题。阶段状态从 `ready_for_review`
  更新为 `accepted`。
- 当前停止位置：下一项允许执行 M6-B.2 Managed reconciler 离线实现。
  该授权只包括 fake Server/runtime/filesystem 与 locked/offline 质量门；仍不允许连接
  真实 Docker socket、写入真实 Server、停止 Python Worker、切换 maintenance、创建
  GPU workload 或执行真实推理。

### 2026-10-28：M6-B.2 Managed reconciler 离线实现完成

- 代码身份：最终源码检查点为
  `6550ac6a419b0253f2219b0b3f0456a3c5e42bf0`；功能提交为 `edbff3e1`，后续
  `6550ac6a` 只补齐同一 Rust 1.99.1 formatter 结果。`Cargo.lock` 未改变，SHA-256
  仍为 `4094d427f9ec18e671e9d639bd9a576c0d620af9da1ca87970da05319dd4857b`。
- 显式模式：CLI 新增与前三种受限模式互斥的 `worker run --managed-runtime`。只有该模式
  构造 Docker runtime 协调器；`control-plane-only`、`resource-api-only` 和
  `managed-file-only` 的既有启动边界不变。
- 权威协调：协调器只处理最新 Server 上仍分配给本 Worker 的 `STARTING` 实例。创建、
  启动、`RUNNING`/`ERROR` 写回和删除清理前均重新 GET 并校验 assignment/spec；重复事件
  接管唯一完整 ownership tuple，不重复创建。只有 `/v1/models` 返回期望 served model
  后才写 `RUNNING`，退出或超时写有界脱敏 `ERROR` 并保留失败容器供诊断。
- 本地状态与清理：新增 0700 Managed 目录、0600 原子 manifest 和当前 session 日志，
  restart count 固定为 0。Managed 在完整生命周期同时持有 Lab/Managed 排他锁。缓存缺口
  不能授权删除；只有精确实例经 Server 反复确认 404 后，才会对完整 owned tuple 执行
  stop/remove，并删除该实例的 cache、日志和 manifest。未知或冲突资源保持 fail closed。
- 验证：90 服务器在干净、已提交且已推送的精确源码 HEAD 上通过
  `cargo fmt --all -- --check`、workspace all-targets/all-features Clippy `-D warnings`、
  `cargo test --workspace --all-targets --locked --offline`（176 项通过、0 失败）、
  locked/offline workspace build 和 `git diff --check`。新增 fake 覆盖健康后晋级、重复事件
  幂等、退出/超时错误、写前状态漂移、缓存缺口、权威删除、输入漂移、私有权限、symlink
  拒绝和 Lab/Managed 排他锁；秘密文件扫描为 0，未新增依赖。
- 安全边界与停止位置：本次没有连接真实 Docker socket、写入真实 Server、切换
  maintenance、停止 Python Worker、创建 GPU workload 或执行推理。完整 crash/restart
  恢复、动态 readiness、推理代理和面向用户的 logs/inference CLI 尚未实现。状态为
  `ready_for_review`；用户接受详细实现报告前不得进入 M6-B.3。

### 2026-10-28：M6-B.2 详细实现报告已接受

- 验收：用户已审阅 M6-B.2 的显式模式、权威刷新、容器创建/接管、健康门、状态写回、
  日志/manifest、精确清理、测试证据、安全边界和未实现范围，并明确确认无问题。阶段状态
  从 `ready_for_review` 更新为 `accepted`。
- 当前停止位置：下一项允许执行 M6-B.3 推理代理与 CLI 离线实现。该授权只覆盖 mock
  transport、fake runtime 和 locked/offline 质量门；仍不允许连接真实 Docker/Server endpoint、
  停止 Python Worker、切换 maintenance、创建 GPU workload 或执行真实推理。

### 2026-10-28：M6-B.3 推理代理、动态 readiness 与 CLI 离线实现完成

- 代码身份：产品代码检查点为
  `c78cfc5652808ee5c16b40b11df411dadc7d6b6d`；固定 Rust 1.99.1，`Cargo.lock`
  SHA-256 为 `90f1aeaa22b4f06dc77e48497f2fe3394ab07ab55441c8ddb3124a67dca07010`。
- 路由与 readiness：新增同进程 Managed route registry。vLLM 健康后先发布实例 ID 到固定
  loopback 端口的路由，再向 Server 写 `RUNNING`；写回失败、实例错误和权威删除都撤销
  路由。只有 `--managed-runtime` 在 Docker 先决条件和首次权威收敛成功后返回 ready，退出
  或基础设施/API 收敛失败时恢复 unready；单个模型进入 `ERROR` 不把整个 Worker 判为
  unready。
- 推理代理：Worker只注册 `/v1/chat/completions` 与 `/v1/completions` 两个受认证 POST
  路径，严格要求唯一正十进制 `X-GPUStack-Model-Instance`，并且只能从内部健康路由表选择
  `127.0.0.1:40000-40063`。转发双向删除认证、实例、hop-by-hop、Host 和 Content-Length
  headers，请求体有 16 MiB 上限；buffered 错误和 SSE 分块均保持上游状态与字节流。
- Client/CLI：`model deploy` 设为 `enable_model_route=true`；新增
  `model logs <model> [--tail] [--follow] [--previous]`、`inference models`、
  `inference chat` 和 `inference completion`。日志先解析唯一 R1 ModelInstance，再只调用
  Server 日志 API；Managed 推理只调用 active Context 的 Server `/v1/...`，流式字节增量
  输出。没有上一 restart 时 `--previous` 明确返回 404，不回退当前日志。
- 验证：90 服务器在干净、已提交且已推送的精确代码 HEAD 上通过 workspace rustfmt、
  all-targets/all-features Clippy `-D warnings`、181 项 locked/offline 测试、全部 doc-tests、
  all-targets/all-features locked/offline build 和工作树清洁门。新增 fake/mock 覆盖动态
  readiness、健康路由发布/撤销、严格 header parser、loopback 目标、敏感/hop-by-hop header
  清理、buffered/SSE 代理、日志唯一实例解析、Server-only OpenAI 路径和流式分块保持。
- 依赖与安全边界：没有引入新的第三方包；CLI 只显式复用 workspace 已有
  `futures-util`，因此 lockfile 仅更新该 workspace package 的直接依赖记录。本阶段没有连接
  真实 Server/Docker endpoint、切换 maintenance、停止 Python Worker、创建容器、占用 GPU
  或执行真实推理。
- 状态与下一步：M6-B.3 为 `ready_for_review`，不是 `accepted`。当前只允许交付并共同审阅
  D033 详细实现报告；接受后才能记录验收检查点并进入 M6-B.4 只读预检与离线故障注入。
  M6-B.4 的通过也不自动授权 M6-B.5 维护窗口或真实 GPU smoke。

### 2026-10-28：M6-B.3 详细实现报告已接受

- 验收：用户已审阅 M6-B.3 的内部健康路由表、动态 readiness、Worker 推理代理、
  Server-only 日志/推理 CLI、测试证据、安全边界、局限和后续范围，并明确确认无问题。
  阶段状态从 `ready_for_review` 更新为 `accepted`。
- 当前停止位置：下一项允许执行 M6-B.4 只读预检与离线故障注入。该阶段只允许读取并核对
  Server/Python Worker、Docker daemon 与固定镜像、Qwen manifest、GPU0、端口、冲突项、
  回滚材料和凭据准备边界，并用 fake 或执行门禁证明失败时不产生真实副作用；不得停止
  Python Worker、切换 maintenance、写入真实 Server、创建或改变真实容器、申请 GPU、
  启动 vLLM 或执行真实推理。M6-B.4 通过后仍需另行讨论和明确授权 M6-B.5 维护窗口。

### 2026-10-28：M6-B.4 只读预检与故障注入协议冻结

- 协议：新增 `m6b4-managed-runtime-preflight.md`，冻结产品代码、Server/Worker、Python
  Worker、Runner、Qwen2.5 revision/manifest、GPU0、端口、只读查询、离线故障注入和证据
  权限要求。前两次交互式查询只用于修正 Docker 29 模板和 Server `/v2` 路径，不是正式
  计分证据。
- 授权边界：正式执行只允许 GET/本机只读检查、locked/offline fake/mock 测试和受保护证据
  写入。不得停止 Python Worker、改变 maintenance、写 Server、改变容器、启动 Rust/vLLM、
  申请 GPU 或推理。当前无活动 operator Context 是 M6-B.5 前置缺口，不是 B.4 失败。
- 当前停止位置：提交并同步本协议后，允许把正式执行器绑定到新的干净治理 HEAD并运行一次
  M6-B.4。结果在详细报告和用户审阅前最多为 `ready_for_review`；不得自动进入 M6-B.5。

### 2026-10-28：M6-B.4 attempt 2 只读预检与故障注入通过

- 固定身份：治理 HEAD `a4e2aa2620efab7bb09667fa3640c8b358a374ad`，产品代码祖先
  `c78cfc5652808ee5c16b40b11df411dadc7d6b6d`，Rust 1.99.1，`Cargo.lock`
  SHA-256 `90f1aeaa22b4f06dc77e48497f2fe3394ab07ab55441c8ddb3124a67dca07010`。
- attempt 判定：attempt 1 的外部前后快照一致，但清单封存后仍追加 `console.log`，独立哈希
  复核失败，已保留并标记 `invalidated`。attempt 2 修正封存顺序后通过；执行器 SHA-256 为
  `3ec89d149c347fab539358b693767fd2a84e28c865ac5d3221ad609fc9dd1e2f`。
- 只读环境门：Server v2.2.3/`1cfcbbf`、Server/Python Worker/metrics HTTP 200；Worker ID 5
  唯一、maintenance、无 ModelInstance 或 `m6b` 模型；精确 Python Worker 容器持续运行；
  固定 Runner digest 存在，零 Rust-owned 容器。Qwen2.5 模型 canonical path、10 个文件、
  999,604,126 bytes、权限、架构、chat template 和完整 manifest 全部通过。
- 隔离与故障门：GPU0 固定 UUID、0 MiB、0% 且无 compute process；`gpustack-rs` 为 0，
  `20150/20151` 和 `40000-40063` 空闲。locked/offline workspace build 和六项精确 fake/mock
  故障注入通过，覆盖配置前置失败、创建前输入漂移、RUNNING 写回前状态漂移、readiness
  超时、缓存缺口不得清理以及受限 loopback 推理代理。全部正式 before/after 快照一致。
- 证据：attempt 2 目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6b4-readonly-preflight-a4e2aa26-attempt-2`
  为 0700，44 个文件全部 0600，秘密扫描与独立清单复核通过；清单 identity 为
  `412f05b238fbdd4fbdb1553cf19cdf9806765e97d614d2f25ac7251a26361346`。
- 状态与停止位置：M6-B.4 为 `ready_for_review`，不是 `accepted`。当前没有活动 operator
  Context；这是 M6-B.5 前置准备项。用户接受详细报告后，也只能讨论并冻结新的短期最小权限
  Key、正式资源名、watchdog、回滚和维护窗口；本次通过不授权停止 Python Worker、改变
  maintenance、创建模型/容器、申请 GPU、启动 vLLM 或推理。

### 2026-10-28：M6-B.4 详细验证报告已接受

- 验收：用户已审阅 M6-B.4 的阶段目的、只读环境门、六项 fake/mock 故障注入、attempt 1
  无效判定、attempt 2 证据链、前后不变性、凭据缺口和结论边界，并明确确认无问题。
  阶段状态从 `ready_for_review` 更新为 `accepted`。
- 当前停止位置：下一项只允许讨论并冻结 M6-B.5 的阶段专用最小权限 API Key/Context、唯一
  测试资源 identity、独立 watchdog、维护窗口进入门、完整状态链、推理/日志验收、删除后
  零残留和 Python Worker 恢复方案。尚未授权创建凭据、停止 Python Worker、改变
  maintenance、创建模型/容器、申请 GPU、启动 vLLM 或推理。

### 2026-10-28：M6-B.5.0 正式 Managed smoke 方案已冻结

- 决策：用户接受三段式 M6-B.5；新增 `m6b5-managed-runtime-smoke.md` 和 D160–D165，冻结
  B.5.0 文档、B.5.1 短期凭据/只读准备、B.5.2 独立维护窗口，以及唯一模型、GPU0、Runner、
  20 分钟 watchdog、完整状态链、Server-only 推理/日志、三轮重复协调、精确清理、Python
  Worker恢复和三条证据链。
- 凭据：API Key name 固定为 `rust-worker-m6b5-operator-20261028`，有效期不超过 24 小时；
  Context 为 `m6b5-operator`，token file 为
  `/home/wujiaxing2/.config/gpustack-rust/m6b5-operator-token`。用户须直接把 token 写入 90，
  不能通过聊天或日志传递；不能复用 Cluster token 或 M6-A Key。
- 当前停止位置：M6-B.5.0 为 `accepted_design`。下一项只允许用户创建阶段 API Key，以及在
  token 安全落盘后执行 B.5.1 的认证 GET、只读漂移门、locked/offline build、执行器/watchdog
  静态审计和无写故障注入。B.5.2 仍须新的明确维护窗口授权。

### 2026-10-28：M6-B.5.1 operator 凭据完成窄幅修订

- 观察：用户按初版方案创建的 API Key ID 20、名称
  `rust-worker-m6b5-operator-20261028`，公开元数据显示 scope 仅为 `management`，有效期为
  7 天。v2.2.3 源码确认 management 与 inference 是两个独立门，因此该 Key 不能支持冻结的
  Server-only 推理，也不满足 24 小时生命周期。
- 决策与授权：用户明确接受 D166。使用 ID 20 创建替代 Key ID 21、名称
  `rust-worker-m6b5-operator-20261028-v2`、scope `management,inference`、有效期精确
  86400 秒；不改变其余正式 workload 或授权边界。
- 结果：ID 21 分别对 `/v2/workers` 和 `/v1/models` 返回 200 后原子替换
  `/home/wujiaxing2/.config/gpustack-rust/m6b5-operator-token`；文件 mode 0600、owner
  `wujiaxing2:wujiaxing2`、link count 1。旧 Key ID 20 删除成功，旧 token 随后返回 401。
  完整 token 未进入聊天、命令参数、日志、Git 或证据。
- 当前停止位置：`operator_credential_ready`。下一项允许在包含 D166 的干净治理 HEAD 上创建
  `m6b5-operator` Context 并执行 B.5.1 只读预检/执行器准备；仍不授权停止 Python Worker、
  改变 maintenance、创建模型/容器、申请 GPU 或推理。

### 2026-10-28：M6-B.5.1 正式门脚本与前两次准备结果

- 正式脚本：提交 `03fe57bd385a61441db26052ed01fb9b39cb3adc` 新增 Git 管理的
  `preflight.sh`、`executor.sh` 和独立 `watchdog.sh`。执行器只使用 `worker run
  --managed-runtime`，覆盖完整状态链、Server-only models/chat stream/chat buffered/completion、
  tail/follow 日志、三轮同进程幂等、精确 Model/ModelFile/owned runtime 清理、精确 Rust PID
  终止和 Python Worker恢复；watchdog 的硬超时为 1200 秒。
- attempt 1：在 HEAD `03fe57bd385a61441db26052ed01fb9b39cb3adc` 的凭据门停止。原因是
  deployed Server v2.2.3 的 API Key 管理路径为 `/v2/api-keys`，脚本误写为 `/v1/api-keys`；
  Context 正确，未进入构建、服务切换或业务写入。修正提交为
  `120096f10fba429385bf9c31d7a2a20c0149b60d`。
- attempt 2：九组准备门实质通过。locked/offline workspace build、六项 Managed failure
  injection、脚本静态审计、错误 HEAD 无写执行器注入、前后资源/容器/GPU/端口/进程不变、
  秘密扫描和独立清单复核均通过；清单文件 SHA-256 为
  `4a9218109ef6792faeeb80a9d03ae9ed48b2344c00a7ad216a3c48d0deac9dd2`。独立复核时发现
  `preflight-result.txt` 的 attempt 字段仍硬编码为 1，因此该证据保留但不作为最终维护窗口
  授权门。
- 当前停止位置：预检 attempt 号已改为强制显式输入。必须在包含本记录的最终干净治理 HEAD
  上执行 attempt 3；只有 attempt 3 完整通过才能请求 B.5.2 的独立维护窗口授权。此前未停止
  Python Worker、未改变 maintenance、未创建模型/容器、未使用 GPU、未执行推理。

### 2026-10-28：M6-B.5.2 attempt 1 正式失败、恢复完成并完成离线修复

- 准备与窗口：attempt 3 在治理 HEAD
  `145437bc852cd30fae318aceeb9dea1e07f84adc` 通过，清单文件 SHA-256 为
  `c570b918ad7a6b1f6cd70ee2f605ee830c5ba96aadb031dd2ab5ba27f22813e5`；用户随后明确授权
  B.5.2 attempt 1。最终漂移门和 watchdog 先行门通过，精确 Python Worker停止后 Rust
  Worker启动、readiness 和两张 A100 上报均通过。
- 正式失败：解除 maintenance 后执行器立即创建 Model 15/Instance 30，Worker 尚未稳定
  READY；CLI 先观察 `analyzing`，随后观察因 READY 排除而回退的 `pending`，因此完整状态链
  门已不可满足。该实例之后仍经 ModelFile 9（999,604,126 字节、ready）进入 `starting`，
  创建唯一容器 `f1d9876d28bd21fbfccffb4e98aba70dd942aa15e9f0c6c52192a9b9989b1b78`，并在真实
  vLLM 健康后进入 `running`；这些只作为失败诊断证据，不能提升为正式通过。
- 清理与恢复：finalizer 恢复 maintenance 并删除 Model，权威 Instance 30 返回 404；固定
  Server 未及时向 Worker送达删除 watch，原 5 分钟 full sync 又超过 180 秒 cleanup 门，
  因而自动 runtime 清理未发生。严格核对唯一完整 ownership tuple 后只对该测试容器执行
  fallback stop/remove，并删除精确 manifest/cache/session log；ModelFile 9 元数据删除，宿主
  模型目录保持完整。Rust PID 未在 SIGINT/SIGTERM 后退出，最终在精确 PID/starttime/exe/hash
  核对且 workload 为零后 SIGKILL。精确 Python Worker恢复。
- 证据判定：watchdog `status=FAIL`、executor exit 137、`recovery_ok=0`，其清单文件 SHA-256
  为 `5b558069d35cf8f6a0530ff151db0a2f690ce00e1dc7bba6f400d3f79266dc3a`。独立事后审计为
  PASS，清单文件 SHA-256 为
  `4d7c71a269cc6d965ff6d649afaf9d38de646ff4da3fbc095ec8b20d435e28a8`，证明 Server
  资源集合、Python Worker、maintenance、Docker、端口、GPU0、本地状态和 Qwen 文件恢复
  基线。它只证明恢复，不改变 attempt 1 的 `failed_during_window` 判定。
- 修复：D167-D170 在代码 HEAD `e66634c523ca37c2e08d50a7656710cb6b16480b` 实现创建前
  ModelInstance watch、稳定 READY 门、七状态有序子序列检查、30 秒权威 full sync、协调中
  shutdown、15 秒顶层退出上界、精确 watchdog 信号升级和显式 attempt 号。新增五个针对性
  回归测试，完整 locked/offline 门通过 188 项测试、格式、all-features/all-targets Clippy、
  build 与 doc-tests；没有停止服务、写 Server、创建容器或使用 GPU。
- 当前停止位置：`retry_fix_offline_verified`。下一步只允许把本记录纳入干净治理 HEAD，三端
  同步后运行全新的 preflight attempt 4 和无写注入。通过后状态最多为
  `ready_for_maintenance_authorization`；正式 attempt 2 必须重新取得明确维护窗口授权。

### 2026-10-28：M6-B.5.1 preflight attempt 4 因旧产品检查点安全停止

- 目的：在不进入维护窗口的前提下，用包含 D167-D170 离线修复和治理记录的干净 HEAD
  执行 retry 2 的首次只读准备门。
- 结果：attempt 4 在第一组 `source_and_scripts` 返回 `STOP`、`failed_step=source_and_scripts`。
  预检脚本仍把 attempt 1 的产品提交
  `c78cfc5652808ee5c16b40b11df411dadc7d6b6d` 作为固定检查点，因此正确拒绝了 D167-D169
  已验证修复对 `rust/crates` 造成的预期差异。
- 证据与影响：证据目录
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6b5-preflight-attempt-4`
  只包含 `console.log` 和 `preflight-result.txt`；无写注入目录未创建。此次停止发生在凭据、
  Server/Worker health、Docker、GPU、端口和进程检查之前，没有改变外部运行状态。
- 判定：attempt 4 保持不可改写的安全 STOP，不作为维护窗口授权证据。D171 将 retry 2 产品
  检查点显式滚动到已完成全部离线门的
  `e66634c523ca37c2e08d50a7656710cb6b16480b`，不改变 workload、验收标准或授权边界。
- 当前停止位置：`retry_checkpoint_rollover_pending_preflight`。下一项只允许在新的干净治理
  HEAD 上使用 `m6b5-preflight-attempt-5` 和
  `m6b5-managed-runtime-smoke-attempt-5-pre-window-injection` 完整重跑只读准备门；正式
  B.5.2 attempt 2 仍须新的明确维护窗口授权。

### 2026-10-28：M6-B.5.1 preflight attempt 5 因远端跟踪缓存安全停止

- 结果：attempt 5 仍在第一组 `source_and_scripts` 返回 `STOP`。逐项复核证明产品提交
  ancestry/diff、Rust 1.99.1、`Cargo.lock` 和干净工作树均正确；唯一失败项是 90 本地
  `origin/codex/rust-worker` 仍为 `2817aef01e93a61f0889399d4511fb41054ac7ef`，而 90 HEAD
  和 GitHub 分支已为 `5912b4eedc1cf983acb0775d1f1004441a84d27c`。
- 处理与影响：在本机 `git ls-remote` 独立确认 GitHub 身份后，90 执行有 60 秒硬超时的
  `git fetch origin codex/rust-worker`，6 秒内把远端跟踪引用同步到 `5912b4ee`。未手工改写
  引用。attempt 5 只产生 `console.log` 和 `preflight-result.txt`，没有进入凭据、服务、Docker、
  GPU、端口或进程检查，无写注入目录未创建。
- 当前停止位置：`remote_tracking_synced_pending_preflight`。下一项只允许使用全新的
  `m6b5-preflight-attempt-6` 和 `m6b5-managed-runtime-smoke-attempt-6-pre-window-injection`
  完整重跑只读准备门；正式 B.5.2 attempt 2 仍须新的明确维护窗口授权。

### 2026-10-28：M6-B.5.1 preflight attempt 6 通过并停在维护授权门

- 执行：治理 HEAD `cdb2a5eeaf7f8f7ffe11fb4eff34ec6259867562` 上的九组准备门全部
  通过，包括固定产品代码/工具链/脚本、双 scope Context、完整外部基线、Runner/模型/GPU0/
  端口/Python Worker身份、locked/offline build、Managed failure injection、脚本静态审计、
  错误 HEAD 无写执行器注入，以及前后不变式和证据封存。
- 证据：`m6b5-preflight-attempt-6` 为 0700，55 个 manifest 条目全通过，文件均为 0600，
  秘密扫描 PASS，清单文件 SHA-256 为
  `bb394acc499cf03169ea74547554a76f44d77e7cd414b7001e7b762abe26dab2`。对应无写注入目录
  同为 0700，9 个 manifest 条目全通过、文件均为 0600、秘密文件数为 0，清单文件 SHA-256
  为 `6a071b9d646687d7a6a0c8c1da9af4982126223193bd12eee579b1b23f78e0cc`；注入以
  `source_head_mismatch`、`window_entered=0`、`cleanup_ok=1` 正确停止。
- 独立复核：Server、Python Worker health/metrics 均为 200；Rust 进程和 Rust Managed 容器为
  0；GPU0/GPU1 均为 0 MiB、0%；90 HEAD、远端跟踪引用与 GitHub 一致且工作树干净。没有
  停止服务、改变 maintenance、写 Server 资源、创建容器或使用 GPU。
- 当前停止位置：`ready_for_maintenance_authorization`。下一项只能由用户明确授权正式
  B.5.2 attempt 2；该授权应限定精确 Python Worker stop/restore、Worker maintenance、唯一
  测试 Model/Instance/File、唯一 Rust vLLM 容器、GPU0、Server-only 推理/日志及精确回滚。

### 2026-10-28：M6-B.5.2 attempt 2 完整状态链后失败并恢复

- 授权与执行：用户对精确维护范围回复“没问题”。最终漂移门通过后，独立 watchdog 启动，
  精确 Python Worker停止，Rust Worker启动并通过 maintenance/双 GPU/稳定 READY 门。Model
  16、Instance 31、ModelFile 10 和唯一容器
  `da22527ac44242607fac1af3d9e0585eb98fa889f45eef431ca65faf361f8a9e` 被创建；完整观察
  `pending -> analyzing -> scheduled -> initializing -> downloading -> starting -> running`。
- 正式失败：Server watch 在 `20:20:27.668637972+08:00` 观察 RUNNING，Rust 的
  `outcome=running` 日志在 `20:20:27.689129112+08:00` 输出。执行器无法用后置日志证明健康
  先于写回，因此在 Server-only 推理前以 `server_running_preceded_runtime_health` fail closed；
  `state_chain_complete=1`，`inference_passed=0`，`idempotency_passed=0`。
- 清理与证据：统一 cleanup 自动恢复 maintenance、删除精确 Model/Instance/File 和 owned
  runtime，Rust 优雅退出，精确 Python Worker恢复。正式结果 `cleanup_ok=1`，watchdog
  `recovery_ok=1`；两处清单 SHA-256 分别为
  `f32b8f86380c438236af1f97fa884b601bef1f7c0aaddbd7aa9b1a5537e01de6` 和
  `fa137abae7d87ef75a3633cbc5aa3f9eab7981ae622c733a4c89df9664a72843`，秘密扫描均为 0。
- 独立审计：首次目录完成业务检查但因零秘密匹配被 `pipefail` 中断，未生成 manifest，保持
  不可采纳。新目录 `m6b5-managed-runtime-post-audit-attempt-2-retry-1` 重新执行相同只读检查并
  PASS，清单 SHA-256 为 `313313059d5174d675461212b86435a8373ba2dbd41c890fc2bba3adaaab8c24`；
  全局资源、Python Worker、maintenance、Docker、GPU、端口、本地状态和模型文件恢复基线。
- 修复与验证：产品提交 `799351f1068adabfded57ab2a6cd2a56749352e6` 在 health Ready 后、
  Server RUNNING update 前捕获 `health_ready_at_unix_nanos`，executor 与 Server watch 时间统一
  转成 Unix 纳秒比较。新增两个回归测试，完整离线门通过 189 项测试、格式、Clippy、build、
  doc-tests 和真实 shell 时间 fixture。
- 当前停止位置：`retry_3_fix_offline_verified`。下一项只允许补齐治理提交并执行全新的
  preflight attempt 7 和无写注入；通过后正式 attempt 3 仍须新的明确维护窗口授权。

### 2026-10-28：M6-B.5.1 preflight attempt 7 通过

- 三端与传输：90 到 GitHub 的精确 fetch 连续两次在 60 秒硬超时内无进展；本机
  `git ls-remote` 已确认 GitHub 为 `aa691887fb9caedff0fb7e7dec9bcdd53a5fe735`，随后通过既有
  增量 bundle 补充路径同步 90 的远端跟踪缓存。bundle 两端 SHA-256 均为
  `9154c497a91d8922ed379e3b0ec2c2eda04b615ee3992bcf79b1c1206a77c370`，工作树未改变。
- 结果：治理 HEAD `aa691887fb9caedff0fb7e7dec9bcdd53a5fe735` 上的九组准备门通过。
  `m6b5-preflight-attempt-7` 为 0700、55 个 manifest 条目全通过、文件均为 0600、秘密扫描
  PASS，清单文件 SHA-256 为
  `8f3eb6a3e5443013daeb11f49f0a03f0b51c42e2fef08bcf98b796665b0b4a58`。
- 无写注入：对应目录为 0700、9 个 manifest 条目全通过、文件均为 0600、秘密文件数为 0，
  清单文件 SHA-256 为 `b7522c7465e5add88f2b069d17e2bb3919158fac441018c1bdac8db6d3f9b8ce`；
  以 `source_head_mismatch`、`window_entered=0`、`cleanup_ok=1` 正确停止。
- 外部不变式：Server、Python Worker health/metrics 均为 200，无 Rust 进程和 Managed 容器，
  GPU0/GPU1 均为 0 MiB、0%；没有停止服务、写 Server、创建容器或使用 GPU。
- 当前停止位置：`ready_for_maintenance_authorization`。下一项只能由用户明确授权正式
  B.5.2 attempt 3，授权范围仍限于精确 Python Worker、maintenance、唯一测试资源、唯一
  owned vLLM 容器、GPU0、Server-only 推理/日志及冻结回滚。

### 2026-10-28：M6-B.5.2 attempt 3 完整状态链后在 Worker 推理认证失败并恢复

- 授权与执行：用户重新明确授权 formal attempt 3。治理 HEAD
  `12a175bae7c19c1f60820a9f848deccb0c5fc7b3`、产品代码
  `799351f1068adabfded57ab2a6cd2a56749352e6` 和 preflight attempt 7 identity
  通过最终漂移门后，独立 watchdog 启动并进入维护窗口。Rust Worker稳定 READY 后创建
  Model 17、Instance 32、ModelFile 11 和唯一容器
  `0178168ca0c6401b22eca4141c59fd0491b5a1bd425ef03e21dbd25ef68f5ba3`；完整观察
  `pending -> analyzing -> scheduled -> initializing -> downloading -> starting -> running`。
- 健康顺序：vLLM Ready 的预写回时间为 Unix 纳秒
  `1793191487433701106`，Server watch 在
  `2026-10-28T20:44:47.468634423+08:00` 观察 RUNNING，修订后的健康先于写回门通过。
  `inference models` 随后经 Server 返回唯一目标模型，证明 operator Key 的 inference scope、
  Server 模型路由和列表接口可用。
- 正式失败：第一条 buffered chat 经 Server 路由后返回
  `401 Unauthorized: Invalid authentication credentials`，因此
  `state_chain_complete=1`、`inference_passed=0`、`idempotency_passed=0`。API Key ID 21
  在失败后仍可分别通过 management API 与不存在模型的 inference 认证门，且有效期未到；
  它不是根因。源码交叉审计确认固定 Server 的 ai-proxy 对自托管模型使用 Cluster
  registration token，而 Python Worker 的 `worker_auth` 同时接受 per-Worker token 与
  registration token；当前 Rust Worker受保护路由只接受 per-Worker token，因而真实推理
  请求在 Rust Worker认证层被确定性拒绝。这是 M6-B.3 对固定 Server 数据面凭据的协议误判。
- 清理与恢复：统一 finalizer 重新启用 maintenance，删除精确 Model/Instance/File 和完整
  owned runtime，Rust Worker优雅退出并恢复精确 Python Worker。正式目录和 watchdog
  均为 FAIL，但 `cleanup_ok=1`；两处清单文件 SHA-256 分别为
  `26e74a828d1f5e2a6d66f278817979b60e1f5375d4835408b9c76a4ca417067d` 和
  `5f0d6794016a545d7ed8d69b5233a4b62b28ff2d6a6833f27a82a907d9dce84d`，秘密扫描为 0。
  executor 的 `set -e` 直接退出没有把 401 写入 `failure_reason`，所以封存结果只留下通用
  `maintenance_window_or_cleanup_failed`；原始 `live.log` 保留真实错误，后续执行器必须为每个
  推理/日志命令显式记录失败门。
- 独立审计：首次 post-audit 的业务检查全部通过，但 PowerShell 到 SSH 的最后一个回车符在
  manifest 生成后追加到 `live.log`，清单复核失败，目录保持不可采纳。全新
  `m6b5-managed-runtime-post-audit-attempt-3-retry-1` 对源码、正式/watchdog 清单、三项服务、
  Server 资源 ID、maintenance、Docker/Python Worker身份、Rust 进程和端口、两张 GPU、
  Qwen manifest 全部复核为 PASS；22 个文件均为 0600，秘密文件数为 0，清单文件 SHA-256
  为 `5105207d88c14a21efd237518f0dc18dc4d9ab78ab8cae4e8ae8773ae8cac708`。
- 判定与停止位置：attempt 3 固定为 `failed_during_window`；恢复成功不能改判业务结果。
  下一项必须先讨论并冻结 Managed 推理路由对 Cluster registration token 的最小兼容修订，
  同时保持 M4 resource API、日志、文件、config 与 `/token-auth` 的 per-Worker-only 边界。
  修订接受前不得实现该认证变化或再次进入维护窗口。

### 2026-10-28：M6-B.5 retry 4 托管推理鉴权兼容已离线实现

- 接受边界：用户接受 D178 的最小修订。只有 Managed chat/completion 路由同时接受
  per-Worker token 与 Cluster registration token；`/token-auth`、文件、日志、config 和其他
  Worker API 不放宽。Bearer 仍优先，两种允许凭据均使用 constant-time 比较。
- 实现：`WorkerApiState` 分离普通 Worker 认证和 Managed inference 认证；
  `--managed-runtime` 在启动早期通过 `CredentialStore` 从受保护 `token_file` 读取 Cluster
  registration token，缺失或不安全文件 fail closed。正式执行器为 models、buffered/streaming
  chat、completion、tail logs、幂等 status 和幂等后 chat 分别记录精确失败原因。
- 安全测试：新增回归断言证明 per-Worker 与 Cluster token 都能通过 Managed inference，
  Cluster token 不能通过普通 Worker 认证，无效 Bearer 不能靠有效 `X-API-Key` 绕过。没有
  把任何 token 值写入代码、Git、日志或文档。
- 验证：产品提交 `e3028fae364b299091b5e93be9b30d15815689ca` 在 90 上通过新增测试
  1/1、Worker lib 131/131、workspace 190/190、all-targets/all-features Clippy `-D warnings`、
  doc-tests、格式、`git diff --check`、executor shell 语法和 locked/offline build。验证没有停止
  Python Worker、改变 maintenance、写 Server 资源、创建容器或占用 GPU。
- 当前停止位置：`retry_4_fix_offline_verified`。下一项只允许在包含 D177-D179 的干净治理
  HEAD 上执行全新的 preflight attempt 8 和无写注入；通过后 formal attempt 4 仍须新的明确
  维护窗口授权。

### 2026-10-28：M6-B.5.1 preflight attempt 8 通过

- 结果：治理 HEAD `9024116de93a1d2ce4a4dcb5b99cf9bc344f96d6` 上九组准备门全部
  通过，包括 retry 4 产品检查点、双 scope Context、完整外部基线、Runner/模型/GPU0/端口/
  Python Worker身份、locked/offline build、Managed failure injection、脚本静态审计、错误
  HEAD 无写执行器注入、前后不变式与秘密扫描。
- 证据：`m6b5-preflight-attempt-8` 为 0700，55 个 manifest 条目全部通过、文件均为 0600，
  清单文件 SHA-256 为
  `9259b2d17c1d9efb8d9e0184ee42fbe8da160ae1a7dc2334a2a4d7422a7add43`。无写注入目录
  同为 0700，9 个条目全部通过、文件均为 0600，清单文件 SHA-256 为
  `a1b214fd5fc384392c3547e1a6a94ace427323589267e79d1e8155d0a92d709d`；结果为
  `STOP/source_head_mismatch`、`window_entered=0`、`cleanup_ok=1`。
- 独立复核：Server、Python Worker health/metrics 均为 200；精确 Python Worker运行；Rust
  进程和 Managed 容器为 0；GPU0/GPU1 均为 0 MiB、0%；90 HEAD、远端跟踪引用和工作树
  一致且干净。没有停止服务、改变 maintenance、写 Server、创建容器或使用 GPU。
- 当前停止位置：`ready_for_maintenance_authorization`。下一项只能由用户明确授权 formal
  attempt 4；授权范围仍限于精确 Python Worker stop/restore、Worker 5 maintenance、唯一
  测试 Model/Instance/File、唯一 Rust-owned vLLM 容器、GPU0、Server-only 推理/日志和冻结
  回滚。attempt 3 的授权不延伸。

### 2026-10-28：M6-B.5.2 attempt 4 在 Rust Worker启动前失败并恢复

- 授权与窗口：用户明确授权 formal attempt 4。治理 HEAD
  `fda60c7a2619471ebebd6749bb7191ae76b5ff97`、产品代码 `e3028fae`、preflight attempt 8
  和最终漂移门通过后，独立 watchdog 启动并停止精确 Python Worker。
- 正式失败：Rust Worker在启动阶段立即返回
  `resource Worker mode requires configuration field token_file`；formal reason 为
  `rust_worker_exited_during_startup`。此时 `model_id=none`、`owned_container_id=none`，未创建
  Model/Instance/File、未启动 vLLM、未使用 GPU，也未进入状态链、推理或幂等门。
- 恢复与证据：统一 cleanup 和 watchdog 恢复精确 Python Worker；health/metrics 均为 200，
  无 Rust 进程或 Managed 容器。formal 为 FAIL、`cleanup_ok=1`，watchdog 为 FAIL、
  `recovery_ok=1`；两处清单文件 SHA-256 分别为
  `d5f2e72f785c14bd8f68300421d71a8b8834194f6751cec6f476942fd586feb8` 和
  `d936540b4de28307887c2b896f90fdc9cbabc487d5840cc14ccb5bb282b9bd08`。
- 独立事后审计：`m6b5-managed-runtime-post-audit-attempt-4` 对 formal/watchdog 清单、三项
  服务、全局资源 ID、Worker maintenance、Docker/Python Worker身份、Rust 进程和端口、
  两张 GPU、固定模型 manifest 与源码身份全部复核为 PASS；21 个清单条目全部通过，秘密
  文件数为 0，清单文件 SHA-256 为
  `93db665b14f6a0eac63f95abb3d521b47610d46b12e278a7e91983fb6e2de3ca`。
- 根因与修复：D178 使 Managed runtime 必须读取 Cluster registration token，但 executor 的
  固定 Worker 参数遗漏 `--token-file`。修订后显式传入既有受保护文件，并在 config 中核对
  精确路径；preflight 增加文件安全、只读认证 200 和静态参数审计。90 上 shell 语法与真实
  `validate-config` 路径检查通过，token 值未进入日志、Git 或证据。
- 当前停止位置：`attempt_4_recovered_retry_5_fix_pending_checkpoint`。下一项只允许提交修复、
  完成独立事后审计并执行全新的 preflight attempt 9；formal attempt 5 必须重新获得明确授权。

### 2026-10-28：M6-B.5.1 preflight attempt 9 通过

- 结果：治理 HEAD `dd8406df05bbdb915252894ca5bce12b407d02ba` 上九组准备门全部
  通过。除既有门外，新增检查确认 Cluster registration token 文件非 symlink、mode 0600、
  owner 正确、link count 1、只读认证返回 200；executor 静态审计确认 `--token-file` 与
  `CLUSTER_TOKEN_FILE` 均存在。
- 证据：`m6b5-preflight-attempt-9` 为 0700，56 个 manifest 条目全部通过、文件均为 0600，
  清单文件 SHA-256 为
  `c580bbcf22991e1ef17f2f79ef37ae44a348ea2ef9ca69e6f7b1b57ceab7d87e`。无写注入目录
  同为 0700，9 个条目全部通过、文件均为 0600，清单文件 SHA-256 为
  `b6f60ed99d90bdbe554c00e1bfd84eb45dad49425df238a5f48abefd14168843`；结果保持
  `STOP/source_head_mismatch`、`window_entered=0`、`cleanup_ok=1`。
- 独立复核：Server、Python Worker health/metrics 均为 200；Python Worker运行；无 Rust
  进程或 Managed 容器；GPU0/GPU1 均为 0 MiB、0%；仓库干净。准备过程没有停止服务、切换
  maintenance、写 Server、创建容器或使用 GPU。
- 当前停止位置：`ready_for_maintenance_authorization`。下一项只能由用户重新明确授权 formal
  attempt 5，attempt 4 的授权不延伸。

### 2026-10-28：M6-B.5.2 attempt 5 在 buffered chat 失败并恢复

- 授权与运行：用户在 preflight attempt 9 通过后明确授权 formal attempt 5。最终漂移门通过，
  独立 watchdog 停止精确 Python Worker；Rust Worker带 Cluster registration token 启动，解除
  maintenance 后稳定 READY，并在 GPU0 创建唯一 Qwen2.5-0.5B-Instruct vLLM 容器。
- 成功前缀：Model 18、Instance 33、ModelFile 12 和端口 40000 完成
  `pending -> analyzing -> scheduled -> initializing -> downloading -> starting -> running`；健康时间
  早于 Server `RUNNING` 观测，`inference models` 也经 Server 路径返回目标模型。
- 正式失败：第一条 buffered chat 返回 `HTTP 400 BadRequest: Invalid request`。formal reason 为
  `buffered_chat_request_failed`，因此 streaming chat、completion、logs 和三轮幂等未执行，
  attempt 5 保持 FAIL。
- 回滚与证据：统一 cleanup 删除本 attempt 的 Model/Instance/ModelFile、唯一 owned 容器、
  listener 和本地辅助状态，释放 GPU0，恢复精确 Python Worker；`cleanup_ok=1`、
  `recovery_ok=1`。formal 与 watchdog 清单文件 SHA-256 分别为
  `4cda778e8b3cb1d12a3ee714e00a0b32827384c2ce0b655294fc1b21257c5d32` 和
  `3c1f88229e18fb2c1cca62b020f59739df041273556d550062ee497948e7c53e`。
- 独立事后审计：`m6b5-managed-runtime-post-audit-attempt-5` 对上游清单、三项服务、Server
  全局资源 ID、Worker maintenance、Docker/Python Worker身份、Rust 进程和端口、两张 GPU、
  固定模型 manifest 与源码身份复核为 PASS；22 个清单条目全部通过，秘密文件数为 0，清单
  文件 SHA-256 为
  `65f2d38720a25714234c363ddf8386e5f72d3dfb93a841aca188ea4d5cf5af6f`。
- 根因与修复：固定 Server发送 `X-GPUStack-Model-Instance: model-18-33.static`，原 Python
  Worker从复合 destination 中解析 Instance ID；Rust Worker却只接受纯数字。产品提交
  `c725436bb2e117228866441f59715c332bfa042a` 改为解析原格式及可选 alias，同时继续拒绝重复、
  缺失和畸形头并在转发前剥离内部头。90 上 17 个聚焦 API 测试、131 个 Worker 测试、
  workspace 190 个测试、fmt、Clippy `-D warnings`、doc-tests 和 locked/offline build 全部通过。
- 当前停止位置：`retry_6_preflight_pending`。下一项只允许绑定新产品提交、建立 preflight/
  injection attempt 10 并运行离线/只读门；formal attempt 6 必须重新取得明确授权。

### 2026-10-28：M6-B.5.1 preflight attempt 10 通过

- 结果：治理 HEAD `255f87c916462b485e4ddb69bd4bf9d65de262a9` 上九组准备门全部
  通过；产品 ancestry 固定到 `c725436b`，新增复合 destination 解析回归测试和原推理代理测试
  均通过，locked/offline workspace build、凭据元数据、双 scope 与 Cluster token 只读认证、
  Runner/模型/GPU0/端口/Python Worker身份和脚本静态门全部通过。
- 证据：`m6b5-preflight-attempt-10` 为 0700、57 个 manifest 条目全部通过、文件均为 0600，
  清单文件 SHA-256 为
  `1c08a998d040a2a264d29194c1ecb3aeccb0c798299a62e39eeeccd88f9b06d2`。对应无写注入目录
  为 0700、9 个条目全部通过，清单文件 SHA-256 为
  `6c9f520e16bad891aaffa24a83e61f3d037d604592fb318bcc8b9c29e0bfae2f`；结果按预期保持
  `STOP/source_head_mismatch`、`window_entered=0`、`cleanup_ok=1`。
- 独立复核：Server、Python Worker health/metrics 均为 200；精确 Python Worker运行；无 Rust
  进程或 Managed 容器；GPU0/GPU1 均为 0 MiB/0%；HEAD、origin 和工作树一致且干净。
- 当前停止位置：`ready_for_maintenance_authorization`。下一项只能由用户明确授权 formal
  attempt 6，attempt 5 的授权不延伸。

### 2026-10-29：M6-B.5.2 attempt 6 在 vLLM 上游连接失败并恢复

- 授权与成功前缀：用户明确授权 formal attempt 6。最终漂移门通过后 watchdog 停止精确
  Python Worker，Rust Worker稳定 READY；Model 19、Instance 34、ModelFile 13、端口 40000
  和唯一 GPU0 vLLM 容器完成完整七状态链，健康先于 `RUNNING`，`inference models` 通过。
- 正式失败：第一条 buffered chat 已通过网关 destination 解析并进入 Rust Worker，但 Worker
  返回 `502 Managed inference upstream failed`，Server外层返回 503。formal reason 仍为
  `buffered_chat_request_failed`；streaming、completion、logs 和三轮幂等未执行，attempt 6
  固定为 `failed_during_window`。
- 根因：vLLM 按冻结 workload 在 host network 上监听 `10.213.80.90:40000`，健康检查也访问
  该地址；Rust 代理却固定请求 `127.0.0.1:40000`。这与原 Python Worker通过配置的
  `worker_ip` 代理到实例端口的语义不一致，连接因此在发送 HTTP 请求前失败。
- 回滚与审计：cleanup/watchdog 删除本次 Model/Instance/ModelFile、owned 容器、listener 和
  本地状态，释放 GPU0 并恢复精确 Python Worker；`cleanup_ok=1`、`recovery_ok=1`。formal、
  watchdog 清单文件 SHA-256 分别为
  `bc51a04aab4e8ea21d68b5d9b80ab7715bbf57e70aca55ddedc822a49cff48ad`、
  `f837e04a3b7b54172ce6e24c39bf8276ae26581d10f17bd98bac86cf10943ff7`。独立 post-audit
  attempt 6 的 22 个条目全部通过，清单文件 SHA-256 为
  `fcb934464a1126dba66e8f963f234d461a63ba95ac2a478ecc80b0eff05c157a`。
- 离线修复：产品提交 `d4920e71b89d6507f8e82fb2aff7dfbb24490103` 从受管理状态保存
  `worker_ip`，用 `SocketAddr(worker_ip, assigned_port)` 构造 upstream，并在缺少 Worker IP
  时 fail closed。真实节点地址转发回归、17 个 Worker API 测试、131 个 Worker 测试、
  workspace 190 个测试、fmt、Clippy `-D warnings`、doc-tests 和 locked/offline build 全部通过。
- 当前停止位置：`retry_7_preflight_pending`。下一项只允许使用全新 preflight/injection
  attempt 11 验证新检查点；formal/watchdog/post-audit attempt 7 必须重新取得明确授权。

### 2026-10-29：M6-B.5.1 preflight attempt 11 通过

- 结果：治理 HEAD `76c0f5d855de3878fefb0a826da64053d070bd76` 上九组准备门全部
  通过；产品 ancestry 固定到 `d4920e71`，真实 Worker地址代理回归、locked/offline workspace
  build、凭据元数据与双 scope、Cluster token只读认证、Runner/模型/GPU0/端口/Python Worker
  身份和脚本静态门全部通过。
- 证据：`m6b5-preflight-attempt-11` 为 0700、57 个 manifest 条目全部通过，清单文件
  SHA-256 为 `1c76b57e12b6540fc835ca81eec340de22e003a3b3805497d5cafc43b1ea10c9`；
  对应无写注入目录为 0700、9 个条目全部通过，清单文件 SHA-256 为
  `1a9fe515f3f6c3ff39961802b6c70020b2c709d68e0b3b283a6e75b13df5e6eb`。注入按预期为
  `STOP/source_head_mismatch`、`window_entered=0`、`cleanup_ok=1`。
- 独立复核：Server、Python Worker health/metrics 均为 200；精确 Python Worker运行；无 Rust
  进程或 Managed 容器；两张 GPU 均为 0 MiB/0%；HEAD、origin 和工作树一致且干净。
- 当前停止位置：`ready_for_maintenance_authorization`。下一项只能由用户明确授权 formal
  attempt 7，attempt 6 的授权不延伸。

### 2026-10-29：M6-B.5.2 formal attempt 7 全链通过并恢复

- 授权与执行：用户明确授权停止并恢复精确 Python Worker、切换 Worker 5 maintenance、创建
  并删除唯一 `m6b5-qwen25-05b-gpu0` 资源和 Rust vLLM 容器、仅使用 GPU0，并要求 watchdog
  完整回滚。最终漂移门通过后，独立 watchdog 以 1200 秒硬超时保护正式执行器。
- 控制链：Rust Worker稳定 READY 后创建 Model 20、Instance 35、ModelFile 14，分配端口
  40000 和唯一 owned container
  `c63e726ffb484825f6a25fa731f610f2b3d1b225fcd08904e68e4d9e0930c106`；真实观察到
  `pending -> analyzing -> scheduled -> initializing -> downloading -> starting -> running`，
  vLLM 健康时间早于 Server `RUNNING` 写回。
- 数据面与日志：只经 active Context 的 Server 路径完成 `inference models`、buffered chat、
  SSE streaming chat、completion、错误模型拒绝、`model logs --tail` 和有界 `--follow`；
  D189 的 `worker_ip + assigned_port` upstream 在真实节点地址上通过。
- 幂等：模型进入 `RUNNING` 后覆盖至少三轮同进程协调，Model/Instance/port、容器 ID、spec
  hash 和 ownership labels 均未变化，Rust-owned 容器始终为 1；随后再次 chat 成功。
- 清理与恢复：先恢复 maintenance，再删除精确 Model 并等待 Instance 404，协调器删除唯一
  owned runtime 和本地状态，精确清理 ModelFile，优雅停止 Rust Worker，最后恢复原 Python
  Worker container。`smoke_passed=1`、`cleanup_ok=1`、正式和 watchdog 均为 PASS。
- 证据：正式目录含 87 个清单条目，清单文件 SHA-256 为
  `c81af217bf80a4ca568de8c45e5a602c624fe87507967c6ce1f6f4dbe9fc9eb3`；watchdog 含 5 个
  条目，清单文件 SHA-256 为
  `cabb29165c85b5a8b68cbe1023b8863ab2d12bc1d4385eed1eb271899c947fa7`。独立 post-audit
  的 22 个条目全部通过，清单文件 SHA-256 为
  `8d60613d2086ec708261368a126079f896bd49f96bba067ffac931ee3a6482da`，外部变更为 `none`。
- 最终现场：Server、Python Worker health/metrics 均为 200；精确 Python Worker运行；Rust
  进程、Managed 容器、20150/40000 listener 和 GPU compute process 均为 0；两张 GPU 均为
  0 MiB/0%，源码 HEAD、origin 和工作树一致。
- 当前停止位置：`ready_for_review`。用户审阅详细报告前不得标记 `accepted` 或进入 M7；审阅
  通过后只允许先退役阶段专用 API Key、Context 和 token 文件并证明旧 token 返回 401。

### 2026-10-29：M6-B.5 阶段凭据退役并完成验收

- 用户验收：用户接受 formal attempt 7 的详细报告，按 D193 只解锁阶段凭据退役，不自动执行
  M7。
- 精确删除：删除前再次确认唯一 API Key ID 21、名称
  `rust-worker-m6b5-operator-20261028-v2`、scope `management,inference` 和本地 active Context
  identity。Server `DELETE /v2/api-keys/21` 返回 200，原 token 随后访问 Worker管理接口返回
  401；只删除 `m6b5-operator` Context 和
  `/home/wujiaxing2/.config/gpustack-rust/m6b5-operator-token`。
- 保留边界：M3-D 的 Cluster registration token 文件仍为相同 inode/size/mtime identity；没有
  删除或改写其他 API Key、Context、Worker credential、模型文件、Server 资源或容器。
- 独立复核：退役证据目录 mode 0700、15 个文件均为 0600，manifest 全部通过，秘密扫描无
  命中，清单文件 SHA-256 为
  `87e34cef088ee9506a8ced0157d2bd1eadb5f55f99292d5469089c473a04e6a0`。Server、Python
  Worker health/metrics 均为 200；Rust 进程、Managed 容器和 20150/40000 listener 均为 0，
  两张 GPU 均为 0 MiB/0%。
- 当前停止位置：M6-B.5 更新为 `accepted`。下一项只允许共同讨论和冻结 M7 的目标、故障模型、
  验收门和授权边界；本次验收不授权启动 M7、停止服务、创建 workload 或使用 GPU。

### 2026-10-29：M7-0 恢复与双 GPU 设计冻结

- 用户决定：接受 M7 继续使用宿主机 Rust Worker完成恢复能力和双 GPU TP=2，Worker容器化及
  最终 R1 重跑留在 M8；本次只冻结设计，不执行维护窗口或 GPU 操作。
- 冻结范围：启动时以 Server 权威 full sync 和 Docker 实际状态接管唯一完整 tuple/spec 匹配
  的健康容器；多匹配、缺标签和 spec 冲突 fail closed；Worker退出不删除健康 Managed workload。
  `restart_on_error` 对齐 Python Worker 的 0/10/20/40/80 秒至 300 秒封顶退避，持久
  `restart_count` 与进程内连续失败计数分离，日志保留当前和上一代。
- 故障与实验：Server/watch/Docker 短暂故障不能授权删除；90 上使用只影响 Rust Worker 的
  TCP/Unix socket 代理，不停止真实 Server 或 Docker daemon。单 GPU 恢复拆为 Worker crash
  接管、vLLM crash 自动恢复、连接故障恢复三个独立授权窗口；双 GPU继续使用固定
  Qwen2.5-0.5B、Runner digest、GPU0/GPU1 和 TP=2。
- 证据角色：M7 的真实结果是宿主机阶段证据；M8 必须使用容器化 Worker重跑恢复与单/双 GPU
  套件后才能形成最终 R1 candidate。所有真实窗口继续要求 Python Worker精确 stop/restore、
  watchdog、独立事后审计、短期凭据退役和用户审阅。
- 漂移审计：目标、Server/Docker 权威边界、固定 vLLM/Docker/local-path/单 Worker范围、未知
  容器保护和 CLI-first 原则均未改变。没有把 M6 的同进程幂等证据提升为 crash recovery，也
  没有把宿主机证据提升为容器化发布证据，结论为 `aligned`。
- 结果：M7-0 `accepted`。本检查点只修改治理文档，没有改产品代码、连接真实 Docker socket、
  写 Server、停止 Python Worker、切换 maintenance、创建 workload、使用 GPU 或创建凭据。
- 当前停止位置：下一项只允许进入 M7-A 启动接管的离线实现和自动化验证；完成后必须先在聊天
  中交付逐项实现报告并由用户审阅。M7-B 及任何真实阶段不能自动开始。

### 2026-10-29：M7-A 启动接管离线实现待审阅

- 目的与范围：只实现 Worker进程重启后的本地启动接管路径，继续使用 fake Server、fake
  Docker、临时文件系统和模拟健康探针；没有连接真实 Docker socket、写真实 Server、停止
  Python Worker、切换 maintenance、创建 workload、使用 GPU 或创建凭据。
- 实现：revision 0 的空 watch seed 不再被当成 Server 权威空状态；首次 full sync 后分别按
  `STARTING` 和 `RUNNING` 规划。`RUNNING` 只接管唯一完整 ownership tuple、spec hash、镜像、
  运行状态和 `/v1/models` 健康均匹配的容器，并在健康探测后再次刷新 Server 和 Docker 两个
  权威来源，随后重建 cache、manifest、当前日志跟踪和内存 route。已接管实例的后续协调直接
  `Noop`，不重复探测或修改 runtime。
- 失败边界：缺失或不健康的 `RUNNING` 容器、冲突 manifest、多匹配、部分标签、spec 漂移和
  非零 `restart_count` 均 fail closed；不创建、启动、停止、删除容器，不发布 route，也不写回
  Server。只有权威 `STARTING` 继续使用既有创建路径；`RUNNING` 缺失后的自动恢复、日志代际和
  连续故障监测留给 M7-B/C。
- 代码与证据：产品提交为
  `852e16c87be2f6dfb6b0d0b9bc1c925399bb04a5`，只修改
  `rust/crates/gpustack-worker/src/managed/runtime.rs` 和
  `rust/crates/gpustack-worker/src/managed/reconciler.rs`。90 上 `cargo fmt --all --check`、
  workspace/all-targets/all-features Clippy `-D warnings`、197 项 locked/offline all-target 测试、
  doc-tests 和 locked/offline build 全部通过；目标文件的本地 pre-commit 和 `git diff --check`
  通过。90 的仓库级 `make lint` 因环境没有 `uv` 未能启动，未产生文件或外部状态变化。
- 解释：证据证明启动接管的离线判定、状态重建、幂等和 fail-closed 行为，不证明真实 Docker
  crash recovery、vLLM 自动重启、日志代际、连接故障恢复或双 GPU；这些不能从本阶段外推。
- 当前停止位置：M7-A 为 `ready_for_review`，不是 `accepted`。下一项只允许在聊天中交付并由
  用户审阅详细实现报告；接受后才能进入 M7-B 离线设计/实现，任何真实阶段仍需独立授权。

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
