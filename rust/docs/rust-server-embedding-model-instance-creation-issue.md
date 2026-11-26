# Rust Server ModelInstance租户可见性不一致调查报告

## 1. 摘要

本报告修正此前“embedding Model未生成ModelInstance”的归因。2026-11-25的Model 23实验只通过
管理API Key执行ModelInstance list和watch；这两个接口会继续应用租户权限过滤。`page=-1`只取消
分页，不取消权限过滤，因此“完整可见列表中为0”不能证明数据库中没有实例。

Rust Server开发者随后完成三组验证：

1. 隔离环境中的普通Model和embedding Model均会自动生成ModelInstance；
2. 使用现场Model、ModelInstance和Worker数据执行副本对账，可为Model 23创建pending实例；
3. 106线上进程创建诊断embedding Model 24后，约1.2秒生成Instance 111，随后已清理。

2026-11-26项目又创建Model 25进行独立复核。数据库在约0.96秒后出现Instance 112，但同一管理
API Key的list仍返回0，watch也没有把实例事件交给CLI。这直接证明此前的观测存在租户可见性
假阴性，不能再归因于副本控制器。

当前确认的问题是创建和查询使用了不同的归属规则：管理员创建Model时使用Cluster归属
`owner_principal_id=1`；ModelInstance继承该归属；管理API Key（包括现场operator Key 32）属于
个人principal 4，list和watch按API Key主体过滤，因此看不到owner 1的实例。

Model 23已回滚，且缺少当时的数据库快照和Server日志，所以不能断言租户可见性是历史现象的唯一
原因。可以确定的是：原报告的“Server没有创建Instance”结论证据不足，副本控制器现已通过真实
数据库和线上创建实验验证。

## 2. 影响与归属

- 严重程度：P1，管理API Key无法可靠观察或等待自己创建的托管资源；
- 归属组件：Rust Server的Model创建归属、ModelInstance继承归属、list/watch租户上下文；
- 已排除归属：Rust Worker、Worker inference proxy、Docker、vLLM和GPU；
- 已验证正常：普通Model和embedding Model的副本创建与补偿对账；
- 当前阶段：`M8-EMB.7 waiting_on_rust_server_visibility_fix_deployment`；
- 首次错误归因：2026-11-25；修正时间：2026-11-26，Asia/Shanghai。

## 3. 环境与固定资源

| 项目 | 值 |
|---|---|
| Rust Server | `http://10.213.80.106:18091/` |
| 数据库 | PostgreSQL，数据库名`gpustack223` |
| Rust Worker | ID 17，`jiaxingHaoyuNB-106`，端口20150/20151 |
| GPU | index 0，UUID `GPU-be80b448-dd25-a523-676f-a279bf5c9c9c` |
| 原运行模型 | Model 21 / Instance 87 / port 40000 |
| 原容器 | `gpustack-rs-managed-87`，ID前缀`3233adac6f2a` |
| 历史实验 | Model 23，`m8emb7-bge-small-zh-gpu0` |
| 修正复验 | Model 25，`m8emb7-bge-small-zh-gpu0-r2` / Instance 112 |
| 固定模型 | `BAAI/bge-small-zh-v1.5@7999e1d3359715c523056ef9478215996d62a620` |
| 模型文件 | 11个，96,378,336字节，目录0555、文件0444 |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |

## 4. 归属不一致

当前实现的有效行为如下：

```text
管理员API Key创建Model
  -> 创建分支按Cluster确定owner_principal_id=1
  -> 副本控制器创建ModelInstance并继承owner_principal_id=1

同一管理API Key执行list/watch
  -> 查询按API Key自身principal 4建立TenantContext
  -> 只返回owner_principal_id=4的ModelInstance
  -> owner_principal_id=1的实例被权限条件过滤
```

这会导致同一次CLI部署出现互相矛盾的观测：Model POST成功，副本控制器也创建了Instance，但
`model deploy --wait`使用的watch和后续list都看不到它。

## 5. 已完成验证

### 5.1 开发者隔离验证

开发者报告的三组结果为：

| 检查 | 结果 |
|---|---|
| 隔离环境创建普通Model | 自动生成实例 |
| 隔离环境创建embedding Model | 自动生成实例 |
| 现场数据副本对账 | 可为Model 23生成pending实例 |
| 线上创建诊断Model 24 | 约1.2秒生成Instance 111 |

Model 24和Instance 111已由开发者清理。

### 5.2 等效管理Key可见性验证

开发者使用等效管理Key得到：

| 检查方式 | 结果 |
|---|---|
| 数据库直接查询 | 1个实例 |
| 管理Key查询实例列表 | 0个实例 |
| 管理Key的watch | 收不到实例事件 |
| 无租户限制的管理员会话 | 1个实例 |

### 5.3 项目独立复验

项目使用标准CLI创建Model 25，关键定义为：

```text
replicas=1
categories=["embedding"]
worker=jiaxingHaoyuNB-106
gpu=0
runner=pooling
local_path=<固定BGE目录>
```

复验结果：

