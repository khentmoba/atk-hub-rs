//! Live desktop wallpapers: HTML packages, videos and images shown in
//! borderless windows parented behind the desktop icons, one per monitor.

use crate::live::lock_err;
use crate::user_data_dir;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager};

const PLAYER: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>ATK wallpaper</title>
<style>html,body{margin:0;width:100%;height:100%;overflow:hidden;background:#000}
#media{width:100%;height:100%;object-fit:cover;display:block}</style></head>
<body><script>
const q = new URLSearchParams(location.search);
const src = q.get("src") || "";
const el = document.createElement(q.get("type") === "video" ? "video" : "img");
el.id = "media";
if (el.tagName === "VIDEO") { el.autoplay = true; el.loop = true; el.muted = true; el.playsInline = true; }
el.src = src;
document.body.appendChild(el);
window.__atkStyle = (s) => {
  el.style.filter = `brightness(${s.brightness}%) contrast(${s.contrast}%) saturate(${s.saturation}%) blur(${s.blur}px) hue-rotate(${s.hueRotate}deg) grayscale(${s.grayscale}%)`;
};
window.__atkStyle(Object.fromEntries(["brightness","contrast","saturation","blur","hueRotate","grayscale"].map((k) => [k, Number(q.get(k) ?? 0)])));
</script></body></html>
"#;

/// Wallpaper Engine web API shim injected into HTML wallpapers.
const BRIDGE: &str = r"(() => {
  const audio = [], props = [], thumb = [], playback = [], timeline = [];
  window.wallpaperRegisterAudioListener = (f) => audio.push(f);
  window.wallpaperRegisterMediaPropertiesListener = (f) => props.push(f);
  window.wallpaperRegisterMediaThumbnailListener = (f) => thumb.push(f);
  window.wallpaperRegisterMediaPlaybackListener = (f) => playback.push(f);
  window.wallpaperRegisterMediaTimelineListener = (f) => timeline.push(f);
  window.wallpaperMediaIntegration = { PLAYBACK_PLAYING: 0, PLAYBACK_PAUSED: 1, PLAYBACK_STOPPED: 2 };
  window.wallpaperPropertyListener = window.wallpaperPropertyListener || { applyUserProperties() {}, applyGeneralProperties() {} };
  window.wallpaperReady = () => {};
  window.__atkWpAudio = (d) => audio.forEach((f) => { try { f(d); } catch (e) { console.error(e); } });
  window.__atkWpPlayback = (d) => playback.forEach((f) => { try { f(d); } catch (e) { console.error(e); } });
  window.__atkWpMedia = (m) => {
    const st = m.state === 'playing' ? 0 : m.state === 'paused' ? 1 : 2;
    props.forEach((f) => { try { f({ title: m.title || '', artist: m.artist || '', albumTitle: m.album || '' }); } catch (e) { console.error(e); } });
    playback.forEach((f) => { try { f({ state: st }); } catch (e) { console.error(e); } });
    timeline.forEach((f) => { try { f({ position: m.position || 0, duration: m.duration || 0 }); } catch (e) { console.error(e); } });
  };
})();";

/// CSS filter settings for video and image wallpapers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WallStyle {
    /// 0-200 %.
    pub brightness: u32,
    /// 0-200 %.
    pub contrast: u32,
    /// 0-200 %.
    pub saturation: u32,
    /// 0-20 px.
    pub blur: u32,
    /// 0-360 degrees.
    pub hue_rotate: u32,
    /// 0-100 %.
    pub grayscale: u32,
}

impl Default for WallStyle {
    fn default() -> Self {
        Self {
            brightness: 100,
            contrast: 100,
            saturation: 100,
            blur: 0,
            hue_rotate: 0,
            grayscale: 0,
        }
    }
}

impl WallStyle {
    fn valid(&self) -> bool {
        self.brightness <= 200
            && self.contrast <= 200
            && self.saturation <= 200
            && self.blur <= 20
            && self.hue_rotate <= 360
            && self.grayscale <= 100
    }
}

