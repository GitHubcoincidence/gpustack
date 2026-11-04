# M7-E.3 控制连接故障恢复协议

## 1. 研究问题与结论边界

M7-E.3 要回答的问题是：当健康的 GPU0 TP=1 vLLM 已由 Rust Worker托管时，只中断该
Rust Worker自己的 Server连接或 Docker连接，Rust Worker能否保留最后确认的权威状态和健康
runtime，并在连接恢复后通过 full sync或重新枚举收敛到同一个容器，而不产生错误状态、重启、
重复容器或推理中断。

可证伪主张固定为：

1. Server代理故障期间，Rust Worker观测到 heartbeat/status/watch失败和 reconnect递增，但
   ModelInstance保持`RUNNING`，同一vLLM容器和推理继续工作；恢复代理后，先成功full sync再
   重新建立三个watch，并继续接管原容器。
2. Docker代理故障期间，2秒运行时协调真实触达Docker失败并使本地ready gate降级，但该不确定
   错误不写Server `ERROR`、不触发restart、不改变容器；恢复代理后重新枚举同一容器并恢复ready
   和日志读取。
3. 两个故障均不得在Docker生命周期中产生create/start/die/stop/kill/restart/destroy，且三轮
   后置协调完全幂等。

通过本阶段不证明真实Server进程故障、Docker daemon故障、网络分区、跨Worker恢复、GPU1、
TP=2或容器化Worker；这些不在M7-E.3范围内。

## 2. 实验变量与固定输入

唯一自变量是Rust Worker专属代理的可达性，分为两个严格串行子阶段：

- Server连接：`127.0.0.1:18080 -> 127.0.0.1:80`；
- Docker连接：
  `/home/wujiaxing2/.local/state/gpustack-rust-worker/m7/docker-proxy.sock ->
  /var/run/docker.sock`。

固定控制变量沿用M7-D和已接受的M7-E.1/E.2：

| 项目 | 固定值 |
|---|---|
| Server | GPUStack v2.2.3，commit `1cfcbbf` |
| Rust Worker | Worker ID 5，`rust-m3d-r760xa`，固定persisted UUID |
| 模型 | `Qwen2.5-0.5B-Instruct`，999,604,126字节，固定全文件manifest |
| Runner | 固定`quay.io/gpustack/runner` digest |
| workload | GPU0、TP=1、固定Managed端口池和spec |
| GPU资源边界 | GPU0排他；GPU1只观测，不进入调度、容器请求或清理 |
| Worker API/metrics | 20150/20151 |
| Server代理故障时间 | 至少70秒 |
| Docker代理故障时间 | 至少20秒 |
| 协调周期 | runtime 2秒；heartbeat/status/full sync 30秒 |

70秒Server故障保证至少跨越两轮30秒控制面周期；20秒Docker故障保证至少跨越十轮2秒运行时
协调。时间不足的attempt无论其他结果如何都不能通过。

## 3. 一个窗口、两个串行子阶段

M7-E.3使用一个明确授权的维护窗口，但包含两个独立故障子阶段：

1. 建立双代理、Rust Worker和健康workload基线；
2. 只终止Server代理完整进程组；
3. 恢复Server代理并通过full sync/watch/runtime/推理恢复屏障；
4. 只终止Docker代理完整进程组；
5. 恢复Docker代理并通过重新枚举/runtime/日志/推理恢复屏障；
6. 三轮幂等、cleanup、Python Worker恢复和独立post-audit。

两个故障不得重叠。Server子阶段失败时立即停止，不执行Docker子阶段；两个子阶段之间若任一
代理、Rust Worker、workload身份或推理不满足基线，也立即停止。

## 4. 代理身份与故障注入边界

代理使用`setsid`启动`socat`，每次记录role、PID、`/proc` starttime、PGID、SID、executable
路径和SHA-256、完整cmdline SHA-256、监听端点及探针结果。

代理leader必须同时满足`PID == PGID == SID`。终止前重新验证上述身份，并枚举该PGID全部成员；
所有成员必须属于同一SID且executable都是同一个`socat`。只有验证通过后才能对该专用进程组发
TERM，超时后才可对同一已验证进程组发KILL。不得使用`pkill`、进程名、通配符或未知PID。

