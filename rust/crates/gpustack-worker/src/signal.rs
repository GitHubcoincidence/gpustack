//! Graceful process shutdown signal handling.

/// Operating-system signal that requested Worker shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// Interactive interrupt, normally `SIGINT` or Ctrl+C.
    Interrupt,
    /// Service-manager termination request, normally `SIGTERM`.
    Terminate,
}

/// Waits until the process receives an interrupt or termination signal.
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

/// Waits until the process receives the platform interrupt signal.
#[cfg(not(unix))]
pub async fn wait_for_shutdown() -> std::io::Result<ShutdownSignal> {
    tokio::signal::ctrl_c().await?;
    Ok(ShutdownSignal::Interrupt)
}
