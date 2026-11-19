//! Keyboard 6x6 RGB dot-matrix screen player and artwork library.

use crate::user_data_dir;
use atk_hid::dotscreen::{Artwork, DotScreen, DotTarget, Frame, SIDE, list_dot_screens};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Store {
    saved: Vec<Artwork>,
    active: Option<Artwork>,
    auto_reconnect: bool,
}

impl Default for Store {
    fn default() -> Self {
        Self {
            saved: default_artworks(),
            active: None,
            auto_reconnect: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct Runtime {
    running: bool,
    connected: bool,
    artwork: Option<Artwork>,
    frame_index: usize,
    error: Option<String>,
}

/// Snapshot returned to the frontend and emitted on `dotscreen:status`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DotStatus {
    /// Whether playback is currently active.
    pub running: bool,
    /// Whether a dot-screen keyboard is currently streaming.
    pub connected: bool,
    /// Currently playing or last active artwork.
    pub artwork: Option<Artwork>,
    /// Index of the current frame.
    pub frame_index: usize,
    /// Last error, if any.
    pub error: Option<String>,
    /// Connected dot-screen keyboards.
    pub targets: Vec<DotTarget>,
    /// Saved artwork library.
    pub saved: Vec<Artwork>,
    /// Automatically reconnect when the keyboard is plugged in.
    pub auto_reconnect: bool,
}

static GENERATION: AtomicU64 = AtomicU64::new(0);
static STOP: AtomicBool = AtomicBool::new(true);

fn store_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-dotscreen.json")
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        let path = store_path();
        let loaded = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Store>(&text).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
}

fn runtime() -> &'static Mutex<Runtime> {
    static RUNTIME: OnceLock<Mutex<Runtime>> = OnceLock::new();
    RUNTIME.get_or_init(|| Mutex::new(Runtime::default()))
}

fn persist(data: &Store) {
    let path = store_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(path, json);
    }
}

fn snapshot() -> DotStatus {
    let targets = list_dot_screens().unwrap_or_default();
    let rt = runtime().lock().map(|g| g.clone()).unwrap_or_default();
    let st = store().lock().map(|g| g.clone()).unwrap_or_default();
    DotStatus {
        running: rt.running,
        connected: rt.connected,
        artwork: rt.artwork.or(st.active),
        frame_index: rt.frame_index,
        error: rt.error,
        targets,
        saved: st.saved,
        auto_reconnect: st.auto_reconnect,
    }
}

fn emit(app: &AppHandle) {
    let _ = app.emit("dotscreen:status", snapshot());
}

