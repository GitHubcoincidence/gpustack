# M6-A.3 Managed 文件准备真实验证手册

本文冻结 M6-A.3 的验证方法、输入、停止条件、证据和回滚顺序。它不是执行授权，也不是
一键脚本。M6-A.3.0 只读预检与 M6-A.3.1 维护窗口必须分别获得用户明确授权；预检通过
不会自动授权维护窗口。

## 验证目标与解释边界

M6-A.3 要验证的可证伪主张是：固定 GPUStack v2.2.3 Server 能把一个精确调度到 Rust
Worker GPU0 的本地 Qwen2.5 模型推进到 `SCHEDULED`；宿主机运行的
`worker run --managed-file-only` 随后完成端口分配、`INITIALIZING` 写回和本地
ModelFile 准备，Server 再把实例推进到 `STARTING`。Rust Worker 观察到 `STARTING`
后必须停止协调动作。

正式通过只证明以下状态边界：

```text
ModelInstance: SCHEDULED -> INITIALIZING -> DOWNLOADING -> STARTING
ModelFile:     downloading -> ready
```

`PENDING`、`ANALYZING` 和调度决定仍由 Server/Scheduler 所有，属于状态链上下文；正式
实现边界从 Rust Worker 收到 `SCHEDULED` 开始。M6-A.3 不创建 Docker client、容器或
vLLM，不监听推理端口、不执行推理、不产生 GPU compute process，也不把实例写为
`RUNNING`。这些能力属于 M6-B。

正式运行禁止给 `model deploy` 传 `--wait`。当前 `--wait` 只有在实例进入 `RUNNING` 时
才成功，而 M6-A.3 的正确终点是 `STARTING`；使用它会把预期结果误判成超时。

## 固定输入

