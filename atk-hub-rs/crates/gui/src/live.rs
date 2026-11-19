//! Live hardware and OS features: COMPX mouse, Windows settings, tray,
//! autostart, lock-key hints, game auto-switch, downloads and installers.

use crate::{HubState, resolve_asar_path, user_data_dir};
use atk_config::Config;
use atk_features::{GameRule, HubSettings, PetSettings};
use atk_hid::HidError;
use atk_hid::compx::{
    CompxMouse, DpiLight, DpiStage, MouseFunction, MouseSnapshot, Performance, ReportRateDpi,
};
use atk_hid::{DeviceInfo, HidTransport};
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// Lazily opened mouse driver shared by commands and the game-switch thread.
pub type SharedMouse = Arc<Mutex<Option<CompxMouse<HidTransport>>>>;

fn lock_err<T>(_: T) -> String {
    "state lock poisoned".to_owned()
}

fn run_mouse<R>(
    mouse: &SharedMouse,
    action: impl FnOnce(&mut CompxMouse<HidTransport>) -> Result<R, HidError>,
) -> Result<R, String> {
    let mut guard = mouse.lock().map_err(lock_err)?;
    if guard.is_none() {
        let (transport, _) = HidTransport::open_first_mouse().map_err(|error| error.to_string())?;
        let mut driver = CompxMouse::new(transport);
        // Selects the sensor codec; ignored while the mouse sleeps.
        let _ = driver.identify();
        *guard = Some(driver);
    }
    let driver = guard.as_mut().ok_or("mouse unavailable")?;
    let result = action(driver);
    if matches!(result, Err(HidError::Api(_))) {
        // Unplugged or re-enumerated: reopen on the next call.
        *guard = None;
    }
    result.map_err(|error| error.to_string())
}

async fn with_mouse<R: Send + 'static>(
    state: &HubState,
    action: impl FnOnce(&mut CompxMouse<HidTransport>) -> Result<R, HidError> + Send + 'static,
) -> Result<R, String> {
    let mouse = Arc::clone(&state.mouse);
    tauri::async_runtime::spawn_blocking(move || run_mouse(&mouse, action))
        .await
        .map_err(|error| error.to_string())?
}

async fn blocking<R: Send + 'static>(
    work: impl FnOnce() -> Result<R, String> + Send + 'static,
) -> Result<R, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn mouse_list() -> Result<Vec<DeviceInfo>, String> {
    blocking(|| atk_hid::list_mice().map_err(|error| error.to_string())).await
}

#[tauri::command]
pub async fn mouse_snapshot(state: State<'_, HubState>) -> Result<MouseSnapshot, String> {
    with_mouse(&state, CompxMouse::snapshot).await
}

/// Partial mouse settings sent by the Mouse page; absent fields are untouched.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MousePatch {
    rate: Option<ReportRateDpi>,
    stages: Option<Vec<DpiStage>>,
    function: Option<MouseFunction>,
    performance: Option<Performance>,
    light: Option<DpiLight>,
    lift_off: Option<u8>,
    far_distance: Option<u8>,
    angle: Option<i8>,
    angle_enabled: Option<bool>,
}

#[tauri::command]
pub async fn mouse_apply(
    patch: MousePatch,
    state: State<'_, HubState>,
) -> Result<MouseSnapshot, String> {
    with_mouse(&state, move |mouse| {
        if !mouse.online()? {
            return Err(HidError::Offline);
        }
        mouse.identify()?;
        if let Some(stages) = &patch.stages {
            mouse.write_stages(stages)?;
        }
        if let Some(rate) = &patch.rate {
            mouse.write_rate(rate)?;
        }
        if let Some(function) = &patch.function {
            mouse.write_function(function)?;
        }
        if let Some(performance) = &patch.performance {
            mouse.write_performance(performance)?;
        }
        if let Some(light) = &patch.light {
            mouse.write_light(light)?;
        }
        if let Some(level) = patch.lift_off {
            mouse.write_lift_off(level)?;
        }
        if let Some(mode) = patch.far_distance {
            mouse.write_far_distance(mode)?;
        }
        if let Some(angle) = patch.angle {
            mouse.write_angle(angle, patch.angle_enabled.unwrap_or(angle != 0))?;
        }
        mouse.snapshot()
    })
    .await
}

#[tauri::command]
pub async fn mouse_button(
    index: usize,
    mapping: [u8; 3],
    state: State<'_, HubState>,
) -> Result<(), String> {
    with_mouse(&state, move |mouse| mouse.write_button(index, mapping)).await
}

#[tauri::command]
pub async fn mouse_profile(
    profile: u8,
    state: State<'_, HubState>,
) -> Result<MouseSnapshot, String> {
    with_mouse(&state, move |mouse| {
        mouse.set_profile(profile)?;
        std::thread::sleep(Duration::from_millis(300));
        mouse.snapshot()
    })
    .await
}

