//! Tauri desktop shell for the ATK HUB Rust port.

use atk_config::Config;
use atk_features::{DriverInfo, GameRule, HubSettings, Lighting, Macro, MacroStep, RepeatMode};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};

mod live;
mod overlay;

#[cfg(not(windows))]
use atk_agent::PipeName;
#[cfg(windows)]
use atk_agent::{AgentClient, DeviceId, PipeName, Rule, RuleAction};

#[cfg(test)]
const DEFAULT_PIPE: &str = "atk-hub-agent-session-1";
pub(crate) const DEFAULT_ASAR: &str = r"C:\Program Files\ATK HUB\resources\app.asar";
pub(crate) const FALLBACK_ASAR: &str = r"C:\Program Files\ATK HUB\resources\main\app.asar";
#[cfg(windows)]
const SERVICE_EXE: &str = r"C:\Program Files\ATK HUB\resources\service\atk-hub-service.exe";

struct HubState {
    #[cfg(windows)]
    client: Mutex<Option<AgentClient>>,
    #[cfg(not(windows))]
    client: Mutex<Option<()>>,
    macro_path: Mutex<String>,
    macros: Mutex<Vec<Macro>>,
    mouse: live::SharedMouse,
    keyboard: live::SharedKeyboard,
    settings: Arc<Mutex<HubSettings>>,
    games: Arc<Mutex<Vec<GameRule>>>,
}

impl Default for HubState {
    fn default() -> Self {
        let user_data = user_data_dir();
        Self {
            client: Mutex::new(None),
            macro_path: Mutex::new(user_data.join("macros.json").to_string_lossy().into_owned()),
            macros: Mutex::new(Vec::new()),
            mouse: Arc::new(Mutex::new(None)),
            keyboard: Arc::new(Mutex::new(None)),
            settings: Arc::new(Mutex::new(live::read_settings())),
            games: Arc::new(Mutex::new(live::initial_game_rules())),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct DeviceControlArgs {
    operation: String,
    #[serde(default)]
    vendor_id: String,
    #[serde(default)]
    product_id: String,
    #[serde(default)]
    report: String,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    rule_name: String,
    #[serde(default)]
    packet: String,
    #[serde(default)]
    target: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PageInfo {
    id: &'static str,
    label: &'static str,
    subtitle: &'static str,
    group: &'static str,
}

const PAGES: &[PageInfo] = &[
    PageInfo {
        id: "overview",
        label: "Device Center",
        subtitle: "Connected devices and system status",
        group: "Device",
    },
    PageInfo {
        id: "devices",
        label: "Devices",
        subtitle: "Live HID interfaces from the ATK agent",
        group: "Device",
    },
    PageInfo {
        id: "mouse",
        label: "Mouse",
        subtitle: "DPI, polling rate, buttons and sensor settings",
        group: "Device",
    },
    PageInfo {
        id: "keyboard",
        label: "Keyboard",
        subtitle: "Key remap, layers, magnetic switches and backlight",
        group: "Device",
    },
    PageInfo {
        id: "lighting",
        label: "Light Show",
        subtitle: "Keyboard backlight and effects",
        group: "Device",
    },
    PageInfo {
        id: "macros",
        label: "Macros",
        subtitle: "Key sequences and bindings",
        group: "Device",
    },
    PageInfo {
        id: "audio",
        label: "Audio",
        subtitle: "THX spatial audio properties",
        group: "Device",
    },
    PageInfo {
        id: "games",
        label: "Game Mode",
        subtitle: "Switch mouse profiles when a game is in focus",
        group: "Device",
    },
    PageInfo {
        id: "wallpapers",
        label: "Background Theme",
        subtitle: "Cloud wallpaper catalogue",
        group: "Library",
    },
    PageInfo {
        id: "music",
        label: "Music Score Tool",
        subtitle: "Legacy music rhythm service",
        group: "Library",
    },
    PageInfo {
        id: "pets",
        label: "Desktop Pets",
        subtitle: "Companion and wooden-fish settings",
        group: "Library",
    },
    PageInfo {
        id: "plugins",
        label: "Plugin Market",
        subtitle: "Installed and downloadable extensions",
        group: "Library",
    },
    PageInfo {
        id: "updater",
        label: "Update",
        subtitle: "Version feed and installer",
        group: "System",
    },
    PageInfo {
        id: "drivers",
        label: "Drivers",
        subtitle: "Driver packages and firmware",
        group: "System",
    },
    PageInfo {
        id: "settings",
        label: "Settings",
        subtitle: "Persisted application configuration",
        group: "System",
    },
    PageInfo {
        id: "diagnostics",
        label: "Diagnostics",
        subtitle: "ASAR, endpoints, IPC and agent protocol",
        group: "System",
    },
];

fn user_data_dir() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("C:\\Users\\Admin"))
                .join("AppData")
                .join("Roaming")
        })
        .join("ATK-HUB")
}

