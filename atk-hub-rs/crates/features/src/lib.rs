//! Feature logic behind the ATK HUB Rust GUI pages.
//!
//! Each function works on real installed assets or live services; no Electron
//! process is involved.

use atk_asar::Manifest;
use atk_config::Config;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Cursor, Read};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;
use thiserror::Error;

/// Failures in the feature layer.
#[derive(Debug, Error)]
pub enum FeatureError {
    /// JSON was malformed.
    #[error("feature JSON is invalid")]
    Json(#[from] serde_json::Error),
    /// A file could not be read or written.
    #[error("feature file I/O failed")]
    Io(#[from] std::io::Error),
    /// An ASAR manifest could not be parsed.
    #[error("feature ASAR parse failed")]
    Asar(#[from] atk_asar::AsarError),
    /// A settings document could not be changed.
    #[error("feature settings error")]
    Config(#[from] atk_config::ConfigError),
    /// An HTTP request failed.
    #[error("feature HTTP request failed")]
    Request(#[from] Box<ureq::Error>),
    /// A remote document had an unexpected shape.
    #[error("unexpected remote document: {field}")]
    UnexpectedDocument {
        /// Field that was missing or malformed.
        field: String,
    },
    /// A downloaded bundle did not match its expected SHA-256.
    #[error("SHA-256 mismatch: expected {expected}, got {actual}")]
    HashMismatch {
        /// Expected digest.
        expected: String,
        /// Observed digest.
        actual: String,
    },
    /// A ZIP archive was invalid or contained an unsafe path.
    #[error("invalid plugin bundle archive: {reason}")]
    InvalidArchive {
        /// Reason for rejection.
        reason: String,
    },
}

/// Keymap asset available in the installed renderer bundle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Keymap {
    /// Asset file name.
    pub name: String,
    /// Declared size in bytes.
    pub bytes: u64,
}

/// Lists keymap JSON assets in an Electron ASAR.
///
/// # Errors
///
/// Returns an error if the archive cannot be read or parsed.
pub fn list_keymaps(asar_path: &Path) -> Result<Vec<Keymap>, FeatureError> {
    let manifest = Manifest::parse(&fs::read(asar_path)?)?;
    let mut keymaps = Vec::new();
    for path in manifest.paths() {
        if !path.starts_with("keymaps/") || !path.ends_with(".json") {
            continue;
        }
        if let Some(entry) = manifest.lookup(&path) {
            keymaps.push(Keymap {
                name: path.trim_start_matches("keymaps/").to_owned(),
                bytes: entry.size,
            });
        }
    }
    keymaps.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(keymaps)
}

/// Keyboard lighting settings, mapped to an output HID report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lighting {
    /// Selected keymap name.
    pub keymap: String,
    /// Red component.
    pub red: u8,
    /// Green component.
    pub green: u8,
    /// Blue component.
    pub blue: u8,
    /// Brightness level.
    pub brightness: u8,
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            keymap: String::new(),
            red: 255,
            green: 255,
            blue: 255,
            brightness: 128,
        }
    }
}

impl Lighting {
    /// Builds the report passed to `device.write`.
    #[must_use]
    pub fn report(&self) -> Vec<u8> {
        vec![0, 170, 1, self.red, self.green, self.blue, self.brightness]
    }
}

/// One step of a macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MacroStep {
    /// Key name or key code.
    pub key: String,
    /// Delay after the key, in milliseconds.
    pub delay_ms: u64,
}

/// A named macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    /// Macro name.
    pub name: String,
    /// Ordered key steps.
    pub steps: Vec<MacroStep>,
}

/// Loads macros from a JSON file.
///
/// # Errors
///
/// Returns an error for a missing file or malformed JSON.
pub fn load_macros(path: &Path) -> Result<Vec<Macro>, FeatureError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

/// Saves macros to a JSON file.
///
/// # Errors
///
/// Returns an error when the file cannot be written or encoded.
pub fn save_macros(path: &Path, macros: &[Macro]) -> Result<(), FeatureError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(macros)?)?;
    Ok(())
}

/// Returns whether the legacy music service is listening on port 5577.
#[must_use]
pub fn music_service_running() -> bool {
    let address = ("127.0.0.1", 5577_u16);
    match address.to_socket_addrs() {
        Ok(mut addresses) => addresses.next().is_some_and(|socket| {
            TcpStream::connect_timeout(&socket, Duration::from_millis(300)).is_ok()
        }),
        Err(_) => false,
    }
}

