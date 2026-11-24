# Rust Server缺陷报告：ModelInstance列表忽略model_id过滤

## 摘要

Rust Server对以下已认证请求返回HTTP 200，但忽略`model_id=21`，把当前主体可见的全部3个
ModelInstance都放入响应，而不是只返回Model 21的Instance 87：

```text
GET /v2/model-instances?page=-1&model_id=21
```

原GPUStack v2.2.3明确支持该过滤参数，Rust CLI的`model status`、`model logs`和
`model deploy --wait`都依赖此契约。本问题已实际导致`model logs 21`误判目标Model有3个Instance并
退出；与此同时，按精确Instance ID读取日志返回HTTP 200和真实vLLM日志，证明Worker日志链路正常。

- 报告日期：2026-11-24
- Rust Server：`v2.2.3`，commit `9f0941c`，implementation `rust`，build `0.1.0`
- Rust CLI：`0.1.0`，commit `b530fb9fc07b`
- 运行环境：`10.213.80.106:18091`
- 受影响接口：`GET /v2/model-instances`
- 被忽略参数：`model_id`；OpenAPI同时缺少原版其他列表过滤参数
- 严重度：高，阻断多模型环境中的CLI状态、日志和等待语义
- 问题主归属：Rust Server
- 阶段结论：`FAIL/model_id_filter_ignored`
- 数据副作用：无；全部复现请求均为GET，Model 21和Instance 87持续running

## 接口作用

原GPUStack通过：

```text
GET /v2/model-instances
```

分页列出当前主体有权访问的ModelInstance，并支持通过查询参数缩小集合。`model_id`用于只返回指定
Model的实例，是Model状态、日志选择、部署等待和控制器诊断的基础契约。

过滤必须在分页统计和响应构造前应用。认证与租户可见性先决定“调用方可以看到哪些资源”，
`model_id`再从该可见集合中选择目标Model；二者不能互相替代。

## 原版依据

权威基线为原GPUStack v2.2.3 commit：

```text
1cfcbbff02aacdb51f0a390378dd7bcddd9bf365
```

原版`gpustack/routes/model_instances.py`中的集合GET明确声明：

```python
@router.get("", response_model=ModelInstancesPublic)
async def get_model_instances(
    ctx: TenantContextDep,
    params: ListParamsDep,
    id: Optional[int] = None,
    model_id: Optional[int] = None,
    worker_id: Optional[int] = None,
    cluster_id: Optional[int] = None,
    state: Optional[str] = None,
    search: Optional[str] = None,
):
    ...
    if model_id:
        fields["model_id"] = model_id
```

相关位置：

- `gpustack/routes/model_instances.py:101`：集合GET handler；
- `gpustack/routes/model_instances.py:105`：声明`model_id`；
- `gpustack/routes/model_instances.py:118`：把`model_id`加入数据库查询字段。

原版还支持`id`、`worker_id`、`cluster_id`、`state`和`search`。本报告只对`model_id`给出正式运行
证据，不把尚未逐项验证的其他参数直接判定为同一运行缺陷。

## 前置状态

复现时106共有3个当前主体可见的ModelInstance：

| Instance ID | Model ID | 状态 | Worker ID |
|---:|---:|---|---:|
| 83 | 17 | running | 24 |
| 87 | 21 | running | 17 |
| 90 | 20 | error | 22 |

目标资源为：

- Model 21：`rust-rs6-106-qwen25-05b-gpu0`；
- `replicas=1`、`ready_replicas=1`；
- Instance 87：running；
- Worker 17：`jiaxingHaoyuNB-106`；
- GPU indexes：`[0]`；
- 端口：40000；
- 运行容器：`gpustack-rs-managed-87`，restart count 0。

## 最小复现

### 请求一：带model_id的集合查询

只发送一次已认证GET；认证值已脱敏：

```http
GET /v2/model-instances?page=-1&model_id=21 HTTP/1.1
Host: 10.213.80.106:18091
Authorization: Bearer <REDACTED>
Accept: application/json
```

也可用正页码复现：

```text
GET /v2/model-instances?page=1&perPage=1000&model_id=21
```

### 实际结果

HTTP状态为200，但响应关键字段等价于：

