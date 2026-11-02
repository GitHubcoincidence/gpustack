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
| 禁止GPU | GPU1 |
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
- GPU0/GPU1均无显存和compute进程；
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
-无第二个容器、无第二代日志、无GPU1活动。

## 6. watchdog、cleanup与失败语义

独立watchdog硬超时1500秒，kill-after 180秒。正式executor非零退出时，watchdog按以下顺序恢复：

1. 根据完整身份恢复缺失的Server/Docker代理；
2. 恢复Worker maintenance；
3. 发现并删除唯一阶段model；
4. 精确停止Rust Worker；
5. 仅对完整ownership/spec匹配的遗留容器执行fallback cleanup；
6. 停止两个专用代理，删除精确Docker代理socket和空目录；
7. 删除本阶段新增且`cleanup_on_delete != true`的ModelFile元数据；
8. 确认容器、端口和双GPU为空后恢复同一Python Worker。

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