| 输入 | 固定值 |
|---|---|
| Server | GPUStack `v2.2.3`，commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` |
| Rust Worker | `rust-m3d-r760xa`，复用受保护的持久 identity 与 per-Worker token |
| Worker 地址 | `10.213.80.90:20150`，metrics `10.213.80.90:20151` |
| Worker 数据目录 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/1`，固定 Worker ID 5、UUID `7c4e07a3-6b97-4577-9ff0-1fc6c2319596` |
| 模型根目录 | `/home/wujiaxing2/flowscale-system-validation/models` |
| 模型 | `Qwen/Qwen2.5-0.5B-Instruct` revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 本地路径 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct` |
| Model name | `m6a3-qwen25-05b-gpu0` |
| Runner image | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| GPU | index `0`；预检时重新核对 UUID 为 `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| 部署参数 | tensor parallel `1`、max model length `2048`、GPU memory utilization `0.5`、dtype `auto`、eager execution |
| Managed 端口池 | `40000-40063`，由 Rust Worker 分配或复用 |
| 第一次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal`，已保存停止证据，不得修改 |
| 第二次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-2`，已保存 identity mismatch 停止证据，不得修改 |
| 身份恢复证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-identity-recovery-20261026`，已保存 ID 5 token 只读验证与权限收紧证据，不得修改 |
| 第三次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-3`，已封存无效 CLI 调用证据，不计分且不得修改 |
| 第四次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-4`，已保存 GPU0 被既有 Lab 占用的停止证据，不得修改 |
| 第五次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-5`，因 Cargo working directory 错误按非评分 `INVALIDATED` 封存，不得修改 |
| 第六次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-6`，已通过并封存；`manifest.sha256` 文件 SHA-256 为 `984919960979ff8d0df503da0a0355d5b703850ad162bd4f585e0c4a0ea04c59`，不得修改 |
| 第七次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-7`，已通过并封存；`manifest.sha256` 文件 SHA-256 为 `f018b990caf44cf2201101d6468aebb2421e2b978eae69d170b70b3c58ef622a`，不得修改 |
| 第八次预检证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal-attempt-8`，已通过并封存；`manifest.sha256` 文件 SHA-256 为 `2acf1400baea3948a5e823ea91100c8728431d500794cd7822053f104b5ff777`，不得修改 |
| 第一次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-1`，已在维护窗口前以 `STOP/persisted_worker_id_mismatch` 封存；`manifest.sha256` 文件 SHA-256 为 `acca6ea071ebae4ed3c52d7bf32b81fb85ed7dca375671c3bcd19f240283999c`，不得修改 |
| 第二次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-2`，已在维护窗口前以 `STOP/persisted_worker_name_mismatch` 封存；`manifest.sha256` 文件 SHA-256 为 `5777af9731fe639a10840217f02f598d3e0ed03b6b2ff62f5ba006db1cd17b0a`，不得修改 |
| 第二次 smoke watchdog 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-2-watchdog`，清单文件 SHA-256 为 `220c46ec3d01dddc762d2219e0bc00b379d57091e10cb397d1bbed088f48911b`，不得修改 |
| 第三次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-3`，已在维护窗口内以 `FAIL/maintenance_window_or_cleanup_failed` 封存；`manifest.sha256` 文件 SHA-256 为 `e7d3d05710ebb5755a363658daf7580fcbc0701065ee49ca9b33873347a6e180`，不得修改 |
| 第三次 smoke watchdog 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-3-watchdog`，清单文件 SHA-256 为 `a51a74bbb9e3a1fec29bb512d0527dd3813f0133f6f4446f8b6e111ba879c324`，不得修改 |
| 第四次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-4`，已在维护窗口内以 `FAIL/rust_worker_startup_gate_timeout` 封存；`manifest.sha256` 文件 SHA-256 为 `bb259eb9b0162c86f24417f1ae20b28091d6a15d0ac8f0daf4446cc8d7775e5e`，不得修改 |
| 第四次 smoke watchdog 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-4-watchdog`，清单文件 SHA-256 为 `65495a2d2835f07bf8ff09628035e622ded1e57905e857462538f6f060b601ae`，不得修改 |
| 第五次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-5`，已在维护窗口内以 `FAIL/rust_worker_startup_gate_timeout` 封存；`manifest.sha256` 文件 SHA-256 为 `139821ac350946463a1413da0f455decb8172ab006f02d468004122407ecfb2b`，不得修改 |
| 第五次 smoke watchdog 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-5-watchdog`，清单文件 SHA-256 为 `4ed10c279a5afb982885a64172c3b9805be0b7767e1dae830550c28afead8d60`，不得修改 |
| 第六次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-6`，已在维护窗口内以 `FAIL/maintenance_window_or_cleanup_failed` 封存；`manifest.sha256` 文件 SHA-256 为 `79c532457a5106eb7e81829c19d81fb682cdb2335fc922467da454d74419bd1d`，不得修改 |
| 第六次 smoke watchdog 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-6-watchdog`，清单文件 SHA-256 为 `5c456c399188bbe2a6097831a03ef7f19eb860f7e390c12862edb54cda418b94`，不得修改 |
| 第七次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-7`，已在维护窗口内以 `FAIL/state_chain_did_not_reach_starting` 封存；`manifest.sha256` 文件 SHA-256 为 `81e34f127d853eaf8bd5e326e071156a3fd539f958c08ab82371aecc48d0a43e`，不得修改 |
| 第七次 smoke watchdog 证据 | 未创建；正式 executor 在 watchdog 时限内完成失败恢复和封存 |
| 第八次预窗口故障注入 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-8-pre-window-injection`，以 `STOP/source_head_mismatch` 封存；`manifest.sha256` 文件 SHA-256 为 `e94f833beceb96bf3ba881cc4619803d31ba9cf42dc35650f07b8679fa732262`，不得修改 |
| 第八次真实 smoke 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-8`，D144 已冻结但尚未创建 |
| 第八次 smoke watchdog 证据 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-managed-file-smoke-attempt-8-watchdog`，D144 已冻结但尚未创建 |

Runner image 和 vLLM 参数仍进入 Server Model spec，保证后续 M6-B 可以沿用同类输入；
M6-A.3 不 inspect、pull 或运行该镜像。模型使用内置 chat template，不启用
`trust_remote_code`。若任何固定输入需要改变，必须先讨论并记录修订，不能在运行中临时
替换。

## Operator Context 准备门禁

固定 v2.2.3 的 `/auth/login` 只创建 session cookie；Rust CLI 则固定用 Bearer 认证，而
Server 把 Bearer 解释为 API Key。因此不能把 login JWT、Cluster registration token 或
per-Worker token 写入 Context。正式 Context 使用唯一
`rust-worker-m6a3-operator-20261024` API Key，scope 仅为 `management`，有效期 7 天。

用户在 90 主机终端无回显输入本地管理员用户名和密码。登录表单、cookie jar 和包含完整
API Key 的创建响应只能存在于 mode 0600 临时文件；token 值不得进入参数、终端输出、聊天、
Git 或正式证据。最终 token 文件固定为
`/home/wujiaxing2/.config/gpustack-rs/credentials/m6a3-operator.token`，Context 名为
`lab90`，Server URL 为 `http://127.0.0.1/`。创建后只允许用 `context list` 和
`worker get rust-m3d-r760xa` 验证 Context 与只读管理权限，随后逐文件删除临时 secret。

