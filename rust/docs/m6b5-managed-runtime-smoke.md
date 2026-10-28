# M6-B.5 Managed 单 GPU 正式 smoke

## 目的与结论边界

M6-B.5 在 90 服务器上以宿主机 Rust Worker 和 Docker vLLM 闭合第一次正式 Managed
单 GPU 路径，证明用户只通过 CLI 和 Server 即可部署模型、读取日志、完成推理并删除全部
测试资源。它只覆盖同进程、单 Worker、单 GPU、TP=1 的 R1 路径，不证明 Worker 崩溃恢复、
多 GPU、跨 Worker、Kubernetes、restart-on-error 或日志轮转；这些能力仍属于 M7/M8。

## 三段门禁

1. **M6-B.5.0 方案冻结**：只更新治理文档，不创建凭据、不写 Server、不改变服务或 GPU。
2. **M6-B.5.1 凭据准备与最终预检**：用户创建短期 API Key并直接写入 90 的受保护文件；
   只执行认证 GET、构建、执行器静态检查、故障注入和外部只读漂移门。不得停止 Python
   Worker、改变 maintenance、创建模型/容器或使用 GPU。
3. **M6-B.5.2 正式维护窗口**：只有 M6-B.5.1 通过、执行器和 watchdog 绑定到干净治理
   HEAD，并且用户再次明确授权后才能执行。授权必须明确允许停止并恢复精确 Python Worker、
   改变 Worker 5 maintenance、创建和删除唯一测试模型/ModelInstance/ModelFile、创建和删除
   唯一 Rust-owned vLLM 容器、使用 GPU0 并通过 Server 执行推理。

任何较早阶段的“同意”都不能替代 M6-B.5.2 的独立维护窗口授权。

## 固定身份和输入

| 项目 | 固定值 |
|---|---|
| 分支 | `codex/rust-worker` |
| 产品代码 | `c78cfc5652808ee5c16b40b11df411dadc7d6b6d` |
| Rust | `1.99.1` |
| Server | `v2.2.3`，commit `1cfcbbf` |
| Worker | ID `5`，name `rust-m3d-r760xa`；进入窗口前必须处于 maintenance |
| Worker 地址 | `10.213.80.90:20150`，metrics `10.213.80.90:20151` |
| Worker 数据目录 | `/home/wujiaxing2/.local/state/gpustack-rust-worker/1` |
| 模型根 | `/home/wujiaxing2/flowscale-system-validation/models` |
| 模型 | `Qwen/Qwen2.5-0.5B-Instruct` revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 模型路径 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct` |
| Model name / served name | `m6b5-qwen25-05b-gpu0` |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| GPU | index `0`，UUID `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| 参数 | TP=1、max model length 2048、GPU memory utilization 0.5、dtype auto、eager、无 trust-remote-code |
| Managed 端口 | Worker 从 `40000-40063` 分配；执行器不预留或指定具体端口 |
| watchdog | 独立进程，硬超时 1200 秒 |

正式执行器必须绑定到包含本手册和准备记录的后续干净治理 HEAD；该治理提交不能改变产品
代码身份。进入窗口前重新记录精确 Python Worker container ID/image/restart policy/network/
mount，并只允许停止和恢复该对象。若身份相对 M6-B.4 漂移，窗口不得进入。

## 阶段专用凭据

- API Key name：`rust-worker-m6b5-operator-20261028`；
- scope：完成 Worker maintenance、Model、ModelInstance 和 ModelFile 运维所需的平台管理权限；
- expiry：从创建起不超过 24 小时；
- Context name：`m6b5-operator`；
- token file：`/home/wujiaxing2/.config/gpustack-rust/m6b5-operator-token`；
- token 文件和 Context 存储均须为 0600，父目录为 0700；
- token 只能由用户直接粘贴进 90 文件，不通过聊天、命令历史、日志或证据文件传递；
- 证据只记录 API Key ID、名称、scope、expiry、文件元数据和认证 principal，不记录 token；
- 不能把 Cluster token 当 operator token，不能恢复或复用 M6-A 已退役的 Key；
- M6-B.5 验收后删除 API Key、Context 和 token 文件，并证明旧 token 返回 401。

