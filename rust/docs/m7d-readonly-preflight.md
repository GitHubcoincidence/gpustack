# M7-D 90 只读预检

## 目的和判定边界

M7-D 在 90 服务器上冻结 M7-E/F 将使用的真实输入、恢复材料和故障注入边界。它只执行
认证 GET、锁定离线构建、本地镜像/文件/设备读取以及受保护证据写入，不停止服务、不切换
maintenance、不创建 Server 资源或 Docker workload，也不启动代理或占用 GPU。

通过 M7-D 只表示可以请求 M7-E 的精确维护窗口授权，不证明任何真实 crash recovery、
restart-on-error、连接恢复或双 GPU 推理已经通过。

## 冻结输入

| 项目 | M7-D 固定值 |
|---|---|
| 治理 HEAD | `d6bba026d818370cae3ed0b46e6958ec3af693b0` |
| M7-C 产品祖先 | `0d861a4c9d5502341cbc78eefc60e30bb32c24fd` |
| `Cargo.lock` SHA-256 | `90f1aeaa22b4f06dc77e48497f2fe3394ab07ab55441c8ddb3124a67dca07010` |
| Rust binary SHA-256 | `6d147b8bb723833aa0b1432a16218f5d40cdc4479ee694a37967a0861d680a75` |
| Server | `v2.2.3` / `1cfcbbf` |
| Worker | ID 5，`rust-m3d-r760xa`，UUID `7c4e07a3-6b97-4577-9ff0-1fc6c2319596` |
| Worker 地址 | `10.213.80.90:20150`，metrics `20151` |
| Python Worker | 容器 `68cd9a516eb2d002a97e5d645f84162125774cc86cc5fa00b6dcae47b87ae165` |
| Runner | `quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af` |
| 模型 | `Qwen/Qwen2.5-0.5B-Instruct` revision `7ae557604adf67be50417f59c2c2f167def9a775` |
| 模型路径 | `/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct` |
| 模型清单 | 10 个文件，999,604,126 字节，manifest SHA-256 `645a03c556c40adb3d2d8e48e76c3aedd8875bcf84ee78b0778b9cdd24a0d4f6` |
| GPU0 | `GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3` |
| GPU1 | `GPU-b1f89e65-e9c4-26f5-a306-8a520df07708` |
| Managed 端口 | `40000-40063` |

## 正式门禁

1. 源码分支、HEAD、origin、产品祖先、工具链和 `Cargo.lock` 精确匹配，工作树干净。
2. 清理 `gpustack-cli` 缓存后，以 Rust 1.99.1 完成 workspace/all-targets/all-features
   `--locked --offline` 构建；binary 自报治理 HEAD 的 12 位 commit。
3. Server、Python Worker health 和 metrics 均为 HTTP 200；Server 版本与固定部署一致。
4. Cluster token 的只读查询和 per-Worker token 的 `me=true` 查询都只返回 Worker ID 5；
   本地 identity 的 ID、名称和 UUID 精确匹配，凭据文件只记录元数据和权限，不记录内容。
5. Worker 处于 maintenance 且可达，没有关联 ModelInstance 和 M7 模型资源；没有 Rust
   Worker进程、Rust ownership 容器或目标端口监听。
6. Runner digest 在本地；固定模型规范路径、文件数、总大小、权限、架构、chat template 和
   全文件 SHA-256 manifest 两次通过。
7. 两张 A100 的 index/UUID/80 GiB identity 精确匹配，显存使用和利用率均为 0，没有 compute
   process。
8. Python Worker 的容器 ID、镜像 ID、`unless-stopped`、host network 和运行状态被记录为
   恢复材料；正式窗口必须停止并恢复同一容器，以 health/metrics 200 作为恢复门。
9. Server 专用候选代理固定为 `127.0.0.1:18080 -> 127.0.0.1:80`；Docker 专用候选代理固定为
   `/home/wujiaxing2/.local/state/gpustack-rust-worker/m7/docker-proxy.sock -> /var/run/docker.sock`。
   两个代理只允许注入到 Rust Worker 配置，并用独立进程组启动/终止；真实 Server、Docker
   daemon 和 Python Worker 路径保持不变。M7-D 只验证 `socat`、`setsid`、端口、socket 和权限，
   不启动代理。
10. M7 operator token 预留路径不存在，Context 列表没有 active operator context。短期平台管理
    API Key 只有在具体真实窗口获授权后才创建，并在该阶段用户验收后退役和验证失效。
11. 运行前后服务、全部容器 ID、Server/Python 容器 identity、GPU、compute process、监听端口、
    Rust 进程、Worker/实例/模型查询共 13 组快照逐字节一致。

## 诊断尝试

- attempt 1 在构建后调用旧 CLI 参数 `version --format json`，按预期在 binary identity 门停止；
  没有执行外部写入。诊断证据清单 identity 为
  `059254c0b4d26b24026047a70b033ab37bac052c74960df318a16a18447fd427`。
- attempt 2 在 Runner inspect 中读取稀疏 Docker image `Config.User` 键时停止。该字段不属于
  M7-D 必需 identity，正式脚本改为只固定 ID、RepoDigest、OS 和架构。诊断证据清单 identity 为
  `dc6e0465918390b88703366f8b9aae109a6a51d05a98619125baa72bf72fb079`。
- 两次诊断目录均完成独立秘密扫描、`0600` 权限和 manifest 封存；Server、Python Worker、
  容器、端口和 GPU 未改变。它们不属于通过证据。

## attempt 3 正式结果

正式脚本为
`/home/wujiaxing2/.local/state/gpustack-rust-worker/tools/m7d-readonly-preflight-d6bba026-attempt-3.sh`，
SHA-256 为 `ac67b8d53f4d5596d9232086c3a6bcdd2592306399a17ccefc7ee02b09d89214`。
90 主机没有 ShellCheck；脚本通过 `bash -n`，全部尚未执行的 Docker/model/identity/proxy 命令在
attempt 3 前分别完成只读兼容性探针。

正式结果为 `PASS / ready_for_maintenance_authorization`。证据目录：

`/home/wujiaxing2/.local/state/gpustack-rust-worker/evidence/m7d-readonly-preflight-d6bba026-attempt-3`

证据清单文件 SHA-256 为
`6a2fcd779b51f20c0e31fc62c8c00c22d227912cadcfd57e521c7a8e323957c2`。独立复核确认：

- 49 个证据文件全部为 `0600`，原始 API response 文件为 0；
- manifest 逐项复算通过，Cluster token、per-Worker token 和 API-key-shaped 值均未出现；
- 固定模型 manifest 在正式过程前后都通过；
- Server、Python Worker 和 metrics 保持 200；Python Worker容器 ID 未改变且持续运行；
- Rust ownership 容器、Rust Worker进程、目标监听端口均为 0；
- 两张 GPU 在过程前后均为 0 MiB、0%，且没有 compute process；
- M7 state/proxy 路径和 operator token 预留路径仍不存在；
- 仓库 HEAD/origin 一致，工作树保持干净。

## 停止位置

M7-D 当前为 `ready_for_maintenance_authorization`。下一步只能在用户明确授权后准备并执行
M7-E.1“Rust Worker crash 后接管同一健康 vLLM”窗口。该授权不能自动覆盖 M7-E.2、M7-E.3
或 M7-F；每个窗口仍需独立 watchdog、精确 cleanup、Python Worker恢复、事后审计、短期凭据
退役和用户审阅。
