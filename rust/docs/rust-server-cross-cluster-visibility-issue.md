# Rust Server 缺陷报告：Worker SYSTEM 主体可读取其他 Cluster 资源

## 摘要

Rust Worker 使用 Cluster 3 签发的 Worker SYSTEM Token 调用 Rust Server 时，能够读取
Cluster 1 的 5 个 Model，以及属于 Worker 13～15 的 4 个 ModelFile。该结果与原始
GPUStack v2.2.3 的 Cluster SYSTEM 数据可见性规则不一致。

Rust Worker 自身对 ModelInstance 和 ModelFile 还有 `worker_id` 二次所有权检查，因此本次
未启动容器、未使用 GPU，也未修改其他 Worker 的资源；但这只能阻止运行时副作用，不能替代
Server 应执行的数据隔离。

- 报告日期：2026-11-14
- 首次确认日期：2026-11-13
- 问题归属：Rust Server
- 联调状态：阻断 M8-RS.4.2
- 建议优先级：P0 兼容性阻断；若两个 Cluster 属于不同租户，还同时构成跨租户元数据泄露风险
- 正式实验结论：`FAIL/unexpected_desired_models`，清理结果 `PASS`

## 环境与身份

| 项目 | 值 |
|---|---|
| Rust Server | `http://10.213.80.106:18091/` |
| Rust Server 版本 | `v2.2.3` |
| 实验时 Server commit | `b51893c` |
| Server implementation | `rust` |
| Server build version | `0.1.0` |
| Rust Worker | ID 16，`rust-rs3-worker-90-20261113` |
| Worker 所属 Cluster | Cluster 3 |
| Worker 主体 | Cluster 3 绑定的 SYSTEM 主体 |
| Worker 状态屏障 | `maintenance.enabled=true` |
| Rust Worker 源码 HEAD | `5f00536b29a6319b7381269b876c537051be0252` |
| Rust Worker 二进制内嵌 commit | `5f00536b29a6` |
| 兼容性权威 | 原 GPUStack v2.2.3 commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` |

## 前置条件

正式运行前已通过管理主体和 Worker 主体分别确认：

- Cluster 3 的 Model 数量为 0；
- Cluster 3 的 ModelInstance 数量为 0；
- Worker 16 的 ModelFile 数量为 0；
- Worker 16 精确属于 Cluster 3；
- Worker 16 处于 maintenance，不能参与调度；
- 目标端口和进程为空，没有 Rust vLLM 容器；
- 管理主体完整清单中的既有资源属于 Cluster 1 和 Worker 13～15，不属于本次测试身份。

因此，Worker 16 首次 full sync 的三个 desired 集合都应为空。

## 复现请求与实际表现

下列命令只展示脱敏调用形式。不要把真实 Token 写入问题单、聊天、Git 或共享日志。

### 1. Model 列表未按 SYSTEM 主体的 Cluster 隔离

请求：

```bash
curl --silent --show-error --fail \
  -H 'Authorization: Bearer <WORKER_16_SYSTEM_TOKEN>' \
  'http://10.213.80.106:18091/v2/models?page=-1'
```

期望：

- 只返回 `cluster_id=3` 或 `cluster_id=null` 的 Model；
- 当前测试数据中两类资源均不存在，所以 `items` 应为空。

实际：HTTP 200，返回 5 个 Cluster 1 Model：

| Model ID | 名称 | `cluster_id` |
|---:|---|---:|
| 1 | `qwen3-0.6b` | 1 |
| 2 | `demo-qwen25-05b` | 1 |
| 3 | `demo-qwen25-05b-new` | 1 |
| 4 | `demo-qwen25-05b-new1` | 1 |
| 5 | `demo-qwen25-05b-new2` | 1 |

直接影响：Rust Worker 的 full sync 结果变为 `desired_models=5`，而不是期望的 0。

### 2. ModelFile 列表忽略 `worker_id=16`

请求：

```bash
curl --silent --show-error --fail \
  -H 'Authorization: Bearer <WORKER_16_SYSTEM_TOKEN>' \
  'http://10.213.80.106:18091/v2/model-files?page=-1&worker_id=16'