Server代理停止后，真实`127.0.0.1:80`必须继续返回200；Docker代理停止后，`docker info`和
`/var/run/docker.sock`必须继续正常。真实Server、Docker daemon、Higress和其他容器均不是故障
对象。

## 5. 正式执行顺序

### 5.1 窗口前门禁

正式执行前必须重新确认：

- governance HEAD、origin、产品提交、`Cargo.lock`和五份脚本identity一致；
- locked/offline workspace构建和八项控制连接定向测试通过；
- Server、Python Worker和metrics为200；
-目标model/instance/Rust容器/Managed端口/Rust进程为零；
- 18080未监听，Docker代理socket不存在；
- GPU0无显存和compute进程；GPU1活动完整记录但不作为成败门；
-固定模型、Runner、Worker/Python Worker/GPU identity不漂移；
-阶段API Key、Context和三种token权限、模式、owner、link count及秘密扫描通过；
-错误HEAD注入在`window_entered=0`安全停止，前后现场完全不变。

只读preflight通过仅进入`ready_for_maintenance_authorization`，不自动授权正式窗口。

### 5.2 基线建立

正式窗口先停止精确Python Worker容器，再启动两个代理和Rust Worker。Rust Worker配置必须只用
代理URL/socket。创建GPU0 TP=1 workload并等待完整部署状态链和健康`RUNNING`，随后重新启用
maintenance以冻结调度边界。

基线至少记录：Model/Instance/File、restart字段、端口、spec/ownership、容器ID/StartedAt/PID、
manifest、generation 0日志identity、Worker PID/listeners、GPU进程、Server-only和直连推理。

### 5.3 Server连接故障

停止Server代理后必须同时满足：

- Server代理探针失败而Docker代理仍健康；
- Rust Worker PID、starttime、cmdline、20150/20151 listeners不变；
- heartbeat/status均出现0，三个watch均断开，reconnect计数增加；
- 70秒窗口内至少在即时、约40秒和约70秒完成连续性取证；
-每次ModelInstance都是`RUNNING`、restart count为0、last restart为空；
-容器ID、StartedAt、PID、端口、manifest、spec、ownership和generation 0日志不变；
-直连vLLM和Server-only models/chat/completion持续成功。

恢复Server代理后，heartbeat/status恢复、三个watch全部重连。watch只有在成功full sync后才打开，
因此三个watch恢复是本阶段的full sync恢复边界。随后相同runtime和完整Server-only日志必须可读。

Server可能暂时把Worker节点显示为不可达；这只允许是节点存活性表现，不允许改变ModelInstance或
runtime。

### 5.4 Docker连接故障

只有Server恢复屏障通过后才停止Docker代理。必须同时满足：

- Docker代理探针失败，真实Docker daemon和Server代理仍健康；
- Rust Worker本地`readyz`出现503；
-故障后新增runtime事件含`outcome=failed`，证明2秒协调真实触达Docker错误；
- 20秒窗口内直连和Server-only models/chat/completion持续成功；
- ModelInstance、restart字段、容器、端口、spec、ownership和日志代际保持基线；
-不得出现`outcome=error`、`restart_scheduled`、`created=true`或`deleted`。

Docker API不可用时读取容器日志可以暂时失败，因此故障窗口不把日志连续性作为PASS条件；恢复
Docker代理后，`readyz`必须恢复200，runtime事件必须重新枚举并报告同一容器`running`，完整
Server-only日志重新可读。

### 5.5 后置门禁

两个子阶段完成后：

- ModelInstance watch只允许观察到`RUNNING`；
-整个连接故障区间Docker生命周期事件文件必须为空；
-同一容器ID、StartedAt、端口、spec、ownership、restart count 0和generation 0保持不变；
-连续三轮协调及最后一次直连/Server-only推理和日志通过；
-无第二个容器、无第二代日志，且容器DeviceRequests和managed manifest始终只绑定GPU0；GPU1
  宿主机活动仅记录，不归因于本实验。

## 6. watchdog、cleanup与失败语义

独立watchdog硬超时1500秒，kill-after 180秒。正式executor非零退出时，watchdog按以下顺序恢复：