2026-10-26 的实际恢复没有复用错误输入的本地密码：用户通过既有已认证 Web 管理会话创建
API Key，并把完整值直接无回显写入 90 主机的 mode 0600 文件。网页表象与 API 权威元数据
不一致时，以 API 返回为准；最初的永久、`management + inference` Key 仅作为一次性凭据，
通过 30 分钟、仅 `management` 的桥接 Key 原子替换为同名 7 天、仅 `management` 的正式
Key，随后删除原 Key、桥接 Key 和全部临时 secret。该恢复路径不改变最终凭据契约。

该门禁的唯一 Server 写入是创建上述 API Key；它不修改 Worker、maintenance、Model、
ModelInstance 或 ModelFile。准备成功不等于 M6-A.3.0 通过，也不自动授权第二次预检。
M6-A.3.1 完成、项目中止或凭据不再需要时必须删除 Server API Key、Context 和 token 文件。

## CLI 前置条件

手册冻结时的静态核对发现，已接受 CLI 只有 `worker maintenance enable|disable` 和
`model file get/delete`，尚不能以只读命令取得精确 Server Worker，也不能按 Worker 列出
ModelFile。M6-A.3.0 需要这两项能力才能证明 maintenance/身份和同路径 ModelFile 无冲突；
若改用 Web UI 或未文档化原始 API 请求，会违反 D090 的正式 CLI-first 门禁。

该缺口已经通过 D103–D106 冻结为 M6-A.2.3，接口为
`worker get <worker-id-or-exact-name>` 与
`model file list --worker <worker-id-or-exact-name>`。源码、离线质量门禁和 D033 详细报告
已经用户接受；首次 M6-A.3.0 执行前状态为 `ready_for_authorization`。不得把 M6-A.2.3
的接受解释为预检执行授权，也不得用 curl 临时绕过冻结命令。

第一次 M6-A.3.0 已因缺少 Context 停止；正式 Context 随后准备并通过只读 Worker 查询。
第二次 M6-A.3.0 于 2026-10-26 获授权并使用 `m6a3-formal-attempt-2`，但本地持久 Worker
ID 2 与 Server 当前同名 Worker ID 5 不一致，因此在 identity 门禁按协议停止。该目录现已
占用且不得复用或覆盖。随后按 D114–D116 接受现有 `/1` 目录为当前 ID 5 的固定数据目录：
其 token 通过 `GET /v2/workers?me=true` 返回唯一 ID 5/name，本地 identity 提供 UUID，
顶层目录权限已从 0775 收紧为 0700；旧 `m3d` 中 ID 2 未改动。第三次预检固定使用
`m6a3-formal-attempt-3`，但身份恢复成功不是第三次预检的执行授权。该次执行完成冻结构建
后，`context list` 因被错误传入全局 `--context` 而在本地参数检查阶段拒绝；没有读取 token
或发送该 CLI 请求，后续身份、资源、模型、GPU、端口和 Docker 门禁均未执行。attempt 3
按 `INVALIDATED` 非评分封存，不能在同一目录修改命令重跑；下一个路径必须另行冻结。

D117–D119 随后把第四次证据固定为 `m6a3-formal-attempt-4` 并获得只读执行授权。本地 Context
枚举必须使用不带 selector 的 `gpustack-rs --output json context list`；Worker、Model 和
ModelFile 等真实 Server 查询继续显式使用 `--context lab90`。该修正不改变 Context 内容、
认证凭据或任何正式输入。

第四次预检随后通过源码/构建、Context、ID 5 identity、目标资源零冲突和固定模型 manifest
门禁，但 GPU0 上存在既有 Rust Lab `qwen2.5-0.5b-m5e`：其容器使用固定 Runner，唯一
compute process 占用约 76 GiB 显存。该 Lab 不属于 Server ModelInstance，也不是 attempt 4
创建的资源。预检按 `STOP/gpu0_not_idle` 失败关闭；后续端口、Python Worker Docker 恢复
基线、回滚 CLI 和全局资源 ID 门禁未执行。attempt 4 不得修改或继续，Lab 处置和下次证据
路径必须另行讨论授权。

