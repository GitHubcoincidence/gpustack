# Rust Server平台管理员无法管理Default Cluster缺陷报告

## 1. 摘要

106 Rust Server修复Model/ModelInstance租户归属不一致并重启后，平台管理员API Key无法再通过标准
CLI向Default Cluster创建Model。Server在Model持久化之前返回HTTP 403：

```text
Forbidden: The selected cluster does not belong to the current organization.
```

请求使用的Key属于`is_admin=true`的平台管理员，scope为`["*"]`；目标Worker属于Default Cluster。
数据库中Default Cluster由ORG principal 1拥有，API Key由USER principal 4拥有，且没有任何
`principal_memberships`或`cluster_access`记录。当前实现既没有把平台管理员视为跨组织管理员，也
没有为初始admin建立Default ORG membership，因此合法的平台管理入口被完全阻断。

该问题是前一项租户可见性修复后的授权闭环缺口。旧实现允许USER 4创建owner 1资源，但后续
list/watch不可见；新实现阻止了错误创建，却没有提供管理员访问Default Cluster的合法路径。

## 2. 严重程度与归属

- 严重程度：P1，阻断平台管理员通过API Key和CLI部署任何Cluster托管模型；
- 归属组件：Rust Server认证上下文、组织membership、Cluster访问校验和平台admin授权；
- 非归属组件：Rust CLI请求构造、Rust Worker、scheduler、副本控制器、Docker、vLLM和GPU；
- 当前阶段：`M8-EMB.7 waiting_on_rust_server_admin_cluster_authorization`；
- 确认时间：2026-11-26，Asia/Shanghai。

## 3. 环境

| 项目 | 值 |
|---|---|
| Rust Server | `http://10.213.80.106:18091/` |
| 修复后进程 | PID 1186032，owner `lenovo` |
| 启动时间 | 2026-11-26 11:03:23 |
| 数据库 | PostgreSQL，数据库名`gpustack223` |
| 管理Key | ID 32，7天平台管理Key，scope `[*]` |
| Key主体 | principal 4，kind USER，name `admin`，`is_admin=true` |
| 目标Cluster | ID 1，`Default Cluster` |
| Cluster owner | principal 1，kind ORG，name `default` |
| Cluster SYSTEM主体 | principal 3，`system/cluster-1` |
| 目标Worker | ID 17，`jiaxingHaoyuNB-106`，Cluster 1 |
| 原模型 | Model 21 / Instance 87 / GPU0 / port 40000 |

Key secret、session cookie、registration token和Worker token均未写入证据或本文档。

## 4. 复现步骤

### 4.1 建立API Key Context

```bash
export GPUSTACK_RS_CONTEXTS_FILE="$HOME/.config/gpustack-rust/m8emb7-visibility-contexts.json"

gpustack-rs --output json context add m8emb7-visibility-rust-server \
  --server-url http://127.0.0.1:18091/ \
  --token-file "$HOME/.config/gpustack-rust/<management-key-file>" \
  --use
```

### 4.2 创建绑定Default Cluster Worker的Model

```bash
gpustack-rs --output json model deploy m8emb7-visibility-fix-20261126 \
  --worker jiaxingHaoyuNB-106 \
  --local-path <fixed-local-model-path> \
  --image <fixed-runner-digest> \
  --gpu 0 \
  --category embedding \
  --runner pooling \
  --tensor-parallel-size 1 \
  --max-model-len 512 \
  --gpu-memory-utilization 0.1 \
  --dtype auto \
  --enforce-eager
```

CLI会先按Worker名称唯一解析Worker 17，并把其`cluster_id=1`写入ModelCreate请求。该行为是既有
CLI安全约束：手工GPU selector必须与目标Worker处于同一Cluster。

## 5. 实际结果

命令退出码为30，Server返回：

```text
HTTP 403 Forbidden
The selected cluster does not belong to the current organization.
```

失败发生在Model持久化之前：

- 目标Model数量为0；
- 目标ModelInstance数量为0；
- 没有ModelFile、容器、端口或GPU进程；
- 原Worker、Instance 87和Qwen保持健康。

## 6. 数据库归属证据

### 6.1 Cluster

```text
cluster.id=1
cluster.name=Default Cluster
cluster.owner_principal_id=1
owner.kind=ORG
owner.name=default
cluster.system_principal_id=3
```

### 6.2 API Key

```text
api_key.id=32
api_key.owner_principal_id=4
principal.kind=USER
principal.name=admin
principal.is_admin=true
api_key.scope=["*"]
```

### 6.3 授权关系

```text
principal_memberships: 0 rows
cluster_access: 0 rows
```

因此当前TenantContext无法从USER 4推导出ORG 1或Cluster 1，但该用户又是平台admin。Server必须
明确平台admin是否跨组织，或者在初始化数据中建立可继承的Default ORG关系。

## 7. 预期结果

平台管理员通过有效、未过期、scope为`[*]`的API Key访问Default Cluster时，Server应选择一种
明确且一致的授权语义：

1. 平台admin覆盖：`is_admin=true`可管理所有组织和Cluster；或
2. 组织membership：初始admin是Default ORG成员，API Key继承其TenantContext；或
3. 显式授权：`cluster_access`把admin主体或所属组织授予Cluster 1。

无论采用哪一种，以下操作必须使用同一授权判断：

- Worker解析和Model创建；
- Model、ModelInstance和ModelFile的list/get；
- ModelInstance和ModelFile watch；
- Model更新、删除和副本对账；
- 推理路由和日志访问。