1. 根据完整身份恢复缺失的Server/Docker代理；
2. 恢复Worker maintenance；
3. 发现并删除唯一阶段model；
4. 精确停止Rust Worker；
5. 仅对完整ownership/spec匹配的遗留容器执行fallback cleanup；
6. 停止两个专用代理，删除精确Docker代理socket和空目录；
7. 删除本阶段新增且`cleanup_on_delete != true`的ModelFile元数据；
8. 确认容器、端口和GPU0上的本实验进程为空后恢复同一Python Worker；GPU1活动不阻塞恢复。

正常cleanup执行同样的安全顺序。未知进程、未知容器、多于允许数量的owned容器、身份不匹配或
无法归因的代理均fail closed，不得按名称或宽标签清理。

任何子阶段失败、故障重叠、时间不足、推理失败、状态漂移、容器事件、cleanup失败、watchdog失败
或post-audit失败都使formal attempt失败；已有局部证据不得提升为阶段PASS。

## 7. 证据和凭据边界

formal、watchdog和独立post-audit分别使用独立0700目录，文件统一0600并生成SHA-256 manifest。
Docker inspect只保存白名单字段，不保存`Config.Env`。operator、Cluster和persisted Worker token
只从受保护文件读取，原值不得进入证据、聊天或Git；精确值和通用secret模式扫描必须为零。

阶段短期API Key必须同时具备management/inference scope，最长7天。formal结果先进入
`ready_for_review`；用户接受后才删除API Key、Context和operator token文件并验证旧token为401，
随后M7-E.3才能标记`accepted`。

## 8. 当前停止点

协议已由用户接受并冻结。五份正式工具已在提交
`3a01873f050f9c7aefee8d4d6bc9c45cdda758f3`完成：本机项目锁定环境ShellCheck零告警，90上
五脚本`bash -n`、八项定向回归、workspace all-targets/all-features locked/offline build、代理参数
和cmdline identity自测全部通过；只读现场确认三个健康端点200、代理路径未使用、无Rust进程且
双GPU空闲。

用户已授权并完成阶段凭据与只读preflight attempt 1：API Key ID 25、名称
`rust-worker-m7e3-operator-20261102`、scope`management,inference`、生命周期604,800秒；active
Context为`m7e3-operator`。治理HEAD`6a86b6e9b98a431293c24093511061c6331e6e12`上的八组门通过，
错误HEAD注入按预期为`STOP/source_head_mismatch/window_entered=0`，三套证据权限、manifest、
秘密扫描和现场不变式通过。

记录该结果会推进治理HEAD。下一步只允许在新HEAD运行全新的只读preflight/injection attempt 2；
通过后状态为`ready_for_maintenance_authorization`。尚未授权停止Python Worker、启动代理、创建
workload、使用GPU、执行真实连接故障或进入M7-F。

formal attempt 1随后在精确停止Python Worker后固定失败于`server_proxy_start_failed`：后台PID
在`setsid`尚未exec为`socat1`时被过早采集，严格identity门拒绝继续；没有启动Rust Worker、创建
workload、使用GPU或进入故障子阶段。该PID随后成为预期代理但因current identity未发布而残留，
首次post-audit发现18080监听并失败。按完整identity精确终止后，独立supplemental recovery audit
attempt 2证明现场恢复；原formal和一次补救证据因封存后日志写入标记为INVALIDATED。

工具已改为等待exec完成后才发布proxy identity，并让watchdog拒绝任何可达但无匹配identity的
代理。修复通过本地ShellCheck、90 Bash语法和Server/Docker代理各20次真实identity循环。当前为
`retry_preflight_pending`；attempt 1授权已消费，正式retry仍须重新授权。

修复提交`777506c53c66aa75ee41fe9c5c8a0054dc3240b6`上的preflight/injection attempt 3已通过，新增
代理等待与watchdog身份门、八项恢复测试、双scope、固定输入、错误HEAD无写和外部不变式成立。
记录该结果会推进治理HEAD，因此下一步只允许在最终文档HEAD运行attempt 4；formal retry仍须
新的明确维护授权。

