use serde::Deserialize;

/// `config.toml`. Every key is optional; unknown keys are ignored so a file
/// written by a newer version still loads.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub general: General,
    pub sync: Sync,
    pub appearance: Appearance,
    pub codes: Codes,
    pub updates: Updates,
    /// `[shortcuts]`: keys for the window's shortcuts, by name, over the
    /// defaults. This device's own: keys differ between systems.
    pub shortcuts: crate::ShortcutSettings,
}

/// Telling when a newer authwing is out.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Updates {
    /// Look once a day (reading the signed list of releases); off: only when
    /// asked to, in Settings › About.
    pub check: bool,
}

impl Default for Updates {
    fn default() -> Self {
        Updates { check: true }
    }
}

/// Codes on this device: what happens to one that is used.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Codes {
    /// Seconds a copied code stays on the clipboard before it is cleared
    /// (if the clipboard still holds it); 0: it stays.
    pub clear_clipboard: u32,
}

impl Default for Codes {
    fn default() -> Self {
        Codes {
            clear_clipboard: 20,
        }
    }
}

/// A folder as written in the settings, `~/` expanded. None when empty.
fn expand_home(f: &str) -> Option<std::path::PathBuf> {
    let f = f.trim();
    if f.is_empty() {
        return None;
    }
    Some(match (f.strip_prefix("~/"), std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => std::path::PathBuf::from(f),
    })
}

/// Syncing the vault through a folder (docs/design/sync.md).
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct Sync {
    /// The folder, `~/` allowed; None: no sync.
    pub folder: Option<String>,
    /// A WebDAV server instead (Nextcloud, Synology, Koofr). Its password is
    /// in the vault, this device's own.
    pub webdav: Option<WebDavSync>,
    /// S3-compatible storage instead (AWS, R2, B2, MinIO, Wasabi). Its secret
    /// key is in the vault, this device's own.
    pub s3: Option<S3Sync>,
    /// A Git repository instead, through the system's git. A token, if one
    /// is used, is in the vault, this device's own.
    pub git: Option<GitSync>,
    /// The settings that are not this device's own sync too
    /// ([`crate::settings_sync::SYNCED`]).
    pub settings: bool,
}

impl Default for Sync {
    fn default() -> Self {
        Sync {
            folder: None,
            webdav: None,
            s3: None,
            git: None,
            settings: true,
        }
    }
}

impl Sync {
    /// The folder with `~/` expanded.
    pub fn folder_path(&self) -> Option<std::path::PathBuf> {
        expand_home(self.folder.as_deref()?)
    }

    /// Where the vault syncs: one place, the folder first should several be
    /// set by hand.
    pub fn place(&self) -> Option<SyncPlace> {
        if let Some(f) = self.folder_path() {
            return Some(SyncPlace::Folder(f));
        }
        if let Some(w) = &self.webdav {
            return Some(SyncPlace::WebDav(w.clone()));
        }
        if let Some(s3) = &self.s3 {
            return Some(SyncPlace::S3(s3.clone()));
        }
        self.git.clone().map(SyncPlace::Git)
    }
}

/// `[sync] webdav`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct WebDavSync {
    /// The folder's address: `https://cloud.example.com/remote.php/dav/files/me/`.
    pub url: String,
    pub user: String,
}

/// `[sync] s3`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct S3Sync {
    /// `https://s3.<region>.amazonaws.com`, `https://<account>.r2.cloudflarestorage.com`…
    pub endpoint: String,
    /// Empty: `auto`, as R2 takes.
    pub region: String,
    pub bucket: String,
    /// A folder in the bucket; empty: its top.
    pub prefix: String,
    pub access_key: String,
}

/// `[sync] git`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct GitSync {
    /// `https://github.com/you/vault.git` or `git@github.com:you/vault.git`.
    pub url: String,
    /// Empty: the repository's default.
    pub branch: String,
    /// Who a token signs in as, where the host needs it (Gitea); empty:
    /// any, as GitHub and GitLab take.
    pub user: String,
}

/// Where the vault syncs, as `[sync]` says; secrets are not in it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncPlace {
    Folder(std::path::PathBuf),
    WebDav(WebDavSync),
    S3(S3Sync),
    Git(GitSync),
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct General {
    /// The app's language, a tag (`en`, `vi`); None or `auto`: the
    /// system's. This device's own: it does not sync.
    pub language: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Appearance {
    /// Dark or light, or as the system is.
    pub mode: ColorMode,
    /// The app's text.
    pub ui_font: UiFont,
}

/// The app's font.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(default)]
pub struct UiFont {
    /// None: the system's interface font.
    pub family: Option<String>,
    /// The size of body text; the rest scales with it.
    pub size: f32,
}

/// The UI font size by default.
pub const UI_FONT_SIZE: f32 = 14.0;

impl Default for UiFont {
    fn default() -> Self {
        UiFont {
            family: None,
            size: UI_FONT_SIZE,
        }
    }
}

/// Whether the app is dark or light.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorMode {
    /// As the system is, and changing when it does.
    #[default]
    System,
    Dark,
    Light,
}

impl Settings {
    /// Brings numbers that would break the layout back into a usable range.
    pub(crate) fn sanitized(mut self) -> Self {
        let a = &mut self.appearance;
        // Past 20 the fixed-height rows of the app run out of room.
        a.ui_font.size = a.ui_font.size.clamp(10.0, 20.0);
        if a.ui_font
            .family
            .as_deref()
            .is_some_and(|f| f.trim().is_empty())
        {
            a.ui_font.family = None;
        }
        // An hour at most: a code is good for 30 seconds.
        self.codes.clear_clipboard = self.codes.clear_clipboard.min(3600);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_file_is_defaults() {
        let s: Settings = toml::from_str("").expect("parse");
        assert_eq!(s, Settings::default());
        assert_eq!(s.codes.clear_clipboard, 20);
        assert_eq!(s.appearance.ui_font.size, UI_FONT_SIZE);
    }

    #[test]
    fn reads_every_key_and_clamps() {
        let s: Settings = toml::from_str(
            r#"
            # A comment.
            [appearance]
            mode = "light"
            ui_font = { family = "Inter", size = 40 }
            something_new = true

            [codes]
            clear_clipboard = 90000

            [general]
            language = "vi"
            "#,
        )
        .expect("parse");
        let s = s.sanitized();
        assert_eq!(s.appearance.mode, ColorMode::Light);
        assert_eq!(s.appearance.ui_font.family.as_deref(), Some("Inter"));
        assert_eq!(s.appearance.ui_font.size, 20.0);
        assert_eq!(s.codes.clear_clipboard, 3600);
        assert_eq!(s.general.language.as_deref(), Some("vi"));
    }
}