用户随后确认该 Lab 是此前测试部署，并自行停止和删除。只读复核确认精确容器和部署状态
文件均不存在，GPU0 显存与利用率为 0、没有 compute process；固定 Qwen2.5 模型目录仍
存在，Server、Python Worker health/metrics 正常，Rust Worker 进程和端口均无残留。
这项外部清理不修改 attempt 4 证据，也不把其追溯改判为通过。下一次预检必须使用新的
未占用证据目录，并在重新执行全部门禁前另获明确授权。D120–D122 已将第五次目录固定为
`m6a3-formal-attempt-5` 并授予只读执行权限；attempt 5 必须从头重跑所有检查，授权仍不
包含任何 M6-A.3.1 写操作。

attempt 5 实际执行时，预检脚本在仓库根目录而非 `rust/` workspace 调用
`cargo clean -p gpustack-cli`，Cargo 因找不到 `Cargo.toml` 以退出码 101 拒绝。该错误发生
在源码 identity 记录之后、正式构建和真实查询之前；证据按
`INVALIDATED/invalid_cargo_working_directory` 非评分封存。不得在 attempt 5 内改目录继续，
下一次必须冻结新路径、先验证脚本 working directory，并重新取得执行授权。

D123–D125 已把 attempt 6 路径冻结为 `m6a3-formal-attempt-6` 并授予只读执行权限。Cargo
命令改为显式使用 `$REPO/rust/Cargo.toml`；正式目录创建前须完成 `bash -n`、
`cargo metadata --manifest-path ...` 和与 attempt 5 脚本的窄范围差异审计。证据日志必须
同步落盘，seal 后不得再改变已进入 manifest 的文件；attempt 5 manifest 也必须加入前序
证据 identity 检查。上述修正不改变任何正式输入或验收条件。

attempt 6 的目录外静态审计与完整预检随后全部通过。44 个 manifest 条目逐项校验成功，
证据目录为 0700、45 个文件均为 0600，秘密扫描为 0。固定源码/二进制、Server/Context、
ID 5 identity、目标资源零冲突、模型 manifest、GPU0、端口、Python Worker 精确恢复材料、
回滚 CLI 和全局 ID 基线均通过；运行没有改变 maintenance、停止 Python Worker、启动 Rust
Worker、写 Server 资源或创建 Docker/GPU workload。M6-A.3.0 因而为 `passed_preflight`，
M6-A.3.1 仅为 `ready_for_authorization`，仍未获得维护窗口授权。

用户随后接受 D126–D128，授权第一次 M6-A.3.1 维护窗口。attempt 1 只允许执行本文固定的
停止/恢复精确 Python Worker、Worker ID 5 maintenance 切换、唯一测试资源创建/清理和
`--managed-file-only` 生命周期；不允许 Docker/vLLM workload、GPU compute 或推理。
正式执行前必须重新通过外部漂移门禁，不能仅复用 M6-A.3.0 的历史快照。

attempt 1 的静态审计和外部漂移门禁通过，但执行器随后把
`managed/worker-identity.json` 的 `worker_id` 字段错误读取为 `.id`，在资源基线和维护窗口
之前以 `STOP/persisted_worker_id_mismatch` 退出。由于 `before-global-ids.json` 尚未生成，
通用 cleanup/final-check 同时把 `cleanup_ok` 记为 0；独立审计确认没有停止 Python Worker、
没有切换 maintenance、没有创建测试资源、没有启动 Rust Worker，也没有端口、Docker 或
GPU 残留。attempt 1 证据保持不可变。任何 attempt 2 必须先修正 identity 字段和
pre-window cleanup 的基线前分支，冻结新证据路径，并重新取得维护窗口授权。

正式 seal 后才生成的 watchdog log/PID/exit 使用独立补充目录
`m6a3-managed-file-smoke-attempt-1-watchdog-2` 保存，其 `manifest.sha256` 文件 SHA-256 为
`c16b337f85b0b14a96f22277bb74bb65a200bb94fa2a2e57d3dadbafc487934f`。此前同名不带 `-2`
的补充目录因零匹配秘密扫描与 `pipefail` 组合而中断，未 seal、不得作为通过证据，也不得
覆盖；`watchdog-2` 已记录其文件哈希。该补充封装失误不改变正式 attempt 1 的 STOP 判定。

