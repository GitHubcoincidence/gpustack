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

### RS.4.1 maintenance安全屏障

RS.4.1在统一Worker首次空载启动前，只对Cluster 3中唯一Worker ID 16执行一次完整
`PUT /v2/workers/16`。写入前重新确认身份唯一、21150/21151空闲、目标进程不存在、Cluster 3无
Model/ModelInstance且Worker 16无ModelFile；现有Python Worker和其他任务保持在线。请求从即时
Worker快照构造完整`WorkerUpdate`，原样保留name和全部labels，只把maintenance设为
`enabled=true`，message固定为`M8-RS.4 empty-run safety barrier`。

响应和随后GET必须确认ID、Cluster、name和labels未变，maintenance精确启用且隔离资源计数仍为零。
超时或响应不明确时先GET判定实际状态，禁止盲目重试PUT；发现并发身份或标签变化立即停止。成功后
保留maintenance作为RS.4.2启动安全屏障，不在本门自动恢复；失败时也优先保留已经生效的安全状态，
只有能证明本次写入所有权且不会覆盖并发更新时才允许恢复原maintenance。本门不启动Worker、不停止
Python Worker，也不调用Docker/NVML或使用GPU。方案冻结不构成真实写入授权。

### RS.4.2 统一Worker空载运行

RS.4.2只使用普通生产入口`worker run`恢复Worker 16，固定Server、名称、宿主机身份、21150/21151、
Cluster registration token、既有`m8rs3`数据目录、`m8rs4/logs`日志目录、本地模型根和Docker socket。
二进制必须从冻结后同步且干净的HEAD用Rust 1.99.1和locked/offline依赖重新构建，并核对内嵌commit、
SHA-256和Cargo.lock。启动前须再次确认maintenance精确保持、隔离资源为零、目标端口/进程不存在及
现有服务健康；GPU活动只观测，不要求排他。

运行窗口固定约130秒，覆盖至少四轮30秒heartbeat/status、初始和多轮30秒full sync、两个45秒
watch idle边界及多轮10秒资源刷新。Worker本地`/readyz=200`表示resources、Docker、Worker API、
metrics、identity、full sync和runtime七项门健康；Server仍须保持`state=maintenance`，两者不能混为
调度就绪。health、version、认证、metrics、双A100资源、heartbeat推进、三类watch和零desired资源
必须通过；不得出现Managed reconciliation或Docker容器集合变化。

空载生产runtime允许创建私有`lab.lock`、`managed.lock`、`manifests/`、`cache/`和`logs/serve/`，但
manifests、实例cache和模型日志必须为空。结束时只向精确PID发送SIGINT，15秒内优雅退出并输出零
desired/零reconciliation、control health全通过且watch reconnect为0的JSON总结；随后端口和进程必须
消失，maintenance继续保留。失败时先停止精确PID，不关闭maintenance、不删除身份或数据目录、不
操作容器/GPU，也不自动重试。方案冻结不授权构建、启动或任何运行时操作。

2026-11-15临时推进裁决：RS.4.2 attempt 1仍为`FAIL/unexpected_desired_models`，但其失败已明确
归因于Rust Server跨Cluster list/watch范围缺口，且Worker本地所有权防线、零reconciliation、零容器
变化和完整清理均有正式证据。用户决定在等待Server修复期间把RS.4更新为
`complete_with_open_server_difference`并继续RS.5，不能把该状态表述为完全兼容或正式通过。

临时推进期间增加以下硬约束：

- 只使用由管理主体精确创建在Cluster 3、并显式绑定Worker 16的唯一测试资源；
- 不得把Worker Token返回的其他Cluster Model或其他Worker ModelFile用于任何写请求；
- Worker 16继续保持maintenance，除非后续单独冻结并授权精确状态变化；
- 每一门前后核对目标ID、Cluster、worker_id、容器集合和reconciliation计数；
- Worker本地ModelInstance/ModelFile所有权检查不得放宽或删除；
- RS.7最终验收前，Rust Server必须修复Model Cluster范围和ModelFile `worker_id`过滤，并重新通过
  list、watch和RS.4.2空载回归。

