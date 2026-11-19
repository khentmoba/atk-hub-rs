//! Wooden fish and desktop pet overlay windows, mirroring the original
//! floating knock counter and the `HoopsPet` desktop companion.

use crate::live::{load_config, lock_err, pet_path, save_config};
use crate::{resolve_asar_path, user_data_dir};
use atk_features::PetSettings;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, LogicalSize, Manager};

const FISH: &str = "woodenfish";
const PET: &str = "pet";
const SPRITE_LIMIT: usize = 4 * 1024 * 1024;

/// Desktop pet preferences, matching the original `desktop-pet.dat` fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PetPrefs {
    /// Pet window shown.
    pub visible: bool,
    /// Size in percent: 50, 75, 100, 125 or 150.
    pub scale: u32,
    /// Keep above other windows.
    pub always_on_top: bool,
    /// Let clicks pass through the pet.
    pub click_through: bool,
    /// Play idle animations on its own.
    pub auto_play: bool,
    /// React to keyboard and pointer input.
    pub follow_input: bool,
    /// Show the pressed key above the pet.
    pub show_key_hints: bool,
    /// Show name and level below the pet.
    pub show_level: bool,
    /// Display name.
    pub pet_name: String,
    /// Experience points earned from input.
    pub xp: u64,
}

impl Default for PetPrefs {
    fn default() -> Self {
        Self {
            visible: false,
            scale: 100,
            always_on_top: true,
            click_through: false,
            auto_play: true,
            follow_input: true,
            show_key_hints: true,
            show_level: true,
            pet_name: "Aby".to_owned(),
            xp: 0,
        }
    }
}

impl PetPrefs {
    /// Level derived from experience.
    #[must_use]
    pub fn level(&self) -> u64 {
        (self.xp / 20).isqrt() + 1
    }

    /// Logical window size for the current scale.
    #[must_use]
    pub fn window_size(&self) -> (f64, f64) {
        let display = 128.0 * f64::from(self.scale.clamp(50, 150)) / 100.0;
        let width = display.ceil().max(192.0);
        let height = (96.0 + display * 208.0 / 192.0 + 24.0).ceil();
        (width, height)
    }
}

#[derive(Default)]
struct Overlay {
    fish: PetSettings,
    pet: PetPrefs,
    last_xp: Option<Instant>,
    dirty_days: bool,
    dirty_pet: bool,
}

fn overlay() -> &'static Mutex<Overlay> {
    static STATE: OnceLock<Mutex<Overlay>> = OnceLock::new();
    STATE.get_or_init(|| {
        let fish = PetSettings::from_config(&load_config(&pet_path("")));
        let pet = fs::read_to_string(prefs_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Mutex::new(Overlay {
            fish,
            pet,
            ..Overlay::default()
        })
    })
}

fn prefs_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-pet.json")
}

fn sound_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-fish-sound.txt")
}

fn sprite_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-pet-sprite.txt")
}

fn write_text(path: &PathBuf, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, text).map_err(|error| error.to_string())
}

fn fish_shown(fish: &PetSettings) -> bool {
    fish.enabled && fish.pet_visible
}

fn days_map(fish: &mut PetSettings) -> &mut Map<String, Value> {
    if !fish.days.is_object() {
        fish.days = Value::Object(Map::new());
    }
    fish.days.as_object_mut().expect("days is an object")
}

fn today() -> String {
    #[cfg(windows)]
    {
        atk_input::system::local_date()
    }
    #[cfg(not(windows))]
    {
        "1970-01-01".to_owned()
    }
}

/// Adds one knock for today and returns the strike payload.
fn strike(state: &mut Overlay) -> Value {
    let day = today();
    let days = days_map(&mut state.fish);
    let count = days.get(&day).and_then(Value::as_u64).unwrap_or(0) + 1;
    days.insert(day.clone(), json!(count));
    state.dirty_days = true;
    let total: u64 = days.values().filter_map(Value::as_u64).sum();
    json!({ "day": day, "count": count, "sequence": total })
}

fn snapshot(state: &Overlay) -> Value {
    json!({
        "fish": state.fish,
        "pet": state.pet,
        "level": state.pet.level(),
        "customSound": sound_path().exists(),
        "customSprite": sprite_path().exists(),
    })
}

