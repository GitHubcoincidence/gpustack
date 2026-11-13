use std::{env, path::PathBuf, process::ExitCode, time::Duration};

use gpustack_client::{Context, ServerClient, http::ReqwestTransport, load_context_token};
use serde_json::json;
use url::Url;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Worker uniqueness audit failed: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = env::args_os();
    let program = arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .unwrap_or_else(|| "m8rs_worker_uniqueness".to_owned());
    let values: Vec<_> = arguments.collect();
    if values.len() != 5 {
        return Err(format!(
            "usage: {program} <server-url> <token-file> <worker-name> <cluster-id> <worker-id>"
        )
        .into());
    }

    let server_url = Url::parse(&values[0].to_string_lossy())?;
    let token_file = PathBuf::from(&values[1]);
    let worker_name = values[2].to_string_lossy().into_owned();
    let cluster_id: i64 = values[3].to_string_lossy().parse()?;
    let expected_worker_id: i64 = values[4].to_string_lossy().parse()?;
    let context = Context {
        name: "m8rs-worker-uniqueness".to_owned(),
        server_url: server_url.clone(),
        token_file,
    };
    let token = load_context_token(&context)?;
    let transport = ReqwestTransport::new()?;
    let client = ServerClient::with_timeout(server_url, transport, Duration::from_secs(30))?;
    let page = client.list_workers(&token).await?;
    let total_items = page.items.len();
    let exact: Vec<_> = page
        .items
        .into_iter()
        .filter(|worker| worker.name == worker_name && worker.cluster_id == Some(cluster_id))
        .collect();

    if exact.len() != 1 {
        return Err(format!("expected one exact Worker, found {}", exact.len()).into());
    }
    let worker = &exact[0];
    if worker.id != expected_worker_id {
        return Err(format!(
            "expected Worker ID {expected_worker_id}, found {}",
            worker.id
        )
        .into());
    }

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "request_method": "GET",
            "request_path": "/v2/workers?page=-1",
            "total_items": total_items,
            "pagination_total": page.pagination.total,
            "exact_match_count": exact.len(),
            "worker": {
                "id": worker.id,
                "name": worker.name,
                "cluster_id": worker.cluster_id,
                "labels": worker.labels,
                "maintenance": worker.maintenance,
                "worker_version": worker.worker_version,
            }
        }))?
    );
    Ok(())
}
