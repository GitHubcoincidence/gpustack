# M7-E.2 vLLM crash 自动恢复验证方案

## 1. 文档状态

- 阶段：M7-E.2。
- 状态：`formal_attempt_1_failed / remediation_validated / retry_preflight_pending`。
- 已完成范围：冻结方案、实现正式工具链、准备阶段凭据、完成 formal attempt 1及独立恢复审计，
  并验证该次暴露的空 LoRA表示兼容修复。
- 当前停止边界：只允许准备 attempt 2的新证据 identity并运行无副作用质量门和 preflight；不得
  自动停止 Python Worker、切换 maintenance、创建模型或容器、使用 GPU或执行 retry。

本方案已经由用户接受，但接受不等于授权真实执行。工具链完成后仍须分别取得阶段凭据准备、
只读 preflight/无写注入和唯一 formal attempt 的精确授权。M7-E.3、M7-F 和 M8 不在本方案的
授权范围内。

## 2. 验证目标

在 GPU0 上部署固定 TP=1 vLLM workload，保持 Rust Worker、GPUStack Server 和宿主机 Docker
daemon 正常，只精确终止唯一 Rust-owned vLLM 容器，证明：

1. Rust Worker确认 runtime 终止后先撤销本地 route，并把权威 ModelInstance 从 `RUNNING`
   写为 `ERROR`；
2. `restart_on_error=true` 时首次失败使用冻结的 0 秒退避层级，把持久 `restart_count` 从 0
   增加到 1、写入 `last_restart_time`，并把同一实例写回 `SCHEDULED`；
3. 同一 Worker继续完成 `SCHEDULED -> INITIALIZING -> STARTING -> RUNNING`，复用既有 ready
   ModelFile、端口、spec 和 GPU0 assignment；
4. 旧容器按完整 ownership tuple 安全删除，只创建并启动一个新容器；新旧 container ID 与
   `StartedAt` 不同，但 Runner digest、端口、模型路径、GPU UUID、spec hash 和 ownership
   不变；
5. generation 0 与 generation 1 日志同时保留，`model logs` 读取当前代，
   `model logs --previous` 读取上一代；
6. Server-only models/chat/completion/logs 在新 runtime健康后恢复，三轮重复协调不再增加
   restart count、日志代际或容器；
7. 正常或失败路径均精确清除测试资源，恢复同一 Python Worker及 Server、Docker、GPU、端口、
   模型文件和源码基线。

M7-E.2 验证的是 **vLLM crash 后由存活 Rust Worker自动重建 runtime**。它不验证 Rust Worker
crash 接管、Server/Docker代理故障、完整真实退避阶梯、GPU1、跨 Worker恢复或容器化 Worker。

## 3. 明确不做的事情

- 不停止或重启真实 GPUStack Server、宿主机 Docker daemon、Higress或 Rust Worker。
- 不使用 M7-D 冻结的 Server TCP/Docker Unix socket代理；连接故障属于 M7-E.3。
- 不向 Python Worker、未知容器、Lab容器、非本次 owned容器或 GPU1 注入故障。
- 不使用 `pkill`、进程名、容器名、名称通配或宽标签选择器终止 workload。
- 不使用 Docker restart policy替代 Rust Worker状态机，也不手工创建 replacement容器。
- 不修改模型文件、Runner镜像、Server数据库或 restart字段来伪造恢复。
- 不把 cleanup成功、直接 vLLM恢复或新容器 running 单独提升为正式 PASS。
- 不把宿主机 Worker证据外推为 M8 容器化 Worker发布证据。

## 4. 冻结输入

