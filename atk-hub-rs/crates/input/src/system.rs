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

fn send_inputs(inputs: &[windows::Win32::UI::Input::KeyboardAndMouse::INPUT]) {
    let size = i32::try_from(std::mem::size_of::<
        windows::Win32::UI::Input::KeyboardAndMouse::INPUT,
    >())
    .unwrap_or(i32::MAX);
    // SAFETY: `inputs` is a valid slice of fully initialized INPUT structs and
    // `size` is the size of one INPUT, as SendInput requires.
    unsafe { windows::Win32::UI::Input::KeyboardAndMouse::SendInput(inputs, size) };
}

fn mouse_input(
    dx: i32,
    dy: i32,
    data: u32,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) -> windows::Win32::UI::Input::KeyboardAndMouse::INPUT {
    use windows::Win32::UI::Input::KeyboardAndMouse::{INPUT, INPUT_0, INPUT_MOUSE, MOUSEINPUT};
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Moves the pointer by a relative amount, like a real mouse would.
pub fn mouse_move_rel(dx: i32, dy: i32) {
    if dx == 0 && dy == 0 {
        return;
    }
    send_inputs(&[mouse_input(
        dx,
        dy,
        0,
        windows::Win32::UI::Input::KeyboardAndMouse::MOUSEEVENTF_MOVE,
    )]);
}

/// Moves the pointer to absolute screen coordinates.
pub fn mouse_move_abs(x: i32, y: i32) {
    // SAFETY: SetCursorPos takes plain coordinates and has no preconditions.
    let _ = unsafe { windows::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y) };
}

/// Presses or releases a mouse button: 1 left, 2 right, 3 middle, 4 and 5
/// the side buttons.
pub fn mouse_button(button: u8, down: bool) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP,
        MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP,
    };
    let (flags, data) = match (button, down) {
        (1, true) => (MOUSEEVENTF_LEFTDOWN, 0),
        (1, false) => (MOUSEEVENTF_LEFTUP, 0),
        (2, true) => (MOUSEEVENTF_RIGHTDOWN, 0),
        (2, false) => (MOUSEEVENTF_RIGHTUP, 0),
        (3, true) => (MOUSEEVENTF_MIDDLEDOWN, 0),
        (3, false) => (MOUSEEVENTF_MIDDLEUP, 0),
        (4 | 5, true) => (MOUSEEVENTF_XDOWN, u32::from(button - 3)),
        (4 | 5, false) => (MOUSEEVENTF_XUP, u32::from(button - 3)),
        _ => return,
    };
    send_inputs(&[mouse_input(0, 0, data, flags)]);
}

/// Types one UTF-16 code unit as a Unicode key press and release.
pub fn send_unicode(unit: u16) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY,
    };
    let key = |up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(0),
                wScan: unit,
                dwFlags: if up {
                    KEYEVENTF_UNICODE | KEYEVENTF_KEYUP
                } else {
                    KEYEVENTF_UNICODE
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    send_inputs(&[key(false), key(true)]);
}

/// Types a whole string through Unicode key events.
pub fn send_text(text: &str) {
    for unit in text.encode_utf16() {
        send_unicode(unit);
    }
}

/// Clipboard change counter; it increases whenever the clipboard changes.
#[must_use]
pub fn clipboard_sequence() -> u32 {
    // SAFETY: GetClipboardSequenceNumber has no preconditions.
    unsafe { windows::Win32::System::DataExchange::GetClipboardSequenceNumber() }
}

/// Reads the clipboard as text.
#[must_use]
pub fn clipboard_text() -> Option<String> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData, OpenClipboard};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    // SAFETY: the clipboard is opened for this thread and closed on every
    // path; the handle is locked only while its UTF-16 text is copied.
    unsafe {
        OpenClipboard(None).ok()?;
        let text = GetClipboardData(CF_UNICODETEXT).ok().and_then(|handle| {
            let global = HGLOBAL(handle.0);
            let ptr = GlobalLock(global).cast::<u16>();
            if ptr.is_null() {
                return None;
            }
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let text = String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len));
            let _ = GlobalUnlock(global);
            Some(text)
        });
        let _ = CloseClipboard();
        text
    }
}

/// Replaces the clipboard with text.
pub fn set_clipboard_text(text: &str) {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    const CF_UNICODETEXT: u32 = 13;
    let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: the buffer is allocated with room for every unit, filled while
    // locked, and handed to the clipboard, which then owns it.
    unsafe {
        if OpenClipboard(None).is_err() {
            return;
        }
        let _ = EmptyClipboard();
        if let Ok(global) = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2) {
            let ptr = GlobalLock(global).cast::<u16>();
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
                let _ = GlobalUnlock(global);
                let _ = SetClipboardData(CF_UNICODETEXT, Some(HANDLE(global.0)));
            }
        }
        let _ = CloseClipboard();
    }
}

