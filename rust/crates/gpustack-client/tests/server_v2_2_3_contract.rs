//! Frozen GPUStack v2.2.3 fixture compatibility tests.

use gpustack_client::types::{
    ModelFilePublic, ModelFileUpdate, ModelInstancePublic, ModelInstanceUpdate, ModelPublic,
    Paginated, ServerVersion, WorkerCreate, WorkerPublic, WorkerRegistrationPublic,
    WorkerStatusReport,
};
use serde_json::Value;

const VERSION: &str = include_str!("../../../fixtures/server-v2.2.3/live-version.response.json");
const UNAUTHENTICATED: &str =
    include_str!("../../../fixtures/server-v2.2.3/unauthenticated.response.json");
const WORKER_REGISTER_REQUEST: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-register.request.json");
const WORKER_REGISTER_RESPONSE: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-register.response.json");
const WORKERS_ME: &str = include_str!("../../../fixtures/server-v2.2.3/workers-me.response.json");
const WORKER_MAINTENANCE_ME: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-maintenance-me.response.json");
const WORKER_HEARTBEAT: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-heartbeat.request.json");
const WORKER_STATUS: &str =
    include_str!("../../../fixtures/server-v2.2.3/worker-status.request.json");
const MODELS_FULL_SYNC: &str =
    include_str!("../../../fixtures/server-v2.2.3/models-full-sync.response.json");
const MODEL_INSTANCES_FULL_SYNC: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json");
const MODEL_INSTANCE_UPDATE: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-instance-update.request.json");
const MODEL_FILES_FULL_SYNC: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-files-full-sync.response.json");
const MODEL_FILE_UPDATE: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-file-update.request.json");
const MODEL_WATCH: &str = include_str!("../../../fixtures/server-v2.2.3/model-watch.ndjson");
const MODEL_INSTANCE_WATCH: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-instance-watch.ndjson");
const MODEL_FILE_WATCH: &str =
    include_str!("../../../fixtures/server-v2.2.3/model-file-watch.ndjson");

fn parse_json(name: &str, raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|error| panic!("invalid {name}: {error}"))
}

#[test]
fn json_fixtures_are_well_formed_and_redacted() {
    let fixtures = [
        ("version", VERSION),
        ("unauthenticated", UNAUTHENTICATED),
        ("worker register request", WORKER_REGISTER_REQUEST),
        ("worker register response", WORKER_REGISTER_RESPONSE),
        ("workers me", WORKERS_ME),
        ("maintenance worker me", WORKER_MAINTENANCE_ME),
        ("worker heartbeat", WORKER_HEARTBEAT),
        ("worker status", WORKER_STATUS),
        ("models full sync", MODELS_FULL_SYNC),
        ("model instances full sync", MODEL_INSTANCES_FULL_SYNC),
        ("model instance update", MODEL_INSTANCE_UPDATE),
        ("model files full sync", MODEL_FILES_FULL_SYNC),
        ("model file update", MODEL_FILE_UPDATE),
    ];

    for (name, raw) in fixtures {
        parse_json(name, raw);
        assert!(
            !raw.contains("Authorization"),
            "{name} contains an auth header"
        );
        assert!(
            !raw.contains("gpustack_"),
            "{name} contains an API-key-shaped value"
        );
    }
}

#[test]
fn live_fixtures_pin_the_server_identity_and_error_shape() {
    let version = parse_json("version", VERSION);
    assert_eq!(version["version"], "v2.2.3");
    assert_eq!(version["git_commit"], "1cfcbbf");

    let unauthorized = parse_json("unauthenticated", UNAUTHENTICATED);
    assert_eq!(unauthorized["code"], 401);
    assert_eq!(unauthorized["reason"], "Unauthorized");
    assert_eq!(
        unauthorized["message"],
        "Invalid authentication credentials"
    );
}

