# Rust GPUStack CLI-First Design

## Contract

Every user-facing or operator-facing capability supported by a release has a
stable non-interactive CLI command. The CLI invokes typed Server or inference
APIs for Managed operations and does not recreate Server business logic.

The development binary is `gpustack-rs`; the intended final public binary is
`gpustack`. This temporary name prevents collision with the Python command on
the validation host.

## R1 command tree

```text
gpustack-rs
  context add|use|list|remove
  worker run
  worker doctor
  worker validate-config
  worker gpu list
  worker runtime info
  worker config get
  worker lab deploy|list|status|logs|infer|stop|remove|cleanup
  model deploy|list|get|status|logs|delete
  inference models|chat|completion
  version
```

The final command tree retains these subcommands under `gpustack` and expands
with later rewritten Server modules.

## Managed model commands

`model deploy` authenticates as an operator and creates a model through the
Server API. It does not contact Docker or the Worker directly. R1 requires an
explicit target Worker so a test cannot be scheduled onto the Python Worker.

The R1 deployment input includes:

- model name;
- target Worker name;
- local model directory;
- explicit Runner image or a configured default;
- one or two local GPU indexes;
- tensor parallel size when using two GPUs;
- the supported structured vLLM options;
- `--wait` to follow the ModelInstance to `RUNNING` or `ERROR`.

The CLI converts friendly GPU flags into the existing Server selector form such
as `worker-name:cuda:0`. Port allocation remains a Worker/Server concern.

`model logs` calls the Server log API so that normal user authorization and
tenant visibility checks apply before the Server requests logs from the Worker.

## Inference commands

Managed `inference` commands call the Server or gateway OpenAI-compatible
endpoint. Lab inference calls the isolated local vLLM endpoint and is visibly
identified as Lab output. The two forms never silently switch endpoints.

Streaming commands preserve incremental output and handle interruption without
leaking authentication headers.

## Lab commands

Lab mode demonstrates the actual Worker runtime before Server integration is
complete. It reuses the production workload planner, runtime, health checks,
logging, and reconciler. It has no Server registration, Worker credential,
Server database row, or gateway route.

Lab deploy refuses to run when the Rust Managed data-directory lock is active,
when the requested GPU is already in use, when its port is occupied, or when a
same-name Lab workload conflicts. Formal testing also requires the Python
Worker to be drained and stopped; Rust cannot prove exclusive ownership merely
from its own lock.

Lab cleanup is label-scoped and defaults to `--dry-run`. There is no R1 flag to
force GPU preemption or delete an unknown container.

## Authentication

- `worker run` uses a registration credential and the resulting Worker system
  credential.
- `model` and Managed `inference` commands use an operator/user API credential.
- Lab commands use neither credential.
- Credentials are read from restricted token files or approved environment
  sources. A normal command-line argument must not contain the token value.
- Context configuration may record a Server URL and token-file path, never the
  token itself.

## Output and automation

All applicable commands support stable machine-readable output and predictable
process behavior:

```text
--output table|json
--quiet
--no-color
--timeout
--context
```

Result data goes to stdout; diagnostics and progress go to stderr. Success is
exit code zero. Configuration, authentication, compatibility, scheduling,
runtime, health, and timeout failures have distinguishable nonzero exit codes.
JSON field names are treated as a versioned compatibility surface.

Destructive user commands require confirmation in an interactive terminal and
support an explicit `--yes` for automation. Dry-run output contains the
normalized request or ownership-scoped cleanup plan without secrets.

## CLI parity gate

A feature is not accepted for a release if its supported user or operator flow
requires the Web UI or an undocumented raw request. Raw API escape hatches may
exist for diagnostics, but they do not satisfy CLI parity.
