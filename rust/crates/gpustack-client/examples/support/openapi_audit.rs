use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

const MAX_DOCUMENT_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("input must be a regular non-symlink file")]
    InvalidInput,
    #[error("input exceeds the 16 MiB audit limit")]
    InputTooLarge,
    #[error("failed to read the OpenAPI document: {0}")]
    Read(#[from] std::io::Error),
    #[error("failed to parse the OpenAPI document: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("document root must be a JSON object")]
    InvalidRoot,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditReport {
    pub schema_version: u8,
    pub compatible: bool,
    pub document: DocumentSummary,
    pub summary: AuditSummary,
    pub findings: Vec<Finding>,
}

impl AuditReport {
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        if self.compatible { 0 } else { 2 }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DocumentSummary {
    pub openapi: String,
    pub title: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuditSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub id: &'static str,
    pub severity: &'static str,
    pub area: &'static str,
    pub expected: String,
    pub actual: String,
    pub document_status: &'static str,
    pub runtime_status: &'static str,
}

pub fn audit_file(path: &Path) -> Result<AuditReport, AuditError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AuditError::InvalidInput);
    }
    if metadata.len() > MAX_DOCUMENT_BYTES {
        return Err(AuditError::InputTooLarge);
    }

    let document: Value = serde_json::from_slice(&fs::read(path)?)?;
    audit_document(&document)
}

pub fn audit_document(document: &Value) -> Result<AuditReport, AuditError> {
    if !document.is_object() {
        return Err(AuditError::InvalidRoot);
    }

    let mut findings = Vec::new();
    audit_registration(document, &mut findings);
    audit_worker_liveness(document, &mut findings);
    audit_control_resources(document, &mut findings);
    audit_watch(document, &mut findings);
    audit_inference(document, &mut findings);
    audit_common_shapes(document, &mut findings);

    let passed = findings
        .iter()
        .filter(|finding| finding.document_status == "pass")
        .count();
    let failed = findings.len() - passed;

    Ok(AuditReport {
        schema_version: 1,
        compatible: failed == 0,
        document: DocumentSummary {
            openapi: string_at(document, &["openapi"]),
            title: string_at(document, &["info", "title"]),
            version: string_at(document, &["info", "version"]),
        },
        summary: AuditSummary {
            total: findings.len(),
            passed,
            failed,
        },
        findings,
    })
}

fn audit_registration(document: &Value, findings: &mut Vec<Finding>) {
    let registration_request = request_schema(document, "/v2/workers", "post");
    push_shape_finding(
        findings,
        document,
        "RS-C001",
        "blocker",
        "worker_registration_request",
        registration_request,
        ShapeExpectation {
            properties: &[
                "hostname",
                "ifname",
                "ip",
                "name",
                "port",
                "status",
                "worker_uuid",
            ],
            required: &[
                "hostname",
                "ifname",
                "ip",
                "name",
                "port",
                "status",
                "worker_uuid",
            ],
        },
    );

    let registration_response = response_schema(document, "/v2/workers", "post", "200");
    push_shape_finding(
        findings,
        document,
        "RS-C002",
        "blocker",
        "worker_registration_response",
        registration_response,
        ShapeExpectation {
            properties: &["id", "name", "token", "worker_uuid"],
            required: &["id", "name", "token", "worker_uuid"],
        },
    );

    let parameters = query_parameters(document, "/v2/workers", "get");
    push_set_finding(
        findings,
        "RS-C003",
        "blocker",
        "worker_identity_lookup",
        &["me", "page", "perPage", "watch"],
        &parameters,
    );
}

fn audit_worker_liveness(document: &Value, findings: &mut Vec<Finding>) {
    let heartbeat_body = request_schema(document, "/v2/worker-heartbeat", "post").is_some();
    let heartbeat_204 = has_response(document, "/v2/worker-heartbeat", "post", "204");
    push_boolean_finding(
        findings,
        "RS-C004",
        "blocker",
        "worker_heartbeat",
        "application/json request body and 204 response",
        format!("request_body={heartbeat_body},response_204={heartbeat_204}"),
        heartbeat_body && heartbeat_204,
    );

    let status_schema = request_schema(document, "/v2/worker-status", "post");
    let status_204 = has_response(document, "/v2/worker-status", "post", "204");
    let status_shape = schema_shape(document, status_schema);
    let expected = set(&["hostname", "ifname", "ip", "port", "status", "worker_uuid"]);
    let shape_ok = expected.is_subset(&status_shape.properties);
    push_boolean_finding(
        findings,
        "RS-C005",
        "blocker",
        "worker_status",
        "typed status body with hostname, ifname, ip, port, status, worker_uuid and 204 response",
        format!(
            "properties={},response_204={status_204}",
            joined(&status_shape.properties)
        ),
        shape_ok && status_204,
    );
}