| 项目 | 固定值 |
|---|---|
| Server | `http://127.0.0.1/`，v2.2.3，commit `1cfcbbf` |
| Rust Worker | ID 5，UUID `7c4e07a3-6b97-4577-9ff0-1fc6c2319596`，名称 `rust-m3d-r760xa` |
| Worker 地址 | `10.213.80.90`，接口 `eno8403`，API `20150`，metrics `20151` |
| Rust data/log | `/home/wujiaxing2/.local/state/gpustack-rust-worker/1` 与 `/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d-logs` |
| Cluster token | 既有 `/home/wujiaxing2/.config/gpustack-rust/m3d-registration-token`，只作为 Worker registration token |
| Operator Context | 待创建的 `m7e2-operator`，阶段专用，management + inference，最长 7 天，验收后立即退役 |
| Operator token 文件 | `/home/wujiaxing2/.config/gpustack-rust/m7e2-operator-token`，owner-only、0600、非 symlink、单硬链接 |
| 测试模型名 | `m7e2-qwen25-05b-gpu0` |
| 模型输入 | `Qwen/Qwen2.5-0.5B-Instruct`，revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 本地模型目录 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct`，10 文件，999,604,126 字节 |
| 模型 manifest | SHA-256 `645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6` |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| vLLM 参数 | TP=1，GPU0，max model len 2048，GPU memory utilization 0.5，dtype auto，enforce eager |
| GPU0 | `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| 禁止使用 GPU | GPU1 `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` |
| Managed 端口池 | `40000-40063`；记录动态端口，并要求重启前后严格不变 |
| watchdog | 独立 session，硬超时 1200 秒 |

源码 HEAD、Rust binary、`Cargo.lock`、Python Worker容器 identity、正式脚本 SHA-256 和 attempt
证据路径在新 preflight 中重新冻结，不提前继承 M7-E.1 的运行时值。M7-E.1 已退役的 API Key、
Context 和 token 文件不得复用。

## 5. 状态所有权与 maintenance

初次创建模型仍需遵循既有调度协议：Rust Worker在 maintenance中启动并完成身份稳定后，operator
暂时解除 maintenance，等待 Worker稳定 READY，再创建唯一模型并观察完整首次部署状态链：

```text
PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING
        -> DOWNLOADING -> STARTING -> RUNNING
```

故障注入前必须重新启用 maintenance，并证明现有 runtime、Server-only推理和直连推理均未变化。
从此时起直到 cleanup，Worker保持 maintenance，禁止新调度。

maintenance 不阻塞本次自动重启。目标 ModelInstance 已绑定 Worker ID 5；Rust Worker在确认
runtime失败后把同一实例从 `ERROR` 写回 `SCHEDULED`，随后 Managed coordinator在同一 Worker
上继续端口初始化。固定 Server v2.2.3 看到已经关联且 ready 的同一 ModelFile后，可直接从
`INITIALIZING` 推进到 `STARTING`，不会重新下载。因此正式重启子链固定为：

```text
RUNNING -> ERROR -> SCHEDULED -> INITIALIZING -> STARTING -> RUNNING
```

`DOWNLOADING` 只属于首次部署完整链；重启时必须证明 ModelFile ID、路径、大小、进度 100% 和
`ready` 状态不变，且没有创建新的 ModelFile。

## 6. 退避证据边界

一次来自健康 `RUNNING` 的故障只能真实验证首次 0 秒退避。正式证据必须同时满足：

- 故障前 runtime 已健康进入 `RUNNING`，进程内连续失败计数已清零；
- `ERROR` 后没有 `RestartPending` 的非零 delay事件；
- Rust结构化事件出现 `restart_scheduled restart_count=1`；
- Server watch随后看到 `SCHEDULED`，并在有界时间内进入启动链；
- `last_restart_time` 位于故障注入与 `SCHEDULED` 观测之间的允许时钟范围。

0、10、20、40、80、160、300 秒封顶的完整序列继续由 M7-B/M7-C 已接受的 fake/mock测试证明。
若要在真实 GPU上验证 10 秒层级，必须在 replacement尚未健康时再次制造启动失败；这是复合故障，
不属于本 formal attempt，也不得临时追加。需要时另行设计 M7-E.2b。

## 7. 正式工具与 attempt 隔离

正式 Git 源固定为后续创建的：

- `rust/scripts/m7e2/lib.sh`
- `rust/scripts/m7e2/preflight.sh`
- `rust/scripts/m7e2/executor.sh`
- `rust/scripts/m7e2/watchdog.sh`
- `rust/scripts/m7e2/post-audit.sh`

attempt 1 使用互不复用的路径：

