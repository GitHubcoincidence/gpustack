# M8-EMB.7 Rust Server管理员session运行时诊断

## 1. 目的和结论边界

本阶段验证`Rust Server → Rust Worker → vLLM pooling`的embedding真实链，避免Rust Server尚未
修复的平台管理API Key组织授权问题阻塞Worker运行时诊断。

本阶段是**诊断性、非正式兼容验收**：管理员Web session通过只代表内部admin路径可用，不能证明
API Key、Rust CLI控制面或多租户授权已经兼容。状态结果也分成两条：Worker embedding诊断可以在
核心状态链和推理门全部通过后记PASS；Rust Server完整状态契约仍须包含七状态。D332及Server状态
缺口均须单独修复和复测。

## 2. 已冻结输入

- 主机：`wujiaxing@10.213.80.106`；
- Rust Server：`http://127.0.0.1:18091/`；
- 原现场：Worker 17 `jiaxingHaoyuNB-106`、Model 21、Instance 87、端口40000；
- 原模型容器：preflight按标签唯一解析并把完整identity写入恢复计划，不在脚本中依赖短容器ID；
- 实验Worker：`rust-m8emb7-106`，控制端口20250，指标端口20251；
- 实验Model：每次正式重试使用全新固定名称，当前为`m8emb7-bge-small-zh-106-attempt4`；preflight
  必须同时证明API不可见且数据库活动/软
  删除同名行总数为0；
- 模型：固定revision的`BAAI/bge-small-zh-v1.5`，11个文件、96,378,336字节、512维；
- Runner：固定digest的GPUStack Runner；
- GPU：只允许唯一GPU0 UUID，106没有第二张GPU可供本阶段使用；
- 凭据：Cluster registration token仅用于Worker注册和Worker直连；控制面只读取受保护的管理员
  cookie文件。

## 3. 鉴权探测结果

同一次隐藏登录产生的凭据已验证：

| 路径 | Model list | ModelInstance list | 判定 |
|---|---:|---:|---|
| Web session cookie | 200 | 200 | 可用于诊断HTTP控制面 |
| session JWT Bearer | 401 | 401 | 不可写入Rust CLI Context |

因此本阶段不创建伪造CLI Context，也不把session JWT当作API Key。创建、查询、watch、maintenance
和删除均由受保护cookie HTTP工具完成；Rust CLI/API Key兼容保持未通过。

## 4. 工具和职责

- `prepare-admin-session.sh`：隐藏登录、保护cookie、分别探测cookie和Bearer；
- `preflight.py`：离线构建并固定源码、模型、原进程、容器、GPU、控制面、数据库清理边界和恢复
  计划；
- `runtime_tool.py`：只替换已验收M8-EMB.6-106生命周期工具的操作员传输、模型创建与embedding
  评分逻辑；
- `executor.sh`：把一次性授权绑定到精确HEAD和preflight manifest；
- `watchdog.sh`：将维护窗口限制为30分钟，并在执行异常时重试精确cleanup；
- `cleanup.sh`：只删除唯一实验Model、Instance、Worker和容器，并恢复原进程、容器和maintenance；
  ModelFile先调用API，只有HTTP 405才启用D335的精确数据库兜底；
- `post-audit.sh`：独立证明实验残留、API/数据库ModelFile残留归零和原现场完整恢复；
- `self-test.sh`：验证语法、授权绑定、双状态门、精确SQL生成、冻结命令和禁止宽泛清理。

M8-EMB.6-106工具的原阶段常量保持默认值；M8-EMB.7只在导入后的模块实例中替换阶段标签，避免
环境变量改变旧正式协议的授权含义。

## 5. 执行门

### 5.1 静态和回归门

必须同时通过：

1. 新工具Python/Shell语法；
2. M8-EMB.7离线self-test；
3. 原M8-EMB.6-106 self-test，证明复用改造未改变旧默认行为；
4. 秘密扫描、宽泛删除扫描和`git diff --check`；
5. 106仓库、GitHub分支和本地工作树同步到同一提交。

### 5.2 只读preflight门

preflight会在原Worker仍运行时离线构建最新二进制并封存恢复材料，但不会切换maintenance、停止
进程/容器、创建Server资源或启动BGE。它必须确认：

- HEAD、远端跟踪引用、Cargo.lock和二进制身份一致；
- Server、原Worker、metrics、ready和原Qwen均为200；
- 管理员cookie仍可读取控制面；
- 实验名称、端口、目录均未占用；
- 原Worker进程、Model、Instance、容器、镜像、GPU UUID和计算进程唯一；
- 固定BGE manifest、文件数、字节数和维度一致；
- PostgreSQL容器名、镜像、Compose项目/服务和数据库身份一致，三个ModelFile相关表存在，固定BGE
  路径没有运行前孤儿元数据；