最终治理HEAD`260da89e727674474131ca08c42c29bc4c48b060`上的preflight/injection
attempt 4随后通过，用户据此重新授权formal attempt 2。正式执行在停止Python Worker并建立双代理
后，Rust Worker后台PID仍处于`setsid`到`gpustack-rs`的exec窗口；执行器立即读取到过渡身份并以
`initial_rust_worker_start_failed`停止。未创建Model、ModelInstance、ModelFile或vLLM容器，也未
进入Server/Docker连接故障子阶段，因此该attempt不支持任何产品恢复结论。

formal在cleanup中因没有已发布的Worker身份而无法精确终止已exec完成的Rust Worker，并在等待
后台PID时阻塞。按PID、父executor、starttime、PGID/SID、二进制和完整cmdline SHA-256精确补证后，
只对该Worker发送有界INT/TERM，原executor继续完成代理清理和Python Worker恢复。formal和watchdog
均保持FAIL，独立post-audit在`watchdog_recovery=0`证据链门停止；成功恢复不得改判正式结果。另对
attempt 1遗留但不可达的Docker代理按原PID/starttime/cmdline与当前最终`socat1`身份交叉验证后
精确清除。

修复提交`e994ed68e0bb134083a42094b3000f78ea5c0409`新增Worker专用身份等待和验证：只在
`gpustack-rs`二进制、固定cmdline及PID=PGID=SID全部成立后原子发布身份；executor和watchdog在停止
Worker前再次验证该身份，缺失或不匹配时立即fail closed而不再无限`wait`。preflight新增静态强制门
和“前两次过渡身份失败、第三次最终身份成功”的回归测试。修复通过`git diff --check`、四脚本
ShellCheck、90上的四脚本`bash -n`和身份转换测试。

当前状态为`retry_preflight_pending`。attempt 2授权已经消费，所有失败和补救证据保持原样；下一步
只允许把本记录纳入新治理HEAD并运行全新的只读preflight/错误HEAD注入。只有新门再次确认GPU0
空闲且外部状态未漂移后，才能请求formal attempt 3的新维护授权；M7-F仍未授权。

最终治理HEAD`055cabdc31a36ecd3046c23bd32c79d4697f44d6`上的preflight/injection
attempt 5随后通过，用户据此授权formal attempt 3。该次运行首次完成双代理和Rust Worker安全
启动，并把唯一GPU0 TP=1 workload部署到`RUNNING`。Server代理中断后，即时直连和Server-only
推理成功；约40秒时同一vLLM容器、ModelInstance `RUNNING`、restart count 0、端口、spec、
ownership和generation 0日志仍未变化，直连vLLM chat继续成功，但Server-only chat返回503
`Managed Worker runtime is not ready`。executor按冻结门以
`server-outage-40s_server_inference_failed`停止，没有把不足70秒的Server故障提升为通过，也没有
进入Docker故障子阶段。

取证确认2秒协调在Server不可达时持续读取ModelInstance失败，并把全局runtime readiness降为
false；Worker推理代理把该全局位当作请求前置门，因而拒绝仍有最后确认路由且健康的vLLM。修复将
`/readyz`与数据面路由分离：`/readyz`仍反映协调健康，而受原认证保护的chat/completion按最后确认
并仍存在的ModelInstance路由转发；路由缺失和上游失败仍分别失败。该语义同时满足后续Docker代理
故障期“ready降级但健康runtime推理连续”的冻结要求。

formal正常cleanup为`cleanup_ok=1`并恢复服务和资源。独立watchdog随后把“正式Worker identity文件
存在但记录PID已安全退出”误判为身份失败，故`recovery_ok=0`，post-audit在证据链门停止。watchdog
已改为只对仍存在的PID执行完整身份验证和终止；记录PID不存在视为幂等成功，格式错误或存活但不
匹配的PID仍fail closed。补充只读审计只证明现场恢复，不改变formal/watchdog/post-audit的FAIL。

当前状态为`retry_preflight_pending`。attempt 3授权已经消费；下一步只允许提交上述产品与watchdog
修复、运行质量门并在新HEAD执行全新的只读preflight/错误HEAD注入。formal attempt 4仍须新的明确
维护授权，M7-F仍未授权。