D129–D131 将 attempt 2 的执行器修正限制为两点：identity 使用权威 `worker_id`；增加
`baseline_ready`，只有全局 ID、Docker、Worker 和模型基线都写入后才允许执行依赖 baseline
的 cleanup/final comparison。此前发生 STOP 且尚未进入窗口时，只执行无写安全复核并封存，
不得把“基线尚不存在”误报为清理失败。基线完成或进入窗口后仍执行本文完整回滚，不得因
该修正跳过资源清理。当前只允许准备和只读审计，attempt 2 维护窗口尚未授权。

attempt 2 的准备门禁通过后，用户按 D132 明确授权一次真实维护窗口。正式执行在
`identity_resource_and_environment_gate` 发现脚本把权威 `worker_name` 错读为 `.name`，以
`STOP/persisted_worker_name_mismatch` 在基线和维护窗口之前停止。该文件的实际值与 Server
一致；这是执行器字段错误，不是 Worker identity 漂移。结果为 `baseline_ready=0`、
`window_entered=0`、`cleanup_ok=1`，无 Python Worker stop、maintenance 写入、测试资源、
Rust Worker、Docker/GPU workload 或推理。正式目录 0700、30 个文件均为 0600，29 个
manifest 条目通过，秘密扫描为 0；watchdog 目录 0700、7 个文件均为 0600，6 个 manifest
条目通过，秘密扫描为 0。事后复核确认服务健康、Worker 仍为 maintenance、目标 Model 和
ModelFile 均为 0、受管端口和 GPU0 空闲。下一次尝试只能使用新证据目录，并把读取窄修正为
`.worker_name`；不得修改 attempt 2 或沿用其执行授权。

D133–D135 随后把 attempt 3 固定为新证据目录，并把 executor 差异限制为 `.name` 到
`.worker_name`、attempt/路径/提交和脚本 identity 更新。用户已授权：静态审计、
pre-window failure injection 和完整只读漂移门禁全部通过后，可执行一次本文定义的维护
窗口；允许的写操作和禁止项与 D135 完全一致。前置门禁失败不得创建正式目录，正式运行失败
不得自动重试。

attempt 3 的静态、failure injection 和只读漂移门禁全部通过后进入维护窗口。Python Worker
stop 成功，但 `rust_worker_start_gate` 中两个 probe 把 `curl` 直接放在命令替换赋值里；在
顶层 `set -e` 下，API 尚未监听时第一次连接拒绝返回 7，导致 120 次 readiness 重试尚未生效
就进入 cleanup。Rust Worker 随后实际启动并上报 ID 5、maintenance-required 与两张 GPU，
但未解除 maintenance，也未创建测试模型。统一 cleanup 以精确 PID 停止 Rust Worker并恢复
原 Python Worker，前后全局资源 ID、Docker 和模型 manifest 一致，`cleanup_ok=1`。正式目录
0700、60 个文件均为 0600、59 条 manifest 通过、秘密扫描为 0；watchdog 目录 0700、7 个
文件均为 0600、6 条 manifest 通过、秘密扫描为 0。后续尝试只能将两个 probe 改为连接未就绪
时不触发 `errexit` 的可重试采样，并使用新目录和新授权。

D136–D138 将 attempt 4 的变化限制为两个 readiness probe 增加 `|| true`，恢复原 120 次
轮询语义。正式目录创建前除既有门禁外，必须用合成 probe 证明“curl 7、curl 7、HTTP 200”
在 `set -e` 下不会提前退出，并审计其他等待预期瞬态状态的命令。用户同时授权在严格不改变
产品代码、固定输入、资源/维护范围、超时、成功标准和回滚协议时，自主修正同类执行器缺陷；
每个正式 attempt 仍必须使用唯一目录、保留失败历史并先证明上次完整恢复。

attempt 4 通过全部增强准备门后进入维护窗口，并证明 Rust Worker 正常监听冻结 Worker IP
`10.213.80.90:20150/20151`；但执行器仍探测 `127.0.0.1`，该地址返回 000，120 次循环后按
`rust_worker_startup_gate_timeout` 失败。cleanup 通过且没有模型、Docker/GPU workload 或推理。
D139 因此把 attempt 5 的唯一变化限定为两个 readiness probe 使用 `10.213.80.90`；正式运行
前必须同时证明 Worker IP 路径可达、loopback 不可达，并重跑既有全部门禁。

attempt 5 通过地址门并在 Worker IP 上返回 200，但 Server 在 maintenance 开启时按协议返回
`state=maintenance`，而执行器错误要求 `ready`，因此再次按启动超时失败；恢复和零残留审计
通过。D140 将 attempt 6 的唯一行为修正限定为要求 `state=maintenance`、
`maintenance.enabled=true`、`unreachable=false` 和双 GPU。正式前已整体排练 CLI Managed、
coordinator、事件格式以及状态链/清理表达式；仍须使用新目录并重跑全部门禁。

