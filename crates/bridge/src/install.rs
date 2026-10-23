//! Telling the browsers where the host is: a host manifest, for each browser
//! the user has, naming the authwing program and the extensions that may
//! start it.
//!
//! Each browser looks in its own places, for the user (not the machine, so
//! no administrator is needed):
//!
//! - **macOS**: a folder `NativeMessagingHosts` in its folder in
//!   `~/Library/Application Support`; Firefox's is `Mozilla`'s, not
//!   `Firefox`'s. Brave reads Chrome's folder, not its own. Arc is said both
//!   to read its own (`Arc/User Data`) and Chrome's, so both are written.
//! - **Linux**: the same in `~/.config` (or `$XDG_CONFIG_HOME`); Firefox's is
//!   `~/.mozilla/native-messaging-hosts`, still so when its profiles are in
//!   `~/.config/mozilla` (Firefox 147 and later). Browsers in Flatpak or Snap
//!   see none of these, and are left out.
//! - **Windows**: a key under `HKEY_CURRENT_USER` whose default value is the
//!   manifest's path, the manifests being in authwing's folder. Edge reads
//!   its own key, then Chromium's and Chrome's; Brave and Vivaldi read
//!   Chromium's, then Chrome's. Arc for Windows is left out: where it looks
//!   is not known.
//!
//! <https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging#native-messaging-host-location>
//! <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_manifests#manifest_location>
//! <https://learn.microsoft.com/en-us/microsoft-edge/extensions/developer-guide/native-messaging>
//!
//! Where each place is comes from [`Dirs`] and an [`Os`], not from the
//! running system, so it can be tested on any.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::protocol::HOST_NAME;

/// The extension's id in the Chrome Web Store, as `allowed_origins` takes
/// it. A placeholder until the extension is published: the store gives the
/// id on the first upload.
pub const CHROME_WEB_STORE_ORIGIN: &str = "chrome-extension://aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/";

/// The extension's id in Microsoft Edge Add-ons, which gives one of its own.
/// A placeholder, as [`CHROME_WEB_STORE_ORIGIN`].
pub const EDGE_ADDONS_ORIGIN: &str = "chrome-extension://bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/";

/// The id of the extension loaded unpacked from `extension/dist/chromium`,
/// fixed by the public key in its manifest. Its private key was not kept,
/// and anyone may load an extension with that key, so only debug builds of
/// authwing let it in.
pub const DEVELOPMENT_ORIGIN: &str = "chrome-extension://jabndagnjdpnmkblgpdjonlpjllpocbl/";

/// The extension's id in Firefox (its manifest's
/// `browser_specific_settings.gecko.id`), for good once it is on
/// addons.mozilla.org. Firefox lets a temporary add-on take any id, so this
/// does not stop one loaded by hand from about:debugging.
pub const FIREFOX_EXTENSION_ID: &str = "authwing@hivepaas.com";

const DESCRIPTION: &str = "authwing: two-factor codes for the page in front";

/// The systems there are places for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    MacOs,
    Linux,
    Windows,
}

impl Os {
    /// The system this runs on; Linux's places for the other Unixes.
    pub const CURRENT: Os = if cfg!(target_os = "macos") {
        Os::MacOs
    } else if cfg!(windows) {
        Os::Windows
    } else {
        Os::Linux
    };
}

/// The browsers authwing writes a manifest for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Browser {
    Chrome,
    Chromium,
    Edge,
    Brave,
    Vivaldi,
    Arc,
    Firefox,
}

impl Browser {
    pub const ALL: [Browser; 7] = [
        Browser::Chrome,
        Browser::Chromium,
        Browser::Edge,
        Browser::Brave,
        Browser::Vivaldi,
        Browser::Arc,
        Browser::Firefox,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Browser::Chrome => "Google Chrome",
            Browser::Chromium => "Chromium",
            Browser::Edge => "Microsoft Edge",
            Browser::Brave => "Brave",
            Browser::Vivaldi => "Vivaldi",
            Browser::Arc => "Arc",
            Browser::Firefox => "Firefox",
        }
    }

    fn family(self) -> Family {
        match self {
            Browser::Firefox => Family::Firefox,
            _ => Family::Chromium,
        }
    }
}

