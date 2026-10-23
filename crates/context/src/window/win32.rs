//! Windows: the foreground window from user32, its process' exe from
//! kernel32.
//!
//! These are C functions the `windows` crate marks unsafe, each called from
//! a function of its own that is allowed `unsafe` and says why the call is
//! sound. A UWP app's windows belong to `ApplicationFrameHost.exe`, which
//! is what they are reported as.

use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
    SetForegroundWindow,
};
use windows::core::PWSTR;

use super::Window;

pub(super) fn active() -> Option<Window> {
    let (handle, pid, title) = foreground()?;
    let exe = exe(pid).unwrap_or_default();
    let exe = Path::new(&exe);
    let name = |part: Option<&std::ffi::OsStr>| part.map(|p| p.to_string_lossy().into_owned());
    Some(Window {
        app_id: name(exe.file_name()).unwrap_or_default(),
        app_name: name(exe.file_stem()).unwrap_or_default(),
        title: (!title.is_empty()).then_some(title),
        pid: (pid != 0).then_some(pid),
        handle: u64::try_from(handle.0.addr()).ok(),
    })
}

/// Makes the window `handle` the foreground window again. Windows lets the
/// process that has the foreground do it.
#[allow(unsafe_code)]
pub(super) fn bring_back(handle: u64) -> bool {
    let Ok(handle) = usize::try_from(handle) else {
        return false;
    };
    let window = HWND(std::ptr::without_provenance_mut(handle));
    // SAFETY: user32 only looks the handle up, and fails on one that is no
    // window (any more).
    unsafe { SetForegroundWindow(window) }.as_bool()
}

/// The foreground window, its process id and its title.
#[allow(unsafe_code)]
fn foreground() -> Option<(HWND, u32, String)> {
    // SAFETY: the handle is only given back to user32, whose calls fail
    // cleanly on a window that has closed since; the text is written into
    // a buffer whose length is passed with it. GetWindowText reads the
    // title another process' window keeps, without waiting on it.
    unsafe {
        let window = GetForegroundWindow();
        if window.is_invalid() {
            return None;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(window, Some(&mut pid));
        let length = usize::try_from(GetWindowTextLengthW(window)).unwrap_or(0);
        let mut text = vec![0u16; length + 1];
        let written = usize::try_from(GetWindowTextW(window, &mut text)).unwrap_or(0);
        text.truncate(written.min(length));
        Some((window, pid, String::from_utf16_lossy(&text)))
    }
}

/// The full path of process `pid`'s exe, if it lets us see it.
#[allow(unsafe_code)]
fn exe(pid: u32) -> Option<String> {
    let mut path = vec![0u16; 32_768];
    let mut length = u32::try_from(path.len()).unwrap_or(u32::MAX);
    // SAFETY: the handle is closed before it goes; the path is written into
    // a buffer whose length is passed with it, and its length written back.
    let named = unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let named = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(path.as_mut_ptr()),
            &mut length,
        );
        let _ = CloseHandle(process);
        named
    };
    named.ok()?;
    path.truncate(usize::try_from(length).unwrap_or(0));
    Some(String::from_utf16_lossy(&path))
}