```

期望：

- 只返回 `worker_id=16` 的 ModelFile；
- 当前 Worker 16 没有 ModelFile，所以 `items` 应为空。

实际：HTTP 200，返回 4 个其他 Worker 的 ModelFile：

| ModelFile ID | 实际 `worker_id` | source | state |
|---:|---:|---|---|
| 1 | 13 | `model_scope` | `ready` |
| 2 | 14 | `model_scope` | `ready` |
| 3 | 15 | `model_scope` | `downloading` |
| 4 | 15 | `local_path` | `ready` |

这说明 Server 接受了 `worker_id=16` 查询参数，但没有把它应用到实际列表查询。

Rust Worker 的本地二次检查丢弃了 `worker_id != 16` 的条目，所以最终
`desired_model_files=0`，没有触发文件协调或运行时动作。该防线不能把 Server 响应判定为兼容。

### 3. ModelInstance 本次没有返回越界数据

请求：

```bash
curl --silent --show-error --fail \
  -H 'Authorization: Bearer <WORKER_16_SYSTEM_TOKEN>' \
  'http://10.213.80.106:18091/v2/model-instances?page=-1'
```

实际：HTTP 200，返回 0 个 ModelInstance。

这只能说明当前数据集没有触发该接口的问题，不能证明 ModelInstance 的 list/watch 已正确实现
Cluster 和 Worker 范围。建议与 Model、ModelFile 一并审计。

## 正式 Worker 运行表现

生产入口 `worker run` 从 2026-11-13 22:50:30 +08:00 运行至 22:52:40，共 130 秒：

- 正常进入 `listening`、`identity_established`、`running`，最后经 SIGINT 优雅退出；
- `/healthz`、`/readyz`、`/metrics` 和带 Token 的 Worker API 均返回 200；
- 未认证的受保护接口返回 401；
- heartbeat、status、full sync 和三类 watch 均已建立；
- `watch_connected=true`，`reconnect_attempts=0`；
- Worker 16 在 Server 上始终保持 maintenance；
- `desired_models=5`，触发本次正式失败；
- `desired_model_instances=0`、`desired_model_files=0`；
- `file_reconciliations=0`、`runtime_reconciliations=0`；
- Docker 容器集合不变，没有启动 vLLM，没有占用 GPU。

因此，控制循环、身份恢复和运行生命周期本身正常，失败点集中在 Rust Server 返回的资源可见范围。

## 原始 GPUStack v2.2.3 的正确语义

原始实现以 commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365` 为权威。

### Cluster SYSTEM 上下文

`gpustack/api/tenant.py` 中：

- `cluster_scoped_system(ctx)`：SYSTEM 主体且 `scoped_cluster_id` 非空时返回 true；
- `_scoped_cluster_conditions(ctx, model)`：对包含 `cluster_id` 的资源生成
  `cluster_id == scoped_cluster_id OR cluster_id IS NULL`；
- `tenant_list_conditions(ctx, model)`：list 查询使用上述 Cluster 条件；
- `scoped_cluster_row_visible(ctx, resource)`：watch 事件使用相同的行级可见性语义。

也就是说，Cluster 3 的 Worker SYSTEM 主体不能读取 Cluster 1 的 Model。

### Model list/watch

`gpustack/routes/models.py` 中：

- 普通 list 调用 `tenant_list_conditions(ctx, Model)`；
- watch 通过 `_make_model_watch_filter` 调用 `scoped_cluster_row_visible`；
- list 与 watch 必须使用一致的 Cluster 可见性规则。

### ModelFile list/watch

`gpustack/routes/model_files.py` 中：

- `worker_id` 被构造成 `fields={"worker_id": worker_id}`；
- 普通 list 把 `fields` 传给分页查询；
- watch 把同一个 `fields` 传给 `ModelFile.streaming(...)`；
- 因此 `worker_id=16` 不能返回 Worker 13～15 的 ModelFile。

ModelFile 本身不依赖 `cluster_id` 列完成本次过滤；这里最直接的问题是 `worker_id` 查询条件没有生效。

## 建议检查的 Rust Server 实现位置

以下是根据 wire 表现给出的排查假设，不代表已读取 Rust Server 源码后的最终根因：

