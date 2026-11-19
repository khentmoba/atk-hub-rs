//! Wire protocol for ATK HUB's local agent named pipe.
//!
//! Requests and responses are newline-delimited JSON frames, capped at
//! 256 KiB, on `\\.\pipe\atk-hub-agent-session-<session id>`.

use serde_json::Value;
use std::str;
use std::time::Duration;
use thiserror::Error;

#[cfg(windows)]
mod client;
#[cfg(windows)]
mod control;
#[cfg(windows)]
pub use client::AgentClient;
#[cfg(windows)]
pub use control::{AgentState, DeviceId, Rule, RuleAction};

/// Maximum size of one JSON frame, including the trailing newline.
pub const MAX_FRAME_BYTES: usize = 256 * 1024;
/// Maximum number of in-flight requests kept by the main process.
pub const MAX_PENDING: usize = 64;
/// Timeout applied to a single agent request.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
/// Fixed prefix of a session pipe name.
pub const PIPE_NAME_PREFIX: &str = "atk-hub-agent-session-";
/// Windows named-pipe device path prefix.
pub const PIPE_PATH_PREFIX: &str = r"\\.\pipe\";

/// Failures while encoding, decoding or naming agent frames.
#[derive(Debug, Error)]
pub enum AgentError {
    /// A frame exceeded the 256 KiB limit.
    #[error("agent frame exceeds {MAX_FRAME_BYTES} bytes: {len}")]
    OversizedFrame {
        /// Observed frame length.
        len: usize,
    },
    /// A frame was not valid UTF-8.
    #[error("agent frame is not valid UTF-8")]
    InvalidUtf8(#[from] str::Utf8Error),
    /// A frame was not valid JSON.
    #[error("agent frame JSON is invalid")]
    Json(#[from] serde_json::Error),
    /// A response frame had no usable request id.
    #[error("agent response has no numeric id")]
    MissingId,
    /// A pipe name did not match the expected session pattern.
    #[error("invalid agent pipe name: {name}")]
    InvalidPipeName {
        /// Rejected pipe name.
        name: String,
    },
    /// The pipe could not be opened.
    #[error("agent pipe connection failed")]
    Connect(#[source] std::io::Error),
    /// A pipe read or write failed.
    #[error("agent pipe I/O failed")]
    Io(#[from] std::io::Error),
    /// No response arrived within the request timeout.
    #[error("agent request timed out: {method}")]
    RequestTimeout {
        /// Method that timed out.
        method: String,
    },
    /// The agent reported a failed request.
    #[error("agent request failed: {message}")]
    Remote {
        /// Error message returned by the agent.
        message: String,
    },
    /// The reader thread ended before a response arrived.
    #[error("agent pipe closed before a response arrived")]
    Closed,
    /// Too many in-flight requests.
    #[error("too many pending agent requests: {len}")]
    TooManyPending {
        /// Current number of in-flight requests.
        len: usize,
    },
    /// An output report was empty or too large.
    #[error("output report must be 1..65536 bytes: {len}")]
    InvalidReportLength {
        /// Report length including the report id.
        len: usize,
    },
    /// A binding scope was empty or contained unsupported characters.
    #[error("invalid agent binding scope: {scope}")]
    InvalidScope {
        /// Rejected scope name.
        scope: String,
    },
    /// An action target was empty or contained a NUL byte.
    #[error("invalid action target")]
    InvalidActionTarget {
        /// Rejected target.
        target: String,
    },
}

/// A validated session pipe name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeName(String);

impl PipeName {
    /// Validates a pipe name returned by `atk-hub-service agent-pipe`.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::InvalidPipeName`] unless the name is the fixed
    /// prefix followed by at least one ASCII digit.
    pub fn parse(name: &str) -> Result<Self, AgentError> {
        let digits =
            name.strip_prefix(PIPE_NAME_PREFIX)
                .ok_or_else(|| AgentError::InvalidPipeName {
                    name: name.to_owned(),
                })?;
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(AgentError::InvalidPipeName {
                name: name.to_owned(),
            });
        }
        Ok(Self(name.to_owned()))
    }

    /// Returns the bare pipe name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the Windows device path used by `net.connect`.
    #[must_use]
    pub fn windows_path(&self) -> String {
        format!("{PIPE_PATH_PREFIX}{}", self.0)
    }
}

/// A request sent to the agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// Monotonically increasing request id.
    pub id: u64,
    /// Allowlisted agent method name.
    pub method: String,
    /// JSON parameters, `null` when absent.
    pub params: Value,
}

impl Request {
    /// Creates a request.
    #[must_use]
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }

    /// Encodes the request as a newline-terminated JSON frame.
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or the frame is too large.
    pub fn encode(&self) -> Result<Vec<u8>, AgentError> {
        let value = serde_json::json!({
            "id": self.id,
            "method": &self.method,
            "params": &self.params,
        });
        let mut text = serde_json::to_string(&value)?;
        text.push('\n');
        let bytes = text.into_bytes();
        if bytes.len() > MAX_FRAME_BYTES {
            return Err(AgentError::OversizedFrame { len: bytes.len() });
        }
        Ok(bytes)
    }
}

/// A decoded response or event frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// Result of a correlated request.
    Response {
        /// Request id echoed by the agent.
        id: u64,
        /// Whether the request succeeded.
        ok: bool,
        /// Result value on success.
        result: Value,
        /// Error message on failure.
        error: Option<String>,
    },
    /// Unsolicited event emitted by the agent.
    Event {
        /// Event name.
        event: String,
        /// Complete event payload.
        payload: Value,
    },
}

impl Frame {
    /// Decodes one JSON value into a frame.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::MissingId`] for a response without a numeric id.
    pub fn decode(value: &Value) -> Result<Self, AgentError> {
        if let Some(event) = value.get("event").and_then(Value::as_str) {
            return Ok(Self::Event {
                event: event.to_owned(),
                payload: value.clone(),
            });
        }

        let id = value
            .get("id")
            .and_then(Value::as_u64)
            .ok_or(AgentError::MissingId)?;
        Ok(Self::Response {
            id,
            ok: value.get("ok").and_then(Value::as_bool).unwrap_or(false),
            result: value.get("result").cloned().unwrap_or(Value::Null),
            error: value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

/// Accumulates partial pipe reads and emits complete frames.
#[derive(Debug, Default)]
pub struct FramedBuffer {
    bytes: Vec<u8>,
}

impl FramedBuffer {
    /// Feeds a chunk of bytes and returns every complete frame it closes.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized frames, invalid UTF-8, invalid JSON, or
    /// a response without an id.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<Frame>, AgentError> {
        self.bytes.extend_from_slice(chunk);
        let mut frames = Vec::new();

        while let Some(end) = self.bytes.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.bytes.drain(..=end).collect();
            let payload = &line[..line.len() - 1];
            if payload.len() > MAX_FRAME_BYTES {
                return Err(AgentError::OversizedFrame { len: payload.len() });
            }
            let text = str::from_utf8(payload)?;
            frames.push(Frame::decode(&serde_json::from_str(text)?)?);
        }

        if self.bytes.len() > MAX_FRAME_BYTES {
            return Err(AgentError::OversizedFrame {
                len: self.bytes.len(),
            });
        }
        Ok(frames)
    }

    /// Returns the number of buffered bytes without a terminating newline.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.bytes.len()
    }
}
