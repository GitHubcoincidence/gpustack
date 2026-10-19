# GPUStack Rust 重写：执行与交接记录

## 文档职责

本文记录已接受决策、关键执行证据、当前状态与下一项允许执行的工作。聊天仍是主要交互渠道。大段命令输出与运行时 artifact 只记录其 identity 和路径，不直接嵌入本文。

## 已冻结决策

- 2026-10-17：接受 R1 范围、非目标、CLI-first 原则、Managed/Lab 边界、Worker 协议界面、协调模型、Git 策略、部署拓扑、验证门禁与回滚方法。
- 2026-10-17：Worker 容器化被纳入 R1 发布门禁；宿主机进程运行仍保留为开发阶段。
- 2026-10-17：最终公开 CLI 统一为 `gpustack`，迁移期二进制为 `gpustack-rs`。更早提出的最终独立 `gpustack-worker` 公开命令已被取代。
- 2026-10-17：用户选择让 90 主机使用账号级 GitHub SSH 权限，而不是仓库级 deploy key。
- 2026-10-17：用户选择以中文维护 `rust/docs/` 内部治理与实验文档；技术标识保留英文。顶层 MkDocs `docs/` 若发布相关内容，单独编写英文版。

## 当前状态

- 状态：`ready_for_review`
- 当前阶段：M3-A Server 控制面契约与 fixture 已完成只读提取，等待共同审阅
- 本地检查过的源码基线：`76dc841916ff8fed4068a1ab99d178d2e27a4a06`
- 分支：`codex/rust-worker`
- 初始英文设计检查点：`74ef8977`（`docs(worker): define rust worker rewrite plan`）
- 检查点前已存在且与本项目无关的工作树状态：未跟踪目录 `output/`；本项目不修改它。
- 90 主机身份：`wujiaxing2@r760xa`；仓库路径：`/home/wujiaxing2/workspaces/gpustack`。
- 90 主机 Git 工具：Git `2.43.0`；OpenSSH `9.6p1`；账号级 SSH 已认证为 `GitHubcoincidence`。
- 90 主机源码状态：M3-A fixture 实现检查点为 `cff7c1fca8272d02600f32333f6bd0b6aa7c5408`；提交并推送后 `codex/rust-worker` 与 `origin/codex/rust-worker` 一致，工作树干净。
- 实现状态：四 crate Rust workspace 和三个首批 CLI 命令已建立；尚未实现 Server 注册、Docker 工作负载或 GPU 路径，也未远程部署 Rust Worker。
- 中文治理文档检查点：`78d46176`（`docs(worker): translate rust worker plan to Chinese`）
- 中文状态检查点：`d3623bfb`（`docs(worker): record Chinese documentation checkpoint`）
- 当前停止位置：M3-A 契约文档、fixture 与离线完整性测试已形成；现有 Python Server/Worker 继续运行，Rust Worker 尚未连接 Server。
- 下一项允许执行的工作：共同审阅 M3-A 契约和 fixture；接受前不实现或执行注册、heartbeat、状态更新等 Server 写路径。

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

## 未解决风险与待确认事项

- 固定 Runner v0.25.1 与 Qwen-7B 的实际加载兼容性尚未验证；必须在获批 GPU 窗口内作为 smoke 证据，不能从历史 v0.2.2 容器结果推断通过。
- Rust Worker API、metrics 和 inference 的准确端口尚未冻结；必须避开现有 80、3000、10150、10151 和其他已监听端口，并在每次启动前检查冲突。
- 真实 drain、停止 Python Worker 与回滚流程尚未执行；`docker stop/start gpustack-worker` 只作为已识别的容器级回滚路径，正式演练需维护窗口。
- Rust 容器最终 user/group 仍需在 M8 根据 Docker socket、NVML 与持久化目录要求验证；宿主机开发用户已确认具备 `docker` group 权限。
- 90 主机到 GitHub 的小规模 fetch/push 可用，但大对象传输吞吐过低；bootstrap 和发布 tag 增量 bundle 是当前已验证的补充路径。