/// A downloaded or imported wallpaper.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wallpaper {
    /// Catalogue id (or a generated id for imports).
    pub id: String,
    /// Title.
    pub title: String,
    /// File (video/image) or folder (HTML).
    pub local_path: PathBuf,
    /// HTML entry page.
    #[serde(default)]
    pub entry_path: Option<PathBuf>,
    /// Thumbnail address.
    #[serde(default)]
    pub preview_url: String,
    /// Unix seconds.
    #[serde(default)]
    pub downloaded_at: u64,
    /// Bytes on disk.
    #[serde(default)]
    pub file_size: u64,
    /// `html`, `video` or `image`.
    pub file_type: String,
    /// Filters.
    #[serde(default)]
    pub style: WallStyle,
}

impl Wallpaper {
    fn present(&self) -> bool {
        match &self.entry_path {
            Some(entry) if self.file_type == "html" => entry.exists(),
            _ => self.local_path.exists(),
        }
    }
}

/// Wallpaper shown on one monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Applied {
    index: usize,
    name: String,
    width: u32,
    height: u32,
    wallpaper_id: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Store {
    wallpapers: Vec<Wallpaper>,
    applied: Vec<Applied>,
    current: Option<String>,
}

fn dir() -> PathBuf {
    user_data_dir().join("wallpapers")
}

fn store_path() -> PathBuf {
    dir().join("live-wallpapers.json")
}

fn store() -> &'static Mutex<Store> {
    static STORE: OnceLock<Mutex<Store>> = OnceLock::new();
    STORE.get_or_init(|| {
        let loaded = std::fs::read_to_string(store_path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Mutex::new(loaded)
    })
}

fn save(store: &Store) -> Result<(), String> {
    std::fs::create_dir_all(dir()).map_err(|error| error.to_string())?;
    let text = serde_json::to_string_pretty(store).map_err(|error| error.to_string())?;
    std::fs::write(store_path(), text).map_err(|error| error.to_string())
}

fn cancelled() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(Mutex::default)
}

fn html_windows() -> &'static Mutex<HashSet<String>> {
    static SET: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SET.get_or_init(Mutex::default)
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// `html` for zip packages, `video` for movie files, otherwise `image`.
fn file_type_for(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    let ext = lower.rsplit('.').next().unwrap_or_default();
    match ext {
        "zip" => "html",
        "mp4" | "webm" | "mov" | "avi" | "mkv" => "video",
        _ => "image",
    }
}

fn safe_id(id: &str) -> Result<String, String> {
    let id = id.trim();
    if id.is_empty()
        || id.len() > 128
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        || id.contains("..")
    {
        return Err("invalid wallpaper id".to_owned());
    }
    Ok(id.to_owned())
}

fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path).map_or(0, |entries| {
        entries
            .flatten()
            .map(|entry| match entry.file_type() {
                Ok(kind) if kind.is_dir() => dir_size(&entry.path()),
                Ok(_) => entry.metadata().map_or(0, |meta| meta.len()),
                Err(_) => 0,
            })
            .sum()
    })
}

fn install_html(bytes: &[u8], folder: &Path) -> Result<PathBuf, String> {
    if folder.exists() {
        std::fs::remove_dir_all(folder).map_err(|error| error.to_string())?;
    }
    atk_features::extract_zip(bytes, folder).map_err(|error| error.to_string())?;
    let entry = folder.join("index.html");
    if !entry.exists() {
        let _ = std::fs::remove_dir_all(folder);
        return Err("Invalid wallpaper package: index.html not found".to_owned());
    }
    Ok(entry)
}

fn file_url(path: &Path) -> Result<tauri::Url, String> {
    tauri::Url::from_file_path(path).map_err(|()| format!("bad path {}", path.display()))
}

fn wallpaper_url(paper: &Wallpaper) -> Result<tauri::Url, String> {
    if paper.file_type == "html" {
        let entry = paper
            .entry_path
            .clone()
            .unwrap_or_else(|| paper.local_path.join("index.html"));
        return file_url(&entry);
    }
    let player = dir().join("player.html");
    std::fs::create_dir_all(dir()).map_err(|error| error.to_string())?;
    if std::fs::read_to_string(&player).ok().as_deref() != Some(PLAYER) {
        std::fs::write(&player, PLAYER).map_err(|error| error.to_string())?;
    }
    player_url(paper, &player)
}