#[test]
fn full_sync_and_worker_fixtures_pin_control_plane_semantics() {
    let request = parse_json("worker register request", WORKER_REGISTER_REQUEST);
    let register = parse_json("worker register response", WORKER_REGISTER_RESPONSE);
    assert_eq!(register["worker_version"], "v2.2.3");
    assert_eq!(register["token"], "<redacted-worker-token>");
    assert_ne!(register["worker_uuid"], request["worker_uuid"]);

    let status = parse_json("worker status", WORKER_STATUS);
    assert_eq!(status["worker_uuid"], register["worker_uuid"]);

    let heartbeat = parse_json("worker heartbeat", WORKER_HEARTBEAT);
    assert_eq!(heartbeat, Value::Object(Default::default()));

    for (name, raw) in [
        ("models", MODELS_FULL_SYNC),
        ("model instances", MODEL_INSTANCES_FULL_SYNC),
        ("model files", MODEL_FILES_FULL_SYNC),
    ] {
        let response = parse_json(name, raw);
        assert_eq!(response["pagination"]["page"], -1);
        assert_eq!(response["pagination"]["perPage"], 100);
        assert_eq!(response["items"].as_array().map(Vec::len), Some(1));
    }
}

#[test]
fn watch_fixtures_use_numeric_event_types_and_id_only_deletes() {
    let model_events: Vec<Value> = MODEL_WATCH
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| parse_json("model watch event", line))
        .collect();
    assert_eq!(model_events.len(), 1);
    assert_eq!(model_events[0]["type"], 1);
    assert_eq!(model_events[0]["id"], 201);

    let instance_events: Vec<Value> = MODEL_INSTANCE_WATCH
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| parse_json("model instance watch event", line))
        .collect();
    assert_eq!(instance_events.len(), 2);
    assert_eq!(instance_events[0]["type"], 1);
    assert_eq!(instance_events[1]["type"], 3);
    assert_eq!(instance_events[1]["data"], serde_json::json!({ "id": 301 }));

    let model_file_events: Vec<Value> = MODEL_FILE_WATCH
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| parse_json("model file watch event", line))
        .collect();
    assert_eq!(model_file_events.len(), 3);
    assert_eq!(model_file_events[0]["type"], 1);
    assert_eq!(model_file_events[0]["data"]["state"], "downloading");
    assert_eq!(model_file_events[1]["type"], 2);
    assert_eq!(model_file_events[1]["data"]["state"], "ready");
    assert_eq!(model_file_events[2]["type"], 3);
    assert_eq!(
        model_file_events[2]["data"],
        serde_json::json!({ "id": 401 })
    );
}

#[test]
fn typed_requests_serialize_to_the_frozen_wire_shape() {
    let worker: WorkerCreate =
        serde_json::from_str(WORKER_REGISTER_REQUEST).expect("typed WorkerCreate");
    assert_eq!(
        serde_json::to_value(worker).expect("serialize WorkerCreate"),
        parse_json("worker register request", WORKER_REGISTER_REQUEST)
    );

    let status: WorkerStatusReport =
        serde_json::from_str(WORKER_STATUS).expect("typed WorkerStatusReport");
    assert_eq!(
        serde_json::to_value(status).expect("serialize WorkerStatusReport"),
        parse_json("worker status request", WORKER_STATUS)
    );

    let update: ModelInstanceUpdate =
        serde_json::from_str(MODEL_INSTANCE_UPDATE).expect("typed ModelInstanceUpdate");
    assert_eq!(
        serde_json::to_value(update).expect("serialize ModelInstanceUpdate"),
        parse_json("model instance update", MODEL_INSTANCE_UPDATE)
    );

    let model_file_update: ModelFileUpdate =
        serde_json::from_str(MODEL_FILE_UPDATE).expect("typed ModelFileUpdate");
    for state in ["downloading", "ready", "error"] {
        let mut candidate = model_file_update.clone();
        candidate.state.0 = state.to_owned();
        candidate
            .validate_for_worker(101)
            .unwrap_or_else(|error| panic!("valid ModelFile state {state}: {error}"));
    }
    assert_eq!(
        serde_json::to_value(model_file_update).expect("serialize ModelFileUpdate"),
        parse_json("model file update", MODEL_FILE_UPDATE)
    );
}