fn parse_id(text: &str) -> Result<u16, String> {
    let trimmed = text.trim();
    let hex = trimmed.starts_with("0x") || trimmed.starts_with("0X");
    let digits = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    u16::from_str_radix(digits, if hex { 16 } else { 10 }).map_err(|error| error.to_string())
}

fn parse_report(text: &str) -> Result<Vec<u8>, String> {
    text.split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let digits = part.strip_prefix("0x").unwrap_or(part);
            u8::from_str_radix(digits, 16).map_err(|error| error.to_string())
        })
        .collect()
}

fn resolve_asar_path() -> PathBuf {
    let primary = PathBuf::from(DEFAULT_ASAR);
    if primary.exists() {
        return primary;
    }
    let fallback = PathBuf::from(FALLBACK_ASAR);
    if fallback.exists() {
        return fallback;
    }
    primary
}

#[tauri::command]
fn app_info(state: State<'_, HubState>) -> Value {
    let update_host = atk_endpoints::updater_feed_url();
    let user_data = user_data_dir();
    let macro_path = state
        .macro_path
        .lock()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| user_data.join("macros.json").to_string_lossy().into_owned());
    json!({
        "name": "ATK HUB",
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "Tauri + Rust",
        "updateHost": update_host,
        "userData": user_data.to_string_lossy(),
        "configPath": user_data.join("config.json").to_string_lossy(),
        "petPath": user_data.join("wooden-fish.json").to_string_lossy(),
        "macroPath": macro_path,
        "asarPath": resolve_asar_path().to_string_lossy(),
        "pages": PAGES,
    })
}