fn audit_control_resources(document: &Value, findings: &mut Vec<Finding>) {
    let model_parameters = query_parameters(document, "/v2/models", "get");
    let instance_parameters = query_parameters(document, "/v2/model-instances", "get");
    let file_parameters = query_parameters(document, "/v2/model-files", "get");
    let filter_ok = set(&["page", "perPage", "watch"]).is_subset(&model_parameters)
        && set(&["model_id", "page", "perPage", "watch"]).is_subset(&instance_parameters)
        && set(&["page", "perPage", "watch", "worker_id"]).is_subset(&file_parameters);
    push_boolean_finding(
        findings,
        "RS-C006",
        "blocker",
        "control_resource_filters",
        "models(page,perPage,watch), model-instances(model_id,page,perPage,watch), model-files(page,perPage,watch,worker_id)",
        format!(
            "models={};model_instances={};model_files={}",
            joined(&model_parameters),
            joined(&instance_parameters),
            joined(&file_parameters)
        ),
        filter_ok,
    );

    push_shape_finding(
        findings,
        document,
        "RS-C007",
        "blocker",
        "model_instance_complete_update",
        request_schema(document, "/v2/model-instances/{id}", "put"),
        ShapeExpectation {
            properties: &[
                "last_restart_time",
                "model_id",
                "model_name",
                "name",
                "owner_principal_id",
                "restart_count",
                "source",
                "state",
                "state_message",
                "worker_id",
            ],
            required: &["model_name", "name", "source"],
        },
    );

    push_shape_finding(
        findings,
        document,
        "RS-C008",
        "blocker",
        "model_file_complete_update",
        request_schema(document, "/v2/model-files/{id}", "put"),
        ShapeExpectation {
            properties: &[
                "base_model",
                "cleanup_on_delete",
                "download_progress",
                "is_lora",
                "local_dir",
                "resolved_paths",
                "source",
                "state",
                "state_message",
                "worker_id",
            ],
            required: &[],
        },
    );
}

fn audit_watch(document: &Value, findings: &mut Vec<Finding>) {
    let descriptions = ["/v2/models", "/v2/model-instances", "/v2/model-files"]
        .iter()
        .filter_map(|path| query_parameter_description(document, path, "get", "watch"))
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let expected_tokens = [
        "1=created",
        "2=updated",
        "3=deleted",
        "4=unknown",
        "5=heartbeat",
    ];
    let recognized = expected_tokens
        .iter()
        .filter(|token| descriptions.contains(*token))
        .copied()
        .collect::<Vec<_>>();
    push_boolean_finding(
        findings,
        "RS-C009",
        "blocker",
        "watch_event_numbers",
        expected_tokens.join(","),
        if descriptions.is_empty() {
            "description=missing".to_owned()
        } else {
            format!(
                "recognized={}",
                if recognized.is_empty() {
                    "none".to_owned()
                } else {
                    recognized.join(",")
                }
            )
        },
        recognized.len() == expected_tokens.len(),
    );
}

fn audit_inference(document: &Value, findings: &mut Vec<Finding>) {
    let required = [
        ("get", "/v1/models"),
        ("post", "/v1/chat/completions"),
        ("post", "/v1/completions"),
    ];
    let actual = required
        .iter()
        .filter(|(method, path)| operation(document, path, method).is_some())
        .map(|(method, path)| format!("{} {path}", method.to_ascii_uppercase()))
        .collect::<Vec<_>>();
    push_boolean_finding(
        findings,
        "RS-C010",
        "high",
        "openai_routes",
        "GET /v1/models,POST /v1/chat/completions,POST /v1/completions",
        if actual.is_empty() {
            "missing".to_owned()
        } else {
            actual.join(",")
        },
        actual.len() == required.len(),
    );
}

fn audit_common_shapes(document: &Value, findings: &mut Vec<Finding>) {
    push_shape_finding(
        findings,
        document,
        "RS-C011",
        "high",
        "error_shape",
        component_schema(document, "ApiError"),
        ShapeExpectation {
            properties: &["code", "message", "reason"],
            required: &[],
        },
    );

    let paginated = component_schema(document, "PaginatedWorkers");
    let pagination = component_schema(document, "Pagination");
    let paginated_shape = schema_shape(document, paginated);
    let pagination_shape = schema_shape(document, pagination);
    let compatible = set(&["items", "pagination"]).is_subset(&paginated_shape.properties)
        && set(&["page", "perPage", "total", "totalPage"]).is_subset(&pagination_shape.properties);
    push_boolean_finding(
        findings,
        "RS-C012",
        "high",
        "pagination_shape",
        "items,pagination and page,perPage,total,totalPage",
        format!(
            "wrapper={};pagination={}",
            joined(&paginated_shape.properties),
            joined(&pagination_shape.properties)
        ),
        compatible,
    );
}

struct ShapeExpectation<'a> {
    properties: &'a [&'a str],
    required: &'a [&'a str],
}

fn push_shape_finding(
    findings: &mut Vec<Finding>,
    document: &Value,
    id: &'static str,
    severity: &'static str,
    area: &'static str,
    schema: Option<&Value>,
    expected: ShapeExpectation<'_>,
) {
    let shape = schema_shape(document, schema);
    let properties_ok = set(expected.properties).is_subset(&shape.properties);
    let required_ok = set(expected.required).is_subset(&shape.required);
    push_boolean_finding(
        findings,
        id,
        severity,
        area,
        format!(
            "properties={};required={}",
            expected.properties.join(","),
            expected.required.join(",")
        ),
        format!(
            "properties={};required={}",
            joined(&shape.properties),
            joined(&shape.required)
        ),
        properties_ok && required_ok,
    );
}

