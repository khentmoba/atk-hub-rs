//! Installed Windows application scanner and native executable picker
//! (`atk-open-target:applications`, `atk-open-target:pick`, `gameEngine_selectExecutable`).

use crate::system::SystemError;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_SZ, RegCloseKey, RegEnumKeyExW,
    RegOpenKeyExW, RegQueryValueExW,
};
use windows::Win32::UI::Controls::Dialogs::{
    GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW,
};
use windows::core::{PCWSTR, PWSTR};

/// An installed Windows application discovered for Game Mode or open-app bindings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    /// Human-readable application name.
    pub name: String,
    /// Lowercase executable file name (e.g. `cs2.exe`).
    pub exe: String,
    /// Full path to the executable when resolved.
    pub path: String,
    /// Publisher name when available.
    pub publisher: String,
}

fn wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn read_reg_string(key: HKEY, name: &str) -> Option<String> {
    let wname = wide_null(name);
    let mut kind = REG_SZ;
    let mut size: u32 = 0;
    // SAFETY: First query determines required byte size.
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wname.as_ptr()),
            None,
            Some(&raw mut kind),
            None,
            Some(&raw mut size),
        )
    };
    if status.is_err() || size < 2 {
        return None;
    }
    let mut buf = vec![0u16; (size as usize).div_ceil(2)];
    // SAFETY: Buffer is allocated for at least `size` bytes.
    let status = unsafe {
        RegQueryValueExW(
            key,
            PCWSTR(wname.as_ptr()),
            None,
            Some(&raw mut kind),
            Some(buf.as_mut_ptr().cast()),
            Some(&raw mut size),
        )
    };
    if status.is_err() {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let value = String::from_utf16_lossy(&buf[..len]).trim().to_owned();
    if value.is_empty() { None } else { Some(value) }
}

/// Extracts an `.exe` path from a registry `DisplayIcon` or `InstallLocation`.
fn extract_exe(display_icon: Option<&str>, install_location: Option<&str>) -> Option<PathBuf> {
    if let Some(raw) = display_icon {
        let trimmed = raw.trim().trim_matches('"');
        let before_comma = trimmed
            .split(',')
            .next()
            .unwrap_or(trimmed)
            .trim()
            .trim_matches('"');
        let candidate = PathBuf::from(before_comma);
        if candidate
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            && candidate.exists()
        {
            return Some(candidate);
        }
    }
    if let Some(dir) = install_location {
        let dir_path = Path::new(dir.trim().trim_matches('"'));
        if dir_path.is_dir()
            && let Ok(entries) = std::fs::read_dir(dir_path)
        {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
                {
                    let name = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_ascii_lowercase();
                    if !name.contains("unins") && !name.contains("setup") && !name.contains("crash")
                    {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}

fn scan_uninstall_root(root: HKEY, subkey: &str, out: &mut BTreeMap<String, InstalledApp>) {
    let wsub = wide_null(subkey);
    let mut handle = HKEY::default();
    // SAFETY: Opens read-only registry key; closed before returning.
    if unsafe {
        RegOpenKeyExW(
            root,
            PCWSTR(wsub.as_ptr()),
            Some(0),
            KEY_READ,
            &raw mut handle,
        )
    }
    .is_err()
    {
        return;
    }

    let mut index: u32 = 0;
    let mut name_buf = [0u16; 256];
    loop {
        let mut name_len = name_buf.len() as u32;
        // SAFETY: Enumerates child key names into `name_buf`.
        let status = unsafe {
            RegEnumKeyExW(
                handle,
                index,
                Some(PWSTR(name_buf.as_mut_ptr())),
                &raw mut name_len,
                None,
                None,
                None,
                None,
            )
        };
        if status.is_err() {
            break;
        }
        index += 1;

        let child_name = String::from_utf16_lossy(&name_buf[..name_len as usize]);
        let wchild = wide_null(&child_name);
        let mut child_key = HKEY::default();
        // SAFETY: Opens child key read-only and closes it immediately after reading.
        if unsafe {
            RegOpenKeyExW(
                handle,
                PCWSTR(wchild.as_ptr()),
                Some(0),
                KEY_READ,
                &raw mut child_key,
            )
        }
        .is_ok()
        {
            let display_name = read_reg_string(child_key, "DisplayName");
            let display_icon = read_reg_string(child_key, "DisplayIcon");
            let install_location = read_reg_string(child_key, "InstallLocation");
            let publisher = read_reg_string(child_key, "Publisher").unwrap_or_default();
            // SAFETY: Closes opened child handle.
            unsafe {
                let _ = RegCloseKey(child_key);
            }

            if let (Some(name), Some(exe_path)) = (
                display_name,
                extract_exe(display_icon.as_deref(), install_location.as_deref()),
            ) {
                let exe = exe_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_ascii_lowercase();
                if !exe.is_empty() && !exe.contains("unins") {
                    out.entry(exe.clone()).or_insert(InstalledApp {
                        name,
                        exe,
                        path: exe_path.to_string_lossy().into_owned(),
                        publisher,
                    });
                }
            }
        }
    }

    // SAFETY: Closes parent handle.
    unsafe {
        let _ = RegCloseKey(handle);
    }
}

/// Lists installed Windows desktop applications with resolved `.exe` paths.
#[must_use]
pub fn list_installed_apps() -> Vec<InstalledApp> {
    let mut apps = BTreeMap::new();
    for (root, path) in [
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_CURRENT_USER,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ] {
        scan_uninstall_root(root, path, &mut apps);
    }
    let mut list: Vec<InstalledApp> = apps.into_values().collect();
    list.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
    });
    list
}

/// Opens a native Windows file picker for `.exe` files.
///
/// # Errors
///
/// Returns an error only if the selected path is invalid.
pub fn pick_executable() -> Result<Option<InstalledApp>, SystemError> {
    let filter: Vec<u16> = "Executables (*.exe)\0*.exe\0All Files (*.*)\0*.*\0\0"
        .encode_utf16()
        .collect();
    let title = wide_null("Select Game or Application Executable");
    let mut file_buf = [0u16; 1024];

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file_buf.as_mut_ptr()),
        nMaxFile: file_buf.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
        ..Default::default()
    };

    // SAFETY: All pointers in `OPENFILENAMEW` live for the duration of `GetOpenFileNameW`.
    let picked = unsafe { GetOpenFileNameW(&raw mut ofn).as_bool() };
    if !picked {
        return Ok(None);
    }
    let len = file_buf
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(file_buf.len());
    let path_str = String::from_utf16_lossy(&file_buf[..len]);
    let path = Path::new(&path_str);
    let exe = path
        .file_name()
        .ok_or_else(|| SystemError::new("selected path has no file name"))?
        .to_string_lossy()
        .to_ascii_lowercase();
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    Ok(Some(InstalledApp {
        name,
        exe,
        path: path_str,
        publisher: String::new(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_apps_scan_does_not_panic() {
        let apps = list_installed_apps();
        assert!(apps.iter().all(|a| !a.exe.is_empty()));
    }
}
