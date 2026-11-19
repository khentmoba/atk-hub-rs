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
    /// Caller input was rejected.
    #[error("{0}")]
    Invalid(String),
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
    /// Recorded event: `"down"`, `"up"`, or `None` for a full press.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// Windows virtual-key code captured by the recorder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vk: Option<u8>,
}

impl MacroStep {
    /// A full key press (down then up) followed by `delay_ms`.
    #[must_use]
    pub fn press(key: impl Into<String>, delay_ms: u64) -> Self {
        Self {
            key: key.into(),
            delay_ms,
            action: None,
            vk: None,
        }
    }
}

/// How a macro repeats when triggered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RepeatMode {
    /// Run `repeat_count` times.
    #[default]
    Count,
    /// Repeat until the trigger key is released (hardware macros).
    WhileHeld,
    /// Toggle on the first press, stop on the next press (hardware macros).
    Toggle,
}

fn default_repeat() -> u32 {
    1
}

/// A named macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Macro {
    /// Macro name.
    pub name: String,
    /// Ordered key steps.
    pub steps: Vec<MacroStep>,
    /// Repeat behaviour.
    #[serde(default)]
    pub repeat_mode: RepeatMode,
    /// Number of runs for [`RepeatMode::Count`].
    #[serde(default = "default_repeat")]
    pub repeat_count: u32,
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
    /// Ships a floating-selection toolbar renderer.
    #[serde(default)]
    pub floating: bool,
    /// Ships a PC script renderer.
    #[serde(default)]
    pub pc_script: bool,
    /// Ships VIA keyboard definitions (`*.json`).
    #[serde(default)]
    pub via_json: bool,
}

fn has_file(dir: &Path, matches: impl Fn(&str) -> bool) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry.file_type().is_ok_and(|kind| kind.is_file())
                && matches(&entry.file_name().to_string_lossy().to_lowercase())
        })
    })
}

/// Whether `id` is a safe plugin directory name.
#[must_use]
pub fn valid_plugin_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && !id.contains("..") && !id.contains(['/', '\\'])
}