fn player_url(paper: &Wallpaper, player: &Path) -> Result<tauri::Url, String> {
    let mut url = file_url(player)?;
    let s = paper.style;
    url.query_pairs_mut()
        .append_pair("src", file_url(&paper.local_path)?.as_str())
        .append_pair("type", &paper.file_type)
        .append_pair("brightness", &s.brightness.to_string())
        .append_pair("contrast", &s.contrast.to_string())
        .append_pair("saturation", &s.saturation.to_string())
        .append_pair("blur", &s.blur.to_string())
        .append_pair("hueRotate", &s.hue_rotate.to_string())
        .append_pair("grayscale", &s.grayscale.to_string());
    Ok(url)
}

fn label(index: usize) -> String {
    format!("wallpaper-{index}")
}

fn close_window(app: &AppHandle, index: usize) {
    let name = label(index);
    if let Ok(mut set) = html_windows().lock() {
        set.remove(&name);
    }
    if let Some(window) = app.get_webview_window(&name) {
        let _ = window.destroy();
    }
}

fn open_window(app: &AppHandle, index: usize, paper: &Wallpaper) -> Result<(), String> {
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let monitor = monitors
        .get(index)
        .ok_or_else(|| format!("display {} not found", index + 1))?;
    let (pos, size) = (*monitor.position(), *monitor.size());
    close_window(app, index);
    let name = label(index);
    let url = wallpaper_url(paper)?;
    let scale = monitor.scale_factor();
    let window = tauri::WebviewWindowBuilder::new(app, &name, tauri::WebviewUrl::External(url))
        .title("ATK wallpaper")
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .position(f64::from(pos.x) / scale, f64::from(pos.y) / scale)
        .inner_size(
            f64::from(size.width) / scale,
            f64::from(size.height) / scale,
        )
        .initialization_script(BRIDGE)
        .build()
        .map_err(|error| error.to_string())?;
    if paper.file_type == "html"
        && let Ok(mut set) = html_windows().lock()
    {
        set.insert(name);
        drop(set);
        crate::rhythm::wallpaper_needs_audio(app);
    }
    attach(app, window, pos.x, pos.y, size.width, size.height);
    Ok(())
}

#[cfg(windows)]
fn attach(app: &AppHandle, window: tauri::WebviewWindow, x: i32, y: i32, w: u32, h: u32) {
    let _ = app.run_on_main_thread(move || {
        let attached = window.hwnd().is_ok_and(|hwnd| {
            atk_input::system::attach_to_desktop(
                hwnd.0 as isize,
                x,
                y,
                i32::try_from(w).unwrap_or(i32::MAX),
                i32::try_from(h).unwrap_or(i32::MAX),
            )
        });
        if !attached {
            let _ = window.set_always_on_bottom(true);
            let _ = window.show();
        }
    });
}

#[cfg(not(windows))]
fn attach(_app: &AppHandle, window: tauri::WebviewWindow, _x: i32, _y: i32, _w: u32, _h: u32) {
    let _ = window.set_always_on_bottom(true);
    let _ = window.show();
}

fn displays(app: &AppHandle, store: &Store) -> Vec<Value> {
    app.available_monitors()
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, monitor)| {
            let applied = store.applied.iter().find(|a| a.index == index);
            let paper =
                applied.and_then(|a| store.wallpapers.iter().find(|w| w.id == a.wallpaper_id));
            json!({
                "index": index,
                "name": monitor.name().cloned().unwrap_or_else(|| format!("Display {}", index + 1)),
                "width": monitor.size().width,
                "height": monitor.size().height,
                "x": monitor.position().x,
                "y": monitor.position().y,
                "wallpaperId": applied.map(|a| a.wallpaper_id.clone()).unwrap_or_default(),
                "url": paper.map(|w| w.preview_url.clone()).unwrap_or_default(),
            })
        })
        .collect()
}

fn snapshot(app: &AppHandle) -> Result<Value, String> {
    let store = store().lock().map_err(lock_err)?;
    Ok(json!({
        "wallpapers": store.wallpapers.iter().filter(|w| w.present()).collect::<Vec<_>>(),
        "current": store.current,
        "displays": displays(app, &store),
    }))
}

/// Whether any HTML wallpaper is showing.
pub fn has_html_wallpaper() -> bool {
    html_windows().lock().is_ok_and(|set| !set.is_empty())
}

/// Forwards an audio spectrum to HTML wallpapers (Wallpaper Engine format).
pub fn forward_audio(app: &AppHandle, spectrum: &[f32]) {
    let Ok(set) = html_windows().lock() else {
        return;
    };
    if set.is_empty() {
        return;
    }
    let payload = json!({ "captureTs": now() * 1000, "spectrum": spectrum }).to_string();
    for name in set.iter() {
        if let Some(window) = app.get_webview_window(name) {
            let _ = window.eval(format!(
                "window.__atkWpAudio && window.__atkWpAudio({payload})"
            ));
        }
    }
}

