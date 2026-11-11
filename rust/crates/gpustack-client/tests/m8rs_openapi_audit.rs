use std::fs;
use std::path::{Path, PathBuf};

#[path = "../examples/support/openapi_audit.rs"]
mod openapi_audit;

fn rust_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> PathBuf {
    rust_root().join("fixtures/m8rs").join(name)
}

#[test]
fn compatible_contract_passes_every_document_gate() {
    let report = openapi_audit::audit_file(&fixture("compatible-openapi.json"))
        .expect("audit compatible fixture");

    assert!(report.compatible);
    assert_eq!(report.exit_code(), 0);
    assert_eq!(report.summary.total, 12);
    assert_eq!(report.summary.passed, 12);
    assert_eq!(report.summary.failed, 0);
    assert!(
        report
            .findings
            .iter()
            .all(|finding| finding.runtime_status == "unverified")
    );
}

#[test]
fn sanitized_rust_server_shape_reports_expected_blockers() {
    let report = openapi_audit::audit_file(&fixture("incompatible-openapi.json"))
        .expect("audit incompatible fixture");
    let failed = report
        .findings
        .iter()
        .filter(|finding| finding.document_status == "fail")
        .map(|finding| finding.id)
        .collect::<Vec<_>>();

    assert!(!report.compatible);
    assert_eq!(report.exit_code(), 2);
    for expected in [
        "RS-C001", "RS-C002", "RS-C003", "RS-C004", "RS-C005", "RS-C006", "RS-C007", "RS-C008",
        "RS-C009", "RS-C010",
    ] {
        assert!(failed.contains(&expected), "missing finding {expected}");
    }
    assert!(!failed.contains(&"RS-C011"));
    assert!(!failed.contains(&"RS-C012"));
}

#[test]
fn report_never_copies_unselected_openapi_examples() {
    let source = fs::read_to_string(fixture("compatible-openapi.json")).expect("fixture");
    let mut document: serde_json::Value = serde_json::from_str(&source).expect("fixture JSON");
    document["x-sensitive-example"] = serde_json::json!({
        "authorization": "Bearer should-never-appear",
        "token": "gpustack_example_secret"
    });
    document["paths"]["/v2/models"]["get"]["parameters"][2]["description"] = serde_json::json!(
        "1=CREATED, 2=UPDATED, 3=DELETED, 4=UNKNOWN, 5=HEARTBEAT; \
             Authorization: Bearer description-secret"
    );

    let report = openapi_audit::audit_document(&document).expect("audit document");
    let output = serde_json::to_string(&report).expect("serialize report");

    assert!(!output.contains("should-never-appear"));
    assert!(!output.contains("gpustack_example_secret"));
    assert!(!output.contains("description-secret"));
    assert!(!output.contains("authorization"));
}

#[test]
fn invalid_document_root_is_rejected() {
    let error =
        openapi_audit::audit_document(&serde_json::json!([])).expect_err("array root must fail");

    assert!(error.to_string().contains("root must be a JSON object"));
}

#[cfg(unix)]
#[test]
fn symlink_input_is_rejected_before_reading() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("temporary directory");
    let target = directory.path().join("openapi.json");
    let link = directory.path().join("openapi-link.json");
    fs::write(&target, "{}").expect("write target");
    symlink(&target, &link).expect("create symlink");

    let error = openapi_audit::audit_file(&link).expect_err("symlink must fail");
    assert!(error.to_string().contains("non-symlink"));
}
