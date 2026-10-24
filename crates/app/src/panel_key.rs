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

/// How `key` (`alt+space`) is shown: ⌥Space on macOS, Alt+Space
/// elsewhere.
pub fn label(key: &str) -> String {
    let mac = cfg!(target_os = "macos");
    key.split('+')
        .map(
            |part| match (part.trim().to_ascii_lowercase().as_str(), mac) {
                ("alt" | "option", true) => "⌥".to_string(),
                ("ctrl" | "control", true) => "⌃".to_string(),
                ("shift", true) => "⇧".to_string(),
                ("cmd" | "command" | "super" | "meta", true) => "⌘".to_string(),
                ("alt" | "option", false) => "Alt+".to_string(),
                ("ctrl" | "control", false) => "Ctrl+".to_string(),
                ("shift", false) => "Shift+".to_string(),
                ("cmd" | "command" | "super" | "meta", false) => "Win+".to_string(),
                (key, _) => {
                    let mut chars = key.chars();
                    chars
                        .next()
                        .map(|c| c.to_uppercase().chain(chars).collect())
                        .unwrap_or_default()
                }
            },
        )
        .collect()
}

/// `key` as a shortcut over any app, if it can be one.
fn parse(key: &str) -> Result<HotKey, String> {
    let hotkey: HotKey = key.parse().map_err(|e| format!("{e}"))?;
    if hotkey.mods.difference(Modifiers::SHIFT).is_empty() {
        return Err(tr!("a key alone, or with Shift alone, is for typing"));
    }
    Ok(hotkey)
}

/// Calls `key`, on the system's thread, each time the key goes down
/// (`true`) or up.
pub fn listen(key: impl Fn(bool) + Send + Sync + 'static) {
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        key(e.state == HotKeyState::Pressed);
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
    fn keys_are_shown_as_the_system_does() {
        if cfg!(target_os = "macos") {
            assert_eq!(super::label("alt+space"), "⌥Space");
            assert_eq!(super::label("cmd+shift+space"), "⌘⇧Space");
        } else {
            assert_eq!(super::label("ctrl+alt+space"), "Ctrl+Alt+Space");
            assert_eq!(super::label("alt+shift+k"), "Alt+Shift+K");
        }
    }

    #[test]
    fn a_key_for_typing_is_refused() {
        for key in ["space", "a", "shift+a", "shift+space", "ctrl+nokey"] {
            assert!(parse(key).is_err(), "{key}");
        }
    }
}