impl fmt::Display for Browser {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Browsers whose manifests are alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Chromium,
    Firefox,
}

/// The user's folders the places are in.
#[derive(Clone, Debug)]
pub struct Dirs {
    /// The home folder.
    pub home: PathBuf,
    /// Linux's `$XDG_CONFIG_HOME`, or `~/.config`.
    pub config: PathBuf,
    /// Windows' `%LOCALAPPDATA%`.
    pub local_app_data: PathBuf,
    /// Windows' `%APPDATA%`.
    pub app_data: PathBuf,
}

impl Dirs {
    /// This user's.
    pub fn for_user() -> io::Result<Dirs> {
        let home = dirs::home_dir().ok_or_else(|| io::Error::other("no home folder"))?;
        Ok(Dirs {
            config: dirs::config_dir().unwrap_or_else(|| home.join(".config")),
            local_app_data: dirs::data_local_dir().unwrap_or_else(|| home.clone()),
            app_data: dirs::data_dir().unwrap_or_else(|| home.clone()),
            home,
        })
    }

    /// macOS's `~/Library/Application Support`.
    fn app_support(&self) -> PathBuf {
        self.home.join("Library").join("Application Support")
    }
}

/// Where a browser looks for a host manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    /// `dev.authwing.bridge.json` in this folder.
    Folder(PathBuf),
    /// A key under `HKEY_CURRENT_USER` whose default value is the path of
    /// `manifest`.
    Registry { key: String, manifest: PathBuf },
}

impl Place {
    /// The manifest's file.
    pub fn manifest(&self) -> PathBuf {
        match self {
            Place::Folder(folder) => folder.join(manifest_file_name()),
            Place::Registry { manifest, .. } => manifest.clone(),
        }
    }
}

impl fmt::Display for Place {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Place::Folder(folder) => write!(f, "{}", folder.join(manifest_file_name()).display()),
            Place::Registry { key, .. } => write!(f, r"HKEY_CURRENT_USER\{key}"),
        }
    }
}

fn manifest_file_name() -> String {
    format!("{HOST_NAME}.json")
}

/// Whether `browser` is there: whether it has run, leaving the file that
/// lists its profiles (`Local State`, Firefox's `profiles.ini`). Not its
/// folder, which installing for another browser may have made (Brave's
/// manifest goes in Chrome's).
pub fn is_installed(os: Os, dirs: &Dirs, browser: Browser) -> bool {
    let marker = match browser.family() {
        Family::Chromium => "Local State",
        Family::Firefox => "profiles.ini",
    };
    profile_folders(os, dirs, browser)
        .iter()
        .any(|f| f.join(marker).is_file())
}

/// Where `browser` keeps its profiles.
fn profile_folders(os: Os, dirs: &Dirs, browser: Browser) -> Vec<PathBuf> {
    match os {
        Os::MacOs => {
            let support = dirs.app_support();
            vec![support.join(match browser {
                Browser::Chrome => "Google/Chrome",
                Browser::Chromium => "Chromium",
                Browser::Edge => "Microsoft Edge",
                Browser::Brave => "BraveSoftware/Brave-Browser",
                Browser::Vivaldi => "Vivaldi",
                Browser::Arc => "Arc/User Data",
                Browser::Firefox => "Firefox",
            })]
        }
        Os::Linux => match browser {
            Browser::Arc => vec![],
            Browser::Firefox => vec![
                dirs.home.join(".mozilla").join("firefox"),
                dirs.config.join("mozilla").join("firefox"),
            ],
            _ => vec![dirs.config.join(match browser {
                Browser::Chromium => "chromium",
                Browser::Edge => "microsoft-edge",
                Browser::Brave => "BraveSoftware/Brave-Browser",
                Browser::Vivaldi => "vivaldi",
                _ => "google-chrome",
            })],
        },
        Os::Windows => match browser {
            Browser::Arc => vec![],
            Browser::Firefox => vec![dirs.app_data.join("Mozilla").join("Firefox")],
            _ => {
                let vendor: &[&str] = match browser {
                    Browser::Chromium => &["Chromium"],
                    Browser::Edge => &["Microsoft", "Edge"],
                    Browser::Brave => &["BraveSoftware", "Brave-Browser"],
                    Browser::Vivaldi => &["Vivaldi"],
                    _ => &["Google", "Chrome"],
                };
                let folder = vendor
                    .iter()
                    .fold(dirs.local_app_data.clone(), |f, part| f.join(part));
                vec![folder.join("User Data")]
            }
        },
    }
}

