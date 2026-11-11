# Rust Worker HTTP API 使用指南

## 文档入口

Rust Worker在配置的Worker API listener上提供两种同源文档：

- `GET /openapi.json`：OpenAPI 3.1机器契约，可导入Postman或用于生成客户端；
- `GET /docs`：内置Swagger UI，适合浏览接口和交互调试。

90验证机的候选端口为`20150`，因此示例地址是
`http://10.213.80.90:20150/docs`。实际部署应以Worker配置的IP和端口为准。

Swagger UI资源已随Worker二进制vendored，页面加载不依赖外部CDN。OpenAPI和Swagger UI只公开
接口形状，不包含Worker token、Cluster registration token、密码或Authorization示例值。

## 认证

公开probe与文档入口不需要认证。其余业务接口接受以下二选一传输方式：

```text
Authorization: Bearer <token>
X-API-Key: <token>
```

如果两个header同时存在，`Authorization: Bearer`具有优先级，不能用合法`X-API-Key`绕过错误的
Bearer值。

- 文件、日志、配置和`/token-auth`只接受Server为当前Worker颁发的per-Worker token；
- Managed chat/completion代理接受per-Worker token或固定v2.2.3链路使用的Cluster registration
  token；
- token只能由受保护文件或受控运行环境提供，不能写入命令历史、日志、OpenAPI artifact或测试
  fixture。

## API分组

OpenAPI按以下标签组织当前R1接口：

- `Probes`：进程存活、完整Worker readiness和构建/Server基线身份；
- `Resources`：受允许模型根目录约束的文件存在性、模型配置和权重大小检查；
- `Logs`：ModelInstance当前/上一代日志发现、tail和follow；
- `Configuration`：只读非敏感配置；`PUT /v2/config`明确返回405；
- `Inference`：只代理`/v1/chat/completions`和`/v1/completions`到内部健康路由表中的本机vLLM。

推理请求必须携带`X-GPUStack-Model-Instance`，格式为
`model-<model-id>-<instance-id>[-<alias>].<registry-type>`。Worker从该值解析实例ID并查询内部健康
路由，客户端不能指定任意upstream。`stream=true`时成功响应为`text/event-stream`，否则通常为
`application/json`。

## metrics

Prometheus `GET /metrics`运行在独立metrics listener上，90验证端口为`20151`。它不与Worker API
共用base URL，因此不伪装成`20150` OpenAPI中的路径。metrics endpoint无需Worker token，但应由
部署网络和监控访问控制保护。

## 状态与调用场景

Server是Model、ModelInstance、ModelFile、调度位置和期望生命周期的权威来源。Worker HTTP API
不是用户绕过Server直接部署模型的控制面。正式模型部署和用户推理继续调用Server；Server及网关
在需要节点文件、日志或Managed数据面时调用Worker接口。

`/healthz`只证明Worker API进程存活。`/readyz`只有在资源、Docker、listener、身份、首次权威
full sync和首次runtime协调等生产门全部满足时才返回200；单个模型失败不等同于整个Worker未就绪。

## 静态规范与变更规则

仓库中的`rust/docs/openapi/worker-managed.openapi.json`由同一Rust定义生成，用于离线审阅、客户端
生成和版本diff。接口变更必须同时满足：

1. 实际Axum路由和OpenAPI操作由同一注册路径生成；
2. 静态JSON与运行时生成结果逐值一致；
3. security scheme、header、请求/响应Content-Type及错误状态有回归测试；
4. 不把状态机、恢复语义或独立metrics端口错误地简化为字段schema。

OpenAPI回答“如何调用”；状态所有权、恢复行为和未实现范围仍以架构、协议和本指南为准。