### RS.5.1 唯一停止态Model创建

RS.5.1只使用106管理主体向`POST /v2/models`发送一次请求，创建名称
`rust-rs5-scope-90-20261115`、cluster_id 3、replicas 0的唯一停止态Model。source固定为
`local_path`，路径为90服务器已经验证存在的Qwen2.5-0.5B-Instruct；backend固定为vLLM，Runner
镜像使用M6/M7已接受的固定digest。worker selector只绑定`rust-rs3-worker-90-20261113`，
`gpu_selector=null`，不创建ModelInstance或ModelFile，不启动Worker，不触发调度、容器或GPU。

请求前重新确认106仍为v2.2.3、Worker 16精确属于Cluster 3并保持maintenance、目标名称不存在、
Cluster 3资源为空、21150/21151空闲、本地模型目录存在以及现有容器/GPU快照。请求HTTP 200或201后，
响应和独立GET必须确认非空ID、精确name、cluster_id 3、replicas 0和ready_replicas 0；随后确认该
Model没有ModelInstance且Worker 16没有ModelFile。Worker SYSTEM Token的Model list只用于确认精确
目标可见，已知的其他Cluster条目必须忽略且不得参与任何写请求。

连接或响应不明确时禁止重复POST，只按精确名称GET判断实际状态；若Server已经创建资源但响应字段或
后置门失败，保留该资源、记录ID并停止，不自动删除。成功资源同样保留给RS.5后续单门使用，最终由
RS.5/RS.7精确清理。方案不授权更新或删除Model，也不授权启动Worker或执行Docker/GPU生命周期。

### RS.5.2 Model完整PUT与UPDATED watch

RS.5.2先用Worker 16 SYSTEM Token建立`GET /v2/models?watch=true`，确认初始回放中Model 6精确
出现后，管理主体只对`PUT /v2/models/6`发送一次完整请求。请求由即时Model 6快照白名单化构造，
只把description改为`M8-RS.5 watch update verified; still stopped`；Cluster、replicas 0、source/path、
backend参数、Worker selector、gpu_selector null和分布式推理false必须保持不变。

原GPUStack v2.2.3对非Custom backend执行Model更新时会把`image_name`和`run_command`设置为null，
因此两字段按oracle单独审计，不混入普通字段保留门。Rust Server若采用不同结果，记录为兼容差异，
但本门硬门仍要求所有归属、停止态和调度字段不变。

通过门要求PUT为HTTP 200、独立GET字段正确，watch在同一连接中收到恰好一个`type=2`、ID 6、
description为新值的UPDATED事件。初始回放中其他Cluster Model继续按D284记为已知Server差异，不能
用于写请求。Model 6和Worker 16仍无Instance/File，目标Worker/端口、容器、GPU和独立20150 Worker
前后不变。结果不明确时只GET判定，禁止第二次PUT；Model 6继续保留，不在本门回滚description或删除。

2026-11-15裁决：RS.5.2保持`FAIL/update_ignored_description`，缺陷交由Rust Server修复并作为RS.7
硬门；等待期间继续RS.5，不在Worker添加迁就分支，也不把空UPDATED事件视为更新成功。

### RS.5.3 唯一pending ModelInstance创建

原GPUStack v2.2.3提供`POST /v2/model-instances`，而106当前OpenAPI只列出该集合路径的GET，未列
POST。RS.5.3仍按原版向运行时发送一次合法POST，以实际HTTP行为判定接口是否存在；若返回404/405，
记录为Rust Server缺失核心接口并停止，不尝试数据库写入、Model扩容或其他绕过路径。

