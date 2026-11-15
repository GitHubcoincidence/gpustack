# Rust Server缺陷报告：缺少ModelInstance创建接口

## 摘要

Rust Server对原GPUStack v2.2.3的ModelInstance集合创建请求返回HTTP 405，且当前OpenAPI只为
`/v2/model-instances`声明GET，没有声明POST。请求没有创建任何资源，说明问题发生在路由层，尚未
进入ModelInstance请求解析、持久化或Rust Worker处理流程。

- 报告日期：2026-11-15
- Rust Server：`v2.2.3`，commit `fba334c`，implementation `rust`，build `0.1.0`
- 请求接口：`POST /v2/model-instances`
- 目标资源：`rust-rs5-instance-90-20261115`，Model 6，Cluster 3，Worker 16
- 问题归属：Rust Server
- 阶段结论：`FAIL/missing_model_instance_create`
- 实际副作用：无；没有创建ModelInstance、ModelFile、进程、端口、容器或GPU任务

## 接口作用

原GPUStack通过：

```text
POST /v2/model-instances
```

创建一条ModelInstance控制面记录。该记录描述一个Model副本的调度与运行状态，包括所属Model、
Cluster、Worker、模型来源、backend、端口、GPU分配、分布式拓扑和状态。Server调度器通常会创建
实例，但该公开接口仍是v2.2.3契约的一部分，也是控制面测试、管理操作和兼容客户端所依赖的能力。

Rust Worker不会通过此接口自行创建实例；Worker只消费Server分配给自己的实例并在后续状态转换中
更新它。因此，请求在路由层返回405不能归因于Worker。

## 原版依据

权威基线为原GPUStack v2.2.3 commit：

```text
1cfcbbff02aacdb51f0a390378dd7bcddd9bf365
```

原版在`gpustack/routes/model_instances.py`中注册：

```python
@router.post("", response_model=ModelInstancePublic)
async def create_model_instance(
    session: SessionDep, model_instance_in: ModelInstanceCreate
):
    ...
```

该handler继承父Model的租户归属，创建ModelInstance并返回`ModelInstancePublic`。路由由
`gpustack/routes/routes.py`挂载在`/v2/model-instances`下，并使用相应的组织所有者权限层。

## 前置状态

正式请求前确认：

- 90与GitHub源码HEAD均为`2fca247fcb858a372117b9c0df56e97a1f27aa35`，90工作树干净；
- Server版本为`v2.2.3`，commit为`fba334c`；
- Worker 16精确属于Cluster 3，名称为`rust-rs3-worker-90-20261113`，处于maintenance；
- Model 6精确属于Cluster 3，名称为`rust-rs5-scope-90-20261115`，`replicas=0`；
- 管理主体与Worker 16 SYSTEM主体均未看到目标实例；
- Model 6的实例数为0，Worker 16的实例数和ModelFile数均为0；
- Worker 16进程未运行，21150/21151无监听；
- 用户独立Worker进程PID 1113114在线且不属于本次测试；
- Docker容器集合和两张A100为空载基线。

## 请求

管理主体只发送了一次请求，没有自动重试或重定向：

```http
POST /v2/model-instances HTTP/1.1
Content-Type: application/json
Authorization: Bearer <REDACTED>
```

请求体为：

```json
{
  "name": "rust-rs5-instance-90-20261115",
  "source": "local_path",
  "local_path": "/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct",
  "state": "pending",
  "model_id": 6,
  "model_name": "rust-rs5-scope-90-20261115",
  "cluster_id": 3,
  "worker_id": 16,
  "worker_name": "rust-rs3-worker-90-20261113",
  "worker_ip": "10.213.80.90",
  "worker_ifname": "eno8403",
  "worker_advertise_address": "10.213.80.90",
  "pid": null,
  "port": null,
  "ports": [],
  "download_progress": null,
  "resolved_path": "/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct",
  "draft_model_source": null,
  "draft_model_download_progress": null,
  "draft_model_resolved_path": null,
  "computed_resource_claim": null,
  "gpu_type": "cuda",
  "gpu_indexes": [],
  "gpu_addresses": [],
  "backend": "vLLM",
  "backend_version": null,
  "distributed_servers": null,
  "mounted_loras": null
}
```

这些字段都存在于106当前OpenAPI的`ModelInstancesCreate` schema中。`state=pending`、空GPU与空端口
保证即使创建成功也不会满足Rust Worker只接管`scheduled`实例的前置条件。

## 实际响应

