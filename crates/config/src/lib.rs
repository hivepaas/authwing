//! authwing's configuration: the TOML files in the config directory.
//!
//! - `config.toml`: [`Settings`], edited by people and by the app.
//! - `state/*.json`: what the app remembers, such as the [`WindowState`]; never
//!   edited by people and never synced.
//!
//! The codes themselves are not here: they are records in the encrypted vault.
//! A missing file means defaults. A file that fails to parse is reported as a
//! [`LoadError`] with its line, and the caller keeps what it had before. The
//! app writes `config.toml` back through `toml_edit`, so comments and order
//! survive (see [`Store::set_setting`]). Layout and rationale:
//! `docs/design/config.md`.

use std::collections::BTreeMap;

mod color;
mod settings;
pub mod settings_sync;
pub mod shortcuts;
mod state;
mod store;

pub use color::Color;
pub use settings::{
    Appearance, Codes, ColorMode, General, GitSync, PANEL_KEY, Panel, S3Sync, Settings, Sync,
    SyncPlace, UI_FONT_SIZE, UiFont, Updates, WebDavSync,
};
pub use shortcuts::{Chord, Hotkey, Keymap, Mods, Os, ShortcutSettings};
pub use state::{UpdateState, WindowMode, WindowState};
pub use store::{EditFile, LoadError, SettingValue, Stamp, Store, SyncPlaceText, WriteError};

/// A vault record's fields, by name: what the synced settings travel as.
pub type Fields = BTreeMap<String, String>;