/// Stops the legacy music service, matching the original's best-effort request.
///
/// # Errors
///
/// Returns an error only for an unexpected transport failure; a refused
/// connection is treated as success because the service is already stopped.
pub fn stop_music_service() -> Result<(), FeatureError> {
    match ureq::get(atk_endpoints::music_service_exit_url())
        .timeout(Duration::from_millis(300))
        .call()
    {
        Ok(_) => Ok(()),
        Err(ureq::Error::Transport(_)) => Ok(()),
        Err(error) => Err(FeatureError::Request(Box::new(error))),
    }
}

/// Opens a desk-pet settings document.
///
/// # Errors
///
/// Returns an error when the file is missing or malformed.
pub fn load_pet_settings(path: &Path) -> Result<Config, FeatureError> {
    Ok(Config::parse(&fs::read_to_string(path)?)?)
}

/// Reads a boolean desk-pet setting.
#[must_use]
pub fn pet_enabled(settings: &Config, key: &str) -> bool {
    settings
        .get(key)
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

/// Updates a boolean desk-pet setting.
///
/// # Errors
///
/// Propagates settings errors.
pub fn set_pet_enabled(
    settings: &mut Config,
    key: &str,
    enabled: bool,
) -> Result<(), FeatureError> {
    settings.set(key, serde_json::json!(enabled))?;
    Ok(())
}

/// Installed plugin bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    /// Plugin id.
    pub id: String,
    /// Installed version directory.
    pub version: String,
    /// Absolute path of the plugin.
    pub path: PathBuf,
}

/// Lists installed plugin bundles.
///
/// # Errors
///
/// Returns an error if the directory cannot be read.
pub fn list_plugins(user_data: &Path) -> Result<Vec<Plugin>, FeatureError> {
    let root = user_data.join("plugins");
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut plugins = Vec::new();
    for id_entry in fs::read_dir(&root)? {
        let id_entry = id_entry?;
        if !id_entry.file_type()?.is_dir() {
            continue;
        }
        for version_entry in fs::read_dir(id_entry.path())? {
            let version_entry = version_entry?;
            if version_entry.file_type()?.is_dir() {
                plugins.push(Plugin {
                    id: id_entry.file_name().to_string_lossy().into_owned(),
                    version: version_entry.file_name().to_string_lossy().into_owned(),
                    path: version_entry.path(),
                });
            }
        }
    }
    plugins.sort_by(|left, right| (&left.id, &left.version).cmp(&(&right.id, &right.version)));
    Ok(plugins)
}

/// Parsed `latest.yml` update metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateInfo {
    /// Available version.
    pub version: String,
    /// Installer URL.
    pub url: String,
    /// Base64 SHA-512 digest.
    pub sha512: String,
    /// Download size in bytes.
    pub size: u64,
}

fn field(text: &str, name: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.trim()
            .trim_start_matches("- ")
            .strip_prefix(name)
            .map(|value| value.trim().to_owned())
    })
}

impl UpdateInfo {
    /// Parses the electron-updater `latest.yml` document.
    ///
    /// # Errors
    ///
    /// Returns an error when a required field is missing or malformed.
    pub fn parse(text: &str) -> Result<Self, FeatureError> {
        let missing = |name: &str| FeatureError::UnexpectedDocument {
            field: name.to_owned(),
        };
        let version = field(text, "version:").ok_or_else(|| missing("version"))?;
        let url = field(text, "url:").ok_or_else(|| missing("url"))?;
        let sha512 = field(text, "sha512:").ok_or_else(|| missing("sha512"))?;
        let size = field(text, "size:")
            .ok_or_else(|| missing("size"))?
            .parse()
            .map_err(|_| missing("size"))?;
        Ok(Self {
            version,
            url,
            sha512,
            size,
        })
    }
}

/// Checks the configured update feed.
///
/// # Errors
///
/// Returns an error for a failed request or malformed metadata.
pub fn check_update() -> Result<UpdateInfo, FeatureError> {
    let url = format!("{}/latest.yml", atk_endpoints::UPDATER_FEED);
    let response = ureq::get(&url)
        .call()
        .map_err(|error| FeatureError::Request(Box::new(error)))?;
    UpdateInfo::parse(&response.into_string()?)
}

/// Compares two dotted numeric versions.
#[must_use]
pub fn version_newer(current: &str, available: &str) -> bool {
    let numbers = |value: &str| -> Vec<u64> {
        value
            .split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    let left = numbers(current);
    let right = numbers(available);
    for index in 0..left.len().max(right.len()) {
        let a = left.get(index).copied().unwrap_or(0);
        let b = right.get(index).copied().unwrap_or(0);
        if a != b {
            return b > a;
        }
    }
    false
}

/// Downloadable driver or service metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriverInfo {
    /// Remote version.
    pub version: String,
    /// Download URL.
    pub file: String,
}

