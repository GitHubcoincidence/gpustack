//! Resilient Worker control-plane loops and authoritative desired state.

use std::collections::BTreeMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use gpustack_client::http::Transport;
use gpustack_client::types::{
    ModelFilePublic, ModelInstancePublic, ModelPublic, WorkerStatusReport,
};
use gpustack_client::watch::{WatchConnection, WatchEvent};
use gpustack_client::{ClientError, SecretToken, ServerClient};
use serde::de::DeserializeOwned;
use thiserror::Error;
use tokio::sync::watch;
use tokio::time::{Instant, MissedTickBehavior};

const AUTHORITATIVE_FULL_SYNC_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DesiredState {
    pub models: BTreeMap<i64, ModelPublic>,
    pub model_instances: BTreeMap<i64, ModelInstancePublic>,
    pub model_files: BTreeMap<i64, ModelFilePublic>,
    pub revision: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ControlPlaneHealth {
    pub heartbeat_ok: bool,
    pub status_ok: bool,
    pub full_sync_ok: bool,
    pub watch_connected: bool,
    pub reconnect_attempts: u64,
}

#[derive(Debug, Clone)]
pub struct ControlLoopConfig {
    pub heartbeat_interval: Duration,
    pub status_interval: Duration,
    pub full_sync_interval: Duration,
    pub reconnect_initial: Duration,
    pub reconnect_max: Duration,
}

impl Default for ControlLoopConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(30),
            status_interval: Duration::from_secs(30),
            // Watch events accelerate convergence, but a filtered or lost delete
            // must still be discovered within an operational cleanup window.
            full_sync_interval: AUTHORITATIVE_FULL_SYNC_INTERVAL,
            reconnect_initial: Duration::from_secs(1),
            reconnect_max: Duration::from_secs(30),
        }
    }
}

impl ControlLoopConfig {
    pub fn validate(&self) -> Result<(), ControlLoopConfigError> {
        let intervals = [
            ("heartbeat_interval", self.heartbeat_interval),
            ("status_interval", self.status_interval),
            ("full_sync_interval", self.full_sync_interval),
            ("reconnect_initial", self.reconnect_initial),
            ("reconnect_max", self.reconnect_max),
        ];
        if let Some((field, _)) = intervals
            .into_iter()
            .find(|(_, duration)| duration.is_zero())
        {
            return Err(ControlLoopConfigError::ZeroDuration(field));
        }
        if self.reconnect_initial > self.reconnect_max {
            return Err(ControlLoopConfigError::InvalidReconnectRange);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlLoopConfigError {
    #[error("{0} must be greater than zero")]
    ZeroDuration(&'static str),
    #[error("reconnect_initial must not exceed reconnect_max")]
    InvalidReconnectRange,
}

pub trait EventStream<T>: Send {
    fn next_event(
        &mut self,
    ) -> impl Future<Output = Result<Option<WatchEvent<T>>, ClientError>> + Send;
}

impl<T> EventStream<T> for WatchConnection<T>
where
    T: DeserializeOwned + Send,
{
    async fn next_event(&mut self) -> Result<Option<WatchEvent<T>>, ClientError> {
        WatchConnection::next_event(self).await
    }
}

/// Operations required by the control loops, separated for offline state-machine tests.
pub trait ControlPlaneApi: Send + Sync + 'static {
    type ModelWatch: EventStream<ModelPublic>;
    type ModelInstanceWatch: EventStream<ModelInstancePublic>;
    type ModelFileWatch: EventStream<ModelFilePublic>;

    fn heartbeat<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + 'a;

    fn report_status<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        status: &'a WorkerStatusReport,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + 'a;

    fn list_models<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Vec<ModelPublic>, ClientError>> + Send + 'a;

    fn list_model_instances<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Vec<ModelInstancePublic>, ClientError>> + Send + 'a;

    fn list_model_files<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
    ) -> impl Future<Output = Result<Vec<ModelFilePublic>, ClientError>> + Send + 'a;

    fn watch_models<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Self::ModelWatch, ClientError>> + Send + 'a;

    fn watch_model_instances<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Self::ModelInstanceWatch, ClientError>> + Send + 'a;

    fn watch_model_files<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
    ) -> impl Future<Output = Result<Self::ModelFileWatch, ClientError>> + Send + 'a;
}

impl<T> ControlPlaneApi for ServerClient<T>
where
    T: Transport + 'static,
{
    type ModelWatch = WatchConnection<ModelPublic>;
    type ModelInstanceWatch = WatchConnection<ModelInstancePublic>;
    type ModelFileWatch = WatchConnection<ModelFilePublic>;

    fn heartbeat<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + 'a {
        ServerClient::heartbeat(self, worker_token)
    }

