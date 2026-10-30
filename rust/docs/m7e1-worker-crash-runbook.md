# M7-E.1 Rust Worker crash 接管验证方案

## 1. 文档状态

- 阶段：M7-E.1。
- 状态：`accepted / credential_ready / preflight_retry_2_pending`。
- 目的：保留 preflight attempt 1 的兼容性停止证据，在窄修正后从头执行 attempt 2 准备门。
- 当前停止边界：API Key ID 23 和 `m7e1-operator` Context 已就绪；只允许重跑只读
  preflight 与错误 HEAD 无写注入，不允许停止 Python Worker、切换 maintenance、创建模型或
  vLLM 资源、启动 Rust Worker/vLLM、使用 GPU 或执行真实 SIGKILL。

本方案已经由用户接受，但接受不等于授权执行。受保护短期凭据已经准备完成，当前授权只覆盖
只读 preflight 与无写故障注入；通过后仍须再次请求只覆盖 M7-E.1 attempt 1 的精确维护窗口
授权。M7-E.2、M7-E.3 和 M7-F 不在该授权范围内。

## 2. 验证目标

在 GPU0 上部署一个固定 TP=1 vLLM workload，精确 `SIGKILL` 宿主机 Rust Worker 进程，证明：

1. Rust Worker 消失期间，既有 vLLM 容器仍以原容器 ID、原启动时间和原端口健康运行；
2. Rust Worker 使用完全相同的配置重启后，基于 Server 权威状态与 Docker 实际状态接管该
   容器，重建 route、manifest、cache 和当前日志跟踪；
3. 接管过程没有创建、启动、停止、重启或删除 vLLM 容器，没有增加
   `restart_count`，也没有创建新的日志代际；
4. Server-only CLI 推理在恢复后重新可用，重复调和保持幂等；
5. 无论业务验证成功或失败，唯一测试资源均被精确清理，原 Python Worker、Server、Docker、
   GPU、端口、模型文件和源码状态恢复基线。

M7-E.1 验证的是 **Worker crash 后接管健康 runtime**，不是 vLLM crash/restart。vLLM 重启、
`ERROR -> SCHEDULED`、restart count 递增和两代日志属于 M7-E.2。

## 3. 明确不做的事情

- 不停止或重启真实 GPUStack Server、宿主机 Docker daemon 或 Higress。
- 不使用 M7-D 记录的 Server TCP/Docker Unix socket 代理；代理故障属于 M7-E.3。
- 不向 Python Worker、非本次 owned 容器或 GPU1 注入任何故障。
- 不使用 `pkill`、名称通配、标签宽匹配或未验证 PID 终止进程。
- 不预留某个固定推理端口；只接受 Worker 从 `40000-40063` 分配的空闲端口，并要求接管前后
  端口严格不变。
- 不把 cleanup 成功、容器存活或直接 vLLM 推理成功单独提升为正式 PASS。
- 不把本阶段的宿主机 Worker 证据外推为 M8 容器化 Worker 的发布证据。

## 4. 冻结输入

| 项目 | 固定值 |
|---|---|
| Server | `http://127.0.0.1/`，v2.2.3，commit `1cfcbbf` |
| Rust Worker | ID 5，UUID `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`，名称 `rust-m3d-r760xa` |
| Worker 地址 | `10.213.80.90`，接口 `eno8403`，API `20150`，metrics `20151` |
| Rust data/log | `/home/wujiaxing2/.local/state/gpustack-rust-worker/1` 与 `/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d-logs` |
| Cluster token | 既有 `/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token`，只作为 Worker registration token |
| Operator Context | `m7e1-operator`，阶段专用、平台管理与推理权限、最长 24 小时 |
| Operator token 文件 | `/home/wujiaxing2/.config/gpustack-rust/m7-operator-token`，owner-only、`0600`、非 symlink、单硬链接 |
| 测试模型名 | `m7e1-qwen25-05b-gpu0` |
| 模型输入 | `Qwen/Qwen2.5-0.5B-Instruct`，revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 本地模型目录 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct`，10 文件，999,604,126 字节 |
| 模型 manifest | SHA-256 `645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6` |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| vLLM 参数 | TP=1，GPU0，max model len 2048，GPU memory utilization 0.5，dtype auto，enforce eager |
| GPU0 | `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| 禁止使用 GPU | GPU1 `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` |
| Managed 端口池 | `40000-40063`；记录动态分配端口并要求全程不变 |
| Python Worker恢复对象 | 容器 ID `68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165`，restart policy `unless-stopped`，host network；image ID 由新 preflight 重新冻结 |

