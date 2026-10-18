#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    Interrupt,
    Terminate,
}

#[cfg(unix)]
pub async fn wait_for_shutdown() -> std::io::Result<ShutdownSignal> {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = signal(SignalKind::terminate())?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            result?;
            Ok(ShutdownSignal::Interrupt)
        }
        _ = terminate.recv() => Ok(ShutdownSignal::Terminate),
    }
}

#[cfg(not(unix))]
pub async fn wait_for_shutdown() -> std::io::Result<ShutdownSignal> {
    tokio::signal::ctrl_c().await?;
    Ok(ShutdownSignal::Interrupt)
}
