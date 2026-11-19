//! Music Practice Mode (`music-practice:input` / `music-practice:key`):
//! polls `GetAsyncKeyState` every 8 ms in the background and streams key
//! transitions to the UI for follow-along score practice.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

static RUNNING: AtomicBool = AtomicBool::new(false);

/// One background key transition emitted on `music-practice:key`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeKeyEvent {
    /// Windows virtual-key code.
    pub vk: u8,
    /// Human-readable key label.
    pub key: String,
    /// Whether the key was pressed (`true`) or released (`false`).
    pub down: bool,
    /// Active mouse/modifier bitmask.
    pub buttons: u8,
}

/// Current practice mode status.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PracticeStatus {
    /// Whether the 8 ms background poller is running.
    pub running: bool,
}

fn vk_name(vk: u8) -> String {
    match vk {
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0D => "Enter".into(),
        0x10 => "Shift".into(),
        0x11 => "Ctrl".into(),
        0x12 => "Alt".into(),
        0x1B => "Esc".into(),
        0x20 => "Space".into(),
        0x25 => "Left".into(),
        0x26 => "Up".into(),
        0x27 => "Right".into(),
        0x28 => "Down".into(),
        0x30..=0x39 => (vk as char).to_string(),
        0x41..=0x5A => (vk as char).to_string(),
        0x60..=0x69 => format!("Num{}", vk - 0x60),
        0x70..=0x7B => format!("F{}", vk - 0x6F),
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
        _ => format!("VK_{vk:02X}"),
    }
}

#[tauri::command]
pub fn practice_status() -> PracticeStatus {
    PracticeStatus {
        running: RUNNING.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub fn practice_start(app: AppHandle) -> PracticeStatus {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return PracticeStatus { running: true };
    }
    std::thread::spawn(move || {
        let mut prev = atk_input::system::poll_async_key_states();
        while RUNNING.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(8));
            let cur = atk_input::system::poll_async_key_states();
            let buttons =
                u8::from(cur[0x01]) | (u8::from(cur[0x02]) << 1) | (u8::from(cur[0x04]) << 2);
            for vk in 8..=222usize {
                if cur[vk] != prev[vk] {
                    let event = PracticeKeyEvent {
                        vk: vk as u8,
                        key: vk_name(vk as u8),
                        down: cur[vk],
                        buttons,
                    };
                    let _ = app.emit("music-practice:key", event);
                }
            }
            prev = cur;
        }
    });
    PracticeStatus { running: true }
}

#[tauri::command]
pub fn practice_stop() -> PracticeStatus {
    RUNNING.store(false, Ordering::SeqCst);
    PracticeStatus { running: false }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vk_labels_cover_common_keys() {
        assert_eq!(vk_name(0x41), "A");
        assert_eq!(vk_name(0x20), "Space");
        assert_eq!(vk_name(0x70), "F1");
    }
}
