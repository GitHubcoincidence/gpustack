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

## 当前状态

- 状态：`ready`
- 当前阶段：M3-B Rust Server client 已接受，M3-C 尚未开始
- 本地检查过的源码基线：`76dc841916ff8fed4068a1ab99d178d2e27a4a06`
- 分支：`codex/rust-worker`
- 初始英文设计检查点：`74ef8977`（`docs(worker): define rust worker rewrite plan`）
- 检查点前已存在且与本项目无关的工作树状态：未跟踪目录 `output/`；本项目不修改它。
- 90 主机身份：`wujiaxing2@r760xa`；仓库路径：`/home/wujiaxing2/workspaces/gpustack`。
- 90 主机 Git 工具：Git `2.43.0`；OpenSSH `9.6p1`；账号级 SSH 已认证为 `GitHubcoincidence`。
- 90 主机源码状态：M3-B 实现检查点为 `fed728afc82c4f4f5d5f5d21e0800a73a61c3ea3`，注释密度校正检查点为 `8287ceb0af461d291c1e4faf780715acd862d7d9`；`codex/rust-worker` 与 `origin/codex/rust-worker` 一致，工作树干净。
- 实现状态：四 crate Rust workspace、三个首批 CLI 命令和类型化 Server client 已建立；尚未实现身份持久化、Worker 控制循环、Docker 工作负载或 GPU 路径，也未远程部署 Rust Worker。
- 中文治理文档检查点：`78d46176`（`docs(worker): translate rust worker plan to Chinese`）
- 中文状态检查点：`d3623bfb`（`docs(worker): record Chinese documentation checkpoint`）
- 当前停止位置：M3-B 类型化 client 已接受；现有 Python Server/Worker 继续运行，Rust Worker 尚未连接 Server，M3-C 尚未开始。
- 下一项允许执行的工作：实现 M3-C 原子 token 持久化、注册状态机、heartbeat/status/watch/full-sync 控制循环与 mock 重连；不得执行真实注册、heartbeat、状态更新等 Server 写请求。

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

## 未解决风险与待确认事项

- 固定 Runner v0.25.1 与 Qwen-7B 的实际加载兼容性尚未验证；必须在获批 GPU 窗口内作为 smoke 证据，不能从历史 v0.2.2 容器结果推断通过。
- Rust Worker API、metrics 和 inference 的准确端口尚未冻结；必须避开现有 80、3000、10150、10151 和其他已监听端口，并在每次启动前检查冲突。
- 真实 drain、停止 Python Worker 与回滚流程尚未执行；`docker stop/start gpustack-worker` 只作为已识别的容器级回滚路径，正式演练需维护窗口。
- Rust 容器最终 user/group 仍需在 M8 根据 Docker socket、NVML 与持久化目录要求验证；宿主机开发用户已确认具备 `docker` group 权限。
- 90 主机到 GitHub 的小规模 fetch/push 可用，但大对象传输吞吐过低；bootstrap 和发布 tag 增量 bundle 是当前已验证的补充路径。
- 90 主机 `127.0.0.1:17897` 与 `[::1]:17897` 当前存在未归因监听；它不影响离线构建证据，但在复用或停止该代理前必须先确认所有者。后续自动化 SSH 命令显式使用 `ClearAllForwardings=yes`，避免自动创建额外转发。
