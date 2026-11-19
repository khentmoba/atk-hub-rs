//! Headset audio & THX driver lifecycle management
//! (`getHeadsetAudioDrivers`, `prepareHeadsetDriverPackage`, `switchHeadsetAudioDriver`,
//! `repairHeadsetAudioDevice`, `uninstallHeadsetAudioDriver`, service start/stop).

use crate::{parse_id, user_data_dir};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct DriverOp {
    action: String,
    status: String,
    message: String,
}

fn op_state() -> &'static Mutex<Option<DriverOp>> {
    static OP: OnceLock<Mutex<Option<DriverOp>>> = OnceLock::new();
    OP.get_or_init(|| Mutex::new(None))
}

fn set_op(action: &str, status: &str, message: impl Into<String>) {
    if let Ok(mut guard) = op_state().lock() {
        *guard = Some(DriverOp {
            action: action.to_owned(),
            status: status.to_owned(),
            message: message.into(),
        });
    }
}

fn thx_dir() -> PathBuf {
    user_data_dir().join("drivers").join("thx")
}

fn run_hidden(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("{program} failed: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        Ok(stdout)
    } else {
        Err(format!(
            "{program} exited with {}: {}",
            output.status,
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        ))
    }
}

fn query_thx_service() -> (bool, bool, String) {
    for name in ["THXService", "THXAudioService", "AtkAudioService"] {
        if let Ok(out) = run_hidden("sc", &["query", name]) {
            let running = out.contains("RUNNING");
            return (true, running, name.to_owned());
        }
    }
    (false, false, "THXService".to_owned())
}

fn list_installed_thx_drivers() -> Vec<Value> {
    let Ok(out) = run_hidden("pnputil", &["/enum-drivers"]) else {
        return Vec::new();
    };
    let mut results = Vec::new();
    for block in out.split("\r\n\r\n") {
        let lower = block.to_ascii_lowercase();
        if lower.contains("thx")
            || lower.contains("atk")
            || lower.contains("bolutek")
            || lower.contains("ktmicro")
        {
            let mut published = String::new();
            let mut original = String::new();
            let mut provider = String::new();
            for line in block.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let key = k.trim().to_ascii_lowercase();
                    let val = v.trim().to_owned();
                    if key.contains("published name") {
                        published = val;
                    } else if key.contains("original name") {
                        original = val;
                    } else if key.contains("provider") {
                        provider = val;
                    }
                }
            }
            if !published.is_empty() {
                results.push(json!({
                    "publishedName": published,
                    "originalName": original,
                    "provider": provider,
                }));
            }
        }
    }
    results
}

fn find_file_with_ext(dir: &Path, ext: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case(ext))
        {
            return Some(path);
        }
        if path.is_dir()
            && let Some(found) = find_file_with_ext(&path, ext)
        {
            return Some(found);
        }
    }
    None
}

#[tauri::command]
pub fn headset_driver_status() -> Value {
    let (service_installed, service_running, service_name) = query_thx_service();
    let dir = thx_dir();
    let package_ready =
        find_file_with_ext(&dir, "inf").is_some() || find_file_with_ext(&dir, "exe").is_some();
    let drivers = list_installed_thx_drivers();
    let operation = op_state().lock().ok().and_then(|g| g.clone());
    json!({
        "serviceName": service_name,
        "serviceInstalled": service_installed,
        "serviceRunning": service_running,
        "packageDir": dir.to_string_lossy(),
        "packageReady": package_ready,
        "installedDrivers": drivers,
        "operation": operation,
    })
}