修复提交`2f72d2bd46bdb6e71ea3f4df7d32fb98e375925c`后的preflight attempt 6按旧
`PRODUCT_COMMIT`漂移门安全停止于第1组，未执行构建、错误HEAD注入或任何外部变更。M7-E.3产品
基线现显式推进到该修复提交；后续治理提交不得再修改`rust/crates`，下一步使用全新attempt 7路径
重新运行完整只读门。

治理HEAD`eb10dd63f8f9c636e255686b1a35d5b990076fce`上的preflight/injection attempt 7
全部通过：新数据面连续性测试、已退出/格式错误Worker identity正反测试、九项恢复测试、脚本静态
门、固定输入、双scope、错误HEAD无写和外部不变式成立。记录该结果会推进治理HEAD，因此下一步
只运行最终只读attempt 8；通过后才能请求formal attempt 4的新维护授权。

最终治理HEAD`027e42aa9bbfea6be21e1711b09578873e6bb1e3`上的preflight/injection attempt 8
通过后，用户授权formal attempt 4。唯一GPU0 workload达到`RUNNING`；Server代理断开70秒期间，
即时、40秒、70秒的直连与Server-only推理全部成功，恢复屏障通过。Docker代理断开时readyz降级，
但两条推理路径继续成功且runtime identity不变；代理恢复后同一实例持续返回
`noop/already adopted`。冻结观察器只接受`outcome=running`，因此formal以
`docker_runtime_reenumeration_timeout`失败，后续完整证据与幂等门未运行，不能判定阶段通过。

提交`8427f657940fa80b5326b3e28eda9939445db7dd`把Docker恢复证据限定为同一实例的`running`或
`noop/already adopted`，随后仍比较容器ID和完整runtime contract；Python Worker回滚只等待本实验
Rust进程、容器和监听清空，不再被无关GPU任务阻塞。formal/watchdog/post-audit保持FAIL；精确恢复
审计先证明服务与本实验资源恢复。外部DiffKV自然结束后，独立补充审计以23个0600文件、0 symlink
和manifest文件SHA-256
`fb6ff649fe9fa64428ddd13b6cdc2fcdbadc60f9981994a6782e1359e0ea4e67`证明全局资源、同一Python
Worker、端口、代理、本地残留和双GPU完整恢复。当前为`retry_preflight_pending`；下一步运行全新
preflight/injection attempt 9，任何formal attempt 5须重新明确授权，M7-F未授权。

preflight attempt 9在治理HEAD`f3bbc1c98f6925a232cac471f5776677a1bd3682`完成前四组后，外部
DiffKV重新占用GPU1，旧双GPU空闲门在`fixed_runtime_inputs`安全STOP；未创建injection目录或进入
维护窗口。提交`8fb4d3e761a045958d72c41ade37ded1bbc8c3c2`把资源边界固定为GPU0排他、GPU1
只观测：GPU0必须预先空闲，唯一Rust容器的Docker DeviceRequests及managed manifest必须精确绑定
GPU0 UUID，故障期持续比较完整identity；GPU1活动只记录，不作为preflight、cleanup或post-audit
成败门。下一步在最终治理HEAD运行preflight/injection attempt 10；通过后才能请求formal attempt 5
维护授权，M7-F未授权。

attempt 10在GPU1外部任务活跃时通过前五组，随后因静态审计扫描preflight自身并匹配检查语句里的
GPU1字面量，以`STOP/script_static_audit`结束；没有创建injection目录或外部变更。提交
`af8bbf13f84f46606ff85c06399c5dec865c1cac`把该扫描限定到executor/post-audit，preflight继续由
第4/5/8组动态验证GPU0边界。下一步在最终治理HEAD运行全新attempt 11；通过后才能请求formal
attempt 5维护授权，M7-F未授权。