1. Model 25成功创建，owner 1、Cluster 1、replicas 1；
2. PostgreSQL约0.96秒后出现Instance 112，owner 1、Cluster 1；
3. 同一管理API Key的`GET /v2/model-instances?page=-1`仍看不到Instance 112；
4. CLI watch没有收到该实例事件；
5. Instance 112随后由Worker推进并进入`error`，证明控制链已越过副本创建阶段；
6. 错误为`GPU index 0 has active compute processes`，源于原Instance 87仍占用GPU0；
7. Model 25及Instance 112已精确删除，原现场保持健康。

第6项是Worker的预期排他保护，不是Server副本控制器错误。`gpu-memory-utilization=0.1`限制vLLM
显存目标，但不会允许两个托管工作负载共享一个已有计算进程的GPU。

## 6. 对历史Model 23证据的重新解释

Model 23实验中以下事实仍成立：

- Model POST成功并保存了embedding、pooling、Worker和GPU selector；
- 管理Key的list和watch没有显示目标Instance；
- 没有观察到目标容器、端口或新增GPU进程；
- 实验资源被完整回滚。

但以下推断不再成立：

```text
list(page=-1)返回0
  != 数据库中一定没有Instance
  != 副本控制器一定没有运行
```

Model 23缺少当时的数据库快照和Server日志，不能追溯证明它是否已经创建过不可见Instance。因此
历史结论应记为`inconclusive_due_to_tenant_visibility`，而不是
`server_did_not_create_model_instance`。

## 7. `page=-1`的准确语义

`page=-1`只取消SQL分页的offset/limit，返回当前主体有权访问的全部条目。它不提升权限、不切换
主体，也不移除Cluster、Tenant或owner条件。正确表达是“全量可见清单”，不是“数据库全量清单”。

因此任何副本创建验收都必须同时证明：

1. 数据库或无租户限制的内部对账确实存在实例；
2. 发起部署的API主体能够通过list看到实例；
3. 同一主体能够通过watch收到实例事件；
4. Worker SYSTEM主体能够看到分配给自己的实例。

## 8. 修复要求

Rust Server需要统一以下四个位置使用的租户上下文：

1. Model创建时的`owner_principal_id`决定规则；
2. ModelInstance从父Model继承owner和Cluster的规则；
3. ModelInstance list的TenantContext和管理员覆盖规则；
4. ModelInstance watch的初始快照与后续事件过滤规则。

可接受方案必须定义清楚“平台管理API Key创建Cluster资源”究竟归Cluster主体还是个人主体，并让
创建、读取、watch、更新和删除都使用同一规则。不能只放宽list而保留watch过滤，也不能让管理Key
无条件看到其他Cluster或Tenant的数据。

## 9. 建议自动化测试

Rust Server至少应增加以下测试：

1. 个人principal管理Key创建Model后，list和watch都能看到其Instance；
2. Cluster principal创建Model后，同Cluster管理Key按明确策略可见；
3. 普通LLM与embedding使用完全相同的归属和副本生命周期；
4. `page=-1`取消分页但继续执行TenantContext过滤；
5. watch初始回放与实时事件使用相同权限规则；
6. 无权限Key不能读取或watch其他Cluster/Tenant的实例；
7. Model、Instance和ModelFile的owner/cluster字段保持一致；
8. reconcile重复执行不创建重复Instance；
9. Server重启后的补偿对账能恢复缺失副本；
10. 管理会话、管理API Key和Worker SYSTEM token分别覆盖正反权限用例。

## 10. M8-EMB.7后续验收

副本创建现已验证，不再等待controller修复。2026-11-26只读检查确认106仍运行
`lenovo`于2026-11-25 22:56启动的旧Server进程；项目复验的list/watch假阴性也发生在该进程上。
修复源码必须由Server开发者构建并重启为新进程，再先通过无GPU可见性门。最终链仍须在隔离维护
窗口中完成，因为原Instance 87
占用唯一GPU0，而Worker会正确拒绝并行启动第二个计算工作负载。下一次测试固定为：

1. 对原Worker 17启用maintenance；
2. 精确停止原Worker进程和原Instance 87容器，但不删除Model、Instance或容器；
3. 启动连接106 Rust Server的隔离Rust Worker；
4. 部署唯一BGE Model并验证数据库、list和watch可见性；
5. 验证完整状态链、float/base64、双输入和重复embedding请求；
6. 删除实验资源；
7. 启动同一原容器并以最新二进制恢复Worker 17；
8. 恢复原maintenance值并验证Model 21、Instance 87和端口40000。

该窗口必须使用一次性授权、精确身份封存、watchdog和幂等cleanup；修复后的Server尚未部署或
未取得明确授权时都不得执行。

## 11. 本次复验回滚

- Model 25和Instance 112已精确删除；
- 复验watchdog已停止并删除控制目录；
- 隔离Context已删除；
- 固定BGE目录和管理Key文件保留；
- 原Model 21、Instance 87和容器继续运行；
- Rust Server、Worker health/ready及原Qwen均为HTTP 200；
- 没有残留实验容器、端口、进程或Server资源。

因此当前可以安全准备隔离维护工具，但必须先由Server开发者部署修复并通过list/watch可见性复验，
之后才能申请独立维护授权并停止原Worker或模型。