/// Where authwing writes `browser`'s manifest; none where it does not.
pub fn places(os: Os, dirs: &Dirs, browser: Browser) -> Vec<Place> {
    let hosts = "NativeMessagingHosts";
    match os {
        Os::MacOs => {
            let support = dirs.app_support();
            let chrome = Place::Folder(support.join("Google/Chrome").join(hosts));
            match browser {
                Browser::Brave => vec![chrome],
                Browser::Arc => vec![
                    Place::Folder(support.join("Arc/User Data").join(hosts)),
                    chrome,
                ],
                Browser::Firefox => vec![Place::Folder(support.join("Mozilla").join(hosts))],
                _ => profile_folders(os, dirs, browser)
                    .into_iter()
                    .map(|f| Place::Folder(f.join(hosts)))
                    .collect(),
            }
        }
        Os::Linux => match browser {
            Browser::Firefox => vec![Place::Folder(
                dirs.home.join(".mozilla").join("native-messaging-hosts"),
            )],
            _ => profile_folders(os, dirs, browser)
                .into_iter()
                .map(|f| Place::Folder(f.join(hosts)))
                .collect(),
        },
        Os::Windows => {
            let (vendor, family) = match browser {
                Browser::Arc => return vec![],
                Browser::Chrome | Browser::Brave | Browser::Vivaldi => {
                    (r"Google\Chrome", "chromium")
                }
                Browser::Chromium => ("Chromium", "chromium"),
                Browser::Edge => (r"Microsoft\Edge", "chromium"),
                Browser::Firefox => ("Mozilla", "firefox"),
            };
            vec![Place::Registry {
                key: format!(r"Software\{vendor}\{hosts}\{HOST_NAME}"),
                manifest: dirs
                    .local_app_data
                    .join("authwing")
                    .join(hosts)
                    .join(family)
                    .join(manifest_file_name()),
            }]
        }
    }
}

/// A host manifest, as both families take it: Chromium's has
/// `allowed_origins`, Firefox's `allowed_extensions`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_origins: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_extensions: Option<Vec<String>>,
}

impl Manifest {
    /// `browser`'s manifest for the host at `program`, which must be an
    /// absolute path (macOS and Linux take no other).
    pub fn new(browser: Browser, program: &Path) -> Manifest {
        let (allowed_origins, allowed_extensions) = match browser.family() {
            Family::Chromium => {
                let mut origins = vec![CHROME_WEB_STORE_ORIGIN, EDGE_ADDONS_ORIGIN];
                if cfg!(debug_assertions) {
                    origins.push(DEVELOPMENT_ORIGIN);
                }
                (Some(origins.into_iter().map(String::from).collect()), None)
            }
            Family::Firefox => (None, Some(vec![FIREFOX_EXTENSION_ID.into()])),
        };
        Manifest {
            name: HOST_NAME.into(),
            description: DESCRIPTION.into(),
            path: program.into(),
            kind: "stdio".into(),
            allowed_origins,
            allowed_extensions,
        }
    }

    pub fn to_json(&self) -> String {
        let mut json = serde_json::to_string_pretty(self).unwrap_or_default();
        json.push('\n');
        json
    }
}

/// What [`status`] found in one place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Found {
    /// A manifest for this program.
    Here,
    /// A manifest for another program: another copy of authwing.
    Elsewhere(PathBuf),
    /// No manifest, or one that cannot be read.
    Missing,
}

/// One browser's place, and what is there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub browser: Browser,
    pub installed: bool,
    pub place: Place,
    pub found: Found,
}