fn flush(state: &mut Overlay) {
    if state.dirty_days {
        let target = pet_path("");
        let mut config = load_config(&target);
        if config.set("progress.days", state.fish.days.clone()).is_ok() {
            let _ = save_config(&target, &config);
        }
        state.dirty_days = false;
    }
    if state.dirty_pet {
        if let Ok(text) = serde_json::to_string_pretty(&state.pet) {
            let _ = write_text(&prefs_path(), &text);
        }
        state.dirty_pet = false;
    }
}

/// Shows, hides and sizes both overlays to match the stored settings.
pub fn apply(app: &AppHandle) {
    let Ok(state) = overlay().lock() else { return };
    if let Some(window) = app.get_webview_window(FISH) {
        let _ = if fish_shown(&state.fish) {
            window.show()
        } else {
            window.hide()
        };
        let _ = window.emit("woodenFish:settings", snapshot(&state));
    }
    if let Some(window) = app.get_webview_window(PET) {
        let (width, height) = state.pet.window_size();
        let _ = window.set_size(LogicalSize::new(width, height));
        let _ = window.set_always_on_top(state.pet.always_on_top);
        let _ = window.set_ignore_cursor_events(state.pet.click_through);
        let _ = if state.pet.visible {
            window.show()
        } else {
            window.hide()
        };
        let _ = window.emit("pet:settings", snapshot(&state));
    }
}

/// Reloads the wooden fish flags after the Pets page saved them.
pub fn reload_fish(app: &AppHandle) {
    if let Ok(mut state) = overlay().lock() {
        state.fish = PetSettings::from_config(&load_config(&pet_path("")));
    }
    apply(app);
}

fn overlay_window(
    app: &tauri::App,
    label: &str,
    page: &str,
    size: (f64, f64),
    offset: (f64, f64),
) -> tauri::Result<()> {
    let window = tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App(page.into()))
        .title(label)
        .inner_size(size.0, size.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()?;
    place(&window, size, offset)
}

fn place(window: &tauri::WebviewWindow, size: (f64, f64), offset: (f64, f64)) -> tauri::Result<()> {
    if let Some(monitor) = window.primary_monitor()? {
        let scale = monitor.scale_factor();
        let screen = monitor.size().to_logical::<f64>(scale);
        window.set_position(tauri::LogicalPosition::new(
            screen.width - size.0 - offset.0,
            screen.height - size.1 - offset.1,
        ))?;
    }
    Ok(())
}

/// Creates the hidden wooden fish and desktop pet windows.
///
/// # Errors
///
/// Returns a Tauri error when a window cannot be created.
pub fn build_overlays(app: &tauri::App) -> tauri::Result<()> {
    overlay_window(app, FISH, "woodenfish.html", (150.0, 170.0), (40.0, 90.0))?;
    let size = overlay()
        .lock()
        .map(|state| state.pet.window_size())
        .unwrap_or((192.0, 260.0));
    overlay_window(app, PET, "pet.html", size, (220.0, 60.0))?;
    apply(app.handle());
    Ok(())
}

fn key_label(vk: u8) -> String {
    match vk {
        b'A'..=b'Z' | b'0'..=b'9' => char::from(vk).to_string(),
        0x60..=0x69 => format!("Num {}", vk - 0x60),
        0x70..=0x87 => format!("F{}", vk - 0x6F),
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x10 => "Shift".into(),
        0x11 => "Ctrl".into(),
        0x12 => "Alt".into(),
        0x14 => "Caps Lock".into(),
        0x1B => "Esc".into(),
        0x20 => "Space".into(),
        0x21 => "Page Up".into(),
        0x22 => "Page Down".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "←".into(),
        0x26 => "↑".into(),
        0x27 => "→".into(),
        0x28 => "↓".into(),
        0x2C => "Print Screen".into(),
        0x2D => "Insert".into(),
        0x2E => "Delete".into(),
        0x5B | 0x5C => "Win".into(),
        0xBA => ";".into(),
        0xBB => "=".into(),
        0xBC => ",".into(),
        0xBD => "-".into(),
        0xBE => ".".into(),
        0xBF => "/".into(),
        0xC0 => "`".into(),
        0xDB => "[".into(),
        0xDC => "\\".into(),
        0xDD => "]".into(),
        0xDE => "'".into(),
        _ => format!("Key {vk}"),
    }
}