- recovery plan和preflight manifest可独立复核。

preflight通过只会进入`ready_for_worker_embedding_diagnostic_authorization`，不会自动开始维护窗口。

### 5.3 一次性维护授权门

真实运行前必须取得用户对当前HEAD和preflight manifest的明确授权，并创建0600单链接JSON：

- `stage=M8-EMB.7-worker-embedding-diagnostic-window`；
- `approved=true`；
- 精确`expected_head`；
- 精确`preflight_manifest_sha256`；
- 唯一nonce。

任一字段漂移都必须失败关闭。

## 6. 真实运行顺序

1. 复核原进程、容器、GPU和preflight没有漂移；
2. 用管理员cookie把原Worker 17置为maintenance；
3. 精确SIGTERM原Worker进程，只停止preflight固定的原容器，并等待GPU0计算进程归零；
4. 启动独立`worker run`，等注册后先置maintenance，固定新Worker ID/UUID，再解除maintenance；
5. 用cookie创建唯一embedding Model，固定Worker、GPU0、本地模型、Runner digest和
   `--runner=pooling`；
6. 同时使用ModelInstance watch和list，分别计算两条结论：Worker诊断必须完整观察
   `pending → scheduled → initializing → starting → running`；Server契约要求完整
   `pending → analyzing → scheduled → initializing → downloading → starting → running`。只有已冻结的
   `analyzing/downloading`同时缺失时允许继续Worker诊断，并把Server契约明确记录为FAIL；
7. 验证唯一容器的Worker UUID、Instance ID、GPU UUID、镜像、spec hash、pooling和max length；
8. 通过Rust Server验证双输入float、单输入base64和三次重复embedding；
9. 通过Rust Worker控制端口和Cluster token验证一次直连embedding；
10. 无论成功或失败都执行精确cleanup并恢复原现场，再运行独立post-audit。

## 7. 评分和验收

Worker embedding诊断只有在以下条件全部满足时才可记为`diagnostic_pass`：

- cookie创建、list和watch看到同一Model/Instance；
- Worker核心状态链有序到达`running`，且不存在除`analyzing/downloading`同时缺失之外的状态差异；
- 唯一vLLM容器只请求GPU0并使用固定镜像和pooling；
- Server float为2×512维，base64解码后为512个float32字节；
- Worker直连为1×512维；
- 三次重复请求保持同一容器ID、restart count和spec hash；
- cleanup与post-audit均为PASS；
- 原Worker 17、Model 21、Instance 87、同一容器、原maintenance和Qwen健康全部恢复；
- cookie、一次性授权和其他临时secret按退役步骤删除。

证据必须同时保存`worker_embedding_diagnostic_passed`、
`server_managed_state_contract_passed`和`missing_server_states`。即使Worker诊断全部通过，缺少
`analyzing/downloading`时Server状态契约仍为FAIL；M8-EMB.7也不能记为API Key正式兼容。Rust
Server须按两份独立缺陷报告修复并复测。

## 8. 失败与回滚规则

- 不手工POST ModelInstance，不改数据库membership/access，不把SYSTEM token当操作员；
- 不停止或删除非preflight固定的进程、容器、Model或Instance；
- 不使用模糊容器名、`pkill`、`docker system prune`或递归删除宽泛目录；
- formal失败仍必须保留证据并执行cleanup；cleanup失败时保留现场身份，先进行精确只读诊断；
- ModelFile数据库兜底不是常规控制面：API非405时禁止使用；事务必须精确匹配ID、Worker、Cluster、
  owner和固定路径，两个Instance关联表计数都为0，删除计数必须恰好为1，否则回滚并判cleanup失败；
- 数据库兜底只删除Server元数据，禁止删除或修改固定BGE本地文件；
- watchdog只在正式证据目录已经建立且执行异常时重试同一精确cleanup；
- 原现场未恢复前不得发起下一次尝试。

## 9. 当前停止位置

attempt 4已通过并经用户验收：核心五状态、唯一GPU0 pooling容器、Server float/base64、Worker
直连、三次幂等、D335数据库兜底、cleanup和独立post-audit全部PASS，原现场完整恢复。管理员
session已logout、验证401并删除，本阶段凭据和授权归零。当前为
`worker_diagnostic accepted / server_state_contract FAIL`；Rust Server仍缺少
`analyzing/downloading`，API Key及多租户正式兼容也未通过。后续须另行讨论和授权，不自动进入
M8-B或新的维护窗口。
