# M7-F 本机双 GPU TP=2 验证方案

## 1. 目标与结论边界

M7-F 要证明：宿主机 Rust Worker 能在单 Worker、本地 Docker 和固定 vLLM Runner 范围内，
把同一 ModelInstance 的有序 GPU assignment `[0, 1]`安全转换为双 GPU WorkloadSpec、Docker
DeviceRequests 和`tensor_parallel_size=2`，并完成完整 Managed 状态链、Server-only 推理、日志、
幂等协调和零残留删除。

本阶段不比较 TP=1/TP=2 性能，不支持 TP>2、跨 Worker分布式推理、Kubernetes、其他后端、GPU
热插拔、MIG或容器化Rust Worker。M7-F是宿主机阶段功能证据；M8仍须在容器化Worker上重跑后
才能形成R1 candidate。

## 2. 产品契约

- `model deploy`继续只允许一个Worker和`distributed_inference_across_workers=false`。
- Managed workload允许一张或两张本机NVIDIA GPU；Lab继续严格限制为单GPU、TP=1。
- GPU index在CLI中规范化为升序；Server返回的ModelInstance assignment必须是1～2个严格递增、
  不重复的index。M7-F正式输入固定为`[0, 1]`。
- 每个index必须解析为当前Worker库存中的稳定GPU UUID；首次创建前所有选中GPU必须无计算进程。
- `tensor_parallel_size`等于选中GPU数量。显式`--tensor-parallel-size`若存在，必须与assignment
  一致；缺省时由assignment数量唯一推导。
- WorkloadSpec、spec hash、managed manifest和Docker DeviceRequests按同一顺序保存全部GPU。
  UUID、顺序、数量或TP任一漂移都必须fail closed。
- Docker只收到选中UUID；vLLM命令使用同一TP值。重复协调、启动接管、restart和删除继续使用完整
  ownership tuple与spec，不按容器名称或部分GPU信息宽松处理。

## 3. 固定正式输入

| 项目 | 固定值 |
|---|---|
| Server | GPUStack v2.2.3，commit `1cfcbbf` |
| Worker | ID 5，`rust-m3d-r760xa`，固定persisted UUID |
| 模型 | 已缓存`Qwen2.5-0.5B-Instruct`固定revision、local path和全文件manifest |
| Runner | 已验证的固定`quay.io/gpustack/runner` digest |
| assignment | GPU0 UUID后接GPU1 UUID，indexes `[0, 1]` |
| vLLM | TP=2、max model length 2048、GPU memory utilization 0.5、dtype auto、eager |
| 网络与端口 | host network，Managed端口池`40000-40063` |
| 服务入口 | 只把Server-only models/chat/completion/logs作为正式用户路径 |

正式窗口要求GPU0和GPU1同时空闲。当前GPU1外部DiffKV任务不是本项目所有物，不得停止或修改；
它只会让后续preflight安全STOP，不阻塞M7-F.1离线实现。

## 4. 分段门禁

### M7-F.0 方案冻结

只记录目标、产品契约、固定输入、正式验收和授权边界，不创建凭据、不停止服务、不使用GPU。

### M7-F.1 离线产品实现

1. 泛化Managed workload GPU契约，同时保留Lab单GPU限制。
2. 泛化Managed planner的assignment、GPU库存解析、空闲门和TP解析。
3. 泛化reconciler scope门，使1～2张严格有序GPU进入已有状态机。
4. 让CLI确定性排序GPU，并补齐双GPU ModelCreate wire测试。
5. 为spec hash、DeviceRequests、vLLM命令、接管、restart和删除补齐TP=2正反回归。

本门只使用fake Server/runtime/GPU/filesystem，不连接真实Docker socket或Server。

### M7-F.2 离线质量门

执行Rustfmt、目标及workspace测试、Clippy和locked/offline build。必须证明TP=1既有路径不回归，
Lab多GPU仍拒绝，并拒绝重复/逆序index、TP不匹配、任一GPU繁忙、UUID缺失、spec或ownership漂移。

### M7-F.3 凭据与只读preflight

另行授权后才创建最长7天、`management,inference`双scope的阶段API Key和Context。只读门固定
源码、脚本、模型、Runner、Worker、Python Worker恢复对象、两张A100 identity、端口和清理材料；
两张GPU任一存在计算进程都必须STOP。错误HEAD注入必须在窗口前无写停止。

### M7-F.4 正式维护窗口

必须再次单独授权。窗口使用独立watchdog，精确停止同一Python Worker，启动宿主机Rust Worker，
经CLI创建唯一TP=2模型并验证完整状态链。结束或失败后删除唯一测试资源和完整owned容器，恢复
同一Python Worker并运行独立post-audit。

### M7-F.5 审阅与凭据退役

formal、watchdog和post-audit同时通过后只进入`ready_for_review`。用户接受后删除阶段API Key、
Context和token文件并验证旧token为401，随后M7-F才能标记`accepted`；不得自动进入M8。