/// Keyboard keys watched for knocks: skips mouse buttons and the
/// left/right modifier duplicates of Shift, Ctrl and Alt.
fn watched(vk: u8) -> bool {
    !matches!(vk, 0x01..=0x07 | 0xA0..=0xA5 | 0xFF)
}

fn on_key(app: &AppHandle, vk: u8) {
    let Ok(mut state) = overlay().lock() else {
        return;
    };
    if fish_shown(&state.fish) {
        let payload = strike(&mut state);
        let _ = app.emit("woodenFish:strike", payload);
    }
    if state.pet.visible && state.pet.follow_input {
        let now = Instant::now();
        if state
            .last_xp
            .is_none_or(|last| now.duration_since(last) >= Duration::from_secs(10))
        {
            state.pet.xp += 1;
            state.last_xp = Some(now);
            state.dirty_pet = true;
        }
        let payload = json!({ "vk": vk, "label": key_label(vk), "level": state.pet.level() });
        let _ = app.emit("pet:input", payload);
    }
}

/// Polls global keyboard and pointer state for the overlays.
#[cfg(windows)]
pub fn spawn_input_watch(app: AppHandle) {
    std::thread::spawn(move || {
        let mut down = [false; 256];
        let mut cursor = atk_input::system::cursor_position();
        let mut last_flush = Instant::now();
        let mut tick = 0_u32;
        loop {
            std::thread::sleep(Duration::from_millis(16));
            let (active, follow) = overlay()
                .lock()
                .map(|state| {
                    let pet = state.pet.visible && state.pet.follow_input;
                    (fish_shown(&state.fish) || pet, pet)
                })
                .unwrap_or((false, false));
            if !active {
                down = [false; 256];
                std::thread::sleep(Duration::from_millis(250));
                continue;
            }
            for vk in 1_u8..=254 {
                if !watched(vk) {
                    continue;
                }
                let pressed = atk_input::system::key_down(vk);
                if pressed && !down[usize::from(vk)] {
                    on_key(&app, vk);
                }
                down[usize::from(vk)] = pressed;
            }
            tick = tick.wrapping_add(1);
            if follow && tick.is_multiple_of(6) {
                let next = atk_input::system::cursor_position();
                if let (Some((x0, y0)), Some((x1, y1))) = (cursor, next)
                    && ((x1 - x0).abs() >= 3 || (y1 - y0).abs() >= 3)
                {
                    let dir = if (x1 - x0).abs() < 3 {
                        "running"
                    } else if x1 > x0 {
                        "running-right"
                    } else {
                        "running-left"
                    };
                    let _ = app.emit("pet:pointer", json!({ "state": dir }));
                }
                cursor = next;
            }
            if last_flush.elapsed() >= Duration::from_secs(5) {
                if let Ok(mut state) = overlay().lock() {
                    flush(&mut state);
                }
                last_flush = Instant::now();
            }
        }
    });
}

#[cfg(not(windows))]
pub fn spawn_input_watch(_app: AppHandle) {}

#[tauri::command]
pub fn overlay_state() -> Result<Value, String> {
    Ok(snapshot(&*overlay().lock().map_err(lock_err)?))
}

#[tauri::command]
pub fn woodenfish_tap(app: AppHandle) -> Result<Value, String> {
    let mut state = overlay().lock().map_err(lock_err)?;
    let payload = strike(&mut state);
    flush(&mut state);
    let _ = app.emit("woodenFish:strike", payload.clone());
    Ok(payload)
}

#[tauri::command]
pub fn woodenfish_set(
    app: AppHandle,
    skin: Option<String>,
    sound: Option<bool>,
    visible: Option<bool>,
) -> Result<Value, String> {
    {
        let target = pet_path("");
        let mut config = load_config(&target);
        let mut fish = PetSettings::from_config(&config);
        if let Some(skin) = skin {
            fish.skin = skin;
        }
        if let Some(sound) = sound {
            fish.sound = sound;
        }
        if let Some(visible) = visible {
            fish.pet_visible = visible;
            if visible {
                fish.enabled = true;
            }
        }
        fish.apply(&mut config).map_err(|error| error.to_string())?;
        save_config(&target, &config)?;
    }
    reload_fish(&app);
    overlay_state()
}

