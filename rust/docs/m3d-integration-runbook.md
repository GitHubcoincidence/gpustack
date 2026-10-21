# M3-D.2 真实控制面集成测试手册

本文把已经验收的 M3-D.2 测试提炼成可以人工复用的分组命令。它不是一键脚本：每组命令执行完成并满足停止条件后，才进入下一组。

## 测试分级

| 级别 | 用途 | 是否写 Server | 建议 |
|---|---|---:|---|
| A：证据复核 | 查看已接受的首次注册和恢复结果 | 否 | 随时可执行 |
| B：身份恢复复测 | 使用现有 per-Worker identity 启动 130 秒 | 是，只有 heartbeat/status 和控制资源读取 | 日常回归首选 |
| C：首次注册复现 | 使用全新的隔离名称、目录、端口和 Cluster token 启动 370 秒 | 是，会创建新 maintenance Worker | 仅在单独批准后执行 |

B 级不会启动 Docker workload、NVML、GPU 或 vLLM，也不会写 ModelInstance/ModelFile 状态。它仍然会更新真实 Server 上 Worker ID 2 的 heartbeat/status，因此不是纯只读命令。

C 级不得复用 `rust-m3d-r760xa`、Worker ID 2 或现有 `m3d` 数据目录。它会留下一个新的 maintenance Worker row，删除该 row 需要针对精确 ID 的单独批准。

## A 级：复核已接受证据

### A1. 定义证据位置

```bash
evidence_root="$HOME/.local/state/gpustack-rust-worker/evidence/m3d2-formal"
first_run="$evidence_root/first-run-retry-1"
second_run="$evidence_root/second-run"
```

目的：只为后续只读命令定义短变量，不创建或修改文件。

### A2. 查看两次运行摘要

```bash
jq '{
  worker_id,
  worker_name,
  worker_uuid,
  identity_source,
  heartbeat_ok,
  status_ok,
  full_sync_ok,
  watch_connected,
  reconnect_attempts,
  shutdown_signal
}' "$first_run/summary.json" "$second_run/summary.json"
```

通过标准：

- 第一次为 `identity_source=registered`，第二次为 `identity_source=persisted`；
- 两次均为 Worker ID 2、同一 name 和 UUID；
- 四个健康位均为 `true`，重连次数为 0；
- 两次都通过 `interrupt` 优雅停止。

### A3. 查看隔离复核结果

```bash
jq . "$second_run/final-admin-review.json"
cat "$first_run/health-60s.txt"
```

通过标准：同名 Worker 只有一个，关联 ModelInstance 和 ModelFile 数量均为 0，宿主机及 Server 容器到 Rust `/healthz` 都返回 200。

## B 级：现有身份恢复复测

B 级应在两个 SSH 终端中执行。终端 A 启动进程并等待 130 秒；终端 B 分别在大约 60 秒和 110 秒执行观测命令。

### B1. 两个终端都定义固定参数

```bash
source "$HOME/.cargo/env"
cd "$HOME/workspaces/gpustack" || exit 1

binary="$PWD/rust/target/debug/gpustack-rs"
data_dir="$HOME/.local/state/gpustack-rust-worker/m3d"
log_dir="$HOME/.local/state/gpustack-rust-worker/m3d-logs"
cluster_token_file="$HOME/.config/gpustack-rust/m3d-registration-token"
worker_token_file="$data_dir/managed/worker-token"
identity_file="$data_dir/managed/worker-identity.json"
worker_name='rust-m3d-r760xa'
worker_port='20150'
```

目的：固定使用已经验收的二进制、身份目录和 Worker 参数。变量不包含任何 secret。

### B2. 核验二进制身份

```bash
"$binary" --output json version
sha256sum "$binary"
```

当前已接受结果：

- embedded commit：`e30e7bd40d0d`；
- SHA-256：`4272530f2dde55f4679c955544bb64104c8cbff16ee7cc8ca8e32d0b875ca713`。

任一值不同都表示正在测试另一个构建，不得把结果追加到 M3-D.2 的已接受证据中。

### B3. 启动前停止门禁

```bash
test -f "$identity_file" || { echo 'STOP: worker identity is missing'; exit 1; }
test -f "$worker_token_file" || { echo 'STOP: per-Worker token is missing'; exit 1; }
test ! -e "$cluster_token_file" || { echo 'STOP: Cluster token copy must remain absent'; exit 1; }

if pgrep -af '[g]pustack-rs'; then
  echo 'STOP: a Rust Worker process is already running'
  exit 1
fi

if [ -n "$(ss -H -lnt "sport = :$worker_port")" ]; then
  echo "STOP: port $worker_port is already listening"
  exit 1
fi
```

目的：证明恢复所需的最小权限身份完整，共享 Cluster token 不在本地，同时避免重复启动或端口冲突。任一 `STOP` 都必须先调查，不能继续。

### B4. 检查现有 Python 服务

```bash
printf 'server=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 http://127.0.0.1/healthz)"
printf 'python_worker=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 http://127.0.0.1:10150/healthz)"
printf 'python_metrics=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 http://127.0.0.1:10151/metrics)"
```

通过标准：三项均为 200。这个基线用于证明 Rust 测试没有以停止 Python Server/Worker 为前提。

### B5. 终端 A 启动 130 秒恢复测试

