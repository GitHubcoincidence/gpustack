# Rust Server缺陷报告：ModelFile过滤与删除契约不完整

## 摘要

M8-EMB.7清理实验Worker 418时同时确认了三个Rust Server ModelFile问题：

1. `GET /v2/model-files?page=-1&worker_id=418`忽略`worker_id`，返回当前管理员可见的全部ModelFile；
2. `DELETE /v2/model-files/23?cleanup=false`返回HTTP 405，OpenAPI只声明GET和PUT；
3. 删除Worker 418后ModelFile 23没有级联删除，形成引用不存在Worker的孤儿元数据。

清理工具因第一项问题拒绝删除其他Worker文件并失败关闭。最终在用户明确授权后，通过数据库单事务
只删除精确匹配的孤儿ModelFile 23；固定BGE目录未删除，其他ModelFile和软删除Model 28均保留。

- 报告日期：2026-11-26
- 环境：`10.213.80.106:18091`
- 目标Worker：418，`rust-m8emb7-106`
- 目标ModelFile：23
- 固定路径：`/home/wujiaxing/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d`
- 结果：`FAIL/model_file_filter_and_delete_contract`
- 问题主归属：Rust Server
- 安全影响：宽集合响应可能导致不安全客户端误操作其他Worker元数据

## 原版契约

GPUStack v2.2.3的`gpustack/routes/model_files.py`提供：

```text
GET    /v2/model-files?worker_id=<id>
GET    /v2/model-files/{id}
POST   /v2/model-files
PUT    /v2/model-files/{id}
DELETE /v2/model-files/{id}?cleanup=<bool>
```

集合GET把`worker_id`放入数据库查询字段，watch也使用同一字段过滤。DELETE在确认文件未被实例使用后
删除元数据；`cleanup=false`用于只删除Server记录而保留本地模型文件。

## 问题一：忽略worker_id过滤

请求：

```http
GET /v2/model-files?page=-1&worker_id=418
Cookie: <REDACTED>
```

期望只返回ModelFile 23。实际返回了ID 1至23的全部可见ModelFile，覆盖多个Worker。目标文件只是
其中最后一条：

```text
id=23
worker_id=418
source=local_path
state=ready
```

清理工具原本信任Server过滤，逐条检查路径时发现第一条不属于实验Worker，立即以
`unexpected ModelFile belongs to experimental Worker`停止。没有误删任何其他Worker文件。

## 问题二：缺少DELETE接口

精确请求：

```http
DELETE /v2/model-files/23?cleanup=false
Cookie: <REDACTED>
```

实际返回：

```text
HTTP 405
```

当前OpenAPI为：

```text
/v2/model-files       methods=get
/v2/model-files/{id}  methods=get,put
```

这不仅影响实验清理，也阻断Rust CLI的安全ModelFile元数据清理能力。

## 问题三：删除Worker后遗留孤儿ModelFile

实验Model 28已通过正式Model DELETE进入软删除状态；Instance 115和实验容器已删除。随后删除
Worker 418返回成功，API对Worker 418返回404，但：

- `GET /v2/model-files/23`仍返回200；
- PostgreSQL `gpustack223.model_files`仍存在`id=23, worker_id=418`；
- `workers`表中`id=418`为0行；
- 等待30秒后仍未异步回收。

这说明当前删除流程既没有DELETE端点，也没有Worker删除级联或孤儿回收器。

## 安全边界

修复集合过滤时必须先应用TenantContext可见性，再应用`worker_id`条件。不能为了修复过滤而返回其他
组织的资源。DELETE也必须调用单资源可见性检查，并在文件仍被ModelInstance使用时返回409。

客户端仍应二次检查每个返回对象的`worker_id`，但这只是纵深防御，不能代替Server查询过滤。

## 建议修复

1. 为集合GET和watch实现`worker_id`数据库及事件过滤；
2. 在分页前应用过滤，确保`total`、页内容和`page=-1`一致；
3. 在OpenAPI恢复DELETE路由和`cleanup`参数；
4. `cleanup=false`只删元数据，不删除本地路径；
5. `cleanup=true`仍须通过Worker安全清理契约，不能由Server直接递归删除任意路径；
6. 有Instance引用时返回409；
7. 删除Worker时选择并实现明确语义：级联删除未被使用的ModelFile，或在拒绝删除Worker前要求先
   清理；不能留下无有效Worker的记录；
8. 为历史孤儿数据提供可审计迁移或后台回收；
9. 所有删除操作写审计日志，但不得记录凭据或模型敏感内容。

## 建议测试

1. 创建两个Worker，各自至少两个ModelFile；
2. GET带`worker_id`只返回目标Worker文件，`total`正确；
3. 普通分页、`page=-1`和watch使用相同过滤；
4. 不存在Worker ID返回空集合；
5. DELETE `cleanup=false`删除元数据并保留本地目录；
6. DELETE `cleanup=true`只通过受控Worker文件接口清理允许路径；
7. 有Instance引用时返回409且记录保持；
8. 删除无Instance的Worker后不留下孤儿ModelFile；
9. 普通用户不能读取或删除其他组织文件；
10. Rust CLI `model file cleanup`对Rust Server成功；
11. 未实现查询参数时不要静默忽略，应在兼容实现完成前明确拒绝；
12. 回归ModelFile watch重连与全量同步。

## 恢复与证据

正式失败和两次安全停止：

```text
m8emb7-106/cleanup-1393536b-attempt1
m8emb7-106/watchdog-cleanup-1393536b-attempt1
m8emb7-106/recovery-1393536b-attempt1
```

对应manifest SHA-256：

```text
64c2f0e9a831ad5eb57aed47a587b38b00c3e423834224751461a98918c213f3
685db917025762f453ab0aabe0dedf535778b5fa7ed9db2f33f34d362e1a35bf
08c31c20c2aa0a042b13e5e73766dc7ff3f551ff9b2a05d2198533b440be1581
```

精确人工恢复与数据库补救：

```text
m8emb7-106/manual-recovery-1393536b-attempt1
m8emb7-106/db-recovery-1393536b-attempt1
m8emb7-106/post-audit-1393536b-attempt2
```

对应manifest SHA-256：

```text
097263337a3b1dc56eb31c15d4d380ead1a245ad2ce3dc7a8447435abb97a2c6
d7088fbc4e96ef6767daaa8dd5100c2f1bb30335bf765ea74012e582f6e19289
f70fbeb9a54bf6b77515eac053b3eb3aa649d3d7512462f9b0d2b4e84a255cc2
```

数据库事务在删除前断言Worker 418和Instance 115不存在、Model 28恰好一条且已经软删除、ModelFile
23的worker/cluster/owner/local_path全部匹配；只删除一行，否则整笔回滚。删除后API返回404，固定
BGE的11个文件、96,378,336字节和manifest复核通过，其他ModelFile计数未被该事务改变。
