//! PC scripts: per-profile Lua scripts whose `on_tick` runs every 16 ms,
//! ported from the original Hub's fengari runtime and its Lua API.

use crate::live::lock_err;
use crate::user_data_dir;
use mlua::{Function, Lua, LuaOptions, MultiValue, StdLib, Table, Value as LuaValue, Variadic};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PRELUDE: &str = include_str!("pc_prelude.lua");
const LOG_LIMIT: usize = 500;
const SCRIPT_NAME: &str = "your script";

/// One saved script profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptProfile {
    /// Stable id.
    #[serde(default)]
    pub id: String,
    /// Display name.
    #[serde(default)]
    pub name: String,
    /// Lua source defining `on_tick`.
    #[serde(default)]
    pub lua_script: String,
    /// Run this profile.
    #[serde(default)]
    pub enabled: bool,
}

/// Persisted PC script settings, matching the original `pcScript.v1` key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptConfig {
    /// Any profile enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Start enabled scripts when the Hub starts.
    #[serde(default = "yes")]
    pub resume_on_startup: bool,
    /// Script profiles.
    #[serde(default)]
    pub profiles: Vec<ScriptProfile>,
    /// Profile shown in the editor.
    #[serde(default)]
    pub active_profile_id: Option<String>,
    /// Per-profile foreground executables; empty runs everywhere.
    #[serde(default)]
    pub profile_foreground_exes: BTreeMap<String, Vec<String>>,
}

fn yes() -> bool {
    true
}

impl Default for ScriptConfig {
    fn default() -> Self {
        let id = new_id();
        Self {
            enabled: false,
            resume_on_startup: true,
            profiles: vec![ScriptProfile {
                id: id.clone(),
                name: "Default".into(),
                lua_script: String::new(),
                enabled: false,
            }],
            active_profile_id: Some(id),
            profile_foreground_exes: BTreeMap::new(),
        }
    }
}

fn new_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |time| time.as_nanos());
    format!("{nanos:x}-{:x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

impl ScriptConfig {
    /// Fills missing ids and names, drops filters for unknown profiles and
    /// normalizes executable paths.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        if self.profiles.is_empty() {
            return Self {
                resume_on_startup: self.resume_on_startup,
                ..Self::default()
            };
        }
        for profile in &mut self.profiles {
            if profile.id.trim().is_empty() {
                profile.id = new_id();
            }
            if profile.name.trim().is_empty() {
                profile.name = "Untitled".into();
            }
            profile.name = profile.name.trim().to_owned();
        }
        let ids: Vec<&str> = self.profiles.iter().map(|p| p.id.as_str()).collect();
        if self
            .active_profile_id
            .as_deref()
            .is_some_and(|id| !ids.contains(&id))
        {
            self.active_profile_id = None;
        }
        self.profile_foreground_exes = self
            .profile_foreground_exes
            .into_iter()
            .filter(|(id, _)| ids.contains(&id.as_str()))
            .map(|(id, exes)| {
                let exes = exes
                    .iter()
                    .map(|exe| exe.trim().replace('/', "\\").to_lowercase())
                    .filter(|exe| !exe.is_empty())
                    .collect::<Vec<_>>();
                (id, exes)
            })
            .filter(|(_, exes)| !exes.is_empty())
            .collect();
        self.enabled = self.profiles.iter().any(|p| p.enabled);
        self
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry {
    ts: u128,
    profile_id: String,
    message: String,
}

#[derive(Default)]
struct Runtime {
    generation: u64,
    running: bool,
    tick_errors: BTreeMap<String, Option<String>>,
    logs: VecDeque<LogEntry>,
}

fn runtime() -> &'static Mutex<Runtime> {
    static STATE: OnceLock<Mutex<Runtime>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

fn push_log(profile_id: &str, message: String) {
    if let Ok(mut state) = runtime().lock() {
        state.logs.push_back(LogEntry {
            ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |time| time.as_millis()),
            profile_id: profile_id.to_owned(),
            message,
        });
        while state.logs.len() > LOG_LIMIT {
            state.logs.pop_front();
        }
    }
}

fn config_path() -> PathBuf {
    user_data_dir().join("atk-hub-rs-pc-script.json")
}

fn load_config() -> ScriptConfig {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<ScriptConfig>(&text).ok())
        .unwrap_or_default()
        .normalized()
}