#[tauri::command]
pub fn woodenfish_sound_get() -> Option<String> {
    fs::read_to_string(sound_path()).ok()
}

#[tauri::command]
pub fn woodenfish_sound_set(app: AppHandle, data: Option<String>) -> Result<Value, String> {
    match data {
        Some(data) if data.starts_with("data:audio/") => write_text(&sound_path(), &data)?,
        Some(_) => return Err("Choose an audio file".into()),
        None => {
            let _ = fs::remove_file(sound_path());
        }
    }
    apply(&app);
    overlay_state()
}

#[tauri::command]
pub fn desktop_pet_set(app: AppHandle, prefs: PetPrefs) -> Result<Value, String> {
    {
        let mut state = overlay().lock().map_err(lock_err)?;
        let xp = state.pet.xp;
        state.pet = PetPrefs { xp, ..prefs };
        state.dirty_pet = true;
        flush(&mut state);
    }
    apply(&app);
    overlay_state()
}

#[tauri::command]
pub fn desktop_pet_reset_position(app: AppHandle) -> Result<(), String> {
    let size = overlay().lock().map_err(lock_err)?.pet.window_size();
    if let Some(window) = app.get_webview_window(PET) {
        place(&window, size, (220.0, 60.0)).map_err(|error| error.to_string())?;
    }
    if let Some(window) = app.get_webview_window(FISH) {
        place(&window, (150.0, 170.0), (40.0, 90.0)).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn desktop_pet_sprite() -> Result<String, String> {
    if let Ok(custom) = fs::read_to_string(sprite_path()) {
        return Ok(custom);
    }
    tauri::async_runtime::spawn_blocking(|| {
        let mut last = String::from("Pet sprite not found");
        for path in [
            PathBuf::from(crate::FALLBACK_ASAR),
            PathBuf::from(crate::DEFAULT_ASAR),
            resolve_asar_path(),
        ] {
            match atk_features::read_asar_asset(&path, "aby-", ".webp") {
                Ok(bytes) => return Ok(atk_features::data_url("image/webp", &bytes)),
                Err(error) => last = error.to_string(),
            }
        }
        Err(last)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn desktop_pet_upload(app: AppHandle, data: Option<String>) -> Result<Value, String> {
    match data {
        Some(data) => {
            if !(data.starts_with("data:image/png;") || data.starts_with("data:image/webp;")) {
                return Err("The sprite must be a PNG or WebP image".into());
            }
            if data.len() > SPRITE_LIMIT * 4 / 3 + 64 {
                return Err("The sprite must be 4 MB or smaller".into());
            }
            write_text(&sprite_path(), &data)?;
        }
        None => {
            let _ = fs::remove_file(sprite_path());
        }
    }
    let _ = app.emit("pet:sprite", ());
    overlay_state()
}

/// Saves pending counts and experience, e.g. before exit.
pub fn flush_now() {
    if let Ok(mut state) = overlay().lock() {
        flush(&mut state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_size_matches_original_formula() {
        let mut prefs = PetPrefs::default();
        assert_eq!(prefs.window_size(), (192.0, 259.0));
        prefs.scale = 150;
        assert_eq!(prefs.window_size(), (192.0, 328.0));
    }

    #[test]
    fn strikes_count_per_day() {
        let mut state = Overlay::default();
        strike(&mut state);
        let payload = strike(&mut state);
        assert_eq!(payload["count"], 2);
        assert_eq!(payload["sequence"], 2);
    }

    #[test]
    fn reads_installed_pet_sprite() {
        let path = PathBuf::from(crate::FALLBACK_ASAR);
        if !path.exists() {
            return;
        }
        let bytes = atk_features::read_asar_asset(&path, "aby-", ".webp").expect("sprite");
        assert_eq!(&bytes[..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WEBP");
    }

    #[test]
    fn modifier_duplicates_are_skipped() {
        assert!(!watched(0xA0));
        assert!(!watched(0x01));
        assert!(watched(b'A'));
        assert_eq!(key_label(0x70), "F1");
    }
}