平台admin成功不代表普通USER可以跨组织。非管理员且无membership/access的主体仍必须得到403。

## 8. 与上一缺陷的关系

旧行为：

```text
USER 4管理Key创建Model
  -> Model/Instance归ORG 1
  -> 同一Key的Instance list/watch按USER 4过滤
  -> 创建成功但后续不可见
```

修复后行为：

```text
USER 4管理Key选择Cluster 1
  -> 组织校验发现4 != 1且无membership/access
  -> 即使is_admin=true、scope=["*"]仍返回403
```

新行为避免了跨租户错误写入，但还没有完成管理员授权闭环。不能把403视为上一缺陷已经通过验收；
完整验收必须覆盖create、list和watch三部分。

## 9. 已排除原因

### 9.1 不是无效或过期Key

同一Key可以读取API Key列表、Worker和现有资源；Key未过期，scope为`[*]`。

### 9.2 不是Worker或Cluster不存在

Worker 17存在、ready并属于Cluster 1；Cluster 1为Default Cluster且状态正常。

### 9.3 不是CLI遗漏cluster_id

CLI按解析出的Worker显式提交`cluster_id=1`。403正文也证明Server已经读取并校验该Cluster。

### 9.4 不是embedding专属问题

失败发生在Model通用Cluster授权检查，尚未处理category、runner、ModelInstance或scheduler。

### 9.5 不能用SYSTEM token替代

Cluster registration token和Worker SYSTEM token读取Model为HTTP 200，但读取ModelInstance均为401。
它们的职责也不是平台模型管理；即使放宽也不能证明管理员API Key兼容。

## 10. 建议排查位置

1. API Key认证后如何从`owner_principal_id=4`构造TenantContext；
2. `is_admin`是否在Cluster访问校验前丢失；
3. scope `[*]`表示平台权限还是仅表示模型范围；
4. 默认admin创建流程是否应写入`principal_memberships`；
5. Default ORG和Default Cluster初始化顺序是否漏写membership/access；
6. `cluster_access`查询是否包含直接主体、所属组织和平台admin；
7. Web session与API Key是否走了不同的admin覆盖逻辑；
8. Model创建与ModelInstance list/watch是否复用同一个权限helper；
9. 数据迁移是否只修复新安装而遗漏现有数据库；
10. 403是否应带稳定错误码，便于CLI区分无权限与资源不存在。

## 11. 建议自动化测试

至少增加以下Server集成测试：

1. 平台admin session可以在Default Cluster创建Model；
2. 同一admin的scope `[*]` API Key具有与明确产品语义一致的Cluster访问；
3. 创建后同一Key能list/get该Model和Instance；
4. 同一Key的watch能收到pending及后续事件；
5. `page=-1`取消分页但不取消权限过滤；
6. 普通USER无membership/access时创建返回403；
7. 普通USER获得membership后只访问所属ORG资源；
8. 显式cluster_access只授予目标Cluster；
9. 其他ORG的Model/Instance不会因admin修复泄漏给普通USER；
10. 既有数据库迁移后初始admin具备预期Default ORG关系；
11. API Key撤销或过期后立即失去权限；
12. 普通LLM和embedding走相同授权与副本路径。

## 12. 修复验收标准

修复部署后使用全新唯一Model名称重跑，必须同时满足：

1. 平台管理Key创建Model返回成功；
2. Model owner、Cluster和创建主体符合选定授权设计；
3. 30秒内创建恰好一个ModelInstance；
4. 同一Key的list和watch均看到该Instance；
5. `gpustack-rs model status`显示同一Instance；
6. 普通无权限Key对Cluster 1仍返回403；
7. 删除Model后Instance及关联资源归零；
8. 不需要直接写数据库、伪造membership或使用SYSTEM token；
9. Server、Worker和既有模型全程健康；
10. 通过后才能进入需要释放GPU0的embedding运行时维护窗口。

## 13. 证据与回滚

失败证据目录：

```text
/home/wujiaxing/.local/state/gpustack-rust-worker/evidence/
  m8emb7-visibility-fix-20261126
```

manifest SHA-256：

```text
cfd8cf81fac52d7be0a3e325346ec8b4fbe5461b205aa1897e1c609e5c2d4c3b
```

证据秘密扫描为0。隔离Context和watchdog均已删除；没有实验Server资源、容器、端口或GPU进程。

## 14. 临时绕过边界

为继续诊断Worker和embedding运行时，可以使用短期管理员Web session执行控制面请求。该绕过：

- 不修改数据库membership或cluster_access；
- 不使用Cluster/Worker SYSTEM token冒充平台管理员；
- session cookie和JWT只保存在106的0600文件，不进入Git或证据；
- 只证明Server内部admin session、Worker、Docker、vLLM和推理链；
- 不能证明API Key、`model deploy --wait`或CLI控制面兼容；
- 测试结束立即删除session文件。

M8-EMB.7在API Key修复并复测前不得标记为正式通过。

## 15. 临时绕过实测补充

2026-11-26在106使用同一次管理员登录分别探测两种传输：

```text
cookie_models_http=200
cookie_instances_http=200
bearer_models_http=401
bearer_instances_http=401
```

这说明当前Rust Server把管理员Web session cookie识别为可访问全量控制面的主体，但不接受从同一
session提取的JWT作为Bearer凭据。临时诊断因此只能走cookie HTTP工具，不能把JWT写入Rust CLI
Context。该结果不修复也不弱化本报告的API Key 403问题，反而说明Web admin与API Key授权路径的
语义差异需要在Server测试中显式覆盖。