attempt 6 首次通过启动门并解除 maintenance，但固定 `model deploy` 在创建 Model 前被 Server
以 HTTP 400 拒绝：手工 GPU 选择需要 Model `cluster_id`。目标 Worker 的权威 `cluster_id=1`，
而 Rust `ModelCreate` 当前没有该字段，CLI 也没有先解析 Worker 集群。cleanup 已恢复
maintenance/Python Worker且零残留；这属于产品契约缺口，必须先共同接受代码修正并完成回归
验证，不能作为执行器问题直接冻结 attempt 7。

D141 已在 `208ac76dfd78beca4c697afd05235e0c974bdd8d` 修复该缺口：CLI 从唯一目标 Worker
推导非空集群并写入 ModelCreate，无集群时在 POST 前失败；全 workspace 150 个离线测试及
相关 Clippy 通过。该结果只解锁 attempt 7 的无中断准备，不自动授权维护窗口。

D142 固定 attempt 7 继续使用同一输入、状态链、超时、成功标准和 cleanup；只更新新路径、
HEAD/代码/预检 identity、attempt 编号，并修正遗留的 cleanup 提示文本。只读预检、预窗口
故障注入和外部漂移门已通过；停止 Python Worker和真实运行仍需单独授权。

attempt 7 在授权后成功创建 Model 10/Instance 25，但实例始终为 `pending`，调度器报告目标
Worker 未通过 READY 门。Rust Worker 在 maintenance 解除后仍上报非空受限模式
`state_message`，固定 Server 因而保留 `not_ready`。300 秒门失败后 cleanup 完整恢复且没有
ModelFile、端口、Docker/GPU workload 或推理。D143 只修正 `managed-file-only` 的健康报告为
空 `state_message`；`resource-api-only` 仍保持不可调度，其他正式输入和成功标准不变。新的
正式尝试必须使用新目录并重新取得维护窗口授权。

D143 已在代码检查点 `85d6b8f478de2b096e35d0c22ddaa8ab1768fa39` 通过目标回归、150 个
workspace 测试、全 workspace/all-targets Clippy 和格式门。D144 固定 attempt 8 使用新的
预检、正式和 watchdog 路径，并要求重新完成全部静态、故障注入和只读漂移门；准备通过仍不
授权真实维护窗口。

## M6-A.3.0：独立只读预检

### 目的

在不修改 Server、Worker、Docker、端口或 GPU 状态的前提下，证明正式运行的输入唯一、
可恢复且无已知冲突。预检允许在 Git 外建立受保护的证据目录并从干净源码构建正式
二进制；它不启动 Rust Worker，不改变 maintenance，不停止 Python Worker，不创建或删除
任何 Server 资源。

### 检查组

1. **源码和二进制 identity**
   - 本机、fork 和 90 工作树处于同一 `codex/rust-worker` 提交，90 工作树干净；
   - M6-A.2.2 实现提交 `f19a8fcd41ed8368931509ae3120a6b0d7098043` 是当前提交祖先，
     其后的差异不包含未接受的 Rust 代码或 `Cargo.lock` 漂移；
   - 使用仓库固定 Rust 1.99.1、`Cargo.lock` 和 locked/offline 依赖，从干净冻结提交构建
     `gpustack-rs`，记录 embedded commit、二进制 SHA-256 与 `Cargo.lock` SHA-256。
2. **Server、Context 与身份**
   - `/version` 精确匹配固定 Server 版本和 commit，Server health 为 200；
   - operator Context 存在，token 文件属主和 mode 安全，使用只读 CLI 请求验证权限；
   - Rust Worker 名称唯一，持久 identity 与 Server ID/name 一致，仍处于 maintenance，
     且没有分配给它的 ModelInstance 或 ModelFile；
   - 固定数据目录和 `managed/` 均为 mode 0700；Worker identity/token 文件存在、属主正确、
     mode 0600 且为单链接；任何检查都不得输出 secret；
   - ID 5 的 per-Worker token 再次调用 `me` 必须返回唯一 ID 5/name，旧 `m3d` 中 ID 2
     identity 保持原样且不得被选作当前运行输入。
