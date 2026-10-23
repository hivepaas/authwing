//! The quick panel's shortcut, registered with the system so it works over
//! any app (`[panel] key` in config.toml), and registered again when it
//! changes.

use authwing_i18n::tr;
use global_hotkey::hotkey::{HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};

pub struct PanelKey {
    manager: GlobalHotKeyManager,
    /// The key registered now, as written and as parsed.
    now: Option<(String, HotKey)>,
}

impl gpui::Global for PanelKey {}

impl PanelKey {
    /// The system's registry of shortcuts; on the main thread, as macOS
    /// wants it.
    pub fn new() -> Result<PanelKey, String> {
        let manager = GlobalHotKeyManager::new().map_err(|e| e.to_string())?;
        Ok(PanelKey { manager, now: None })
    }

    /// Registers `key` (`alt+space`) in place of the one before; an empty one
    /// registers none. A key alone or with Shift alone is refused, as it is
    /// for typing; another app holding it already is said.
    pub fn set(&mut self, key: &str) -> Result<(), String> {
        let key = key.trim();
        if self.now.as_ref().map(|(k, _)| k.as_str()) == Some(key) {
            return Ok(());
        }
        if let Some((_, old)) = self.now.take() {
            let _ = self.manager.unregister(old);
        }
        if key.is_empty() {
            return Ok(());
        }
        let hotkey = parse(key)?;
        self.manager.register(hotkey).map_err(|e| e.to_string())?;
        self.now = Some((key.to_string(), hotkey));
        Ok(())
    }
}

/// `key` as a shortcut over any app, if it can be one.
fn parse(key: &str) -> Result<HotKey, String> {
    let hotkey: HotKey = key.parse().map_err(|e| format!("{e}"))?;
    if hotkey.mods.difference(Modifiers::SHIFT).is_empty() {
        return Err(tr!("a key alone, or with Shift alone, is for typing"));
    }
    Ok(hotkey)
}

/// Calls `pressed`, on the system's thread, each time the key goes down.
pub fn listen(pressed: impl Fn() + Send + Sync + 'static) {
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state == HotKeyState::Pressed {
            pressed();
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn the_choices_in_settings_read() {
        for key in [
            "alt+space",
            "ctrl+alt+space",
            "cmd+shift+space",
            "alt+shift+space",
            "ctrl+shift+space",
            authwing_config::PANEL_KEY,
        ] {
            assert!(parse(key).is_ok(), "{key}");
        }
    }

    #[test]
    fn a_key_for_typing_is_refused() {
        for key in ["space", "a", "shift+a", "shift+space", "ctrl+nokey"] {
            assert!(parse(key).is_err(), "{key}");
        }
    }
}