最终治理HEAD`954b1a393607284c3a1648c0bf2a572fdfc20ad3`上的preflight/injection attempt 11
通过，manifest文件SHA-256分别为
`c90e6e2d4e3f6efa2598aa8f25f7586f242c26790b284341f8525adae396ff1d`和
`4435f46889b391a02f638b78efa29f8ab37c995b94079a910ad783e6ed33fa47`。用户授权formal attempt 5；
GPU0为空闲排他卡，GPU1外部DiffKV只观测。运行在双代理和Rust Worker启动后创建唯一实例45，
状态到达`STARTING`，但成功启动的容器仍短暂显示`Created`，下一轮重复首次GPU空闲授权并把同容器
自己的GPU0进程误判为外部冲突，实例两次进入`ERROR`。未进入任一连接故障子阶段，因此formal
保持FAIL。

formal/watchdog/post-audit manifest文件SHA-256分别为
`ceda8d49ce881e61edc1681776bb311de458ced092b0ce7de80f94a25e458c86`、
`f1e23e5fadb17f1a71ca6826ddcad0bc99e9faa701fdc614c7c3ed03369439da`和
`5f60ac45a7de9830ce8631f5152136b551fc646ef1257bba73f7cc024200bc89`；formal为FAIL但
`cleanup_ok=1`，watchdog为FAIL但`recovery_ok=1`，独立post-audit为PASS且只证明恢复基线。

产品提交`e842c01ffbe011cda45c6b0e4447e5bea95288ac`在成功Docker start后立即持久化同一manifest为
`starting`。若Docker状态仍滞留`Created`，后续协调不再重复GPU空闲授权或start；真正start失败时
manifest仍为`created`并继续使用既有安全重试。新竞态回归、全部workspace测试、格式和Clippy通过，
preflight固定加入该回归，`PRODUCT_COMMIT`推进到该提交。当前为`retry_preflight_pending`；下一步
只在新治理HEAD运行preflight/injection attempt 12，formal attempt 6仍须独立维护授权，M7-F未授权。

治理HEAD`cd24ac93e218c09de9bb2fe70d2cfff4e21647fa`上的preflight/injection attempt 12
随后通过。preflight含73个0600文件，manifest文件SHA-256为
`06dd06441c1a737c7b999283d3e722f4fc4d4cf90093480d21ace0b8420f071e`；错误HEAD注入含11个
0600文件，按预期为`STOP/source_head_mismatch/window_entered=0/cleanup_ok=1`，manifest文件
SHA-256为`5e055ef42949cdc408dbb5b2577721c65d69693c0c6de1799a0b64cec240388e`。两目录均为
0700、无symlink，清单、秘密扫描、GPU0空闲和GPU1只观测边界通过。用户随后明确授权formal
attempt 6。

formal attempt 6完整通过两个串行故障子阶段。唯一Model 31、Instance 46、ModelFile 25和GPU0
容器`a9eab8be38788148a8f8e7dec4cfc1b2049249262da4a993a10cd97fda01b332`在端口40000达到
健康`RUNNING`。Server代理中断70秒、恢复屏障、Docker代理中断20秒和恢复屏障全部通过；全程
ModelInstance只观测到`RUNNING`，restart count保持0，容器ID、端口、spec、ownership和generation
0不变，直连及Server-only推理连续，Docker生命周期事件为零，三轮后置协调幂等。容器
DeviceRequests和managed manifest只绑定GPU0；GPU1外部DiffKV进程在前后保持同一PID和显存，未被
本实验操作。

formal、watchdog和独立post-audit分别为`PASS/cleanup_ok=1`、`PASS/recovery_ok=1`和
`PASS/recovery_baseline=restored`；三目录分别含280/8/27个0600文件、0 symlink，manifest文件
SHA-256分别为`5107479bbee63b56a7e2f205ad1381bbb0723475c2146b76e2f1f0806330f4f3`、
`8fdeddf319ad5f679a7301c840accd20641cc0915fdac8139ca848b398795462`和
`3bc303086e1557188d25542914eecf1409d6b09905671f18f3461e060e32a208`。同一Python Worker和三个
健康端点已恢复，测试资源、Rust进程/容器、代理、socket、监听和GPU0计算进程全部清空。

当前状态为`ready_for_review`，不是`accepted`。下一步只允许用户审阅并接受或拒绝attempt 6报告；
接受后才能删除API Key ID 25、`m7e3-operator` Context和operator token文件，验证旧token为401，
再把M7-E.3标记为`accepted`。M7-F仍未授权。
