# Rust GPUStack CLI-first 设计

## 契约

一个版本支持的每项面向用户或运维人员的能力，都必须有稳定的非交互式 CLI 命令。对于 Managed 操作，CLI 调用类型化 Server API 或推理 API，不在客户端重复实现 Server 业务逻辑。

开发阶段二进制为 `gpustack-rs`，最终公开二进制为 `gpustack`。临时名称用于避免与验证主机上的 Python 命令冲突。

## R1 命令树

```text
gpustack-rs
  context add|use|list|remove
  worker run
  worker doctor
  worker validate-config
  worker gpu list
  worker runtime info
  worker config get
  worker lab deploy|list|status|logs|infer|stop|remove|cleanup
  model deploy|list|get|status|logs|delete
  inference models|chat|completion
  version
```

最终命令树在 `gpustack` 下保留这些子命令，并随着后续 Server 模块的重写继续扩展。

## Managed 模型命令

`model deploy` 使用运维人员身份认证，并通过 Server API 创建模型。它不直接访问 Docker 或 Worker。R1 强制显式指定目标 Worker，避免测试任务被调度到 Python Worker。

R1 部署输入包括：

- 模型名称；
- 目标 Worker 名称；
- 本地模型目录；
- 显式 Runner 镜像或配置的默认镜像；
- 一张或两张本地 GPU 的 index；
- 使用两张 GPU 时的 tensor parallel size；
- 已支持的结构化 vLLM 选项；
- 使用 `--wait` 跟踪 ModelInstance，直至进入 `RUNNING` 或 `ERROR`。

CLI 把友好的 GPU 参数转换为现有 Server selector 形式，例如 `worker-name:cuda:0`。端口分配仍由 Worker/Server 负责。

`model logs` 调用 Server 日志 API；Server 在向 Worker 请求日志之前继续执行现有用户授权与 tenant 可见性检查。

## 推理命令

Managed `inference` 命令调用 Server 或 gateway 的 OpenAI-compatible endpoint。Lab 推理调用隔离的本地 vLLM endpoint，并在输出中明确标记为 Lab。两种形式绝不静默切换 endpoint。

流式命令保留增量输出，并在处理中断时确保认证 header 不会泄露。

## Lab 命令

Lab 模式用于在完成 Server 集成前演示真实 Worker runtime。它复用生产路径中的 workload planner、runtime、health check、logging 和 reconciler，但不进行 Server 注册，不使用 Worker credential，不创建 Server 数据库记录或 gateway route。

出现以下任一情况时，Lab deploy 拒绝执行：Rust Managed 数据目录锁处于活动状态、目标 GPU 已被占用、目标端口已占用或存在同名 Lab 工作负载。正式测试还要求先 drain 并停止 Python Worker；Rust 自身的锁无法证明它独占物理 GPU。

Lab cleanup 只处理标签匹配的资源，默认使用 `--dry-run`。R1 不提供强制抢占 GPU 或删除未知容器的参数。

## 认证

- `worker run` 使用注册凭据以及注册后获得的 Worker 系统凭据。
- `model` 和 Managed `inference` 命令使用运维人员或用户 API 凭据。
- Lab 命令不使用上述任何凭据。
- 凭据来自权限受限的 token 文件或获准的环境变量来源。普通命令行参数不得包含 token 值。
- Context 配置可以记录 Server URL 与 token 文件路径，但绝不记录 token 本身。

## 输出与自动化

所有适用命令都支持稳定的机器可读输出与可预测的进程行为：

```text
--output table|json
--quiet
--no-color
--timeout
--context
```

结果数据写入 stdout，诊断和进度写入 stderr。成功退出码为 0；配置、认证、兼容性、调度、运行时、健康与超时错误使用可区分的非零退出码。JSON 字段名作为带版本的兼容界面管理。

破坏性用户命令在交互式终端中要求确认，并为自动化提供显式 `--yes`。Dry-run 输出包含规范化请求或按所有权限定的清理计划，且不得包含敏感信息。

## CLI 对等门禁

如果某项已支持的用户或运维流程必须依赖 Web UI 或未文档化的原始请求，该功能就不能通过对应版本的验收。可以为诊断保留原始 API escape hatch，但它不能满足 CLI 对等要求。