/// Forwards a Windows SMTC media snapshot to HTML wallpapers.
fn forward_media(app: &AppHandle, media: &atk_input::media::MediaSnapshot) {
    let Ok(set) = html_windows().lock() else {
        return;
    };
    if set.is_empty() {
        return;
    }
    let Ok(payload) = serde_json::to_string(media) else {
        return;
    };
    for name in set.iter() {
        if let Some(window) = app.get_webview_window(name) {
            let _ = window.eval(format!(
                "window.__atkWpMedia && window.__atkWpMedia({payload})"
            ));
        }
    }
}

/// Puts the saved wallpapers back after startup and starts the SMTC media bridge.
pub fn restore(app: AppHandle) {
    let media_app = app.clone();
    std::thread::spawn(move || {
        let mut last: Option<atk_input::media::MediaSnapshot> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let has_html = html_windows()
                .lock()
                .map(|s| !s.is_empty())
                .unwrap_or(false);
            if !has_html {
                continue;
            }
            if let Some(current) = atk_input::media::current_media()
                && last.as_ref() != Some(&current)
            {
                forward_media(&media_app, &current);
                last = Some(current);
            }
        }
    });
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        let Ok(guard) = store().lock() else {
            return;
        };
        let applied = guard.applied.clone();
        let papers = guard.wallpapers.clone();
        drop(guard);
        let monitors = app.available_monitors().unwrap_or_default();
        for entry in applied {
            // Prefer the monitor with the same name; fall back to the index.
            let index = monitors
                .iter()
                .position(|m| m.name().is_some_and(|n| *n == entry.name))
                .unwrap_or(entry.index);
            if let Some(paper) = papers
                .iter()
                .find(|w| w.id == entry.wallpaper_id && w.present())
            {
                let _ = open_window(&app, index, paper);
            }
        }
    });
}

#[tauri::command]
pub fn livewall_list(app: AppHandle) -> Result<Value, String> {
    snapshot(&app)
}

#[tauri::command]
pub async fn livewall_download(
    app: AppHandle,
    id: String,
    url: String,
    title: String,
    preview_url: String,
) -> Result<Value, String> {
    let id = safe_id(&id)?;
    if let Ok(mut set) = cancelled().lock() {
        set.remove(&id);
    }
    let handle = app.clone();
    let paper = tauri::async_runtime::spawn_blocking(move || -> Result<Wallpaper, String> {
        let path_part = url.split(['?', '#']).next().unwrap_or_default();
        let kind = file_type_for(path_part);
        let response = ureq::get(&url)
            .call()
            .map_err(|error| format!("download failed: {error}"))?;
        let total: u64 = response
            .header("content-length")
            .and_then(|value| value.parse().ok())
            .unwrap_or(0);
        let mut reader = response.into_reader();
        let mut bytes = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        let mut last = 0u64;
        loop {
            if cancelled().lock().is_ok_and(|set| set.contains(&id)) {
                return Err("download cancelled".to_owned());
            }
            let read = reader.read(&mut chunk).map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..read]);
            if total > 0 {
                let percent = bytes.len() as u64 * 100 / total;
                if percent != last {
                    last = percent;
                    let _ =
                        handle.emit("livewall:progress", json!({ "id": id, "percent": percent }));
                }
            }
        }
        std::fs::create_dir_all(dir()).map_err(|error| error.to_string())?;
        let (local_path, entry_path, file_size) = if kind == "html" {
            let folder = dir().join(&id);
            let entry = install_html(&bytes, &folder)?;
            let size = dir_size(&folder);
            (folder, Some(entry), size)
        } else {
            let ext = path_part
                .rsplit('.')
                .next()
                .unwrap_or("bin")
                .to_ascii_lowercase();
            let file = dir().join(format!("{id}.{ext}"));
            std::fs::write(&file, &bytes).map_err(|error| error.to_string())?;
            (file, None, bytes.len() as u64)
        };
        Ok(Wallpaper {
            id: id.clone(),
            title,
            local_path,
            entry_path,
            preview_url,
            downloaded_at: now(),
            file_size,
            file_type: kind.to_owned(),
            style: WallStyle::default(),
        })
    })
    .await
    .map_err(|error| error.to_string())??;
    {
        let mut store = store().lock().map_err(lock_err)?;
        store.wallpapers.retain(|w| w.id != paper.id);
        store.wallpapers.push(paper);
        save(&store)?;
    }
    snapshot(&app)
}

