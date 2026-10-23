//! The QR codes on the screen: every display captured, then read.
//!
//! - macOS: each display through CoreGraphics (`xcap`). Capturing needs the
//!   user to allow authwing in Privacy & Security, Screen Recording
//!   ([`access`], [`ask_access`]). Without it the system does not refuse: it
//!   gives the desktop with every window left out, so [`screen`] checks
//!   first and says [`Error::NotAllowed`] rather than find nothing.
//! - Windows: each display through GDI (`xcap`). Nothing to allow.
//! - Linux under Wayland: the desktop portal's screenshot, one image of
//!   every display. Wayland gives an app no other way to see other apps'
//!   windows. The portal asks the user itself the first time, and says when
//!   they decline; [`access`] cannot know before. It leaves the screenshot
//!   in a file, which is read and removed: it shows the code's secret.
//! - Linux under X11: each RandR monitor of the root window, from the X
//!   server.
//!
//! `xcap` covers Linux too, but links PipeWire and libgbm there, C libraries
//! for its screen recording, which this does not need: the portal and the X
//! server are reached here in pure Rust, as GPUI and `authwing-context`
//! already do.

use image::RgbaImage;

use crate::{Error, decode};

#[cfg(target_os = "linux")]
mod linux;

/// A QR code's text on the screen, and the display it is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub text: String,
    /// The display's place in the system's list of them, from 0: on macOS
    /// the main display first. Under Wayland the portal gives every display
    /// as one image, which is 0.
    pub display: usize,
}

/// Whether authwing may capture the screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Granted,
    NotGranted,
    /// A system this cannot capture the screen of.
    Unknown,
}

/// Whether authwing may capture the screen. Only macOS asks the user before
/// (the Screen Recording permission); Windows has nothing to allow, and
/// under Wayland the portal asks at the capture itself, so both are
/// [`Access::Granted`].
pub fn access() -> Access {
    #[cfg(target_os = "macos")]
    {
        return granted(objc2_core_graphics::CGPreflightScreenCaptureAccess());
    }
    #[cfg(any(windows, target_os = "linux"))]
    {
        return Access::Granted;
    }
    #[allow(unreachable_code)]
    Access::Unknown
}

/// Has macOS ask the user to allow screen capture: the first time, its
/// dialog leads to System Settings, where they turn authwing on; after,
/// it asks nothing. Says whether it is already allowed. macOS applies the
/// user's answer when authwing next starts, so until then [`access`] stays
/// [`Access::NotGranted`]. Elsewhere it asks nothing.
pub fn ask_access() -> Access {
    #[cfg(target_os = "macos")]
    {
        return granted(objc2_core_graphics::CGRequestScreenCaptureAccess());
    }
    #[allow(unreachable_code)]
    access()
}

#[cfg(target_os = "macos")]
fn granted(granted: bool) -> Access {
    if granted {
        Access::Granted
    } else {
        Access::NotGranted
    }
}

/// The text of every QR code on every display, each once, with the first
/// display it is on. Takes a moment: each display is read on a thread of
/// its own, and the call waits for them all.
pub fn screen() -> Result<Vec<Found>, Error> {
    if access() == Access::NotGranted {
        return Err(Error::NotAllowed);
    }
    let displays = displays()?;
    let texts: Vec<Vec<String>> = std::thread::scope(|s| {
        let readers: Vec<_> = displays
            .iter()
            .map(|image| s.spawn(|| decode(image)))
            .collect();
        readers
            .into_iter()
            .map(|reader| {
                reader
                    .join()
                    .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            })
            .collect()
    });
    let mut found: Vec<Found> = Vec::new();
    for (display, texts) in texts.into_iter().enumerate() {
        for text in texts {
            if !found.iter().any(|seen| seen.text == text) {
                found.push(Found { text, display });
            }
        }
    }
    Ok(found)
}

/// An image of each display, in the system's order.
#[cfg(any(target_os = "macos", windows))]
fn displays() -> Result<Vec<RgbaImage>, Error> {
    let failed = |error: xcap::XCapError| Error::Capture(error.to_string());
    xcap::Monitor::all()
        .map_err(failed)?
        .iter()
        .map(|monitor| monitor.capture_image().map_err(failed))
        .collect()
}

#[cfg(target_os = "linux")]
fn displays() -> Result<Vec<RgbaImage>, Error> {
    linux::displays()
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn displays() -> Result<Vec<RgbaImage>, Error> {
    Err(Error::Capture("not on this system".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whatever this machine allows, the answer must hold together, and
    /// asking again changes nothing. Neither call shows a dialog.
    #[test]
    fn access_answers() {
        let now = access();
        assert_eq!(access(), now);
        if cfg!(any(windows, target_os = "linux")) {
            assert_eq!(now, Access::Granted);
        }
    }

    /// Only where capture needs no dialog: it must not ask the user while
    /// tests run.
    #[test]
    fn screen_when_not_allowed() {
        if access() == Access::NotGranted {
            assert!(matches!(screen(), Err(Error::NotAllowed)));
        }
    }
}
