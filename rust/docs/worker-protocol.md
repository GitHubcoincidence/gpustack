# Rust Worker Protocol and HTTP Compatibility

## Compatibility baseline

R1 targets the exact GPUStack Server deployment already running on the 90 host.
M1 records its version, container image tag and digest, and corresponding source
commit. The Server is not upgraded during R1 implementation or verification.

The local source commit `76dc841916ff8fed4068a1ab99d178d2e27a4a06` was used
for design inspection only. It is not the formal compatibility baseline until
M1 proves that it matches the deployed Server.

## Worker-to-Server operations

The typed Rust client must support the deployed equivalents of:

- Server version compatibility check;
- `POST /workers` registration;
- retrieval of the authenticated Worker identity;
- `POST /worker-heartbeat`;
- `POST /worker-status`;
- model and ModelInstance list/watch operations;
- authoritative unpaginated ModelInstance listing;
- ModelInstance observed-state updates;
- operator model CRUD and log APIs used by the R1 CLI;
- OpenAI-compatible inference requests used by the R1 CLI.

Exact prefixes, schemas, authentication headers, null behavior, enum spelling,
and error bodies are captured as contract fixtures from the frozen M1 Server.

## Worker HTTP API

### Public probes

| Method | Path | Contract |
|---|---|---|
| GET | `/healthz` | Process is alive; does not imply Server readiness. |
| GET | `/readyz` | Returns success only after prerequisites and initial authoritative reconciliation. |
| GET | `/version` | Rust version, Git commit, build target, and protocol baseline. |

### Worker-authenticated endpoints

| Method | Path | Contract |
|---|---|---|
| GET | `/token-auth` | Validate Worker or registration credential without exposing it. |
| GET | `/files/file-exists` | Report existence/type for a path under an allowed model root. |
| GET | `/files/model-config` | Return parsed JSON only for an allowed model configuration filename. |
| GET | `/files/model-weight-size` | Return weight bytes for an allowed local model directory. |
| GET | `/serveLogOptions/{id}` | Return compatible current/previous restart and container log options. |
| GET | `/serveLogs/{id}` | Stream compatible model logs and support the deployed query semantics. |
| GET | `/v2/config` | Return the non-sensitive Worker configuration view. |

The compatibility baseline may require a different versioned config prefix; M1
evidence takes precedence over this inspected default.

### Metrics

`GET /metrics` exposes Prometheus metrics for Worker readiness, Server
connectivity, heartbeat, status reporting, event reconnects, reconciliation,
Docker calls, workload health, model counts, and GPU inventory. The exact port
is frozen after M1.

### Deferred endpoints

R1 intentionally omits dynamic config writes, inference proxy, cluster proxy,
GGUF parsing, benchmark logs, cache-service logs, and the Python debug surface.
Their absence must be reported as unsupported rather than silently emulated.

## Authentication and authorization

Probe endpoints are unauthenticated and return no sensitive data. Other Worker
HTTP endpoints require the Worker credential or the registration credential
according to the frozen Server contract. Credential comparison is constant-time
where applicable, errors are generic, and logs contain only redacted
identifiers.

Managed CLI requests use an operator credential against the Server. The Server
performs existing tenant and authorization checks before proxying model log
requests. Worker system credentials are never used to create user resources.

## Readiness definition

Readiness is false until Docker and NVML are usable, registration succeeds,
initial status is published, the authoritative model snapshot is obtained, and
the first reconciliation completes. A sustained Server loss or unrecoverable
runtime failure makes readiness false while liveness may remain true.

## Event and reconciliation contract

Watch events mark a ModelInstance dirty and trigger keyed reconciliation.
Reconnect always triggers an authoritative full list. A periodic authoritative
sync is the final backstop. Cleanup never derives from a transient empty cache,
a failed request, or an incomplete page.

The Worker handles only instances assigned to its Worker identity. R1 rejects
cross-Worker distributed placement. A same-Worker assignment may contain one or
two GPU indexes.

## Model state reporting

R1 reports the existing state vocabulary:

```text
SCHEDULED -> INITIALIZING -> STARTING -> RUNNING
                                |           |
                                +-> ERROR <-+
```

Local model deployments do not enter `DOWNLOADING`. State messages are concise,
actionable, and redacted. The Server remains responsible for `PENDING`,
`ANALYZING`, scheduling, and unreachable-worker interpretation.

## Filesystem security

All requested paths are canonicalized with symlinks resolved and must remain
under a configured model root. Configuration reads use a filename whitelist.
The endpoints do not provide a general file-read primitive. Access failure,
malformed JSON, wrong file type, and out-of-root paths have stable error
categories captured by contract tests.