fn save_config(config: &ScriptConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|error| error.to_string())?;
    fs::write(path, text).map_err(|error| error.to_string())
}

/// Windows input used by the Lua API; inert elsewhere.
mod os {
    #[cfg(windows)]
    pub use atk_input::system::{
        cursor_position, key_down, mouse_button, mouse_move_abs, mouse_move_rel, send_text,
        send_unicode,
    };

    #[cfg(windows)]
    pub fn key(vk: u8, down: bool) {
        atk_input::send_vk(vk, down);
    }

    #[cfg(windows)]
    pub fn foreground() -> Option<String> {
        atk_input::system::foreground_process()
    }

    #[cfg(not(windows))]
    mod stub {
        pub fn cursor_position() -> Option<(i32, i32)> {
            None
        }
        pub fn key_down(_: u8) -> bool {
            false
        }
        pub fn mouse_button(_: u8, _: bool) {}
        pub fn mouse_move_abs(_: i32, _: i32) {}
        pub fn mouse_move_rel(_: i32, _: i32) {}
        pub fn send_text(_: &str) {}
        pub fn send_unicode(_: u16) {}
        pub fn key(_: u8, _: bool) {}
        pub fn foreground() -> Option<String> {
            None
        }
    }
    #[cfg(not(windows))]
    pub use stub::*;
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn vk_of(value: f64) -> u8 {
    (value.round() as i64 & 0xFF) as u8
}

#[allow(clippy::cast_possible_truncation)]
fn int(value: f64) -> i32 {
    value
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn chord(main: u8, modifiers: &[u8]) {
    for &modifier in modifiers {
        os::key(modifier, true);
    }
    os::key(main, true);
    os::key(main, false);
    for &modifier in modifiers.iter().rev() {
        os::key(modifier, false);
    }
}

fn join_values(values: &MultiValue) -> String {
    values
        .iter()
        .map(|value| match value {
            LuaValue::Nil => "nil".to_owned(),
            LuaValue::Boolean(flag) => flag.to_string(),
            LuaValue::Integer(number) => number.to_string(),
            LuaValue::Number(number) => number.to_string(),
            LuaValue::String(text) => text.to_string_lossy(),
            other => format!("<{}>", other.type_name()),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn install_api(lua: &Lua, profile_id: &str) -> mlua::Result<()> {
    let globals = lua.globals();
    for name in ["load", "loadfile", "dofile"] {
        globals.set(name, LuaValue::Nil)?;
    }
    let input: Table = lua.create_table()?;
    for (name, vk) in [
        ("VK_LBUTTON", 1),
        ("VK_RBUTTON", 2),
        ("VK_MBUTTON", 4),
        ("VK_XBUTTON1", 5),
        ("VK_XBUTTON2", 6),
        ("VK_BACK", 8),
        ("VK_TAB", 9),
        ("VK_RETURN", 13),
        ("VK_SHIFT", 16),
        ("VK_CONTROL", 17),
        ("VK_MENU", 18),
        ("VK_ESCAPE", 27),
        ("VK_SPACE", 32),
        ("VK_LEFT", 37),
        ("VK_UP", 38),
        ("VK_RIGHT", 39),
        ("VK_DOWN", 40),
    ] {
        input.set(name, vk)?;
    }
    for letter in b'A'..=b'Z' {
        input.set(format!("VK_{}", char::from(letter)), letter)?;
    }
    for digit in 0..=9_u8 {
        input.set(format!("VK_{digit}"), 48 + digit)?;
    }
    for f in 1..=12_u8 {
        input.set(format!("VK_F{f}"), 111 + f)?;
    }
    input.set(
        "move_rel",
        lua.create_function(|_, (dx, dy): (f64, f64)| {
            os::mouse_move_rel(int(dx), int(dy));
            Ok(())
        })?,
    )?;
    input.set(
        "move_abs",
        lua.create_function(|_, (x, y): (f64, f64)| {
            os::mouse_move_abs(int(x), int(y));
            Ok(())
        })?,
    )?;
    input.set(
        "get_cursor_pos",
        lua.create_function(|lua, ()| {
            os::cursor_position().map_or(Ok(LuaValue::Nil), |(x, y)| {
                let table = lua.create_table()?;
                table.set("x", x)?;
                table.set("y", y)?;
                Ok(LuaValue::Table(table))
            })
        })?,
    )?;
    input.set(
        "is_key_down",
        lua.create_function(|_, vk: f64| Ok(os::key_down(vk_of(vk))))?,
    )?;
    input.set(
        "is_key_down_raw",
        lua.create_function(|_, vk: f64| Ok(i32::from(os::key_down(vk_of(vk)))))?,
    )?;
    for (name, vk) in [
        ("mouse_lmb_raw", 1),
        ("mouse_rmb_raw", 2),
        ("mouse_mmb_raw", 4),
        ("mouse_xbutton1_raw", 5),
        ("mouse_xbutton2_raw", 6),
    ] {
        input.set(
            name,
            lua.create_function(move |_, ()| Ok(i32::from(os::key_down(vk))))?,
        )?;
    }
    input.set(
        "send_unicode_utf16",
        lua.create_function(|_, unit: f64| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            os::send_unicode((unit.round() as i64 & 0xFFFF) as u16);
            Ok(())
        })?,
    )?;
    input.set(
        "send_key_tap",
        lua.create_function(|_, vk: f64| {
            chord(vk_of(vk), &[]);
            Ok(())
        })?,
    )?;
    globals.set("input", input)?;

    let id = profile_id.to_owned();
    globals.set(
        "input_log_native",
        lua.create_function(move |_, values: MultiValue| {
            push_log(&id, join_values(&values));
            Ok(())
        })?,
    )?;
    globals.set(
        "get_cursor_pos_native",
        lua.create_function(|_, ()| {
            Ok(os::cursor_position().map_or((None, None), |(x, y)| (Some(x), Some(y))))
        })?,
    )?;
    globals.set(
        "move_abs_native",
        lua.create_function(|_, (x, y): (Option<f64>, Option<f64>)| {
            if let (Some(x), Some(y)) = (x, y) {
                os::mouse_move_abs(int(x), int(y));
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "move_rel_native",
        lua.create_function(|_, (dx, dy): (Option<f64>, Option<f64>)| {
            if let (Some(dx), Some(dy)) = (dx, dy) {
                os::mouse_move_rel(int(dx), int(dy));
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "send_unicode_utf16_native",
        lua.create_function(|_, unit: Option<f64>| {
            if let Some(unit) = unit {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                os::send_unicode((unit.round() as i64 & 0xFFFF) as u16);
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "send_text_native",
        lua.create_function(|_, text: Option<String>| {
            if let Some(text) = text {
                os::send_text(&text);
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "send_key_tap_native",
        lua.create_function(|_, vk: Option<f64>| {
            if let Some(vk) = vk {
                chord(vk_of(vk), &[]);
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "send_key_down_native",
        lua.create_function(|_, vk: Option<f64>| {
            if let Some(vk) = vk {
                os::key(vk_of(vk), true);
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "send_key_up_native",
        lua.create_function(|_, vk: Option<f64>| {
            if let Some(vk) = vk {
                os::key(vk_of(vk), false);
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "chord_tap_native",
        lua.create_function(|_, (main, modifiers): (Option<f64>, Variadic<f64>)| {
            if let Some(main) = main {
                let modifiers: Vec<u8> = modifiers.iter().copied().map(vk_of).collect();
                chord(vk_of(main), &modifiers);
            }
            Ok(())
        })?,
    )?;
    globals.set(
        "shift_key_tap_native",
        lua.create_function(|_, vk: Option<f64>| {
            if let Some(vk) = vk {
                chord(vk_of(vk), &[16]);
            }
            Ok(())
        })?,
    )?;
    for (name, down, up) in [
        ("mouse_click_native", true, true),
        ("mouse_down_native", true, false),
        ("mouse_up_native", false, true),
    ] {
        globals.set(
            name,
            lua.create_function(move |_, button: Option<f64>| {
                if let Some(button) = button {
                    let button = vk_of(button);
                    if down {
                        os::mouse_button(button, true);
                    }
                    if up {
                        os::mouse_button(button, false);
                    }
                }
                Ok(())
            })?,
        )?;
    }
    globals.set(
        "pc_key_pressed_native",
        lua.create_function(|_, vk: Option<f64>| {
            Ok(vk.map_or(0, |vk| i32::from(os::key_down(vk_of(vk)))))
        })?,
    )?;
    Ok(())
}

/// Turns a Lua error into the original's friendly message with line context.
fn friendly(error: &mlua::Error, source: &str) -> String {
    let raw = match error {
        mlua::Error::CallbackError { cause, .. } => cause.to_string(),
        other => other.to_string(),
    };
    let first = raw.lines().next().unwrap_or("Unknown Lua error").trim();
    let mut text = first
        .replace("runtime error: ", "")
        .replace("syntax error: ", "")
        .replacen("script:", &format!("{SCRIPT_NAME}:"), 1);
    if let Some(line) = text
        .split(':')
        .nth(1)
        .and_then(|part| part.trim().parse::<usize>().ok())
        .filter(|line| *line > 0)
    {
        text = text.replacen(&format!(":{line}:"), &format!(" line {line}:"), 1);
        let lines: Vec<&str> = source.lines().collect();
        let mut context = Vec::new();
        for number in line.saturating_sub(1).max(1)..=line + 1 {
            if let Some(code) = lines.get(number - 1) {
                let marker = if number == line { ">" } else { " " };
                context.push(format!("{marker} {number} | {code}"));
            }
        }
        if !context.is_empty() {
            text.push_str("\n---\n");
            text.push_str(&context.join("\n"));
        }
    }
    if text.contains("attempt to call a nil value") {
        text.push_str(
            "\nHint: the script calls a function that does not exist; check the API list.",
        );
    } else if text.contains("'end' expected") || text.contains("unfinished") {
        text.push_str("\nHint: function / if / for / while and end do not match.");
    } else if text.contains("syntax error")
        || text.contains("unexpected symbol")
        || text.contains("expected")
    {
        text.push_str("\nHint: check this line and the one above for a missing end or bracket.");
    }
    text
}

fn build_state(source: &str, profile_id: &str) -> Result<Lua, String> {
    let lua = Lua::new_with(
        StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::MATH | StdLib::UTF8,
        LuaOptions::default(),
    )
    .map_err(|error| error.to_string())?;
    install_api(&lua, profile_id).map_err(|error| error.to_string())?;
    lua.load(PRELUDE)
        .set_name("=pc-script-bootstrap")
        .exec()
        .map_err(|error| format!("Hub prelude: {error}"))?;
    lua.load(source)
        .set_name("=script")
        .exec()
        .map_err(|error| friendly(&error, source))?;
    Ok(lua)
}

fn validate(source: &str) -> Result<(), String> {
    build_state(source, "__validate__").map(drop)
}

fn tick(lua: &Lua, source: &str) -> Option<String> {
    let on_tick: Function = match lua.globals().get::<LuaValue>("on_tick") {
        Ok(LuaValue::Function(function)) => function,
        _ => return None,
    };
    on_tick
        .call::<()>(())
        .err()
        .map(|error| friendly(&error, source))
}

fn stem(name: &str) -> String {
    let lower = name.to_lowercase();
    lower
        .strip_suffix(".exe")
        .map(str::to_owned)
        .unwrap_or(lower)
}

fn basename(path: &str) -> String {
    path.rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_lowercase()
}

/// Matches `Game-Win64-Shipping.exe` style names against a configured `game.exe`.
fn related(actual: &str, wanted: &str) -> bool {
    let (actual, wanted) = (stem(actual), stem(wanted));
    if actual == wanted {
        return true;
    }
    if wanted.len() < 2
        || !(actual.starts_with(&format!("{wanted}-")) || actual.starts_with(&format!("{wanted}_")))
    {
        return false;
    }
    [
        "win64",
        "win32",
        "shipping",
        "_test",
        "-test",
        "editor",
        "development",
    ]
    .iter()
    .any(|marker| actual.contains(marker))
}

fn exe_matches(foreground: Option<&str>, filters: &[String]) -> bool {
    let Some(foreground) = foreground.filter(|_| !filters.is_empty()) else {
        return true;
    };
    let full = foreground.replace('/', "\\").to_lowercase();
    let base = basename(&full);
    filters.iter().any(|filter| {
        let wanted = filter.trim().replace('/', "\\").to_lowercase();
        !wanted.is_empty()
            && (full == wanted || basename(&wanted) == base || related(&base, &basename(&wanted)))
    })
}

fn stop() {
    if let Ok(mut state) = runtime().lock() {
        state.generation += 1;
        state.running = false;
        state.tick_errors.clear();
    }
}

fn start(config: &ScriptConfig) -> Result<BTreeMap<String, String>, String> {
    stop();
    let mut load_errors = BTreeMap::new();
    let mut states = Vec::new();
    for profile in config.profiles.iter().filter(|p| p.enabled) {
        let source = profile.lua_script.trim();
        if source.is_empty() {
            continue;
        }
        match build_state(source, &profile.id) {
            Ok(lua) => states.push((profile.id.clone(), source.to_owned(), lua)),
            Err(error) => {
                load_errors.insert(profile.id.clone(), error);
            }
        }
    }
    if states.is_empty() {
        return Err(load_errors
            .values()
            .next()
            .cloned()
            .unwrap_or_else(|| "No script to run".into()));
    }
    let filters = config.profile_foreground_exes.clone();
    let generation = {
        let mut state = runtime().lock().map_err(lock_err)?;
        state.running = true;
        state.generation
    };
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_millis(16));
            if runtime()
                .lock()
                .map_or(true, |state| state.generation != generation)
            {
                break;
            }
            let foreground = if filters.is_empty() {
                None
            } else {
                os::foreground()
            };
            let mut errors = BTreeMap::new();
            for (id, source, lua) in &states {
                let allowed = filters
                    .get(id)
                    .is_none_or(|exes| exe_matches(foreground.as_deref(), exes));
                errors.insert(id.clone(), if allowed { tick(lua, source) } else { None });
            }
            if let Ok(mut state) = runtime().lock()
                && state.generation == generation
            {
                state.tick_errors = errors;
            }
        }
    });
    Ok(load_errors)
}

fn last_tick_error(state: &Runtime) -> Option<String> {
    let lines: Vec<String> = state
        .tick_errors
        .iter()
        .filter_map(|(id, error)| error.as_ref().map(|error| format!("[{id}] {error}")))
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Saves the config, validates enabled scripts (disabling broken ones) and
/// restarts the runtime, like the original `save-and-apply`.
fn save_and_apply(config: ScriptConfig) -> Value {
    let mut config = config.normalized();
    let mut profile_errors = BTreeMap::new();
    for profile in &mut config.profiles {
        if !profile.enabled || profile.lua_script.trim().is_empty() {
            continue;
        }
        if let Err(error) = validate(profile.lua_script.trim()) {
            profile_errors.insert(profile.id.clone(), error);
            profile.enabled = false;
        }
    }
    config.enabled = config.profiles.iter().any(|p| p.enabled);
    if let Err(error) = save_config(&config) {
        return json!({ "ok": false, "error": error });
    }
    let warnings: Vec<String> = config
        .profiles
        .iter()
        .filter_map(|profile| {
            profile_errors.get(&profile.id).map(|error| {
                format!(
                    "\u{300c}{}\u{300d}: {} (disabled until fixed)",
                    profile.name,
                    error.lines().next().unwrap_or_default()
                )
            })
        })
        .collect();
    let runnable = config
        .profiles
        .iter()
        .any(|p| p.enabled && !p.lua_script.trim().is_empty());
    if !runnable {
        stop();
        return json!({ "ok": true, "profileErrors": profile_errors, "warnings": warnings, "config": config });
    }
    match start(&config) {
        Ok(_) => {
            json!({ "ok": true, "profileErrors": profile_errors, "warnings": warnings, "config": config })
        }
        Err(error) => {
            stop();
            json!({ "ok": false, "error": error, "profileErrors": profile_errors, "config": config })
        }
    }
}

/// Starts enabled scripts at launch when "resume on startup" is on.
pub fn resume_on_startup() {
    let config = load_config();
    if config.resume_on_startup
        && config
            .profiles
            .iter()
            .any(|p| p.enabled && !p.lua_script.trim().is_empty())
    {
        let result = save_and_apply(config);
        if result["ok"] != json!(true) {
            push_log("__hub__", format!("Resume failed: {}", result["error"]));
        }
    }
}

#[tauri::command]
pub fn pc_script_get_state() -> Result<Value, String> {
    let state = runtime().lock().map_err(lock_err)?;
    let errors: BTreeMap<&String, &Option<String>> = state.tick_errors.iter().collect();
    Ok(json!({
        "platformOk": cfg!(windows),
        "running": state.running,
        "config": load_config(),
        "lastLuaTickError": last_tick_error(&state),
        "profileTickErrors": errors,
    }))
}

#[tauri::command]
pub async fn pc_script_save_and_apply(config: ScriptConfig) -> Value {
    tauri::async_runtime::spawn_blocking(move || save_and_apply(config))
        .await
        .unwrap_or_else(|error| json!({ "ok": false, "error": error.to_string() }))
}

#[tauri::command]
pub fn pc_script_validate(lua_script: String) -> Value {
    let source = lua_script.trim();
    if source.is_empty() {
        return json!({ "ok": false, "error": "The script is empty" });
    }
    match validate(source) {
        Ok(()) => json!({ "ok": true }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

#[tauri::command]
pub fn pc_script_stop() -> Value {
    stop();
    json!({ "ok": true })
}

#[tauri::command]
pub fn pc_script_logs() -> Result<Value, String> {
    let state = runtime().lock().map_err(lock_err)?;
    Ok(
        json!({ "logs": state.logs, "running": state.running, "lastLuaTickError": last_tick_error(&state) }),
    )
}

#[tauri::command]
pub fn pc_script_clear_logs() -> Result<Value, String> {
    runtime().lock().map_err(lock_err)?.logs.clear();
    Ok(json!({ "ok": true }))
}

#[tauri::command]
pub fn pc_script_probe_keys() -> Value {
    json!({
        "VK_LBUTTON": os::key_down(1),
        "VK_RBUTTON": os::key_down(2),
        "VK_XBUTTON1": os::key_down(5),
        "VK_XBUTTON2": os::key_down(6),
        "VK_A": os::key_down(65),
        "VK_SPACE": os::key_down(32),
    })
}

#[tauri::command]
pub fn pc_script_debug_move(dx: Option<f64>, dy: Option<f64>) -> Value {
    os::mouse_move_rel(int(dx.unwrap_or(0.0)), int(dy.unwrap_or(10.0)));
    json!({ "ok": true })
}

/// Writes a script to the Downloads folder and returns the path.
#[tauri::command]
pub fn pc_script_export(name: String, content: String) -> Result<String, String> {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let dir = std::env::var_os("USERPROFILE")
        .map(|home| PathBuf::from(home).join("Downloads"))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(user_data_dir);
    let path = dir.join(format!(
        "{}.lua",
        if safe.is_empty() { "script" } else { &safe }
    ));
    fs::write(&path, content).map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_script_validates() {
        validate(
            "function on_tick()\n  if pc_rmb_pressed() and pc_lmb_pressed() then\n    input.move_rel(0, 3)\n  end\nend",
        )
        .expect("valid");
    }

    #[test]
    fn syntax_errors_name_the_line() {
        let error = validate("function on_tick()\n  if true then\nend").unwrap_err();
        assert!(error.contains("your script"), "{error}");
        assert!(error.contains("> 3 |") || error.contains("line"), "{error}");
    }

    #[test]
    fn sandbox_removes_load() {
        let error = validate("load('x = 1')()").unwrap_err();
        assert!(error.contains("nil"), "{error}");
    }

    #[test]
    fn logs_go_to_the_console() {
        let lua = build_state("input.log('hello', 3)", "test-profile").expect("runs");
        drop(lua);
        let state = runtime().lock().unwrap();
        assert!(state.logs.iter().any(|entry| entry.message == "hello 3"));
    }

    #[test]
    fn tick_errors_are_reported() {
        let source = "function on_tick() error('boom') end";
        let lua = build_state(source, "t").expect("loads");
        let error = tick(&lua, source).expect("error");
        assert!(error.contains("boom"), "{error}");
    }

    #[test]
    fn foreground_filters_match_shipping_builds() {
        let filters = vec!["game.exe".to_owned()];
        assert!(exe_matches(Some(r"C:\Games\game.exe"), &filters));
        assert!(exe_matches(
            Some(r"C:\Games\Game-Win64-Shipping.exe"),
            &filters
        ));
        assert!(!exe_matches(Some(r"C:\Other\notepad.exe"), &filters));
        assert!(exe_matches(None, &filters));
    }

    #[test]
    fn normalizing_drops_unknown_filters() {
        let mut config = ScriptConfig::default();
        config
            .profile_foreground_exes
            .insert("missing".into(), vec!["a.exe".into()]);
        let id = config.profiles[0].id.clone();
        config
            .profile_foreground_exes
            .insert(id.clone(), vec![" C:/X/Game.EXE ".into()]);
        let config = config.normalized();
        assert_eq!(config.profile_foreground_exes.len(), 1);
        assert_eq!(
            config.profile_foreground_exes[&id],
            vec![r"c:\x\game.exe".to_owned()]
        );
    }
}