- formal：`~/.local/state/gpustack-rust-worker/evidence/m7e2-vllm-crash-attempt-1`
- watchdog：`~/.local/state/gpustack-rust-worker/evidence/m7e2-vllm-crash-watchdog-attempt-1`
- post-audit：`~/.local/state/gpustack-rust-worker/evidence/m7e2-vllm-crash-post-audit-attempt-1`
- preflight：`~/.local/state/gpustack-rust-worker/evidence/m7e2-vllm-crash-preflight-attempt-1`
- no-write injection：`~/.local/state/gpustack-rust-worker/evidence/m7e2-vllm-crash-pre-window-injection-attempt-1`

工具副本只能从干净冻结 HEAD安装。preflight、executor、watchdog和 post-audit必须交叉验证 HEAD、
产品提交、`Cargo.lock`、binary及五份脚本 SHA。任何 retry使用全新路径和新授权，不覆盖或拼接旧
attempt证据。

## 8. 正式维护窗口顺序

### 8.1 进入窗口与建立基线

1. 保存 Server、Python Worker、maintenance、Server资源、Docker、GPU、端口、本地状态和 Git
   的 before基线。
2. 重新确认并只停止 preflight冻结的精确 Python Worker容器；不得停止 Server容器。
3. 以固定 binary、参数、data/log和 Cluster token启动宿主机 Rust Worker；冻结 PID、starttime、
   executable、binary/cmdline SHA、PGID/SID及 20150/20151 listener owner。
4. 通过三端点、新 heartbeat和双 GPU上报证明 maintenance Worker已加入控制环；随后暂时解除
   maintenance并确认稳定 READY。
5. 用 `model deploy` 创建唯一模型，等待完整首次状态链、健康先于 `RUNNING`、唯一 owned容器和
   ModelFile ready/100%。
6. 记录 Model、ModelInstance、ModelFile、端口、restart字段、容器、manifest、cache和 generation
   0 日志；完成 Server-only models/chat/completion/logs及直连 vLLM基线。
7. 重新启用 maintenance，确认 Worker控制环、runtime identity和两条推理路径均未变化。

### 8.2 crash 前不可变 identity

故障前必须冻结并再次验证：

- Model、ModelInstance、ModelFile ID，实例为 `RUNNING`、`restart_count=0`、
  `last_restart_time=null`；
- ModelFile为同一 ready/100%对象，resolved path、size和 manifest匹配；
- 唯一容器 full ID、image ID/digest、`StartedAt`、端口和 running/healthy状态；
- Worker UUID、ModelInstance ID、mode、component、schema、spec hash等完整 ownership tuple；
- 唯一 GPU assignment为 GPU0，GPU1无测试进程；
- manifest中的 container ID/spec/restart generation与 Server/Docker一致；
- 只有 `<instance>.0.log`，不存在 `<instance>.1.log`；
- Rust Worker PID/starttime/executable/binary/cmdline/PGID/SID和 listener owner未漂移；
- Server-only与直连 vLLM推理仍成功。

在发送信号前启动 Server ModelInstance watch和精确 Docker event采集，避免因状态快速变化漏掉
`ERROR` 或 `SCHEDULED`。

### 8.3 精确故障注入

executor写入 `intentional-vllm-crash` marker后，立即重新验证完整 container identity、Rust PID、
Worker listeners、Server实例状态和 GPU assignment。只有全部一致才执行：

```text
docker kill --signal KILL <exact-container-full-id>
```

随后必须证明旧容器退出码 137，Rust Worker PID及 20150/20151 listeners仍为原 identity。禁止按
容器名、进程名、端口、宽标签或 GPU PID发送信号。

### 8.4 自动恢复与状态链

正式 observer必须记录并验证：

1. 原 route撤销，ModelInstance先进入 `ERROR`；
2. Rust Worker对完整失败容器重新授权后删除旧容器；
3. `restart_count`精确变为 1、`last_restart_time`写入，实例进入 `SCHEDULED`；
4. 同一实例和端口经过 `INITIALIZING -> STARTING`；同一 ModelFile继续 ready，不重新下载；
5. 只创建一个 replacement容器，健康检查通过后才写 `RUNNING`并恢复 route；
6. Rust Worker进程 identity全程不变。

