//! Operator context, Managed model, and Worker maintenance CLI orchestration.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand, ValueEnum};
use gpustack_client::http::Transport;
use gpustack_client::types::{
    GpuSelector, ModelCreate, ModelInstancePublic, ModelPublic, ModelSource, WorkerMaintenance,
    WorkerPublic, WorkerUpdate,
};
use gpustack_client::{
    ClientError, Context, ContextStore, ContextStoreError, SecretToken, ServerClient,
};
use serde::Serialize;
use thiserror::Error;
use tokio::time::Instant;
use url::Url;

use crate::{EXIT_SUCCESS, Execution, OutputFormat};

#[derive(Debug, Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub command: ContextSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ContextSubcommand {
    /// Add a named Server URL and token-file reference.
    Add(ContextAddArgs),
    /// Select the default context for Managed commands.
    Use(ContextNameArgs),
    /// List configured contexts without reading their tokens.
    List,
    /// Remove a context without deleting its referenced token file.
    Remove(ContextRemoveArgs),
}

#[derive(Debug, Args)]
pub struct ContextAddArgs {
    /// Unique local context name.
    pub name: String,
    /// GPUStack Server base URL.
    #[arg(long)]
    pub server_url: Url,
    /// Absolute path to a permission-restricted operator token file.
    #[arg(long)]
    pub token_file: PathBuf,
    /// Make this context active immediately.
    #[arg(long = "use")]
    pub use_now: bool,
}

#[derive(Debug, Args)]
pub struct ContextNameArgs {
    /// Existing context name.
    pub name: String,
}

#[derive(Debug, Args)]
pub struct ContextRemoveArgs {
    /// Existing context name.
    pub name: String,
    /// Skip the interactive confirmation.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct ModelArgs {
    #[command(subcommand)]
    pub command: ModelSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ModelSubcommand {
    /// Create a Server-managed local-path vLLM model.
    Deploy(ModelDeployArgs),
    /// List Server-managed models visible to the active context.
    List,
    /// Get a model by numeric ID or exact name.
    Get(ModelSelectorArgs),
    /// Show a model and all of its ModelInstance states.
    Status(ModelSelectorArgs),
    /// Delete a model by numeric ID or exact name.
    Delete(ModelDeleteArgs),
}

#[derive(Debug, Args)]
pub struct ModelDeployArgs {
    /// Unique Managed model name.
    pub name: String,
    /// Exact target Worker name.
    #[arg(long)]
    pub worker: String,
    /// Absolute model path shared by the Worker and vLLM container.
    #[arg(long)]
    pub local_path: PathBuf,
    /// Immutable or explicitly selected Runner image.
    #[arg(long)]
    pub image: String,
    /// GPU index on the selected Worker; pass once or twice.
    #[arg(long = "gpu", required = true)]
    pub gpu_indexes: Vec<u32>,
    /// Must equal the number of selected GPUs.
    #[arg(long)]
    pub tensor_parallel_size: Option<u32>,
    /// Maximum vLLM model context length.
    #[arg(long)]
    pub max_model_len: Option<u32>,
    /// Fraction of GPU memory vLLM may reserve, in (0, 1].
    #[arg(long)]
    pub gpu_memory_utilization: Option<f64>,
    /// vLLM model data type.
    #[arg(long, value_enum)]
    pub dtype: Option<ManagedDtype>,
    /// Allow model repository code to execute inside vLLM.
    #[arg(long)]
    pub trust_remote_code: bool,
    /// Force vLLM eager execution.
    #[arg(long)]
    pub enforce_eager: bool,
    /// Follow ModelInstance states until all run or any fails.
    #[arg(long)]
    pub wait: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ManagedDtype {
    Auto,
    Half,
    Float16,
    Bfloat16,
    Float,
    Float32,
}

impl ManagedDtype {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Half => "half",
            Self::Float16 => "float16",
            Self::Bfloat16 => "bfloat16",
            Self::Float => "float",
            Self::Float32 => "float32",
        }
    }
}

#[derive(Debug, Args)]
pub struct ModelSelectorArgs {
    /// Numeric Model ID or exact Model name.
    pub model: String,
}