#[tauri::command]
pub async fn mouse_pair(state: State<'_, HubState>) -> Result<(), String> {
    with_mouse(&state, |mouse| {
        let (cid, mid, _) = mouse.identify()?;
        mouse.start_pairing(cid, mid)
    })
    .await
}

#[tauri::command]
pub async fn mouse_pair_status(state: State<'_, HubState>) -> Result<Value, String> {
    with_mouse(&state, |mouse| {
        let (status, seconds) = mouse.pairing_status()?;
        Ok(json!({ "status": status, "seconds": seconds }))
    })
    .await
}

#[tauri::command]
pub async fn mouse_reset(state: State<'_, HubState>) -> Result<(), String> {
    with_mouse(&state, CompxMouse::factory_reset).await
}

#[tauri::command]
pub fn keymap_load(name: String) -> Result<Value, String> {
    atk_features::read_keymap(&resolve_asar_path(), &name).map_err(|error| error.to_string())
}

#[cfg(windows)]
#[tauri::command]
pub fn system_mouse_get() -> Result<Value, String> {
    use atk_input::system;
    Ok(json!({
        "speed": system::pointer_speed().map_err(|error| error.to_string())?,
        "wheelLines": system::wheel_lines().map_err(|error| error.to_string())?,
    }))
}

#[cfg(windows)]
#[tauri::command]
pub fn system_mouse_set(speed: u32, wheel_lines: u32) -> Result<(), String> {
    use atk_input::system;
    system::set_pointer_speed(speed).map_err(|error| error.to_string())?;
    system::set_wheel_lines(wheel_lines).map_err(|error| error.to_string())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn system_mouse_get() -> Result<Value, String> {
    Err("system mouse settings are only available on Windows".to_owned())
}

#[cfg(not(windows))]
#[tauri::command]
pub fn system_mouse_set(_speed: u32, _wheel_lines: u32) -> Result<(), String> {
    Err("system mouse settings are only available on Windows".to_owned())
}

fn config_path() -> PathBuf {
    user_data_dir().join("config.json")
}

fn load_config(path: &Path) -> Config {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| Config::parse(&text).ok())
        .unwrap_or_default()
}

fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, config.to_json().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

/// Reads the persisted `setting.*` block.
pub fn read_settings() -> HubSettings {
    HubSettings::from_config(&load_config(&config_path()))
}

#[tauri::command]
pub fn hub_settings_get(state: State<'_, HubState>) -> Result<HubSettings, String> {
    Ok(state.settings.lock().map_err(lock_err)?.clone())
}

#[tauri::command]
pub fn hub_settings_set(
    settings: HubSettings,
    app: AppHandle,
    state: State<'_, HubState>,
) -> Result<HubSettings, String> {
    let path = config_path();
    let mut config = load_config(&path);
    settings
        .apply(&mut config)
        .map_err(|error| error.to_string())?;
    save_config(&path, &config)?;
    set_autostart(settings.open_at_login)?;
    apply_hotkey(&app, &settings);
    *state.settings.lock().map_err(lock_err)? = settings.clone();
    Ok(settings)
}

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "ATK HUB RS";

/// Adds or removes the sign-in Run entry, launching hidden like the original.
#[cfg(windows)]
fn set_autostart(enabled: bool) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let mut command = std::process::Command::new("reg");
    command.creation_flags(CREATE_NO_WINDOW);
    if enabled {
        let exe = std::env::current_exe().map_err(|error| error.to_string())?;
        let data = format!("\"{}\" --hidden", exe.display());
        command.args([
            "add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &data, "/f",
        ]);
    } else {
        command.args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"]);
    }
    let output = command.output().map_err(|error| error.to_string())?;
    // Deleting a value that does not exist is fine.
    if enabled && !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_autostart(_enabled: bool) -> Result<(), String> {
    Ok(())
}

/// Registers the global show/hide hotkey from settings.
pub fn apply_hotkey(app: &AppHandle, settings: &HubSettings) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    if settings.global_hub_hotkey_enabled
        && !settings.global_hub_hotkey_accelerator.trim().is_empty()
    {
        let _ = shortcuts.register(settings.global_hub_hotkey_accelerator.as_str());
    }
}

/// Shows and focuses the main window.
pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Toggles the main window between hidden and shown.
pub fn toggle_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
        } else {
            show_main(app);
        }
    }
}

/// Builds the tray icon with the original "Show" / "Exit" menu.
///
/// # Errors
///
/// Returns a Tauri error when the tray cannot be created.
pub fn build_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let show = MenuItem::with_id(app, "show-main", "Show ATK HUB", true, None::<&str>)?;
    let exit = MenuItem::with_id(app, "exit-app", "Exit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &exit])?;
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("ATK HUB")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show-main" => show_main(app),
            "exit-app" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }
    builder.build(app)?;
    Ok(())
}

