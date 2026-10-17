# Rust Worker: Architecture and Interface Design

## Responsibility and boundaries

The Rust Worker owns node inventory, Server connectivity, Worker HTTP APIs,
desired/actual state reconciliation, Docker workload lifecycle, vLLM health,
logs, and recovery. It does not own model business state, scheduling policy,
gateway routing, user authorization, or vLLM implementation.

The existing Server remains authoritative for Managed model specifications,
placement, and desired lifecycle. Runner images remain the Python/CUDA/vLLM
payload. The Rust runtime replaces only the Docker, NVML, and system functions
needed by R1.

## Workspace boundary

The accepted initial workspace is:

```text
rust/
  Cargo.toml
  Cargo.lock
  crates/
    gpustack-cli/
    gpustack-client/
    gpustack-worker/
    gpustack-runtime/
  docs/
```

The four crates are stable ownership boundaries:

- `gpustack-cli`: command parsing, contexts, output, exit codes, and command
  routing.
- `gpustack-client`: typed Server REST, watch-stream, Worker registration, and
  OpenAI-compatible inference clients.
- `gpustack-worker`: supervisor, HTTP API, desired-state sources, reconciler,
  vLLM planning, status sinks, local state, and log service.
- `gpustack-runtime`: Docker Engine API, NVML, host inventory, workload status,
  and log primitives.

No additional `common` or `types` crate is introduced until a demonstrated
dependency boundary requires it.

## Operating modes

### Managed mode

```text
CLI or Web UI
  -> GPUStack Server
  -> scheduler and ModelInstance desired state
  -> Rust Worker watch/full-sync client
  -> reconciler
  -> Docker Engine
  -> gpustack/runner vLLM container
  -> direct inference endpoint
```

Managed mode starts with `gpustack-rs worker run`. Model commands call the
Server; they never directly create a production container.

### Lab mode

```text
gpustack-rs worker lab ...
  -> Lab desired-state adapter
  -> the same reconciler and WorkloadSpec
  -> the same Docker runtime and health checker
  -> isolated Lab container and local status store
```

Lab mode does not register with the Server, create Server database rows, use a
Worker token, or create gateway routes. Lab evidence is diagnostic or smoke
evidence, never a substitute for Managed acceptance.

## State ownership

| State | Authority |
|---|---|
| Model specification, replicas, placement, Worker/GPU assignment | Server |
| Worker identity and system credential | Server |
| Container existence, state, exit code, and image digest | Docker Engine |
| GPU identity and live utilization | NVML, correlated with Docker |
| Port reservation, restart count, logs, and local operation metadata | Rust Worker |
| Model readiness | vLLM health endpoint interpreted by Rust Worker |

Local manifests are recovery aids, not authorities. They must be reconstructible
from an authoritative Server snapshot and Docker ownership labels.

## Startup lifecycle

1. Parse configuration and acquire an exclusive data-directory process lock.
2. Start the HTTP listener with liveness available and readiness false.
3. Validate Docker, NVML, directories, model roots, and service port range.
4. verify compatibility with the frozen Server baseline.
5. Register the Worker and atomically persist returned identity and credential.
6. Publish initial system and GPU status.
7. Fetch an unpaginated authoritative ModelInstance snapshot.
8. Enumerate Docker containers carrying Rust ownership labels.
9. Perform the first full reconciliation and adopt matching healthy containers.
10. Start heartbeat, status, watch-stream, health, and periodic full-sync loops.
11. Mark readiness true only after the first authoritative convergence.

## Reconciliation model

Events are acceleration signals, not the sole source of truth. A keyed,
deduplicated work queue serializes reconciliation per ModelInstance. Startup,
watch reconnect, and a configurable periodic interval trigger an authoritative
full sync.

```text
desired ModelInstance + actual Docker workload + local recovery record
  -> normalize WorkloadSpec
  -> compare identity and spec hash
  -> create, adopt, replace, stop, or report
  -> persist local record
  -> report observed state
```

Deletion is allowed only after an explicit authoritative deletion/unassignment
or a successful full Server snapshot. An empty watch cache or failed API request
must never be interpreted as permission to delete.

Managed containers carry at least:

```text
io.gpustack.implementation=rust
io.gpustack.mode=managed
io.gpustack.worker-uuid=<uuid>
io.gpustack.model-instance-id=<id>
io.gpustack.spec-hash=<hash>
```

Lab containers carry `io.gpustack.mode=lab` and a Lab identifier. Each mutation
validates the complete ownership tuple.

## Model lifecycle

R1 uses the existing state vocabulary without the download phase:

```text
SCHEDULED -> INITIALIZING -> STARTING -> RUNNING
                                |           |
                                +-> ERROR <-+
```

- `INITIALIZING`: validate path, image, GPU assignment, port, and normalized
  workload plan.
- `STARTING`: pull/inspect the image, create and start the container, persist
  logs, and wait for the health deadline.
- `RUNNING`: the vLLM health probe succeeds.
- `ERROR`: startup, health, or runtime failure that cannot presently converge.

When `restart_on_error` is enabled, retries use delays of zero, 10, 20, 40, 80
seconds and so on, capped at 300 seconds. Restart count and current/previous logs
are preserved. Worker shutdown does not remove healthy Managed containers.

## Failure behavior

- Server unavailable: keep healthy workloads, stop destructive reconciliation,
  retry connectivity, make readiness false after a configured threshold, and
  perform a full sync after recovery.
- Watch stream unavailable: keep heartbeats and workloads, reconnect with
  backoff, then perform a full sync.
- Docker temporarily unavailable: report degraded Worker health, apply a grace
  period, then re-enumerate workloads. Do not immediately mark every instance
  failed.
- vLLM startup failure: capture a redacted diagnostic, set `ERROR`, and follow
  the configured restart policy.
- Unknown container: report but never mutate it.

## R1 vLLM WorkloadSpec

The normalized internal plan includes:

```text
instance identity
model path
image reference and resolved digest
GPU UUIDs and runtime indexes
service port
tensor parallel size
vLLM arguments
environment
mounts and shared-memory settings
health deadline
ownership labels
spec hash
```

Image selection order is explicit model image, configured default vLLM image,
then hard failure. The implementation does not import or invoke the Python
Runner catalog in R1.

The supported structured options are tensor parallel size, maximum model
length, GPU memory utilization, dtype, served model name, trust remote code,
and eager execution. A repeated controlled raw vLLM argument is permitted, but
it cannot override the Worker-owned model path, service port, GPU assignment,
container identity, or mounts.

## Path contract

Docker bind sources are interpreted by the host Docker daemon, not by the
Worker container. R1 therefore requires configured model roots to use identical
host and Worker-container paths, for example:

```text
host /data/models -> Worker /data/models -> vLLM /data/models
```

Every filesystem API resolves symlinks and requires the result to remain under
an allowed model root. Arbitrary host filesystem access is forbidden.

## Configuration and secrets

Configuration precedence is:

```text
CLI > environment > YAML file > defaults
```

The intended Linux locations are:

```text
/etc/gpustack-rust/config.yaml
/var/lib/gpustack-rust-worker/managed/
/var/lib/gpustack-rust-worker/lab/
/var/log/gpustack-rust-worker/
```

Exact runtime locations are verified during M1. Configuration contains secret
file references rather than secret values. Worker system credentials, operator
API credentials, and Git credentials are separate. Only a narrow non-sensitive
runtime configuration whitelist may eventually support hot updates; identity,
Docker endpoint, roots, and data directories require restart.

## Containerized R1 topology

The formal R1 topology is a Rust Worker container using the host Docker Engine
to manage sibling Runner containers. The expected baseline is host networking,
the Docker socket, NVML access, persistent Worker state/log volumes, and
identical model-root mounts. Exact Compose flags and user/group permissions are
frozen after M1 inspection.

Host-process Worker execution is allowed for bring-up through the first
single-GPU smoke test. All formal R1 recovery and two-GPU evidence is repeated
with the Worker container.

## Observability and recovery records

The Worker emits structured, redacted logs and Prometheus metrics for readiness,
Server connectivity, watch reconnects, reconciliation, Docker calls, workload
states, health failures, and GPU inventory. Per-instance atomic manifests record
container ID, spec hash, image digest, GPU UUIDs, port, restart count, and last
observed state. Loss of a manifest must not prevent recovery.