    fn report_status<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        status: &'a WorkerStatusReport,
    ) -> impl Future<Output = Result<(), ClientError>> + Send + 'a {
        ServerClient::report_status(self, worker_token, status)
    }

    async fn list_models(
        &self,
        worker_token: &SecretToken,
    ) -> Result<Vec<ModelPublic>, ClientError> {
        ServerClient::list_models(self, worker_token)
            .await
            .map(|page| page.items)
    }

    async fn list_model_instances(
        &self,
        worker_token: &SecretToken,
    ) -> Result<Vec<ModelInstancePublic>, ClientError> {
        ServerClient::list_model_instances(self, worker_token)
            .await
            .map(|page| page.items)
    }

    async fn list_model_files(
        &self,
        worker_token: &SecretToken,
        worker_id: i64,
    ) -> Result<Vec<ModelFilePublic>, ClientError> {
        ServerClient::list_model_files(self, worker_token, worker_id)
            .await
            .map(|page| page.items)
    }

    fn watch_models<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Self::ModelWatch, ClientError>> + Send + 'a {
        ServerClient::watch_models(self, worker_token)
    }

    fn watch_model_instances<'a>(
        &'a self,
        worker_token: &'a SecretToken,
    ) -> impl Future<Output = Result<Self::ModelInstanceWatch, ClientError>> + Send + 'a {
        ServerClient::watch_model_instances(self, worker_token)
    }

    fn watch_model_files<'a>(
        &'a self,
        worker_token: &'a SecretToken,
        worker_id: i64,
    ) -> impl Future<Output = Result<Self::ModelFileWatch, ClientError>> + Send + 'a {
        ServerClient::watch_model_files(self, worker_token, worker_id)
    }
}

pub struct ControlPlaneHandle {
    desired_state: watch::Receiver<DesiredState>,
    health: watch::Receiver<ControlPlaneHealth>,
}

impl ControlPlaneHandle {
    pub fn desired_state(&self) -> watch::Receiver<DesiredState> {
        self.desired_state.clone()
    }

    pub fn health(&self) -> watch::Receiver<ControlPlaneHealth> {
        self.health.clone()
    }
}

pub struct WorkerControlPlane<A> {
    api: Arc<A>,
    worker_token: SecretToken,
    worker_id: i64,
    config: ControlLoopConfig,
    desired_tx: watch::Sender<DesiredState>,
    health_tx: watch::Sender<ControlPlaneHealth>,
}

impl<A: ControlPlaneApi> WorkerControlPlane<A> {
    pub fn new(
        api: A,
        worker_token: SecretToken,
        worker_id: i64,
        config: ControlLoopConfig,
    ) -> Result<(Self, ControlPlaneHandle), ControlLoopConfigError> {
        config.validate()?;
        let (desired_tx, desired_state) = watch::channel(DesiredState::default());
        let (health_tx, health) = watch::channel(ControlPlaneHealth::default());
        Ok((
            Self {
                api: Arc::new(api),
                worker_token,
                worker_id,
                config,
                desired_tx,
                health_tx,
            },
            ControlPlaneHandle {
                desired_state,
                health,
            },
        ))
    }

    pub async fn run(
        self,
        status: watch::Receiver<WorkerStatusReport>,
        shutdown: watch::Receiver<bool>,
    ) {
        let liveness = run_liveness_loop(
            Arc::clone(&self.api),
            self.worker_token.clone(),
            self.config.clone(),
            self.health_tx.clone(),
            status,
            shutdown.clone(),
        );
        let desired = run_desired_state_loop(
            self.api,
            self.worker_token,
            self.worker_id,
            self.config,
            self.desired_tx,
            self.health_tx,
            shutdown,
        );
        tokio::join!(liveness, desired);
    }
}