请求创建唯一`rust-rs5-instance-90-20261115`，固定model_id 6、model name、Cluster 3、Worker 16
身份与10.213.80.90网络字段、local_path、vLLM和`state=pending`；port/pid为空，ports、GPU indexes/
addresses为空，distributed servers和draft/LoRA字段为空。pending状态不满足Rust Worker只接管
scheduled实例的前置条件，即使后续启动Worker也不能进入端口分配或runtime；本门仍禁止启动Worker。

通过门为HTTP 200/201、正ID、字段精确一致，管理主体和Worker 16 Token各自只看到一次目标实例；
Model 6保持replicas 0、Worker 16 maintenance，ModelFile为0，端口/进程/容器/GPU不变。结果不明确
时只按精确name GET，禁止第二次POST。成功实例保留给后续ModelInstance watch/完整PUT门，失败时不
删除Model 6或修改其他资源。

2026-11-15实际结果：唯一POST传输成功但返回HTTP 405、空响应体，管理主体和Worker 16 SYSTEM主体
后置list均确认目标实例为0；OpenAPI与运行时同时缺少集合POST。RS.5.3判定为
`FAIL/missing_model_instance_create`，问题归属Rust Server，完整报告见
[Rust Server ModelInstance创建接口缺失报告](rust-server-model-instance-create-missing-issue.md)。
Model 6和Worker 16继续保持停止/maintenance隔离，未触发Worker、Docker或GPU。

### RS.5.4 Server驱动的pending ModelInstance生成

RS.5.4验证正常用户部署链与直接ModelInstance POST是两条独立路径。Rust CLI不调用缺失的
`POST /v2/model-instances`；它先精确解析Worker，再调用`POST /v2/models`，由Server内部Model
controller按replicas生成ModelInstance。本门使用CLI且不带`--wait`，避免把预期pending误判为超时。

保留Model 6及其replicas 0不变，另创建唯一Model
`rust-rs5-server-driven-qwen25-05b-gpu0`。新增第二个Model是用户接受RS.5.3差异后继续真实主链的受控
范围扩展：Model 6固定保存RS.5.1/5.2证据，不能通过已确认存在字段问题的PUT改写。新Model固定
Cluster 3、replicas 1、Qwen2.5-0.5B本地路径、vLLM、既有Runner digest、Worker 16名称selector、
GPU0 selector、TP=1、max model length 2048、GPU memory utilization 0.5、dtype auto、eager且禁止
跨Worker分布式推理。

CLI使用新增但不激活的本地Context `m8rs-rs5-control`，只引用既有0600 management key；不得改变
用户当前active Context。请求前确认目标Model/Context均不存在、Worker 16仍停止并maintenance、Model
6仍replicas 0、Worker 16无Instance/File、21150/21151空闲、容器和GPU基线稳定。只允许一次CLI
deploy；结果不明确时按精确Model名称GET，禁止第二次POST。

通过门为CLI创建Model成功，响应和独立GET确认正ID、Cluster 3、replicas 1及冻结spec；Server在60秒
内内部创建恰好一个属于新Model的pending ModelInstance。由于Worker 16仍停止且maintenance，实例
不得进入scheduled，worker_id/port/GPU assignment保持空，Worker 16 ModelFile、目标进程、端口、
容器和GPU均不得增加。成功资源保留给后续ModelInstance list/watch及RS.6准备；本门不启动Worker、
不修改maintenance、不修复Worker标签、不停止任何现有Worker，也不调用Docker生命周期。

2026-11-15实际结果：非评分attempt 1在模型写入前因`pipefail`把`pgrep`零匹配误判为失败，确认没有
创建Context或发送Model POST后保留证据。attempt 2使用HEAD `29deb51605b1`只执行一次无`--wait`
CLI deploy，创建Model 7；Server约5秒后内部生成Instance 14，状态为pending，worker/port/GPU均空。
管理主体与Worker 16 SYSTEM主体各自精确看到一个目标实例；Model 6、Worker 16 maintenance、
ModelFile、进程、端口、容器和两张GPU均保持基线。RS.5.4判定为
`PASS/server_created_pending_instance`，资源保留给下一门。

