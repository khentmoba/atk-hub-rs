//! Access to ATK HUB's `config.json`, preserving electron-store semantics.
//!
//! The original store serializes with `JSON.stringify(value, undefined, "\t")`,
//! reads keys using dot notation, and keeps unknown keys untouched.

use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

/// Key holding the per-install user identifier.
pub const USER_ID_KEY: &str = "USERINFO.userId";
/// Key holding the downloaded background service URL.
pub const SERVICE_URL_KEY: &str = "atk_service_url";
/// Key holding the last acknowledged renderer bundle version.
pub const ACKNOWLEDGED_UI_VERSION_KEY: &str = "hub.viteElectronVersion.lastAcknowledged";

/// Failures while reading or changing configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The configuration is not valid JSON.
    #[error("config JSON is invalid")]
    Json(#[from] serde_json::Error),
    /// The serialized JSON could not be represented as UTF-8.
    #[error("config JSON is not valid UTF-8")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),
    /// A key was empty or contained an empty segment.
    #[error("config key must have nonempty dot-separated segments")]
    EmptyKey,
    /// A path segment was not an object, so electron-store could not descend.
    #[error("config key {key:?} traverses a non-object segment")]
    NonObjectSegment {
        /// The full key being written.
        key: String,
    },
}

/// A parsed ATK HUB configuration document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    root: Value,
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl Config {
    /// Creates an empty configuration document.
    #[must_use]
    pub fn new() -> Self {
        Self {
            root: Value::Object(serde_json::Map::new()),
        }
    }

    /// Parses a configuration document from JSON text.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Json`] when `input` is not valid JSON.
    pub fn parse(input: &str) -> Result<Self, ConfigError> {
        Ok(Self {
            root: serde_json::from_str(input)?,
        })
    }

    /// Reads a value by dot-separated key, following electron-store notation.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        if key.is_empty() {
            return None;
        }
        let mut value = &self.root;
        for part in key.split('.') {
            value = value.get(part)?;
        }
        Some(value)
    }

    /// Reads a string value by dot-separated key.
    #[must_use]
    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// Writes a value by dot-separated key, creating intermediate objects.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::EmptyKey`] for an empty or malformed key, or
    /// [`ConfigError::NonObjectSegment`] when an intermediate segment exists
    /// but is not an object.
    pub fn set(&mut self, key: &str, value: Value) -> Result<(), ConfigError> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.iter().any(|part| part.is_empty()) {
            return Err(ConfigError::EmptyKey);
        }
        let (leaf, parents) = parts.split_last().ok_or(ConfigError::EmptyKey)?;

        let mut cursor = &mut self.root;
        for part in parents {
            if !cursor.is_object() {
                return Err(ConfigError::NonObjectSegment {
                    key: key.to_owned(),
                });
            }
            let object = cursor.as_object_mut().expect("checked with is_object");
            if !object.contains_key(*part) {
                object.insert((*part).to_owned(), Value::Object(serde_json::Map::new()));
            }
            cursor = object.get_mut(*part).expect("key exists after insertion");
        }

        let object = cursor
            .as_object_mut()
            .ok_or_else(|| ConfigError::NonObjectSegment {
                key: key.to_owned(),
            })?;
        object.insert((*leaf).to_owned(), value);
        Ok(())
    }

    /// Serializes with the same tab indentation as the original store.
    ///
    /// # Errors
    ///
    /// Returns a serialization or UTF-8 error if the document cannot be encoded.
    pub fn to_json(&self) -> Result<String, ConfigError> {
        let mut buffer = Vec::new();
        {
            let formatter = serde_json::ser::PrettyFormatter::with_indent(b"\t");
            let mut serializer = serde_json::Serializer::with_formatter(&mut buffer, formatter);
            self.root.serialize(&mut serializer)?;
        }
        Ok(String::from_utf8(buffer)?)
    }

    /// Returns the stored user identifier, if present.
    #[must_use]
    pub fn user_id(&self) -> Option<&str> {
        self.get_str(USER_ID_KEY)
    }

    /// Returns the stored background service URL, if present.
    #[must_use]
    pub fn service_url(&self) -> Option<&str> {
        self.get_str(SERVICE_URL_KEY)
    }

    /// Updates the stored background service URL.
    ///
    /// # Errors
    ///
    /// Propagates errors from [`Self::set`].
    pub fn set_service_url(&mut self, url: &str) -> Result<(), ConfigError> {
        self.set(SERVICE_URL_KEY, Value::String(url.to_owned()))
    }

    /// Returns the last acknowledged renderer bundle version, if present.
    #[must_use]
    pub fn acknowledged_ui_version(&self) -> Option<&str> {
        self.get_str(ACKNOWLEDGED_UI_VERSION_KEY)
    }
}
