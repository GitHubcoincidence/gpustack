# Rust Server缺陷报告：Managed状态链跳过ANALYZING和DOWNLOADING

## 摘要

Rust Server在真实`local_path` embedding部署中创建了Model 28、Instance 115和ModelFile 23，Rust
Worker也成功启动固定vLLM pooling容器并最终上报`running`。但是ModelInstance watch完整观察到的
状态只有：

```text
pending -> scheduled -> initializing -> starting -> running
```

`analyzing`和`downloading`从未出现。这与GPUStack v2.2.3及本项目冻结的状态所有权契约不一致：
`analyzing`属于Server/Scheduler，`downloading`属于Server ModelFile Controller；即使模型是已存在
的本地路径，也必须经过ModelFile协调阶段。

- 报告日期：2026-11-26
- 环境：`10.213.80.106:18091`
- Model：28，`m8emb7-bge-small-zh-106`
- ModelInstance：115
- ModelFile：23
- Worker：418，`rust-m8emb7-106`
- 模型：固定`BAAI/bge-small-zh-v1.5`，512维
- 结果：`FAIL/incomplete_managed_state_sequence`
- 问题主归属：Rust Server
- Worker运行结果：vLLM容器实际到达running，但推理评分因状态门失败而未执行

## 期望契约

冻结的完整状态链为：

```text
PENDING -> ANALYZING -> SCHEDULED -> INITIALIZING
                                      |
                                      v
                                DOWNLOADING -> STARTING -> RUNNING
```

状态所有权：

1. Server/Scheduler创建`pending`并推进`analyzing`、`scheduled`；
2. Worker接收自己的`scheduled`实例并写回`initializing`；
3. Server为实例创建或关联ModelFile，并推进`downloading`；
4. Worker把本地ModelFile校验为`ready`；
5. Server观察文件ready后推进`starting`；
6. Worker仅在`starting`时创建/接管vLLM，健康后写回`running`。

`rust/docs/worker-protocol.md`明确规定本地模型也进入`downloading`协调阶段，只是不执行网络下载。
因此不能用“文件已经在本地”解释完全跳过该状态。

## 实际事件

ModelInstance watch在创建前已经建立，并返回HTTP 200。目标Instance 115的全部事件为：

| 事件类型 | 状态 | `updated_at` |
|---:|---|---|
| 1 | pending | 2026-11-26T06:38:26.560753Z |
| 2 | scheduled | 2026-11-26T06:38:26.574064Z |
| 2 | initializing | 2026-11-26T06:38:28.208310Z |
| 2 | starting | 2026-11-26T06:38:31.580619Z |
| 2 | running | 2026-11-26T06:39:46.405700Z |

事件流不是只漏掉最后一条：`pending`、Server调度、Worker初始化、Server启动许可和Worker运行写回均
按顺序可见，只有两个Server拥有的中间状态缺失。轮询也没有观察到这两个状态。

## Worker与运行时对照

以下事实证明实例没有停在Worker之前：

- Instance 115绑定Worker 418和GPU0；
- 唯一容器标签包含实验Worker UUID和Instance 115；
- 容器使用固定Runner digest和`--runner=pooling`；
- DeviceRequest只包含冻结的GPU0 UUID；
- vLLM最终健康，Instance到达`running`；
- 原Qwen容器在维护窗口中被精确停止，没有GPU竞争。

因此本缺陷不是Rust Worker拒绝调度、Docker启动失败或GPU不足。Worker只能负责`initializing`和
`running`写回，不能自行补造Server拥有的`analyzing/downloading`事件。

## 影响

1. 与原GPUStack状态机不兼容；
2. CLI `model deploy --wait`无法提供完整状态历史；
3. 运维无法区分模型分析、文件准备和运行时启动耗时；
4. ModelFile失败可能被错误归入初始化或启动阶段；
5. 依赖完整状态链的兼容测试必须失败关闭；
6. 如果直接放宽客户端门，会掩盖Server Controller缺少状态写回的问题。

## 建议修复

1. Model创建后先持久化`pending`，由分析/调度控制器显式写入`analyzing`；
2. 完成资源估算后再写入`scheduled`；
3. Worker写入`initializing`后，由ModelFile Controller显式写入`downloading`；
4. 本地路径也创建/关联ModelFile并保持同一状态流程；
5. 只有关联文件全部ready后才写入`starting`；
6. 每次状态更新都发布watch事件，不能只更新内存快照或合并掉中间状态；
7. 状态写入和事件发布应保持单调顺序，重复事件允许但不能倒退；
8. 不要让Worker写入`analyzing/downloading`来掩盖Server职责缺口。

## 建议测试

1. 普通LLM和embedding各执行一次完整部署；
2. `local_path`和需要下载的远程模型分别测试；
3. 断言watch按序包含七个状态，允许重复但不允许缺失或倒退；
4. 在ModelFile校验前暂停Worker，断言Instance稳定停在`downloading`；
5. 让ModelFile进入error，断言错误从`downloading`路径产生；
6. 断开并重连watch，断言全量快照与后续事件一致；
7. 使用`model deploy --wait`验证转换历史完整；
8. 多副本并行时每个Instance独立满足状态链；
9. 不同租户主体只能观察授权范围内的状态事件。

## 证据

正式证据：

```text
/home/wujiaxing/.local/state/gpustack-rust-worker/evidence/
  m8emb7-106/formal-1393536b-attempt1
```

manifest SHA-256：

```text
d1517fc0a8ded72711168f66fb840a3c9d3967e41722cb2b9db9e4ca67be90a1
```

关键文件：

- `model-instance-watch.body`：完整目标事件流；
- `model-create-request.json`：固定ModelCreate字段；
- `model-create-response.json`：Model 28身份；
- `runtime-state.json`：实验Worker和Model身份；
- `result.txt`：正式失败阶段和原因。

容器、GPU、镜像、命令和标签在精确恢复前保存于：

```text
m8emb7-106/manual-recovery-1393536b-attempt1/
  experimental-container-before-remove.json
```

该目录manifest SHA-256为
`097263337a3b1dc56eb31c15d4d380ead1a245ad2ce3dc7a8447435abb97a2c6`。该次运行已完整恢复原
Worker、Model、Instance、容器和GPU现场；恢复证据见执行日志。