### RS.5.5 ModelInstance精确GET与watch初始回放

RS.5.5只读验证Server内部生成的Model 7和Instance 14可被正常控制面读取，并能通过Rust Worker真实
使用的ModelInstance watch wire到达Worker 16。管理主体分别调用`GET /v2/models/7`和
`GET /v2/model-instances/14`；Worker 16 SYSTEM Token调用
`GET /v2/model-instances?watch=true`，观察初始回放后主动超时结束，不制造更新事件。

通过门要求两个精确GET均为HTTP 200且保持Model 7的Cluster 3、replicas 1/ready 0及Instance 14的
Model 7、Cluster 3、pending、空worker/port/GPU assignment；watch为HTTP 200和SSE，并在初始
`type=1`事件中恰好一次回放Instance 14，关键字段与精确GET一致。106已知跨Cluster范围缺陷可能让
Worker 16同时收到其他实例；额外事件单独计数并保留为RS.7差异，但不得参与目标字段判定或任何写入。

本门前后核对Server commit、Model 6、Model 7、Instance 14、Worker 16 maintenance、ModelFile、
目标进程/21150/21151、用户独立Worker、Context active状态、Docker和GPU。禁止PUT/POST/DELETE、
启动Worker、修改labels/maintenance、创建ModelFile或触发Docker生命周期。watch传输按固定短时窗口
结束时，curl超时退出28是预期的观察终点，不视为Server失败。

2026-11-15实际结果：两个精确GET均为HTTP 200；Worker watch为HTTP 200、`text/event-stream`，12秒
窗口只收到一个type=1事件，精确为Instance 14，字段与GET一致且其他Cluster事件为0。原分类器因
Instance 14的`updated_at`在调度器周期重试中自然推进约15秒而把字节级JSON差异判为FAIL；独立离线
裁决确认除`updated_at`外完整对象相同，原v2.2.3调度器在无候选Worker时同样调用实例更新服务。
原FAIL保留为非评分harness记录，未重发网络请求；RS.5.5正式判定为
`PASS/exact_get_and_initial_watch_replay`。

### RS.6 GPU0端到端执行顺序

RS.6复用Model 7和Instance 14，不创建第三个Model。固定顺序为：RS.6.0只读正式预检；RS.6.1在
maintenance下用普通`worker run`启动Worker 16并验证空载readiness；RS.6.2在maintenance下用一次
完整Worker PUT补齐规范`worker-name=rust-rs3-worker-90-20261113`标签；RS.6.3预先建立Instance、
ModelFile、Docker和GPU观察器及watchdog后解除maintenance，等待Server把Instance 14调度至Worker
16/GPU0并完成完整状态链；RS.6.4验证Server推理、Worker受保护代理、日志和重复请求；RS.6.5恢复
maintenance并精确清理Model/Instance/ModelFile、容器、进程、端口和GPU0残留。

每一阶段单独验收，前一阶段不自动授权后一阶段。Worker可以盘点并上报两张A100，但Model selector、
Instance assignment和vLLM容器设备必须只使用GPU0 UUID；GPU1无论空闲或存在外部任务都只观察，禁止
停止、分配或传入容器。Python Worker、用户独立Worker PID/20150/20151及其数据目录始终不在操作
范围内。

RS.6.3的目标状态链为`pending -> analyzing -> pending -> scheduled -> initializing -> downloading ->
starting -> running`；允许调度器重试导致相同pending事件或`updated_at`推进，但不得跳过Worker和
Server各自拥有的关键状态。Instance必须绑定Worker 16、`gpu_indexes=[0]`和唯一40000～40063端口；
ModelFile必须属于Worker 16并验证既有local path；只允许一个带完整ownership标签的Rust容器，固定
Runner digest且容器GPU可见性只含GPU0。vLLM健康必须先于或等于Server running写回，restart count
保持0且不得出现重复容器、重复ModelFile或GPU1进程。