## 证据路径

attempt 1 使用以下尚未创建的路径：

- 准备门：`~/.local/state/gpustack-rust-worker/evidence/m6b5-preflight-attempt-1`；
- 正式执行：`~/.local/state/gpustack-rust-worker/evidence/m6b5-managed-runtime-smoke-attempt-1`；
- watchdog：`~/.local/state/gpustack-rust-worker/evidence/m6b5-managed-runtime-watchdog-attempt-1`；
- 独立事后审计：`~/.local/state/gpustack-rust-worker/evidence/m6b5-managed-runtime-post-audit-attempt-1`。

目录须为 0700，文件须为 0600；每条证据链独立生成 SHA-256 清单并执行秘密扫描。失败和
无效 attempt 不重用目录，也不覆盖原始结果。

## M6-B.5.1 准备门

准备门必须证明：

1. 本地、GitHub 和 90 治理 HEAD 一致，90 工作树干净，产品 commit 是 HEAD 祖先；
2. Rust 1.99.1、固定 `Cargo.lock` 和 locked/offline build 通过；
3. 新 Context 只读认证成功，Key identity、scope 和 expiry 符合本手册；
4. Server、Python Worker health/metrics 正常，Worker ID 5 唯一且保持 maintenance；
5. 唯一 Model name、ModelInstance、ModelFile 和 Rust-owned 容器不存在；
6. 固定 Runner、Qwen revision/manifest、GPU0、端口和容器基线与 M6-B.4 一致；
7. Python Worker精确恢复材料、Rust 二进制 identity、PID 校验方式和日志目录就绪；
8. 正式执行器和 watchdog 通过静态审计、无写故障注入、秘密扫描与证据清单门；
9. 准备前后 Server 资源、容器、服务、GPU、端口和进程完全一致。

通过后状态只能是 `ready_for_maintenance_authorization`，不能自动进入正式窗口。

## M6-B.5.2 正式顺序

### 1. 最终漂移门与 watchdog

- 重新执行 M6-B.5.1 的身份、服务、资源、模型、GPU0、端口和冲突检查；
- 保存全局 Model/ModelInstance/ModelFile、全部 Docker ID、GPU 和端口基线；
- 先启动独立 watchdog，再停止 Python Worker；
- watchdog 与主执行器不能共享同一 SSH 生命周期，硬超时 1200 秒；
- 未完成全部基线时发生失败，finalizer 只能复核和恢复服务，不能依据不完整 ID 集删除资源。

### 2. 切换 Worker 实现

- 精确停止预检记录的 Python Worker container ID；
- 以固定二进制、Worker ID 5 的持久身份和 `worker run --managed-runtime` 启动 Rust Worker；
- Rust Worker启动时保持 maintenance；
- `/healthz`、`/readyz`、`/metrics`、Server identity、Worker API 和两张 GPU 上报全部通过后，
  才能用 `m6b5-operator` 显式解除 maintenance。

### 3. CLI 部署与完整状态链

唯一部署必须等价于：

```text
model deploy m6b5-qwen25-05b-gpu0
  --worker rust-m3d-r760xa
  --local-path /home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct
  --image <固定 Runner digest>
  --gpu 0
  --tensor-parallel-size 1
  --max-model-len 2048
  --gpu-memory-utilization 0.5
  --dtype auto
  --enforce-eager
  --wait
```

不得传 `--trust-remote-code`，不得直接指定 Managed 端口。CLI transition sink 或 Server watch
必须保存以下完整状态链：

```text
PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING
-> DOWNLOADING -> STARTING -> RUNNING
```

vLLM `/v1/models` 健康时间必须早于或等于 Server `RUNNING` 写回；容器 running 本身不能作为
模型就绪证据。

### 4. Server-only 推理和日志

所有用户操作都通过 active Context 的 Server 执行，不允许直接请求 vLLM 端口：