3. **目标资源无冲突**
   - `m6a3-qwen25-05b-gpu0` 不存在；若重名则停止，不复用也不自动删除；
   - Rust Worker 上不存在同一 `local_path` source 的 ModelFile。已有 ready 记录会绕过
     `downloading -> ready`，因此属于正式验证冲突，预检不得删除它；
   - 记录 Model、ModelInstance、ModelFile 和 Worker 的基线 ID 集，供最终差分。
4. **模型、GPU、端口和进程**
   - 模型 canonical path 位于固定根目录内，revision、10 个顶层文件、全文件 manifest、
     999,604,126 bytes 总大小和主权重 SHA-256 与已接受证据一致；
   - GPU0 的 index/UUID/name 不漂移且没有 compute process；GPU1 只记录可归因基线，本项目
     不修改它；
   - 没有 `gpustack-rs` 进程，`20150/20151` 空闲，`40000-40063` 没有未知监听；
   - 只读记录 Docker 容器 ID/名称/镜像和 GPU 基线，不调用 Rust Docker runtime。
5. **Python Worker 可恢复性与证据安全**
   - Server、Python Worker health `10150` 和 metrics `10151` 均正常；
   - 记录精确 `gpustack-worker` container ID、名称、镜像 digest、命令、restart policy、
     network、mount 和当前状态，保证后续只停止并恢复同一对象；
   - 下一次正式证据目录必须另行冻结且尚不存在；前两次停止证据、attempt 3 无效证据、
     attempt 4 GPU 停止证据和 identity 恢复证据必须原样存在。新目录建立时使用 `0700`，
     文件使用 `0600`；
   - 固定二进制 help 中存在 maintenance、Model、ModelFile 清理入口，回滚不依赖 Web UI
     或未文档化写请求。

### 预检停止条件

任一 identity 不匹配、工作树或代码漂移、Server 版本漂移、Rust Worker 不唯一或不在
maintenance、目标 name/ModelFile 冲突、模型 manifest 不一致、GPU0 不空闲、受管端口有
未知占用、Python Worker 不健康或无法精确恢复、凭据权限不安全、证据目录已存在，均令
M6-A.3.0 返回 `STOP`。不得在同一次预检中通过删除资源、换 name、换 GPU、换模型、换
镜像或放宽检查来继续。

预检通过状态为 `passed_preflight`，只证明可以共同审阅是否进入 M6-A.3.1；它不证明
Managed 文件准备正确，也不授予维护窗口权限。

## M6-A.3.1：真实无 Docker 维护窗口

### 前置授权

只有 M6-A.3.0 通过、输出获共同审阅且外部状态未漂移后，用户才能另行授权 M6-A.3.1。
授权范围必须明确包含：停止并恢复精确 Python Worker、改变精确 Rust Worker maintenance、
创建和删除唯一测试 Model/ModelInstance/ModelFile，以及启动和停止
`--managed-file-only`。授权不包含 Docker workload、GPU workload 或推理。

### 执行顺序

1. 建立总时限 900 秒的独立 watchdog 和退出 trap，并记录运行前 identity/baseline。
2. 只停止预检记录的 `gpustack-worker` 容器；Server 保持运行。确认 Python Worker 端口
   关闭、Server 仍健康、GPU0 仍空闲，且 Rust 端口仍空闲。
3. 以固定配置启动正式二进制的 `worker run --managed-file-only`。启动门禁必须看到 Rust
   Worker 仍处于 maintenance；120 秒内要求 Worker API、metrics、控制面连接和双 GPU
   上报可观察。
4. 使用 operator Context 的 CLI 对精确 Rust Worker执行 `maintenance disable --yes`；
   不由 Worker 进程自行解除 maintenance。
5. 使用不带 `--wait` 的 `model deploy` 创建唯一固定 Model，精确指定 Worker、GPU0、本地
   路径、Runner image 和固定参数。保存创建后的 Model、ModelInstance 与 ModelFile ID。
6. 使用 `model status`、只读 Server 查询和 Rust 结构化事件组成一条按 ID 对齐的时间线；
   状态链 300 秒内必须到达 `STARTING`。
7. 在 `STARTING` 保持观察 60 秒：Rust 只能记录 no-op，不得产生后续状态写入。同期复核
   Docker、监听端口和 NVML，证明没有新容器、assigned inference port 没有 listener、
   GPU0 没有 compute process 或可归因显存增长，实例没有进入 `RUNNING` 或 `ERROR`。

### 必须出现的正向证据