解除maintenance前必须启用独立watchdog。任一失败优先恢复Worker 16 maintenance；随后向精确Rust
Worker PID发送SIGINT，并在核对Instance 14、Worker UUID、component和ownership标签后停止/删除唯一
Rust容器以释放GPU0。故障路径不自动删除Model 7/Instance 14，先保留证据；Server不可达时也必须
执行本地Worker/owned-container安全回滚。正常清理则先恢复maintenance，再删除Model 7并等待实例和
容器清理，对孤立ModelFile只用`cleanup=false`删除元数据，最后停止Worker。不得用回滚操作触碰
Python Worker、用户独立Worker或GPU1。

### RS.6.0 只读正式预检

RS.6.0冻结源码、Cargo.lock、Rust 1.99.1和重新构建的二进制身份；验证106仍为v2.2.3且单次预检
期间commit稳定，三类凭据和Context引用受保护。Server资源必须保持Model 7 replicas 1/ready 0、
Instance 14 pending且未分配Worker/端口/GPU、Worker 16 maintenance且缺少待修的worker-name标签、
Model 6 replicas 0和Worker 16 ModelFile为0。

宿主机门包括：21150/21151和40000～40063空闲、无Worker 16进程和Instance 14 owned container、
Docker socket可读写且nvidia runtime可见、固定Runner digest已存在、本地Qwen2.5-0.5B路径可解析、
GPU0无计算进程。GPU1负载只记录，不作为通过条件；Python Server/Worker、用户Worker health/metrics、
PID和容器身份必须保持。预留RS.6日志目录必须未占用。

本门只允许GET、文件元数据、Docker/NVML只读查询、locked/offline构建和Git外0700证据写入；禁止
Worker/容器生命周期、Server POST/PUT/DELETE、label/maintenance修改、镜像拉取和GPU任务。通过后
只解锁RS.6.1方案冻结与单独启动授权，不自动启动Worker。

2026-11-15实际结果：预检在HEAD `0ba5d54eae72`上通过。Rust Server保持v2.2.3 commit
`fba334c`，Worker 16保持maintenance且仅有`implementation=rust`标签；Model 7保持replicas 1/
ready 0，Instance 14保持pending且未分配Worker、端口或GPU，Worker 16 ModelFile为0。固定Runner
digest已存在，Qwen2.5-0.5B规范路径含10个文件、共999,604,126字节；21150/21151和40000～40063
空闲，没有目标Worker进程或Instance 14容器。

GPU0 UUID为`GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3`，显存、利用率和计算进程均为0；GPU1只
记录且同样空闲。Python Server/Worker和用户Worker的health/metrics均为200，用户Worker PID
1113114及20150/20151保持。locked/offline构建后的二进制SHA-256为
`e9fe6184f0307168e574775970e775967fef6beef2d91bbcbe3a14f81ed13905`，Cargo.lock SHA-256为
`3573ec9493671ebe67590500f841e0b7cbe1625241eb95250f36741d9f2e6599`。

attempt 1因错误使用loopback检查仅绑定宿主IP的用户Worker而提前退出；attempt 2因版本路径写错为
`/v1-openai/version`而退出；attempt 3把离线重建后仍运行的旧inode显示为`(deleted)`误判为错误，
其continuation又因`jq`管道优先级误判Context。原始STOP均保留，最终独立裁决用PID、`comm`、argv0、
监听端口和health确认用户Worker身份，并用加括号的Context谓词完成余下门。组合证据判定为
`PASS/readonly_preflight_ready_composite`，未发出写请求或触发进程、容器、镜像和GPU生命周期。

### RS.6.1 maintenance下统一Worker空载启动

