# Rust Server embedding Model不生成ModelInstance缺陷报告

## 1. 摘要

106上的Rust Server已经能够处理`POST /v1/embeddings`，并可把请求转发给现有running实例；但通过
标准Rust CLI创建一个`replicas=1`的embedding Model后，Server只持久化了Model，没有生成任何
ModelInstance。由于Server托管链停在Model到Instance的副本同步阶段，目标Rust Worker没有收到
workload，Docker、vLLM和GPU阶段均未开始。

本问题阻断以下正式链路：

```text
Rust CLI -> Rust Server -> 106 Rust Worker -> vLLM /v1/embeddings
```

本报告只讨论Model副本控制器问题。`/v1/embeddings`未进入OpenAPI以及认证前模型查找顺序不属于
本次阻断范围。

## 2. 严重程度与归属

- 严重程度：P1，阻断Rust Server托管的新Model部署；
- 归属组件：Rust Server的Model创建事件、副本控制器或ModelInstance创建逻辑；
- 非归属组件：Rust Worker、Worker inference proxy、Docker runtime、vLLM和GPU；
- 当前阶段：`M8-EMB.7 waiting_on_rust_server_instance_creation`；
- 首次确认时间：2026-11-25，Asia/Shanghai。

## 3. 环境

| 项目 | 值 |
|---|---|
| Rust Server | `http://10.213.80.106:18091/` |
| Server运行方式 | `gpustack-server ... --port 18091 --require-auth` |
| 数据库 | `postgresql://.../gpustack223` |
| Server产品版本 | v2.2.3兼容Rust实现，开发构建未以Git commit作为测试门 |
| Rust Worker | ID 17，`jiaxingHaoyuNB-106`，ready，非maintenance |
| Worker API | `10.213.80.106:20150` |
| GPU | index 0，RTX 5090，UUID `GPU-be80b448-dd25-a523-676f-a279bf5c9c9c` |
| 原运行模型 | Model 21 / Instance 87 / Worker 17 / GPU0 / port 40000 |
| 原容器 | `gpustack-rs-managed-87`，ID前缀`3233adac6f2a` |
| 实验模型 | Model 23，`m8emb7-bge-small-zh-gpu0` |
| 固定模型 | `BAAI/bge-small-zh-v1.5@7999e1d3359715c523056ef9478215996d62a620` |
| 本地模型目录 | `/home/wujiaxing/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d` |
| 模型文件身份 | 11个文件，96,378,336字节，目录0555、文件0444 |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| Worker代码HEAD | `160070df248ec266270dd9aedf592b6f468337af`，运行二进制报告产品commit `668bf92ba353` |

正式写入前连续5个样本均确认Rust Server只有一个进程且`/healthz=200`，排除了双进程部署竞争。

## 4. 前置功能证据

新embedding接口不是本问题的失败点。使用Rust Server上现成的Model 17
`Qwen3-VL-Embedding-2B-new`、Instance 98和Worker 24完成了以下请求：

- 单输入float：成功，2048维；
- 单输入base64：成功，按float32解码为2048维；
- 双输入float：成功，返回两个2048维向量；
- 三次重复float：全部成功；
- 请求前后Instance 98保持`running`，`restart_count=1`。

该证据说明Rust Server的新推理路由、模型解析、running目标选择和Worker转发可以工作，但现成模型
运行在Worker 24，不能替代Worker 17的新部署链。

## 5. 复现步骤

### 5.1 准备受保护Context

使用有效的管理API Key文件。文件必须是0600普通文件，命令中只传文件路径，不输出token：

```bash
export GPUSTACK_RS_CONTEXTS_FILE="$HOME/.config/gpustack-rust/m8emb7-contexts.json"

gpustack-rs --output json context add m8emb7-rust-server \
  --server-url http://10.213.80.106:18091/ \
  --token-file "$HOME/.config/gpustack-rust/<management-token-file>" \
  --use
```

### 5.2 确认目标Worker和模型路径

```bash
gpustack-rs --output json worker get 17
```

通过条件：Worker 17为`ready`、`maintenance.enabled=false`，并上报唯一GPU0。

### 5.3 使用标准CLI创建Model

```bash
gpustack-rs --output json model deploy m8emb7-bge-small-zh-gpu0 \
  --worker jiaxingHaoyuNB-106 \
  --local-path "$HOME/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d" \
  --image 'quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af' \
  --gpu 0 \
  --category embedding \
  --runner pooling \
  --tensor-parallel-size 1 \
  --max-model-len 512 \
  --gpu-memory-utilization 0.1 \
  --dtype auto \
  --enforce-eager \
  --timeout 840 \
  --wait
```