/// Polls lock keys and shows the hint overlay when one toggles.
#[cfg(windows)]
pub fn spawn_lock_hint(app: AppHandle, settings: Arc<Mutex<HubSettings>>) {
    std::thread::spawn(move || {
        let mut last = atk_input::system::lock_keys();
        loop {
            std::thread::sleep(Duration::from_millis(150));
            let current = atk_input::system::lock_keys();
            if current == last {
                continue;
            }
            let changed = if current.caps != last.caps {
                ("Caps Lock", current.caps)
            } else if current.num != last.num {
                ("Num Lock", current.num)
            } else {
                ("Scroll Lock", current.scroll)
            };
            last = current;
            let enabled = settings
                .lock()
                .map(|guard| guard.keyboard_lock_hint_enabled)
                .unwrap_or(false);
            if !enabled {
                continue;
            }
            let payload = json!({ "key": changed.0, "on": changed.1 });
            let _ = app.emit("lock-keys", payload.clone());
            if let Some(overlay) = app.get_webview_window("lockhint") {
                let _ = overlay.emit("lock-keys", payload);
                let _ = overlay.show();
            }
        }
    });
}

#[cfg(not(windows))]
pub fn spawn_lock_hint(_app: AppHandle, _settings: Arc<Mutex<HubSettings>>) {}

/// Creates the hidden, non-focusable lock-key overlay window.
///
/// # Errors
///
/// Returns a Tauri error when the window cannot be created.
pub fn build_lock_overlay(app: &tauri::App) -> tauri::Result<()> {
    let (width, height) = (220.0, 64.0);
    let builder = tauri::WebviewWindowBuilder::new(
        app,
        "lockhint",
        tauri::WebviewUrl::App("lockhint.html".into()),
    )
    .title("Lock key")
    .inner_size(width, height)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .focused(false)
    .visible(false);
    let window = builder.build()?;
    if let Some(monitor) = window.primary_monitor()? {
        let scale = monitor.scale_factor();
        let size = monitor.size().to_logical::<f64>(scale);
        window.set_position(tauri::LogicalPosition::new(
            (size.width - width) / 2.0,
            size.height - height - 120.0,
        ))?;
    }
    window.set_ignore_cursor_events(true)?;
    Ok(())
}

#[tauri::command]
pub fn lock_hint_hide(app: AppHandle) {
    if let Some(overlay) = app.get_webview_window("lockhint") {
        let _ = overlay.hide();
    }
}

fn game_rules_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-games.json")
}