#[derive(Debug, Args)]
pub struct ModelDeleteArgs {
    /// Numeric Model ID or exact Model name.
    pub model: String,
    /// Skip the interactive confirmation.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct WorkerMaintenanceArgs {
    #[command(subcommand)]
    pub command: WorkerMaintenanceSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum WorkerMaintenanceSubcommand {
    /// Prevent new scheduling onto the exact Worker.
    Enable(MaintenanceEnableArgs),
    /// Make the exact Worker schedulable again.
    Disable(MaintenanceDisableArgs),
}

#[derive(Debug, Args)]
pub struct MaintenanceEnableArgs {
    /// Numeric Worker ID or exact Worker name.
    pub worker: String,
    /// Operator-visible reason for maintenance.
    #[arg(long)]
    pub message: Option<String>,
}

#[derive(Debug, Args)]
pub struct MaintenanceDisableArgs {
    /// Numeric Worker ID or exact Worker name.
    pub worker: String,
    /// Skip the interactive confirmation.
    #[arg(long)]
    pub yes: bool,
}

pub fn run_context_command(
    output_format: OutputFormat,
    store: &ContextStore,
    arguments: ContextArgs,
) -> Result<Execution, ManagedError> {
    match arguments.command {
        ContextSubcommand::Add(arguments) => {
            let context = Context {
                name: arguments.name,
                server_url: arguments.server_url,
                token_file: arguments.token_file,
            };
            store.add(context.clone(), arguments.use_now)?;
            render_context(output_format, &context, store.resolve(None).ok().as_ref())
        }
        ContextSubcommand::Use(arguments) => {
            let context = store.use_context(&arguments.name)?;
            render_context(output_format, &context, Some(&context))
        }
        ContextSubcommand::List => {
            let snapshot = store.snapshot()?;
            render_context_list(
                output_format,
                snapshot.active.as_deref(),
                &snapshot.contexts,
            )
        }
        ContextSubcommand::Remove(arguments) => {
            if !arguments.yes {
                return Err(ManagedError::ConfirmationRequired(
                    "context remove requires confirmation or --yes",
                ));
            }
            let context = store.remove(&arguments.name)?;
            render_removed_context(output_format, &context)
        }
    }
}

pub async fn run_model_command<T: Transport>(
    output_format: OutputFormat,
    timeout: Duration,
    client: &ServerClient<T>,
    token: &SecretToken,
    arguments: ModelArgs,
    transition_sink: Option<&mut dyn FnMut(&ModelTransition)>,
) -> Result<Execution, ManagedError> {
    match arguments.command {
        ModelSubcommand::Deploy(arguments) => {
            let wait = arguments.wait;
            let request = build_model_create(arguments)?;
            let model = client.create_model(token, &request).await?;
            if wait {
                let (instances, transitions) =
                    wait_for_model(client, token, model.id, timeout, transition_sink).await?;
                render_model_deploy(output_format, &model, &instances, &transitions)
            } else {
                render_model_deploy(output_format, &model, &[], &[])
            }
        }
        ModelSubcommand::List => {
            let models = client.list_models(token).await?.items;
            render_models(output_format, &models)
        }
        ModelSubcommand::Get(arguments) => {
            let model = resolve_model(client, token, &arguments.model).await?;
            render_model(output_format, &model)
        }
        ModelSubcommand::Status(arguments) => {
            let model = resolve_model(client, token, &arguments.model).await?;
            let instances = client
                .list_model_instances_for_model(token, model.id)
                .await?
                .items;
            render_model_status(output_format, &model, &instances)
        }
        ModelSubcommand::Delete(arguments) => {
            if !arguments.yes {
                return Err(ManagedError::ConfirmationRequired(
                    "model delete requires confirmation or --yes",
                ));
            }
            let model = resolve_model(client, token, &arguments.model).await?;
            client.delete_model(token, model.id).await?;
            render_deleted_model(output_format, &model)
        }
    }
}

pub async fn run_worker_maintenance_command<T: Transport>(
    output_format: OutputFormat,
    client: &ServerClient<T>,
    token: &SecretToken,
    arguments: WorkerMaintenanceArgs,
) -> Result<Execution, ManagedError> {
    let (selector, enabled, message) = match arguments.command {
        WorkerMaintenanceSubcommand::Enable(arguments) => {
            let message = arguments
                .message
                .unwrap_or_else(|| "enabled by gpustack-rs operator CLI".to_owned());
            if message.trim().is_empty() || message.chars().any(char::is_control) {
                return Err(ManagedError::InvalidArgument(
                    "maintenance message must be non-empty and contain no control characters"
                        .to_owned(),
                ));
            }
            (arguments.worker, true, Some(message))
        }
        WorkerMaintenanceSubcommand::Disable(arguments) => {
            if !arguments.yes {
                return Err(ManagedError::ConfirmationRequired(
                    "disabling Worker maintenance requires confirmation or --yes",
                ));
            }
            (arguments.worker, false, None)
        }
    };
    let before = resolve_worker(client, token, &selector).await?;
    let mut update = WorkerUpdate::from(&before);
    update.maintenance = Some(WorkerMaintenance { enabled, message });
    let after = client.update_worker(token, before.id, &update).await?;
    render_maintenance_change(output_format, &before, &after)
}

fn build_model_create(arguments: ModelDeployArgs) -> Result<ModelCreate, ManagedError> {
    if arguments.name.trim().is_empty()
        || arguments.name.trim() != arguments.name
        || arguments.name.chars().any(char::is_control)
    {
        return Err(ManagedError::InvalidArgument(
            "model name must be non-empty without surrounding whitespace".to_owned(),
        ));
    }
    if arguments.worker.trim().is_empty()
        || arguments.worker.trim() != arguments.worker
        || arguments.worker.contains(':')
        || arguments.worker.chars().any(char::is_control)
    {
        return Err(ManagedError::InvalidArgument(
            "Worker name must be non-empty, trimmed, and contain no ':'".to_owned(),
        ));
    }
    if !arguments.local_path.is_absolute() {
        return Err(ManagedError::InvalidArgument(
            "--local-path must be absolute".to_owned(),
        ));
    }
    if arguments.image.trim().is_empty()
        || arguments.image.trim() != arguments.image
        || arguments.image.chars().any(char::is_control)
    {
        return Err(ManagedError::InvalidArgument(
            "--image must be non-empty without surrounding whitespace".to_owned(),
        ));
    }
    let unique: BTreeSet<u32> = arguments.gpu_indexes.iter().copied().collect();
    if unique.len() != arguments.gpu_indexes.len() || !(1..=2).contains(&unique.len()) {
        return Err(ManagedError::InvalidArgument(
            "R1 requires one or two distinct --gpu indexes".to_owned(),
        ));
    }
    let tensor_parallel_size = arguments
        .tensor_parallel_size
        .unwrap_or(arguments.gpu_indexes.len() as u32);
    if tensor_parallel_size != arguments.gpu_indexes.len() as u32 {
        return Err(ManagedError::InvalidArgument(
            "--tensor-parallel-size must equal the number of selected GPUs".to_owned(),
        ));
    }
    if arguments
        .gpu_memory_utilization
        .is_some_and(|value| !(0.0..=1.0).contains(&value) || value == 0.0)
    {
        return Err(ManagedError::InvalidArgument(
            "--gpu-memory-utilization must be greater than 0 and at most 1".to_owned(),
        ));
    }

    let mut backend_parameters = vec![format!("--tensor-parallel-size={tensor_parallel_size}")];
    if let Some(value) = arguments.max_model_len {
        backend_parameters.push(format!("--max-model-len={value}"));
    }
    if let Some(value) = arguments.gpu_memory_utilization {
        backend_parameters.push(format!("--gpu-memory-utilization={value}"));
    }
    if let Some(value) = arguments.dtype {
        backend_parameters.push(format!("--dtype={}", value.as_str()));
    }
    if arguments.trust_remote_code {
        backend_parameters.push("--trust-remote-code".to_owned());
    }
    if arguments.enforce_eager {
        backend_parameters.push("--enforce-eager".to_owned());
    }

    let gpu_ids = arguments
        .gpu_indexes
        .iter()
        .map(|index| format!("{}:cuda:{index}", arguments.worker))
        .collect();
    let mut worker_selector = BTreeMap::new();
    worker_selector.insert("worker-name".to_owned(), arguments.worker);
    Ok(ModelCreate {
        name: arguments.name,
        source: ModelSource("local_path".to_owned()),
        local_path: arguments.local_path.display().to_string(),
        replicas: 1,
        backend: "vLLM".to_owned(),
        image_name: arguments.image,
        worker_selector,
        gpu_selector: GpuSelector {
            gpu_ids,
            gpus_per_replica: tensor_parallel_size,
        },
        backend_parameters,
        distributed_inference_across_workers: false,
        // File preparation does not create an inference route before a
        // serving endpoint exists.
        enable_model_route: false,
    })
}

async fn resolve_model<T: Transport>(
    client: &ServerClient<T>,
    token: &SecretToken,
    selector: &str,
) -> Result<ModelPublic, ManagedError> {
    if let Ok(id) = selector.parse::<i64>() {
        return client.get_model(token, id).await.map_err(Into::into);
    }
    let exact: Vec<ModelPublic> = client
        .search_models(token, selector)
        .await?
        .items
        .into_iter()
        .filter(|model| model.name == selector)
        .collect();
    match exact.as_slice() {
        [model] => Ok(model.clone()),
        [] => Err(ManagedError::NotFound("Model", selector.to_owned())),
        _ => Err(ManagedError::Ambiguous("Model", selector.to_owned())),
    }
}

async fn resolve_worker<T: Transport>(
    client: &ServerClient<T>,
    token: &SecretToken,
    selector: &str,
) -> Result<WorkerPublic, ManagedError> {
    if let Ok(id) = selector.parse::<i64>() {
        return client.get_worker(token, id).await.map_err(Into::into);
    }
    let exact: Vec<WorkerPublic> = client
        .list_workers_by_name(token, selector)
        .await?
        .items
        .into_iter()
        .filter(|worker| worker.name == selector)
        .collect();
    match exact.as_slice() {
        [worker] => Ok(worker.clone()),
        [] => Err(ManagedError::NotFound("Worker", selector.to_owned())),
        _ => Err(ManagedError::Ambiguous("Worker", selector.to_owned())),
    }
}

async fn wait_for_model<T: Transport>(
    client: &ServerClient<T>,
    token: &SecretToken,
    model_id: i64,
    timeout: Duration,
    mut transition_sink: Option<&mut dyn FnMut(&ModelTransition)>,
) -> Result<(Vec<ModelInstancePublic>, Vec<ModelTransition>), ManagedError> {
    let deadline = Instant::now() + timeout.max(Duration::from_secs(1));
    let mut last_states = BTreeMap::<i64, String>::new();
    let mut transitions = Vec::new();
    loop {
        let instances = client
            .list_model_instances_for_model(token, model_id)
            .await?
            .items;
        for instance in &instances {
            if last_states.get(&instance.id) != Some(&instance.state.0) {
                last_states.insert(instance.id, instance.state.0.clone());
                let transition = ModelTransition::from(instance);
                if let Some(sink) = transition_sink.as_mut() {
                    sink(&transition);
                }
                transitions.push(transition);
            }
        }
        if instances.iter().any(|instance| instance.state.0 == "error") {
            return Err(ManagedError::ModelFailed {
                states: summarize_states(&instances),
            });
        }
        if !instances.is_empty()
            && instances
                .iter()
                .all(|instance| instance.state.0 == "running")
        {
            return Ok((instances, transitions));
        }
        if Instant::now() >= deadline {
            return Err(ManagedError::WaitTimeout {
                states: summarize_states(&instances),
            });
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

fn summarize_states(instances: &[ModelInstancePublic]) -> String {
    if instances.is_empty() {
        return "no instances".to_owned();
    }
    instances
        .iter()
        .map(|instance| format!("{}={}", instance.id, table_value(&instance.state.0)))
        .collect::<Vec<_>>()
        .join(",")
}

#[derive(Serialize)]
struct ContextView<'a> {
    name: &'a str,
    server_url: &'a Url,
    token_file: &'a PathBuf,
    active: bool,
}

fn context_view<'a>(context: &'a Context, active: Option<&Context>) -> ContextView<'a> {
    ContextView {
        name: &context.name,
        server_url: &context.server_url,
        token_file: &context.token_file,
        active: active.is_some_and(|active| active.name == context.name),
    }
}

fn render_context(
    output_format: OutputFormat,
    context: &Context,
    active: Option<&Context>,
) -> Result<Execution, ManagedError> {
    let view = context_view(context, active);
    let output = match output_format {
        OutputFormat::Json => to_json(&view)?,
        OutputFormat::Table => format!(
            "name: {}\nserver_url: {}\ntoken_file: {}\nactive: {}\n",
            table_value(view.name),
            view.server_url,
            table_value(&view.token_file.display().to_string()),
            view.active
        ),
    };
    success(output)
}

fn render_context_list(
    output_format: OutputFormat,
    active: Option<&str>,
    contexts: &[Context],
) -> Result<Execution, ManagedError> {
    let views: Vec<ContextView<'_>> = contexts
        .iter()
        .map(|context| ContextView {
            name: &context.name,
            server_url: &context.server_url,
            token_file: &context.token_file,
            active: active == Some(context.name.as_str()),
        })
        .collect();
    let output = match output_format {
        OutputFormat::Json => to_json(&views)?,
        OutputFormat::Table => {
            let mut output = String::from("ACTIVE\tNAME\tSERVER\tTOKEN_FILE\n");
            for view in views {
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}",
                    if view.active { "*" } else { "" },
                    table_value(view.name),
                    view.server_url,
                    table_value(&view.token_file.display().to_string())
                )
                .expect("write to String");
            }
            output
        }
    };
    success(output)
}

