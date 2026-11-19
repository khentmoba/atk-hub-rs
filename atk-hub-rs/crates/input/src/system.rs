//! Windows system settings used by the original `atk-mouse-system-api`,
//! wallpaper apply, lock-key hints and game auto-switch.

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL, VK_NUMLOCK, VK_SCROLL};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, SPI_GETMOUSESPEED, SPI_GETWHEELSCROLLLINES,
    SPI_SETDESKWALLPAPER, SPI_SETMOUSESPEED, SPI_SETWHEELSCROLLLINES, SPIF_SENDCHANGE,
    SPIF_UPDATEINIFILE, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::PWSTR;

/// Error from a Win32 system call.
#[derive(Debug, thiserror::Error)]
#[error("Windows call failed: {0}")]
pub struct SystemError(String);

impl From<windows::core::Error> for SystemError {
    fn from(error: windows::core::Error) -> Self {
        Self(error.message())
    }
}

const PERSIST: SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS =
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(SPIF_UPDATEINIFILE.0 | SPIF_SENDCHANGE.0);

/// Windows pointer speed (1-20, default 10).
///
/// # Errors
///
/// Returns the Win32 error.
pub fn pointer_speed() -> Result<u32, SystemError> {
    let mut value: u32 = 0;
    // SAFETY: SPI_GETMOUSESPEED writes one u32 into the provided pointer.
    unsafe {
        SystemParametersInfoW(
            SPI_GETMOUSESPEED,
            0,
            Some((&raw mut value).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    Ok(value)
}

/// Sets the Windows pointer speed (clamped to 1-20) and persists it.
///
/// # Errors
///
/// Returns the Win32 error.
pub fn set_pointer_speed(speed: u32) -> Result<(), SystemError> {
    let speed = speed.clamp(1, 20) as usize;
    // SAFETY: SPI_SETMOUSESPEED takes the value itself in the pointer argument.
    unsafe {
        SystemParametersInfoW(SPI_SETMOUSESPEED, 0, Some(speed as *mut _), PERSIST)?;
    }
    Ok(())
}

/// Lines scrolled per wheel notch.
///
/// # Errors
///
/// Returns the Win32 error.
pub fn wheel_lines() -> Result<u32, SystemError> {
    let mut value: u32 = 0;
    // SAFETY: SPI_GETWHEELSCROLLLINES writes one u32 into the pointer.
    unsafe {
        SystemParametersInfoW(
            SPI_GETWHEELSCROLLLINES,
            0,
            Some((&raw mut value).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    Ok(value)
}

/// Sets lines scrolled per wheel notch (1-100) and persists it.
///
/// # Errors
///
/// Returns the Win32 error.
pub fn set_wheel_lines(lines: u32) -> Result<(), SystemError> {
    // SAFETY: SPI_SETWHEELSCROLLLINES takes the value in `uiParam`.
    unsafe {
        SystemParametersInfoW(SPI_SETWHEELSCROLLLINES, lines.clamp(1, 100), None, PERSIST)?;
    }
    Ok(())
}

/// Sets the desktop wallpaper to an image file and persists it.
///
/// # Errors
///
/// Returns the Win32 error.
pub fn set_desktop_wallpaper(path: &std::path::Path) -> Result<(), SystemError> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide_vec();
    wide.push(0);
    // SAFETY: SPI_SETDESKWALLPAPER reads a NUL-terminated UTF-16 path that
    // lives for the duration of the call.
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_mut_ptr().cast()),
            PERSIST,
        )?;
    }
    Ok(())
}

trait EncodeWideVec {
    fn encode_wide_vec(&self) -> Vec<u16>;
}

impl EncodeWideVec for std::ffi::OsStr {
    fn encode_wide_vec(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().collect()
    }
}

/// Toggle state of Caps Lock, Num Lock and Scroll Lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct LockKeys {
    /// Caps Lock on.
    pub caps: bool,
    /// Num Lock on.
    pub num: bool,
    /// Scroll Lock on.
    pub scroll: bool,
}

/// Reads the current lock-key toggle states.
#[must_use]
pub fn lock_keys() -> LockKeys {
    // SAFETY: GetKeyState has no preconditions for virtual-key codes.
    let toggled = |vk: i32| unsafe { GetKeyState(vk) } & 1 != 0;
    LockKeys {
        caps: toggled(i32::from(VK_CAPITAL.0)),
        num: toggled(i32::from(VK_NUMLOCK.0)),
        scroll: toggled(i32::from(VK_SCROLL.0)),
    }
}

/// Executable file name of the foreground window's process, e.g. `game.exe`.
#[must_use]
pub fn foreground_process() -> Option<String> {
    // SAFETY: all handles are checked and the process handle is closed.
    unsafe {
        let window: HWND = GetForegroundWindow();
        if window.0.is_null() {
            return None;
        }
        let mut pid = 0_u32;
        GetWindowThreadProcessId(window, Some(&raw mut pid));
        if pid == 0 {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = [0_u16; 1024];
        let mut len = buffer.len() as u32;
        let result = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &raw mut len,
        );
        let _ = CloseHandle(process);
        result.ok()?;
        let path = String::from_utf16_lossy(&buffer[..len as usize]);
        path.rsplit(['/', char::from(0x5c)])
            .next()
            .map(str::to_owned)
    }
}

/// Whether a virtual key or mouse button is held right now.
#[must_use]
pub fn key_down(vk: u8) -> bool {
    // SAFETY: GetAsyncKeyState has no preconditions for virtual-key codes.
    let state =
        unsafe { windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(vk)) };
    state as u16 & 0x8000 != 0
}

/// Cursor position in physical screen pixels.
#[must_use]
pub fn cursor_position() -> Option<(i32, i32)> {
    let mut point = windows::Win32::Foundation::POINT::default();
    // SAFETY: GetCursorPos writes one POINT into the provided pointer.
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&raw mut point) }.ok()?;
    Some((point.x, point.y))
}

/// Today's local date as `YYYY-MM-DD`.
#[must_use]
pub fn local_date() -> String {
    // SAFETY: GetLocalTime has no preconditions and returns a SYSTEMTIME.
    let now = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}", now.wYear, now.wMonth, now.wDay)
}
