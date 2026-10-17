# Rust GPUStack Rewrite: Execution and Handoff Log

## Document responsibility

This document records accepted decisions, material execution evidence, current
state, and the next permitted action. Chat remains the primary interaction
channel. Large command outputs and runtime artifacts are referenced by identity
and path rather than embedded here.

## Frozen decisions

- 2026-10-17: R1 scope, non-goals, CLI-first principle, Managed/Lab boundary,
  Worker protocol surface, reconciliation model, Git strategy, deployment
  topology, verification gates, and rollback approach were accepted.
- 2026-10-17: Worker containerization was moved into the R1 release gate;
  host-process execution remains a development stage.
- 2026-10-17: the final public CLI is unified as `gpustack`; the migration
  binary is `gpustack-rs`. The earlier idea of a final standalone
  `gpustack-worker` public command is superseded.
- 2026-10-17: the user selected account-level GitHub SSH access for the 90 host
  instead of a repository-scoped deploy key.

## Current state

- Status: `ready`
- Current stage: M0 design documentation checkpoint
- Source baseline inspected locally:
  `76dc841916ff8fed4068a1ab99d178d2e27a4a06`
- Branch: `codex/rust-worker`
- Existing unrelated working-tree state observed before the checkpoint:
  untracked `output/`; it is not part of this project change.
- Remote host state: unverified and unmodified.
- Implementation state: no Rust source, build, remote deployment, or Server
  mutation has begun.
- Current stopping point: accepted design is recorded under `rust/docs/`.
- Next permitted action: commit the exact documentation paths, establish fork
  and 90-host Git connectivity, then perform the M1 read-only inventory.

## Milestones

### 2026-10-17: M0 design frozen and archived

- Purpose: turn the accepted discussion into an auditable, resumable project
  contract before implementation.
- Outcome: charter, architecture, CLI, Worker protocol, implementation plan,
  decisions, and handoff state were recorded. Status is `ready` for the M1
  read-only baseline after the Git checkpoint.
- Evidence: `rust/docs/`; local base commit
  `76dc841916ff8fed4068a1ab99d178d2e27a4a06`; the Git commit created for these
  exact paths is the durable checkpoint identity.
- Interpretation: architecture and acceptance rules are frozen. This does not
  prove any remote environment fact or implementation capability.
- Invalid or non-formal artifacts: prior chat examples and unverified command
  sketches are explanatory only; no Lab or Managed run exists yet.
- New stopping point: no remote changes and no Rust implementation.
- Next permitted action: M1 read-only inspection and Git authentication setup;
  record verified identities before creating runtime code.

## Open risks and decisions

- The deployed Server version and source identity are unknown until M1.
- The existing Server/Worker Compose, ports, paths, active workloads, and
  maintenance mechanics are unknown until M1.
- Direct Server-to-vLLM network reachability is unverified.
- The validation model and Runner image digest are unverified.
- GitHub reachability and account-level SSH authentication from the 90 host are
  unverified.
- Exact Worker-container user/group and Docker socket permissions depend on M1.