#[test]
fn typed_responses_are_tolerant_and_preserve_the_update_contract() {
    let version: ServerVersion = serde_json::from_str(VERSION).expect("typed ServerVersion");
    version.verify_baseline().expect("frozen Server identity");

    let mut registration_value = parse_json("registration", WORKER_REGISTER_RESPONSE);
    registration_value["future_server_field"] = serde_json::json!({ "enabled": true });
    let registration: WorkerRegistrationPublic =
        serde_json::from_value(registration_value).expect("tolerant registration response");
    assert_eq!(registration.id, 101);
    assert!(!format!("{registration:?}").contains("<redacted-worker-token>"));

    let models: Paginated<ModelPublic> =
        serde_json::from_str(MODELS_FULL_SYNC).expect("typed Models response");
    assert_eq!(models.pagination.page, -1);
    assert_eq!(models.items[0].backend.as_deref(), Some("vLLM"));
    assert_eq!(models.items[0].restart_on_error, Some(true));

    let workers: Paginated<WorkerPublic> =
        serde_json::from_str(WORKER_MAINTENANCE_ME).expect("typed maintenance Worker response");
    assert_eq!(workers.items[0].cluster_id, None);
    let maintenance = workers.items[0]
        .maintenance
        .as_ref()
        .expect("maintenance profile");
    assert!(maintenance.enabled);

    let instances: Paginated<ModelInstancePublic> =
        serde_json::from_str(MODEL_INSTANCES_FULL_SYNC).expect("typed ModelInstances response");
    let update = ModelInstanceUpdate::from(&instances.items[0]);
    assert_eq!(
        serde_json::to_value(update).expect("serialize update"),
        parse_json("model instance update", MODEL_INSTANCE_UPDATE)
    );

    let mut model_files_value = parse_json("model files", MODEL_FILES_FULL_SYNC);
    model_files_value["items"][0]["future_server_field"] = serde_json::json!({ "enabled": true });
    let model_files: Paginated<ModelFilePublic> =
        serde_json::from_value(model_files_value).expect("typed ModelFiles response");
    assert!(
        model_files.items[0]
            .extensions
            .contains_key("future_server_field")
    );

    let expected_update: ModelFileUpdate =
        serde_json::from_str(MODEL_FILE_UPDATE).expect("typed ModelFileUpdate");
    let mut model_file_update = ModelFileUpdate::from(&model_files.items[0]);
    model_file_update.download_progress = expected_update.download_progress;
    model_file_update.resolved_paths = expected_update.resolved_paths;
    model_file_update.state = expected_update.state;
    assert_eq!(
        serde_json::to_value(model_file_update).expect("serialize ModelFile update"),
        parse_json("model file update", MODEL_FILE_UPDATE)
    );
}

#[test]
fn single_worker_distributed_envelope_is_not_cross_worker_inference() {
    let instances: Paginated<ModelInstancePublic> =
        serde_json::from_str(MODEL_INSTANCES_FULL_SYNC).expect("typed ModelInstances response");
    let mut instance = instances.items[0].clone();
    instance.distributed_servers = Some(serde_json::json!({
        "download_model_files": true,
        "mode": "initialize_later",
        "subordinate_workers": null
    }));

    assert!(!instance.spans_workers());
    ModelInstanceUpdate::from(&instance)
        .validate_for_worker(101)
        .expect("single-Worker Server envelope");

    instance.distributed_servers = Some(serde_json::json!({
        "download_model_files": true,
        "mode": "initialize_later",
        "subordinate_workers": [{"worker_id": 202}]
    }));
    assert!(instance.spans_workers());
    assert!(
        ModelInstanceUpdate::from(&instance)
            .validate_for_worker(101)
            .is_err()
    );
}