#[tauri::command]
pub fn livewall_cancel(id: String) -> Result<(), String> {
    cancelled().lock().map_err(lock_err)?.insert(id);
    Ok(())
}

#[tauri::command]
pub async fn livewall_apply(
    app: AppHandle,
    id: String,
    displays: Option<Vec<usize>>,
) -> Result<Value, String> {
    let paper = {
        let store = store().lock().map_err(lock_err)?;
        store
            .wallpapers
            .iter()
            .find(|w| w.id == id && w.present())
            .cloned()
            .ok_or_else(|| "Wallpaper file not found — download it again".to_owned())?
    };
    let monitors = app
        .available_monitors()
        .map_err(|error| error.to_string())?;
    let targets = displays.unwrap_or_else(|| (0..monitors.len()).collect());
    for &index in &targets {
        let Some(monitor) = monitors.get(index) else {
            continue;
        };
        open_window(&app, index, &paper)?;
        let mut store = store().lock().map_err(lock_err)?;
        store.applied.retain(|a| a.index != index);
        store.applied.push(Applied {
            index,
            name: monitor.name().cloned().unwrap_or_default(),
            width: monitor.size().width,
            height: monitor.size().height,
            wallpaper_id: paper.id.clone(),
        });
        store.current = Some(paper.id.clone());
        save(&store)?;
    }
    snapshot(&app)
}

#[tauri::command]
pub async fn livewall_clear(app: AppHandle, displays: Option<Vec<usize>>) -> Result<Value, String> {
    let count = app.available_monitors().map_or(0, |m| m.len());
    let targets = displays.unwrap_or_else(|| (0..count.max(8)).collect());
    for &index in &targets {
        close_window(&app, index);
    }
    {
        let mut store = store().lock().map_err(lock_err)?;
        store.applied.retain(|a| !targets.contains(&a.index));
        if store.applied.is_empty() {
            store.current = None;
        }
        save(&store)?;
    }
    #[cfg(windows)]
    atk_input::system::refresh_desktop_wallpaper();
    snapshot(&app)
}

#[tauri::command]
pub async fn livewall_delete(app: AppHandle, id: String) -> Result<Value, String> {
    let (paper, indexes) = {
        let store = store().lock().map_err(lock_err)?;
        let paper = store.wallpapers.iter().find(|w| w.id == id).cloned();
        let indexes: Vec<usize> = store
            .applied
            .iter()
            .filter(|a| a.wallpaper_id == id)
            .map(|a| a.index)
            .collect();
        (paper, indexes)
    };
    if !indexes.is_empty() {
        livewall_clear(app.clone(), Some(indexes)).await?;
    }
    if let Some(paper) = paper {
        if paper.local_path.starts_with(dir()) {
            let _ = if paper.local_path.is_dir() {
                std::fs::remove_dir_all(&paper.local_path)
            } else {
                std::fs::remove_file(&paper.local_path)
            };
        }
        let mut store = store().lock().map_err(lock_err)?;
        store.wallpapers.retain(|w| w.id != id);
        save(&store)?;
    }
    snapshot(&app)
}

#[tauri::command]
pub fn livewall_style(app: AppHandle, id: String, style: WallStyle) -> Result<Value, String> {
    if !style.valid() {
        return Err("style value out of range".to_owned());
    }
    let indexes: Vec<usize> = {
        let mut store = store().lock().map_err(lock_err)?;
        let paper = store
            .wallpapers
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or_else(|| "unknown wallpaper".to_owned())?;
        paper.style = style;
        save(&store)?;
        store
            .applied
            .iter()
            .filter(|a| a.wallpaper_id == id)
            .map(|a| a.index)
            .collect()
    };
    let payload = serde_json::to_string(&style).map_err(|error| error.to_string())?;
    for index in indexes {
        if let Some(window) = app.get_webview_window(&label(index)) {
            let _ = window.eval(format!("window.__atkStyle && window.__atkStyle({payload})"));
        }
    }
    snapshot(&app)
}