```text
POST attempts: 1
curl exit: 0
HTTP: 405
response body: empty
```

HTTP 405说明Server识别了路径，但该路径没有注册POST方法。它不是请求体校验错误；请求体校验错误
应在路由命中后返回相应的4xx错误，而不是方法不允许。

当前`/openapi.json`也与运行结果一致：

- `/v2/model-instances`只列出GET；
- components中存在`ModelInstancesCreate`；
- `/v2/model-instances/{id}`列出GET、PUT和DELETE；
- 集合POST没有出现在paths中。

因此，当前表现不是单纯OpenAPI漏写，而是运行路由和OpenAPI同时缺少创建操作。

## 期望行为

对合法且已认证的请求，原版接口应：

1. 命中`POST /v2/model-instances`；
2. 校验Model、Cluster和Worker引用；
3. 继承父Model的租户归属；
4. 创建唯一ModelInstance；
5. 返回HTTP 200及带正整数ID的`ModelInstancePublic`；
6. 让管理主体和所属Worker SYSTEM主体在list/watch中看到该资源；
7. 不因`pending`状态启动Worker runtime或分配GPU。

## 后置审计

唯一POST后，管理主体和Worker 16 SYSTEM主体分别重新读取ModelInstance和ModelFile清单：

```text
target instance count (management): 0
target instance count (worker):     0
Model 6 instance count:             0
Worker 16 instance count:           0
Worker 16 ModelFile count:          0
```

同时确认：

- Server commit前后均为`fba334c`；
- Model 6仍为`replicas=0`；
- Worker 16仍为maintenance；
- Worker 16目标进程为0，21150/21151监听为0；
- 用户独立Worker PID 1113114仍在线；
- Docker容器身份集合不变；
- 两张A100前后均为0 MiB、0%利用率。

## 建议修复范围

1. 在`/v2/model-instances`集合路由注册POST handler；
2. 使用与原版`ModelInstanceCreate`兼容的请求DTO和`ModelInstancePublic`响应；
3. 验证父Model、Cluster、Worker引用及其归属关系，拒绝跨Cluster错误绑定；
4. 从父Model继承租户/组织归属，不能信任客户端提供的归属主体；
5. 保持ModelInstance名称唯一约束，并返回稳定的重复资源错误；
6. 创建提交后发布正确的CREATED watch事件；
7. 把POST operation及真实request/response schema加入OpenAPI；
8. 保留现有平台管理员、组织所有者和Worker SYSTEM主体的认证与可见性边界。

## 建议回归测试

1. 在隔离Cluster创建`replicas=0`的Model和maintenance Worker；
2. POST一个`state=pending`、空端口、空GPU的合法ModelInstance；
3. 断言HTTP 200、ID为正整数、请求字段正确持久化；
4. 断言管理主体能GET/list该实例；
5. 断言所属Worker SYSTEM主体能list/watch该实例；
6. 断言其他Cluster的Worker SYSTEM主体不可见；
7. 断言CREATED watch事件类型和数据与原版一致；
8. 断言pending实例不会启动runtime、占用端口或GPU；
9. 对该实例执行完整PUT，验证状态所有权和字段保留；
10. DELETE后断言资源消失且重复删除行为与原版一致；
11. 断言OpenAPI实际包含集合POST且schema引用可解析。

## 证据

90服务器受保护证据目录：

```text
/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m8rs5-pending-instance-create-20261115T131358Z
```

关键文件：

- `request.json`
- `post-status.txt`
- `post-response.json`
- `instances-management-before.json`
- `instances-management-after.json`
- `instances-worker-before.json`
- `instances-worker-after.json`
- `resource-counts-before.txt`
- `resource-counts-after.txt`
- `worker-16.json`与`worker-16-after.json`
- `model-6.json`与`model-6-after.json`
- `isolation-after.txt`
- `result.txt`
- `safety.txt`
- `SHA256SUMS`

目录权限0700、文件权限0600；management key、Cluster registration token、Worker token逐值扫描和
通用凭据模式扫描全部通过。`SHA256SUMS`自身SHA-256：

```text
28996677671f4b46c6e4dab89203f19a29e4701edc27ea02f55928d3903d1e17
```

## 当前资源状态

- Model 6继续保留，仍为停止态；
- 没有创建目标ModelInstance；
- Worker 16继续maintenance且未运行；
- 未发送第二次POST，未进行数据库写入或Model扩容绕过；
- Docker、GPU、端口和用户独立Worker前后不变。