Docker event窗口必须把旧容器的 kill/die/destroy与新容器的 create/start归因到两个精确 ID；不得
出现 Docker `restart`，不得出现第二个 replacement，也不得影响 Python、未知或其他 baseline
容器。health类事件允许记录但不作为容器创建计数。

### 8.5 日志、推理与幂等

恢复后必须同时满足：

- 只有 `<instance>.0.log` 与 `<instance>.1.log`，权限均为 0600；
- serve log options只列出两代，generation 1为 current、generation 0为 previous；
- `model logs` 与 `model logs --previous` 均非空、内容身份不同，并分别对应当前/上一代；
- Server-only `inference models`、buffered chat、completion和 `model logs` 成功；
- 直连新 vLLM `/v1/models` 和 buffered chat成功；
- 连续三轮至少覆盖 6 秒的 status/container/manifest/log/restart identity均不变；
- 幂等后最后一次 Server-only buffered chat仍成功。

## 9. 正式 PASS 与失败条件

正式 PASS 必须同时满足：

1. 故障只由精确容器 SIGKILL产生，旧容器退出码 137；
2. Rust Worker PID与控制端点全程不变；
3. 有序状态子序列为 `RUNNING -> ERROR -> SCHEDULED -> INITIALIZING -> STARTING -> RUNNING`；
4. `restart_count=1`，`last_restart_time`正确，首次 0 秒退避证据成立；
5. 新容器 ID/StartedAt不同，端口、image、spec、ownership、Model/Instance/File身份不变；
6. ModelFile不重新下载，GPU0旧进程消失并出现新进程，GPU1未使用；
7. generation 0/1与 current/previous日志语义通过；
8. Server-only与直连推理恢复，三轮幂等无第二次重启；
9. 正常 cleanup、watchdog、独立 post-audit、秘密扫描、manifest复算和外部基线恢复全部通过。

以下任一情况均为正式失败，cleanup成功不能改判：

- 未观察到 `ERROR`，或从 `RUNNING`直接伪装为新 `RUNNING`；
- restart count不是精确 1，last restart缺失，或出现 generation 2；
- Rust Worker退出、重启或 listener owner变化；
- replacement重复、端口/spec/ownership/GPU漂移，或 ModelFile被重建/重新下载；
- 当前/上一代日志选择错误，或恢复后推理/幂等失败；
- 任何 Python、未知或 GPU1 workload被修改；
- cleanup、Python Worker恢复、watchdog或 post-audit失败。

若停止 Python Worker前发现源码、脚本、凭据、服务、资源、GPU或端口漂移，结果为 STOP，不得
宣称进入过维护窗口。

## 10. watchdog、cleanup 与独立审计

watchdog在独立 session中监视 executor，并预先封存 Python Worker、Rust PID、目标模型和容器
identity。executor失败或超过 1200 秒时按以下顺序回滚：

1. 尽力保持 Worker maintenance，阻止新调度；
2. 只在完整 identity匹配时终止精确 Rust PID；若进程已退出则不按名称搜索替代目标；
3. 优先通过权威 CLI删除唯一测试模型并等待收敛；
4. 若权威清理不可用，只对 attempt记录且完整 ownership/spec匹配的旧/新容器执行精确
   stop/remove；运行态与停止态都必须重新验证 identity；
5. 只清理本 attempt的 manifest/cache/log和 attempt-created ModelFile元数据，不修改本地模型；
6. 只有 Rust-owned容器、Managed端口和两张 GPU workload都回到基线后，才恢复同一 Python
   Worker，避免双 Worker竞争；
7. 重新验证 Server/Python Worker健康、maintenance、资源集合、Docker、GPU、端口、固定模型、
   Git和证据清单。

正常路径同样先保持 maintenance，再删除唯一模型并等待两代日志、manifest、cache、容器和 route
清除；只删除 baseline不存在且本 attempt创建的 ModelFile元数据。随后优雅停止 Rust Worker，
确认 GPU/端口空闲后恢复精确 Python Worker。

post-audit必须由独立脚本读取现场，不复用 executor内存结论。正式结果先进入
`ready_for_review`；用户接受后才删除阶段 API Key、Context和 token文件并验证旧 token返回 401，
随后才能把 M7-E.2 正式验证阶段标记为 `accepted`。