async fn run_liveness_loop<A: ControlPlaneApi>(
    api: Arc<A>,
    worker_token: SecretToken,
    config: ControlLoopConfig,
    health_tx: watch::Sender<ControlPlaneHealth>,
    status: watch::Receiver<WorkerStatusReport>,
    shutdown: watch::Receiver<bool>,
) {
    let heartbeat = run_heartbeat_loop(
        Arc::clone(&api),
        worker_token.clone(),
        config.heartbeat_interval,
        health_tx.clone(),
        shutdown.clone(),
    );
    let status = run_status_loop(
        api,
        worker_token,
        config.status_interval,
        health_tx,
        status,
        shutdown,
    );
    tokio::join!(heartbeat, status);
}

async fn run_heartbeat_loop<A: ControlPlaneApi>(
    api: Arc<A>,
    worker_token: SecretToken,
    interval: Duration,
    health_tx: watch::Sender<ControlPlaneHealth>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let ok = api.heartbeat(&worker_token).await.is_ok();
                health_tx.send_modify(|health| health.heartbeat_ok = ok);
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

async fn run_status_loop<A: ControlPlaneApi>(
    api: Arc<A>,
    worker_token: SecretToken,
    interval: Duration,
    health_tx: watch::Sender<ControlPlaneHealth>,
    status: watch::Receiver<WorkerStatusReport>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut tick = tokio::time::interval(interval);
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            _ = tick.tick() => {
                let current = status.borrow().clone();
                let ok = api.report_status(&worker_token, &current).await.is_ok();
                health_tx.send_modify(|health| health.status_ok = ok);
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_desired_state_loop<A: ControlPlaneApi>(
    api: Arc<A>,
    worker_token: SecretToken,
    worker_id: i64,
    config: ControlLoopConfig,
    desired_tx: watch::Sender<DesiredState>,
    health_tx: watch::Sender<ControlPlaneHealth>,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut backoff = config.reconnect_initial;
    loop {
        if *shutdown.borrow() {
            return;
        }
        let snapshot = match full_sync(api.as_ref(), &worker_token, worker_id).await {
            Ok(snapshot) => snapshot,
            Err(_) => {
                health_tx.send_modify(|health| {
                    health.full_sync_ok = false;
                    health.watch_connected = false;
                    health.reconnect_attempts = health.reconnect_attempts.saturating_add(1);
                });
                if wait_for_retry(backoff, &mut shutdown).await {
                    return;
                }
                backoff = next_backoff(backoff, config.reconnect_max);
                continue;
            }
        };
        replace_snapshot(&desired_tx, snapshot);
        health_tx.send_modify(|health| health.full_sync_ok = true);

        let (mut model_watch, mut instance_watch, mut model_file_watch) =
            match open_watches(api.as_ref(), &worker_token, worker_id).await {
                Ok(watches) => watches,
                Err(_) => {
                    health_tx.send_modify(|health| {
                        health.watch_connected = false;
                        health.reconnect_attempts = health.reconnect_attempts.saturating_add(1);
                    });
                    if wait_for_retry(backoff, &mut shutdown).await {
                        return;
                    }
                    backoff = next_backoff(backoff, config.reconnect_max);
                    continue;
                }
            };
        health_tx.send_modify(|health| health.watch_connected = true);

        let outcome = run_watch_session(
            api.as_ref(),
            &worker_token,
            worker_id,
            config.full_sync_interval,
            &desired_tx,
            &health_tx,
            &mut model_watch,
            &mut instance_watch,
            &mut model_file_watch,
            &mut shutdown,
        )
        .await;
        match outcome {
            WatchSessionOutcome::Shutdown => return,
            WatchSessionOutcome::Reconnect { observed_activity } => {
                if observed_activity {
                    backoff = config.reconnect_initial;
                }
            }
        }
        // A disconnected or ambiguous stream never clears the last confirmed
        // snapshot. The next successful full sync is the recovery boundary.
        health_tx.send_modify(|health| {
            health.watch_connected = false;
            health.reconnect_attempts = health.reconnect_attempts.saturating_add(1);
        });
        if wait_for_retry(backoff, &mut shutdown).await {
            return;
        }
        backoff = next_backoff(backoff, config.reconnect_max);
    }
}

async fn full_sync<A: ControlPlaneApi>(
    api: &A,
    worker_token: &SecretToken,
    worker_id: i64,
) -> Result<DesiredState, ClientError> {
    let models = api.list_models(worker_token).await?;
    let instances = api.list_model_instances(worker_token).await?;
    let model_files = api.list_model_files(worker_token, worker_id).await?;
    Ok(DesiredState {
        models: models.into_iter().map(|model| (model.id, model)).collect(),
        model_instances: instances
            .into_iter()
            .filter(|instance| instance.worker_id == Some(worker_id))
            .map(|instance| (instance.id, instance))
            .collect(),
        // The Server query is scoped, but this second check prevents a buggy
        // or stale response from granting this Worker ownership of another file.
        model_files: model_files
            .into_iter()
            .filter(|model_file| model_file.worker_id == Some(worker_id))
            .map(|model_file| (model_file.id, model_file))
            .collect(),
        revision: 0,
    })
}

async fn open_watches<A: ControlPlaneApi>(
    api: &A,
    worker_token: &SecretToken,
    worker_id: i64,
) -> Result<(A::ModelWatch, A::ModelInstanceWatch, A::ModelFileWatch), ClientError> {
    let model_watch = api.watch_models(worker_token).await?;
    let instance_watch = api.watch_model_instances(worker_token).await?;
    let model_file_watch = api.watch_model_files(worker_token, worker_id).await?;
    Ok((model_watch, instance_watch, model_file_watch))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchSessionOutcome {
    Reconnect { observed_activity: bool },
    Shutdown,
}

#[allow(clippy::too_many_arguments)]
async fn run_watch_session<A: ControlPlaneApi>(
    api: &A,
    worker_token: &SecretToken,
    worker_id: i64,
    full_sync_interval: Duration,
    desired_tx: &watch::Sender<DesiredState>,
    health_tx: &watch::Sender<ControlPlaneHealth>,
    model_watch: &mut A::ModelWatch,
    instance_watch: &mut A::ModelInstanceWatch,
    model_file_watch: &mut A::ModelFileWatch,
    shutdown: &mut watch::Receiver<bool>,
) -> WatchSessionOutcome {
    let start = Instant::now() + full_sync_interval;
    let mut full_sync_tick = tokio::time::interval_at(start, full_sync_interval);
    full_sync_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut observed_activity = false;

    loop {
        tokio::select! {
            event = model_watch.next_event() => {
                match event {
                    Ok(Some(event)) => {
                        if !apply_model_event(desired_tx, event) {
                            return WatchSessionOutcome::Reconnect {
                                observed_activity: true,
                            };
                        }
                        observed_activity = true;
                    }
                    Ok(None) | Err(_) => {
                        return WatchSessionOutcome::Reconnect { observed_activity };
                    }
                }
            }
            event = instance_watch.next_event() => {
                match event {
                    Ok(Some(event)) => {
                        if !apply_instance_event(desired_tx, worker_id, event) {
                            return WatchSessionOutcome::Reconnect {
                                observed_activity: true,
                            };
                        }
                        observed_activity = true;
                    }
                    Ok(None) | Err(_) => {
                        return WatchSessionOutcome::Reconnect { observed_activity };
                    }
                }
            }
            event = model_file_watch.next_event() => {
                match event {
                    Ok(Some(event)) => {
                        if !apply_model_file_event(desired_tx, worker_id, event) {
                            return WatchSessionOutcome::Reconnect {
                                observed_activity: true,
                            };
                        }
                        observed_activity = true;
                    }
                    Ok(None) | Err(_) => {
                        return WatchSessionOutcome::Reconnect { observed_activity };
                    }
                }
            }
            _ = full_sync_tick.tick() => {
                match full_sync(api, worker_token, worker_id).await {
                    Ok(snapshot) => {
                        replace_snapshot(desired_tx, snapshot);
                        health_tx.send_modify(|health| health.full_sync_ok = true);
                        observed_activity = true;
                    }
                    Err(_) => {
                        // A failed refresh cannot prove that any assignment was
                        // removed, so retain the last authoritative state.
                        health_tx.send_modify(|health| health.full_sync_ok = false);
                    }
                }
            }
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return WatchSessionOutcome::Shutdown;
                }
            }
        }
    }
}

fn apply_model_event(
    desired_tx: &watch::Sender<DesiredState>,
    event: WatchEvent<ModelPublic>,
) -> bool {
    match event {
        WatchEvent::Created(model) | WatchEvent::Updated(model) => {
            desired_tx.send_modify(|state| {
                state.models.insert(model.id, model);
                state.revision = state.revision.saturating_add(1);
            });
            true
        }
        WatchEvent::Deleted { id } => {
            desired_tx.send_modify(|state| {
                state.models.remove(&id);
                state.revision = state.revision.saturating_add(1);
            });
            true
        }
        WatchEvent::Heartbeat => true,
        WatchEvent::Unknown { .. } => false,
    }
}

fn apply_instance_event(
    desired_tx: &watch::Sender<DesiredState>,
    worker_id: i64,
    event: WatchEvent<ModelInstancePublic>,
) -> bool {
    match event {
        WatchEvent::Created(instance) | WatchEvent::Updated(instance) => {
            desired_tx.send_modify(|state| {
                if instance.worker_id == Some(worker_id) {
                    state.model_instances.insert(instance.id, instance);
                } else {
                    state.model_instances.remove(&instance.id);
                }
                state.revision = state.revision.saturating_add(1);
            });
            true
        }
        WatchEvent::Deleted { id } => {
            desired_tx.send_modify(|state| {
                state.model_instances.remove(&id);
                state.revision = state.revision.saturating_add(1);
            });
            true
        }
        WatchEvent::Heartbeat => true,
        WatchEvent::Unknown { .. } => false,
    }
}

fn apply_model_file_event(
    desired_tx: &watch::Sender<DesiredState>,
    worker_id: i64,
    event: WatchEvent<ModelFilePublic>,
) -> bool {
    match event {
        WatchEvent::Created(model_file) | WatchEvent::Updated(model_file) => {
            desired_tx.send_modify(|state| {
                if model_file.worker_id == Some(worker_id) {
                    state.model_files.insert(model_file.id, model_file);
                } else {
                    state.model_files.remove(&model_file.id);
                }
                state.revision = state.revision.saturating_add(1);
            });
            true
        }
        WatchEvent::Deleted { id } => {
            // M3-C.2 owns only the authoritative cache. Filesystem cleanup is a
            // later, separately gated ModelFile manager responsibility.
            desired_tx.send_modify(|state| {
                state.model_files.remove(&id);
                state.revision = state.revision.saturating_add(1);
            });
            true
        }
        WatchEvent::Heartbeat => true,
        WatchEvent::Unknown { .. } => false,
    }
}

fn replace_snapshot(desired_tx: &watch::Sender<DesiredState>, mut snapshot: DesiredState) {
    snapshot.revision = desired_tx.borrow().revision.saturating_add(1);
    desired_tx.send_replace(snapshot);
}

async fn wait_for_retry(delay: Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

fn next_backoff(current: Duration, maximum: Duration) -> Duration {
    current.saturating_mul(2).min(maximum)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use gpustack_client::types::Paginated;

    use super::*;

    #[test]
    fn default_full_sync_bounds_authoritative_delete_convergence() {
        assert_eq!(
            ControlLoopConfig::default().full_sync_interval,
            Duration::from_secs(30)
        );
    }

    struct ScriptedWatch<T> {
        delay: Option<Duration>,
        events: VecDeque<Result<Option<WatchEvent<T>>, ClientError>>,
    }

    impl<T> ScriptedWatch<T> {
        fn pending() -> Self {
            Self {
                delay: None,
                events: VecDeque::new(),
            }
        }

        fn delayed_error(delay: Duration) -> Self {
            Self {
                delay: Some(delay),
                events: VecDeque::from([Err(ClientError::Watch("scripted disconnect".to_owned()))]),
            }
        }
    }

    impl<T: Send> EventStream<T> for ScriptedWatch<T> {
        async fn next_event(&mut self) -> Result<Option<WatchEvent<T>>, ClientError> {
            if let Some(delay) = self.delay.take() {
                tokio::time::sleep(delay).await;
            }
            match self.events.pop_front() {
                Some(result) => result,
                None => std::future::pending().await,
            }
        }
    }

    #[derive(Default)]
    struct MockState {
        heartbeat_results: VecDeque<bool>,
        status_results: VecDeque<bool>,
        models: VecDeque<Result<Vec<ModelPublic>, ClientError>>,
        instances: VecDeque<Result<Vec<ModelInstancePublic>, ClientError>>,
        model_files: VecDeque<Result<Vec<ModelFilePublic>, ClientError>>,
        model_watches: VecDeque<Result<ScriptedWatch<ModelPublic>, ClientError>>,
        instance_watches: VecDeque<Result<ScriptedWatch<ModelInstancePublic>, ClientError>>,
        model_file_watches: VecDeque<Result<ScriptedWatch<ModelFilePublic>, ClientError>>,
        heartbeat_calls: u64,
        status_calls: u64,
        status_delay: Option<Duration>,
    }

    #[derive(Clone, Default)]
    struct MockApi {
        state: Arc<Mutex<MockState>>,
    }

    impl ControlPlaneApi for MockApi {
        type ModelWatch = ScriptedWatch<ModelPublic>;
        type ModelInstanceWatch = ScriptedWatch<ModelInstancePublic>;
        type ModelFileWatch = ScriptedWatch<ModelFilePublic>;

        async fn heartbeat(&self, _worker_token: &SecretToken) -> Result<(), ClientError> {
            let mut state = self.state.lock().expect("mock state lock");
            state.heartbeat_calls += 1;
            state
                .heartbeat_results
                .pop_front()
                .unwrap_or(true)
                .then_some(())
                .ok_or_else(|| ClientError::Watch("heartbeat unavailable".to_owned()))
        }

        async fn report_status(
            &self,
            _worker_token: &SecretToken,
            _status: &WorkerStatusReport,
        ) -> Result<(), ClientError> {
            let (result, delay) = {
                let mut state = self.state.lock().expect("mock state lock");
                state.status_calls += 1;
                (
                    state.status_results.pop_front().unwrap_or(true),
                    state.status_delay.take(),
                )
            };
            if let Some(delay) = delay {
                tokio::time::sleep(delay).await;
            }
            result
                .then_some(())
                .ok_or_else(|| ClientError::Watch("status unavailable".to_owned()))
        }

        async fn list_models(
            &self,
            _worker_token: &SecretToken,
        ) -> Result<Vec<ModelPublic>, ClientError> {
            self.state
                .lock()
                .expect("mock state lock")
                .models
                .pop_front()
                .expect("scripted model snapshot")
        }

        async fn list_model_instances(
            &self,
            _worker_token: &SecretToken,
        ) -> Result<Vec<ModelInstancePublic>, ClientError> {
            self.state
                .lock()
                .expect("mock state lock")
                .instances
                .pop_front()
                .expect("scripted instance snapshot")
        }

        async fn list_model_files(
            &self,
            _worker_token: &SecretToken,
            _worker_id: i64,
        ) -> Result<Vec<ModelFilePublic>, ClientError> {
            self.state
                .lock()
                .expect("mock state lock")
                .model_files
                .pop_front()
                .expect("scripted ModelFile snapshot")
        }

        async fn watch_models(
            &self,
            _worker_token: &SecretToken,
        ) -> Result<Self::ModelWatch, ClientError> {
            self.state
                .lock()
                .expect("mock state lock")
                .model_watches
                .pop_front()
                .expect("scripted model watch")
        }

        async fn watch_model_instances(
            &self,
            _worker_token: &SecretToken,
        ) -> Result<Self::ModelInstanceWatch, ClientError> {
            self.state
                .lock()
                .expect("mock state lock")
                .instance_watches
                .pop_front()
                .expect("scripted instance watch")
        }

        async fn watch_model_files(
            &self,
            _worker_token: &SecretToken,
            _worker_id: i64,
        ) -> Result<Self::ModelFileWatch, ClientError> {
            self.state
                .lock()
                .expect("mock state lock")
                .model_file_watches
                .pop_front()
                .expect("scripted ModelFile watch")
        }
    }

    fn model_fixture() -> Vec<ModelPublic> {
        serde_json::from_slice::<Paginated<ModelPublic>>(include_bytes!(
            "../../../fixtures/server-v2.2.3/models-full-sync.response.json"
        ))
        .expect("model fixture")
        .items
    }

    fn instance_fixture() -> Vec<ModelInstancePublic> {
        serde_json::from_slice::<Paginated<ModelInstancePublic>>(include_bytes!(
            "../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json"
        ))
        .expect("model instance fixture")
        .items
    }

    fn model_file_fixture() -> Vec<ModelFilePublic> {
        serde_json::from_slice::<Paginated<ModelFilePublic>>(include_bytes!(
            "../../../fixtures/server-v2.2.3/model-files-full-sync.response.json"
        ))
        .expect("ModelFile fixture")
        .items
    }

    fn status_fixture() -> WorkerStatusReport {
        serde_json::from_slice(include_bytes!(
            "../../../fixtures/server-v2.2.3/worker-status.request.json"
        ))
        .expect("Worker status fixture")
    }

    fn fast_config() -> ControlLoopConfig {
        ControlLoopConfig {
            heartbeat_interval: Duration::from_millis(5),
            status_interval: Duration::from_millis(5),
            full_sync_interval: Duration::from_secs(1),
            reconnect_initial: Duration::from_millis(5),
            reconnect_max: Duration::from_millis(10),
        }
    }

    #[tokio::test]
    async fn liveness_recovers_after_transient_failures() {
        let api = MockApi::default();
        {
            let mut state = api.state.lock().expect("mock state lock");
            state.heartbeat_results = VecDeque::from([false, true]);
            state.status_results = VecDeque::from([false, true]);
            state.status_delay = Some(Duration::from_millis(30));
        }
        let (health_tx, mut health) = watch::channel(ControlPlaneHealth::default());
        let (_status_tx, status) = watch::channel(status_fixture());
        let (shutdown_tx, shutdown) = watch::channel(false);
        let task = tokio::spawn(run_liveness_loop(
            Arc::new(api.clone()),
            SecretToken::new("worker-secret").expect("valid token"),
            fast_config(),
            health_tx,
            status,
            shutdown,
        ));

        tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                if health.borrow().heartbeat_ok && health.borrow().status_ok {
                    break;
                }
                health.changed().await.expect("health update");
            }
        })
        .await
        .expect("liveness recovery timeout");
        shutdown_tx.send(true).expect("request shutdown");
        task.await.expect("liveness task");

        let state = api.state.lock().expect("mock state lock");
        assert!(state.heartbeat_calls >= 4);
        assert!(state.status_calls >= 2);
    }

    #[tokio::test]
    async fn disconnect_preserves_state_until_full_sync_recovery() {
        let api = MockApi::default();
        let first_instances = instance_fixture();
        let mut second_instances = first_instances.clone();
        second_instances[0].id = 302;
        second_instances[0].name = "contract-qwen-002".to_owned();
        let first_model_files = model_file_fixture();
        let mut second_model_files = first_model_files.clone();
        second_model_files[0].id = 402;
        second_model_files[0].local_path = Some("/srv/models/contract-qwen-002".to_owned());
        {
            let mut state = api.state.lock().expect("mock state lock");
            state.models = VecDeque::from([Ok(model_fixture()), Ok(model_fixture())]);
            state.instances = VecDeque::from([Ok(first_instances), Ok(second_instances)]);
            state.model_files = VecDeque::from([Ok(first_model_files), Ok(second_model_files)]);
            state.model_watches =
                VecDeque::from([Ok(ScriptedWatch::pending()), Ok(ScriptedWatch::pending())]);
            state.instance_watches =
                VecDeque::from([Ok(ScriptedWatch::pending()), Ok(ScriptedWatch::pending())]);
            state.model_file_watches = VecDeque::from([
                Ok(ScriptedWatch::delayed_error(Duration::from_millis(30))),
                Ok(ScriptedWatch::pending()),
            ]);
        }
        let (control, handle) = WorkerControlPlane::new(
            api,
            SecretToken::new("worker-secret").expect("valid token"),
            101,
            fast_config(),
        )
        .expect("control plane");
        let mut desired = handle.desired_state();
        let (_status_tx, status) = watch::channel(status_fixture());
        let (shutdown_tx, shutdown) = watch::channel(false);
        let task = tokio::spawn(control.run(status, shutdown));

        desired.changed().await.expect("initial full sync");
        assert!(desired.borrow().model_instances.contains_key(&301));
        assert!(desired.borrow().model_files.contains_key(&401));
        assert_eq!(desired.borrow().revision, 1);
        tokio::time::timeout(Duration::from_millis(200), async {
            loop {
                desired.changed().await.expect("recovery full sync");
                if desired.borrow().revision >= 2 {
                    break;
                }
            }
        })
        .await
        .expect("reconnect timeout");
        assert!(!desired.borrow().model_instances.contains_key(&301));
        assert!(desired.borrow().model_instances.contains_key(&302));
        assert!(!desired.borrow().model_files.contains_key(&401));
        assert!(desired.borrow().model_files.contains_key(&402));

        shutdown_tx.send(true).expect("request shutdown");
        task.await.expect("control task");
    }

    #[test]
    fn instance_events_remove_assignments_moved_to_another_worker() {
        let (desired_tx, desired) = watch::channel(DesiredState::default());
        let mut instance = instance_fixture().remove(0);
        assert!(apply_instance_event(
            &desired_tx,
            101,
            WatchEvent::Created(instance.clone())
        ));
        assert!(desired.borrow().model_instances.contains_key(&301));

        instance.worker_id = Some(202);
        assert!(apply_instance_event(
            &desired_tx,
            101,
            WatchEvent::Updated(instance)
        ));
        assert!(desired.borrow().model_instances.is_empty());
    }

    #[test]
    fn model_file_events_remove_assignments_moved_to_another_worker() {
        let (desired_tx, desired) = watch::channel(DesiredState::default());
        let mut model_file = model_file_fixture().remove(0);
        assert!(apply_model_file_event(
            &desired_tx,
            101,
            WatchEvent::Created(model_file.clone())
        ));
        assert!(desired.borrow().model_files.contains_key(&401));

        model_file.worker_id = Some(202);
        assert!(apply_model_file_event(
            &desired_tx,
            101,
            WatchEvent::Updated(model_file)
        ));
        assert!(desired.borrow().model_files.is_empty());
    }

    #[test]
    fn model_file_delete_only_removes_the_cached_record() {
        let (desired_tx, desired) = watch::channel(DesiredState::default());
        let model_file = model_file_fixture().remove(0);
        assert!(apply_model_file_event(
            &desired_tx,
            101,
            WatchEvent::Created(model_file)
        ));
        assert!(apply_model_file_event(
            &desired_tx,
            101,
            WatchEvent::Deleted { id: 401 }
        ));
        assert!(desired.borrow().model_files.is_empty());
        assert_eq!(desired.borrow().revision, 2);
    }

    #[test]
    fn unknown_model_file_event_requires_a_full_reconnect() {
        let (desired_tx, desired) = watch::channel(DesiredState::default());
        assert!(!apply_model_file_event(
            &desired_tx,
            101,
            WatchEvent::Unknown { id: 401 }
        ));
        assert_eq!(desired.borrow().revision, 0);
    }

    #[tokio::test]
    async fn full_sync_filters_unscoped_model_files_and_rejects_partial_refresh() {
        let api = MockApi::default();
        let mut files = model_file_fixture();
        let mut foreign = files[0].clone();
        foreign.id = 499;
        foreign.worker_id = Some(202);
        files.push(foreign);
        {
            let mut state = api.state.lock().expect("mock state lock");
            state.models = VecDeque::from([Ok(model_fixture()), Ok(model_fixture())]);
            state.instances = VecDeque::from([Ok(instance_fixture()), Ok(instance_fixture())]);
            state.model_files = VecDeque::from([
                Ok(files),
                Err(ClientError::Watch("ModelFile list unavailable".to_owned())),
            ]);
        }
        let token = SecretToken::new("worker-secret").expect("valid token");
        let (desired_tx, desired) = watch::channel(DesiredState::default());

        let first = full_sync(&api, &token, 101).await.expect("first full sync");
        replace_snapshot(&desired_tx, first);
        assert!(desired.borrow().model_files.contains_key(&401));
        assert!(!desired.borrow().model_files.contains_key(&499));

        assert!(full_sync(&api, &token, 101).await.is_err());
        assert!(desired.borrow().model_files.contains_key(&401));
        assert_eq!(desired.borrow().revision, 1);
    }
}
