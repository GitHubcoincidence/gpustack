# M8-EMB.6 106 Worker连接90 Python Server兼容验证方案

## 1. 验证目标

在106主机运行当前分支构建的统一Rust `worker run`，连接90上既有Python GPUStack Server，
通过Server托管链部署`BAAI/bge-small-zh-v1.5`并验证`POST /v1/embeddings`。实验使用106唯一的
GPU0；结束后删除实验资源并恢复106原有Qwen模型实例，但Worker继续运行最新二进制。

本门证明Python Server兼容、embedding调度、vLLM pooling、Worker代理和CLI推理的完整链路。
它不覆盖rerank、多模态、稀疏向量、跨Worker、其他推理后端或容器化Worker。

## 2. 固定现场与恢复目标

preflight必须重新读取并封存现场，以下值只作为当前预期，不代替正式证据：

- 106主机：`lenovo-ThinkStation-P5`，IP `10.213.80.106`，接口`eno8403`；
- 106 Rust Server：`http://10.213.80.106:18091/`；
- 原Worker：ID 17，名称`jiaxingHaoyuNB-106`，端口20150/20151；
- 原模型：ID 21，名称`rust-rs6-106-qwen25-05b-gpu0`；
- 原实例：ID 87，容器名`gpustack-rs-managed-87`，推理端口40000；
- GPU0 UUID：`GPU-be80b448-dd25-a523-676f-a279bf5c9c9c`；
- 原Worker data、token和identity文件必须原地复用；
- 原Model、ModelInstance、manifest和容器只停止，不删除或重建。

恢复成功必须同时满足：原Worker ID/UUID不变，原Model/Instance不变，原容器ID、Runner镜像、
spec hash和GPU UUID不变，40000健康，Worker ready，Model重新ready。Worker进程可以变化，且必须
报告当前治理HEAD；不保存、恢复或重新启动旧二进制。

## 3. Python Server隔离

Python Server固定为`http://10.213.80.90/`。实验Worker使用唯一身份：

- 名称：`rust-m8emb6-106`；
- API端口：20250；metrics端口：20251；
- 独立registration token、data和log目录；
- 独立operator Context；
- 实验Model：`m8emb6-bge-small-zh-106`；
- 固定模型目录：
  `/home/wujiaxing/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d`。

90当前两个活动Model均精确绑定`rust-m3d-r760xa`及其GPU ID，不会调度到实验Worker。preflight仍须
重查：任何副本数大于0但没有精确绑定其他Worker的Model都使门禁STOP。实验Worker注册后立即进入
maintenance；只有确认身份、端口和readiness后才解除maintenance并创建精确绑定
`rust-m8emb6-106:cuda:0`的实验Model。

## 4. 固定模型和运行参数

- 模型：`BAAI/bge-small-zh-v1.5`；
- revision：`7999e1d3359715c523056ef9478215996d62a620`；
- 文件：11个，共96,378,336字节；
- 输出维度：512；最大长度：512；pooling：CLS；
- Runner：
  `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af`；
- 部署：`category=embedding`、`runner=pooling`、GPU0、显存比例0.10、dtype auto、eager。

模型目录是只读固定输入，后续阶段可以复用，不属于实验残留。实验Worker身份、Server资源、容器、
manifest、端口、data/log和短期凭据必须按阶段退役。

## 5. 构建与切换语义

106仓库直接fast-forward到冻结HEAD，并在既有`rust/target`执行locked/offline workspace build。
构建会替换路径中的调试二进制，但不会重启当前Worker；Linux中已运行的旧进程仍持有原inode，直到
维护窗口精确终止。正式窗口不创建旧二进制备份：实验和恢复均使用最新`gpustack-rs`。

恢复原Worker时保留原参数，只删除当前CLI已废弃的`--model-root`。复用原data目录、registration
token和identity文件，使它接管原Worker ID/UUID、manifest及容器，而不是注册第二个106 Worker。

## 6. 分段门禁

### EMB.6B-106 准备与只读preflight

允许同步源码、直接构建最新二进制、安装只读模型、建立受保护Context/token文件、执行只读API、
Docker inspect、GPU和进程检查。不得停止Worker或容器，不得修改maintenance、Model或Instance。
通过后状态只能是`ready_for_maintenance_authorization`。

### EMB.6C-106 正式维护窗口

须再次取得明确授权。一次性0600授权JSON必须固定治理HEAD、preflight manifest SHA-256和随机nonce。
执行顺序固定为：

1. 将原106 Worker置maintenance；
2. 精确终止封存进程，只停止原实例容器；
3. 启动连接90的最新实验Worker，注册后立即置maintenance；
4. 核对readiness后解除maintenance并部署唯一BGE模型；
5. 完成Server CLI和Worker直连的float/base64及重复请求门；
6. 删除唯一实验Model/Instance/容器/Worker与运行目录；
7. 启动原容器，以最新二进制和原identity启动106 Worker；
8. 恢复窗口前maintenance值并验证原模型ready。

watchdog必须在executor异常、超时或控制终端断开时调用同一精确cleanup。任何业务门失败都保持
formal失败；恢复成功不能把业务失败改判为通过。

### EMB.6D-106 审阅与凭据退役

formal、cleanup和独立post-audit均PASS后先进入`ready_for_review`。用户接受后删除106上的隔离
Context、operator与registration token副本和一次性授权文件，再把阶段标记为`accepted`。共享
Server凭据只有在本阶段独立创建时才撤销；不得撤销被既有环境复用的原始凭据。

## 7. 正式验收门

1. 实验Worker在90 Python Server唯一注册，maintenance屏障生效；
2. 实验Model categories精确为`["embedding"]`并只绑定实验Worker GPU0；
3. vLLM命令含`--runner=pooling`，DeviceRequest仅含106 GPU0 UUID；
4. ModelInstance到达running，Server CLI和Worker直连`/v1/embeddings`均返回200；
5. 双输入float各512维；base64解码为2,048字节；table报告dimension 512；
6. 三次重复推理不改变容器ID、spec hash和restart count；
7. 实验资源、端口、进程、manifest及data/log归零；
8. 原Model 21、Instance 87、容器、镜像、spec hash、GPU和推理端口完整恢复；
9. 原Worker ID/UUID不变并由最新治理HEAD二进制运行；
10. 90 Python Server、106 Rust Server、恢复Worker和模型健康；
11. 证据manifest完整、秘密扫描为零，仓库HEAD和工作树无漂移。

## 8. 当前停止点

M8-EMB.6C-106已在工具HEAD`668bf92b`通过正式验证、cleanup和独立post-audit。固定BGE模型继续
保留；实验Worker、Model、Instance、ModelFile、容器、manifest、端口和data/log已经归零，原
Worker 17、Model 21、Instance 87和同一容器已由最新二进制恢复。

用户接受详细报告后，106上的实验专用Python operator副本、Cluster registration token副本、
隔离Context和一次性授权输入均已精确删除。该阶段没有新建90 Server API Key，因此未撤销或删除
90上的共享operator与Cluster原始凭据；它们继续供既有环境使用。退役后106 Worker、原Qwen模型、
106 Rust Server以及90 Python Server/Worker健康检查均为200，固定BGE模型和完整证据链继续保留。
M8-EMB.6现为`accepted`；不得据此自动启动EMB.7或M8-B，下一步只允许先讨论后续范围。
