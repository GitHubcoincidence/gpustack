//! Managed-file-only reconciliation from desired-state triggers.

use std::collections::BTreeMap;
use std::future::Future;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;

use gpustack_client::http::Transport;
use gpustack_client::types::{
    ModelFilePublic, ModelFileState, ModelFileUpdate, ModelInstancePublic, ModelInstanceState,
    ModelInstanceUpdate,
};
use gpustack_client::{ClientError, SecretToken, ServerClient};
use tokio::sync::watch;

use crate::control::DesiredState;

use super::model_file::{LocalModelResolver, ResolveError, ResolvedModelFile};
use super::port::{ManagedPortAllocator, PortAllocationError, PortProbe};

const PUBLIC_MESSAGE_LIMIT: usize = 512;

pub trait ManagedApi: Send + Sync + 'static {
    fn get_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        instance_id: i64,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a;

    fn list_model_instances<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Vec<ModelInstancePublic>, ClientError>> + Send + 'a;

    fn update_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
        instance_id: i64,
        update: &'a ModelInstanceUpdate,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a;

    fn get_model_file<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        model_file_id: i64,
    ) -> impl Future<Output = Result<ModelFilePublic, ClientError>> + Send + 'a;

    fn update_model_file<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
        model_file_id: i64,
        update: &'a ModelFileUpdate,
    ) -> impl Future<Output = Result<ModelFilePublic, ClientError>> + Send + 'a;
}