## 5. 正式验收门

- Server assignment精确为同一Worker的`gpu_indexes=[0,1]`，不包含distributed servers。
- WorkloadSpec、manifest、vLLM命令和Docker DeviceRequests一致表达两个固定UUID和TP=2。
- 两张GPU均出现属于唯一Rust-owned容器的计算进程和显存；受控推理采样中两张卡都观察到计算
  活动，不能把其他宿主机进程计入实验。
- ModelInstance完成冻结的完整Managed状态链并保持`RUNNING`；restart count为0、last restart为空。
- Server-only models、buffered/streaming chat、completion和current logs全部成功；错误模型仍拒绝。
- 至少三轮2秒协调保持同一Model/Instance/File、容器ID、端口、spec、route和日志generation 0，
  不产生第二个容器或状态回退。
- 删除后测试Model/Instance/File、Rust容器/进程、端口、manifest/cache和两张GPU上的实验进程
  全部归零；固定本地模型文件保留，同一Python Worker和三个健康端点恢复。

任一GPU不空闲、assignment/UUID/TP不一致、只有一张GPU实际活动、推理或日志失败、重复容器、
cleanup/watchdog/post-audit失败，都使formal attempt失败；部分状态链或仅恢复成功不能改判。

## 6. 当前停止点

用户已接受详细实现与验证报告，M7-F.1产品实现和M7-F.2离线质量门现为`accepted`。实现提交
`240a5409fee2abde1f8f798148edfe4856784e8f`让CLI、WorkloadSpec、Managed planner、调度前置条件
和runtime协调器贯通同Worker双GPU TP=2，同时保留Lab单GPU边界；格式修正提交为
`5f0318d2b694b2c9491d823dcbddd670407cbea3`。

90上的Rustfmt、全部workspace all-targets locked/offline测试、Clippy `-D warnings`和workspace
locked/offline build均通过；workspace测试共223项、0失败。所有新增测试使用fake Server/runtime/
GPU/filesystem，没有访问真实Server、Docker socket或GPU。

用户随后授权M7-F.3凭据、只读preflight和错误HEAD无写注入，但未授权M7-F.4。工具提交
`bced313496ab1fe93525b7ff1e2a1e25d8618195`增加凭据准备、preflight、formal executor、watchdog和
独立post-audit。90的五脚本`bash -n`通过；独立错误HEAD注入以`source_head_mismatch`、
`window_entered=0`停止，外部状态前后不变。

API Key ID 26、名称`rust-worker-m7f-operator-20261106`、scope`management,inference`、生命周期
604,800秒已创建；完整token只保存在90的0600单硬链接文件，active Context为`m7f-operator`。
credential证据清单文件SHA-256为`5a35aa8744e86125ddbf313b9a3500a8f71b8100e3beeba663bbd53942438e6a`。

只读preflight attempt 1在`before_snapshot`安全STOP：Server为200，但固定Python Worker和metrics
端点均不可达。只读诊断证明固定容器仍声明`runtime=nvidia`，而2026-11-05重新启动的Docker daemon
只注册`runc`；`/etc/docker/daemon.json`缺少`runtimes.nvidia`，尽管NVIDIA Container Toolkit
1.20.0二进制完整。M7-F.3当前为`baseline_repair_required`。修复Docker runtime、reload daemon和
启动固定Python Worker属于新的系统写操作，未包含在本次授权中；M7-F.4仍未授权。

2026-11-09用户尝试执行root修复脚本时，`wujiaxing2`在sudo权限检查阶段被拒绝，脚本未启动且
没有创建证据、备份或修改配置。随后只读复核发现Docker已由本项目之外的操作重新注册
`nvidia-container-runtime`；该变化来源不归因于本项目。提交
`525de611fd3bfecf4cc42cad45508931dd64b039`增加非root、精确容器ID的恢复脚本，只启动既有Python
Worker并在失败时停止本次启动对象。恢复成功，同一容器、Server和三个健康端点均通过，证据清单
文件SHA-256为`31049979d2c849f505ebae558b430935fa6b0d55ce402b6b26aa8e0394126958`。

治理HEAD`525de611fd3bfecf4cc42cad45508931dd64b039`上的preflight attempt 2九组门全部通过，13个
定向失败边界测试通过；错误HEAD注入为`STOP/source_head_mismatch/window_entered=0/cleanup_ok=1`。
preflight和injection清单文件SHA-256分别为
`130d57bb216cb1f1ee5a8f0395ad46b61adf987b4fa61f7605284a2c100b110f`和
`a8813c82ed7e542fe222b045e2f0473bc29f756bc48c7c4d49ad1054432e50a3`。现场三端点200、无Rust
进程/容器、双GPU为空。记录本里程碑会产生新治理HEAD，因此下一步只在新HEAD运行全新只读
preflight attempt 3；通过后才是`ready_for_maintenance_authorization`。M7-F.4仍未授权。
