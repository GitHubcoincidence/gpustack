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
| Worker 数据目录 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/m3d` |
| 模型根目录 | `/home/wujiaxing2/flowscale-system-validation/models` |
| 模型 | `Qwen/Qwen2.5-0.5B-Instruct` revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 本地路径 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct` |
| Model name | `m6a3-qwen25-05b-gpu0` |
| Runner image | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| GPU | index `0`；预检时重新核对 UUID 为 `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| 部署参数 | tensor parallel `1`、max model length `2048`、GPU memory utilization `0.5`、dtype `auto`、eager execution |
| Managed 端口池 | `40000-40063`，由 Rust Worker 分配或复用 |
| 证据目录 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m6a3-formal` |

Runner image 和 vLLM 参数仍进入 Server Model spec，保证后续 M6-B 可以沿用同类输入；
M6-A.3 不 inspect、pull 或运行该镜像。模型使用内置 chat template，不启用
`trust_remote_code`。若任何固定输入需要改变，必须先讨论并记录修订，不能在运行中临时
替换。

## CLI 前置条件

手册冻结时的静态核对发现，已接受 CLI 只有 `worker maintenance enable|disable` 和
`model file get/delete`，尚不能以只读命令取得精确 Server Worker，也不能按 Worker 列出
ModelFile。M6-A.3.0 需要这两项能力才能证明 maintenance/身份和同路径 ModelFile 无冲突；
若改用 Web UI 或未文档化原始 API 请求，会违反 D090 的正式 CLI-first 门禁。

该缺口已经通过 D103–D106 冻结为 M6-A.2.3，接口为
`worker get <worker-id-or-exact-name>` 与
`model file list --worker <worker-id-or-exact-name>`。源码与离线质量门禁已经完成，当前为
`ready_for_review`；在 D033 详细报告获用户接受前，M6-A.3.0 仍为 `not_ready`。不得用
curl 临时绕过，也不得把实现授权解释为预检执行授权。

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
   - Worker identity/token 文件存在、属主和 mode 安全；任何检查都不得输出 secret。
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
   - 正式证据目录必须尚不存在。建立时使用 `0700`，文件使用 `0600`；
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
- Rust canonicalize 固定路径，写入 `resolved_paths`、精确递归 `size`、
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
