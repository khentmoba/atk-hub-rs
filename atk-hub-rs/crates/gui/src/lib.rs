//! Tauri desktop shell for the ATK HUB Rust port.

use atk_config::Config;
use atk_features::{DriverInfo, Lighting, Macro, MacroStep};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Mutex;
use tauri::{Manager, State};

#[cfg(not(windows))]
use atk_agent::PipeName;
#[cfg(windows)]
use atk_agent::{AgentClient, DeviceId, PipeName, Rule, RuleAction};

#[cfg(test)]
const DEFAULT_PIPE: &str = "atk-hub-agent-session-1";
const DEFAULT_ASAR: &str = r"C:\Program Files\ATK HUB\resources\main\app.asar";

struct HubState {
    #[cfg(windows)]
    client: Mutex<Option<AgentClient>>,
    #[cfg(not(windows))]
    client: Mutex<Option<()>>,
    macro_path: Mutex<String>,
    macros: Mutex<Vec<Macro>>,
}

impl Default for HubState {
    fn default() -> Self {
        let user_data = user_data_dir();
        Self {
            client: Mutex::new(None),
            macro_path: Mutex::new(user_data.join("macros.json").to_string_lossy().into_owned()),
            macros: Mutex::new(Vec::new()),
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

#[tauri::command]
fn app_info() -> Value {
    let update_host = atk_endpoints::updater_feed_url();
    json!({
        "name": "ATK HUB",
        "version": env!("CARGO_PKG_VERSION"),
        "engine": "Tauri + Rust",
        "updateHost": update_host,
        "userData": user_data_dir().to_string_lossy(),
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
fn window_close(window: tauri::Window) -> Result<(), String> {
    window.close().map_err(|error| error.to_string())
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
    DEFAULT_ASAR.to_owned()
}

#[tauri::command]
fn config_load(path: String) -> Result<String, String> {
    let config = Config::parse(&fs::read_to_string(&path).map_err(|error| error.to_string())?)
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
    if save.unwrap_or(false) && !path.is_empty() {
        if let Some(parent) = Path::new(&path).parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&path, &output).map_err(|error| error.to_string())?;
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
fn connected_client(
    state: &HubState,
) -> Result<std::sync::MutexGuard<'_, Option<AgentClient>>, String> {
    let guard = state
        .client
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    if guard.is_none() {
        return Err("Connect to the agent first".to_owned());
    }
    Ok(guard)
}

#[cfg(windows)]
#[tauri::command]
fn device_list(state: State<'_, HubState>) -> Result<Value, String> {
    let guard = connected_client(&state)?;
    guard
        .as_ref()
        .expect("checked")
        .request("device.list", Value::Null)
        .map_err(|error| error.to_string())
}

#[cfg(not(windows))]
#[tauri::command]
fn device_list(_state: State<'_, HubState>) -> Result<Value, String> {
    Err("device list is only available on Windows".to_owned())
}

#[cfg(windows)]
#[tauri::command]
fn device_control(args: DeviceControlArgs, state: State<'_, HubState>) -> Result<Value, String> {
    let guard = connected_client(&state)?;
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

#[tauri::command]
fn keymaps() -> Result<Vec<String>, String> {
    let entries =
        atk_features::list_keymaps(Path::new(DEFAULT_ASAR)).map_err(|error| error.to_string())?;
    Ok(entries.into_iter().map(|entry| entry.name).collect())
}

#[tauri::command]
fn lighting_report(document: String) -> Result<Vec<u8>, String> {
    let config = Config::parse(&document).map_err(|error| error.to_string())?;
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
    }
    .report())
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
fn macros_add(name: String, keys: String, state: State<'_, HubState>) -> Result<Value, String> {
    let steps = keys
        .split(',')
        .map(str::trim)
        .filter(|key| !key.is_empty())
        .map(|key| MacroStep {
            key: key.to_owned(),
            delay_ms: 50,
        })
        .collect();
    let mut macros = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    macros.push(Macro { name, steps });
    serde_json::to_value(macros.clone()).map_err(|error| error.to_string())
}

#[tauri::command]
fn macros_run(index: usize, state: State<'_, HubState>) -> Result<String, String> {
    let macros = state
        .macros
        .lock()
        .map_err(|_| "state lock poisoned".to_owned())?;
    let macro_def = macros
        .get(index)
        .ok_or_else(|| "macro not found".to_owned())?;
    for step in &macro_def.steps {
        let sequence =
            atk_input::KeySequence::from_str(&step.key).map_err(|error| error.to_string())?;
        atk_input::KeyInjector
            .press(&sequence)
            .map_err(|error| error.to_string())?;
    }
    Ok(format!("Ran macro {}", macro_def.name))
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
fn pets_load(path: String) -> Result<Value, String> {
    let config =
        atk_features::load_pet_settings(Path::new(&path)).map_err(|error| error.to_string())?;
    Ok(json!({
        "enabled": atk_features::pet_enabled(&config, "enabled"),
        "visible": atk_features::pet_enabled(&config, "petVisible"),
        "sound": atk_features::pet_enabled(&config, "sound"),
    }))
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
            macros_load,
            macros_path,
            macros_save,
            macros_add,
            macros_run,
            music_status,
            music_stop,
            pets_load,
            plugins_list,
            plugins_install,
            update_check,
            driver_metadata,
            driver_prepare,
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_shadow(true)?;
            }
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