源码 HEAD、Rust binary SHA-256、`Cargo.lock` SHA-256 和正式脚本 SHA-256 不在草案中提前写死；
它们在方案接受后的新 preflight 中从干净 governance HEAD 重新构建并冻结。

## 5. 尝试 identity 与受保护路径

formal attempt 1 使用以下互不复用的路径：

- formal evidence：`~/.local/state/gpustack-rust-worker/evidence/m7e1-worker-crash-attempt-1`
- watchdog evidence：`~/.local/state/gpustack-rust-worker/evidence/m7e1-worker-crash-watchdog-attempt-1`
- post-audit evidence：`~/.local/state/gpustack-rust-worker/evidence/m7e1-worker-crash-post-audit-attempt-1`
- preflight attempt 1：`~/.local/state/gpustack-rust-worker/evidence/m7e1-worker-crash-preflight-attempt-1`，
  已按 D210 以 `STOP/fixed_runtime_inputs` 保留，不得修改或复用；
- preflight attempt 2：`~/.local/state/gpustack-rust-worker/evidence/m7e1-worker-crash-preflight-attempt-2`；
- no-write injection attempt 2：`~/.local/state/gpustack-rust-worker/evidence/m7e1-worker-crash-pre-window-injection-attempt-2`；
- executor：`~/.local/state/gpustack-rust-worker/tools/m7e1-worker-crash-attempt-1.sh`
- watchdog：`~/.local/state/gpustack-rust-worker/tools/m7e1-worker-crash-watchdog-attempt-1.sh`
- preflight：`~/.local/state/gpustack-rust-worker/tools/m7e1-worker-crash-preflight-attempt-1.sh`
- post-audit：`~/.local/state/gpustack-rust-worker/tools/m7e1-worker-crash-post-audit-attempt-1.sh`
- shared library：`~/.local/state/gpustack-rust-worker/tools/m7e1-worker-crash-lib-attempt-1.sh`

目录必须事先不存在；脚本为 `0700`，目录为 `0700`，证据文件为 `0600`。attempt 失败后不能覆盖
原路径，任何 retry 必须使用新的 attempt identity、重新 preflight 并重新请求授权。

五份脚本的 Git 权威源分别为 `rust/scripts/m7e1/lib.sh`、`preflight.sh`、`executor.sh`、
`watchdog.sh` 和 `post-audit.sh`。90 上的 tools 副本只能从干净冻结 HEAD 安装，并在 preflight、
watchdog、executor 与 post-audit 之间用 SHA-256 相互验证；不得手工修改运行副本。

### 5.1 正式工具链职责

- `lib.sh`：冻结资源 identity，并集中实现只读采集、PID identity、完整 ownership 校验、精确
  runtime fallback cleanup、Python Worker安全恢复、秘密扫描和证据封存。
- `preflight.sh`：执行源码/脚本身份、locked/offline 构建、六项接管测试、短期凭据、固定输入、
  外部基线和静态安全门；用错误 HEAD 证明 executor 在窗口前 STOP 且不写外部状态。
- `executor.sh`：执行唯一正式状态机；只在所有外部门和 armed watchdog 身份通过后停止固定
  Python Worker，建立 workload，精确 SIGKILL Rust Worker，验证同容器接管并统一 cleanup。
- `watchdog.sh`：以独立 session leader 身份先写 armed identity，再在 1200 秒边界内运行
  executor；任何非零退出都进入精确回滚，未知 PID/容器 fail closed。
- `post-audit.sh`：不复用 executor 内存状态，独立对照 preflight 基线复核服务、资源、容器、
  端口、GPU、源码、模型和三条证据链。

## 6. 正式执行前门禁

### 6.1 源码、构建与脚本

1. 90 服务器的分支、HEAD、origin 和干净 worktree 与接受后的 governance commit 完全一致；
   本机既有未跟踪 `output/` 不进入远端门禁，也不得读取、修改或提交。
