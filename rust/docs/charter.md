# Rust GPUStack Rewrite: Purpose and Principles

## One-sentence objective

Rewrite GPUStack in Rust incrementally, beginning with a CLI-first Worker that
is compatible with the existing GPUStack Server and can reliably run vLLM in
local Docker containers on an NVIDIA host.

## Motivation and users

The project needs a smaller, explicit, recoverable Worker implementation whose
runtime behavior is testable from the command line and whose deployment can be
verified on real GPUs. Operators must be able to provision, inspect, exercise,
and remove every capability supported by a release without depending on the
web UI.

The first users are GPUStack developers and operators validating the Rust
Worker on a Linux host with two NVIDIA A100 GPUs. The long-term user is any
GPUStack operator who needs a scriptable and observable GPU cluster manager.

## Long-term product principle

GPUStack is CLI-first:

- Every user-facing or operator-facing capability supported by a release must
  have a stable, non-interactive CLI entry point.
- The Web UI and CLI are clients of the same Server APIs; neither owns a
  separate business workflow.
- Internal heartbeats, controller callbacks, and event streams are not exposed
  as manual user commands merely to satisfy CLI parity.
- During migration the Rust binary is named `gpustack-rs` to avoid colliding
  with the installed Python `gpustack` command. The intended final public name
  is `gpustack`.

## R1 scope

R1 includes:

- Linux x86_64 and NVIDIA GPUs, formally verified on a two-A100 host.
- A Rust Worker compatible with the existing Server version deployed on that
  host. The exact version, image digest, and commit are frozen by M1 evidence.
- A native Rust subset of `gpustack-runtime` for system inventory, NVML, and
  Docker Engine workload lifecycle management.
- vLLM as the only inference backend.
- Docker as the only workload runtime.
- Local model directories as the only model source.
- One Worker using one or two local GPUs, including tensor parallelism across
  the two GPUs.
- Existing `gpustack/runner` images as the vLLM execution environment.
- Explicit image selection from the model request, with a configured default
  vLLM image as fallback. Absence or incompatibility is a hard error.
- A Server-managed production mode and an isolated local Lab mode that share
  the same workload planner, runtime, health checks, logs, and reconciler.
- A unified Rust CLI for Worker operations, Lab demonstrations, Server-managed
  model operations, and inference requests.
- A minimal authenticated Worker HTTP API for health, readiness, version,
  local model inspection, model logs, configuration inspection, token
  validation, and metrics.
- Worker-container deployment as an R1 release gate. Host-process execution is
  an intermediate development mode only.
- Git-managed source, documentation, and milestone checkpoints.

## R1 non-goals

R1 does not include:

- Kubernetes, Podman, or a remote container runtime.
- AMD, Ascend, MUSA, or other accelerator vendors.
- SGLang, VoxBox, MindIE, custom backends, or arbitrary commands.
- Cross-Worker distributed inference or Ray clusters.
- Hugging Face or ModelScope downloading, cache management, or download
  progress.
- LoRA, GGUF parsing, speculative decoding, or extended KV cache services.
- Benchmark Manager or Cache Service Manager.
- Worker inference proxy, tunnel mode, or cluster proxy.
- Automatic Runner catalog selection or Runner override entries.
- Rewriting the vLLM or Runner container image payload in Rust.
- Rewriting every Server administration area in R1. CLI parity expands with
  each later rewritten module.

## Invariants and constraints

- The existing Server is the source of desired production state.
- Docker is the source of actual container state; the Worker reconciles the
  difference and never treats local manifests as authoritative.
- Only containers carrying matching Rust ownership labels may be mutated or
  deleted.
- Lab and Managed workloads have separate identities, directories, labels,
  ports, and status sinks.
- A transient Server or event-stream failure must not stop or delete healthy
  inference containers.
- Worker shutdown must not delete healthy Managed inference containers.
- Destructive cleanup requires authoritative Server state or an explicit Lab
  command scoped to Lab ownership labels.
- The Python Worker and Rust Worker must not advertise or consume the same
  physical GPUs concurrently during formal GPU testing.
- Worker system credentials and operator credentials remain separate.
- Tokens, Git private keys, model credentials, and sensitive environment values
  must not enter Git, command output, logs, or evidence archives.
- The deployed Server is not upgraded during R1 compatibility development.
- The fork `GitHubcoincidence/gpustack` is the writable source of truth;
  `gpustack/gpustack` is the upstream source.

## Success criteria

R1 is accepted only when evidence shows that:

- the Rust CLI builds reproducibly from a recorded commit and `Cargo.lock`;
- the Worker registers, authenticates, heartbeats, reports status, and recovers
  its event stream against the frozen Server baseline;
- the Server reports both A100 GPUs accurately;
- Lab mode deploys, inspects, invokes, logs, and removes a vLLM model entirely
  through CLI commands;
- Managed mode deploys, inspects, invokes, logs, and removes the same class of
  model entirely through CLI commands and the Server;
- single-GPU and two-GPU tensor-parallel deployments reach `RUNNING` and serve
  a valid inference request;
- process and Worker-container restarts adopt healthy existing workloads rather
  than duplicate or delete them;
- Server and event-stream reconnects converge through an authoritative full
  reconciliation;
- model deletion leaves no owned orphan container or reserved port;
- unknown and Python-owned containers remain untouched;
- the formal deployment runs the Rust Worker in a container and vLLM in sibling
  Runner containers;
- the documented rollback successfully restores the Python Worker;
- unit, contract, integration, security, and recovery evidence is recorded with
  exact commits, image digests, environment versions, and commands.

Performance measurements are mandatory evidence, but R1 has no claim that Rust
must beat Python by a predetermined percentage. Functional compatibility,
operational safety, and recovery are hard gates.

## Invalidating conditions

An R1 result is invalid if it relies on an unrecorded Server upgrade, uses a
different code or image revision than the recorded evidence, allows Python and
Rust Workers to double-count the same GPUs, bypasses the Server for a claimed
Managed test, promotes Lab smoke evidence to formal Managed evidence, exposes a
secret, or cleans up a container without proving ownership.

## Intended final capability

Completion permits GPUStack to claim that the selected Server version can use a
containerized Rust Worker to manage local Docker-based vLLM inference on one or
two NVIDIA GPUs, and that the complete R1 workflow is operable from a stable
CLI without the web UI.