fn load_game_rules() -> Vec<GameRule> {
    fs::read_to_string(game_rules_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn game_rules_get(state: State<'_, HubState>) -> Result<Vec<GameRule>, String> {
    Ok(state.games.lock().map_err(lock_err)?.clone())
}

#[tauri::command]
pub fn game_rules_set(
    rules: Vec<GameRule>,
    state: State<'_, HubState>,
) -> Result<Vec<GameRule>, String> {
    let path = game_rules_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(
        &path,
        serde_json::to_string_pretty(&rules).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    *state.games.lock().map_err(lock_err)? = rules.clone();
    Ok(rules)
}

#[cfg(windows)]
#[tauri::command]
pub fn foreground_process() -> Option<String> {
    atk_input::system::foreground_process()
}

#[cfg(not(windows))]
#[tauri::command]
pub fn foreground_process() -> Option<String> {
    None
}

/// Switches the mouse profile when a configured game gains focus and back
/// to profile 0 when it loses focus.
pub fn spawn_game_switch(
    app: AppHandle,
    mouse: SharedMouse,
    settings: Arc<Mutex<HubSettings>>,
    rules: Arc<Mutex<Vec<GameRule>>>,
) {
    std::thread::spawn(move || {
        let mut active: Option<String> = None;
        loop {
            std::thread::sleep(Duration::from_millis(1500));
            let enabled = settings
                .lock()
                .map(|guard| guard.game_switch_enabled)
                .unwrap_or(false);
            if !enabled {
                active = None;
                continue;
            }
            #[cfg(windows)]
            let exe = atk_input::system::foreground_process().unwrap_or_default();
            #[cfg(not(windows))]
            let exe = String::new();
            let target = rules.lock().ok().and_then(|guard| {
                atk_features::match_game_rule(&guard, &exe)
                    .map(|rule| (rule.exe.clone(), rule.profile))
            });
            let next = match (&target, &active) {
                (Some((name, profile)), current) if current.as_deref() != Some(name.as_str()) => {
                    Some((Some(name.clone()), *profile))
                }
                (None, Some(_)) => Some((None, 0)),
                _ => None,
            };
            if let Some((name, profile)) = next
                && run_mouse(&mouse, |driver| driver.set_profile(profile)).is_ok()
            {
                let _ = app.emit("game-switch", json!({ "exe": name, "profile": profile }));
                active = name;
            }
        }
    });
}

/// Initial rules loaded at startup.
pub fn initial_game_rules() -> Vec<GameRule> {
    load_game_rules()
}

#[tauri::command]
pub fn pets_load(path: String) -> Result<Value, String> {
    let target = pet_path(&path);
    let config = load_config(&target);
    let pet = PetSettings::from_config(&config);
    let mut value = serde_json::to_value(pet).map_err(|error| error.to_string())?;
    value["path"] = json!(target.to_string_lossy());
    value["exists"] = json!(target.exists());
    Ok(value)
}

#[tauri::command]
pub fn pets_save(path: String, settings: PetSettings) -> Result<Value, String> {
    let target = pet_path(&path);
    let mut config = load_config(&target);
    settings
        .apply(&mut config)
        .map_err(|error| error.to_string())?;
    save_config(&target, &config)?;
    pets_load(target.to_string_lossy().into_owned())
}

fn pet_path(path: &str) -> PathBuf {
    if path.trim().is_empty() {
        user_data_dir().join("wooden-fish.json")
    } else {
        PathBuf::from(path)
    }
}

const IMAGE_EXTENSIONS: [&str; 5] = ["jpg", "jpeg", "png", "bmp", "webp"];

#[tauri::command]
pub async fn wallpaper_apply(url: String, apply: bool) -> Result<Value, String> {
    blocking(move || {
        let bytes = atk_features::download(&url).map_err(|error| error.to_string())?;
        let dir = user_data_dir().join("wallpapers");
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let path = dir.join(atk_features::url_file_name(&url));
        fs::write(&path, bytes).map_err(|error| error.to_string())?;
        let is_image = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()));
        let mut applied = false;
        #[cfg(windows)]
        if apply && is_image {
            atk_input::system::set_desktop_wallpaper(&path).map_err(|error| error.to_string())?;
            applied = true;
        }
        #[cfg(not(windows))]
        let _ = apply;
        Ok(json!({ "path": path.to_string_lossy(), "image": is_image, "applied": applied }))
    })
    .await
}

#[tauri::command]
pub async fn update_download() -> Result<String, String> {
    blocking(|| {
        let info = atk_features::check_update().map_err(|error| error.to_string())?;
        let dir = std::env::temp_dir().join("atk-hub-rs-update");
        let path = atk_features::download_update(&info, &dir).map_err(|error| error.to_string())?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
}

fn launch(path: &Path) -> Result<(), String> {
    std::process::Command::new(path)
        .spawn()
        .map(drop)
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[tauri::command]
pub fn installer_launch(path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let temp = std::env::temp_dir();
    if !path.starts_with(&temp) {
        return Err("only installers downloaded by ATK HUB can be launched".to_owned());
    }
    launch(&path)
}

#[tauri::command]
pub async fn driver_install(sha256: String) -> Result<String, String> {
    blocking(move || {
        let info = atk_features::fetch_service_driver().map_err(|error| error.to_string())?;
        let dir = atk_features::download_driver(&info, &std::env::temp_dir(), &sha256)
            .map_err(|error| error.to_string())?;
        let installer = atk_features::find_installer(&dir)
            .ok_or_else(|| format!("no installer found in {}", dir.display()))?;
        launch(&installer)?;
        Ok(installer.to_string_lossy().into_owned())
    })
    .await
}

#[tauri::command]
pub async fn macros_run_steps(
    steps: Vec<atk_features::MacroStep>,
    repeat: u32,
) -> Result<String, String> {
    blocking(move || {
        run_steps(&steps, repeat)?;
        Ok(format!("Ran {} step(s) x{}", steps.len(), repeat.max(1)))
    })
    .await
}

/// Plays macro steps: raw down/up events for recorded steps, full presses
/// for named keys, sleeping `delay_ms` after each.
pub fn run_steps(steps: &[atk_features::MacroStep], repeat: u32) -> Result<(), String> {
    use std::str::FromStr;
    for _ in 0..repeat.clamp(1, 100) {
        for step in steps {
            match (step.vk, step.action.as_deref()) {
                #[cfg(windows)]
                (Some(vk), Some(action)) => atk_input::send_vk(vk, action == "down"),
                _ => {
                    let sequence = atk_input::KeySequence::from_str(&step.key)
                        .map_err(|error| error.to_string())?;
                    atk_input::KeyInjector
                        .press(&sequence)
                        .map_err(|error| error.to_string())?;
                }
            }
            std::thread::sleep(Duration::from_millis(step.delay_ms.min(60_000)));
        }
    }
    Ok(())
}