CLI在POST之前会建立ModelInstance watch，然后调用`POST /v2/models`。本次POST成功并返回Model 23，
但`--wait`持续等待，不产生任何状态转换。

### 5.4 请求JSON

CLI发送的业务body等价于：

```json
{
  "cluster_id": 1,
  "name": "m8emb7-bge-small-zh-gpu0",
  "source": "local_path",
  "categories": ["embedding"],
  "local_path": "/home/wujiaxing/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d",
  "replicas": 1,
  "backend": "vLLM",
  "image_name": "quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af",
  "worker_selector": {
    "worker-name": "jiaxingHaoyuNB-106"
  },
  "gpu_selector": {
    "gpu_ids": ["jiaxingHaoyuNB-106:cuda:0"],
    "gpus_per_replica": 1
  },
  "backend_parameters": [
    "--tensor-parallel-size=1",
    "--runner=pooling",
    "--max-model-len=512",
    "--gpu-memory-utilization=0.1",
    "--dtype=auto",
    "--enforce-eager"
  ],
  "distributed_inference_across_workers": false,
  "enable_model_route": true
}
```

## 6. 实际结果

Server成功持久化Model：

```json
{
  "id": 23,
  "name": "m8emb7-bge-small-zh-gpu0",
  "categories": ["embedding"],
  "replicas": 1,
  "ready_replicas": 0,
  "cluster_id": 1,
  "worker_selector": {
    "worker-name": "jiaxingHaoyuNB-106"
  },
  "gpu_selector": {
    "gpu_ids": ["jiaxingHaoyuNB-106:cuda:0"],
    "gpus_per_replica": 1
  },
  "owner_principal_id": 1,
  "created_at": "2026-11-25T09:49:49.956879Z",
  "updated_at": "2026-11-25T09:49:49.956879Z"
}
```

观察结果：

1. `model deploy --wait`等待超过3分钟，没有收到ModelInstance状态事件；
2. 随后对`GET /v2/model-instances?page=-1`执行10次、间隔10秒的全量读取；
3. 每次在客户端按`model_id == 23`精确过滤，结果均为0；
4. 没有出现目标容器；
5. 没有出现新40000段端口；
6. GPU上没有新增计算进程；
7. Model 23的`updated_at`仍等于`created_at`。

最终判定：

```text
STOP/server_did_not_create_model_instance
```

## 7. 预期结果与Python Server权威行为

原GPUStack Python Server的Model controller在收到Model事件后执行：

```text
Model CREATED/UPDATED event
  -> ModelController._reconcile
  -> sync_replicas(session, model)
  -> re-fetch Model
  -> query instances by model_id
  -> if instance_count < replicas:
       create ModelInstance(state=pending)
```

本次Model 23满足：

```text
replicas = 1
instances(model_id=23) = 0
```

因此Rust Server应创建恰好一个pending ModelInstance，并继承至少以下字段：

- `model_id=23`；
- `model_name=m8emb7-bge-small-zh-gpu0`；
- `cluster_id=1`；
- `owner_principal_id=1`；
- `source=local_path`；
- 固定`local_path`；
- `backend=vLLM`；
- `state=pending`。

之后scheduler才根据父Model的Worker/GPU selector把它分配给Worker 17和GPU0。即使调度条件不满足，
ModelInstance也应先存在并保持pending；不应完全没有Instance。

## 8. 已排除原因

### 8.1 不是请求校验或持久化失败

`POST /v2/models`成功，Model 23可立即GET，所有关键字段均正确。

### 8.2 不是Worker selector错误

Worker 17存在、ready、非maintenance，并有规范标签：

```json
{
  "implementation": "rust",
  "worker-name": "jiaxingHaoyuNB-106"
}
```

即使selector不匹配，也应先得到pending Instance，而不是0个Instance。

### 8.3 不是GPU或显存不足

原模型占用约17.3 GiB，GPU总显存约32.6 GiB；实验模型显存比例固定为0.1。更重要的是，本次从未
进入scheduler、Worker或runtime阶段，没有任何Instance可供资源评估。

### 8.4 不是模型文件问题

固定BGE目录存在、只读、11个文件、96,378,336字节。ModelFile准备只会在Instance分配后由Worker
执行，本次未到达该阶段。

### 8.5 不是Worker或Docker失败

Worker health/ready始终为200；原Qwen容器和端口正常；实验容器从未创建。Worker没有收到目标
ModelInstance，自然不会启动vLLM。

### 8.6 不是多Server进程竞争

