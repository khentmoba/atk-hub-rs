//! Typed HID control methods over the live agent pipe.
//!
//! Parameter shapes were recovered by probing the installed agent's own
//! validation errors, then confirmed with read-only and reversible calls.

use super::{AgentClient, AgentError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Identifies a HID interface for `device.connect` and `bindings.set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceId {
    /// Vendor id.
    pub vid: u16,
    /// Product id.
    pub pid: u16,
    /// Optional HID usage page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_page: Option<u16>,
    /// Optional HID usage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<u16>,
    /// Optional interface number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_number: Option<u16>,
    /// Optional serial number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
}

impl DeviceId {
    /// Creates a device identifier with just vendor and product ids.
    #[must_use]
    pub fn new(vid: u16, pid: u16) -> Self {
        Self {
            vid,
            pid,
            usage_page: None,
            usage: None,
            interface_number: None,
            serial_number: None,
        }
    }

    /// Sets the HID usage page and usage.
    #[must_use]
    pub fn with_usage(mut self, usage_page: u16, usage: u16) -> Self {
        self.usage_page = Some(usage_page);
        self.usage = Some(usage);
        self
    }
}

/// Action attached to a HID packet rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleAction {
    /// Agent action type, normally `open`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Target application, file, folder or URL for `open`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl RuleAction {
    /// Creates an `open` action for `target`.
    #[must_use]
    pub fn open(target: impl Into<String>) -> Self {
        Self {
            kind: "open".to_owned(),
            target: Some(target.into()),
        }
    }
}

/// A packet-triggered rule used by `rules.set` and `bindings.set`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Rule name.
    pub name: String,
    /// Hex packet pattern.
    pub packet: String,
    /// Byte offset of the pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
    /// Minimum milliseconds between activations.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cooldown_ms: Option<u32>,
    /// Action to run when the pattern matches.
    pub action: RuleAction,
}

impl Rule {
    /// Creates a rule with no offset and no cooldown.
    #[must_use]
    pub fn new(name: impl Into<String>, packet: impl Into<String>, action: RuleAction) -> Self {
        Self {
            name: name.into(),
            packet: packet.into(),
            offset: None,
            cooldown_ms: None,
            action,
        }
    }
}

/// State reported by `state.get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentState {
    /// Whether a HID device is currently connected.
    pub connected: bool,
    /// Connected device descriptor, when present.
    pub device: Option<Value>,
    /// Agent process id.
    pub pid: u64,
    /// Configured packet rules.
    pub rules: Vec<Value>,
    /// Path of the agent settings file.
    pub settings_path: String,
    /// Whether input tracing is enabled.
    pub trace: bool,
}

impl AgentClient {
    /// Opens a HID interface. The agent validates `vid` and `pid`.
    ///
    /// # Errors
    ///
    /// Propagates transport and agent errors.
    pub fn connect_device(&self, device: &DeviceId) -> Result<Value, AgentError> {
        self.request("device.connect", serde_json::to_value(device)?)
    }

    /// Closes the currently connected HID interface.
    ///
    /// # Errors
    ///
    /// Propagates transport and agent errors.
    pub fn disconnect_device(&self) -> Result<Value, AgentError> {
        self.request("device.disconnect", Value::Null)
    }

    /// Writes one output report, including the leading report id byte.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::InvalidReportLength`] for an empty or oversized
    /// report, and propagates transport and agent errors.
    pub fn write_report(&self, report: &[u8]) -> Result<Value, AgentError> {
        if report.is_empty() || report.len() > 65_536 {
            return Err(AgentError::InvalidReportLength { len: report.len() });
        }
        let bytes: Vec<Value> = report.iter().map(|byte| json!(byte)).collect();
        self.request("device.write", Value::Array(bytes))
    }

    /// Replaces the agent's live packet rules.
    ///
    /// # Errors
    ///
    /// Propagates transport and agent errors.
    pub fn set_rules(&self, rules: &[Rule]) -> Result<Value, AgentError> {
        self.request("rules.set", serde_json::to_value(rules)?)
    }

    /// Persists device bindings for a scope.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::InvalidScope`] for an empty or malformed scope,
    /// and propagates transport and agent errors.
    pub fn set_bindings(
        &self,
        device: &DeviceId,
        scope: &str,
        rules: &[Rule],
    ) -> Result<Value, AgentError> {
        let valid = !scope.is_empty()
            && scope.len() <= 64
            && scope
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        if !valid {
            return Err(AgentError::InvalidScope {
                scope: scope.to_owned(),
            });
        }
        self.request(
            "bindings.set",
            json!({ "device": device, "scope": scope, "rules": rules }),
        )
    }

    /// Opens an application, file, folder or URL in the interactive session.
    ///
    /// # Errors
    ///
    /// Returns [`AgentError::InvalidActionTarget`] for an empty target and
    /// propagates transport and agent errors.
    pub fn execute_action(&self, target: &str) -> Result<Value, AgentError> {
        if target.is_empty() || target.contains('\0') {
            return Err(AgentError::InvalidActionTarget {
                target: target.to_owned(),
            });
        }
        self.request(
            "action.execute",
            json!({ "type": "open", "target": target }),
        )
    }

    /// Reads the agent's current state.
    ///
    /// # Errors
    ///
    /// Propagates transport and agent errors, including malformed state JSON.
    pub fn state(&self) -> Result<AgentState, AgentError> {
        let value = self.request("state.get", Value::Null)?;
        Ok(serde_json::from_value(value)?)
    }
}
