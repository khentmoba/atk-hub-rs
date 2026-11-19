//! Live Windows named-pipe client with request/response correlation.
//!
//! The agent serves a single client connection at a time, so one handle is
//! shared by the reader thread and the writer. Reads are bounded by a short
//! timeout so the lock is released promptly between frames.

use super::{AgentError, Frame, FramedBuffer, MAX_PENDING, PipeName, REQUEST_TIMEOUT, Request};
use atk_ipc::is_agent_method;
use named_pipe::PipeClient;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

/// How long to wait for the agent pipe to become available.
const CONNECT_TIMEOUT_MS: u32 = 5_000;
/// Read polling interval; bounded so shutdown and writes stay responsive.
const READ_TIMEOUT: Duration = Duration::from_millis(100);
/// Size of each raw pipe read.
const READ_CHUNK: usize = 8 * 1024;

type Pending = HashMap<u64, Sender<Result<Value, String>>>;

/// A connected client for the ATK HUB agent pipe.
pub struct AgentClient {
    pipe: Arc<Mutex<PipeClient>>,
    pending: Arc<Mutex<Pending>>,
    event_senders: Arc<Mutex<Vec<Sender<Value>>>>,
    stopped: Arc<AtomicBool>,
    next_id: AtomicU64,
    reader: Option<JoinHandle<()>>,
}

impl AgentClient {
    /// Connects to the validated session pipe and starts the reader thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the pipe cannot be opened or the reader thread
    /// cannot be started.
    pub fn connect(pipe: &PipeName) -> Result<Self, AgentError> {
        let mut pipe = PipeClient::connect_ms(pipe.windows_path(), CONNECT_TIMEOUT_MS)
            .map_err(AgentError::Connect)?;
        pipe.set_read_timeout(Some(READ_TIMEOUT));
        pipe.set_write_timeout(Some(REQUEST_TIMEOUT));

        let pipe = Arc::new(Mutex::new(pipe));
        let pending: Arc<Mutex<Pending>> = Arc::new(Mutex::new(HashMap::new()));
        let event_senders: Arc<Mutex<Vec<Sender<Value>>>> = Arc::new(Mutex::new(Vec::new()));
        let stopped = Arc::new(AtomicBool::new(false));

        let reader_pipe = Arc::clone(&pipe);
        let reader_pending = Arc::clone(&pending);
        let reader_events = Arc::clone(&event_senders);
        let reader_stopped = Arc::clone(&stopped);
        let reader = thread::Builder::new()
            .name("atk-agent-reader".to_owned())
            .spawn(move || {
                let mut buffer = FramedBuffer::default();
                let mut chunk = [0_u8; READ_CHUNK];

                while !reader_stopped.load(Ordering::Relaxed) {
                    let read = match reader_pipe.lock() {
                        Ok(mut client) => client.read(&mut chunk),
                        Err(_) => break,
                    };
                    match read {
                        Ok(0) => break,
                        Ok(count) => {
                            let frames = match buffer.push(&chunk[..count]) {
                                Ok(frames) => frames,
                                Err(_) => break,
                            };
                            for frame in frames {
                                match frame {
                                    Frame::Response {
                                        id,
                                        ok,
                                        result,
                                        error,
                                    } => {
                                        let sender = reader_pending
                                            .lock()
                                            .expect("pending lock poisoned")
                                            .remove(&id);
                                        if let Some(sender) = sender {
                                            let payload = if ok {
                                                Ok(result)
                                            } else {
                                                Err(error.unwrap_or_else(|| {
                                                    "agent request failed".to_owned()
                                                }))
                                            };
                                            let _ = sender.send(payload);
                                        }
                                    }
                                    Frame::Event { payload, .. } => {
                                        let mut senders =
                                            reader_events.lock().expect("events lock poisoned");
                                        senders
                                            .retain(|sender| sender.send(payload.clone()).is_ok());
                                    }
                                }
                            }
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                            ) =>
                        {
                            continue;
                        }
                        Err(_) => break,
                    }
                }

                let mut pending = reader_pending.lock().expect("pending lock poisoned");
                for (_, sender) in pending.drain() {
                    let _ = sender.send(Err("agent pipe closed".to_owned()));
                }
            })
            .map_err(AgentError::Connect)?;

        Ok(Self {
            pipe,
            pending,
            event_senders,
            stopped,
            next_id: AtomicU64::new(0),
            reader: Some(reader),
        })
    }

    /// Sends an allowlisted agent method and waits for its correlated response.
    ///
    /// # Errors
    ///
    /// Returns an error for a disallowed method, too many in-flight requests,
    /// a write failure, a timeout, or a reported agent error.
    pub fn request(&self, method: &str, params: Value) -> Result<Value, AgentError> {
        if !is_agent_method(method) {
            return Err(AgentError::Remote {
                message: format!("Unsupported Rust Agent method: {method}"),
            });
        }

        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        let (sender, receiver) = mpsc::channel();
        {
            let mut pending = self.pending.lock().expect("pending lock poisoned");
            if pending.len() >= MAX_PENDING {
                return Err(AgentError::TooManyPending { len: pending.len() });
            }
            pending.insert(id, sender);
        }

        let encoded = Request::new(id, method, params).encode()?;
        let written = self
            .pipe
            .lock()
            .expect("pipe lock poisoned")
            .write_all(&encoded);
        if let Err(error) = written {
            self.pending
                .lock()
                .expect("pending lock poisoned")
                .remove(&id);
            return Err(AgentError::Io(error));
        }

        match receiver.recv_timeout(REQUEST_TIMEOUT) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(message)) => {
                self.pending
                    .lock()
                    .expect("pending lock poisoned")
                    .remove(&id);
                Err(AgentError::Remote { message })
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                self.pending
                    .lock()
                    .expect("pending lock poisoned")
                    .remove(&id);
                Err(AgentError::RequestTimeout {
                    method: method.to_owned(),
                })
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(AgentError::Closed),
        }
    }

    /// Subscribes to unsolicited agent events.
    #[must_use]
    pub fn events(&self) -> Receiver<Value> {
        let (sender, receiver) = mpsc::channel();
        self.event_senders
            .lock()
            .expect("events lock poisoned")
            .push(sender);
        receiver
    }
}

impl fmt::Debug for AgentClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AgentClient")
            .field(
                "pending",
                &self.pending.lock().map_or(0, |pending| pending.len()),
            )
            .field("stopped", &self.stopped.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl Drop for AgentClient {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
