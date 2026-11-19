//! Floating selection toolbar: grabs selected text anywhere and offers
//! translate / summarize / polish / search actions next to the cursor.

use crate::live::lock_err;
use crate::user_data_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Emitter, Manager};

const WINDOW: &str = "toolbar";
const SIZE: (f64, f64) = (380.0, 220.0);

/// One toolbar button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolbarButton {
    /// Stable id.
    pub id: String,
    /// `translate`, `summarize`, `polish` or `search`.
    pub kind: String,
    /// Caption.
    pub label: String,
    /// Shown on the toolbar.
    pub enabled: bool,
    /// Sort order.
    pub order: u32,
}

/// User-configured AI endpoint (`POST {baseUrl}/{path}` with `{action, text}`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AiEndpoint {
    /// Server base address.
    pub base_url: String,
    /// Optional bearer token.
    pub api_key: String,
    /// Request path.
    pub path: String,
}

impl Default for AiEndpoint {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            api_key: String::new(),
            path: "/api/ai-tools".to_owned(),
        }
    }
}

/// Toolbar preferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ToolbarPrefs {
    /// Master switch.
    pub enabled: bool,
    /// Show after selecting text with a mouse drag.
    pub mouse_gesture: bool,
    /// Show after pressing Ctrl twice.
    pub double_ctrl: bool,
    /// Show whenever new text is copied.
    pub clipboard_monitor: bool,
    /// Put the previous clipboard back after grabbing a selection.
    pub restore_clipboard: bool,
    /// Fade out after this many milliseconds (0 keeps it open).
    pub auto_hide_ms: u32,
    /// Buttons.
    pub buttons: Vec<ToolbarButton>,
    /// AI endpoint.
    pub ai: AiEndpoint,
}

impl Default for ToolbarPrefs {
    fn default() -> Self {
        let button = |order: u32, kind: &str, label: &str| ToolbarButton {
            id: kind.to_owned(),
            kind: kind.to_owned(),
            label: label.to_owned(),
            enabled: true,
            order,
        };
        Self {
            enabled: false,
            mouse_gesture: true,
            double_ctrl: true,
            clipboard_monitor: false,
            restore_clipboard: true,
            auto_hide_ms: 4000,
            buttons: vec![
                button(0, "translate", "Translate"),
                button(1, "summarize", "Summarize"),
                button(2, "polish", "Polish"),
                button(3, "search", "Search"),
            ],
            ai: AiEndpoint::default(),
        }
    }
}

fn prefs_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-toolbar.json")
}

fn prefs() -> &'static Mutex<ToolbarPrefs> {
    static PREFS: OnceLock<Mutex<ToolbarPrefs>> = OnceLock::new();
    PREFS.get_or_init(|| {
        let loaded = std::fs::read_to_string(prefs_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
}

/// Creates the hidden toolbar window.
///
/// # Errors
///
/// Returns an error when the window cannot be created.
pub fn build_toolbar(app: &tauri::App) -> tauri::Result<()> {
    tauri::WebviewWindowBuilder::new(app, WINDOW, tauri::WebviewUrl::App("toolbar.html".into()))
        .title("Selection toolbar")
        .inner_size(SIZE.0, SIZE.1)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()?;
    Ok(())
}

fn show(app: &AppHandle, text: &str) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    let Some(window) = app.get_webview_window(WINDOW) else {
        return;
    };
    if let Some((x, y)) = atk_input::system::cursor_position() {
        let _ = window.set_position(tauri::PhysicalPosition::new(x + 12, y + 18));
    }
    let _ = window.emit("toolbar:show", text.chars().take(4000).collect::<String>());
    let _ = window.show();
}

#[cfg(windows)]
fn grab_selection(restore: bool) -> Option<String> {
    use atk_input::system::{clipboard_sequence, clipboard_text, send_copy, set_clipboard_text};
    use std::time::{Duration, Instant};
    let previous = if restore { clipboard_text() } else { None };
    let before = clipboard_sequence();
    // Wait for the user's modifier keys to come up so the copy is clean.
    let deadline = Instant::now() + Duration::from_millis(400);
    while [0x10, 0x11, 0x12]
        .iter()
        .any(|vk| atk_input::system::key_down(*vk))
        && Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(10));
    }
    send_copy();
    let deadline = Instant::now() + Duration::from_millis(350);
    while clipboard_sequence() == before && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(15));
    }
    if clipboard_sequence() == before {
        return None;
    }
    let text = clipboard_text();
    if let Some(previous) = previous {
        set_clipboard_text(&previous);
    }
    text
}

#[cfg(windows)]
fn own_window_focused() -> bool {
    let own = std::env::current_exe().ok().and_then(|path| {
        path.file_name()
            .map(|name| name.to_string_lossy().to_lowercase())
    });
    let front = atk_input::system::foreground_process().map(|name| name.to_lowercase());
    own.is_some() && own == front
}