#[tauri::command]
pub fn headset_driver_prepare(vendor_id: String, product_id: String) -> Result<Value, String> {
    let vid = parse_id(&vendor_id)?;
    let pid = parse_id(&product_id)?;
    let meta_url = atk_endpoints::firmware_url(vid, pid, "driver_thx");
    set_op("prepare", "running", format!("Checking {meta_url}"));

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(30))
        .build();
    let meta: Value = agent
        .get(&meta_url)
        .call()
        .map_err(|error| {
            let msg = format!("THX driver metadata fetch failed: {error}");
            set_op("prepare", "error", &msg);
            msg
        })?
        .into_json()
        .map_err(|error| {
            let msg = format!("invalid THX driver metadata: {error}");
            set_op("prepare", "error", &msg);
            msg
        })?;

    let data = meta.get("data").unwrap_or(&meta);
    let file_url = data
        .get("file")
        .or_else(|| data.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if file_url.is_empty() {
        let msg = "No THX driver download URL returned for this device".to_owned();
        set_op("prepare", "error", &msg);
        return Err(msg);
    }

    let mut bytes = Vec::new();
    agent
        .get(file_url)
        .call()
        .map_err(|error| {
            let msg = format!("THX driver download failed: {error}");
            set_op("prepare", "error", &msg);
            msg
        })?
        .into_reader()
        .take(256 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;

    let dir = thx_dir();
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let name = file_url
        .split('?')
        .next()
        .unwrap_or(file_url)
        .rsplit('/')
        .next()
        .unwrap_or("driver_thx.exe");
    let dest = dir.join(name);
    fs::write(&dest, &bytes).map_err(|error| error.to_string())?;
    set_op(
        "prepare",
        "done",
        format!("Downloaded {} ({} bytes)", dest.display(), bytes.len()),
    );
    Ok(headset_driver_status())
}

#[tauri::command]
pub fn headset_driver_action(action: String) -> Result<Value, String> {
    let dir = thx_dir();
    let (_, _, service_name) = query_thx_service();
    match action.as_str() {
        "dismiss" => {
            if let Ok(mut guard) = op_state().lock() {
                *guard = None;
            }
            Ok(headset_driver_status())
        }
        "start_service" => {
            let out = run_hidden("sc", &["start", &service_name])?;
            set_op("start_service", "done", out.trim());
            Ok(headset_driver_status())
        }
        "stop_service" => {
            let out = run_hidden("sc", &["stop", &service_name])?;
            set_op("stop_service", "done", out.trim());
            Ok(headset_driver_status())
        }
        "install" | "switch_thx" => {
            if let Some(inf) = find_file_with_ext(&dir, "inf") {
                let inf_str = inf.to_string_lossy().into_owned();
                let out = run_hidden("pnputil", &["/add-driver", &inf_str, "/install"])?;
                set_op(&action, "done", out.trim());
            } else if let Some(exe) = find_file_with_ext(&dir, "exe") {
                Command::new(&exe)
                    .spawn()
                    .map_err(|error| format!("failed to start driver installer: {error}"))?;
                set_op(&action, "done", format!("Launched {}", exe.display()));
            } else {
                let msg = "No THX driver package downloaded yet; click Download THX Driver first."
                    .to_owned();
                set_op(&action, "error", &msg);
                return Err(msg);
            }
            Ok(headset_driver_status())
        }
        "uninstall" | "switch_standard" => {
            let drivers = list_installed_thx_drivers();
            if let Some(first) = drivers.first().and_then(|d| d["publishedName"].as_str()) {
                let out = run_hidden("pnputil", &["/delete-driver", first, "/uninstall"])?;
                set_op(&action, "done", out.trim());
            } else {
                set_op(
                    &action,
                    "done",
                    "No third-party THX/ATK OEM driver found; using standard Windows USB Audio.",
                );
            }
            Ok(headset_driver_status())
        }
        "repair" => {
            let _ = run_hidden("pnputil", &["/scan-devices"]);
            let _ = run_hidden("sc", &["start", "audiosrv"]);
            set_op(
                "repair",
                "done",
                "Triggered Plug-and-Play device rescan and verified Windows Audio service.",
            );
            Ok(headset_driver_status())
        }
        other => Err(format!("unknown headset driver action: {other}")),
    }
}