#[derive(Serialize)]
struct RemovedContextView<'a> {
    removed: bool,
    name: &'a str,
}

fn render_removed_context(
    output_format: OutputFormat,
    context: &Context,
) -> Result<Execution, ManagedError> {
    let view = RemovedContextView {
        removed: true,
        name: &context.name,
    };
    let output = match output_format {
        OutputFormat::Json => to_json(&view)?,
        OutputFormat::Table => format!("removed: true\nname: {}\n", table_value(&context.name)),
    };
    success(output)
}

fn render_models(
    output_format: OutputFormat,
    models: &[ModelPublic],
) -> Result<Execution, ManagedError> {
    let output = match output_format {
        OutputFormat::Json => to_json(&models)?,
        OutputFormat::Table => {
            let mut output = String::from("ID\tNAME\tSOURCE\tBACKEND\tLOCAL_PATH\n");
            for model in models {
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}\t{}",
                    model.id,
                    table_value(&model.name),
                    table_value(&model.source.0),
                    table_value(model.backend.as_deref().unwrap_or("unknown")),
                    table_value(model.local_path.as_deref().unwrap_or("-"))
                )
                .expect("write to String");
            }
            output
        }
    };
    success(output)
}

fn render_model(
    output_format: OutputFormat,
    model: &ModelPublic,
) -> Result<Execution, ManagedError> {
    let output = match output_format {
        OutputFormat::Json => to_json(model)?,
        OutputFormat::Table => format!(
            "id: {}\nname: {}\nsource: {}\nbackend: {}\nlocal_path: {}\nimage: {}\n",
            model.id,
            table_value(&model.name),
            table_value(&model.source.0),
            table_value(model.backend.as_deref().unwrap_or("unknown")),
            table_value(model.local_path.as_deref().unwrap_or("-")),
            table_value(model.image_name.as_deref().unwrap_or("-"))
        ),
    };
    success(output)
}

