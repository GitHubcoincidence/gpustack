# GPUStack Worker v2.2.3 契约 fixture

本目录固定 M4-B 使用的 Worker HTTP API、认证、错误和 metrics 样本。兼容目标为
GPUStack `v2.2.3`，源码 commit
`1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`；精确语义见
`rust/docs/worker-contract-v2.2.3.md`。

## 证据分类

- `live-healthz.response.json`、`live-readyz-python.response.json`、
  `live-version.response.json` 和 `live-unauthenticated.response.json` 是 2026-10-21
  从 90 主机现有 Python Worker 只读捕获。捕获未携带或记录 token。
- `endpoint-contract.json` 固定 M4 route、认证和响应语义。
- 其余 JSON 与 Prometheus 文件是依据固定源码、现场资源 identity 和已接受安全增强
  构造的 synthetic 预期，不冒充 Rust 实现或真实 M4 运行结果。
- `model-weight-size.response.json` 中的大小是现场 Qwen-7B 目录只读统计结果；模型
  内容没有复制到 fixture。

## 文件与用途

| 文件 | 用途 |
|---|---|
| `endpoint-contract.json` | route、auth、content type、query 和状态码总表 |
| `live-healthz.response.json` | Python Worker liveness 捕获 |
| `live-readyz-python.response.json` | Python Worker 固定 200 readiness 捕获 |
| `live-version.response.json` | Python Worker 版本捕获 |
| `live-unauthenticated.response.json` | 受保护 endpoint 的通用 401 捕获 |
| `resource-readyz.response.json` | Rust resource-api-only 的有意 503 增强 |
| `error-responses.json` | M4 稳定错误分类和通用消息 |
| `file-exists.response.json` | 安全模型根目录内存在性响应 |
| `model-config.response.json` | 白名单 JSON 文件响应形状 |
| `model-weight-size.response.json` | 权重大小响应形状和现场样本值 |
| `serve-log-options.response.json` | restart 顺序、previous 和 container 语义 |
| `config.response.json` | 非敏感只读配置白名单 |
| `metrics.response.prom` | 双 A100 资源指标与 Rust 控制指标样本 |

fixture 不包含 token、Authorization header、cookie、用户密码或任意凭据。M4-B 测试
只能用保留测试值和临时目录，不能访问现场网络、Docker、数据库或 GPU。