2. M7 产品提交 `0d861a4c9d5502341cbc78eefc60e30bb32c24fd` 必须是 HEAD 祖先。
3. 使用仓库固定 Rust 1.99.1、`Cargo.lock` 和 offline 依赖构建；记录 binary、lockfile、脚本
   和配置摘要 SHA-256。
4. executor、watchdog、post-audit 必须静态检查精确资源名、PID guard、ownership tuple、
   cleanup 顺序、证据权限、秘密排除和 Python Worker恢复对象。
5. no-write injection 使用错误 HEAD、错误 PID starttime 或错误 executable identity，必须在
   maintenance、服务、Server、Docker、GPU 或正式证据路径发生任何写入前安全停止。

### 6.2 外部基线

正式窗口前必须重新证明：

- Server、Python Worker health/metrics 均为 200；Python Worker identity 与恢复材料一致；
- Rust Worker进程数为 0，`20150/20151` 空闲，没有 Rust-owned 容器；
- 目标 Worker maintenance、目标名称占用和 Server 资源集合符合基线；
- 测试模型名不存在，固定 ModelFile 是否已存在被单独记录；
- Managed 端口池无监听，GPU0/GPU1 均无 compute process、显存占用为基线；
- 固定模型目录和 manifest 完整，Runner RepoDigest、image ID、OS/arch 符合冻结输入；
- operator Context 可认证，scope/expiry 符合本阶段，Cluster token 与 operator token 角色没有混用；
- formal、watchdog、post-audit 路径尚未使用。

preflight 通过只把状态推进到 `ready_for_maintenance_authorization`，仍不得进入窗口。

## 7. 正式维护窗口顺序

正式 attempt 的硬超时为 1200 秒。watchdog 必须通过独立 SSH 命令以 `setsid` session leader
启动并先写入 armed 状态；它再把 executor 作为受保护子进程启动。executor 只有在验证 watchdog
PID、starttime、PGID、SID、shell identity、watchdog/executor SHA 和 evidence identity 后继续。

### 7.1 进入窗口与建立 workload

1. 记录 Server、Python Worker、maintenance、Server 资源、Docker ID、GPU、端口、本地状态和
   Git 的 before 基线。
2. 只停止第 4 节冻结的精确 Python Worker容器 ID；停止前再次验证容器名称、image ID、
   restart policy 和 network mode。不得停止 Server 容器。
3. 启动与 Worker ID 5 对齐的宿主机 Rust Worker，直接连接真实 Server 和
   `/var/run/docker.sock`，使用既有 data/log 目录。进程通过 `setsid` 建立独立 session；记录
   PID、PGID、`/proc/<pid>/stat` starttime、canonical executable、binary SHA、NUL 分隔参数
   SHA 和两个监听端口的 owner。
4. 先保持 Worker maintenance，等待 API、ready、metrics 和 Server identity 稳定；随后显式
   disable maintenance，并以至少两个间隔样本确认 ready。
5. 通过 `model deploy` 创建唯一模型，使用第 4 节固定 Runner、模型、GPU0 和 vLLM 参数；等待
   完整 Managed 状态链、健康先于 `RUNNING`、ModelFile ready/100% 和唯一 owned 容器。
6. 记录 Model、ModelInstance、ModelFile ID。若 ModelFile 在 before 基线已存在，只记录复用，
   cleanup 不得删除；只有本 attempt 创建且基线不存在的 ModelFile 才可在 cleanup 精确删除。
7. 用 Server-only CLI 完成 `inference models`、buffered chat、completion 和 `model logs` 基线。

### 7.2 crash 前不可变 identity

在注入故障前冻结以下值：

- Model、ModelInstance、ModelFile ID 与 Server `RUNNING` 状态；
- 动态分配端口、`restart_count`、`last_restart_time`；
- 唯一容器 full ID、image ID、`State.StartedAt`、running/health 状态；
- 完整 ownership tuple、spec hash、Worker UUID、ModelInstance ID；
- 容器数量必须恰好为 1，GPU0 必须有对应进程，GPU1 不得出现测试进程；
- 本地 manifest/cache identity 与 `instance.0.log`，不得已有 `instance.1.log`；
- Rust Worker PID/starttime/executable/SHA/参数 SHA/PGID，以及 `20150/20151` listener owner；
- crash 前 Server-only 和直连 vLLM 的 `/v1/models`、buffered chat 成功证据。