/// Imports a wallpaper from raw file bytes. The file name comes in the
/// `x-file-name` header: `.zip` HTML packages, videos and images are accepted.
#[tauri::command]
pub async fn livewall_import(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> Result<Value, String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err("expected raw file bytes".to_owned());
    };
    let name = request
        .headers()
        .get("x-file-name")
        .and_then(|value| value.to_str().ok())
        .map(|value| String::from_utf8(percent_decode(value)).unwrap_or_default())
        .unwrap_or_else(|| "wallpaper.zip".to_owned());
    let kind = file_type_for(&name);
    let id = format!("import-{}", now());
    std::fs::create_dir_all(dir()).map_err(|error| error.to_string())?;
    let (local_path, entry_path, file_size) = if kind == "html" {
        let folder = dir().join(&id);
        let entry = install_html(bytes, &folder)?;
        let size = dir_size(&folder);
        (folder, Some(entry), size)
    } else {
        let ext = name
            .rsplit('.')
            .next()
            .unwrap_or("bin")
            .to_ascii_lowercase();
        let file = dir().join(format!("{id}.{ext}"));
        std::fs::write(&file, bytes).map_err(|error| error.to_string())?;
        (file, None, bytes.len() as u64)
    };
    let title = name
        .rsplit(['/', '\\'])
        .next()
        .and_then(|base| base.rsplit_once('.').map(|(stem, _)| stem.to_owned()))
        .unwrap_or_else(|| format!("Custom wallpaper {}", &id[7..]));
    {
        let mut store = store().lock().map_err(lock_err)?;
        store.wallpapers.push(Wallpaper {
            id,
            title,
            local_path,
            entry_path,
            preview_url: String::new(),
            downloaded_at: now(),
            file_size,
            file_type: kind.to_owned(),
            style: WallStyle::default(),
        });
        save(&store)?;
    }
    snapshot(&app)
}

/// Sets a downloaded image as the classic Windows desktop wallpaper.
#[tauri::command]
pub fn livewall_set_system(id: String) -> Result<(), String> {
    let path = {
        let store = store().lock().map_err(lock_err)?;
        let paper = store
            .wallpapers
            .iter()
            .find(|w| w.id == id && w.file_type == "image")
            .ok_or_else(|| "only image wallpapers can be set as the system wallpaper".to_owned())?;
        paper.local_path.clone()
    };
    #[cfg(windows)]
    atk_input::system::set_desktop_wallpaper(&path).map_err(|error| error.to_string())?;
    #[cfg(not(windows))]
    let _ = path;
    Ok(())
}

fn percent_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(hex) = std::str::from_utf8(&bytes[i + 1..i + 3])
            && let Ok(value) = u8::from_str_radix(hex, 16)
        {
            out.push(value);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_wallpaper_files() {
        assert_eq!(file_type_for("https://x/a/b.ZIP"), "html");
        assert_eq!(file_type_for("clip.webm"), "video");
        assert_eq!(file_type_for("pic.jpg"), "image");
    }

    #[test]
    fn rejects_unsafe_ids_and_styles() {
        assert!(safe_id("../x").is_err());
        assert!(safe_id("a/b").is_err());
        assert_eq!(safe_id(" 1024 ").unwrap(), "1024");
        assert!(WallStyle::default().valid());
        assert!(
            !WallStyle {
                blur: 21,
                ..WallStyle::default()
            }
            .valid()
        );
    }

    #[test]
    fn decodes_header_names() {
        assert_eq!(percent_decode("my%20wall.zip"), b"my wall.zip");
        assert_eq!(percent_decode("%E5%A3%81.mp4"), "壁.mp4".as_bytes());
    }

    #[test]
    fn player_url_carries_style() {
        let paper = Wallpaper {
            id: "1".into(),
            title: "t".into(),
            local_path: std::env::temp_dir().join("a b.mp4"),
            entry_path: None,
            preview_url: String::new(),
            downloaded_at: 0,
            file_size: 0,
            file_type: "video".into(),
            style: WallStyle {
                blur: 3,
                ..WallStyle::default()
            },
        };
        let url = player_url(&paper, &std::env::temp_dir().join("player.html")).unwrap();
        let query = url.query().unwrap();
        assert!(query.contains("type=video"));
        assert!(query.contains("blur=3"));
        assert!(url.path().ends_with("player.html"));
    }
}
