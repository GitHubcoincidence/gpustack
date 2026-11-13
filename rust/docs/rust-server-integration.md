# Rust Worker 与 Rust Server 联调协议

## 目标与权威

M8-RS在真实Worker容器构建前，用宿主机Rust Worker验证Rust Server控制面。兼容性权威依次是：

1. 原GPUStack v2.2.3源码commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`；
2. 90服务器同版本Python Server的真实行为；
3. `rust/fixtures/server-v2.2.3/`中已接受的wire fixture；
4. 原Python Worker的请求行为；
5. Rust Server真实HTTP行为；
6. Rust Server OpenAPI文档。

OpenAPI差异只表示文档不满足基线，不能单独判定运行实现错误。运行问题必须由脱敏请求/响应和
原始oracle共同证明。Worker偏离时修改Worker，Server偏离时修改Server，文档单独偏离时只修改
文档；不能为了互通而放宽身份恢复、状态所有权、资源归属、完整PUT或fail-closed规则。

Worker对Server执行的兼容性硬门只校验`version=v2.2.3`；Worker注册请求的`worker_version`同样固定为
`v2.2.3`。`implementation`、`git_commit`和`build_version`均为可选诊断字段：存在时可以写入证据，
缺失或取值不同都不得阻止首次注册、持久身份恢复或缺失Worker token后的恢复式注册。该规则同时适用
原Python Server和目标Rust Server，不能用`implementation=rust`排除兼容的原版Server。正式实验仍可
因运行期间外部Server发生变化而按证据不可归因规则停止，但这不是Worker产品兼容性校验。原GPUStack
commit `1cfcbbff02aacdb51f0a390378dd7bcddd9bf365`始终保留为协议和行为权威。

## 阶段

| 阶段 | 范围 | 写入边界 | 通过条件 |
|---|---|---|---|
| RS.0 | 冻结协议、证据和权限 | 只修改治理文档 | 用户接受方案 |
| RS.1 | 本地OpenAPI审计、synthetic fixture和测试 | 不联网、不读取凭据 | 审计器正反用例、完整Rust质量门 |
| RS.2 | health/version/auth/error/分页/watch只读探测 | 不创建资源 | 只读wire与oracle一致 |
| RS.3 | 空Cluster中的注册、me、heartbeat/status和身份恢复 | 只创建唯一测试Worker | 无重复Worker、204语义和恢复通过 |
| RS.4 | 宿主机统一Worker空载运行 | 状态和heartbeat；无Model/Runner | full sync/watch/readiness稳定 |
| RS.5 | 隔离资源、过滤、watch和完整PUT | 唯一测试Model/Instance/File | 状态所有权和字段保留通过 |
| RS.6 | 固定Qwen2.5-0.5B的GPU0端到端 | 独立维护窗口；GPU1不参与 | 状态链、推理、日志、幂等和清理通过 |
| RS.7 | 差异闭环、清理和凭据退役 | 精确删除测试身份和资源 | P0/P1清零、旧token 401、现场恢复 |

RS.2及以后分别需要明确授权。RS.6还必须取得停止并恢复精确Python Worker的维护窗口授权。

## 真实联调执行粒度

RS.2及以后采用“单一接口或单一功能门”推进：每次只冻结一个目标、请求、期望、证据和停止条件；
执行后先向用户解释实际wire、与原始oracle的差异和下一步建议，再由用户决定是否进入下一门。
不得因为某一接口通过而批量调用同阶段其他接口，也不得把一次授权扩展为资源创建、更新或删除。

自2026-11-12起，用户决定临时收敛逐接口验证深度：优先验证Rust Worker真实工作流发出的有效请求
能否返回其需要的正确状态、身份和资源值；不再为非法参数、输入校验失败、边缘错误状态或默认排序
单独增加live probe。已经观察到的差异必须保留，不能宣称兼容，但可作为不阻塞当前Worker主路径的
待办留到RS.7复核。认证、注册、`me`、heartbeat/status、list/full sync、watch事件、完整PUT、状态
所有权和推理响应属于Worker真实路径，仍按原v2.2.3严格核对。

RS.2按以下顺序逐门讨论和执行：

1. `GET /healthz`：只验证基础传输和健康响应；
2. `GET /version`：只验证目标Server身份；
3. 一个受保护list接口的无凭据401：只验证认证边界和错误外形；
4. 同一接口的短期凭据只读请求：只验证认证主体与最小分页响应；
5. 有效分页：只验证Worker真实使用或后续控制面需要的query组合；
6. watch：Model、ModelInstance和ModelFile分别执行，且每次只观察不创建事件。

RS.2不注册Worker、不创建测试资源、不修改maintenance。任何需要写入才能验证的语义留到RS.3
或RS.5，并重新取得授权。

## 离线OpenAPI审计器

审计器只接受本地普通JSON文件，不接受URL、header或token：

```bash
cd rust
cargo run --locked --offline \
  -p gpustack-client \
  --example m8rs_openapi_audit \
  -- /absolute/path/to/openapi.json
```

退出码：

- `0`：全部文档契约通过；
- `1`：输入、读取、解析或报告生成失败；
- `2`：文档可解析，但存在契约差异。

输出是稳定JSON矩阵，只包含规范版本、标题、版本和固定检查项的expected/actual摘要。工具不会
复制OpenAPI examples、request bodies、Authorization或token值。每条差异的
`runtime_status`固定为`unverified`，直到后续live wire阶段提供证据。

RS.1固定检查：

- Worker注册请求的必填资源字段；
- 注册响应中的ID、name、Worker token和Server UUID；
- `workers?me=true`；
- heartbeat/status请求体和204响应；
- Model、ModelInstance和ModelFile分页/filter；
- ModelInstance和ModelFile完整PUT字段；
- watch事件编号；
- `/v1/models`、`/v1/chat/completions`和`/v1/completions`；
- 标准错误和分页外形。

`rust/fixtures/m8rs/compatible-openapi.json`是正向synthetic契约；
`rust/fixtures/m8rs/incompatible-openapi.json`只复现已观察的差异类别，不是106原始OpenAPI副本，
不含现场资源示例。

## 初始只读观察

2026-11-11无凭据读取`http://10.213.80.106:18091`：

- `/healthz`：200，`"ok"`；
- `/version`：v2.2.3、commit `1cfcbbf`、implementation `rust`、build version `0.1.0`；
- 未认证`/v2/workers?page=-1`：标准401错误；
- OpenAPI 3.0.3：634,702字节，SHA-256
  `ea8c08f107d0a3ce7493711648d9e1c9ee460bc626347ecfb10e7add836e9635`。

文档初步差异包括注册响应、heartbeat/status、`me`与`worker_id`过滤、完整update schema、watch
编号、OpenAI路由和servers端口。它们尚未被判定为Rust Server运行缺陷。

## 差异记录

每个真实差异必须记录：编号、严重度、method/path、原始预期、Rust Worker实际请求、Rust Server
实际响应、脱敏复现、问题归属、修复提交、回归结果和阻塞状态。诊断、失败和无效attempt不能被
提升为正式通过。