## 11. 证据与秘密边界

- token、密码、Authorization、Cookie和原始登录/API Key响应不得进入日志、聊天、Git或证据；
- Docker inspect只保存白名单字段，写盘前删除整个 `Config.Env`；
- API/CLI JSON写入前执行字段白名单过滤；证据目录为 0700，文件为 0600；
- formal、watchdog和 post-audit分别生成不可追加的 SHA-256 manifest；
- secret scan覆盖真实 token精确值和通用 bearer/API key/password/cookie模式；
- 聊天报告只引用资源 ID、状态、路径和 SHA，不复制 secret。

## 12. 当前停止点

M7-E.2 五份 Git 权威脚本已在提交
`b6ec4eac6c506e6d7a3ad96011c5b8b58d35f24d` 完成。本机 pre-commit ShellCheck、90 上 Bash语法、
workspace locked/offline build、9 项精确定向测试、恢复状态链 mock、双代停止容器 cleanup mock、
必需/禁止模式和运行基线不变检查全部通过；Server、Python Worker和 metrics仍为 200，未创建
Rust runtime或占用 GPU。

阶段 API Key ID 24、`m7e2-operator` Context与受保护 token文件已经准备完成；credential、
preflight attempt 1和错误 HEAD无写注入的 manifest文件 SHA-256分别为
`3030c072729685874cad12d2ae86cb6378ebdcb8e37fd3b145abe7523f48082b`、
`4f38f32c855ed094aa1608e547f20058b9f99f0a574b45e812f978e5aec61209`和
`dcddc7cf5c0c97555d7efae5b12b246d24bd7a5be2eaa9d21977189e61a94e6f`。八组 preflight门、
前后不变式、秘密扫描和独立 manifest复算通过；错误 HEAD注入按预期在 `window_entered=0`停止。

formal attempt 1 在用户明确授权下进入维护窗口。当前 HEAD 的 preflight attempt 2和错误 HEAD
无写注入通过，manifest文件 SHA-256分别为
`b9e3e26d0b907d5b762bec6f778c1cced905f3a75f28aaed4ff1c87973328aa2`和
`0bf2f6639417eda1b843e343c31525dcb3d41c99564e8061e67172f67557cb28`。旧 vLLM经完整 identity
复核后收到唯一 SIGKILL并以 137退出；Rust Worker PID和 listeners保持不变，Server状态依次到达
`ERROR -> SCHEDULED`，`restart_count=1`且 `last_restart_time`有效。随后 Worker连续拒绝该
`SCHEDULED`实例，原因是 Server把无 LoRA表示为 `mounted_loras: []`，而 R1预条件把任意非 null
表示都判为超出范围；replacement容器没有创建，正式结果固定为
`FAIL/automatic_vllm_recovery_timeout`，不得用 cleanup成功改判。

formal、watchdog和 post-audit manifest文件 SHA-256分别为
`5a5c1f6d123af25d1cbd7934325df900451bad45b26f55559fcc203599b0aef3`、
`d8b1e4791071009d18643c3ee30045d94cdb98b47be56f9912988c7ddd9714d9`和
`e78aff45d13d6abe9e2ba22a1d4c6906e5052722d1502323838371f937782652`。`cleanup_ok=1`、watchdog
`recovery_ok=1`且独立 post-audit为 PASS；同一 Python Worker、三个健康端点、Server资源、Rust
进程/容器/端口和两张 GPU均恢复基线。

提交 `d2f7044dc89db74466be96fc2fda3c3a72eb9ec9` 把空数组按“没有挂载 LoRA”处理，同时继续拒绝
非空 LoRA。90 上格式、155项 `gpustack-worker`测试和 all-targets/all-features Clippy通过。当前
状态为 `retry_preflight_pending`：下一项只允许为 attempt 2冻结新证据路径、更新 formal脚本中的
产品提交并运行新的只读 preflight/错误 HEAD注入。真实 retry仍须新的明确维护授权；attempt 1授权
已经消费，M7-E.3和 M7-F仍未授权。