#[derive(Serialize)]
struct ModelStatusView<'a> {
    model: &'a ModelPublic,
    instances: &'a [ModelInstancePublic],
}

fn render_model_status(
    output_format: OutputFormat,
    model: &ModelPublic,
    instances: &[ModelInstancePublic],
) -> Result<Execution, ManagedError> {
    let output = match output_format {
        OutputFormat::Json => to_json(&ModelStatusView { model, instances })?,
        OutputFormat::Table => {
            let mut output = format!(
                "model_id: {}\nmodel_name: {}\n",
                model.id,
                table_value(&model.name)
            );
            writeln!(output, "INSTANCE_ID\tNAME\tSTATE\tWORKER\tPORT\tMESSAGE")
                .expect("write to String");
            for instance in instances {
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    instance.id,
                    table_value(&instance.name),
                    table_value(&instance.state.0),
                    table_value(instance.worker_name.as_deref().unwrap_or("-")),
                    instance
                        .port
                        .map_or_else(|| "-".to_owned(), |port| port.to_string()),
                    table_value(instance.state_message.as_deref().unwrap_or(""))
                )
                .expect("write to String");
            }
            output
        }
    };
    success(output)
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelTransition {
    instance_id: i64,
    instance_name: String,
    state: String,
    state_message: Option<String>,
}

impl From<&ModelInstancePublic> for ModelTransition {
    fn from(instance: &ModelInstancePublic) -> Self {
        Self {
            instance_id: instance.id,
            instance_name: instance.name.clone(),
            state: instance.state.0.clone(),
            state_message: instance.state_message.clone(),
        }
    }
}