记录 crash 开始 UTC 时间，用于随后查询该容器的 Docker event 窗口。

### 7.3 精确故障注入

executor 写入 `intentional-worker-crash` phase marker 后，立即重新验证：

1. PID 为记录的十进制正整数且仍存活；
2. `/proc/<pid>/stat` starttime、canonical executable、binary SHA 和参数 SHA 全部未变；
3. `20150` 与 `20151` 仍由同一 PID 持有；
4. PGID/session 与记录一致；
5. vLLM container ID 与 Rust PID 不相同且仍健康。

只有五组 guard 全部通过才执行 `kill -KILL <exact-rust-pid>`。禁止 `pkill`、PID 模糊搜索、
进程组广播或容器信号。随后必须确认原 PID/starttime 消失、子进程退出状态为 137，Worker API
和 metrics 不可达；任一 identity 漂移都在发送信号前 STOP。

### 7.4 Worker 缺席期间的连续性证明

Rust Worker 消失后、重启前，必须在有界时间内证明：

- vLLM 仍是同一容器 ID、同一 `StartedAt`、同一端口、同一 image/spec/ownership；
- 该 ownership tuple 下仍恰好一个容器，状态 running/healthy；
- ModelInstance ID、`RUNNING`、`restart_count` 和 `last_restart_time` 未变；
- GPU0 对应 compute process 仍在，GPU1 未被使用；
- 直接访问 `http://10.213.80.90:<assigned-port>/v1/models` 和 buffered chat 返回成功。

直连 vLLM 只用于证明 runtime 连续性，不替代正式 Server-only CLI 验收。Worker缺席期间通过
Server路由的结果只作诊断记录，不作为 runtime 是否健康的判断依据。

### 7.5 重启与原位接管

1. 使用与第一次启动完全相同的 binary、参数、data/log 目录、Cluster token 文件和真实
   Server/Docker endpoint 重启 Rust Worker；参数 SHA 必须相同，新 PID/starttime 必须不同。
2. 等待首次权威 full sync、Worker API/ready/metrics 稳定和 Server Worker ready 恢复。
3. 证明 runtime 重新进入本地 Running、route/manifest/cache/log tracking 恢复。
4. 重复采集 crash 前 identity，所有 runtime identity 必须严格相同；Rust 进程 identity 应变化。
5. 查询 crash 开始到接管完成之间的精确 container Docker events。允许 health 状态类事件，
   但不得出现 `create`、`start`、`die`、`stop`、`kill`、`restart` 或 `destroy`。
6. 用 Server-only CLI 再次完成 models、buffered chat、completion 和 logs；连续三轮 status/container/
   identity 查询必须保持同一结果，最后再执行一次 buffered chat。

## 8. 正式 PASS 条件

正式 PASS 必须同时满足：

1. Rust Worker故障确由精确 `SIGKILL` 产生，原进程退出状态为 137；
2. crash 前、缺席期间、接管后是同一 vLLM container ID 与 `StartedAt`；
3. 端口、image、spec hash、完整 ownership tuple、Model/Instance/File ID 均未变化；
4. `restart_count` 和 `last_restart_time` 未变化，日志仍只有 generation 0；
5. Docker event 窗口无容器 create/start/die/stop/kill/restart/destroy；
6. Worker缺席期间直连 vLLM models/chat 成功；恢复后 Server-only models/chat/completion/logs 成功；
7. 三轮重复调和无额外容器、状态写回、restart generation 或 identity 漂移；
8. 正式 cleanup、watchdog 判定、独立 post-audit、秘密扫描、manifest 复算和外部基线恢复全部通过。

以下情况均为正式失败，cleanup 成功不能改判：

- vLLM 容器被重建、重启或 identity 变化；
- 接管后出现第二个容器、端口变化、spec/ownership 不一致；
- `restart_count` 增加、`last_restart_time` 改变或出现 generation 1 日志；
- runtime 连续性通过但 route、Server-only 推理或日志恢复失败；
- business recovery 成功但 cleanup、Python Worker恢复、watchdog 或 post-audit 失败。

