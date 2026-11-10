use std::collections::BTreeMap;
use std::path::PathBuf;

use gpustack_runtime::docker::ContainerRestartPolicy;
use gpustack_worker::workload::{
    LABEL_COMPONENT, LABEL_IMPLEMENTATION, LABEL_MODE, LABEL_MODEL_INSTANCE_ID, LABEL_OWNER_ID,
    LABEL_SCHEMA_VERSION, LABEL_SPEC_HASH, LABEL_WORKER_UUID, OwnershipError, VllmDtype,
    VllmOptions, WorkloadBuildRequest, WorkloadGpu, WorkloadMode, WorkloadSpecV2,
};

const RUNNER: &str = "quay.io/gpustack/runner@sha256:f9c58b163ec7eb537404c7d268f3d96e101cbf19e05acb8df6b7637f9e64c3af";
const MODEL_PATH: &str =
    "/home/wujiaxing2/flowscale-system-validation/models/Qwen2.5-0.5B-Instruct";
const CACHE_PATH: &str = "/home/wujiaxing2/.local/state/gpustack-rust-worker/1/managed/cache/48";
const GPU_0: &str = "GPU-52e4ed55-c51a-56cc-7478-44e056a0b3b3";
const GPU_1: &str = "GPU-b1f89e65-e9c4-26f5-a306-8a520df07708";

fn managed_request(gpus: Vec<WorkloadGpu>) -> WorkloadBuildRequest {
    WorkloadBuildRequest {
        mode: WorkloadMode::Managed,
        owner_id: "rust-m3d-r760xa".to_owned(),
        deployment_id: "48".to_owned(),
        managed_worker_uuid: Some("11111111-2222-4333-8444-555555555555".to_owned()),
        model_path: PathBuf::from(MODEL_PATH),
        cache_path: PathBuf::from(CACHE_PATH),
        served_model_name: "m7f-qwen25-05b-gpu0-gpu1".to_owned(),
        requested_image: RUNNER.to_owned(),
        resolved_image_digest: RUNNER.to_owned(),
        tensor_parallel_size: gpus.len() as u32,
        gpus,
        service_host: "10.213.80.90".to_owned(),
        service_port: 40000,
        container_user: "1004:1004".to_owned(),
        vllm: VllmOptions {
            max_model_len: Some(2048),
            gpu_memory_utilization: Some(0.5),
            dtype: Some(VllmDtype::Auto),
            trust_remote_code: false,
            enforce_eager: true,
            extra_args: Vec::new(),
        },
        environment: BTreeMap::new(),
    }
}

fn gpu(uuid: &str, host_index: u32) -> WorkloadGpu {
    WorkloadGpu {
        uuid: uuid.to_owned(),
        host_index,
    }
}

#[test]
fn m7_single_gpu_managed_contract_is_stable() {
    let mut request = managed_request(vec![gpu(GPU_0, 0)]);
    request.deployment_id = "41".to_owned();
    request.served_model_name = "m7-single-gpu".to_owned();
    request.cache_path =
        PathBuf::from("/home/wujiaxing2/.local/state/gpustack-rust-worker/1/managed/cache/41");
    let spec = WorkloadSpecV2::build(request).expect("frozen single-GPU workload");
    let container = spec.container_spec();

    assert_eq!(
        spec.spec_hash,
        "c22d8080422c02c03dd3ddc10e6531e0add4b819c61b6ae06c4bce7f36e1a8ea"
    );
    assert_eq!(spec.tensor_parallel_size, 1);
    assert_eq!(spec.gpus, vec![gpu(GPU_0, 0)]);
    assert_eq!(container.image, RUNNER);
    assert_eq!(container.user.as_deref(), Some("1004:1004"));
    assert_eq!(container.network_mode, "host");
    assert_eq!(container.restart_policy, ContainerRestartPolicy::No);
    assert!(!container.privileged);
    assert!(!container.auto_remove);
    assert_eq!(container.security_options, ["no-new-privileges:true"]);
    assert_eq!(
        container
            .gpu_request
            .expect("single GPU request")
            .device_ids,
        [GPU_0]
    );
    assert!(
        spec.vllm_command()
            .windows(2)
            .any(|pair| pair == ["--tensor-parallel-size", "1"])
    );
    spec.verify_integrity().expect("single-GPU integrity");
}

#[test]
fn m7_dual_gpu_managed_contract_is_stable() {
    let spec = WorkloadSpecV2::build(managed_request(vec![gpu(GPU_0, 0), gpu(GPU_1, 1)]))
        .expect("frozen dual-GPU workload");
    let container = spec.container_spec();

    assert_eq!(
        spec.spec_hash,
        "8582284d89546a20ba3d4d96b4d9946eed1de105b897f3726983418f7b915b0f"
    );
    assert_eq!(spec.tensor_parallel_size, 2);
    assert_eq!(spec.gpus, vec![gpu(GPU_0, 0), gpu(GPU_1, 1)]);
    assert_eq!(
        container.gpu_request.expect("dual GPU request").device_ids,
        [GPU_0, GPU_1]
    );
    assert!(
        spec.vllm_command()
            .windows(2)
            .any(|pair| pair == ["--tensor-parallel-size", "2"])
    );
    assert_eq!(
        spec.labels.get(LABEL_IMPLEMENTATION).map(String::as_str),
        Some("rust")
    );
    assert_eq!(
        spec.labels.get(LABEL_SCHEMA_VERSION).map(String::as_str),
        Some("2")
    );
    assert_eq!(
        spec.labels.get(LABEL_MODE).map(String::as_str),
        Some("managed")
    );
    assert_eq!(
        spec.labels.get(LABEL_OWNER_ID).map(String::as_str),
        Some("rust-m3d-r760xa")
    );
    assert_eq!(
        spec.labels.get(LABEL_COMPONENT).map(String::as_str),
        Some("vllm")
    );
    assert_eq!(
        spec.labels.get(LABEL_WORKER_UUID).map(String::as_str),
        Some("11111111-2222-4333-8444-555555555555")
    );
    assert_eq!(
        spec.labels.get(LABEL_MODEL_INSTANCE_ID).map(String::as_str),
        Some("48")
    );
    assert_eq!(spec.labels.get(LABEL_SPEC_HASH), Some(&spec.spec_hash));
    spec.verify_integrity().expect("dual-GPU integrity");
}

#[test]
fn m7_workload_identity_drift_remains_fail_closed() {
    let spec = WorkloadSpecV2::build(managed_request(vec![gpu(GPU_0, 0), gpu(GPU_1, 1)]))
        .expect("frozen dual-GPU workload");

    let mut wrong_owner = spec.labels.clone();
    wrong_owner.insert(LABEL_OWNER_ID.to_owned(), "another-worker".to_owned());
    assert!(matches!(
        spec.verify_ownership(&wrong_owner),
        Err(OwnershipError::Mismatch { label, .. }) if label == LABEL_OWNER_ID
    ));

    let mut missing_hash = spec.labels.clone();
    missing_hash.remove(LABEL_SPEC_HASH);
    assert!(matches!(
        spec.verify_ownership(&missing_hash),
        Err(OwnershipError::Missing { label }) if label == LABEL_SPEC_HASH
    ));

    let mut reversed_gpu_order = spec.clone();
    reversed_gpu_order.gpus.reverse();
    assert!(reversed_gpu_order.verify_integrity().is_err());

    let mut mismatched_tp = spec;
    mismatched_tp.tensor_parallel_size = 1;
    assert!(mismatched_tp.verify_integrity().is_err());
}
