# syntax=docker/dockerfile:1.7

FROM rust:1.99.1-slim-bookworm@sha256:f2c004e3d2d8b547e8cf7ffbc29205385f4e71d23e53e4e383065db5e9f822cd AS builder

ARG GPUSTACK_GIT_COMMIT
WORKDIR /workspace

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

RUN case "${GPUSTACK_GIT_COMMIT}" in \
      ''|*[!0-9a-f]*) echo 'GPUSTACK_GIT_COMMIT must be a lowercase hexadecimal commit' >&2; exit 1 ;; \
    esac \
    && test "${#GPUSTACK_GIT_COMMIT}" -eq 40 \
    && GPUSTACK_GIT_COMMIT="${GPUSTACK_GIT_COMMIT}" \
       cargo build --release --locked --package gpustack-cli --bin gpustack-rs \
    && strip /workspace/target/release/gpustack-rs

FROM debian:bookworm-20261027-slim@sha256:a3bba7dc7dbcc1f37c2856c51fc9a7fac96608f50cb87b1b4e4d8b8c85ae988e

ARG GPUSTACK_GIT_COMMIT
ARG GPUSTACK_VERSION=0.1.0

LABEL org.opencontainers.image.title="GPUStack Rust Worker" \
      org.opencontainers.image.description="GPUStack Worker control plane and vLLM runtime coordinator" \
      org.opencontainers.image.source="https://github.com/GitHubcoincidence/gpustack" \
      org.opencontainers.image.version="${GPUSTACK_VERSION}" \
      org.opencontainers.image.revision="${GPUSTACK_GIT_COMMIT}" \
      io.gpustack.component="rust-worker" \
      io.gpustack.implementation="rust"

COPY --from=builder /workspace/target/release/gpustack-rs /usr/local/bin/gpustack-rs
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt
WORKDIR /var/empty
USER 65532:65532
STOPSIGNAL SIGTERM

ENTRYPOINT ["/usr/local/bin/gpustack-rs"]
CMD ["worker", "run"]
