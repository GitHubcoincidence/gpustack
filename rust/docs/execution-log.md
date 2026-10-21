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

## 当前状态

- 状态：`ready`
- 当前阶段：M3-D.2 真实集成分段执行准备
- 本地检查过的源码基线：`76dc841916ff8fed4068a1ab99d178d2e27a4a06`
- 分支：`codex/rust-worker`
- 初始英文设计检查点：`74ef8977`（`docs(worker): define rust worker rewrite plan`）
- 检查点前已存在且与本项目无关的工作树状态：未跟踪目录 `output/`；本项目不修改它。
- 90 主机身份：`wujiaxing2@r760xa`；仓库路径：`/home/wujiaxing2/workspaces/gpustack`。
- 90 主机 Git 工具：Git `2.43.0`；OpenSSH `9.6p1`；账号级 SSH 已认证为 `GitHubcoincidence`。
- 90 主机源码状态：M3-D.1.1 设计检查点为 `eb5a9c2d80d1e191f9030808e1dcc80e7a217445`，代码检查点为 `99e9fdb9038f65c480371005fb62fe1c1d6b1c72`，接受检查点为 `ee701a075d921c95e33e29ea232a0dd1ffd291bb`。
- 实现状态：四 crate Rust workspace、类型化 Server client、Server 权威 Worker 身份两文件状态机、Model/ModelInstance/ModelFile 三资源控制循环，以及显式 `worker run --control-plane-only` supervisor 和最小 `/healthz` 已经建立；尚未执行真实 Server 集成、ModelFile 本地准备、Docker 工作负载、GPU 盘点或 vLLM 路径。
- 中文治理文档检查点：`78d46176`（`docs(worker): translate rust worker plan to Chinese`）
- 中文状态检查点：`d3623bfb`（`docs(worker): record Chinese documentation checkpoint`）
- 当前停止位置：M3-D.2 运行手册已冻结；用户选择亲自执行全部命令，助手按门禁逐段交付。旧 UUID 等值语义和“本地模型跳过 DOWNLOADING”的设计语义保持 `invalidated`。现有 Python Server/Worker 继续运行，Rust Worker 尚未连接真实 Server，也未读取 token。
- 下一项允许执行的工作：用户执行第 0 段只读预检并返回完整非敏感输出；只有该段通过审阅后才允许干净构建，凭据准备和真实写路径继续锁定。

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

## 未解决风险与待确认事项

- 固定 Runner v0.25.1 与 Qwen-7B 的实际加载兼容性尚未验证；必须在获批 GPU 窗口内作为 smoke 证据，不能从历史 v0.2.2 容器结果推断通过。
- Rust Worker API 与 metrics 已分别冻结为 `20150` 和 `20151`；inference 端口仍待 M5/M6 冻结。每次启动前仍须检查端口冲突。
- 真实 drain、停止 Python Worker 与回滚流程尚未执行；`docker stop/start gpustack-worker` 只作为已识别的容器级回滚路径，正式演练需维护窗口。
- Rust 容器最终 user/group 仍需在 M8 根据 Docker socket、NVML 与持久化目录要求验证；宿主机开发用户已确认具备 `docker` group 权限。
- 90 主机到 GitHub 的小规模 fetch/push 可用，但大对象传输吞吐过低；bootstrap 和发布 tag 增量 bundle 是当前已验证的补充路径。
- 90 主机 `127.0.0.1:17897` 与 `[::1]:17897` 曾存在未归因监听，M3-C 完成核验时已无监听；本项目没有停止它。未来复用或处置同端口监听前仍须确认所有者，自动化 SSH 命令继续显式使用 `ClearAllForwardings=yes`，避免自动创建额外转发。
