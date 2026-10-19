//! Build-time identity discovery for the `gpustack-rs` binary.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=GPUSTACK_GIT_COMMIT");
    println!("cargo:rerun-if-changed=../../../.git/HEAD");

    if let Some(reference) = git_output(&["symbolic-ref", "-q", "HEAD"]) {
        println!("cargo:rerun-if-changed=../../../.git/{reference}");
    }

    let commit = env::var("GPUSTACK_GIT_COMMIT")
        .ok()
        .filter(|value| is_safe_identity(value))
        .unwrap_or_else(git_identity);
    println!("cargo:rustc-env=GPUSTACK_GIT_COMMIT={commit}");

    if let Ok(target) = env::var("TARGET") {
        println!("cargo:rustc-env=GPUSTACK_BUILD_TARGET={target}");
    }
}

fn git_identity() -> String {
    let Some(mut commit) = git_output(&["rev-parse", "--short=12", "HEAD"]) else {
        return "unknown".to_owned();
    };
    let dirty = Command::new("git")
        .args(["status", "--porcelain=v1", "--untracked-files=normal"])
        .current_dir(repository_root())
        .output()
        .is_ok_and(|output| output.status.success() && !output.stdout.is_empty());
    if dirty {
        commit.push_str("-dirty");
    }
    commit
}

fn git_output(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository_root())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    Some(value.trim().to_owned())
}

fn is_safe_identity(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
