# Rust Server缺陷报告：Model PUT返回200但忽略description更新

## 摘要

Rust Server的`PUT /v2/models/6`接受了包含完整可写字段的JSON请求，返回HTTP 200并发布了一条
Model UPDATED watch事件，但请求中唯一有意修改的`description`没有被持久化：PUT响应、watch事件
和随后独立GET均返回旧description。Server只推进了`updated_at`，事件的`changed_fields`仍为空。

- 报告日期：2026-11-15
- Rust Server：`v2.2.3`，commit `fba334c`，implementation `rust`，build `0.1.0`
- 目标资源：Model ID 6，`rust-rs5-scope-90-20261115`，Cluster 3
- 问题归属：Rust Server
- 阶段结论：`FAIL/update_ignored_description`
- 副作用清理：PASS；Model继续保持replicas 0，未创建Instance/File/容器或GPU任务

## 接口作用

原GPUStack通过：

```text
PUT /v2/models/{id}
```

更新Model的可写定义。该接口不仅用于副本数，也应处理description、source、selectors、backend配置
等`ModelUpdate`字段，并返回持久化后的资源。Model watch应在更新提交后发布与真实存储状态一致的
UPDATED事件。

## 前置状态

写入前已确认：

- Model 6精确唯一，属于Cluster 3；
- `description="M8-RS.5 isolated stopped model; no scheduling"`；
- `replicas=0`、`ready_replicas=0`；
- source为`local_path`，backend为`vLLM`；
- Worker selector只绑定`rust-rs3-worker-90-20261113`；
- `gpu_selector=null`，不启用跨Worker分布式推理；
- Worker 16处于maintenance；
- Model 6没有ModelInstance，Worker 16没有ModelFile；
- 目标Worker未运行，21150/21151空闲，Docker和两张A100为空载基线。

## 请求

管理主体只发送了一次请求，没有重试：

```http
PUT /v2/models/6 HTTP/1.1
Content-Type: application/json
Authorization: Bearer <REDACTED>
```

请求由即时GET响应按可写字段白名单构造。唯一有意修改项为：

```json
{
  "description": "M8-RS.5 watch update verified; still stopped"
}
```

完整请求同时原样携带：

- name、source和本地模型路径；
- `replicas=0`；
- Cluster 3；
- placement、Worker selector和`gpu_selector=null`；
- vLLM及backend parameters；
- restart、distributed inference、access policy等现有可写字段。

请求中的所有归属、调度和停止态字段与写入前快照完全一致。

## 实际响应与持久状态

### PUT响应

传输结果：

```text
curl exit: 0
HTTP: 200
PUT attempts: 1
```

但是响应仍为旧值：

```json
{
  "id": 6,
  "description": "M8-RS.5 isolated stopped model; no scheduling",
  "cluster_id": 3,
  "replicas": 0,
  "ready_replicas": 0
}
```

### 独立GET

PUT之后再次请求：

```http
GET /v2/models/6
```

仍返回旧description，证明并非PUT响应缓存或响应构造错误，而是新值没有持久化。

### UPDATED watch事件

写入前，Worker 16 SYSTEM Token已成功建立：

```http
GET /v2/models?watch=true
Accept: text/event-stream
```

同一连接先收到Model 6的初始`type=1`事件，PUT后又恰好收到一条Model 6的`type=2`事件。UPDATED事件
中的关键值为：

```json
{
  "type": 2,
  "id": 6,
  "data": {
    "description": "M8-RS.5 isolated stopped model; no scheduling"
  },
  "changed_fields": {}
}
```

事件中的`updated_at`从`2026-11-15T12:19:37.519531Z`推进到
`2026-11-15T12:54:14.491985Z`，但description和其他业务字段没有变化。这表明Server执行了一次
无业务字段变更的更新时间戳写入，并发布了内容为空的UPDATED事件。

## 期望行为

