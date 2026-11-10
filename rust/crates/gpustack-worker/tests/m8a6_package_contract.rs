use std::fs;
use std::path::{Path, PathBuf};

fn rust_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(relative: &str) -> String {
    fs::read_to_string(rust_root().join(relative)).expect("package contract file")
}

#[test]
fn package_contract_pins_images_and_the_unified_worker_command() {
    let dockerfile = read("docker/worker/Dockerfile");
    let ignore = read(".dockerignore");
    let run = read("scripts/m8a6/run-worker-container.sh");
    let stop = read("scripts/m8a6/stop-worker-container.sh");

    assert!(dockerfile.contains("rust:1.99.1-slim-bookworm@sha256:"));
    assert!(dockerfile.contains("debian:bookworm-20261027-slim@sha256:"));
    assert!(dockerfile.contains("cargo build --release --locked"));
    assert!(dockerfile.contains("ENTRYPOINT [\"/usr/local/bin/gpustack-rs\"]"));
    assert!(dockerfile.contains("CMD [\"worker\", \"run\"]"));
    assert!(!dockerfile.contains("python"));
    assert!(!dockerfile.contains("gpustack/runner"));
    assert!(ignore.lines().any(|line| line == "target/"));

    for required in [
        "--network host",
        "--restart unless-stopped",
        "--runtime nvidia",
        "--gpus all",
        "--read-only",
        "--cap-drop ALL",
        "--security-opt no-new-privileges",
    ] {
        assert!(run.contains(required), "missing run contract: {required}");
    }
    for legacy in [
        "--control-plane-only",
        "--resource-api-only",
        "--managed-file-only",
        "--managed-runtime",
    ] {
        assert!(!dockerfile.contains(legacy));
        assert!(!run.contains(legacy));
        assert!(!stop.contains(legacy));
    }
    assert!(!stop.contains("--force"));
}

#[cfg(unix)]
mod unix {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::process::{Command, Output};

    use tempfile::TempDir;

    use super::rust_root;

    const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
    const IMAGE_ID: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const CONTAINER_ID: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    struct RuntimeFixture {
        _directory: TempDir,
        _socket: UnixListener,
        docker: PathBuf,
        capture: PathBuf,
        config: PathBuf,
        token: PathBuf,
        data: PathBuf,
        logs: PathBuf,
        models: PathBuf,
        socket: PathBuf,
    }

    impl RuntimeFixture {
        fn new(fake_docker: &str) -> Self {
            let directory = tempfile::tempdir().expect("runtime fixture");
            let root = directory.path();
            let docker = root.join("docker");
            let capture = root.join("docker-arguments.txt");
            let config = root.join("worker.yaml");
            let token = root.join("registration-token");
            let data = root.join("data");
            let logs = root.join("logs");
            let models = root.join("models");
            let socket = root.join("docker.sock");

            fs::write(&docker, fake_docker).expect("fake Docker");
            fs::set_permissions(&docker, fs::Permissions::from_mode(0o700))
                .expect("fake Docker mode");
            fs::write(&config, "server_url: http://127.0.0.1/\n").expect("configuration");
            fs::write(&token, "redacted-test-token\n").expect("token");
            fs::set_permissions(&config, fs::Permissions::from_mode(0o600))
                .expect("configuration mode");
            fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).expect("token mode");
            for path in [&data, &logs, &models] {
                fs::create_dir(path).expect("runtime directory");
                fs::set_permissions(path, fs::Permissions::from_mode(0o700))
                    .expect("runtime directory mode");
            }
            let listener = UnixListener::bind(&socket).expect("fake Docker socket");

            Self {
                _directory: directory,
                _socket: listener,
                docker,
                capture,
                config,
                token,
                data,
                logs,
                models,
                socket,
            }
        }