impl ModelTransition {
    #[must_use]
    pub fn event_line(&self) -> String {
        let message = self
            .state_message
            .as_deref()
            .filter(|message| !message.is_empty())
            .map_or_else(String::new, |message| {
                format!(" message={}", table_value(message))
            });
        format!(
            "model_instance={} name={} state={}{}",
            self.instance_id,
            table_value(&self.instance_name),
            table_value(&self.state),
            message
        )
    }
}

#[derive(Serialize)]
struct ModelDeployView<'a> {
    model: &'a ModelPublic,
    instances: &'a [ModelInstancePublic],
    transitions: &'a [ModelTransition],
}

fn render_model_deploy(
    output_format: OutputFormat,
    model: &ModelPublic,
    instances: &[ModelInstancePublic],
    transitions: &[ModelTransition],
) -> Result<Execution, ManagedError> {
    if matches!(output_format, OutputFormat::Json) {
        return success(to_json(&ModelDeployView {
            model,
            instances,
            transitions,
        })?);
    }
    let mut output = format!(
        "created: true\nmodel_id: {}\nname: {}\n",
        model.id,
        table_value(&model.name)
    );
    for transition in transitions {
        writeln!(
            output,
            "transition: instance={} state={}",
            transition.instance_id,
            table_value(&transition.state)
        )
        .expect("write to String");
    }
    success(output)
}

#[derive(Serialize)]
struct DeletedModelView<'a> {
    deleted: bool,
    id: i64,
    name: &'a str,
}

fn render_deleted_model(
    output_format: OutputFormat,
    model: &ModelPublic,
) -> Result<Execution, ManagedError> {
    let view = DeletedModelView {
        deleted: true,
        id: model.id,
        name: &model.name,
    };
    let output = match output_format {
        OutputFormat::Json => to_json(&view)?,
        OutputFormat::Table => format!(
            "deleted: true\nid: {}\nname: {}\n",
            model.id,
            table_value(&model.name)
        ),
    };
    success(output)
}

#[derive(Serialize)]
struct MaintenanceView<'a> {
    worker_id: i64,
    worker_name: &'a str,
    before: Option<&'a WorkerMaintenance>,
    after: Option<&'a WorkerMaintenance>,
}

fn render_maintenance_change(
    output_format: OutputFormat,
    before: &WorkerPublic,
    after: &WorkerPublic,
) -> Result<Execution, ManagedError> {
    let view = MaintenanceView {
        worker_id: after.id,
        worker_name: &after.name,
        before: before.maintenance.as_ref(),
        after: after.maintenance.as_ref(),
    };
    let output = match output_format {
        OutputFormat::Json => to_json(&view)?,
        OutputFormat::Table => format!(
            "worker_id: {}\nworker_name: {}\nbefore_enabled: {}\nafter_enabled: {}\n",
            after.id,
            table_value(&after.name),
            before
                .maintenance
                .as_ref()
                .is_some_and(|value| value.enabled),
            after
                .maintenance
                .as_ref()
                .is_some_and(|value| value.enabled)
        ),
    };
    success(output)
}

fn to_json(value: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value).map(|output| format!("{output}\n"))
}