之前部署过程中曾出现双进程，正式写入前已由开发方处理。最终尝试前连续5个样本均只有一个Server
进程且health 200。

### 8.7 不是ModelInstance列表过滤假阴性

Rust Server存在`model_id`列表过滤差异，因此本次没有仅依赖
`GET /v2/model-instances?model_id=23`。验证使用`page=-1`读取完整可见清单，再在客户端按
`model_id == 23`过滤，10次结果仍全部为0。

## 9. 建议排查位置

建议按以下顺序检查Rust Server：

1. `POST /v2/models`事务提交后是否发布Model CREATED事件；
2. Model副本controller是否在当前Server进程中启动并保持运行；
3. controller是否实际订阅到Model CREATED/UPDATED事件；
4. leader选举或单实例controller门是否错误地认为当前进程不是leader；
5. reconcile是否因`categories=["embedding"]`提前过滤或返回；
6. reconcile是否因`owner_principal_id=1`、`cluster_id=1`或selector反序列化失败；
7. reconcile异常是否只写日志、未反映到Model `state_message`或API响应；
8. Model查询得到的`replicas`是否错误为0或空值；
9. ModelInstance create是否失败但事件被确认或丢弃；
10. Server重启后是否会补偿扫描`replicas > instance_count`的Model；若没有，单次事件丢失会永久卡住。

请优先查看Model 23创建时间附近的Server日志：

```text
2026-11-25T09:49:49Z
```

建议增加结构化日志，至少包含`model_id`、`replicas`、现有Instance数、事件类型、controller身份和
ModelInstance创建结果，但不得记录API Key或Authorization header。

## 10. 建议修复

最低修复要求：

1. Model创建事务成功后可靠触发一次副本同步；
2. 对`replicas=1、instances=0`创建恰好一个pending Instance；
3. embedding category和普通LLM走同一副本生命周期，不为embedding增加第二套controller；
4. reconcile幂等，重复事件或Server重启不得产生重复Instance；
5. reconcile失败应有可观测错误，并能重试或由周期性补偿扫描恢复；
6. Model删除时清理其Instance，且不会影响其他Model。

不建议的绕过方案：

- CLI手工`POST /v2/model-instances`；
- CLI直接连接Worker或vLLM部署；
- Worker根据Model列表自行伪造Instance；
- 仅为embedding增加特殊部署路径。

这些方案会绕过Server的状态所有权、调度、watch和清理语义。

## 11. 建议自动化测试

Rust Server应增加至少以下集成测试：

1. 创建普通LLM Model，`replicas=1`，最终存在恰好一个pending Instance；
2. 创建embedding Model，`categories=["embedding"]`，结果与普通LLM一致；
3. Instance继承model、cluster、owner、source、path和backend字段；
4. 重复reconcile不创建第二个Instance；
5. Server重启或controller重新订阅后可补偿缺失Instance；
6. replicas从1改为0后精确删除目标Instance；
7. 删除Model后无孤儿Instance；
8. ModelInstance watch能观察到初始pending事件；
9. controller异常可观察且不会静默吞掉永久状态；
10. 测试同时覆盖`categories=[]`和`categories=["embedding"]`。

测试不需要真实GPU、Docker或模型文件；可使用测试数据库和fake scheduler边界，只验证控制面副本同步。

## 12. 修复验收标准

修复部署后，项目将使用新的唯一模型名重跑。通过必须同时满足：

1. Server只有一个稳定进程；
2. `POST /v2/models`成功；
3. 30秒内出现恰好一个目标ModelInstance；
4. 初始状态至少可观察到pending，watch不丢失目标事件；
5. Instance字段与父Model一致；
6. scheduler将其绑定Worker 17、GPU0和新的40000段端口；
7. Worker完成ModelFile、容器启动和running状态链；
8. Rust CLI经Rust Server完成float、base64、双输入和重复embedding请求；
9. 删除实验Model后Instance、容器、端口、GPU进程和stage文件归零；
10. 原Model 21 / Instance 87 / 容器和Qwen健康保持不变。

## 13. 本次回滚结果

本次失败没有遗留实验资源：

- Model 23已精确删除；
- 目标ModelInstance为0；
- 实验容器、端口和GPU进程为0；
- 30分钟watchdog已disarm并删除；
- 隔离Context已删除；
- 原管理API Key文件保留且权限仍为0600；
- 原Instance 87仍为running；
- 原容器`3233adac6f2a`继续运行；
- Rust Server、Rust Worker、Worker readiness和原Qwen健康均为HTTP 200。

因此开发者可以只修复Rust Server控制面，无需处理106 Worker现场或恢复GPU资源。
