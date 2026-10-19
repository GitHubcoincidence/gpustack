//! Decoder and connection state for GPUStack JSON-line watch streams.

use std::{collections::VecDeque, marker::PhantomData, time::Duration};

use futures_util::StreamExt;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{ClientError, http::ByteStream};

const DEFAULT_MAX_WATCH_LINE_BYTES: usize = 1024 * 1024;
const WATCH_IDLE_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchEventType {
    Created,
    Updated,
    Deleted,
    Unknown,
    Heartbeat,
}

impl TryFrom<u8> for WatchEventType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Created),
            2 => Ok(Self::Updated),
            3 => Ok(Self::Deleted),
            4 => Ok(Self::Unknown),
            5 => Ok(Self::Heartbeat),
            _ => Err(format!("unsupported watch event type {value}")),
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawWatchEvent {
    #[serde(rename = "type")]
    kind: u8,
    data: Value,
    id: i64,
}

#[derive(Debug, Clone, PartialEq)]
/// Resource event decoded from GPUStack's JSON-line watch protocol.
pub enum WatchEvent<T> {
    Created(T),
    Updated(T),
    Deleted { id: i64 },
    Unknown { id: i64 },
    Heartbeat,
}

impl RawWatchEvent {
    fn decode<T: DeserializeOwned>(self) -> Result<WatchEvent<T>, ClientError> {
        let kind = WatchEventType::try_from(self.kind).map_err(ClientError::Watch)?;
        match kind {
            WatchEventType::Created => serde_json::from_value(self.data)
                .map(WatchEvent::Created)
                .map_err(|source| ClientError::Decode {
                    context: "watch CREATED event",
                    source,
                }),
            WatchEventType::Updated => serde_json::from_value(self.data)
                .map(WatchEvent::Updated)
                .map_err(|source| ClientError::Decode {
                    context: "watch UPDATED event",
                    source,
                }),
            WatchEventType::Deleted => {
                // v2.2.3 delete events can carry only an envelope ID. Prefer a
                // body ID when present but preserve the documented fallback.
                let id = self
                    .data
                    .get("id")
                    .and_then(Value::as_i64)
                    .unwrap_or(self.id);
                Ok(WatchEvent::Deleted { id })
            }
            WatchEventType::Unknown => Ok(WatchEvent::Unknown { id: self.id }),
            WatchEventType::Heartbeat => Ok(WatchEvent::Heartbeat),
        }
    }
}

#[derive(Debug)]
struct WatchLineDecoder {
    buffer: Vec<u8>,
    max_line_bytes: usize,
}

impl WatchLineDecoder {
    fn new(max_line_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_line_bytes,
        }
    }

    fn push<T: DeserializeOwned>(
        &mut self,
        chunk: &[u8],
    ) -> Result<Vec<WatchEvent<T>>, ClientError> {
        // The endpoint advertises text/event-stream but emits one raw JSON
        // object per line plus blank keepalives, not standard `data:` SSE frames.
        self.buffer.extend_from_slice(chunk);
        if self.buffer.len() > self.max_line_bytes && !self.buffer.contains(&b'\n') {
            return Err(ClientError::Watch(format!(
                "watch line exceeded {} bytes",
                self.max_line_bytes
            )));
        }

        let mut events = Vec::new();
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=newline).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                continue;
            }
            if line.len() > self.max_line_bytes {
                return Err(ClientError::Watch(format!(
                    "watch line exceeded {} bytes",
                    self.max_line_bytes
                )));
            }
            let raw: RawWatchEvent =
                serde_json::from_slice(&line).map_err(|source| ClientError::Decode {
                    context: "watch event line",
                    source,
                })?;
            events.push(raw.decode()?);
        }
        Ok(events)
    }

    fn finish<T: DeserializeOwned>(&mut self) -> Result<Vec<WatchEvent<T>>, ClientError> {
        if self.buffer.iter().all(u8::is_ascii_whitespace) {
            self.buffer.clear();
            return Ok(Vec::new());
        }
        let mut trailing = std::mem::take(&mut self.buffer);
        trailing.push(b'\n');
        self.push(&trailing)
    }
}

pub struct WatchConnection<T> {
    chunks: ByteStream,
    decoder: WatchLineDecoder,
    pending: VecDeque<WatchEvent<T>>,
    ended: bool,
    marker: PhantomData<T>,
}

impl<T: DeserializeOwned> WatchConnection<T> {
    pub(crate) fn new(chunks: ByteStream) -> Self {
        Self {
            chunks,
            decoder: WatchLineDecoder::new(DEFAULT_MAX_WATCH_LINE_BYTES),
            pending: VecDeque::new(),
            ended: false,
            marker: PhantomData,
        }
    }

    /// Returns the next event and fails idle streams after three keepalive periods.
    pub async fn next_event(&mut self) -> Result<Option<WatchEvent<T>>, ClientError> {
        loop {
            if let Some(event) = self.pending.pop_front() {
                return Ok(Some(event));
            }
            if self.ended {
                return Ok(None);
            }
            let next_chunk = tokio::time::timeout(WATCH_IDLE_TIMEOUT, self.chunks.next())
                .await
                .map_err(|_| {
                    ClientError::Watch("watch stream was idle for more than 45 seconds".to_owned())
                })?;
            match next_chunk {
                Some(Ok(chunk)) => {
                    self.pending.extend(self.decoder.push(&chunk)?);
                }
                Some(Err(error)) => return Err(ClientError::Transport(error)),
                None => {
                    self.pending.extend(self.decoder.finish()?);
                    self.ended = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Item {
        id: i64,
    }

    #[test]
    fn decoder_handles_blank_lines_chunk_boundaries_and_id_only_delete() {
        let mut decoder = WatchLineDecoder::new(1024);
        let first = decoder
            .push::<Item>(b"{\"type\":1,\"data\":{\"id\":7},\"id\":7}\n\n{\"type\":3,")
            .expect("first chunk");
        assert_eq!(first, vec![WatchEvent::Created(Item { id: 7 })]);

        let second = decoder
            .push::<Item>(b"\"data\":{\"id\":7},\"id\":7}\n")
            .expect("second chunk");
        assert_eq!(second, vec![WatchEvent::Deleted { id: 7 }]);
    }
}
