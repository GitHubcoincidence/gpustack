# Rust Worker: Implementation and Verification Plan

## Milestones

| Milestone | Scope | Prerequisites | Output | Acceptance evidence |
|---|---|---|---|---|
| M0 Design freeze | Charter, architecture, CLI, protocol, plan, decisions, and handoff record | User acceptance of design | This documentation set on `codex/rust-worker` | Review plus Git commit containing only intended docs |
| M1 Environment baseline | Read-only inventory of the 90 host, existing Server/Worker, GPU, Docker, network, models, Runner images, and Git connectivity | M0 committed | Verified baseline report and populated environment identities | Commands, versions, image digests, source mapping, network probes, and no deployment mutation |
| M2 Rust workspace and CLI skeleton | Four-crate workspace, config, context, output, error taxonomy, signals, and local tests | M1 baseline and toolchain | `gpustack-rs` with `version`, `validate-config`, and `doctor` | Format, lint, unit tests, Linux build, secret-redaction tests |
| M3 Server control plane | Version check, registration, identity persistence, heartbeat, status, watches, full sync, and minimum HTTP probes | Frozen Server fixtures | Rust Worker visible and stable without launching models | Contract tests, reconnect tests, real Server registration under isolated identity |
| M4 Inventory and Worker APIs | NVML/system reporting, filesystem APIs, logs, config read, metrics, and authentication | M3 | Server sees two A100s and can call required Worker APIs | Python/Rust fixture comparison, path security tests, HTTP auth tests, A100 evidence |
| M5 Docker runtime and Lab | Docker primitives, ownership labels, WorkloadSpec, vLLM builder, Lab commands, logs, health, and cleanup | M4 and maintenance window | CLI-only single-GPU Lab inference | Unit plans, Docker smoke, vLLM response, log stream, cleanup and non-ownership tests |
| M6 Managed single-GPU path | Server desired state, reconciler, state reporting, model CLI, Managed inference, and deletion | M5 | CLI-only Server-managed single-GPU inference | Full command transcript, Server states, response, logs, deletion with no orphan |
| M7 Recovery and two-GPU path | Adoption, event/full-sync recovery, restart policy, Server/Docker failures, two-A100 tensor parallelism | M6 | Reliable host-process Worker behavior | Restart, disconnect, failure, unknown-container, and two-GPU evidence |
| M8 Worker container and R1 candidate | Worker image, socket/NVML/network/path contract, persistent state, container restart recovery, rollback | M7 | Containerized Rust Worker release candidate | Repeat single/two-GPU suites, image digest, rollback to Python Worker, final audit |

## M1 gate

M1 begins with read-only inspection. It must identify:

- existing Server and Python Worker deployment mechanism, images, ports,
  networks, volumes, data directories, names, and current workloads;
- deployed Server version, image digest, and source commit mapping;
- OS, architecture, driver, Docker, NVIDIA Container Toolkit, CUDA visibility,
  and both A100 UUIDs;
- model directories and a suitable existing vLLM-compatible validation model;
- Server-container connectivity to a candidate direct inference port;
- usable Runner image references and digests;
- GitHub HTTPS/SSH connectivity and account-level Git SSH authentication;
- a safe maintenance, drain, test, and rollback procedure.

No existing container, service, configuration, port allocation, or model is
modified in the read-only inventory. Uncertainty that changes the deployment
topology returns the project to design discussion.

## Verification strategy

### Local automated checks

Tests in the repository use mocks and fixtures rather than real network, Docker,
database, or GPU resources. They cover configuration precedence, redaction,
CLI output and exit codes, Server protocol serialization, watch reconnects,
state transitions, image resolution, GPU mapping, port allocation, WorkloadSpec
generation, ownership checks, idempotent reconciliation, and failure policy.

### Contract checks

Fixtures captured from the frozen Python Server cover requests, responses,
enums, null values, error bodies, authentication, pagination, and line-delimited
watch events. Generated Python clients are not edited. The Python Worker and
runtime are compatibility oracles, not Rust runtime dependencies.

### 90-host integration checks

Real Docker, NVML, Runner, vLLM, and GPU tests run only on the 90 host during an
approved maintenance window. The Python Worker is drained and stopped before a
Rust Lab or Managed workload uses the GPUs. The Server remains running.

### Formal evidence identity

Every accepted integration result records:

- Rust and Server commits;
- `Cargo.lock` identity;
- Worker and Runner image names and digests;
- model path and stable model identity or hash information;
- OS, driver, Docker, toolkit, and relevant runtime versions;
- exact command and configuration identities;
- start/end time, result, logs, and cleanup verification.

Lab evidence is labelled diagnostic or smoke. Only the complete Managed flow
can satisfy Managed acceptance.

## R1 acceptance matrix

- CLI help, stable JSON output, redaction, confirmation, and exit codes.
- registration, authentication, heartbeat, status, and version compatibility.
- accurate two-A100 inventory and stable UUID/index mapping.
- Worker probe, filesystem, log, config, metrics, and authentication APIs.
- Lab deploy, status, logs, inference, stop, and remove on one GPU.
- Managed deploy, status, logs, inference, and delete on one GPU.
- Managed two-GPU tensor parallel inference.
- Worker process and Worker-container restart adoption.
- Server and watch-stream reconnect with authoritative convergence.
- vLLM abnormal exit with recorded restart/backoff behavior.
- Docker daemon transient-failure behavior.
- port conflict, invalid model path, incompatible image, and health timeout.
- no orphan owned workload after deletion.
- no mutation of unknown, Python-owned, or differently scoped containers.
- no credential in Git, logs, CLI output, or evidence.
- successful rollback to the preserved Python Worker.

Performance measurements include idle memory, startup time, reconciliation
latency, and control-plane overhead. They are characterization evidence rather
than pass/fail superiority claims for R1.

## Risk containment

- GPU double counting: never run Python and Rust Workers as schedulable owners
  of the same GPUs; use a maintenance window and explicit target Worker.
- Existing deployment damage: use separate identity, directories, ports,
  Compose project, and labels; preserve the Python configuration and data.
- Wrong cleanup: require the complete ownership label tuple and authoritative
  desired state; dry-run Lab cleanup by default.
- Protocol drift: pin the deployed Server and build fixtures from it.
- Path namespace mismatch: use identical host/Worker/vLLM model roots and test
  canonical mapping before model startup.
- Secret leakage: use token files, redaction tests, exact-path Git staging, and
  log/evidence review.
- Git loss: push material checkpoints to the writable fork; the 90 worktree is
  not the sole copy.
- Network or GitHub failure: stop before implementation and establish a safe,
  auditable synchronization path rather than leaving unpushed remote work.

## Rollback and cutover

The Python Worker remains intact through R1. Tests use a new Rust Worker name,
UUID, token, directories, ports, labels, and deployment definition. For a GPU
test, drain and stop the Python Worker, verify GPU and workload state, run the
Rust test, remove only Rust-owned test workloads, and restore the Python Worker
if the gate fails.

A permanent identity migration is outside R1 until the containerized candidate,
recovery suite, and rollback are accepted.

## Current stopping point

The design is frozen and implementation has not begun. The next permitted
action is to commit this documentation checkpoint, configure the writable fork
and 90-host Git access, then execute the read-only M1 inventory. Rust source
creation and deployment mutation are not yet permitted.