fn table_value(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                write!(escaped, "\\u{{{:04x}}}", character as u32).expect("write to String");
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn success(output: String) -> Result<Execution, ManagedError> {
    Ok(Execution {
        output,
        exit_code: EXIT_SUCCESS,
    })
}

#[derive(Debug, Error)]
pub enum ManagedError {
    #[error(transparent)]
    Context(#[from] ContextStoreError),
    #[error(transparent)]
    Client(#[from] ClientError),
    #[error("invalid CLI argument: {0}")]
    InvalidArgument(String),
    #[error("{0}")]
    ConfirmationRequired(&'static str),
    #[error("{0} '{1}' was not found")]
    NotFound(&'static str, String),
    #[error("{0} selector '{1}' is ambiguous")]
    Ambiguous(&'static str, String),
    #[error("Model entered error state while waiting: {states}")]
    ModelFailed { states: String },
    #[error("timed out waiting for Model to run; last states: {states}")]
    WaitTimeout { states: String },
    #[error("failed to serialize command output")]
    Serialization(#[from] serde_json::Error),
}

impl ManagedError {
    #[must_use]
    pub const fn is_control_plane(&self) -> bool {
        matches!(
            self,
            Self::Client(_)
                | Self::NotFound(_, _)
                | Self::Ambiguous(_, _)
                | Self::ModelFailed { .. }
                | Self::WaitTimeout { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use clap::Parser;
    use gpustack_client::http::{
        HttpRequest, HttpResponse, Method, StreamResponse, TransportError,
    };
    use serde_json::{Value, json};
    use tempfile::tempdir;

    use super::*;
    use crate::{Cli, Command, WorkerCommand};

    const MODEL_PAGE: &str =
        include_str!("../../../fixtures/server-v2.2.3/models-full-sync.response.json");
    const INSTANCE_PAGE: &str =
        include_str!("../../../fixtures/server-v2.2.3/model-instances-full-sync.response.json");
    const WORKERS_ME: &str =
        include_str!("../../../fixtures/server-v2.2.3/workers-me.response.json");

    #[derive(Clone, Default)]
    struct MockTransport {
        state: Arc<Mutex<MockState>>,
    }

    #[derive(Default)]
    struct MockState {
        requests: Vec<HttpRequest>,
        responses: VecDeque<Result<HttpResponse, TransportError>>,
    }

    impl MockTransport {
        fn push_json(&self, status: u16, body: impl Into<Vec<u8>>) {
            self.state
                .lock()
                .expect("mock lock")
                .responses
                .push_back(Ok(HttpResponse {
                    status,
                    content_type: Some("application/json".to_owned()),
                    body: body.into(),
                }));
        }

        fn requests(&self) -> Vec<HttpRequest> {
            self.state.lock().expect("mock lock").requests.clone()
        }
    }

    impl Transport for MockTransport {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, TransportError> {
            let mut state = self.state.lock().expect("mock lock");
            state.requests.push(request);
            state
                .responses
                .pop_front()
                .unwrap_or_else(|| Err(TransportError::Request("missing response".to_owned())))
        }

        async fn open_stream(
            &self,
            _request: HttpRequest,
        ) -> Result<StreamResponse, TransportError> {
            Err(TransportError::Request("unexpected stream".to_owned()))
        }
    }

    fn item_from_page(raw: &str) -> Vec<u8> {
        let value: Value = serde_json::from_str(raw).expect("page JSON");
        serde_json::to_vec(&value["items"][0]).expect("item JSON")
    }

    fn worker_item_with_maintenance(enabled: bool) -> Vec<u8> {
        let mut value: Value = serde_json::from_str(WORKERS_ME).expect("Worker page JSON");
        value["items"][0]["maintenance"] = json!({
            "enabled": enabled,
            "message": enabled.then_some("planned maintenance")
        });
        serde_json::to_vec(&value["items"][0]).expect("Worker item JSON")
    }

    #[test]
    fn complete_managed_command_surface_parses() {
        for command in [
            vec![
                "gpustack-rs",
                "context",
                "add",
                "prod",
                "--server-url",
                "http://127.0.0.1",
                "--token-file",
                "/run/secrets/operator",
            ],
            vec!["gpustack-rs", "context", "use", "prod"],
            vec!["gpustack-rs", "context", "list"],
            vec!["gpustack-rs", "context", "remove", "prod", "--yes"],
            vec![
                "gpustack-rs",
                "model",
                "deploy",
                "demo",
                "--worker",
                "rust-worker",
                "--local-path",
                "/models/demo",
                "--image",
                "runner@sha256:aaaa",
                "--gpu",
                "0",
            ],
            vec!["gpustack-rs", "model", "list"],
            vec!["gpustack-rs", "model", "get", "demo"],
            vec!["gpustack-rs", "model", "status", "demo"],
            vec!["gpustack-rs", "model", "delete", "demo", "--yes"],
            vec![
                "gpustack-rs",
                "worker",
                "maintenance",
                "enable",
                "rust-worker",
            ],
            vec![
                "gpustack-rs",
                "worker",
                "maintenance",
                "disable",
                "rust-worker",
                "--yes",
            ],
        ] {
            Cli::try_parse_from(command).expect("parse managed command");
        }
    }

    #[test]
    fn table_values_escape_terminal_control_characters() {
        assert_eq!(
            table_value("safe\tline\n\u{001b}"),
            "safe\\tline\\n\\u{001b}"
        );
    }

    #[test]
    fn context_commands_persist_stable_state_without_credentials() {
        let temporary = tempdir().expect("temporary directory");
        let store = ContextStore::new(temporary.path().join("contexts.json"));
        let token_file = temporary.path().join("operator.token");

        let added = run_context_command(
            OutputFormat::Json,
            &store,
            ContextArgs {
                command: ContextSubcommand::Add(ContextAddArgs {
                    name: "test".to_owned(),
                    server_url: Url::parse("http://127.0.0.1/base").expect("URL"),
                    token_file: token_file.clone(),
                    use_now: true,
                }),
            },
        )
        .expect("add context");
        let added: Value = serde_json::from_str(&added.output).expect("add output JSON");
        assert_eq!(added["name"], "test");
        assert_eq!(added["active"], true);

        let selected = run_context_command(
            OutputFormat::Json,
            &store,
            ContextArgs {
                command: ContextSubcommand::Use(ContextNameArgs {
                    name: "test".to_owned(),
                }),
            },
        )
        .expect("use context");
        let selected: Value = serde_json::from_str(&selected.output).expect("use output JSON");
        assert_eq!(selected["active"], true);

        let listed = run_context_command(
            OutputFormat::Table,
            &store,
            ContextArgs {
                command: ContextSubcommand::List,
            },
        )
        .expect("list contexts");
        assert_eq!(
            listed.output,
            format!(
                "ACTIVE\tNAME\tSERVER\tTOKEN_FILE\n*\ttest\thttp://127.0.0.1/base\t{}\n",
                token_file.display()
            )
        );

        let error = run_context_command(
            OutputFormat::Json,
            &store,
            ContextArgs {
                command: ContextSubcommand::Remove(ContextRemoveArgs {
                    name: "test".to_owned(),
                    yes: false,
                }),
            },
        )
        .expect_err("remove confirmation");
        assert!(matches!(error, ManagedError::ConfirmationRequired(_)));
        assert_eq!(store.resolve(None).expect("context remains").name, "test");

        run_context_command(
            OutputFormat::Json,
            &store,
            ContextArgs {
                command: ContextSubcommand::Remove(ContextRemoveArgs {
                    name: "test".to_owned(),
                    yes: true,
                }),
            },
        )
        .expect("remove context");
        let stored = std::fs::read_to_string(store.path()).expect("read context store");
        assert!(!stored.contains("operator-secret"));
    }

    #[tokio::test]
    async fn deploy_builds_an_explicit_local_vllm_request_without_runtime_access() {
        let transport = MockTransport::default();
        transport.push_json(200, item_from_page(MODEL_PAGE));
        let client = ServerClient::new(
            Url::parse("http://127.0.0.1/base").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let token = SecretToken::new("operator-secret").expect("token");
        let cli = Cli::try_parse_from([
            "gpustack-rs",
            "model",
            "deploy",
            "demo",
            "--worker",
            "rust-worker",
            "--local-path",
            "/models/demo",
            "--image",
            "runner@sha256:aaaa",
            "--gpu",
            "0",
            "--max-model-len",
            "2048",
            "--enforce-eager",
        ])
        .expect("parse deploy");
        let Command::Model(arguments) = cli.command else {
            panic!("model command");
        };

        run_model_command(
            OutputFormat::Json,
            Duration::from_secs(1),
            &client,
            &token,
            arguments,
            None,
        )
        .await
        .expect("deploy");

        let requests = transport.requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].url.path(), "/base/v2/models");
        let body: Value =
            serde_json::from_slice(requests[0].body.as_deref().expect("request body"))
                .expect("request JSON");
        assert_eq!(body["source"], "local_path");
        assert_eq!(body["worker_selector"]["worker-name"], "rust-worker");
        assert_eq!(body["gpu_selector"]["gpu_ids"][0], "rust-worker:cuda:0");
        assert_eq!(body["enable_model_route"], false);
        assert_eq!(
            body["backend_parameters"],
            json!([
                "--tensor-parallel-size=1",
                "--max-model-len=2048",
                "--enforce-eager"
            ])
        );
        assert!(!format!("{requests:?}").contains("operator-secret"));
    }

    #[tokio::test]
    async fn list_get_and_delete_use_managed_model_endpoints() {
        let transport = MockTransport::default();
        transport.push_json(200, MODEL_PAGE);
        transport.push_json(200, MODEL_PAGE);
        transport.push_json(200, item_from_page(MODEL_PAGE));
        transport.push_json(200, Vec::new());
        let client = ServerClient::new(
            Url::parse("http://127.0.0.1/base").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let token = SecretToken::new("operator-secret").expect("token");

        let listed = run_model_command(
            OutputFormat::Table,
            Duration::from_secs(1),
            &client,
            &token,
            ModelArgs {
                command: ModelSubcommand::List,
            },
            None,
        )
        .await
        .expect("list models");
        assert!(
            listed
                .output
                .starts_with("ID\tNAME\tSOURCE\tBACKEND\tLOCAL_PATH\n")
        );

        let fetched = run_model_command(
            OutputFormat::Json,
            Duration::from_secs(1),
            &client,
            &token,
            ModelArgs {
                command: ModelSubcommand::Get(ModelSelectorArgs {
                    model: "contract-qwen".to_owned(),
                }),
            },
            None,
        )
        .await
        .expect("get model");
        let fetched: Value = serde_json::from_str(&fetched.output).expect("get output JSON");
        assert_eq!(fetched["id"], 201);

        let error = run_model_command(
            OutputFormat::Json,
            Duration::from_secs(1),
            &client,
            &token,
            ModelArgs {
                command: ModelSubcommand::Delete(ModelDeleteArgs {
                    model: "201".to_owned(),
                    yes: false,
                }),
            },
            None,
        )
        .await
        .expect_err("delete confirmation");
        assert!(matches!(error, ManagedError::ConfirmationRequired(_)));
        assert_eq!(transport.requests().len(), 2);

        let deleted = run_model_command(
            OutputFormat::Json,
            Duration::from_secs(1),
            &client,
            &token,
            ModelArgs {
                command: ModelSubcommand::Delete(ModelDeleteArgs {
                    model: "201".to_owned(),
                    yes: true,
                }),
            },
            None,
        )
        .await
        .expect("delete model");
        let deleted: Value = serde_json::from_str(&deleted.output).expect("delete output JSON");
        assert_eq!(
            deleted,
            json!({"deleted": true, "id": 201, "name": "contract-qwen"})
        );

        let requests = transport.requests();
        assert_eq!(requests.len(), 4);
        assert_eq!(
            requests[0].url.as_str(),
            "http://127.0.0.1/base/v2/models?page=-1"
        );
        assert_eq!(
            requests[1].url.as_str(),
            "http://127.0.0.1/base/v2/models?page=-1&search=contract-qwen"
        );
        assert_eq!(requests[2].url.path(), "/base/v2/models/201");
        assert_eq!(requests[3].method, Method::Delete);
        assert_eq!(requests[3].url.path(), "/base/v2/models/201");
        assert!(!format!("{requests:?}").contains("operator-secret"));
    }

    #[tokio::test]
    async fn status_uses_typed_model_and_instance_queries() {
        let transport = MockTransport::default();
        transport.push_json(200, item_from_page(MODEL_PAGE));
        transport.push_json(200, INSTANCE_PAGE);
        let client = ServerClient::new(
            Url::parse("http://127.0.0.1").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let token = SecretToken::new("operator-secret").expect("token");
        let arguments = ModelArgs {
            command: ModelSubcommand::Status(ModelSelectorArgs {
                model: "201".to_owned(),
            }),
        };

        let execution = run_model_command(
            OutputFormat::Json,
            Duration::from_secs(1),
            &client,
            &token,
            arguments,
            None,
        )
        .await
        .expect("status");
        let output: Value = serde_json::from_str(&execution.output).expect("output JSON");
        assert_eq!(output["model"]["id"], 201);
        assert_eq!(output["instances"][0]["state"], "scheduled");
        let requests = transport.requests();
        assert_eq!(requests[0].url.path(), "/v2/models/201");
        assert_eq!(
            requests[1].url.as_str(),
            "http://127.0.0.1/v2/model-instances?page=-1&model_id=201"
        );
    }

    #[tokio::test]
    async fn deploy_wait_reports_each_observed_state_transition() {
        let transport = MockTransport::default();
        transport.push_json(200, item_from_page(MODEL_PAGE));
        transport.push_json(200, INSTANCE_PAGE);
        let mut running: Value = serde_json::from_str(INSTANCE_PAGE).expect("instance page JSON");
        running["items"][0]["state"] = json!("running");
        transport.push_json(
            200,
            serde_json::to_vec(&running).expect("running instance page"),
        );
        let client = ServerClient::new(Url::parse("http://127.0.0.1").expect("URL"), transport)
            .expect("client");
        let token = SecretToken::new("operator-secret").expect("token");
        let arguments = ModelArgs {
            command: ModelSubcommand::Deploy(ModelDeployArgs {
                name: "demo".to_owned(),
                worker: "rust-worker".to_owned(),
                local_path: PathBuf::from("/models/demo"),
                image: "runner@sha256:aaaa".to_owned(),
                gpu_indexes: vec![0],
                tensor_parallel_size: None,
                max_model_len: None,
                gpu_memory_utilization: None,
                dtype: None,
                trust_remote_code: false,
                enforce_eager: false,
                wait: true,
            }),
        };

        let mut observed = Vec::new();
        let execution = {
            let mut transition_sink = |transition: &ModelTransition| {
                observed.push(transition.state.clone());
            };
            run_model_command(
                OutputFormat::Json,
                Duration::from_secs(2),
                &client,
                &token,
                arguments,
                Some(&mut transition_sink),
            )
            .await
            .expect("wait for model")
        };
        let output: Value = serde_json::from_str(&execution.output).expect("deploy output JSON");
        assert_eq!(output["transitions"][0]["state"], "scheduled");
        assert_eq!(output["transitions"][1]["state"], "running");
        assert_eq!(observed, ["scheduled", "running"]);
    }

    #[tokio::test]
    async fn maintenance_disable_requires_confirmation_before_transport() {
        let transport = MockTransport::default();
        let client = ServerClient::new(
            Url::parse("http://127.0.0.1").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let token = SecretToken::new("operator-secret").expect("token");
        let arguments = WorkerMaintenanceArgs {
            command: WorkerMaintenanceSubcommand::Disable(MaintenanceDisableArgs {
                worker: "rust-worker".to_owned(),
                yes: false,
            }),
        };

        let error = run_worker_maintenance_command(OutputFormat::Json, &client, &token, arguments)
            .await
            .expect_err("confirmation required");
        assert!(matches!(error, ManagedError::ConfirmationRequired(_)));
        assert!(transport.requests().is_empty());
    }

    #[tokio::test]
    async fn maintenance_enable_preserves_worker_name_and_labels() {
        let transport = MockTransport::default();
        transport.push_json(200, WORKERS_ME);
        transport.push_json(200, worker_item_with_maintenance(true));
        let client = ServerClient::new(
            Url::parse("http://127.0.0.1/base").expect("URL"),
            transport.clone(),
        )
        .expect("client");
        let token = SecretToken::new("operator-secret").expect("token");
        let arguments = WorkerMaintenanceArgs {
            command: WorkerMaintenanceSubcommand::Enable(MaintenanceEnableArgs {
                worker: "rust-worker-contract".to_owned(),
                message: Some("planned maintenance".to_owned()),
            }),
        };

        let execution =
            run_worker_maintenance_command(OutputFormat::Json, &client, &token, arguments)
                .await
                .expect("enable maintenance");
        let output: Value = serde_json::from_str(&execution.output).expect("maintenance JSON");
        assert_eq!(output["before"], Value::Null);
        assert_eq!(output["after"]["enabled"], true);

        let requests = transport.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(
            requests[0].url.as_str(),
            "http://127.0.0.1/base/v2/workers?page=-1&name=rust-worker-contract"
        );
        assert_eq!(requests[1].method, Method::Put);
        assert_eq!(requests[1].url.path(), "/base/v2/workers/101");
        let update: Value =
            serde_json::from_slice(requests[1].body.as_deref().expect("update body"))
                .expect("update JSON");
        assert_eq!(update["name"], "rust-worker-contract");
        assert_eq!(update["labels"], json!({"arch": "x86_64", "os": "linux"}));
        assert_eq!(update["maintenance"]["enabled"], true);
        assert!(!format!("{requests:?}").contains("operator-secret"));
    }

    #[test]
    fn worker_maintenance_command_is_nested_under_worker() {
        let cli = Cli::try_parse_from([
            "gpustack-rs",
            "worker",
            "maintenance",
            "enable",
            "rust-worker",
        ])
        .expect("parse maintenance");
        let Command::Worker(worker) = cli.command else {
            panic!("worker command");
        };
        assert!(matches!(
            worker.command,
            WorkerCommand::Maintenance(WorkerMaintenanceArgs { .. })
        ));
    }
}