1. Worker/Cluster Token 认证完成后，认证上下文可能没有保留 `scoped_cluster_id=3`；
2. Model list 的 repository/service 层可能没有应用 TenantContext 生成的 Cluster 条件；
3. ModelFile list 虽然解析了 `worker_id`，但没有把它传到数据库 WHERE 条件；
4. watch 可能使用独立的初始快照或广播路径，未复用 list 的可见性谓词；
5. CREATED、UPDATED、DELETED 事件可能只做资源类型过滤，没有做 Cluster/Worker 过滤；
6. ModelInstance 的 list/watch 可能存在同类缺口，只是当前空数据没有暴露。

建议不要只在 Worker 16 或本次测试名称上增加特例，也不要依赖 Worker 客户端二次过滤。范围控制应在
Rust Server 的认证上下文和通用 list/watch 查询层完成。

## 建议修复范围

1. SYSTEM Token 认证上下文必须携带其唯一 `scoped_cluster_id`；
2. Model list 对 Cluster SYSTEM 主体应用：
   `cluster_id == scoped_cluster_id OR cluster_id IS NULL`；
3. Model watch 的初始快照及 CREATED、UPDATED、DELETED 事件应用同一条件；
4. ModelFile list 必须应用请求中的 `worker_id`；
5. ModelFile watch 的初始快照及后续事件必须应用同一 `worker_id`；
6. 审计 ModelInstance list/watch 的 Cluster 与 Worker 所有权范围；
7. 保留平台管理员原有的合法全局可见性，不要把 Worker 范围误加到管理主体；
8. 对 list 和 watch 共用或复用相同的 scope predicate，避免两个路径再次漂移。

## 回归测试与验收标准

建议至少加入以下自动化用例：

1. 准备 Cluster 1 和 Cluster 3；Cluster 1 中存在 5 个 Model；
2. Cluster 3 中创建 Worker 16，并用其 SYSTEM Token 请求 Model list；
3. 断言响应只包含 Cluster 3 或 `cluster_id=null` 的 Model；当前数据下应为 0；
4. 在 Worker 13～15 创建 ModelFile，Worker 16 不创建 ModelFile；
5. 使用 Worker 16 Token 请求 `model-files?worker_id=16`，断言为 0；
6. 创建一个 Cluster 1 Model watch 事件，断言 Worker 16 的 watch 不收到；
7. 创建一个 Cluster 3 Model watch 事件，断言 Worker 16 能收到；
8. 分别验证 ModelFile watch 的同 Worker事件可见、其他 Worker 事件不可见；
9. 对 ModelInstance list/watch 执行相同的正反范围测试；
10. 使用平台管理员 Token 验证其合法全局可见性未被误删；
11. 再次运行 Rust Worker 空载测试，最终必须满足：
    `desired_models=0`、`desired_model_instances=0`、`desired_model_files=0`；
12. 全程不得出现容器创建、文件协调、runtime reconciliation 或 GPU workload。

若 Cluster 1 与 Cluster 3 分属不同租户，还应增加跨租户负向用例，确保 SYSTEM Token 不能读取其他
租户的 Model、ModelInstance、ModelFile 或 watch 事件。

## 证据与可复现性

正式证据保存在 90 服务器：

```text
/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m8rs4-empty-run-formal-attempt1
```

关键文件：

- `worker-token-models.json`
- `worker-token-model-files.json`
- `worker-token-instances.json`
- `worker-summary.json`
- `formal-result.txt`
- `post-audit-summary.txt`
- `SHA256SUMS`

证据目录权限为 0700，共 62 个 0600 普通文件；三类 Token 逐值扫描均无命中。
`SHA256SUMS` 文件自身的 SHA-256 为：

```text
f3d7e75223fc724ffe9cf7b2d2c305d2f013830e3ba4afd7bce1bc8dadb74273
```

本报告只包含脱敏后的请求和资源摘要，不包含任何可用凭据。

## 修复后的联调入口

Rust Server 修复部署后，先只复测以下三个只读请求：

1. Worker 16 Token 的 `GET /v2/models?page=-1`；
2. Worker 16 Token 的 `GET /v2/model-files?page=-1&worker_id=16`；
3. Worker 16 Token 的 `GET /v2/model-instances?page=-1`。

三者范围正确后，再单独验证 watch 过滤。全部通过后，Rust Worker 项目才会申请新的 RS.4.2 正式
执行授权，并使用全新的证据目录重跑 130 秒空载测试；不会把本次失败 attempt 改写为通过。