fn push_set_finding(
    findings: &mut Vec<Finding>,
    id: &'static str,
    severity: &'static str,
    area: &'static str,
    expected: &[&str],
    actual: &BTreeSet<String>,
) {
    let expected_set = set(expected);
    push_boolean_finding(
        findings,
        id,
        severity,
        area,
        joined(&expected_set),
        joined(actual),
        expected_set.is_subset(actual),
    );
}

fn push_boolean_finding(
    findings: &mut Vec<Finding>,
    id: &'static str,
    severity: &'static str,
    area: &'static str,
    expected: impl Into<String>,
    actual: impl Into<String>,
    pass: bool,
) {
    findings.push(Finding {
        id,
        severity,
        area,
        expected: expected.into(),
        actual: actual.into(),
        document_status: if pass { "pass" } else { "fail" },
        // A specification mismatch is not promoted to a runtime defect until a live wire probe proves it.
        runtime_status: "unverified",
    });
}

#[derive(Default)]
struct SchemaShape {
    properties: BTreeSet<String>,
    required: BTreeSet<String>,
}

fn schema_shape(document: &Value, schema: Option<&Value>) -> SchemaShape {
    let mut shape = SchemaShape::default();
    let mut visited = BTreeSet::new();
    if let Some(schema) = schema {
        collect_schema_shape(document, schema, &mut visited, &mut shape);
    }
    shape
}

fn collect_schema_shape(
    document: &Value,
    schema: &Value,
    visited: &mut BTreeSet<String>,
    shape: &mut SchemaShape,
) {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str)
        && visited.insert(reference.to_owned())
        && let Some(name) = reference.strip_prefix("#/components/schemas/")
        && let Some(target) = component_schema(document, name)
    {
        collect_schema_shape(document, target, visited, shape);
    }
    if let Some(all_of) = schema.get("allOf").and_then(Value::as_array) {
        for part in all_of {
            collect_schema_shape(document, part, visited, shape);
        }
    }
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        shape.properties.extend(properties.keys().cloned());
    }
    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        shape
            .required
            .extend(required.iter().filter_map(Value::as_str).map(str::to_owned));
    }
}

fn request_schema<'a>(document: &'a Value, path: &str, method: &str) -> Option<&'a Value> {
    operation(document, path, method)?
        .get("requestBody")?
        .get("content")?
        .get("application/json")?
        .get("schema")
}

fn response_schema<'a>(
    document: &'a Value,
    path: &str,
    method: &str,
    status: &str,
) -> Option<&'a Value> {
    operation(document, path, method)?
        .get("responses")?
        .get(status)?
        .get("content")?
        .get("application/json")?
        .get("schema")
}

fn has_response(document: &Value, path: &str, method: &str, status: &str) -> bool {
    operation(document, path, method)
        .and_then(|operation| operation.get("responses"))
        .and_then(|responses| responses.get(status))
        .is_some()
}

fn query_parameters(document: &Value, path: &str, method: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(path_item) = document.get("paths").and_then(|paths| paths.get(path)) else {
        return names;
    };
    for container in [
        path_item.get("parameters"),
        operation(document, path, method).and_then(|op| op.get("parameters")),
    ]
    .into_iter()
    .flatten()
    {
        let Some(parameters) = container.as_array() else {
            continue;
        };
        for parameter in parameters {
            if parameter.get("in").and_then(Value::as_str) == Some("query")
                && let Some(name) = parameter.get("name").and_then(Value::as_str)
            {
                names.insert(name.to_owned());
            }
        }
    }
    names
}

fn query_parameter_description<'a>(
    document: &'a Value,
    path: &str,
    method: &str,
    name: &str,
) -> Option<&'a str> {
    operation(document, path, method)?
        .get("parameters")?
        .as_array()?
        .iter()
        .find(|parameter| {
            parameter.get("in").and_then(Value::as_str) == Some("query")
                && parameter.get("name").and_then(Value::as_str) == Some(name)
        })?
        .get("description")?
        .as_str()
}

fn operation<'a>(document: &'a Value, path: &str, method: &str) -> Option<&'a Value> {
    document.get("paths")?.get(path)?.get(method)
}

fn component_schema<'a>(document: &'a Value, name: &str) -> Option<&'a Value> {
    document.get("components")?.get("schemas")?.get(name)
}

fn string_at(document: &Value, path: &[&str]) -> String {
    path.iter()
        .try_fold(document, |value, key| value.get(*key))
        .and_then(Value::as_str)
        .unwrap_or("missing")
        .to_owned()
}

fn set(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn joined(values: &BTreeSet<String>) -> String {
    if values.is_empty() {
        "missing".to_owned()
    } else {
        values.iter().cloned().collect::<Vec<_>>().join(",")
    }
}
