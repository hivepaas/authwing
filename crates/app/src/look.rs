//! The settings in `config.toml`, loaded again when the file changes, and the
//! sizes the views are written in.

use std::sync::Arc;

use authwing_config::{ColorMode, Keymap, LoadError, Os, Settings, Stamp, Store, SyncPlace};

/// A text size as drawn at the default UI font size; it grows and shrinks with
/// the chosen one (the window's rem follows it, see the workspace).
pub fn ui(px: f32) -> gpui::Rems {
    gpui::rems(px / 16.)
}

/// The UI font size the views' sizes are written for (body text 13 px, the
/// rem 16 px, as GPUI's).
const DRAWN_AT: f32 = 13.;

/// The window's rem for a UI font size.
pub fn rem_for(ui_size: f32) -> gpui::Pixels {
    gpui::px(16. * ui_size / DRAWN_AT)
}

pub struct Live {
    store: Option<Store>,
    stamp: Stamp,
    settings: Settings,
    /// The window's shortcuts, as `[shortcuts]` sets them.
    keymap: Arc<Keymap>,
}

impl Live {
    /// Loads the config directory; `None` (no home directory) means defaults only.
    pub fn open(store: Option<Store>) -> (Live, Vec<String>) {
        let mut live = Live {
            stamp: Stamp::default(),
            store,
            settings: Settings::default(),
            keymap: Arc::new(Keymap::defaults(Os::this())),
        };
        let problems = live.load();
        (live, problems)
    }

    pub fn store(&self) -> Option<&Store> {
        self.store.as_ref()
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Where the vault syncs, if anywhere.
    pub fn sync_place(&self) -> Option<SyncPlace> {
        self.settings.sync.place()
    }

    /// Whether the app is light now: its mode, or the system's when it
    /// follows the system (`system_light`).
    pub fn light_for(&self, system_light: bool) -> bool {
        match self.settings.appearance.mode {
            ColorMode::System => system_light,
            ColorMode::Dark => false,
            ColorMode::Light => true,
        }
    }

    pub fn keymap(&self) -> Arc<Keymap> {
        self.keymap.clone()
    }

    fn load(&mut self) -> Vec<String> {
        let Some(store) = &self.store else {
            return vec![];
        };
        self.stamp = store.stamp();
        let mut problems: Vec<LoadError> = vec![];
        match store.settings() {
            Ok(s) => self.settings = s,
            Err(e) => problems.push(e),
        }
        let (keymap, wrong) = Keymap::new(&self.settings.shortcuts, Os::this());
        self.keymap = Arc::new(keymap);
        let mut problems: Vec<String> = problems.iter().map(ToString::to_string).collect();
        problems.extend(wrong.iter().map(crate::hotkeys::problem));
        problems
    }

    /// Reloads if a file changed since the last load: `Some(problems)`.
    pub fn reload_if_changed(&mut self) -> Option<Vec<String>> {
        let store = self.store.as_ref()?;
        (store.stamp() != self.stamp).then(|| self.load())
    }
}