/// Presses Ctrl+C in the focused window.
pub fn send_copy() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };
    let key = |vk: u16, up: bool| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: if up {
                    KEYEVENTF_KEYUP
                } else {
                    KEYBD_EVENT_FLAGS(0)
                },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    send_inputs(&[
        key(0x11, false),
        key(0x43, false),
        key(0x43, true),
        key(0x11, true),
    ]);
}

/// Opens a web address in the default browser.
pub fn open_url(url: &str) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    use windows::core::{HSTRING, w};
    let target = HSTRING::from(url);
    // SAFETY: both strings are valid null-terminated UTF-16 for the call.
    unsafe {
        ShellExecuteW(None, w!("open"), &target, None, None, SW_SHOWNORMAL);
    }
}

/// Finds the desktop layer that sits behind the icons and parents `hwnd` to
/// it at the given virtual-screen rectangle (like `electron-as-wallpaper`).
///
/// Returns `false` when no desktop host window was found.
pub fn attach_to_desktop(hwnd: isize, x: i32, y: i32, width: i32, height: i32) -> bool {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, FindWindowW, GWL_EXSTYLE, GWL_STYLE, GetSystemMetrics,
        GetWindowLongPtrW, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SMTO_NORMAL, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SendMessageTimeoutW, SetParent, SetWindowLongPtrW,
        SetWindowPos, WS_CHILD, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
    };
    use windows::core::{BOOL, w};

    unsafe extern "system" fn find_worker(top: HWND, out: LPARAM) -> BOOL {
        // SAFETY: `out` points at the `HWND` owned by `attach_to_desktop`.
        unsafe {
            if FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), None).is_ok()
                && let Ok(worker) = FindWindowExW(None, Some(top), w!("WorkerW"), None)
            {
                *(out.0 as *mut HWND) = worker;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    let window = HWND(hwnd as *mut core::ffi::c_void);
    // SAFETY: plain Win32 calls on window handles; failures are reported by
    // return values and every pointer passed lives for the call.
    unsafe {
        let Ok(progman) = FindWindowW(w!("Progman"), None) else {
            return false;
        };
        // Ask Explorer to split a WorkerW behind the desktop icons.
        for wparam in [0x0D, 0] {
            let _ = SendMessageTimeoutW(
                progman,
                0x052C,
                WPARAM(wparam),
                LPARAM(i32::from(wparam != 0) as isize),
                SMTO_NORMAL,
                1000,
                None,
            );
        }
        let mut worker = HWND::default();
        let _ = EnumWindows(Some(find_worker), LPARAM(&raw mut worker as isize));

        let style = GetWindowLongPtrW(window, GWL_STYLE);
        SetWindowLongPtrW(
            window,
            GWL_STYLE,
            (style & !(WS_POPUP.0 as isize)) | WS_CHILD.0 as isize,
        );
        let ex = GetWindowLongPtrW(window, GWL_EXSTYLE);
        SetWindowLongPtrW(
            window,
            GWL_EXSTYLE,
            (ex & !(WS_EX_APPWINDOW.0 as isize))
                | (WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0) as isize,
        );

        let origin = (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
        );
        let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
        if !worker.is_invalid() {
            if SetParent(window, Some(worker)).is_err() {
                return false;
            }
            let _ = SetWindowPos(
                window,
                None,
                x - origin.0,
                y - origin.1,
                width,
                height,
                flags,
            );
            return true;
        }
        // Windows 11 24H2+: the WorkerW lives inside Progman. Slot our window
        // between the icon view and that WorkerW.
        let Ok(defview) = FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), None) else {
            return false;
        };
        if SetParent(window, Some(progman)).is_err() {
            return false;
        }
        let _ = SetWindowPos(
            window,
            Some(defview),
            x - origin.0,
            y - origin.1,
            width,
            height,
            flags,
        );
        if let Ok(inner) = FindWindowExW(Some(progman), None, w!("WorkerW"), None) {
            let _ = SetWindowPos(
                inner,
                Some(window),
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            );
        }
        true
    }
}

/// Re-applies the current static desktop wallpaper so the desktop repaints
/// after a live wallpaper window is removed.
pub fn refresh_desktop_wallpaper() {
    use windows::Win32::UI::WindowsAndMessaging::SPI_GETDESKWALLPAPER;
    let mut buffer = [0u16; 520];
    // SAFETY: the buffer is writable for its full length, which is passed in.
    unsafe {
        if SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buffer.len() as u32,
            Some(buffer.as_mut_ptr().cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
        {
            let _ = SystemParametersInfoW(
                SPI_SETDESKWALLPAPER,
                0,
                Some(buffer.as_mut_ptr().cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            );
        }
    }
}