- Rust 收到分配给自身的 `SCHEDULED` ModelInstance；
- Rust 从 `40000-40063` 分配或安全复用一个端口，并把 `port`、`ports` 和
  `INITIALIZING` 作为一次完整更新写回；
- Server 创建关联的本地 ModelFile，并把实例推进为 `DOWNLOADING`；
- Rust canonicalize 固定路径；目录模型的 `resolved_paths` 是只含规范化目录的单元素数组，
  目录内文件数由独立 manifest 校验；同时写入精确递归 `size`、
  `download_progress=100` 和 ModelFile `ready`；
- Server 在文件 ready 后把实例推进为 `STARTING`；
- Rust 观察 `STARTING` 并记录 no-op，60 秒内不创建 runtime 或写入 `RUNNING`。

状态证据以 Rust 脱敏结构化事件、CLI 快照和 Server 对象快照的组合为准。瞬时状态不要求
恰好被某一次人工刷新捕获，但必须能从带资源 ID 和时间戳的连续证据链证明顺序；缺失关键
迁移不能靠最终 `STARTING` 反推为通过。

## 清理和恢复顺序

正常结束和失败 trap 使用同一有界顺序：

1. 先对精确 Rust Worker 恢复 `maintenance enable`，阻止新任务进入；
2. 删除本次创建的精确 Model，并轮询确认 Model 与 ModelInstance 消失；
3. 用捕获的 ModelFile ID 执行 `model file get`，重新核对 Worker、`local_path` source 和
   固定路径；再执行 `model file delete --yes`。命令固定 `cleanup=false`，只删 Server
   元数据；
4. 重新核对 Qwen2.5 目录的 manifest、大小和关键 SHA-256，证明宿主机模型数据未删除；
5. 向精确 Rust PID 发送 SIGINT，等待有界退出摘要；必要时只升级到 SIGTERM，不按名称
   广泛 kill。确认 `20150/20151` 关闭；
6. 启动预检记录的原 `gpustack-worker` 容器，验证 Server、Python Worker health/metrics
   和 Worker 恢复；
7. 对比运行前后资源 ID 集、Docker、端口、进程和 GPU：测试 Model/ModelInstance/
   ModelFile 全部不存在，Rust Worker 保持 maintenance，没有 Rust 进程、容器、inference
   listener、端口预留或 GPU 负载残留。

如果 Server 不可用，优先停止 Rust Worker 并恢复 Python Worker；保留精确资源 ID，待
Server 恢复后再做窄范围元数据清理。不得删除未知 ModelFile、容器或监听者。Python Worker
未恢复、测试资源有残留或本地模型数据发生变化时，即使状态链曾到达 `STARTING`，整个
M6-A.3.1 仍判为失败，不能接受。

## 证据目录与秘密处理

证据至少包含：

- source HEAD、M6-A.2.2 代码祖先、embedded commit、二进制和 `Cargo.lock` SHA-256；
- Server version/commit、脱敏配置和 Worker/模型/GPU/Runner identity；
- 预检报告、运行前后资源 ID 集和模型 manifest 对比；
- 带时间戳的 `timeline.ndjson`、Rust stdout/stderr、优雅退出摘要；
- Docker、listener、GPU 的前/中/后快照；
- cleanup、Python Worker 恢复和最终零残留结果；
- 证据 secret scan 结果。

不得保存 token、密码、Authorization header、Git 私钥或完整进程环境。原始证据保留在
Git 外；Git 只记录证据路径、文件 identity、摘要、结论和相关 commit。

## 失效条件

以下任一情况都使正式运行失败或失效：

- 使用未冻结的源码、二进制、Server、模型、路径、Runner、Worker、GPU 或参数；
- 使用 `model deploy --wait`、Web UI、手工数据库修改或未文档化原始写请求完成正式步骤；
- 运行中临时换 name、模型、镜像、GPU、端口策略或超时以迁就结果；
- Rust Worker 未看到 `SCHEDULED`，关键状态链或 ModelFile 证据不完整；
- 出现 Docker 容器、vLLM、inference listener、GPU compute process、`RUNNING` 或非预期
  `ERROR`；
- Python 与 Rust Worker 在 GPU0 上同时成为活动执行者；
- 清理、恢复、零残留、模型数据保留或秘密扫描任一门禁失败；
- 将诊断、失败或只读预检 artifact 提升为正式通过证据。

未经讨论不自动重试失败的正式运行。失败现场先脱敏归档，再按精确所有权回滚；下一次
尝试必须重新检查外部漂移并获得新的维护窗口授权。