```json
{
  "items": [
    {"id": 83, "model_id": 17, "state": "running", "worker_id": 24},
    {"id": 87, "model_id": 21, "state": "running", "worker_id": 17},
    {"id": 90, "model_id": 20, "state": "error", "worker_id": 22}
  ]
}
```

不带`model_id`和带`model_id=21`的请求都返回3个相同实例：

```text
all_count=3
filtered_count=3
filtered_model_ids={17,20,21}
```

### 对照请求

Rust Server的模型作用域接口：

```http
GET /v2/models/21/instances HTTP/1.1
Authorization: Bearer <REDACTED>
```

返回HTTP 200且只有：

```json
{
  "items": [
    {"id": 87, "model_id": 21, "state": "running", "worker_id": 17}
  ]
}
```

这证明数据库中Model归属正确，缺陷位于通用集合列表的查询参数处理，而不是Instance数据损坏。

## CLI实际失败

Rust客户端`rust/crates/gpustack-client/src/client.rs:333`的
`list_model_instances_for_model`发送：

```text
v2/model-instances?page=-1&model_id={model_id}
```

`rust/crates/gpustack-cli/src/managed.rs:423`中的`model logs`先调用该方法，再要求返回集合恰好包含一个
Instance。实际命令：

```text
gpustack-rs model logs 21 --tail 80 --context m8rs6-106-control --output json
```

稳定复现为：

```text
exit code: 30
error: Model 'rust-rs6-106-qwen25-05b-gpu0' has 3 ModelInstances; R1 logs require exactly one
```

CLI没有进入精确Instance日志请求，因此输出文件为空。

## Worker日志链路对照

使用同一管理主体直接调用精确Instance接口：

```http
GET /v2/model-instances/87/logs?tail=80&follow=false HTTP/1.1
Authorization: Bearer <REDACTED>
```

实际结果：

```text
HTTP: 200
curl exit: 0
body size: 7194 bytes
```

正文包含vLLM APIServer的真实请求日志，例如`GET /v1/models HTTP/1.1`的200记录。因此以下链路已经
证明可用：

```text
Rust Server -> Rust Worker Instance日志端点 -> vLLM容器日志
```

本缺陷不能归因于Rust Worker、Docker日志读取或vLLM没有产生日志。

## OpenAPI证据

106当前`/openapi.json`为`GET /v2/model-instances`只声明：

- `page`；
- `perPage`；
- `watch`。

没有声明原版的`model_id`。运行结果与OpenAPI一致：参数没有进入过滤逻辑。由于Server对未知查询参数
保持宽松并返回200，调用方无法从HTTP状态判断过滤已被忽略。

## 影响范围

### 已实际证明

1. 多模型环境中，Rust CLI `model logs`无法选择唯一目标Instance；
2. CLI错误数量来自全局可见Instance集合，不是目标Model真实副本数；
3. 精确Instance日志可读，但模型级CLI日志被过滤缺口阻断。

### 代码路径推导，需修复后回归

同一个`list_model_instances_for_model`还用于：

- `model status`；
- `model deploy --wait`轮询；
- watch结束后的状态收敛检查。

因此可能出现：

- `model status`混入其他Model的实例；
- 目标模型部署等待被无关Model的`error`实例错误终止；
- 目标已running但因无关实例未running而超时；
- 状态摘要、日志实例数量和转换判定不可信。

这些是从已确认的公共客户端调用路径推导出的影响，不冒充逐命令正式复现结果。

## 安全与租户边界

本次使用平台管理主体，返回的3个Instance都在该主体授权可见范围内。没有观察到跨租户越权，也没有
证明认证过滤失效。缺陷是“在已授权可见集合内忽略资源过滤”，不能被描述为已确认的数据泄漏。

修复时仍必须先应用TenantContext可见性条件，再应用`model_id`等资源过滤，不能为了补过滤而绕过现有
租户、Cluster或SYSTEM主体边界。

## Rust Server建议修复范围