        fn run_script(&self, script: &str) -> Output {
            Command::new("bash")
                .arg(rust_root().join("scripts/m8a6").join(script))
                .env("GPUSTACK_DOCKER_BIN", &self.docker)
                .env("GPUSTACK_RS_IMAGE", "gpustack-rust-worker:test")
                .env("GPUSTACK_RS_EXPECTED_IMAGE_ID", IMAGE_ID)
                .env("GPUSTACK_RS_EXPECTED_GIT_COMMIT", COMMIT)
                .env("GPUSTACK_RS_CONFIG_PATH", &self.config)
                .env("GPUSTACK_RS_TOKEN_FILE", &self.token)
                .env("GPUSTACK_RS_DATA_DIR", &self.data)
                .env("GPUSTACK_RS_LOG_DIR", &self.logs)
                .env("GPUSTACK_RS_MODEL_ROOT", &self.models)
                .env("GPUSTACK_RS_DOCKER_SOCKET", &self.socket)
                .env("FAKE_CAPTURE", &self.capture)
                .env("FAKE_IMAGE_ID", IMAGE_ID)
                .env("FAKE_COMMIT", COMMIT)
                .env("FAKE_CONTAINER_ID", CONTAINER_ID)
                .output()
                .expect("run package script")
        }
    }

    fn output_text(output: &Output) -> String {
        format!(
            "stdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    }

    #[test]
    fn package_validator_is_docker_free_and_passes() {
        let output = Command::new("bash")
            .arg(rust_root().join("scripts/m8a6/validate-package.sh"))
            .output()
            .expect("package validator");

        assert!(output.status.success(), "{}", output_text(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "m8a6_package_contract=PASS\n"
        );
    }

    #[test]
    fn run_script_emits_the_hardened_container_contract_with_fake_docker() {
        let fixture = RuntimeFixture::new(
            r#"#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail
if [[ "$1 $2" == 'image inspect' ]]; then
  printf '%s|%s|rust-worker|rust|["/usr/local/bin/gpustack-rs"]|["worker","run"]\n' "$FAKE_IMAGE_ID" "$FAKE_COMMIT"
elif [[ "$1 $2" == 'container ls' ]]; then
  exit 0
elif [[ "$1" == 'run' ]]; then
  printf '%s\n' "$@" >"$FAKE_CAPTURE"
  printf '%s\n' "$FAKE_CONTAINER_ID"
else
  exit 90
fi
"#,
        );

        let output = fixture.run_script("run-worker-container.sh");

        assert!(output.status.success(), "{}", output_text(&output));
        let arguments = fs::read_to_string(&fixture.capture).expect("captured Docker arguments");
        for required in [
            "--network\nhost\n",
            "--restart\nunless-stopped\n",
            "--runtime\nnvidia\n",
            "--gpus\nall\n",
            "--read-only\n",
            "--cap-drop\nALL\n",
            "--security-opt\nno-new-privileges\n",
            "--config\n",
            "worker\nrun\n",
        ] {
            assert!(
                arguments.contains(required),
                "missing arguments: {required}"
            );
        }
        assert!(!arguments.contains("io.gpustack.mode"));
        assert!(!arguments.contains("io.gpustack.model-instance-id"));
    }

    #[test]
    fn run_script_rejects_an_unexpected_image_before_container_creation() {
        let fixture = RuntimeFixture::new(
            r#"#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail
if [[ "$1 $2" == 'image inspect' ]]; then
  printf 'sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc|%s|rust-worker|rust|["/usr/local/bin/gpustack-rs"]|["worker","run"]\n' "$FAKE_COMMIT"
elif [[ "$1" == 'run' ]]; then
  printf 'unsafe-run\n' >"$FAKE_CAPTURE"
  exit 91
else
  exit 90
fi
"#,
        );

        let output = fixture.run_script("run-worker-container.sh");

        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("image ID does not match"));
        assert!(!fixture.capture.exists());
    }

    #[test]
    fn stop_script_removes_only_a_complete_worker_identity() {
        let fixture = RuntimeFixture::new(
            r#"#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail
if [[ "$1 $2" == 'container ls' ]]; then
  printf '%s\n' "${FAKE_CONTAINER_ID:0:12}"
elif [[ "$1 $2" == 'container inspect' && "$3" == '--format' && "$4" == '{{.State.Running}}' ]]; then
  printf 'false\n'
elif [[ "$1 $2" == 'container inspect' ]]; then
  printf '%s|%s|/gpustack-rust-worker|rust-worker|rust|%s|gpustack-rust-worker|<no value>|<no value>|<no value>|<no value>\n' "$FAKE_CONTAINER_ID" "$FAKE_IMAGE_ID" "$FAKE_COMMIT"
elif [[ "$1 $2" == 'container stop' || "$1 $2" == 'container rm' ]]; then
  printf '%s\n' "$*" >>"$FAKE_CAPTURE"
else
  exit 90
fi
"#,
        );

        let output = fixture.run_script("stop-worker-container.sh");

        assert!(output.status.success(), "{}", output_text(&output));
        let operations = fs::read_to_string(&fixture.capture).expect("captured stop operations");
        assert!(operations.contains("container stop --time 30"));
        assert!(operations.contains("container rm"));
        assert!(!operations.contains("--force"));
    }

    #[test]
    fn stop_script_rejects_managed_runner_labels_before_mutation() {
        let fixture = RuntimeFixture::new(
            r#"#!/usr/bin/env bash
set -o errexit -o nounset -o pipefail
if [[ "$1 $2" == 'container ls' ]]; then
  printf '%s\n' "${FAKE_CONTAINER_ID:0:12}"
elif [[ "$1 $2" == 'container inspect' ]]; then
  printf '%s|%s|/gpustack-rust-worker|rust-worker|rust|%s|gpustack-rust-worker|managed|worker-uuid|7|spec-hash\n' "$FAKE_CONTAINER_ID" "$FAKE_IMAGE_ID" "$FAKE_COMMIT"
elif [[ "$1 $2" == 'container stop' || "$1 $2" == 'container rm' ]]; then
  printf 'unsafe-mutation\n' >"$FAKE_CAPTURE"
  exit 91
else
  exit 90
fi
"#,
        );

        let output = fixture.run_script("stop-worker-container.sh");

        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Managed Runner ownership"));
        assert!(!fixture.capture.exists());
    }
}
