# Rust GPUStack Rewrite Documents

This directory is the durable design, governance, verification, and handoff
record for the Rust GPUStack rewrite. The first release milestone is a Rust
Worker that remains compatible with the existing Python Server.

The design was frozen through discussion before implementation. Chat remains
the primary collaboration channel; these files preserve the decisions and
evidence needed to resume the work without relying on chat history.

## Documents

- [charter.md](charter.md): objective, R1 scope, non-goals, invariants, and
  release criteria.
- [architecture.md](architecture.md): component boundaries, state ownership,
  reconciliation, runtime, configuration, and deployment topology.
- [cli-design.md](cli-design.md): CLI-first product contract and the Managed
  and Lab command surfaces.
- [worker-protocol.md](worker-protocol.md): Server/Worker compatibility,
  Worker HTTP API, authentication, and model lifecycle.
- [implementation-plan.md](implementation-plan.md): gated milestones,
  verification matrix, evidence requirements, and rollback containment.
- [decisions.md](decisions.md): accepted architecture decisions and rejected
  alternatives.
- [execution-log.md](execution-log.md): material checkpoints, current stopping
  point, and the next permitted action.

## Current status

- Design status: `frozen`
- Implementation status: `not started`
- Current stopping point: the accepted design has been recorded on the
  `codex/rust-worker` branch.
- Next permitted action: perform the M1 read-only inventory of the existing
  deployment on the 90 host, establish Git connectivity, and replace the
  environment placeholders recorded in these documents with verified values.

No Rust implementation, remote deployment change, or Python Worker change is
part of this documentation checkpoint.
