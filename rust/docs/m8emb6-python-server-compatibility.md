# M8-EMB.6 Python Server embedding兼容验证方案

## 1. 目标与结论边界

本阶段在90服务器验证统一Rust `worker run`能被既有Python GPUStack Server管理，并通过同一
Server托管链完成文本embedding部署、Worker代理和CLI推理。结论只适用于vLLM、单机单Worker、
GPU0和固定BGE模型；不覆盖rerank、多模态、稀疏向量、token ID输入、跨Worker或其他后端。

M8-EMB.5的离线通过不能替代本阶段真实证据。本方案接受、模型准备、凭据/preflight和正式维护窗口
分别授权；任何前置阶段通过都不自动授权停止106 Worker、停止现有容器或使用GPU0。

## 2. 现场差异与隔离决定

方案冻结时，90上的普通Rust Worker连接106 Rust Server，并用GPU0运行Instance 95；GPU1由外部
任务占用。正式窗口采用以下隔离策略：

1. 不删除106的Model、ModelInstance或现有容器；
2. 保存原Worker进程identity、二进制和NUL分隔argv；
3. 启用原Worker maintenance后精确停止该进程，再停止但不删除原容器；
4. 用独立Worker名称、端口、token、data和log连接本机Python Server；
5. cleanup先清除实验资源，再启动原容器和原Worker，并恢复窗口前maintenance值；
6. GPU1只记录，不作为进入门，不停止其进程，也不进入任何DeviceRequest。

preflight发现原Worker、容器、Model、Instance、GPU0进程或所有权元组变化时必须STOP。正式窗口中
只能操作preflight封存的精确106身份和本阶段新建的精确实验身份。

## 3. 固定输入

- 产品提交：`92184388070f961e8bbc1fbed4a3ec3c84691829`；
- 模型仓库：`BAAI/bge-small-zh-v1.5`；
- revision：`7999e1d3359715c523056ef9478215996d62a620`；
- 模型文件：11个，96,378,336字节，逐文件SHA-256见
  `rust/fixtures/m8emb/bge-small-zh-v1.5-7999e1d.sha256`；
- 输出维度：512；最大模型长度：512；pooling：CLS；
- Runner：
  `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af`；
- 部署参数：`category=embedding`、`runner=pooling`、GPU0、显存比例0.10、dtype auto、eager；
- 实验Worker：`rust-m8emb6-r760xa`，API端口20250，metrics端口20251；
- 实验Model：`m8emb6-bge-small-zh`；
- 固定模型目录：
  `/home/wujiaxing2/.local/share/gpustack-rust-worker/models/bge-small-zh-v1.5-7999e1d`。

模型目录是后续EMB.7和容器化验证复用的只读固定输入，不属于运行残留。Server资源、Worker身份、
容器、route、manifest、端口、进程、阶段data/log和短期凭据必须清理。

## 4. 工具链

`rust/scripts/m8emb6/`包含：

- `common.sh`：固定输入、token文件检查、进程/容器身份、模型和证据manifest；
- `prepare-model.sh`：从已下载stage目录原子安装只读固定模型；
- `preflight.sh`：只读固定源码、模型、服务、106 Worker/Model/Instance/容器和双GPU观察基线；
- `executor.sh`：唯一正式写窗口，要求一次性授权JSON；
- `cleanup.sh`：精确删除实验资源并恢复106基线；
- `watchdog.sh`：30分钟deadline和180秒强制终止后恢复；
- `post-audit.sh`：独立验证证据链、实验零残留和106基线恢复；
- `self-test.sh`：bash语法、禁止宽泛删除/GPU1、身份正反门和无授权拒绝测试。

脚本从受保护token文件读取secret，通过curl配置FD传递，不把token写入argv、日志或证据。证据目录
固定0700、普通文件0600；唯一用于恢复的Worker二进制副本为0700，并由manifest固定内容。证据同时
生成`manifest.sha256`和秘密扫描结果。

## 5. 分段门禁

### EMB.6A 模型与工具准备

只允许提交工具和固定model manifest、把模型安装到只读固定路径、运行shell静态检查和离线
self-test。不创建Context/API Key/Worker/Model，不停止服务、调用Docker写操作或使用GPU。

### EMB.6B 凭据与只读preflight

须另行授权。创建两个短期operator Context和一个Python Server Cluster registration token，均保存
为0600单硬链接文件。preflight固定最终治理HEAD、二进制、模型、Runner镜像、106恢复身份和实验
保留路径；只读通过后最多进入`ready_for_maintenance_authorization`。

### EMB.6C 正式维护窗口

须再次明确授权，并创建0600一次性JSON：

```json
{
  "stage": "M8-EMB.6-maintenance-window",
  "approved": true,
  "expected_head": "<preflight固定HEAD>",
  "preflight_manifest_sha256": "<preflight manifest文件SHA-256>",
  "nonce": "<至少16字符的一次性随机值>"
}
```

watchdog只接受与当前HEAD和preflight manifest完全一致的文件。授权文件不能提交Git，窗口结束后
删除。

### EMB.6D 审阅与凭据退役

formal、cleanup、watchdog和post-audit均PASS后只进入`ready_for_review`。用户接受后删除两个短期
API Key/Context、registration token和授权文件，证明旧token返回401，再标记`accepted`。

## 6. 正式验收门

必须同时满足：

1. 唯一状态链为
   `pending -> analyzing -> scheduled -> initializing -> downloading -> starting -> running`；
2. Model categories精确为`["embedding"]`；
3. vLLM命令显式包含`--runner=pooling`，容器只请求GPU0 UUID；
4. Worker直连和Python Server/CLI两条`/v1/embeddings`路径均返回200；
5. 双输入float响应有2项，每项512个数值；
6. base64向量解码后为2,048字节，table输出报告dimension 512；
7. 三次重复请求不改变Model/Instance、容器ID、spec hash或restart count；
8. 实验Model、Instance、ModelFile、Worker、容器、route、manifest、端口、进程和阶段目录归零；
9. 106原Worker ID/UUID、Model/Instance、容器ID、镜像、spec hash和maintenance恢复；
10. Python Server/Worker/metrics及106 Server健康，GPU1没有被本阶段请求或操作；
11. formal、cleanup、watchdog和独立post-audit证据manifest及秘密扫描全部通过。

任一业务门失败都使formal失败。cleanup或恢复成功只能证明环境恢复，不能把业务失败改判为通过。

## 7. 当前停止点

用户已接受本方案并只授权EMB.6A。模型和工具完成离线准备后必须交付详细报告；EMB.6B凭据和只读
preflight、EMB.6C真实维护窗口以及任何Server、Docker或GPU写操作仍未授权。
