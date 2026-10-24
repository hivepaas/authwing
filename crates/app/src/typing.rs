//! Typing a code into the app in front, as keys: the system's own events, so
//! any field takes it, a web page's or a VPN client's alike. On macOS only
//! with the Accessibility permission; elsewhere the events are just sent.

use enigo::{Enigo, Keyboard, Settings};

/// Types `text` into whatever has the keyboard now.
pub fn type_text(text: &str) -> Result<(), String> {
    let mut keys = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    keys.text(text).map_err(|e| e.to_string())
}

/// Whether the system lets authwing send keys to other apps.
pub fn allowed() -> bool {
    authwing_context::window::title_access() == authwing_context::window::Access::Granted
}