impl<T> ManagedApi for ServerClient<T>
where
    T: Transport + 'static,
{
    fn get_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        instance_id: i64,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a {
        ServerClient::get_model_instance(self, worker_token, instance_id)
    }

    async fn list_model_instances(
        &self,
        worker_token: &SecretToken,
    ) -> Result<Vec<ModelInstancePublic>, ClientError> {
        ServerClient::list_model_instances(self, worker_token)
            .await
            .map(|page| page.items)
    }

    fn update_model_instance<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
        instance_id: i64,
        update: &'a ModelInstanceUpdate,
    ) -> impl Future<Output = Result<ModelInstancePublic, ClientError>> + Send + 'a {
        ServerClient::update_assigned_model_instance(
            self,
            worker_token,
            worker_id,
            instance_id,
            update,
        )
    }

    fn get_model_file<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        model_file_id: i64,
    ) -> impl Future<Output = Result<ModelFilePublic, ClientError>> + Send + 'a {
        ServerClient::get_model_file(self, worker_token, model_file_id)
    }

    fn update_model_file<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
        model_file_id: i64,
        update: &'a ModelFileUpdate,
    ) -> impl Future<Output = Result<ModelFilePublic, ClientError>> + Send + 'a {
        ServerClient::update_assigned_model_file(
            self,
            worker_token,
            worker_id,
            model_file_id,
            update,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ManagedTarget {
    ModelInstance(i64),
    ModelFile(i64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManagedOutcome {
    InstanceInitialized { port: u16, reused: bool },
    InstanceErrored { message: String },
    ModelFileReady { path_count: usize, size: u64 },
    ModelFileErrored { message: String },
    Noop { reason: &'static str },
    Failed { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedResult {
    pub target: ManagedTarget,
    pub observed_state: Option<String>,
    pub outcome: ManagedOutcome,
}

pub struct ManagedCoordinator<A, P> {
    api: Arc<A>,
    worker_token: SecretToken,
    worker_id: i64,
    worker_ip: IpAddr,
    resolver: LocalModelResolver,
    ports: ManagedPortAllocator<P>,
}

impl<A: ManagedApi, P: PortProbe> ManagedCoordinator<A, P> {
    pub fn new(
        api: A,
        worker_token: SecretToken,
        worker_id: i64,
        worker_ip: IpAddr,
        resolver: LocalModelResolver,
        ports: ManagedPortAllocator<P>,
    ) -> Self {
        Self {
            api: Arc::new(api),
            worker_token,
            worker_id,
            worker_ip,
            resolver,
            ports,
        }
    }

    pub async fn reconcile_snapshot(&self, desired: &DesiredState) -> Vec<ManagedResult> {
        let targets = desired
            .model_instances
            .iter()
            .map(|(id, instance)| {
                (
                    ManagedTarget::ModelInstance(*id),
                    Some(instance.state.0.clone()),
                )
            })
            .chain(desired.model_files.iter().map(|(id, model_file)| {
                (
                    ManagedTarget::ModelFile(*id),
                    Some(model_file.state.0.clone()),
                )
            }));
        self.reconcile_entries(targets).await
    }

    pub async fn reconcile_targets<I>(&self, targets: I) -> Vec<ManagedResult>
    where
        I: IntoIterator<Item = ManagedTarget>,
    {
        self.reconcile_entries(targets.into_iter().map(|target| (target, None)))
            .await
    }

    async fn reconcile_entries<I>(&self, targets: I) -> Vec<ManagedResult>
    where
        I: IntoIterator<Item = (ManagedTarget, Option<String>)>,
    {
        let unique = targets.into_iter().collect::<BTreeMap<_, _>>();
        let mut results = Vec::with_capacity(unique.len());
        for (target, observed_state) in unique {
            let outcome = match target {
                ManagedTarget::ModelInstance(id) => self.reconcile_instance(id).await,
                ManagedTarget::ModelFile(id) => self.reconcile_model_file(id).await,
            };
            results.push(ManagedResult {
                target,
                observed_state,
                outcome,
            });
        }
        results
    }

    pub async fn run<F>(
        &self,
        mut desired: watch::Receiver<DesiredState>,
        mut shutdown: watch::Receiver<bool>,
        mut observe: F,
    ) where
        F: FnMut(ManagedResult),
    {
        let mut last_revision = None;
        loop {
            if *shutdown.borrow() {
                return;
            }
            let snapshot = desired.borrow().clone();
            if last_revision != Some(snapshot.revision) {
                for result in self.reconcile_snapshot(&snapshot).await {
                    observe(result);
                }
                last_revision = Some(snapshot.revision);
            }

            tokio::select! {
                changed = desired.changed() => {
                    if changed.is_err() {
                        return;
                    }
                }
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return;
                    }
                }
            }
        }
    }

    async fn reconcile_instance(&self, id: i64) -> ManagedOutcome {
        let observed = match self.api.get_model_instance(&self.worker_token, id).await {
            Ok(instance) => instance,
            Err(error) => return failed(error),
        };
        if let Some(outcome) = self.instance_precondition(&observed) {
            return outcome;
        }

        let authoritative = match self.api.list_model_instances(&self.worker_token).await {
            Ok(instances) => instances,
            Err(error) => return failed(error),
        };
        let latest = match self.api.get_model_instance(&self.worker_token, id).await {
            Ok(instance) => instance,
            Err(error) => return failed(error),
        };
        if let Some(outcome) = self.instance_precondition(&latest) {
            return outcome;
        }
        if !same_instance_inputs(&observed, &latest) {
            return ManagedOutcome::Noop {
                reason: "ModelInstance inputs changed during refresh",
            };
        }

        match self
            .ports
            .assign(&latest, self.worker_id, self.worker_ip, &authoritative)
        {
            Ok(assignment) => {
                let mut update = ModelInstanceUpdate::from(&latest);
                update.state = ModelInstanceState("initializing".to_owned());
                update.port = Some(assignment.port);
                update.ports = Some(vec![assignment.port]);
                match self
                    .api
                    .update_model_instance(&self.worker_token, self.worker_id, id, &update)
                    .await
                {
                    Ok(_) => ManagedOutcome::InstanceInitialized {
                        port: assignment.port,
                        reused: assignment.reused,
                    },
                    Err(error) => failed(error),
                }
            }
            Err(error) => self.report_instance_port_error(id, &latest, error).await,
        }
    }

    async fn report_instance_port_error(
        &self,
        id: i64,
        latest: &ModelInstancePublic,
        error: PortAllocationError,
    ) -> ManagedOutcome {
        let message = public_message("Managed service port allocation failed", &error);
        let mut update = ModelInstanceUpdate::from(latest);
        update.state = ModelInstanceState("error".to_owned());
        update.state_message = Some(message.clone());
        match self
            .api
            .update_model_instance(&self.worker_token, self.worker_id, id, &update)
            .await
        {
            Ok(_) => ManagedOutcome::InstanceErrored { message },
            Err(error) => failed(error),
        }
    }

    fn instance_precondition(&self, instance: &ModelInstancePublic) -> Option<ManagedOutcome> {
        if instance.worker_id != Some(self.worker_id) {
            return Some(ManagedOutcome::Noop {
                reason: "ModelInstance is assigned to another Worker",
            });
        }
        if instance.state.0 != "scheduled" {
            return Some(ManagedOutcome::Noop {
                reason: "ModelInstance state is not scheduled",
            });
        }
        let worker_ip = instance
            .worker_ip
            .as_deref()
            .and_then(|value| value.parse::<IpAddr>().ok());
        if worker_ip != Some(self.worker_ip) {
            return Some(ManagedOutcome::Noop {
                reason: "ModelInstance Worker IP does not match this Worker",
            });
        }
        if !instance.source.is_local_path()
            || instance.backend.as_deref() != Some("vLLM")
            || instance.local_path.as_deref().is_none_or(str::is_empty)
            || instance.spans_workers()
            || instance.draft_model_source.is_some()
            || instance.mounted_loras.is_some()
            || instance
                .gpu_indexes
                .as_ref()
                .is_none_or(|indexes| indexes.len() != 1)
        {
            return Some(ManagedOutcome::Noop {
                reason: "ModelInstance is outside the R1 Managed scope",
            });
        }
        None
    }

    async fn reconcile_model_file(&self, id: i64) -> ManagedOutcome {
        let observed = match self.api.get_model_file(&self.worker_token, id).await {
            Ok(model_file) => model_file,
            Err(error) => return failed(error),
        };
        if let Some(outcome) = self.model_file_precondition(&observed) {
            return outcome;
        }

        let path = PathBuf::from(observed.local_path.as_deref().unwrap_or_default());
        let resolver = self.resolver.clone();
        let resolution = tokio::task::spawn_blocking(move || resolver.resolve(&path)).await;
        let resolution = match resolution {
            Ok(result) => result,
            Err(_) => {
                return ManagedOutcome::Failed {
                    message: "local model resolver task failed".to_owned(),
                };
            }
        };

        let latest = match self.api.get_model_file(&self.worker_token, id).await {
            Ok(model_file) => model_file,
            Err(error) => return failed(error),
        };
        if let Some(outcome) = self.model_file_precondition(&latest) {
            return outcome;
        }
        if !same_model_file_inputs(&observed, &latest) {
            return ManagedOutcome::Noop {
                reason: "ModelFile inputs changed during refresh",
            };
        }

        match resolution.and_then(resolved_wire_values) {
            Ok((resolved, paths)) => {
                let mut update = ModelFileUpdate::from(&latest);
                update.state = ModelFileState("ready".to_owned());
                update.state_message = None;
                update.download_progress = Some(100.0);
                update.resolved_paths = paths;
                update.size = Some(resolved.size as i64);
                match self
                    .api
                    .update_model_file(&self.worker_token, self.worker_id, id, &update)
                    .await
                {
                    Ok(_) => ManagedOutcome::ModelFileReady {
                        path_count: resolved.paths.len(),
                        size: resolved.size,
                    },
                    Err(error) => failed(error),
                }
            }
            Err(error) => self.report_model_file_error(id, &latest, error).await,
        }
    }

    async fn report_model_file_error(
        &self,
        id: i64,
        latest: &ModelFilePublic,
        error: ResolveError,
    ) -> ManagedOutcome {
        let message = public_message("Local model resolution failed", &error);
        let mut update = ModelFileUpdate::from(latest);
        update.state = ModelFileState("error".to_owned());
        update.state_message = Some(message.clone());
        match self
            .api
            .update_model_file(&self.worker_token, self.worker_id, id, &update)
            .await
        {
            Ok(_) => ManagedOutcome::ModelFileErrored { message },
            Err(error) => failed(error),
        }
    }

    fn model_file_precondition(&self, model_file: &ModelFilePublic) -> Option<ManagedOutcome> {
        if model_file.worker_id != Some(self.worker_id) {
            return Some(ManagedOutcome::Noop {
                reason: "ModelFile is assigned to another Worker",
            });
        }
        if model_file.state.0 != "downloading" {
            return Some(ManagedOutcome::Noop {
                reason: "ModelFile state is not downloading",
            });
        }
        if !model_file.source.is_local_path()
            || model_file.local_path.as_deref().is_none_or(str::is_empty)
            || model_file.is_lora
        {
            return Some(ManagedOutcome::Noop {
                reason: "ModelFile is outside the R1 Managed scope",
            });
        }
        None
    }
}

fn same_instance_inputs(left: &ModelInstancePublic, right: &ModelInstancePublic) -> bool {
    left.worker_id == right.worker_id
        && left.state == right.state
        && left.model_id == right.model_id
        && left.source == right.source
        && left.backend == right.backend
        && left.local_path == right.local_path
        && left.distributed_servers == right.distributed_servers
        && left.draft_model_source == right.draft_model_source
        && left.mounted_loras == right.mounted_loras
        && left.gpu_addresses == right.gpu_addresses
        && left.gpu_indexes == right.gpu_indexes
        && left.worker_name == right.worker_name
        && left.worker_ifname == right.worker_ifname
        && left.worker_ip == right.worker_ip
        && left.port == right.port
        && left.ports == right.ports
}

fn same_model_file_inputs(left: &ModelFilePublic, right: &ModelFilePublic) -> bool {
    left.worker_id == right.worker_id
        && left.state == right.state
        && left.source == right.source
        && left.local_path == right.local_path
        && left.is_lora == right.is_lora
        && left.base_model == right.base_model
}

fn resolved_wire_values(
    resolved: ResolvedModelFile,
) -> Result<(ResolvedModelFile, Vec<String>), ResolveError> {
    if resolved.size > i64::MAX as u64 {
        return Err(ResolveError::SizeOverflow);
    }
    let paths = resolved
        .paths
        .iter()
        .map(|path| {
            path.to_str()
                .map(str::to_owned)
                .ok_or(ResolveError::NonUtf8Path)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((resolved, paths))
}

fn public_message(prefix: &str, error: &impl std::fmt::Display) -> String {
    format!("{prefix}: {error}")
        .chars()
        .take(PUBLIC_MESSAGE_LIMIT)
        .collect()
}

fn failed(error: ClientError) -> ManagedOutcome {
    ManagedOutcome::Failed {
        message: public_message("Managed reconciliation request failed", &error),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::fs;
    use std::io;
    use std::net::SocketAddr;
    use std::ops::RangeInclusive;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use gpustack_client::types::Paginated;

    use super::*;

    #[derive(Clone)]
    struct FakeApi {
        state: Arc<Mutex<FakeState>>,
    }

    struct FakeState {
        instance: ModelInstancePublic,
        model_file: ModelFilePublic,
        listed_instances: Vec<ModelInstancePublic>,
        instance_reads: VecDeque<ModelInstancePublic>,
        model_file_reads: VecDeque<ModelFilePublic>,
        instance_updates: Vec<ModelInstanceUpdate>,
        model_file_updates: Vec<ModelFileUpdate>,
        get_instance_count: usize,
        get_model_file_count: usize,
    }

    impl FakeApi {
        fn new(instance: ModelInstancePublic, model_file: ModelFilePublic) -> Self {
            let listed_instances = vec![instance.clone()];
            Self {
                state: Arc::new(Mutex::new(FakeState {
                    instance,
                    model_file,
                    listed_instances,
                    instance_reads: VecDeque::new(),
                    model_file_reads: VecDeque::new(),
                    instance_updates: Vec::new(),
                    model_file_updates: Vec::new(),
                    get_instance_count: 0,
                    get_model_file_count: 0,
                })),
            }
        }
    }

    impl ManagedApi for FakeApi {
        async fn get_model_instance(
            &self,
            _worker_token: &SecretToken,
            _instance_id: i64,
        ) -> Result<ModelInstancePublic, ClientError> {
            let mut state = self.state.lock().expect("fake lock");
            state.get_instance_count += 1;
            Ok(state
                .instance_reads
                .pop_front()
                .unwrap_or_else(|| state.instance.clone()))
        }

        async fn list_model_instances(
            &self,
            _worker_token: &SecretToken,
        ) -> Result<Vec<ModelInstancePublic>, ClientError> {
            Ok(self
                .state
                .lock()
                .expect("fake lock")
                .listed_instances
                .clone())
        }

        async fn update_model_instance(
            &self,
            _worker_token: &SecretToken,
            _worker_id: i64,
            _instance_id: i64,
            update: &ModelInstanceUpdate,
        ) -> Result<ModelInstancePublic, ClientError> {
            let mut state = self.state.lock().expect("fake lock");
            state.instance_updates.push(update.clone());
            state.instance.state = update.state.clone();
            state.instance.state_message = update.state_message.clone();
            state.instance.port = update.port;
            state.instance.ports = update.ports.clone();
            Ok(state.instance.clone())
        }

        async fn get_model_file(
            &self,
            _worker_token: &SecretToken,
            _model_file_id: i64,
        ) -> Result<ModelFilePublic, ClientError> {
            let mut state = self.state.lock().expect("fake lock");
            state.get_model_file_count += 1;
            Ok(state
                .model_file_reads
                .pop_front()
                .unwrap_or_else(|| state.model_file.clone()))
        }

        async fn update_model_file(
            &self,
            _worker_token: &SecretToken,
            _worker_id: i64,
            _model_file_id: i64,
            update: &ModelFileUpdate,
        ) -> Result<ModelFilePublic, ClientError> {
            let mut state = self.state.lock().expect("fake lock");
            state.model_file_updates.push(update.clone());
            state.model_file.state = update.state.clone();
            state.model_file.state_message = update.state_message.clone();
            state.model_file.download_progress = update.download_progress;
            state.model_file.resolved_paths = update.resolved_paths.clone();
            state.model_file.size = update.size;
            Ok(state.model_file.clone())
        }
    }

    #[derive(Default)]
    struct FakeProbe;

    impl PortProbe for FakeProbe {
        fn is_available(&self, _address: SocketAddr) -> io::Result<bool> {
            Ok(true)
        }
    }

    fn fixtures() -> (ModelInstancePublic, ModelFilePublic) {
        let mut instance =
            serde_json::from_slice::<Paginated<ModelInstancePublic>>(include_bytes!(
                "../../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json"
            ))
            .expect("instance fixture")
            .items
            .remove(0);
        instance.worker_ip = Some("127.0.0.1".to_owned());
        let model_file = serde_json::from_slice::<Paginated<ModelFilePublic>>(include_bytes!(
            "../../../../fixtures/server-v2.2.3/model-files-full-sync.response.json"
        ))
        .expect("ModelFile fixture")
        .items
        .remove(0);
        (instance, model_file)
    }

    fn coordinator(
        api: FakeApi,
        root: PathBuf,
        range: RangeInclusive<u16>,
    ) -> ManagedCoordinator<FakeApi, FakeProbe> {
        ManagedCoordinator::new(
            api,
            SecretToken::new("worker-token").expect("token"),
            101,
            "127.0.0.1".parse().expect("IP"),
            LocalModelResolver::new([root]).expect("resolver"),
            ManagedPortAllocator::new(range, FakeProbe).expect("port allocator"),
        )
    }

    #[tokio::test]
    async fn deduplicates_and_idempotently_initializes_an_instance() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, model_file) = fixtures();
        instance.local_path = Some(root.path().to_string_lossy().into_owned());
        let api = FakeApi::new(instance, model_file);
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let first = worker
            .reconcile_targets([
                ManagedTarget::ModelInstance(301),
                ManagedTarget::ModelInstance(301),
            ])
            .await;
        let second = worker
            .reconcile_targets([ManagedTarget::ModelInstance(301)])
            .await;

        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].outcome,
            ManagedOutcome::InstanceInitialized {
                port: 40_000,
                reused: false
            }
        );
        assert!(matches!(second[0].outcome, ManagedOutcome::Noop { .. }));
        let state = api.state.lock().expect("fake lock");
        assert_eq!(state.instance_updates.len(), 1);
        assert_eq!(state.instance_updates[0].state.0, "initializing");
        assert_eq!(state.instance_updates[0].ports, Some(vec![40_000]));
        assert_eq!(state.get_instance_count, 3);
    }

    #[tokio::test]
    async fn initializes_an_instance_with_the_server_single_worker_envelope() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, model_file) = fixtures();
        instance.local_path = Some(root.path().to_string_lossy().into_owned());
        instance.distributed_servers = Some(serde_json::json!({
            "download_model_files": true,
            "mode": "initialize_later",
            "subordinate_workers": null
        }));
        let api = FakeApi::new(instance, model_file);
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let result = worker
            .reconcile_targets([ManagedTarget::ModelInstance(301)])
            .await;

        assert_eq!(
            result[0].outcome,
            ManagedOutcome::InstanceInitialized {
                port: 40_000,
                reused: false
            }
        );
        let state = api.state.lock().expect("fake lock");
        assert_eq!(state.instance_updates.len(), 1);
        assert_eq!(state.instance_updates[0].state.0, "initializing");
    }

    #[tokio::test]
    async fn rejects_an_instance_with_subordinate_workers() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, model_file) = fixtures();
        instance.local_path = Some(root.path().to_string_lossy().into_owned());
        instance.distributed_servers = Some(serde_json::json!({
            "download_model_files": true,
            "mode": "initialize_later",
            "subordinate_workers": [{"worker_id": 202}]
        }));
        let api = FakeApi::new(instance, model_file);
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let result = worker
            .reconcile_targets([ManagedTarget::ModelInstance(301)])
            .await;

        assert_eq!(
            result[0].outcome,
            ManagedOutcome::Noop {
                reason: "ModelInstance is outside the R1 Managed scope"
            }
        );
        assert!(
            api.state
                .lock()
                .expect("fake lock")
                .instance_updates
                .is_empty()
        );
    }

    #[tokio::test]
    async fn stops_when_instance_assignment_changes_during_refresh() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, model_file) = fixtures();
        instance.local_path = Some(root.path().to_string_lossy().into_owned());
        let mut moved = instance.clone();
        moved.worker_id = Some(202);
        let api = FakeApi::new(instance.clone(), model_file);
        {
            let mut state = api.state.lock().expect("fake lock");
            state.instance_reads = VecDeque::from([instance, moved]);
        }
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let result = worker
            .reconcile_targets([ManagedTarget::ModelInstance(301)])
            .await;

        assert!(matches!(result[0].outcome, ManagedOutcome::Noop { .. }));
        assert!(
            api.state
                .lock()
                .expect("fake lock")
                .instance_updates
                .is_empty()
        );
    }

    #[tokio::test]
    async fn reports_port_exhaustion_as_an_instance_error() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, model_file) = fixtures();
        instance.local_path = Some(root.path().to_string_lossy().into_owned());
        let mut other = instance.clone();
        other.id = 302;
        other.port = Some(40_000);
        other.ports = Some(vec![40_000]);
        let api = FakeApi::new(instance.clone(), model_file);
        api.state.lock().expect("fake lock").listed_instances = vec![instance, other];
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_000);

        let result = worker
            .reconcile_targets([ManagedTarget::ModelInstance(301)])
            .await;

        let ManagedOutcome::InstanceErrored { message } = &result[0].outcome else {
            panic!("expected ModelInstance error")
        };
        assert!(message.contains("no Managed service port is available"));
        let state = api.state.lock().expect("fake lock");
        assert_eq!(state.instance_updates.len(), 1);
        assert_eq!(state.instance_updates[0].state.0, "error");
        assert_eq!(state.instance_updates[0].port, None);
    }

    #[tokio::test]
    async fn resolves_a_model_file_and_writes_ready_once() {
        let root = tempfile::tempdir().expect("model root");
        let first = root.path().join("model-00001-of-00002.safetensors");
        let second = root.path().join("model-00002-of-00002.safetensors");
        fs::write(&first, b"123").expect("first shard");
        fs::write(&second, b"4567").expect("second shard");
        let (instance, mut model_file) = fixtures();
        model_file.local_path = Some(first.to_string_lossy().into_owned());
        let api = FakeApi::new(instance, model_file);
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let first_result = worker
            .reconcile_targets([ManagedTarget::ModelFile(401), ManagedTarget::ModelFile(401)])
            .await;
        let second_result = worker
            .reconcile_targets([ManagedTarget::ModelFile(401)])
            .await;

        assert_eq!(
            first_result[0].outcome,
            ManagedOutcome::ModelFileReady {
                path_count: 2,
                size: 7
            }
        );
        assert!(matches!(
            second_result[0].outcome,
            ManagedOutcome::Noop { .. }
        ));
        let state = api.state.lock().expect("fake lock");
        assert_eq!(state.model_file_updates.len(), 1);
        assert_eq!(state.model_file_updates[0].state.0, "ready");
        assert_eq!(state.model_file_updates[0].download_progress, Some(100.0));
        assert_eq!(state.model_file_updates[0].size, Some(7));
        assert_eq!(state.get_model_file_count, 3);
    }

    #[tokio::test]
    async fn reports_a_redacted_bounded_model_file_error() {
        let root = tempfile::tempdir().expect("model root");
        let missing = root.path().join("secret-model-name-that-must-not-leak");
        let (instance, mut model_file) = fixtures();
        model_file.local_path = Some(missing.to_string_lossy().into_owned());
        let api = FakeApi::new(instance, model_file);
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let result = worker
            .reconcile_targets([ManagedTarget::ModelFile(401)])
            .await;

        let ManagedOutcome::ModelFileErrored { message } = &result[0].outcome else {
            panic!("expected ModelFile error")
        };
        assert!(message.len() <= PUBLIC_MESSAGE_LIMIT);
        assert!(!message.contains("secret-model-name"));
        let state = api.state.lock().expect("fake lock");
        assert_eq!(state.model_file_updates.len(), 1);
        assert_eq!(state.model_file_updates[0].state.0, "error");
    }

    #[tokio::test]
    async fn stops_when_model_file_assignment_changes_during_resolution() {
        let root = tempfile::tempdir().expect("model root");
        let model = root.path().join("model.safetensors");
        fs::write(&model, b"weights").expect("model file");
        let (instance, mut model_file) = fixtures();
        model_file.local_path = Some(model.to_string_lossy().into_owned());
        let mut moved = model_file.clone();
        moved.worker_id = Some(202);
        let api = FakeApi::new(instance, model_file.clone());
        api.state.lock().expect("fake lock").model_file_reads = VecDeque::from([model_file, moved]);
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);

        let result = worker
            .reconcile_targets([ManagedTarget::ModelFile(401)])
            .await;

        assert!(matches!(result[0].outcome, ManagedOutcome::Noop { .. }));
        assert!(
            api.state
                .lock()
                .expect("fake lock")
                .model_file_updates
                .is_empty()
        );
    }

    #[tokio::test]
    async fn starting_and_ready_resources_never_write() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, mut model_file) = fixtures();
        instance.state = ModelInstanceState("starting".to_owned());
        model_file.state = ModelFileState("ready".to_owned());
        let api = FakeApi::new(instance.clone(), model_file.clone());
        let worker = coordinator(api.clone(), root.path().to_path_buf(), 40_000..=40_003);
        let desired = DesiredState {
            models: BTreeMap::new(),
            model_instances: BTreeMap::from([(instance.id, instance)]),
            model_files: BTreeMap::from([(model_file.id, model_file)]),
            revision: 1,
        };

        let results = worker.reconcile_snapshot(&desired).await;

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].observed_state.as_deref(), Some("starting"));
        assert_eq!(results[1].observed_state.as_deref(), Some("ready"));
        assert!(
            results
                .iter()
                .all(|result| matches!(result.outcome, ManagedOutcome::Noop { .. }))
        );
        let state = api.state.lock().expect("fake lock");
        assert!(state.instance_updates.is_empty());
        assert!(state.model_file_updates.is_empty());
    }

    #[tokio::test]
    async fn run_observes_desired_state_and_stops_on_shutdown() {
        let root = tempfile::tempdir().expect("model root");
        let (mut instance, mut model_file) = fixtures();
        instance.state = ModelInstanceState("starting".to_owned());
        model_file.state = ModelFileState("ready".to_owned());
        let api = FakeApi::new(instance.clone(), model_file.clone());
        let worker = coordinator(api, root.path().to_path_buf(), 40_000..=40_003);
        let desired = DesiredState {
            models: BTreeMap::new(),
            model_instances: BTreeMap::from([(instance.id, instance)]),
            model_files: BTreeMap::from([(model_file.id, model_file)]),
            revision: 7,
        };
        let (_desired_tx, desired_rx) = watch::channel(desired);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (result_tx, mut result_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            worker
                .run(desired_rx, shutdown_rx, |result| {
                    let _ = result_tx.send(result);
                })
                .await;
        });

        let first = tokio::time::timeout(Duration::from_secs(1), result_rx.recv())
            .await
            .expect("first result timeout")
            .expect("first result");
        let second = tokio::time::timeout(Duration::from_secs(1), result_rx.recv())
            .await
            .expect("second result timeout")
            .expect("second result");
        shutdown_tx.send_replace(true);
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("coordinator shutdown timeout")
            .expect("coordinator task");

        assert_eq!(first.observed_state.as_deref(), Some("starting"));
        assert_eq!(second.observed_state.as_deref(), Some("ready"));
        assert!(matches!(first.outcome, ManagedOutcome::Noop { .. }));
        assert!(matches!(second.outcome, ManagedOutcome::Noop { .. }));
    }
}