/// Watches for text selections and pops the toolbar up.
#[cfg(windows)]
pub fn spawn_selection_watch(app: AppHandle) {
    use atk_input::system::{clipboard_sequence, clipboard_text, cursor_position, key_down};
    use std::time::{Duration, Instant};
    std::thread::spawn(move || {
        let mut ctrl_down: Option<Instant> = None;
        let mut last_ctrl_tap: Option<Instant> = None;
        let mut drag: Option<((i32, i32), Instant)> = None;
        let mut clip_seq = clipboard_sequence();
        loop {
            let current = prefs().lock().map(|p| p.clone()).unwrap_or_default();
            if !current.enabled {
                std::thread::sleep(Duration::from_millis(250));
                clip_seq = clipboard_sequence();
                continue;
            }
            std::thread::sleep(Duration::from_millis(20));
            let mut grab = false;

            let ctrl = key_down(0x11);
            match (ctrl, ctrl_down) {
                (true, None) => ctrl_down = Some(Instant::now()),
                (false, Some(start)) => {
                    ctrl_down = None;
                    if start.elapsed() < Duration::from_millis(300) {
                        if last_ctrl_tap
                            .is_some_and(|tap| tap.elapsed() < Duration::from_millis(450))
                        {
                            last_ctrl_tap = None;
                            grab = current.double_ctrl;
                        } else {
                            last_ctrl_tap = Some(Instant::now());
                        }
                    }
                }
                _ => {}
            }

            let left = key_down(0x01);
            match (left, drag) {
                (true, None) => drag = cursor_position().map(|pos| (pos, Instant::now())),
                (false, Some((start, since))) => {
                    drag = None;
                    if let Some(end) = cursor_position() {
                        let distance = (end.0 - start.0).abs().max((end.1 - start.1).abs());
                        if distance >= 12 && since.elapsed() > Duration::from_millis(120) {
                            grab = grab || current.mouse_gesture;
                        }
                    }
                }
                _ => {}
            }

            if grab && !own_window_focused() {
                if let Some(text) = grab_selection(current.restore_clipboard) {
                    show(&app, &text);
                }
                clip_seq = clipboard_sequence();
                continue;
            }

            let seq = clipboard_sequence();
            if seq != clip_seq {
                clip_seq = seq;
                if current.clipboard_monitor
                    && !own_window_focused()
                    && let Some(text) = clipboard_text()
                {
                    show(&app, &text);
                }
            }
        }
    });
}

/// Selection watching needs Win32.
#[cfg(not(windows))]
pub fn spawn_selection_watch(_app: AppHandle) {}

#[tauri::command]
pub fn toolbar_get() -> Result<ToolbarPrefs, String> {
    Ok(prefs().lock().map_err(lock_err)?.clone())
}

#[tauri::command]
pub fn toolbar_set(settings: ToolbarPrefs) -> Result<ToolbarPrefs, String> {
    let text = serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?;
    std::fs::write(prefs_path(), text).map_err(|error| error.to_string())?;
    *prefs().lock().map_err(lock_err)? = settings.clone();
    Ok(settings)
}

#[tauri::command]
pub fn toolbar_preview(app: AppHandle, text: String) {
    show(&app, &text);
}

#[tauri::command]
pub fn toolbar_hide(app: AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        let _ = window.hide();
    }
}

#[tauri::command]
pub fn toolbar_copy(text: String) {
    #[cfg(windows)]
    atk_input::system::set_clipboard_text(&text);
    #[cfg(not(windows))]
    let _ = text;
}

/// Runs a toolbar action and returns the text to show (empty for search).
#[tauri::command]
pub async fn toolbar_action(kind: String, text: String) -> Result<String, String> {
    let text = text.trim().to_owned();
    if text.is_empty() {
        return Ok(String::new());
    }
    if kind == "search" {
        let url = format!("https://www.bing.com/search?q={}", url_encode(&text));
        #[cfg(windows)]
        atk_input::system::open_url(&url);
        return Ok(String::new());
    }
    let ai = prefs().lock().map_err(lock_err)?.ai.clone();
    let base = ai.base_url.trim().trim_end_matches('/').to_owned();
    if base.is_empty() {
        return Err("Set an AI server address on the Selection Toolbar page first".to_owned());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let url = format!("{base}/{}", ai.path.trim_start_matches('/'));
        let mut request = ureq::post(&url).set("Content-Type", "application/json");
        if !ai.api_key.trim().is_empty() {
            request = request.set("Authorization", &format!("Bearer {}", ai.api_key.trim()));
        }
        let body: serde_json::Value = request
            .send_json(serde_json::json!({ "action": kind, "text": text }))
            .map_err(|error| format!("AI request failed: {error}"))?
            .into_json()
            .map_err(|error| error.to_string())?;
        body.get("result")
            .or_else(|| body.get("message"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| "AI reply has no result text".to_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn url_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                char::from(byte).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_search_terms() {
        assert_eq!(url_encode("a b&c"), "a%20b%26c");
        assert_eq!(url_encode("é"), "%C3%A9");
    }

    #[test]
    fn defaults_have_four_buttons() {
        let prefs: ToolbarPrefs = serde_json::from_str("{\"enabled\":true}").unwrap();
        assert!(prefs.enabled);
        assert_eq!(prefs.buttons.len(), 4);
        assert_eq!(prefs.ai.path, "/api/ai-tools");
    }
}