#[tauri::command]
fn window_minimize(window: tauri::Window) -> Result<(), String> {
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn window_toggle_maximize(window: tauri::Window) -> Result<(), String> {
    let maximized = window.is_maximized().map_err(|error| error.to_string())?;
    if maximized {
        window.unmaximize().map_err(|error| error.to_string())
    } else {
        window.maximize().map_err(|error| error.to_string())
    }
}

#[tauri::command]
fn window_close(window: tauri::Window, state: State<'_, HubState>) -> Result<(), String> {
    let to_tray = state
        .settings
        .lock()
        .map(|settings| settings.close_minimize_tray)
        .unwrap_or(true);
    if to_tray {
        return window.hide().map_err(|error| error.to_string());
    }
    window.app_handle().exit(0);
    Ok(())
}

#[tauri::command]
fn app_exit(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn endpoint_catalog() -> Value {
    json!({
        "firmwareBase": atk_endpoints::FIRMWARE_BASE,
        "wallpaperCatalog": atk_endpoints::wallpaper_catalog_url(),
        "officialSite": atk_endpoints::official_site_url(),
        "musicServiceExit": atk_endpoints::music_service_exit_url(),
        "updaterFeed": atk_endpoints::updater_feed_url(),
    })
}

#[tauri::command]
fn firmware_url(vendor_id: String, product_id: String, tag: String) -> Result<String, String> {
    Ok(atk_endpoints::firmware_url(
        parse_id(&vendor_id)?,
        parse_id(&product_id)?,
        &tag,
    ))
}

#[tauri::command]
fn device_update_version_url(host: String, platform: String) -> String {
    atk_endpoints::device_update_version_url(&host, &platform)
}

#[tauri::command]
fn ipc_categories() -> Value {
    let groups = atk_ipc::by_category()
        .iter()
        .map(|(category, channels)| {
            json!({
                "category": category.to_string(),
                "channels": channels.iter().map(ToString::to_string).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    json!({ "groups": groups, "agentMethods": atk_ipc::AGENT_METHODS })
}

#[tauri::command]
fn asar_manifest(path: String) -> Result<Value, String> {
    let manifest = atk_asar::Manifest::parse(&fs::read(&path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    fn node(entry: &atk_asar::Node) -> Value {
        match entry {
            atk_asar::Node::File(file) => json!({
                "kind": "file",
                "size": file.size,
                "offset": file.offset,
                "unpacked": file.unpacked,
                "executable": file.executable,
            }),
            atk_asar::Node::Directory(children) => json!({
                "kind": "directory",
                "children": children.iter().map(|(name, child)| json!({"name": name, "node": node(child)})).collect::<Vec<_>>(),
            }),
        }
    }
    Ok(json!({
        "dataOffset": manifest.data_offset,
        "files": manifest.files.iter().map(|(name, entry)| json!({"name": name, "node": node(entry)})).collect::<Vec<_>>(),
    }))
}

#[tauri::command]
fn default_asar_path() -> String {
    resolve_asar_path().to_string_lossy().into_owned()
}

#[tauri::command]
fn config_load(path: String) -> Result<String, String> {
    let target = if path.trim().is_empty() {
        user_data_dir().join("config.json")
    } else {
        PathBuf::from(&path)
    };
    let config = Config::parse(&fs::read_to_string(&target).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    config.to_json().map_err(|error| error.to_string())
}

#[tauri::command]
fn config_apply(
    path: String,
    document: String,
    key: String,
    value: String,
    save: Option<bool>,
) -> Result<String, String> {
    let mut config = Config::parse(&document).map_err(|error| error.to_string())?;
    let parsed: Value = serde_json::from_str(&value).unwrap_or_else(|_| Value::String(value));
    config
        .set(&key, parsed)
        .map_err(|error| error.to_string())?;
    let output = config.to_json().map_err(|error| error.to_string())?;
    if save.unwrap_or(false) {
        let target = if path.trim().is_empty() {
            user_data_dir().join("config.json")
        } else {
            PathBuf::from(&path)
        };
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&target, &output).map_err(|error| error.to_string())?;
    }
    Ok(output)
}

#[tauri::command]
fn agent_encode(request_id: String, method: String, params: String) -> Result<String, String> {
    let id = request_id
        .parse::<u64>()
        .map_err(|error| error.to_string())?;
    let parsed = serde_json::from_str(&params).unwrap_or(Value::Null);
    let request = atk_agent::Request::new(id, method, parsed);
    let bytes = request.encode().map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).replace('\n', "\\n"))
}

#[tauri::command]
fn agent_decode(frame: String) -> Result<String, String> {
    let mut buffer = atk_agent::FramedBuffer::default();
    let mut chunk = frame;
    chunk.push('\n');
    let frames = buffer
        .push(chunk.as_bytes())
        .map_err(|error| error.to_string())?;
    let lines = frames
        .iter()
        .map(|frame| match frame {
            atk_agent::Frame::Response {
                id,
                ok,
                result,
                error,
            } => {
                format!("response id={id} ok={ok} result={result} error={error:?}")
            }
            atk_agent::Frame::Event { event, payload } => format!("event {event}: {payload}"),
        })
        .collect::<Vec<_>>();
    Ok(lines.join("\n"))
}

#[tauri::command]
fn agent_validate_pipe(name: String) -> Result<String, String> {
    let pipe = PipeName::parse(&name).map_err(|error| error.to_string())?;
    Ok(pipe.windows_path())
}

#[cfg(windows)]
#[tauri::command]
fn agent_connect(name: String, state: State<'_, HubState>) -> Result<String, String> {
    let pipe = PipeName::parse(&name).map_err(|error| error.to_string())?;
    let client = AgentClient::connect(&pipe).map_err(|error| error.to_string())?;
    let path = pipe.windows_path();
    *state
        .client
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())? = Some(client);
    Ok(format!("Connected to {path}"))
}

#[cfg(not(windows))]
#[tauri::command]
fn agent_connect(_name: String, _state: State<'_, HubState>) -> Result<String, String> {
    Err("agent pipe is only available on Windows".to_owned())
}

#[tauri::command]
fn agent_request(
    method: String,
    params: String,
    state: State<'_, HubState>,
) -> Result<String, String> {
    #[cfg(windows)]
    {
        let guard = state
            .client
            .lock()
            .map_err(|_| "state lock poisoned".to_owned())?;
        let client = guard
            .as_ref()
            .ok_or_else(|| "Connect to the agent first".to_owned())?;
        let parsed = serde_json::from_str(&params).unwrap_or(Value::Null);
        let value = client
            .request(&method, parsed)
            .map_err(|error| error.to_string())?;
        serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (method, params, state);
        Err("agent pipe is only available on Windows".to_owned())
    }
}

#[tauri::command]
fn agent_disconnect(state: State<'_, HubState>) -> Result<(), String> {
    *state
        .client
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())? = None;
    Ok(())
}

#[cfg(windows)]
fn discover_pipe_name() -> Result<PipeName, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    if Path::new(SERVICE_EXE).exists()
        && let Ok(output) = std::process::Command::new(SERVICE_EXE)
            .arg("agent-pipe")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
    {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if let Ok(pipe) = PipeName::parse(&name) {
            return Ok(pipe);
        }
    }
    PipeName::parse("atk-hub-agent-session-1").map_err(|error| error.to_string())
}

#[cfg(windows)]
fn ensure_connected(
    state: &HubState,
) -> Result<std::sync::MutexGuard<'_, Option<AgentClient>>, String> {
    let mut guard = state
        .client
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    if guard.is_none() {
        let pipe = discover_pipe_name()?;
        let client = AgentClient::connect(&pipe).map_err(|error| error.to_string())?;
        *guard = Some(client);
    }
    Ok(guard)
}

#[tauri::command]
fn agent_auto_connect(state: State<'_, HubState>) -> Value {
    #[cfg(windows)]
    {
        match discover_pipe_name() {
            Ok(pipe) => match ensure_connected(&state) {
                Ok(_) => json!({
                    "connected": true,
                    "pipe": pipe.as_str(),
                    "windowsPath": pipe.windows_path(),
                }),
                Err(error) => json!({
                    "connected": false,
                    "pipe": pipe.as_str(),
                    "error": error,
                }),
            },
            Err(error) => json!({
                "connected": false,
                "error": error,
            }),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = state;
        json!({
            "connected": false,
            "error": "agent pipe is only available on Windows",
        })
    }
}

#[cfg(windows)]
#[tauri::command]
fn device_list(state: State<'_, HubState>) -> Result<Value, String> {
    let result = {
        let guard = ensure_connected(&state)?;
        guard
            .as_ref()
            .expect("checked")
            .request("device.list", Value::Null)
    };
    match result {
        Ok(value) => Ok(value),
        Err(_) => {
            *state
                .client
                .lock()
                .map_err(|_| "state lock poisoned".to_owned())? = None;
            let guard = ensure_connected(&state)?;
            guard
                .as_ref()
                .expect("checked")
                .request("device.list", Value::Null)
                .map_err(|error| error.to_string())
        }
    }
}

#[cfg(not(windows))]
#[tauri::command]
fn device_list(_state: State<'_, HubState>) -> Result<Value, String> {
    Err("device list is only available on Windows".to_owned())
}

#[cfg(windows)]
#[tauri::command]
fn device_control(args: DeviceControlArgs, state: State<'_, HubState>) -> Result<Value, String> {
    let guard = ensure_connected(&state)?;
    let client = guard.as_ref().expect("checked");
    match args.operation.as_str() {
        "connect" => client
            .connect_device(&DeviceId::new(
                parse_id(&args.vendor_id)?,
                parse_id(&args.product_id)?,
            ))
            .map_err(|error| error.to_string()),
        "disconnect" => client
            .disconnect_device()
            .map_err(|error| error.to_string()),
        "write" => client
            .write_report(&parse_report(&args.report)?)
            .map_err(|error| error.to_string()),
        "clear-rules" => client.set_rules(&[]).map_err(|error| error.to_string()),
        "bindings" => {
            let rule = Rule::new(args.rule_name, args.packet, RuleAction::open(args.target));
            client
                .set_bindings(
                    &DeviceId::new(parse_id(&args.vendor_id)?, parse_id(&args.product_id)?),
                    &args.scope,
                    &[rule],
                )
                .map_err(|error| error.to_string())
        }
        "state" => serde_json::to_value(client.state().map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string()),
        other => Err(format!("unknown device operation: {other}")),
    }
}

#[cfg(not(windows))]
#[tauri::command]
fn device_control(_args: DeviceControlArgs, _state: State<'_, HubState>) -> Result<Value, String> {
    Err("device control is only available on Windows".to_owned())
}

#[tauri::command]
fn wallpaper_catalog() -> Result<Value, String> {
    let catalog = atk_wallpaper::fetch_catalog().map_err(|error| error.to_string())?;
    serde_json::to_value(catalog).map_err(|error| error.to_string())
}

#[tauri::command]
fn audio_config(product: String) -> Result<String, String> {
    let config = atk_audio::load_endpoint_config(&product).map_err(|error| error.to_string())?;
    Ok(format!(
        "Connection file valid: {} (TLS certificate present)",
        config.address
    ))
}

#[tauri::command]
fn audio_status(product: String) -> Result<String, String> {
    let config = atk_audio::load_endpoint_config(&product).map_err(|error| error.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let result = runtime.block_on(async {
        let mut client = atk_audio::ThxClient::connect(&config).await?;
        client.status().await
    });
    serde_json::to_string_pretty(&result.map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn audio_spatial(product: String, enabled: bool) -> Result<String, String> {
    let config = atk_audio::load_endpoint_config(&product).map_err(|error| error.to_string())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    runtime
        .block_on(async {
            let mut client = atk_audio::ThxClient::connect(&config).await?;
            client
                .set_bool(
                    atk_audio::HEADPHONES_ENDPOINT,
                    atk_audio::SPATIAL_ENABLED,
                    enabled,
                )
                .await
        })
        .map_err(|error| error.to_string())?;
    Ok("Spatial setting updated".to_owned())
}

fn lighting_from_document(document: &str) -> Result<Lighting, String> {
    let config = Config::parse(document).map_err(|error| error.to_string())?;
    let number = |key: &str| -> u8 {
        config
            .get(key)
            .and_then(Value::as_u64)
            .unwrap_or(255)
            .min(255) as u8
    };
    Ok(Lighting {
        keymap: config.get_str("keymap").unwrap_or_default().to_owned(),
        red: number("red"),
        green: number("green"),
        blue: number("blue"),
        brightness: number("brightness"),
    })
}

#[tauri::command]
fn keymaps() -> Result<Vec<String>, String> {
    let entries =
        atk_features::list_keymaps(&resolve_asar_path()).map_err(|error| error.to_string())?;
    Ok(entries.into_iter().map(|entry| entry.name).collect())
}

#[tauri::command]
fn lighting_report(document: String) -> Result<Vec<u8>, String> {
    Ok(lighting_from_document(&document)?.report())
}

#[tauri::command]
fn lighting_apply(
    vendor_id: String,
    product_id: String,
    document: String,
    state: State<'_, HubState>,
) -> Result<Value, String> {
    let report = lighting_from_document(&document)?.report();
    #[cfg(windows)]
    {
        let guard = ensure_connected(&state)?;
        let client = guard.as_ref().expect("checked");
        client
            .connect_device(&DeviceId::new(
                parse_id(&vendor_id)?,
                parse_id(&product_id)?,
            ))
            .map_err(|error| error.to_string())?;
        let written = client
            .write_report(&report)
            .map_err(|error| error.to_string())?;
        Ok(json!({ "report": report, "result": written }))
    }
    #[cfg(not(windows))]
    {
        let _ = (vendor_id, product_id, state, report);
        Err("lighting apply is only available on Windows".to_owned())
    }
}

#[tauri::command]
fn macros_load(state: State<'_, HubState>) -> Result<Value, String> {
    let path = state
        .macro_path
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?
        .clone();
    let macros = atk_features::load_macros(Path::new(&path)).map_err(|error| error.to_string())?;
    *state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())? = macros.clone();
    serde_json::to_value(macros).map_err(|error| error.to_string())
}

#[tauri::command]
fn macros_path(path: Option<String>, state: State<'_, HubState>) -> Result<String, String> {
    let mut current = state
        .macro_path
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    if let Some(path) = path {
        *current = path;
    }
    Ok(current.clone())
}

#[tauri::command]
fn macros_save(state: State<'_, HubState>) -> Result<(), String> {
    let path = state
        .macro_path
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?
        .clone();
    let macros = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    atk_features::save_macros(Path::new(&path), &macros).map_err(|error| error.to_string())
}

#[tauri::command]
fn macros_add(
    name: String,
    keys: Option<String>,
    steps: Option<String>,
    repeat_mode: Option<RepeatMode>,
    repeat_count: Option<u32>,
    index: Option<usize>,
    state: State<'_, HubState>,
) -> Result<Value, String> {
    let steps: Vec<MacroStep> = match steps.filter(|text| !text.trim().is_empty()) {
        Some(json) => serde_json::from_str(&json).map_err(|error| error.to_string())?,
        None => keys
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(|key| MacroStep::press(key, 50))
            .collect(),
    };
    if steps.is_empty() {
        return Err("a macro needs at least one step".to_owned());
    }
    let entry = Macro {
        name,
        steps,
        repeat_mode: repeat_mode.unwrap_or_default(),
        repeat_count: repeat_count.unwrap_or(1).max(1),
    };
    let mut macros = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    match index {
        Some(index) if index < macros.len() => macros[index] = entry,
        _ => macros.push(entry),
    }
    serde_json::to_value(macros.clone()).map_err(|error| error.to_string())
}

#[tauri::command]
fn macros_list(state: State<'_, HubState>) -> Result<Value, String> {
    let macros = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    serde_json::to_value(macros.clone()).map_err(|error| error.to_string())
}

#[tauri::command]
fn macros_delete(index: usize, state: State<'_, HubState>) -> Result<Value, String> {
    let mut macros = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    if index >= macros.len() {
        return Err("macro not found".to_owned());
    }
    macros.remove(index);
    serde_json::to_value(macros.clone()).map_err(|error| error.to_string())
}

#[tauri::command]
async fn macros_run(index: usize, state: State<'_, HubState>) -> Result<String, String> {
    let macro_def = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?
        .get(index)
        .cloned()
        .ok_or_else(|| "macro not found".to_owned())?;
    let repeat = match macro_def.repeat_mode {
        RepeatMode::Count => macro_def.repeat_count,
        RepeatMode::WhileHeld | RepeatMode::Toggle => 1,
    };
    tauri::async_runtime::spawn_blocking(move || {
        live::run_steps(&macro_def.steps, repeat)?;
        Ok(format!("Ran macro {}", macro_def.name))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
fn music_status() -> bool {
    atk_features::music_service_running()
}

#[tauri::command]
fn music_stop() -> Result<(), String> {
    atk_features::stop_music_service().map_err(|error| error.to_string())
}

#[tauri::command]
fn plugins_list() -> Result<Value, String> {
    let plugins =
        atk_features::list_plugins(&user_data_dir()).map_err(|error| error.to_string())?;
    serde_json::to_value(plugins).map_err(|error| error.to_string())
}

#[tauri::command]
fn plugins_install(
    url: String,
    id: String,
    version: String,
    sha256: String,
) -> Result<String, String> {
    let path = atk_features::install_bundle(&url, &id, &version, &sha256, &user_data_dir())
        .map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
fn update_check() -> Result<Value, String> {
    let info = atk_features::check_update().map_err(|error| error.to_string())?;
    serde_json::to_value(info).map_err(|error| error.to_string())
}

fn driver_info() -> Result<DriverInfo, String> {
    atk_features::fetch_service_driver().map_err(|error| error.to_string())
}

#[tauri::command]
fn driver_metadata() -> Result<Value, String> {
    serde_json::to_value(driver_info()?).map_err(|error| error.to_string())
}

#[tauri::command]
fn driver_prepare(sha256: String) -> Result<String, String> {
    let cache = std::env::temp_dir();
    let path = atk_features::download_driver(&driver_info()?, &cache, &sha256)
        .map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

/// Starts the Tauri desktop application.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(HubState::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            window_minimize,
            window_toggle_maximize,
            window_close,
            endpoint_catalog,
            firmware_url,
            device_update_version_url,
            ipc_categories,
            asar_manifest,
            default_asar_path,
            config_load,
            config_apply,
            agent_encode,
            agent_decode,
            agent_validate_pipe,
            agent_auto_connect,
            agent_connect,
            agent_request,
            agent_disconnect,
            device_list,
            device_control,
            wallpaper_catalog,
            audio_config,
            audio_status,
            audio_spatial,
            keymaps,
            lighting_report,
            lighting_apply,
            macros_list,
            macros_load,
            macros_path,
            macros_save,
            macros_add,
            macros_delete,
            macros_run,
            music_status,
            music_stop,
            live::pets_load,
            live::pets_save,
            overlay::overlay_state,
            overlay::woodenfish_tap,
            overlay::woodenfish_set,
            overlay::woodenfish_sound_get,
            overlay::woodenfish_sound_set,
            overlay::desktop_pet_set,
            overlay::desktop_pet_reset_position,
            overlay::desktop_pet_sprite,
            overlay::desktop_pet_upload,
            live::mouse_list,
            live::kb_list,
            live::kb_snapshot,
            live::kb_keymap,
            live::kb_set_key,
            live::kb_reset_keymap,
            live::kb_apply,
            live::kb_actuation,
            live::kb_set_actuation,
            live::kb_rapid_trigger,
            live::kb_set_rapid_trigger,
            live::mouse_snapshot,
            live::mouse_apply,
            live::mouse_button,
            live::mouse_profile,
            live::mouse_pair,
            live::mouse_pair_status,
            live::mouse_reset,
            live::mouse_macro_read,
            live::mouse_macro_write,
            live::mouse_shortcut_read,
            live::mouse_shortcut_write,
            live::keymap_load,
            live::system_mouse_get,
            live::system_mouse_set,
            live::hub_settings_get,
            live::hub_settings_set,
            live::lock_hint_hide,
            live::game_rules_get,
            live::game_rules_set,
            live::foreground_process,
            live::wallpaper_apply,
            live::update_download,
            live::installer_launch,
            live::driver_install,
            live::macros_run_steps,
            app_exit,
            plugins_list,
            plugins_install,
            update_check,
            driver_metadata,
            driver_prepare,
        ])
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        live::toggle_main(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_shadow(true)?;
                if std::env::args().any(|arg| arg == "--hidden") {
                    window.hide()?;
                }
            }
            live::build_tray(app)?;
            live::build_lock_overlay(app)?;
            overlay::build_overlays(app)?;
            overlay::spawn_input_watch(app.handle().clone());
            let state = app.state::<HubState>();
            let settings = state
                .settings
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default();
            live::apply_hotkey(app.handle(), &settings);
            live::spawn_lock_hint(app.handle().clone(), Arc::clone(&state.settings));
            live::spawn_game_switch(
                app.handle().clone(),
                Arc::clone(&state.mouse),
                Arc::clone(&state.settings),
                Arc::clone(&state.games),
            );
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start ATK HUB");
}
#[cfg(test)]
mod tests {
    use super::*;
    use atk_features::UpdateInfo;

    #[test]
    fn every_page_has_unique_id() {
        let mut ids: Vec<_> = PAGES.iter().map(|page| page.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), PAGES.len());
    }

    #[test]
    fn report_parser_accepts_hex_bytes() {
        assert_eq!(parse_report("00 AA 01, ff").unwrap(), vec![0, 170, 1, 255]);
    }

    #[test]
    fn pipe_name_validation_matches_agent() {
        assert!(PipeName::parse(DEFAULT_PIPE).is_ok());
        assert!(PipeName::parse("wrong").is_err());
    }

    #[test]
    fn update_parser_reads_known_feed_shape() {
        let raw = "version: 3.2.31\nurl: https://example.invalid/a.exe\nsha512: abc\nsize: 42\n";
        let info = UpdateInfo::parse(raw).unwrap();
        assert_eq!(info.version, "3.2.31");
        assert_eq!(info.size, 42);
    }
}