/// Fetches game-power service metadata from the firmware API.
///
/// # Errors
///
/// Returns an error for a failed request or unexpected document shape.
pub fn fetch_service_driver() -> Result<DriverInfo, FeatureError> {
    let url = atk_endpoints::firmware_service_url(atk_endpoints::FirmwareService::GamePower);
    let response = ureq::get(&url)
        .call()
        .map_err(|error| FeatureError::Request(Box::new(error)))?;
    let document: serde_json::Value = serde_json::from_str(&response.into_string()?)?;
    let data = document
        .get("data")
        .ok_or_else(|| FeatureError::UnexpectedDocument {
            field: "data".to_owned(),
        })?;
    let version = data
        .get("version")
        .and_then(|value| value.as_str())
        .ok_or_else(|| FeatureError::UnexpectedDocument {
            field: "version".to_owned(),
        })?
        .to_owned();
    let file = data
        .get("file")
        .and_then(|value| value.as_str())
        .ok_or_else(|| FeatureError::UnexpectedDocument {
            field: "file".to_owned(),
        })?
        .to_owned();
    Ok(DriverInfo { version, file })
}

/// Computes the lowercase hex SHA-256 of a byte buffer.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Verifies a bundle digest. An empty expected digest skips verification.
///
/// # Errors
///
/// Returns [`FeatureError::HashMismatch`] when the digests differ.
pub fn verify_sha256(bytes: &[u8], expected: &str) -> Result<(), FeatureError> {
    if expected.is_empty() {
        return Ok(());
    }
    let actual = sha256_hex(bytes);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(FeatureError::HashMismatch {
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}

fn safe_zip_target(name: &str) -> Result<PathBuf, FeatureError> {
    let path = Path::new(name);
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(FeatureError::InvalidArchive {
            reason: format!("unsafe path: {name}"),
        });
    }
    Ok(path.to_path_buf())
}

/// Extracts a verified plugin bundle into `user_data/plugins/<id>/<version>`.
///
/// # Errors
///
/// Returns an error for a hash mismatch, an invalid archive, an unsafe entry
/// path, or a filesystem failure.
pub fn install_bundle_bytes(
    id: &str,
    version: &str,
    bytes: &[u8],
    expected_sha256: &str,
    user_data: &Path,
) -> Result<PathBuf, FeatureError> {
    verify_sha256(bytes, expected_sha256)?;
    let destination = user_data.join("plugins").join(id).join(version);
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error: zip::result::ZipError| {
            FeatureError::InvalidArchive {
                reason: error.to_string(),
            }
        })?;
    fs::create_dir_all(&destination)?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(
                |error: zip::result::ZipError| FeatureError::InvalidArchive {
                    reason: error.to_string(),
                },
            )?;
        let relative = safe_zip_target(entry.name())?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut output = fs::File::create(target)?;
            std::io::copy(&mut entry, &mut output)?;
        }
    }
    Ok(destination)
}

/// Downloads and installs a plugin bundle after SHA-256 verification.
///
/// # Errors
///
/// Returns an error for a download failure, hash mismatch, invalid archive or
/// filesystem failure.
pub fn install_bundle(
    url: &str,
    id: &str,
    version: &str,
    expected_sha256: &str,
    user_data: &Path,
) -> Result<PathBuf, FeatureError> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| FeatureError::Request(Box::new(error)))?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes)?;
    install_bundle_bytes(id, version, &bytes, expected_sha256, user_data)
}

/// Prepares a driver download: verifies the digest and extracts the archive.
///
/// # Errors
///
/// Returns an error for a download failure, hash mismatch, invalid archive or
/// filesystem failure.
pub fn download_driver(
    driver: &DriverInfo,
    cache_dir: &Path,
    expected_sha256: &str,
) -> Result<PathBuf, FeatureError> {
    let response = ureq::get(&driver.file)
        .call()
        .map_err(|error| FeatureError::Request(Box::new(error)))?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes)?;
    verify_sha256(&bytes, expected_sha256)?;
    let destination = cache_dir.join("game-power").join(&driver.version);
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error: zip::result::ZipError| {
            FeatureError::InvalidArchive {
                reason: error.to_string(),
            }
        })?;
    fs::create_dir_all(&destination)?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(
                |error: zip::result::ZipError| FeatureError::InvalidArchive {
                    reason: error.to_string(),
                },
            )?;
        let relative = safe_zip_target(entry.name())?;
        let target = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&target)?;
        } else if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
            let mut output = fs::File::create(target)?;
            std::io::copy(&mut entry, &mut output)?;
        }
    }
    Ok(destination)
}
