//! Minimal unauthenticated liveness endpoint for Worker discovery.

use std::future::Future;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio::task::JoinSet;

const MAX_REQUEST_BYTES: usize = 8 * 1024;
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);
const HEALTH_RESPONSE: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 3\r\nConnection: close\r\n\r\nok\n";
const NOT_FOUND_RESPONSE: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: 10\r\nConnection: close\r\n\r\nnot found\n";

pub type LivenessFuture = Pin<Box<dyn Future<Output = io::Result<()>> + Send>>;

/// A bound liveness endpoint that can be replaced by a no-network test double.
pub trait LivenessServer: Send {
    fn local_addr(&self) -> SocketAddr;

    fn serve(self, shutdown: watch::Receiver<bool>) -> LivenessFuture;
}

pub struct HealthServer {
    listener: TcpListener,
    local_addr: SocketAddr,
}

impl HealthServer {
    pub async fn bind(address: SocketAddr) -> io::Result<Self> {
        let listener = TcpListener::bind(address).await?;
        let local_addr = listener.local_addr()?;
        Ok(Self {
            listener,
            local_addr,
        })
    }
}

impl LivenessServer for HealthServer {
    fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    fn serve(self, mut shutdown: watch::Receiver<bool>) -> LivenessFuture {
        Box::pin(async move {
            let mut connections = JoinSet::new();
            loop {
                tokio::select! {
                    accepted = self.listener.accept() => {
                        let (stream, _) = accepted?;
                        connections.spawn(serve_connection(stream));
                    }
                    completed = connections.join_next(), if !connections.is_empty() => {
                        // Client I/O failures are isolated to that probe. A panicked
                        // handler is fatal because it indicates an internal fault.
                        if let Some(Err(error)) = completed
                            && !error.is_cancelled()
                        {
                            return Err(io::Error::other(error));
                        }
                    }
                    changed = shutdown.changed() => {
                        if changed.is_err() || *shutdown.borrow() {
                            break;
                        }
                    }
                }
            }

            connections.abort_all();
            while connections.join_next().await.is_some() {}
            Ok(())
        })
    }
}

async fn serve_connection(mut stream: TcpStream) -> io::Result<()> {
    let response = tokio::time::timeout(CONNECTION_TIMEOUT, read_request(&mut stream))
        .await
        .map_or(NOT_FOUND_RESPONSE, |result| {
            result.map_or(NOT_FOUND_RESPONSE, |request| response_for(&request))
        });
    stream.write_all(response).await?;
    stream.shutdown().await
}

async fn read_request(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut request = Vec::with_capacity(512);
    while request.len() < MAX_REQUEST_BYTES {
        let mut chunk = [0_u8; 512];
        let remaining = MAX_REQUEST_BYTES - request.len();
        let limit = remaining.min(chunk.len());
        let read = stream.read(&mut chunk[..limit]).await?;
        if read == 0 {
            break;
        }
        request.extend_from_slice(&chunk[..read]);
        if request.windows(2).any(|window| window == b"\r\n") {
            break;
        }
    }
    Ok(request)
}

fn response_for(request: &[u8]) -> &'static [u8] {
    let first_line = request
        .split(|byte| *byte == b'\n')
        .next()
        .and_then(|line| std::str::from_utf8(line).ok());
    match first_line.map(str::trim_end) {
        Some("GET /healthz HTTP/1.1" | "GET /healthz HTTP/1.0") => HEALTH_RESPONSE,
        _ => NOT_FOUND_RESPONSE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthz_is_the_only_successful_route() {
        assert!(response_for(b"GET /healthz HTTP/1.1\r\n").starts_with(b"HTTP/1.1 200"));
        assert!(response_for(b"GET /readyz HTTP/1.1\r\n").starts_with(b"HTTP/1.1 404"));
        assert!(response_for(b"POST /healthz HTTP/1.1\r\n").starts_with(b"HTTP/1.1 404"));
    }
}
