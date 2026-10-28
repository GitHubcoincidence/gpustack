# M6-B.4 Managed runtime 只读预检与故障注入

## 目的与结论边界

M6-B.4 在 90 服务器上证明 M6-B.5 的固定输入仍然可用，并用离线 fake/mock 测试证明
关键故障在真实副作用前失败。该阶段只能得出“可以讨论 M6-B.5 维护窗口”的结论，不能证明
真实 Managed runtime、Server 推理路由或 GPU 推理已经通过。

## 固定输入

| 项目 | 固定值 |
|---|---|
| 分支 | `codex/rust-worker` |
| 产品代码 | `c78cfc5652808ee5c16b40b11df411dadc7d6b6d` |
| Rust | `1.99.1` |
| Server | `v2.2.3`，commit `1cfcbbf` |
| Worker | ID `5`，name `rust-m3d-r760xa`，预检期间保持 maintenance |
| Python Worker | 只读记录精确容器身份，预检期间必须持续运行 |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| 模型 | `Qwen/Qwen2.5-0.5B-Instruct` revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 模型路径 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct` |
| 模型 manifest | `rust/fixtures/m5e/qwen2.5-0.5b-instruct-7ae557604adf.sha256`，SHA-256 `645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6` |
| GPU0 | `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| 端口 | Worker `20150/20151`；Managed `40000-40063` |

正式执行器绑定到包含本手册的干净治理 HEAD，并把该身份写入证据；治理 HEAD 不改变上表的
产品代码身份。

## 允许与禁止

允许：

- 读取 Git、服务健康、Server 版本和经既有 Cluster token 授权的 GET 资源；
- 读取 Docker daemon、固定镜像和容器元数据；
- 读取并校验模型 manifest；
- 读取 GPU、进程、端口和凭据文件元数据；
- 在 locked/offline 模式运行只使用 fake Server/runtime/filesystem 的指定 Rust 测试；
- 创建权限为 0700/0600 的 M6-B.4 证据目录和阶段执行器。

禁止：

- 停止或重启 Python Worker、改变 Worker maintenance；
- 对 Server 发起 POST、PUT、PATCH 或 DELETE；
- pull 镜像、创建、启动、停止或删除任何真实容器；
- 启动 Rust Worker、vLLM 或任何 GPU workload；
- 占用 GPU、发起推理或创建 M6-B.5 测试模型；
- 恢复或复用已经退役的 M6-A operator API Key。

## 正式检查

正式证据必须同时证明：

1. 本地、GitHub 和 90 的治理 HEAD 一致，90 工作树在执行前后均干净；
2. Server、Python Worker health 和 metrics 均为 HTTP 200，Server identity 不漂移；
3. Worker ID 5 唯一、仍为 maintenance 且无 ModelInstance，`m6b` 测试模型不存在；
4. Python Worker 精确容器持续运行，前后全容器 ID 集和 Rust-owned 容器集不变；
5. Docker daemon 可读，固定 Runner digest 已存在，不执行 pull；
6. 模型 canonical path、revision identity、10 个文件、999,604,126 bytes、权限、架构、
   chat template 和完整 manifest 均匹配；
7. GPU0 UUID 不变、显存和利用率为 0、没有 compute process；GPU1 只记录且不修改；
8. 没有 `gpustack-rs` 进程，`20150/20151` 和 `40000-40063` 没有监听；
9. 当前没有活动 operator Context。该结果不是 M6-B.4 失败，但必须作为 M6-B.5 前置缺口；
   M6-B.5 只能使用新建的阶段专用、最小权限、短期 API Key；
10. 证据不包含 token 内容，目录为 0700、文件为 0600，并有完整 SHA-256 清单。

## 离线故障注入

在相同治理 HEAD、固定 `Cargo.lock` 和 `--offline` 下运行以下精确 fake/mock 用例：

- 缺少模型根目录时，在任何 runtime 或网络工作前失败；
- 创建容器前发生权威输入漂移时，不产生 create/start；
- 健康后、`RUNNING` 写回前发生状态漂移时，不覆盖 Server 状态且撤销内部 route；
- vLLM readiness 超时时写有界错误并保留诊断容器的 fake 状态，不执行误删；
- 缓存快照缺口不能授权清理权威实例；
- 推理代理只能使用内部 loopback route，并删除敏感和 hop-by-hop headers。

这些测试只能证明代码失败边界，不替代 M6-B.5 的真实 Docker、Server 和 GPU 证据。

## 判定与停止条件

- 所有只读门、故障注入和前后不变性比较通过时，结果为 `PASS/ready_for_review`。
- 任一服务、身份、模型、GPU0、端口、容器或源码门不满足时，结果为 `STOP`；不得自动修复
  外部状态，也不得进入维护窗口。
- 用户审阅 M6-B.4 报告前不得把结果标记为 `accepted`。
- M6-B.4 即使接受，也只允许下一步讨论、冻结并请求 M6-B.5 的精确维护窗口授权。