- `inference models` 包含唯一模型名；
- 非流式 chat 返回成功且 `choices[0].message.content` 非空；
- 流式 chat 至少产生一个有效 SSE data chunk 并正常结束；
- completion 返回成功且 `choices[0].text` 非空；
- 错误模型名由 Server 拒绝，不能绕过模型路由；
- `model logs --tail` 返回当前 restart 0 的 vLLM 日志；
- `model logs --follow` 使用有界外层 timeout 采集日志后主动结束，不遗留跟随进程；
- 日志和证据不得包含 operator token、Cluster token 或 Worker系统认证头。

### 5. 同进程重复协调幂等

Managed reconciler 每 2 秒协调同一 desired snapshot。模型进入 `RUNNING` 后等待至少 6 秒，
覆盖至少三轮重复协调，并比较：

- Model、ModelInstance 和 assigned port 不变；
- 精确 vLLM container ID、spec hash 和完整 ownership labels 不变；
- Rust-owned 容器数量始终为 1；
- route 持续可用，推理仍成功；
- 没有重复 create/start 或第二个日志 session。

不通过修改 Server、重启 Worker或人工重复创建模型来制造事件；crash/restart 恢复留给 M7。

### 6. 精确清理与恢复

完成推理后按以下顺序执行：

1. 先为 Worker 5 重新启用 maintenance，阻止新调度；
2. 删除精确 Model ID，并等待其 ModelInstance 权威 404；
3. 保持 Rust Worker运行，等待协调器删除完整 owned tuple 的 vLLM 容器、route、cache、
   manifest 和当前 session 日志；
4. 若 Server 保留独立 local-path ModelFile，核对 ID、Worker、source、canonical path 后使用
   `model file delete <id> --yes` 只删除元数据，宿主机模型文件必须保留；
5. 证明测试 Model/ModelInstance/ModelFile、owned container、listener、route 和 GPU0 workload
   全部为零；
6. 向精确 Rust PID 发送 SIGINT，等待优雅退出；必要时只对同一 PID/starttime/exe 使用
   SIGTERM，不使用名称级 kill；
7. 恢复预检记录的精确 Python Worker container ID；
8. 验证 Server/Python Worker health/metrics、Worker 5 maintenance、GPU、端口、容器和全局
   Server 资源集合恢复基线。

## watchdog 和失败回滚

watchdog 或 finalizer 只能操作本 attempt 记录的精确身份：

- 优先恢复 Worker 5 maintenance；若 Server 不可用，不猜测写入结果并保持失败证据；
- 只删除正式执行器创建并记录的 Model ID；
- 只有 ModelInstance 已权威确认 404，且容器完整标签 tuple、Worker UUID、Instance ID、
  spec hash 和 `component=vllm` 全部匹配时，才允许兜底 stop/remove；
- 标签缺失、多匹配、Server 状态不明或 identity 漂移时 fail closed，保留资源供人工审计；
- 只终止同时匹配 PID、进程 starttime、二进制路径和 SHA-256 的 Rust 进程；
- 无论业务测试是否成功，都尝试恢复精确 Python Worker；若测试资源未清理，Python Worker
  恢复后仍保持 maintenance，避免重新调度；
- 不修改 GPU1、未知容器、非测试 Server 资源或宿主机模型文件；
- 失败 attempt 保留全部日志并标记 `FAIL` 或 `invalidated`，不得改写成 PASS。

## 验收门

M6-B.5 只有在以下条件全部成立时才能进入 `ready_for_review`：

- 完整状态链和健康先于 `RUNNING` 的证据通过；
- Server-only models、buffered/streaming chat、completion 和 logs 通过；
- 至少三轮重复协调保持单实例、单容器、单端口和可用 route；
- 删除后测试 Model/Instance/File、owned runtime、本地辅助状态、listener 和 GPU0 workload
  零残留，固定模型文件仍完整；
- Python Worker、maintenance、Server、全局资源、Docker、GPU1 和未知容器恢复基线；
- 正式、watchdog 和独立事后审计三条证据链完整、权限正确、清单与秘密扫描通过；
- API Key、Context 和 token 文件在阶段验收后完成退役。

任何一项失败都不能通过缩小成功标准来接受。用户审阅详细报告前状态不得标记为
`accepted`，也不得据此进入 M7。