```bash
umask 077
run_dir="$HOME/.local/state/gpustack-rust-worker/evidence/manual-recovery-$(date +%Y%m%d-%H%M%S)"
install -d -m 700 "$run_dir"
printf 'run_dir=%s\n' "$run_dir"

timeout --preserve-status --signal=INT --kill-after=20s 130s \
  "$binary" \
    --no-color \
    --output json \
    --timeout 30 \
    worker \
      --server-url http://127.0.0.1/ \
      --worker-name "$worker_name" \
      --worker-hostname r760xa \
      --worker-ip 10.213.80.90 \
      --worker-ifname eno8403 \
      --worker-port "$worker_port" \
      --token-file "$cluster_token_file" \
      --data-dir "$data_dir" \
      --log-dir "$log_dir" \
      --docker-socket /var/run/docker.sock \
      run --control-plane-only \
  >"$run_dir/summary.json" \
  2>"$run_dir/events.log"

run_exit=$?
printf 'run_exit=%s\n' "$run_exit"
```

CLI 层级必须是：顶层选项 → `worker` → Worker 配置选项 → `run --control-plane-only`。不能把 `--server-url` 等 Worker 选项放到 `worker` 之前。

`timeout` 在 130 秒时发送 SIGINT；Worker 应优雅退出并令 `run_exit=0`。如果进程提前退出，保留 `run_dir`，不要覆盖证据或自动清理身份。

### B6. 终端 B 检查双向 health

等待约 60 秒后执行：

```bash
printf 'host_health=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 "http://10.213.80.90:$worker_port/healthz")"

docker exec gpustack python -c \
  'import urllib.request; print("server_container_health=" + str(urllib.request.urlopen("http://10.213.80.90:20150/healthz", timeout=10).status))'
```

通过标准：两项均为 200。第二项验证的是 Server 容器实际使用的网络方向，不只是宿主机回环访问。

### B7. 终端 B 检查 Server 上的 Worker

```bash
{
  printf 'Authorization: Bearer '
  tr -d '\r\n' < "$worker_token_file"
  printf '\n'
} | curl \
  --silent \
  --show-error \
  --fail-with-body \
  --max-time 10 \
  --header @- \
  --get \
  --data-urlencode 'me=true' \
  http://127.0.0.1/v2/workers \
  | jq '.items[] | {
      id,
      name,
      state,
      unreachable,
      heartbeat_time,
      maintenance,
      metrics_port,
      gpu_devices
    }'
```

这条命令通过标准输入向 curl 提供 Authorization header，不把 token 放进命令行参数或输出。约 60 秒和 110 秒各执行一次。

通过标准：

- ID 为 2，name 不变；固定 v2.2.3 的 `me` 响应不包含 UUID，UUID 在 B8 的退出摘要中核对；
- `state=maintenance`、`maintenance.enabled=true`；
- `unreachable=false`、`metrics_port=null`、`gpu_devices=[]`；
- 第二次的 `heartbeat_time` 晚于第一次。

### B8. 终端 A 审阅最终摘要

B5 返回后，在同一个终端 A 执行：

```bash
jq '{
  worker_id,
  worker_name,
  worker_uuid,
  identity_source,
  heartbeat_ok,
  status_ok,
  full_sync_ok,
  watch_connected,
  reconnect_attempts,
  shutdown_signal
}' "$run_dir/summary.json"
```

通过标准：`identity_source=persisted`，身份仍为 Worker ID 2，四个健康位均为 `true`，重连次数为 0，停止信号为 `interrupt`。

### B9. 结束后隔离核验

```bash
pgrep -af '[g]pustack-rs' || true
ss -H -lnt "sport = :$worker_port"

printf 'server=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 http://127.0.0.1/healthz)"
printf 'python_worker=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 http://127.0.0.1:10150/healthz)"
printf 'python_metrics=%s\n' "$(curl -sS -o /dev/null -w '%{http_code}' --max-time 10 http://127.0.0.1:10151/metrics)"

docker ps --filter 'name=gpustack' \
  --format 'container={{.Names}} status={{.Status}} image={{.Image}}'

nvidia-smi --query-gpu=index,uuid,name,memory.used,utilization.gpu \
  --format=csv,noheader
```

通过标准：无 Rust 进程、20150 无监听；三项原服务仍为 200；原 GPUStack 容器仍在运行；测试没有产生预期外的 GPU 使用。

## C 级：复现首次注册

首次注册复现不是 B 级命令的“清空后重跑”。它必须：

1. 先获得真实写入的单独批准；
2. 选择全新的 Worker name、data/log 目录和已确认空闲的端口；
3. 通过隐藏输入或官方认证流程把默认 Cluster registration token 写入新的 `0600` 临时文件，绝不回显；
4. 使用与 B5 相同的 CLI 层级运行 370 秒，在约 60 秒和 330 秒执行 B6/B7 类观测；
5. 第一次尝试一旦结束，无论成功或失败，都删除精确 Cluster token 临时文件；
6. 预期摘要为 `identity_source=registered`；随后在 Cluster token 缺失时再运行 130 秒，预期为 `identity_source=persisted` 且 ID/UUID 不变；
7. 不自动删除新建的 Server Worker row 或本地身份。精确清理需要独立审批和证据。

当前已经验收的 `rust-m3d-r760xa`、Worker ID 2、端口 20150 和 `m3d` 数据目录只用于恢复测试，不再作为“首次注册”输入。

## 结果解释边界

M3-D.2 通过只证明真实 v2.2.3 Server 上的注册或身份恢复、heartbeat/status、三资源 full-sync/watch、最小 `/healthz`、优雅停止和 control-plane-only 隔离。它不证明完整 Worker HTTP API、ModelFile 准备、Docker runtime、GPU 盘点、vLLM 加载或推理请求。这些能力必须在后续里程碑各自建立证据。