/// Writes the manifest for `program`, an absolute path, in each place of
/// each browser the user has; the places written.
pub fn install(os: Os, dirs: &Dirs, program: &Path) -> io::Result<Vec<(Browser, Place)>> {
    if !program.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} is not an absolute path", program.display()),
        ));
    }
    let mut written = Vec::new();
    for browser in Browser::ALL {
        if !is_installed(os, dirs, browser) {
            continue;
        }
        for place in places(os, dirs, browser) {
            let manifest = place.manifest();
            if let Some(folder) = manifest.parent() {
                std::fs::create_dir_all(folder)?;
            }
            std::fs::write(&manifest, Manifest::new(browser, program).to_json())?;
            if let Place::Registry { key, .. } = &place {
                registry::set(key, &manifest)?;
            }
            written.push((browser, place));
        }
    }
    Ok(written)
}

/// Removes the manifests from every place of every browser, there or not;
/// the places they were in.
pub fn uninstall(os: Os, dirs: &Dirs) -> io::Result<Vec<(Browser, Place)>> {
    let mut removed = Vec::new();
    for browser in Browser::ALL {
        for place in places(os, dirs, browser) {
            let mut was = false;
            if let Place::Registry { key, .. } = &place {
                was |= registry::remove(key)?;
            }
            match std::fs::remove_file(place.manifest()) {
                Ok(()) => was = true,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e),
            }
            if was {
                removed.push((browser, place));
            }
        }
    }
    Ok(removed)
}

/// What is in each place of each browser, and whether it names `program`.
pub fn status(os: Os, dirs: &Dirs, program: &Path) -> Vec<Entry> {
    let mut entries = Vec::new();
    for browser in Browser::ALL {
        let installed = is_installed(os, dirs, browser);
        for place in places(os, dirs, browser) {
            let found = read(&place).map_or(Found::Missing, |manifest| {
                if same_file(&manifest.path, program) {
                    Found::Here
                } else {
                    Found::Elsewhere(manifest.path)
                }
            });
            entries.push(Entry {
                browser,
                installed,
                place,
                found,
            });
        }
    }
    entries
}

fn read(place: &Place) -> Option<Manifest> {
    let file = match place {
        Place::Folder(_) => place.manifest(),
        Place::Registry { key, .. } => registry::get(key)?,
    };
    let manifest: Manifest = serde_json::from_slice(&std::fs::read(file).ok()?).ok()?;
    (manifest.name == HOST_NAME).then_some(manifest)
}

fn same_file(a: &Path, b: &Path) -> bool {
    a == b || matches!((a.canonicalize(), b.canonicalize()), (Ok(a), Ok(b)) if a == b)
}

#[cfg(windows)]
mod registry {
    use std::io;
    use std::path::{Path, PathBuf};

    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    pub fn set(key: &str, manifest: &Path) -> io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(key)?;
        key.set_value("", &manifest.as_os_str())
    }

    /// Whether it was there.
    pub fn remove(key: &str) -> io::Result<bool> {
        match RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(key) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub fn get(key: &str) -> Option<PathBuf> {
        let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(key).ok()?;
        key.get_value::<std::ffi::OsString, _>("")
            .ok()
            .map(PathBuf::from)
    }
}

/// Elsewhere there is no registry: Windows' places are only planned, as in
/// the tests.
#[cfg(not(windows))]
mod registry {
    use std::io;
    use std::path::{Path, PathBuf};