RS.6.1只使用普通生产`worker run`，复用Worker 16的持久身份、Cluster注册凭据和m8rs3 data目录，
固定宿主身份`r760xa/10.213.80.90/eno8403`、21150/21151、本地Qwen model root和Docker socket。
本阶段创建独立m8rs6 control/log/evidence目录并启动一个精确PID；不使用阶段模式，不补标签，不关闭
maintenance，不调用容器生命周期。

启动前在包含本方案的干净HEAD上用Rust 1.99.1 locked/offline重建，记录二进制和Cargo.lock身份。
重新确认Worker 16 maintenance、Model 7/Instance 14冻结字段、Worker 16 ModelFile为0、目标进程和
端口池空闲、GPU0无计算进程、Python及用户Worker健康。GPU1允许存在外部任务，只记录其进程和资源，
不得把GPU1空闲作为启动条件或对其采取操作。

启动后观察至少90秒：本地health、ready和metrics为200，21150/21151由精确PID监听，Server heartbeat
前进且Worker 16继续maintenance、unreachable false、上报两张GPU和metrics port 21151。Model 7
保持replicas 1/ready 0，Instance 14保持pending且worker/port/GPU为空，Worker 16 ModelFile为0；
不得出现Instance 14容器、40000～40063监听、GPU0计算进程或managed reconciliation。已知D284跨
Cluster list/watch差异只记录，不把其他Cluster资源用于动作。

启动前建立短时watchdog。maintenance丢失或Instance 14开始分配时，先用即时完整Worker对象恢复
maintenance，再向本阶段精确PID发送SIGINT；若出现容器，只能在Instance 14、Worker UUID、component
和ownership全部匹配后停止/删除。其他失败只停止精确PID并保留maintenance。成功时PID保持运行，
control文件记录身份，供RS.6.2在maintenance下补齐`worker-name`；通过不授权RS.6.2写入。

2026-11-15 formal attempt 1：Worker PID 1148891在4秒内达到health/ready/metrics 200，identity从
m8rs3持久状态恢复并持续保持maintenance；Instance 14始终pending未分配，没有目标容器、运行端口
或reconciliation。观察第16秒watchdog检测到GPU0短时计算进程，按门立即SIGINT精确PID并判
`FAIL/gpu0_compute_appeared`。停止摘要确认heartbeat/status/full sync/watch成功、reconnect 0、
Instance/File desired为0及全部runtime/file计数为0。

回滚后PID、21150/21151和运行端口全部消失，maintenance、Model 7、Instance 14、ModelFile、容器、
GPU0及既有服务均恢复冻结基线。GPU1外部任务的PID同期从1100412变为1152850，强相关于外部任务
重启并可能在初始化时短暂探测GPU0，但trigger瞬间未采集进程身份，不能把该推断当作已证明归因。
attempt 1保持正式失败，不自动重试；下次须先稳定观察GPU0并让watchdog在触发时保存完整
compute-app快照。

2026-11-16 attempt 2修订：保留attempt 1全部目录，使用新的attempt2 control/log/evidence子目录。
正式启动前先连续90秒、每秒保存完整NVML compute-app快照；GPU0任一采样出现进程即判
`STOP/gpu0_not_stable`且不得启动。GPU1进程、显存和PID变化只记录，不作为停止条件。

通过稳定窗口后重新确认Server资源和既有服务，再启动唯一Worker PID。运行watchdog除原有maintenance、
Instance、容器、端口和GPU0计数外，必须在每次采样及任何触发分支先写入完整GPU UUID、PID、进程名、
显存和GPU总览，然后才执行SIGINT或条件回滚。运行观察仍至少90秒；成功终态、失败回滚和RS.6.2
授权边界不变。

attempt 2实际在稳定窗口第一次采样即捕获GPU0外部进程：用户`ckpt`、PID 3547519、Python路径
`/home/ckpt/anaconda3/envs/ckpt/bin/python`，触发时使用4,626 MiB，随后GPU0达到约36 GiB和100%。
GPU1同时由用户`jeff`的PID 1628716使用。门判定为`STOP/external_gpu0_occupied`，Worker未启动，
未创建attempt2 control/log目录，也未对两个外部进程采取操作。后续只能在用户确认GPU0空闲后建立
全新尝试，仍须重新通过完整稳定窗口。