fn sleep_until_stop(duration: Duration, run_id: u64) -> bool {
    let deadline = Instant::now() + duration;
    while Instant::now() < deadline {
        if STOP.load(Ordering::Relaxed) || GENERATION.load(Ordering::Relaxed) != run_id {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    STOP.load(Ordering::Relaxed) || GENERATION.load(Ordering::Relaxed) != run_id
}

fn start_player(app: AppHandle, artwork: Artwork, target_path: Option<String>) {
    let run_id = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    STOP.store(false, Ordering::SeqCst);

    if let Ok(mut rt) = runtime().lock() {
        rt.running = true;
        rt.connected = false;
        rt.artwork = Some(artwork.clone());
        rt.frame_index = 0;
        rt.error = None;
    }
    emit(&app);

    std::thread::spawn(move || {
        run_loop(app, artwork, target_path, run_id);
    });
}

fn run_loop(app: AppHandle, artwork: Artwork, target_path: Option<String>, run_id: u64) {
    let interval = Duration::from_millis((1000 / u64::from(artwork.frame_rate.max(1))).max(50));

    while !STOP.load(Ordering::Relaxed) && GENERATION.load(Ordering::Relaxed) == run_id {
        let chosen = match &target_path {
            Some(path) => Some(path.clone()),
            None => list_dot_screens()
                .ok()
                .and_then(|list| list.into_iter().next().map(|t| t.path)),
        };

        let Some(path) = chosen else {
            if let Ok(mut rt) = runtime().lock() {
                rt.connected = false;
                rt.error = Some("No dot-screen keyboard connected".into());
            }
            emit(&app);
            if !store().lock().map(|s| s.auto_reconnect).unwrap_or(false) {
                break;
            }
            if sleep_until_stop(Duration::from_secs(2), run_id) {
                break;
            }
            continue;
        };

        let mut screen = match DotScreen::open(&path) {
            Ok(device) => device,
            Err(error) => {
                if let Ok(mut rt) = runtime().lock() {
                    rt.connected = false;
                    rt.error = Some(error.to_string());
                }
                emit(&app);
                if sleep_until_stop(Duration::from_secs(2), run_id) {
                    break;
                }
                continue;
            }
        };

        let previous = match screen.begin_stream() {
            Ok(prev) => prev,
            Err(error) => {
                if let Ok(mut rt) = runtime().lock() {
                    rt.connected = false;
                    rt.error = Some(error.to_string());
                }
                emit(&app);
                if sleep_until_stop(Duration::from_secs(2), run_id) {
                    break;
                }
                continue;
            }
        };

        if let Ok(mut rt) = runtime().lock() {
            rt.connected = true;
            rt.error = None;
        }
        emit(&app);

        let started = Instant::now();
        let mut last_index: Option<usize> = None;
        let mut last_write = Instant::now() - Duration::from_secs(10);
        let mut stream_err: Option<String> = None;

        while !STOP.load(Ordering::Relaxed) && GENERATION.load(Ordering::Relaxed) == run_id {
            let elapsed_ms = started.elapsed().as_millis();
            let step_ms = interval.as_millis().max(1);
            let index = usize::try_from(elapsed_ms / step_ms).unwrap_or(0) % artwork.frames.len();
            let changed =
                last_index.is_none_or(|prev| artwork.frames[prev] != artwork.frames[index]);

            if changed || last_write.elapsed() >= Duration::from_secs(4) {
                if let Err(error) = screen.write_frame(&artwork.frames[index]) {
                    stream_err = Some(error.to_string());
                    break;
                }
                last_write = Instant::now();
            }

            if last_index != Some(index) {
                last_index = Some(index);
                if let Ok(mut rt) = runtime().lock() {
                    rt.frame_index = index;
                }
                emit(&app);
            }

            let rem_ms = step_ms - (elapsed_ms % step_ms);
            let wait = Duration::from_millis(u64::try_from(rem_ms).unwrap_or(50).max(10));
            if sleep_until_stop(wait, run_id) {
                break;
            }
        }

        if stream_err.is_none() {
            let _ = screen.end_stream(&previous);
        } else if let Some(err) = stream_err {
            if let Ok(mut rt) = runtime().lock() {
                rt.connected = false;
                rt.error = Some(err);
            }
            emit(&app);
            if !store().lock().map(|s| s.auto_reconnect).unwrap_or(false) {
                break;
            }
            if sleep_until_stop(Duration::from_secs(2), run_id) {
                break;
            }
        }
    }

    if GENERATION.load(Ordering::Relaxed) == run_id {
        if let Ok(mut rt) = runtime().lock() {
            rt.running = false;
            rt.connected = false;
        }
        emit(&app);
    }
}

/// Restores the last active dot-screen animation on startup when auto-reconnect is enabled.
pub fn restore(app: AppHandle) {
    let active = store()
        .lock()
        .ok()
        .filter(|s| s.auto_reconnect)
        .and_then(|s| s.active.clone());
    if let Some(artwork) = active
        && artwork.validate().is_ok()
    {
        start_player(app, artwork, None);
    }
}

#[tauri::command]
pub fn dotscreen_status() -> DotStatus {
    snapshot()
}

#[tauri::command]
pub fn dotscreen_start(
    app: AppHandle,
    artwork: Artwork,
    path: Option<String>,
) -> Result<DotStatus, String> {
    artwork.validate().map_err(|e| e.to_string())?;
    if let Ok(mut st) = store().lock() {
        st.active = Some(artwork.clone());
        persist(&st);
    }
    start_player(app, artwork, path.filter(|p| !p.trim().is_empty()));
    Ok(snapshot())
}

#[tauri::command]
pub fn dotscreen_stop(app: AppHandle) -> DotStatus {
    STOP.store(true, Ordering::SeqCst);
    GENERATION.fetch_add(1, Ordering::SeqCst);
    if let Ok(mut rt) = runtime().lock() {
        rt.running = false;
        rt.connected = false;
        rt.error = None;
    }
    if let Ok(mut st) = store().lock() {
        st.active = None;
        persist(&st);
    }
    emit(&app);
    snapshot()
}

#[tauri::command]
pub fn dotscreen_save(artwork: Artwork) -> Result<DotStatus, String> {
    artwork.validate().map_err(|e| e.to_string())?;
    if let Ok(mut st) = store().lock() {
        let name = artwork.name.trim().to_owned();
        if let Some(existing) = st.saved.iter_mut().find(|a| a.name.trim() == name) {
            *existing = artwork;
        } else {
            st.saved.push(artwork);
        }
        persist(&st);
    }
    Ok(snapshot())
}

#[tauri::command]
pub fn dotscreen_delete(name: String) -> DotStatus {
    if let Ok(mut st) = store().lock() {
        let trimmed = name.trim();
        st.saved.retain(|a| a.name.trim() != trimmed);
        persist(&st);
    }
    snapshot()
}

#[tauri::command]
pub fn dotscreen_auto_reconnect(enabled: bool) -> DotStatus {
    if let Ok(mut st) = store().lock() {
        st.auto_reconnect = enabled;
        persist(&st);
    }
    snapshot()
}

fn frame_from_pattern(rows: [&str; SIDE], fg: [u8; 3]) -> Frame {
    let mut frame: Frame = [[[0; 3]; SIDE]; SIDE];
    for (r, row) in rows.iter().enumerate() {
        for (c, ch) in row.chars().take(SIDE).enumerate() {
            if ch != '.' {
                frame[r][c] = fg;
            }
        }
    }
    frame
}

fn default_artworks() -> Vec<Artwork> {
    let heart_small = frame_from_pattern(
        ["......", ".#..#.", "#####.", ".###..", "..#...", "......"],
        [255, 60, 90],
    );
    let heart_big = frame_from_pattern(
        [".##.##", "######", "######", ".####.", "..##..", "......"],
        [255, 40, 70],
    );
    let wave_1 = frame_from_pattern(
        ["......", "..##..", ".#..#.", "#....#", "......", "......"],
        [0, 180, 255],
    );
    let wave_2 = frame_from_pattern(
        ["......", "......", "#....#", ".#..#.", "..##..", "......"],
        [0, 220, 255],
    );
    let atk = frame_from_pattern(
        ["..##..", ".#..#.", "######", "#....#", "#....#", "......"],
        [255, 105, 0],
    );

    vec![
        Artwork {
            version: 1,
            name: "Heart Pulse".into(),
            frame_rate: 2,
            frames: vec![heart_small, heart_big],
        },
        Artwork {
            version: 1,
            name: "Cyan Wave".into(),
            frame_rate: 3,
            frames: vec![wave_1, wave_2],
        },
        Artwork {
            version: 1,
            name: "ATK Glow".into(),
            frame_rate: 1,
            frames: vec![atk],
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_presets_are_valid() {
        let presets = default_artworks();
        assert_eq!(presets.len(), 3);
        for art in presets {
            assert!(art.validate().is_ok());
        }
    }
}