1. 为`GET /v2/model-instances`增加与原版兼容的可选`model_id`查询参数；
2. 在数据库查询和分页统计前应用`model_id`条件；
3. `page=-1`和普通分页必须使用相同过滤语义；
4. `watch=true&model_id=<id>`只发送该Model的初始数据与后续事件；
5. 把`model_id`加入OpenAPI parameters，并给出整数schema；
6. 对齐原版的`id`、`worker_id`、`cluster_id`、`state`和`search`参数，但每项应有独立测试；
7. 保持TenantContext、平台管理员和SYSTEM主体的现有授权边界；
8. 确保`/v2/models/{id}/instances`和通用集合过滤对同一Model返回一致集合；
9. 不要仅在响应层过滤已分页结果，否则pagination total和跨页结果仍会错误；
10. 不要通过修改CLI错误数量或放宽“恰好一个Instance”检查来掩盖服务端过滤缺口。

## 建议回归测试

1. 创建至少3个Model，每个Model各有一个Instance；
2. 让其中一个无关Instance处于error，另一个无关Instance处于running；
3. 请求`GET /v2/model-instances?page=-1&model_id=<target>`；
4. 断言只返回目标Model的Instance，pagination total等于目标数量；
5. 使用普通分页重复测试，断言每页和total都基于过滤后集合；
6. 使用不存在的model_id，断言HTTP 200且items为空、total为0；
7. 组合`model_id`与`worker_id`，断言采用AND语义；
8. 建立`watch=true&model_id=<target>`，断言无关Model事件不会进入流；
9. 对照`GET /v2/models/{id}/instances`，断言两个接口返回相同目标ID集合；
10. 使用不同租户/Cluster/SYSTEM主体重复，断言过滤不会扩大可见性；
11. 校验OpenAPI包含`model_id`以及其他已实现过滤参数；
12. 用Rust CLI回归`model status <target>`，断言不显示无关实例；
13. 用Rust CLI回归`model logs <target> --tail 80`，断言退出码0且返回目标日志；
14. 用Rust CLI回归`model deploy --wait`，断言无关error实例不会污染目标结果。

## 客户端兼容加固建议

Rust CLI可以把`list_model_instances_for_model`改为调用双方均已实现的：

```text
GET /v2/models/{id}/instances?page=-1
```

这能减少客户端对通用过滤实现的依赖，并使`model status/logs/wait`在当前Rust Server上恢复。但这是
防御性兼容加固，不能替代Rust Server修复：原始GPUStack公开的`model_id`过滤仍应对其他客户端、分页
和watch保持正确。

## Server重启观察的边界

诊断期间曾观察`127.0.0.1:18091`短暂拒绝连接，随后出现新的`gpustack-server`进程并恢复health 200。
直接Instance日志请求是在拒绝连接之后才准备执行，现有证据不能证明日志请求导致Server重启。因此：

- 本报告不把重启列为`model_id`过滤缺陷的后果；
- 不要求开发者按本报告结论接受日志导致崩溃的推断；
- 如需调查重启，应使用Server supervisor/core dump/退出码/服务日志建立独立证据链。

## 证据

106服务器正式裁决目录：

```text
/home/wujiaxing/.local/state/gpustack-rust-worker/evidence/m8rs6-106-inference-logs-adjudication-20261124T021651Z
```

关键文件：

- `instance-filter-diagnostic.json`：带/不带model_id及模型作用域接口对照；
- `cli-logs-retry-exit.txt`与`cli-logs-retry.stderr`：CLI稳定失败；
- `direct-instance-87-logs-status.txt`与`direct-instance-87-logs.txt`：精确日志接口PASS；
- `final-control-plane.json`：最终Worker、Model和Instance状态；
- `final-container.txt`、`final-gpu.txt`、`final-gpu-compute.txt`：运行面稳定性；
- `server-processes.txt`、`listeners.txt`、`health-status.txt`：Server重启后的可用性；
- `result.json`、`secret-scan.txt`、`SHA256SUMS`。

目录权限0700、文件权限0600；证据秘密扫描和manifest自校验均PASS。`SHA256SUMS`自身SHA-256：

```text
021533c45521e4088b170867a62bdea09e4941b3727ad6938d692a9bec1cbd97
```

## 当前资源状态

- Model 21继续`replicas=1`、`ready_replicas=1`；
- Instance 87继续running，绑定Worker 17、GPU0和端口40000；
- `gpustack-rs-managed-87`继续running且restart count 0；
- Worker 17为ready、maintenance disabled、unreachable false；
- 没有为复现执行POST、PUT、DELETE、maintenance更新或容器操作；
- CLI日志失败没有影响已经通过的`inference models`和chat completion链路。