用户确认GPU0空闲后的attempt 3只读前检发现Server资源已外部漂移：Model 7的replicas从1变为0，
更新时间为2026-11-16T07:21:37Z；Instance 14精确GET为404，Cluster 3实例清单为0。GPU0当时确实
空闲，Worker 16仍maintenance且没有目标进程或端口，但缺少真实pending Instance使D297隔离门失去
输入，因此attempt 3在稳定窗口和Worker启动前判`STOP/external_server_resource_drift`。

不得把空载启动替代该门，也不建议用已知会丢字段的Model PUT恢复Model 7。建议在获得独立授权后用
已验证CLI deploy创建唯一替代Model，复用相同Cluster 3、Worker 16、GPU0、Qwen路径、Runner和vLLM
参数，让Server内部生成新的pending Instance；新ID经只读验收后才能重启attempt 3。Model 7保持
replicas 0作为历史资源，后续统一清理。

用户已授权替代资源。唯一名称固定为`rust-rs6-server-driven-qwen25-05b-gpu0-attempt3`，通过非active
Context `m8rs-rs5-control`执行一次不带`--wait`的CLI `model deploy`。请求固定Cluster 3、Worker
`rust-rs3-worker-90-20261113`、GPU0、本地Qwen2.5-0.5B路径、原Runner digest和五项vLLM参数；
连接或响应不明确时只按精确名称GET，禁止第二次POST。

通过门为新Model字段精确匹配且Server内部生成恰好一个pending Instance，worker/port/GPU为空；
Worker 16保持maintenance，Model 7保持replicas 0，Worker 16 ModelFile、目标进程、端口、容器和
GPU0均无新增。新Model/Instance ID经证据冻结后替代D293中的Model 7/Instance 14用于余下RS.6，
旧资源只保留到最终清理。

实际恢复结果：第一次执行单元在GitHub fetch阶段卡住并被中断，POST未发送；第二次执行器在POST前
因GPU0外部任务停止，同样未发送请求。资源创建不运行Worker或GPU，故第三次执行把GPU状态改为只
记录外部基线，仍严格要求maintenance、无目标Worker/端口/容器。唯一CLI POST成功创建Model 9和
Server内部Instance 19；Instance为pending且worker/port/GPU为空。Model 9与Instance 19正式替代
Model 7/Instance 14用于余下RS.6；Worker启动前的90秒GPU0空闲门保持不变。

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

当前阻断差异：Cluster 3 的 Worker SYSTEM 主体可以读取 Cluster 1 的 Model，且
`GET /v2/model-files?worker_id=16`返回Worker 13～15的ModelFile。完整脱敏报告、原版行为依据、
建议修复范围和验收用例见
[Rust Server 跨 Cluster 可见性缺陷报告](rust-server-cross-cluster-visibility-issue.md)。
该差异当前由Rust Server开发者处理中；项目按2026-11-15临时推进裁决继续RS.5，但它仍阻断RS.7
最终兼容验收。

RS.5.2又确认`PUT /v2/models/{id}`返回HTTP 200并发布UPDATED事件，但忽略请求中的description，
响应、GET和watch均保留旧值；详细wire、oracle和建议修复见
[Rust Server Model PUT忽略字段缺陷报告](rust-server-model-update-ignored-fields-issue.md)。

RS.5.3确认106没有实现原版`POST /v2/model-instances`：唯一合法请求返回HTTP 405且未创建资源，
OpenAPI也只列集合GET。详细请求、原版依据、零副作用审计和建议回归见
[Rust Server ModelInstance创建接口缺失报告](rust-server-model-instance-create-missing-issue.md)。
