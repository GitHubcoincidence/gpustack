# Rust GPUStack Rewrite: Accepted Decisions

All entries below are accepted unless explicitly marked pending. Architectural
changes require discussion and a new dated amendment; they must not be changed
silently to accommodate implementation difficulty.

## 2026-10-17 design freeze

| ID | Decision | Accepted choice | Reason |
|---|---|---|---|
| D001 | First rewrite boundary | Worker first; existing Server remains | Provides a testable compatibility boundary and avoids a simultaneous control-plane rewrite. |
| D002 | R1 backend | vLLM only | Establishes one complete path before adding backend variability. |
| D003 | R1 runtime | Local Docker on Linux/NVIDIA | Matches the validation host and keeps Kubernetes, Podman, and other vendors out of the first proof. |
| D004 | Distributed inference | Same-Worker one/two GPU only | Validates tensor parallelism without Ray or cross-Worker coordination. |
| D005 | Runner | Reuse existing `gpustack/runner` images | vLLM remains Python/CUDA software; rebuilding image payloads is unrelated to Worker control logic. |
| D006 | Runner selection | Explicit model image, then configured default, then error | Avoids hidden catalog choices and silent version mismatch. |
| D007 | Runtime package | Native Rust subset; no Python subprocess, PyO3, or FFI dependency | Produces an independently deployable Worker while limiting R1 runtime scope. |
| D008 | Model source | Local model directories only | Separates Worker execution from download and cache complexity. |
| D009 | Development topology | Host-process bring-up, then mandatory Worker container for R1 | Simplifies early debugging without weakening the production deployment gate. |
| D010 | Production topology | Worker container manages sibling vLLM containers through the host Docker Engine | Matches the intended GPUStack deployment model and supports process-independent workload recovery. |
| D011 | Network path | Direct Server/gateway access to vLLM for R1 | Avoids implementing Worker proxy and tunnel before the core lifecycle works. |
| D012 | Source control | Writable fork `GitHubcoincidence/gpustack`; upstream `gpustack/gpustack`; branch `codex/rust-worker` | Keeps history auditable and separates project work from upstream main. |
| D013 | Development host | 90 is the primary Linux/GPU worktree and validation host, not the sole source copy | Hardware-dependent work stays close to the target while Git remains authoritative. |
| D014 | Git authentication | Account-level SSH key on the 90 host, per explicit user choice | Enables access according to the user's GitHub account. The broader risk is controlled by a dedicated key, permissions, redaction, and later revocation. |
| D015 | Existing deployment | Preserve Server; drain and stop Python Worker only in approved GPU test windows | Prevents GPU double counting and supports immediate rollback. |
| D016 | Compatibility baseline | Pin the actual Server deployment found in M1 and do not upgrade it during R1 | Makes protocol evidence attributable and reproducible. |
| D017 | CLI product principle | CLI-first; every supported user/operator capability has a stable CLI | Makes the rewrite scriptable and prevents Web-only workflows. |
| D018 | Binary naming | `gpustack-rs` during migration, final `gpustack` | Avoids collision with the installed Python command while preserving the intended public interface. |
| D019 | Operating modes | Managed and Lab adapters over the same reconciler/runtime | Enables early CLI demonstration without creating a second production control plane. |
| D020 | Managed deployment | Model CLI calls the Server; Server remains desired-state authority | Preserves scheduling, authorization, database state, gateway behavior, and recovery. |
| D021 | Lab evidence | Lab is isolated diagnostic/smoke evidence, never formal Managed evidence | Prevents a successful local container run from being misreported as Server compatibility. |
| D022 | Worker API | Implement only the R1 probes, filesystem checks, model logs, config read, token validation, and metrics | Supports local-path deployment and CLI logs without importing proxy, K8s, benchmark, cache, or GGUF scope. |
| D023 | Reconciliation | Events plus startup/reconnect/periodic authoritative full sync | Recovers from missed events while preventing destructive decisions from transient caches. |
| D024 | Ownership | Complete mode/Worker/instance/spec labels gate every mutation | Protects unknown, Python-owned, Lab, and other Worker containers. |
| D025 | Shutdown | Do not delete healthy Managed model containers | Allows Worker process/container restart and adoption without model interruption. |
| D026 | Workspace | Four crates: CLI, client, Worker, and runtime | These are stable dependency and responsibility boundaries justified by the CLI-first scope. |
| D027 | Configuration | CLI over environment over YAML over defaults; secrets referenced by file | Supports automation while keeping secret values out of configuration history. |
| D028 | Performance | Measure performance in R1 but make compatibility and reliability the pass/fail gates | Avoids an unsupported speed claim before a controlled baseline exists. |

## Pending M1 identities

The following are intentionally unresolved values rather than open architecture
decisions: deployed Server version/commit/digest, current deployment topology,
driver and Docker versions, exact ports and paths, validation model identity,
Runner image digest, direct-network reachability, and final container user/group
flags. M1 must replace them with observed evidence before implementation relies
on them.