按原GPUStack v2.2.3 commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`：

- `gpustack/routes/models.py::update_model`接收`ModelUpdate`；
- `ModelService.update`应把请求提供的可写字段应用到Model；
- 返回值应为提交后的Model；
- watch事件应反映提交后的真实数据。

因此，本次应满足：

```text
PUT response.description == requested description
GET response.description == requested description
UPDATED event.data.description == requested description
```

原v2.2.3还有一项独立特殊语义：非Custom backend更新时，现有`image_name`和`run_command`会被设为
null。本次Rust Server保留了固定Runner image。该差异应单独按原版决定是否修正，但不能解释
description被忽略。

## OpenAPI相关观察

当前`/openapi.json`中，`PUT /v2/models/{id}`的requestBody引用：

```text
#/components/schemas/ModelCreate
```

但当前components实际提供的名称是`ModelsCreate`，没有`ModelCreate`。这是一项独立的OpenAPI引用
错误，会影响代码生成和调用方判断可写字段；运行时仍接受了本次JSON，因此它不是description丢失的
充分解释，但建议与handler修复一并处理。

## 可能的Rust Server排查方向

以下是根据wire表现提出的假设，不代表已读取Rust Server源码后的最终根因：

1. PUT handler可能只显式处理`replicas`等少数字段，忽略description；
2. request DTO与数据库update DTO之间的字段映射可能遗漏description；
3. update方法可能只更新时间戳并无条件发布UPDATED事件；
4. handler返回了更新前对象，且事务中也没有真正写入description；
5. OpenAPI使用了错误或不完整的Create/Update schema，运行实现与文档共同漂移。

## 建议修复范围

1. 为PUT定义与原版`ModelUpdate`一致的明确请求DTO；
2. 把所有允许更新且请求中提供的字段传入持久化层；
3. 提交后重新读取并返回真实数据库记录；
4. UPDATED事件必须携带提交后的数据；
5. 明确无业务字段变化时是否应发布UPDATED，避免仅更新时间戳的空事件；
6. 按原版处理非Custom backend的`image_name/run_command`特殊语义；
7. 修正OpenAPI requestBody引用和schema名称；
8. 保留现有认证、Cluster可见性和平台管理员权限边界。

## 建议回归测试

1. 创建`replicas=0`的Cluster 3 Model，description为A；
2. 建立SYSTEM主体Model watch并确认初始事件；
3. 完整PUT，只把description从A改为B；
4. 断言PUT响应description为B；
5. 断言独立GET description为B；
6. 断言收到恰好一条目标UPDATED事件，data.description为B；
7. 断言Cluster、replicas、source、path、backend参数和selectors未改变；
8. 断言没有创建ModelInstance、ModelFile、容器或GPU任务；
9. 增加description以外的代表字段更新用例，防止handler只特判一个字段；
10. 单独覆盖非Custom backend的image/run command原版语义；
11. 校验OpenAPI中的PUT schema引用真实存在且与运行DTO一致。

## 证据

90服务器受保护证据目录：

```text
/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m8rs5-model-watch-update-20261115T125249Z
```

关键文件：

- `model-before.json`
- `model-update-request.json`
- `model-update-response.json`
- `model-after.json`
- `put-transport.txt`
- `watch-body.txt`
- `watch-events.ndjson`
- `watch-events.json`
- `formal-result.txt`
- `secret-scan.txt`
- `SHA256SUMS`

目录权限0700、文件权限0600；management key、Cluster registration token、Worker token逐值扫描和
通用凭据模式扫描全部通过。`SHA256SUMS`自身SHA-256：

```text
cc28aa021b59c3a2fbfd76cc6ddc8b7019434badaa1be71adb6bac194212d9af
```

## 当前资源状态

- Model 6仍存在且保持原description；
- `replicas=0`、`ready_replicas=0`；
- ModelInstance为0，Worker 16 ModelFile为0；
- Worker 16继续maintenance；
- 未执行第二次PUT，未回滚，未删除Model；
- Docker、GPU、端口和独立`jiaxingHaoyuNB` Worker前后不变。