若在停止 Python Worker前发现源码、凭据、基线、GPU workload 或其他 drift，则结果为 STOP，
不得宣称进入过维护窗口。

## 9. watchdog 与失败回滚

watchdog 监视主 executor，而不是把“Rust PID 消失”本身当作故障；
`intentional-worker-crash` marker 使计划内的 Rust 缺席成为允许阶段。如果 executor 在 crash 后
退出或超过 1200 秒，watchdog 使用已封存 identity 执行回滚：

1. 尽力将 Worker设置为 maintenance，阻止新的调度；
2. 若 Rust Worker仍存在，只在 PID/starttime/executable/SHA/参数/PGID 全部匹配后终止该精确
   进程；不得使用名称或端口宽匹配杀进程；
3. 通过唯一测试模型名、before ID 集合与 attempt 记录定位 Server 资源；优先用权威 CLI 删除
   唯一测试模型并等待收敛；
4. 若 Rust 已死且权威删除无法清理 runtime，只允许对记录的精确 container ID 在 image、
   ModelInstance ID、Worker UUID、mode、component、schema、spec hash 全部匹配时 stop/remove；
5. 只清理该 attempt 精确 manifest/cache/log 和 attempt-created ModelFile，不触碰未知、Python、
   Lab 或 baseline 资源；
6. 只有 owned 容器为零、GPU0/GPU1 回到基线、Managed 端口无监听且 Rust PID 已消失后，才启动
   第 4 节固定的 Python Worker容器；
7. 验证 Server/Python Worker health、metrics、maintenance、Server 资源、Docker ID、GPU、端口、
   固定模型和 Git 基线，随后封存 watchdog manifest。

若无法证明容器 ownership 或无法把 GPU/端口清到安全基线，watchdog 必须报告 recovery failure，
不得在 Rust-owned workload 仍可能占用 GPU 时启动 Python Worker制造双 Worker竞争。

## 10. 正常 cleanup 与独立事后审计

正常路径先 enable maintenance，再通过 CLI 删除唯一测试模型并等待精确 ModelInstance、容器、
manifest/cache/log 收敛；仅在基线不存在且本 attempt 创建时删除 ModelFile。随后优雅停止 Rust
Worker，确认端口和 GPU空闲，再恢复精确 Python Worker并验证三个既有健康端点。

post-audit 必须由独立脚本重新读取现场，而不是复用 executor 的内存判定。它至少复核：

- Python Worker精确 ID/image/restart/network 与三项健康端点；
- maintenance、测试资源和 before Server ID 集合；
- before/after Docker ID 集合、Rust-owned 容器、Managed 端口和 GPU进程；
- Rust PID、data/log 精确 attempt 残留、固定模型 manifest 和 Git状态；
- formal/watchdog evidence 权限、文件数、SHA manifest 和秘密扫描。

正式结果先进入 `ready_for_review`。用户审阅接受后，才删除阶段 API Key、Context 和 operator
token 文件并证明旧 token 返回 401；凭据退役完成后才能把 M7-E.1 标记为 `accepted`。

## 11. 证据与秘密边界

- token、密码、Authorization header、Cookie 和原始登录/API Key响应不得进入日志或证据；
- Docker证据只保留允许字段，禁止记录 `Config.Env`；
- CLI/API JSON 必须先经过字段白名单过滤，再写入 `0600` 文件；
- executor、watchdog、post-audit 各自生成完成后的不可变 SHA-256 manifest；封存后不得追加日志；
- evidence secret scan 必须覆盖常见 bearer、API key、password、cookie 和 token模式；
- 聊天报告只引用资源 ID、状态、路径和 SHA，不复制任何 secret 值。

## 12. 当前停止点

本方案已接受，attempt 1 五份脚本在提交
`d549825c1a0069ee75ea56895eda9d58f0e87551` 完成静态审计：本机与 90 的 `bash -n`、90 上
ShellCheck 0.9.0、必需/禁止模式、Git identity 和只读现场隔离门全部通过。当前停在
`credential_pending`；准备短期凭据和运行 preflight/无写注入必须取得下一项单独授权。任何
真实服务、Server、Docker、GPU 或故障注入操作仍需要后续独立、精确的 M7-E.1 maintenance
authorization。
