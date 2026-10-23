//! The window the user is in: its app, and its title where the system
//! tells it.
//!
//! Polled, not watched: the app calls [`active`] every half second or so,
//! off the UI thread, and compares with the last answer. To be told instead
//! takes another mechanism on each system (NSWorkspace's notifications plus
//! an accessibility observer per app for its titles on macOS,
//! `SetWinEventHook` on Windows, `PropertyNotify` on X11's root window),
//! each with a thread or run loop to keep, for news still in time half a
//! second late. A call is a handful of system calls.
//!
//! - macOS: the frontmost app from NSWorkspace, which needs no permission.
//!   The focused window's title from the Accessibility API, which needs the
//!   user to allow authwing in Privacy & Security, Accessibility
//!   ([`title_access`], [`ask_title_access`]); until then the title is None.
//!   The frontmost app is kept up to date by the main run loop, which the
//!   app runs: in a process that runs none it stays what it was at start.
//! - Windows: the foreground window, its title and its process' exe.
//! - Linux on X11: the window manager's `_NET_ACTIVE_WINDOW`, its
//!   `WM_CLASS` and `_NET_WM_NAME`. Under Wayland there is none: compositors
//!   do not tell an app about other apps' windows, by design, and they
//!   share no protocol that would.
//!
//! Authwing's own windows are reported like any other: the app keeps the
//! last window that is not its own ([`Window::pid`] against
//! [`std::process::id`]).

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod win32;
#[cfg(target_os = "linux")]
mod x11;

/// A window, by its app.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Window {
    /// What the app is known by to the system: its bundle id on macOS, its
    /// exe's file name on Windows, its `WM_CLASS` on X11. Empty when the
    /// system would not say.
    pub app_id: String,
    /// The app's name to show: the localised name on macOS, the exe's name
    /// without `.exe` on Windows, the `WM_CLASS` on X11.
    pub app_name: String,
    /// The window's title, if it has one and it can be read.
    pub title: Option<String>,
    pub pid: Option<u32>,
}

/// Whether other apps' window titles can be read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Granted,
    NotGranted,
}

/// The window the user is in, if the system says.
pub fn active() -> Option<Window> {
    #[cfg(target_os = "macos")]
    {
        return macos::active();
    }
    #[cfg(windows)]
    {
        return win32::active();
    }
    #[cfg(target_os = "linux")]
    {
        return x11::active();
    }
    #[allow(unreachable_code)]
    None
}

/// Whether authwing may read other apps' window titles. Only macOS asks
/// the user (the Accessibility permission); elsewhere there is nothing to
/// grant.
pub fn title_access() -> Access {
    #[cfg(target_os = "macos")]
    {
        return access(macos::trusted());
    }
    #[allow(unreachable_code)]
    Access::Granted
}

/// Has macOS ask the user to allow reading titles: its dialog leads to
/// System Settings, where they turn authwing on. Says whether it is already
/// allowed; the user's answer comes later, so the app polls
/// [`title_access`] after. Elsewhere it asks nothing.
pub fn ask_title_access() -> Access {
    #[cfg(target_os = "macos")]
    {
        return access(macos::ask());
    }
    #[allow(unreachable_code)]
    Access::Granted
}

#[cfg(target_os = "macos")]
fn access(granted: bool) -> Access {
    if granted {
        Access::Granted
    } else {
        Access::NotGranted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever the machine running the tests has in front: the call must
    /// not fail, and what it says must hold together.
    #[test]
    fn active_answers() {
        if let Some(window) = active() {
            assert!(
                !window.app_id.is_empty() || !window.app_name.is_empty() || window.pid.is_some()
            );
            assert_ne!(window.title.as_deref(), Some(""));
        }
        let _ = title_access();
    }
}