    pub fn set(_key: &str, _manifest: &Path) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no registry here",
        ))
    }

    pub fn remove(_key: &str) -> io::Result<bool> {
        Ok(false)
    }

    pub fn get(_key: &str) -> Option<PathBuf> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_home() -> (tempfile::TempDir, Dirs) {
        let root = tempfile::tempdir().expect("temporary folder");
        let home = root.path().join("home");
        let dirs = Dirs {
            config: home.join(".config"),
            local_app_data: home.join("AppData").join("Local"),
            app_data: home.join("AppData").join("Roaming"),
            home,
        };
        (root, dirs)
    }

    fn folder(place: &Place) -> &Path {
        match place {
            Place::Folder(folder) => folder,
            Place::Registry { .. } => panic!("not a folder: {place}"),
        }
    }

    /// Makes the file `relative` in the fake home, as a browser that has run
    /// has it.
    fn make(dirs: &Dirs, relative: &str) {
        let file = dirs.home.join(relative);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("made");
        std::fs::write(file, "").expect("written");
    }

    #[test]
    fn chromium_manifest() {
        let manifest = Manifest::new(
            Browser::Edge,
            Path::new("/Applications/authwing.app/Contents/MacOS/authwing"),
        );
        let json: serde_json::Value = serde_json::from_str(&manifest.to_json()).expect("JSON");
        assert_eq!(json["name"], "dev.authwing.bridge");
        assert_eq!(json["type"], "stdio");
        assert_eq!(
            json["path"],
            "/Applications/authwing.app/Contents/MacOS/authwing"
        );
        let origins = json["allowed_origins"].as_array().expect("origins");
        assert!(origins.contains(&CHROME_WEB_STORE_ORIGIN.into()));
        assert!(origins.contains(&EDGE_ADDONS_ORIGIN.into()));
        assert_eq!(
            origins.contains(&DEVELOPMENT_ORIGIN.into()),
            cfg!(debug_assertions)
        );
        for origin in origins {
            let origin = origin.as_str().expect("a string");
            let id = origin
                .strip_prefix("chrome-extension://")
                .and_then(|o| o.strip_suffix('/'))
                .expect("chrome-extension://ID/");
            assert!(
                id.len() == 32 && id.bytes().all(|b| (b'a'..=b'p').contains(&b)),
                "{origin}"
            );
        }
        assert!(json.get("allowed_extensions").is_none());
    }

    #[test]
    fn firefox_manifest() {
        let manifest = Manifest::new(Browser::Firefox, Path::new("/usr/bin/authwing"));
        let json: serde_json::Value = serde_json::from_str(&manifest.to_json()).expect("JSON");
        assert_eq!(
            json["allowed_extensions"],
            serde_json::json!(["authwing@hivepaas.com"])
        );
        assert!(json.get("allowed_origins").is_none());
    }

    #[test]
    fn macos_places() {
        let (_root, dirs) = fake_home();
        let support = dirs.home.join("Library/Application Support");
        let at = |b| {
            places(Os::MacOs, &dirs, b)
                .iter()
                .map(|p| folder(p).to_path_buf())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            at(Browser::Chrome),
            [support.join("Google/Chrome/NativeMessagingHosts")]
        );
        assert_eq!(
            at(Browser::Brave),
            [support.join("Google/Chrome/NativeMessagingHosts")]
        );
        assert_eq!(
            at(Browser::Edge),
            [support.join("Microsoft Edge/NativeMessagingHosts")]
        );
        assert_eq!(
            at(Browser::Arc),
            [
                support.join("Arc/User Data/NativeMessagingHosts"),
                support.join("Google/Chrome/NativeMessagingHosts")
            ]
        );
        assert_eq!(
            at(Browser::Firefox),
            [support.join("Mozilla/NativeMessagingHosts")]
        );
    }

    #[test]
    fn linux_places() {
        let (_root, dirs) = fake_home();
        let at = |b| {
            places(Os::Linux, &dirs, b)
                .iter()
                .map(|p| folder(p).to_path_buf())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            at(Browser::Chrome),
            [dirs.config.join("google-chrome/NativeMessagingHosts")]
        );
        assert_eq!(
            at(Browser::Brave),
            [dirs
                .config
                .join("BraveSoftware/Brave-Browser/NativeMessagingHosts")]
        );
        assert_eq!(
            at(Browser::Firefox),
            [dirs.home.join(".mozilla/native-messaging-hosts")]
        );
        assert!(at(Browser::Arc).is_empty());
    }

    #[test]
    fn windows_places() {
        let (_root, dirs) = fake_home();
        let place = |b| places(Os::Windows, &dirs, b).remove(0);
        let shared = dirs
            .local_app_data
            .join("authwing")
            .join("NativeMessagingHosts");
        assert_eq!(
            place(Browser::Edge),
            Place::Registry {
                key: r"Software\Microsoft\Edge\NativeMessagingHosts\dev.authwing.bridge".into(),
                manifest: shared.join("chromium").join("dev.authwing.bridge.json"),
            }
        );
        assert_eq!(
            place(Browser::Brave),
            Place::Registry {
                key: r"Software\Google\Chrome\NativeMessagingHosts\dev.authwing.bridge".into(),
                manifest: shared.join("chromium").join("dev.authwing.bridge.json"),
            }
        );
        assert_eq!(
            place(Browser::Firefox),
            Place::Registry {
                key: r"Software\Mozilla\NativeMessagingHosts\dev.authwing.bridge".into(),
                manifest: shared.join("firefox").join("dev.authwing.bridge.json"),
            }
        );
        assert!(places(Os::Windows, &dirs, Browser::Arc).is_empty());
    }

    #[test]
    fn installed_browsers_are_found_by_what_they_leave() {
        let (_root, dirs) = fake_home();
        assert!(!is_installed(Os::MacOs, &dirs, Browser::Chrome));
        // A folder alone, as installing for Brave makes Chrome's, is not Chrome.
        make(
            &dirs,
            "Library/Application Support/Google/Chrome/NativeMessagingHosts/dev.authwing.bridge.json",
        );
        assert!(!is_installed(Os::MacOs, &dirs, Browser::Chrome));
        make(
            &dirs,
            "Library/Application Support/Google/Chrome/Local State",
        );
        make(&dirs, "Library/Application Support/Firefox/profiles.ini");
        assert!(is_installed(Os::MacOs, &dirs, Browser::Chrome));
        assert!(is_installed(Os::MacOs, &dirs, Browser::Firefox));
        assert!(!is_installed(Os::MacOs, &dirs, Browser::Brave));

        make(&dirs, ".config/mozilla/firefox/profiles.ini");
        assert!(is_installed(Os::Linux, &dirs, Browser::Firefox));
        make(
            &dirs,
            "AppData/Local/BraveSoftware/Brave-Browser/User Data/Local State",
        );
        assert!(is_installed(Os::Windows, &dirs, Browser::Brave));
    }

    /// Installs for the browsers there, reads them back, and removes them,
    /// in a home of its own, with macOS's places: folders, where Windows'
    /// are registry keys, so the test runs on every Unix.
    #[cfg(unix)]
    #[test]
    fn install_status_uninstall() {
        let (_root, dirs) = fake_home();
        let os = Os::MacOs;
        make(
            &dirs,
            "Library/Application Support/Google/Chrome/Local State",
        );
        make(
            &dirs,
            "Library/Application Support/BraveSoftware/Brave-Browser/Local State",
        );
        make(&dirs, "Library/Application Support/Firefox/profiles.ini");
        let program = dirs.home.join("authwing");
        std::fs::write(&program, "").expect("written");
        assert!(install(os, &dirs, Path::new("authwing")).is_err());

        let written = install(os, &dirs, &program).expect("installed");
        let browsers: Vec<_> = written.iter().map(|(b, _)| *b).collect();
        assert_eq!(
            browsers,
            [Browser::Chrome, Browser::Brave, Browser::Firefox]
        );

        let chrome = dirs.home.join("Library/Application Support/Google/Chrome/NativeMessagingHosts/dev.authwing.bridge.json");
        let manifest: Manifest =
            serde_json::from_slice(&std::fs::read(&chrome).expect("there")).expect("JSON");
        assert_eq!(manifest.path, program);

        let found = |entries: &[Entry], browser| {
            entries
                .iter()
                .find(|e| e.browser == browser)
                .map(|e| e.found.clone())
        };
        let entries = status(os, &dirs, &program);
        assert_eq!(found(&entries, Browser::Chrome), Some(Found::Here));
        assert_eq!(found(&entries, Browser::Firefox), Some(Found::Here));
        assert_eq!(found(&entries, Browser::Edge), Some(Found::Missing));

        let other = status(os, &dirs, Path::new("/elsewhere/authwing"));
        assert_eq!(
            found(&other, Browser::Chrome),
            Some(Found::Elsewhere(program.clone()))
        );

        let removed = uninstall(os, &dirs).expect("removed");
        assert!(removed.iter().any(|(b, _)| *b == Browser::Firefox));
        assert!(!chrome.exists());
        let entries = status(os, &dirs, &program);
        assert!(entries.iter().all(|e| e.found == Found::Missing));
    }
}