/// Deletes every installed version of a plugin.
///
/// # Errors
///
/// Returns an error for an invalid id or when the files cannot be removed.
pub fn remove_plugin(user_data: &Path, id: &str) -> Result<(), FeatureError> {
    let id = id.trim();
    if !valid_plugin_id(id) {
        return Err(FeatureError::Invalid("invalid plugin id".to_owned()));
    }
    let dir = user_data.join("plugins").join(id);
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    Ok(())
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
                let path = version_entry.path();
                plugins.push(Plugin {
                    id: id_entry.file_name().to_string_lossy().into_owned(),
                    version: version_entry.file_name().to_string_lossy().into_owned(),
                    floating: has_file(&path, |name| {
                        name.starts_with("floating-selection") && name.ends_with(".js")
                    }),
                    pc_script: has_file(&path, |name| name == "pc-script.js"),
                    via_json: has_file(&path, |name| name.ends_with(".json")),
                    path,
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

/// Extracts a zip archive into `destination`, rejecting unsafe entry paths.
///
/// # Errors
///
/// Returns an error for an invalid archive, an unsafe entry path, or a
/// filesystem failure.
pub fn extract_zip(bytes: &[u8], destination: &Path) -> Result<(), FeatureError> {
    let mut archive =
        zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error: zip::result::ZipError| {
            FeatureError::InvalidArchive {
                reason: error.to_string(),
            }
        })?;
    fs::create_dir_all(destination)?;
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
    Ok(())
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
    extract_zip(bytes, &destination)?;
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

/// Reads one keymap JSON document (`keymaps/<name>`) from an Electron ASAR.
///
/// # Errors
///
/// Returns an error when the archive cannot be parsed, the entry is missing,
/// or the entry is not valid JSON.
pub fn read_keymap(asar_path: &Path, name: &str) -> Result<serde_json::Value, FeatureError> {
    let bytes = fs::read(asar_path)?;
    let manifest = Manifest::parse(&bytes)?;
    let path = format!("keymaps/{}", name.trim_start_matches("keymaps/"));
    let entry = manifest
        .lookup(&path)
        .ok_or_else(|| FeatureError::UnexpectedDocument {
            field: path.clone(),
        })?;
    let start = usize::try_from(manifest.data_offset + entry.offset).unwrap_or(usize::MAX);
    let end = start.saturating_add(usize::try_from(entry.size).unwrap_or(usize::MAX));
    let slice = bytes
        .get(start..end)
        .ok_or(FeatureError::UnexpectedDocument { field: path })?;
    Ok(serde_json::from_slice(slice)?)
}

/// Downloads a URL into memory.
///
/// # Errors
///
/// Returns an error for a failed request or read.
pub fn download(url: &str) -> Result<Vec<u8>, FeatureError> {
    let response = ureq::get(url)
        .call()
        .map_err(|error| FeatureError::Request(Box::new(error)))?;
    let mut bytes = Vec::new();
    response.into_reader().read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// File name component of a URL, without query or fragment.
#[must_use]
pub fn url_file_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let name = path.rsplit('/').next().unwrap_or_default();
    let safe: String = name
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || "._-%()".contains(*character))
        .collect();
    if safe.is_empty() || safe.chars().all(|character| character == '.') {
        "download.bin".to_owned()
    } else {
        safe
    }
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(char::from(TABLE[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Base64 SHA-512 digest, the format used by electron-updater `latest.yml`.
#[must_use]
pub fn sha512_base64(bytes: &[u8]) -> String {
    base64(&sha2::Sha512::digest(bytes))
}

/// Downloads the installer named by `info`, verifies its SHA-512 and saves it.
///
/// Relative URLs resolve against the update feed, as electron-updater does.
///
/// # Errors
///
/// Returns an error for a failed download, digest mismatch or write failure.
pub fn download_update(info: &UpdateInfo, cache_dir: &Path) -> Result<PathBuf, FeatureError> {
    let url = if info.url.starts_with("http://") || info.url.starts_with("https://") {
        info.url.clone()
    } else {
        format!("{}/{}", atk_endpoints::UPDATER_FEED, info.url)
    };
    let bytes = download(&url)?;
    let actual = sha512_base64(&bytes);
    if !info.sha512.is_empty() && actual != info.sha512 {
        return Err(FeatureError::HashMismatch {
            expected: info.sha512.clone(),
            actual,
        });
    }
    fs::create_dir_all(cache_dir)?;
    let path = cache_dir.join(url_file_name(&url));
    fs::write(&path, bytes)?;
    Ok(path)
}

/// Finds the first Windows executable or MSI inside an extracted directory.
#[must_use]
pub fn find_installer(dir: &Path) -> Option<PathBuf> {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().map(|entry| entry.path()).collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("msi")
                })
            {
                return Some(path);
            }
        }
    }
    None
}

/// Wooden-fish / desk-pet settings stored under `progress` in
/// `wooden-fish.json`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetSettings {
    /// Feature enabled.
    pub enabled: bool,
    /// Desktop pet window visible.
    pub pet_visible: bool,
    /// Knock sound on.
    pub sound: bool,
    /// Selected skin id.
    #[serde(default)]
    pub skin: String,
    /// Merit period label.
    #[serde(default)]
    pub period: String,
    /// Recorded day counters, kept verbatim.
    #[serde(default)]
    pub days: serde_json::Value,
}

impl PetSettings {
    /// Reads the `progress.*` keys, defaulting missing values.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        let flag = |key: &str| {
            config
                .get(&format!("progress.{key}"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false)
        };
        let text = |key: &str| {
            config
                .get(&format!("progress.{key}"))
                .map(|value| {
                    value
                        .as_str()
                        .map_or_else(|| value.to_string(), str::to_owned)
                })
                .unwrap_or_default()
        };
        Self {
            enabled: flag("enabled"),
            pet_visible: flag("petVisible"),
            sound: flag("sound"),
            skin: text("skin"),
            period: text("period"),
            days: config
                .get("progress.days")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        }
    }

    /// Writes the editable flags back under `progress`.
    ///
    /// # Errors
    ///
    /// Propagates settings errors.
    pub fn apply(&self, config: &mut Config) -> Result<(), FeatureError> {
        config.set("progress.enabled", serde_json::json!(self.enabled))?;
        config.set("progress.petVisible", serde_json::json!(self.pet_visible))?;
        config.set("progress.sound", serde_json::json!(self.sound))?;
        if !self.skin.is_empty() {
            config.set("progress.skin", serde_json::json!(self.skin))?;
        }
        Ok(())
    }
}

/// Application settings stored under `setting` in `config.json`, matching the
/// original electron-store keys.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubSettings {
    /// Launch at Windows sign-in.
    pub open_at_login: bool,
    /// Closing the window minimizes to the tray instead of exiting.
    pub close_minimize_tray: bool,
    /// Ask before exiting.
    pub exit_detection_prompt_enabled: bool,
    /// Show the Caps/Num/Scroll Lock overlay hint.
    pub keyboard_lock_hint_enabled: bool,
    /// Global show/hide hotkey enabled.
    pub global_hub_hotkey_enabled: bool,
    /// Global show/hide hotkey accelerator, e.g. `Alt+Shift+H`.
    pub global_hub_hotkey_accelerator: String,
    /// UI language code.
    pub language: String,
    /// Game auto-switch enabled.
    pub game_switch_enabled: bool,
}

impl Default for HubSettings {
    fn default() -> Self {
        Self {
            open_at_login: false,
            close_minimize_tray: true,
            exit_detection_prompt_enabled: true,
            keyboard_lock_hint_enabled: false,
            global_hub_hotkey_enabled: false,
            global_hub_hotkey_accelerator: "Alt+Shift+H".to_owned(),
            language: "en".to_owned(),
            game_switch_enabled: false,
        }
    }
}

impl HubSettings {
    /// Reads `setting.*` from a config document, defaulting missing keys.
    #[must_use]
    pub fn from_config(config: &Config) -> Self {
        let defaults = Self::default();
        let flag = |key: &str, default: bool| {
            config
                .get(&format!("setting.{key}"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(default)
        };
        let text = |key: &str, default: &str| {
            config
                .get_str(&format!("setting.{key}"))
                .unwrap_or(default)
                .to_owned()
        };
        Self {
            open_at_login: flag("openAtLogin", defaults.open_at_login),
            close_minimize_tray: flag("closeMinimizeTray", defaults.close_minimize_tray),
            exit_detection_prompt_enabled: flag(
                "exitDetectionPromptEnabled",
                defaults.exit_detection_prompt_enabled,
            ),
            keyboard_lock_hint_enabled: flag(
                "keyboardLockHintEnabled",
                defaults.keyboard_lock_hint_enabled,
            ),
            global_hub_hotkey_enabled: flag(
                "globalHubHotkeyEnabled",
                defaults.global_hub_hotkey_enabled,
            ),
            global_hub_hotkey_accelerator: text(
                "globalHubHotkeyAccelerator",
                &defaults.global_hub_hotkey_accelerator,
            ),
            language: text("language", &defaults.language),
            game_switch_enabled: flag("gameSwitchEnabled", defaults.game_switch_enabled),
        }
    }

    /// Writes every key back under `setting`.
    ///
    /// # Errors
    ///
    /// Propagates settings errors.
    pub fn apply(&self, config: &mut Config) -> Result<(), FeatureError> {
        let value = serde_json::to_value(self)?;
        if let Some(object) = value.as_object() {
            for (key, value) in object {
                config.set(&format!("setting.{key}"), value.clone())?;
            }
        }
        Ok(())
    }
}

/// One game auto-switch rule: when `exe` is in the foreground, load `profile`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameRule {
    /// Executable file name, matched case-insensitively.
    pub exe: String,
    /// Onboard mouse profile index.
    pub profile: u8,
    /// Keyboard profile index, when the keyboard should switch too.
    #[serde(
        default,
        rename = "keyboardProfile",
        skip_serializing_if = "Option::is_none"
    )]
    pub keyboard_profile: Option<u8>,
}

/// Picks the rule for a foreground executable.
#[must_use]
pub fn match_game_rule<'a>(rules: &'a [GameRule], exe: &str) -> Option<&'a GameRule> {
    rules.iter().find(|rule| rule.exe.eq_ignore_ascii_case(exe))
}

/// Reads the first `static/` asset whose file name starts with `prefix` and
/// ends with `extension`, e.g. the hashed `aby-*.webp` pet sprite.
///
/// # Errors
///
/// Returns an error when the archive cannot be read or has no such asset.
pub fn read_asar_asset(
    asar_path: &Path,
    prefix: &str,
    extension: &str,
) -> Result<Vec<u8>, FeatureError> {
    let bytes = fs::read(asar_path)?;
    let manifest = Manifest::parse(&bytes)?;
    let path = manifest
        .paths()
        .into_iter()
        .find(|path| {
            path.strip_prefix("static/")
                .is_some_and(|name| name.starts_with(prefix) && name.ends_with(extension))
        })
        .ok_or_else(|| FeatureError::UnexpectedDocument {
            field: format!("static/{prefix}*{extension}"),
        })?;
    let entry = manifest
        .lookup(&path)
        .ok_or_else(|| FeatureError::UnexpectedDocument {
            field: path.clone(),
        })?;
    let start = usize::try_from(manifest.data_offset + entry.offset).unwrap_or(usize::MAX);
    let end = start.saturating_add(usize::try_from(entry.size).unwrap_or(usize::MAX));
    bytes
        .get(start..end)
        .map(<[u8]>::to_vec)
        .ok_or(FeatureError::UnexpectedDocument { field: path })
}

/// Encodes bytes as a `data:` URL with the given media type.
#[must_use]
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(bytes))
}
