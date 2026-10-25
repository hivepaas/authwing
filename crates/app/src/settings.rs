//! Settings: a page over the window, its sections on the left. What lives in
//! config.toml is written there at once (its comments kept) and the window
//! takes it in; what is the vault's (the passphrase, the keychain, Touch ID,
//! a sync place's password) goes to the vault.

use std::path::PathBuf;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent,
    PathPromptOptions, PromptLevel, Render, SharedString, Window, div, prelude::*, px, rgb,
};

use authwing_backup::{Format, Imported, Item};

use authwing_bridge::install::{self, Dirs, Found, Os};
use authwing_config::{
    ColorMode, GitSync, S3Sync, SettingValue, Settings, Store, SyncPlaceText, WebDavSync,
};
use authwing_i18n::{tr, tr_n};
use authwing_vault::{AskState, Presence as _};

mod about;

use crate::colors::colors;
use crate::input::{TextInput, said};
use crate::join::{JoinEvent, JoinView};
use crate::look::ui;
use crate::notice::Notice;
use crate::sync_panel::SyncSnap;
use crate::syncing::{PLACE_SECRET, SyncWorker};
use crate::vault_slot::VaultSlot;
use crate::widgets::{
    choice, input_box, labeled, outline_button, primary_button, switch, text_button,
};
use crate::workspace::UpdateSnap;

pub enum SettingsEvent {
    Close,
    /// config.toml changed: the window takes it in now rather than at its
    /// next look.
    Changed,
    Say(Notice),
    Lock,
    /// Codes came in from a backup or another app's export.
    CodesChanged,
    /// Show the welcome again.
    Welcome,
    /// Settings › About: look for a newer release now.
    CheckUpdates,
    /// Download the release offered, or say what updates this copy.
    GetUpdate,
    /// Not offered again until a newer one comes.
    SkipUpdate,
    /// Quit, install the release downloaded, and start it.
    RestartToUpdate,
    /// Settings › Sync's status card: sync now, or try again.
    SyncNow,
    /// This device joined the vault its place holds.
    Joined,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    General,
    Voice,
    Sync,
    Security,
    Backup,
    Browsers,
    About,
}

impl Page {
    const ALL: [Page; 7] = [
        Page::General,
        Page::Voice,
        Page::Sync,
        Page::Security,
        Page::Backup,
        Page::Browsers,
        Page::About,
    ];

    fn title(self) -> String {
        match self {
            Page::General => tr!("General"),
            Page::Voice => tr!("Voice"),
            Page::Sync => tr!("Sync"),
            Page::Security => tr!("Security"),
            Page::Backup => tr!("Backup"),
            Page::Browsers => tr!("Browsers"),
            Page::About => tr!("About"),
        }
    }
}

/// Where the vault syncs, as the form has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Folder,
    Git,
    WebDav,
    S3,
}

/// The fields of the place's form.
struct PlaceForm {
    kind: Kind,
    folder: Entity<TextInput>,
    git_url: Entity<TextInput>,
    git_branch: Entity<TextInput>,
    dav_url: Entity<TextInput>,
    dav_user: Entity<TextInput>,
    s3_endpoint: Entity<TextInput>,
    s3_region: Entity<TextInput>,
    s3_bucket: Entity<TextInput>,
    s3_prefix: Entity<TextInput>,
    s3_access: Entity<TextInput>,
    /// The WebDAV password or the S3 secret key: kept in the vault.
    secret: Entity<TextInput>,
    /// A token for git, where it cannot sign in by itself, and its user:
    /// the token kept in the vault, as the secret is.
    git_token: Entity<TextInput>,
    git_user: Entity<TextInput>,
    /// What is missing, after Connect.
    problem: Option<String>,
    /// Why git cannot be used here, after Connect, with how to get it.
    no_git: Option<authwing_remote::NoGit>,
    /// Installing git now.
    installing: bool,
}

pub struct SettingsView {
    page: Page,
    store: Option<Store>,
    settings: Settings,
    vault: VaultSlot,
    sync: SyncWorker,
    /// How sync stands, as the window last said.
    sync_snap: SyncSnap,
    /// Joining the vault the place holds, while this device does not sync
    /// there.
    join: Entity<JoinView>,
    /// How looking for updates stands, as the window last said.
    update: Option<UpdateSnap>,
    place: PlaceForm,
    pass: Entity<TextInput>,
    pass_again: Entity<TextInput>,
    shown_key: Option<String>,
    /// Something runs off the window's thread (a key derived, the system
    /// asked); the buttons wait.
    busy: bool,
    problem: Option<String>,
    browsers: Vec<install::Entry>,
    backup_pass: Entity<TextInput>,
    backup_pass_again: Entity<TextInput>,
    /// A file to import that waits for its password.
    pending: Option<Pending>,
    import_pass: Entity<TextInput>,
    focus: FocusHandle,
}

/// A file chosen to import, encrypted: what it is, and its bytes.
struct Pending {
    name: String,
    format: Format,
    bytes: Vec<u8>,
}

impl EventEmitter<SettingsEvent> for SettingsView {}

impl Focusable for SettingsView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn set_text(field: &Entity<TextInput>, text: &str, cx: &mut Context<SettingsView>) {
    field.update(cx, |f, cx| f.set_text(text, text.len()..text.len(), cx));
}

/// iCloud Drive's folder on this Mac, if it has one.
fn icloud_drive() -> Option<PathBuf> {
    let dir = std::env::home_dir()?
        .join("Library")
        .join("Mobile Documents")
        .join("com~apple~CloudDocs");
    dir.is_dir().then_some(dir)
}

impl SettingsView {
    pub fn new(
        store: Option<Store>,
        vault: VaultSlot,
        sync: SyncWorker,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = store
            .as_ref()
            .and_then(|s| s.settings().ok())
            .unwrap_or_default();
        let masked = |cx: &mut Context<Self>, hint: fn() -> String| {
            cx.new(|cx| TextInput::said(hint, cx).masked())
        };
        let place = PlaceForm {
            kind: Kind::Folder,
            folder: said(cx, || tr!("A folder another app keeps in sync")),
            git_url: said(cx, || "git@github.com:me/vault.git".to_string()),
            git_branch: said(cx, || tr!("The repository's default")),
            dav_url: said(cx, || {
                "https://cloud.example.com/remote.php/dav/files/me/".to_string()
            }),
            dav_user: said(cx, || "me".to_string()),
            s3_endpoint: said(cx, || {
                "https://<account>.r2.cloudflarestorage.com".to_string()
            }),
            s3_region: said(cx, || "auto".to_string()),
            s3_bucket: said(cx, || "vault".to_string()),
            s3_prefix: said(cx, || tr!("None: the bucket's top")),
            s3_access: said(cx, || tr!("Access key")),
            secret: masked(cx, || tr!("Kept in the vault, on this device only")),
            git_token: masked(cx, || tr!("none: git signs in as it does for you")),
            git_user: said(cx, || tr!("any (Gitea: yours)")),
            problem: None,
            no_git: None,
            installing: false,
        };
        let join = cx.new(|cx| JoinView::new(sync.clone(), vault.clone(), cx));
        cx.subscribe(&join, |_, _, event: &JoinEvent, cx| match event {
            JoinEvent::Joined => cx.emit(SettingsEvent::Joined),
        })
        .detach();
        let mut view = SettingsView {
            page: Page::General,
            store,
            settings,
            vault,
            sync,
            sync_snap: SyncSnap::off(),
            join,
            update: None,
            place,
            pass: masked(cx, || tr!("At least 10 characters, hard to guess")),
            pass_again: masked(cx, || tr!("The same again")),
            shown_key: None,
            busy: false,
            problem: None,
            browsers: vec![],
            backup_pass: masked(cx, || tr!("A passphrase for this backup")),
            backup_pass_again: masked(cx, || tr!("The same again")),
            pending: None,
            import_pass: masked(cx, || tr!("The file's password")),
            focus: cx.focus_handle(),
        };
        view.fill_place(cx);
        view
    }

    pub fn set_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.problem = None;
        self.shown_key = None;
        if page == Page::Browsers {
            self.look_at_browsers();
        }
        cx.notify();
    }

    pub fn set_sync_snap(&mut self, snap: SyncSnap, cx: &mut Context<Self>) {
        if snap != self.sync_snap {
            self.sync_snap = snap;
            cx.notify();
        }
    }

    /// Where a request to join from this device stands.
    pub fn set_asking(&mut self, state: AskState, cx: &mut Context<Self>) {
        self.join.update(cx, |j, cx| j.set_asking(state, cx));
    }

    pub fn pairing_failed(&mut self, why: String, cx: &mut Context<Self>) {
        self.join.update(cx, |j, cx| j.failed(why, cx));
    }

    pub fn set_passphrase_joins(&mut self, joins: bool, cx: &mut Context<Self>) {
        self.join
            .update(cx, |j, cx| j.set_passphrase_joins(joins, cx));
    }

    pub fn set_update(&mut self, update: UpdateSnap, cx: &mut Context<Self>) {
        self.update = Some(update);
        cx.notify();
    }

    /// The form as config.toml has the place now.
    fn fill_place(&mut self, cx: &mut Context<Self>) {
        let s = &self.settings.sync;
        self.place.kind = if s.folder.is_some() {
            Kind::Folder
        } else if s.webdav.is_some() {
            Kind::WebDav
        } else if s.s3.is_some() {
            Kind::S3
        } else if s.git.is_some() {
            Kind::Git
        } else {
            Kind::Folder
        };
        let (folder, webdav, s3, git) = (
            s.folder.clone(),
            s.webdav.clone(),
            s.s3.clone(),
            s.git.clone(),
        );
        if let Some(f) = folder {
            set_text(&self.place.folder, &f, cx);
        }
        if let Some(w) = webdav {
            set_text(&self.place.dav_url, &w.url, cx);
            set_text(&self.place.dav_user, &w.user, cx);
        }
        if let Some(s3) = s3 {
            set_text(&self.place.s3_endpoint, &s3.endpoint, cx);
            set_text(&self.place.s3_region, &s3.region, cx);
            set_text(&self.place.s3_bucket, &s3.bucket, cx);
            set_text(&self.place.s3_prefix, &s3.prefix, cx);
            set_text(&self.place.s3_access, &s3.access_key, cx);
        }
        if let Some(g) = git {
            set_text(&self.place.git_url, &g.url, cx);
            set_text(&self.place.git_branch, &g.branch, cx);
            set_text(&self.place.git_user, &g.user, cx);
        }
    }

    /// Writes one setting to config.toml, and tells the window.
    fn set(&mut self, path: &[&str], value: Option<SettingValue>, cx: &mut Context<Self>) {
        let Some(store) = &self.store else {
            return;
        };
        match store.set_setting(path, value) {
            Ok(()) => {
                if let Ok(s) = store.settings() {
                    self.settings = s;
                }
                cx.emit(SettingsEvent::Changed);
            }
            Err(e) => self.problem = Some(e.to_string()),
        }
        cx.notify();
    }

    fn fail(&mut self, why: String, cx: &mut Context<Self>) {
        self.problem = Some(why);
        cx.notify();
    }

    /// Saves the place in the form (its secret in the vault) and syncs.
    /// The place the form has, and its secret (none for Git without a
    /// token), or what is missing.
    fn place_typed(&self, cx: &App) -> Result<(SyncPlaceText, Option<String>), String> {
        let f = &self.place;
        let need = |v: String, what: String| -> Result<String, String> {
            if v.is_empty() {
                Err(tr!("Fill in {what}.", what = what))
            } else {
                Ok(v)
            }
        };
        let typed = |i: &Entity<TextInput>| i.read(cx).text().trim().to_string();
        let saved = self
            .vault
            .ready()
            .and_then(|v| v.device_secret(PLACE_SECRET).ok().flatten())
            .is_some();
        // The password or secret key, or the one saved before.
        let secret = |missing: String| -> Result<Option<String>, String> {
            match typed(&f.secret) {
                s if !s.is_empty() => Ok(Some(s)),
                _ if saved => Ok(None),
                _ => Err(missing),
            }
        };
        match f.kind {
            Kind::Folder => Ok((
                SyncPlaceText::Folder(need(typed(&f.folder), tr!("the folder"))?),
                None,
            )),
            Kind::Git => {
                let place = GitSync {
                    url: need(typed(&f.git_url), tr!("the repository's address"))?,
                    branch: typed(&f.git_branch),
                    user: typed(&f.git_user),
                };
                let token = typed(&f.git_token);
                Ok((
                    SyncPlaceText::Git(place),
                    (!token.is_empty()).then_some(token),
                ))
            }
            Kind::WebDav => {
                let url = need(typed(&f.dav_url), tr!("the folder's address"))?;
                if !url.starts_with("https://") && !url.starts_with("http://") {
                    return Err(tr!("The address starts with https://."));
                }
                let user = need(typed(&f.dav_user), tr!("the user"))?;
                let password = secret(tr!("Fill in the password."))?;
                Ok((SyncPlaceText::WebDav(WebDavSync { url, user }), password))
            }
            Kind::S3 => {
                let endpoint = need(typed(&f.s3_endpoint), tr!("the endpoint"))?;
                if !endpoint.starts_with("https://") && !endpoint.starts_with("http://") {
                    return Err(tr!("The endpoint starts with https://."));
                }
                let bucket = need(typed(&f.s3_bucket), tr!("the bucket"))?;
                let access_key = need(typed(&f.s3_access), tr!("the access key"))?;
                let key = secret(tr!("Fill in the secret key."))?;
                Ok((
                    SyncPlaceText::S3(S3Sync {
                        endpoint,
                        region: typed(&f.s3_region),
                        bucket,
                        prefix: typed(&f.s3_prefix),
                        access_key,
                    }),
                    key,
                ))
            }
        }
    }

    /// Starts syncing with the place the form has. Git: the system's, or
    /// how to get it, first; and a repository anyone can read is asked
    /// about before anything goes there.
    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (place, secret) = match self.place_typed(cx) {
            Ok(p) => p,
            Err(e) => {
                self.place.problem = Some(e);
                return cx.notify();
            }
        };
        self.place.problem = None;
        self.place.no_git = None;
        if matches!(place, SyncPlaceText::Git(_))
            && let Err(e) = authwing_remote::Git::find()
        {
            self.place.no_git = Some(e);
            return cx.notify();
        }
        let Some(vault) = self.vault.ready() else {
            self.place.problem = Some(tr!("Open the vault first."));
            return cx.notify();
        };
        // The password, secret key or token, this device's own; for a
        // folder, or Git with no token, none: not the last place's.
        let keep = match (&place, &secret) {
            (SyncPlaceText::WebDav(_) | SyncPlaceText::S3(_), None) => Ok(()),
            (_, secret) => vault.set_device_secret(PLACE_SECRET, secret.as_deref()),
        };
        if let Err(e) = keep {
            self.place.problem = Some(tr!("Cannot keep the password: {e}", e = e.to_string()));
            return cx.notify();
        }
        for f in [&self.place.secret, &self.place.git_token] {
            set_text(f, "", cx);
        }
        let SyncPlaceText::Git(git) = &place else {
            return self.use_place(Some(place), cx);
        };
        let url = git.url.clone();
        self.busy = true;
        cx.notify();
        let check = cx.background_spawn(async move {
            authwing_remote::Git::find().is_ok_and(|git| git.readable_by_anyone(&url))
        });
        cx.spawn_in(window, async move |this, cx| {
            let public = check.await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.busy = false;
                if !public {
                    return view.use_place(Some(place), cx);
                }
                // Anyone can read it: used only if the user says so.
                let detail = tr!(
                    "What the vault keeps there is encrypted, but anyone can see how many codes it holds and when your devices sync. A private repository is safer."
                );
                let (another, anyway) = (tr!("Use another repository"), tr!("Use it anyway"));
                let answer = window.prompt(
                    PromptLevel::Warning,
                    &tr!("Anyone can read this repository"),
                    Some(&detail),
                    &[another.as_str(), anyway.as_str()],
                    cx,
                );
                cx.spawn(async move |this, cx| {
                    if matches!(answer.await, Ok(1)) {
                        let _ = this.update(cx, |view, cx| view.use_place(Some(place), cx));
                    }
                })
                .detach();
                cx.notify();
            });
        })
        .detach();
    }

    /// Writes where the vault syncs (None: nowhere), and syncs.
    fn use_place(&mut self, place: Option<SyncPlaceText>, cx: &mut Context<Self>) {
        let Some(store) = self.store.clone() else {
            return;
        };
        match store.set_sync_place(place.as_ref()) {
            Ok(()) => {
                if let Ok(s) = store.settings() {
                    self.settings = s;
                }
                self.place.problem = None;
                cx.emit(SettingsEvent::Changed);
                cx.emit(SettingsEvent::Say(Notice::good(match place {
                    Some(_) => tr!("Saved. Syncing now."),
                    None => tr!("Sync is off. What is in the place stays there."),
                })));
                self.sync.now();
            }
            Err(e) => self.place.problem = Some(e.to_string()),
        }
        cx.notify();
    }

    /// Stops syncing: the place is forgotten, and its password on this
    /// device; what is there stays.
    fn stop_sync(&mut self, cx: &mut Context<Self>) {
        if let Some(vault) = self.vault.ready() {
            let _ = vault.set_device_secret(PLACE_SECRET, None);
        }
        self.use_place(None, cx);
        self.fill_place(cx);
    }

    /// Installs git where the system asks for the rights itself, then looks
    /// for it again.
    fn install_git(&mut self, how: authwing_remote::Install, cx: &mut Context<Self>) {
        self.place.installing = true;
        cx.notify();
        let job = cx.background_spawn(async move {
            authwing_remote::git::install(&how)?;
            Ok::<_, std::io::Error>(authwing_remote::Git::find())
        });
        cx.spawn(async move |this, cx| {
            let done = job.await;
            let _ = this.update(cx, |view, cx| {
                let form = &mut view.place;
                form.installing = false;
                match done {
                    Ok(Ok(git)) => {
                        form.no_git = None;
                        form.problem = None;
                        cx.emit(SettingsEvent::Say(Notice::good(tr!(
                            "git {version} is installed: Connect again.",
                            version = git.version().to_string()
                        ))));
                    }
                    // Apple's dialog goes on by itself after this returns.
                    Ok(Err(_)) if cfg!(target_os = "macos") => {
                        form.problem = Some(tr!("When Apple's installer is done, Connect again."));
                    }
                    Ok(Err(e)) => form.no_git = Some(e),
                    Err(e) => {
                        form.problem = Some(tr!("git was not installed: {e}", e = e.to_string()))
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(tr!("Sync here").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(p) = paths.into_iter().next()
            {
                let _ = this.update(cx, |view, cx| {
                    set_text(&view.place.folder, &p.display().to_string(), cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn show_key(&mut self, cx: &mut Context<Self>) {
        match self.vault.ready().map(|v| v.recovery_key()) {
            Some(Ok(Some(k))) => self.shown_key = Some(k.to_string()),
            Some(Ok(None)) => self.problem = Some(tr!("This vault has no recovery key yet.")),
            Some(Err(e)) => self.problem = Some(e.to_string()),
            None => {}
        }
        cx.notify();
    }

    /// Sets the vault's passphrase from the two fields, if they agree and it
    /// is hard enough to guess.
    fn set_passphrase(&mut self, cx: &mut Context<Self>) {
        let (p, again) = (
            self.pass.read(cx).text().to_string(),
            self.pass_again.read(cx).text().to_string(),
        );
        if p.chars().count() < authwing_vault::MIN_PASSPHRASE {
            return self.fail(tr!("A passphrase needs at least 10 characters."), cx);
        }
        if p != again {
            return self.fail(tr!("The two passphrases differ."), cx);
        }
        if authwing_vault::passphrase_strength(&p) < 3 {
            return self.fail(tr!("Too easy to guess: try several unrelated words."), cx);
        }
        let Some(vault) = self.vault.ready() else {
            return;
        };
        self.busy = true;
        self.problem = None;
        // Argon2 takes half a second: off the window's thread.
        let done = cx.background_spawn(async move {
            vault
                .set_vault_passphrase(Some(&p))
                .map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = done.await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(()) => {
                        set_text(&view.pass, "", cx);
                        set_text(&view.pass_again, "", cx);
                        cx.emit(SettingsEvent::Say(Notice::good(tr!(
                            "The passphrase is set."
                        ))));
                    }
                    Err(e) => view.problem = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Puts the vault's key in the keychain (it opens by itself), or takes
    /// it out (only the passphrase opens it, and it can lock).
    fn set_keychain(&mut self, keep: bool, cx: &mut Context<Self>) {
        let done = if keep {
            self.vault.keep_in_keychain()
        } else {
            self.vault.leave_keychain()
        };
        match done {
            Ok(()) => self.problem = None,
            Err(e) => self.problem = Some(e),
        }
        cx.notify();
    }

    /// Lets Touch ID or Windows Hello open the vault, or stops it: the system
    /// asks first, off the window's thread.
    fn set_presence(&mut self, on: bool, cx: &mut Context<Self>) {
        self.busy = true;
        let slot = self.vault.clone();
        let done = cx.background_spawn(async move { slot.set_presence(on) });
        cx.spawn(async move |this, cx| {
            let result = done.await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                view.problem = result.err();
                cx.notify();
            });
        })
        .detach();
    }

    /// The codes in the vault, as a backup holds them.
    fn items(&self) -> Option<Vec<Item>> {
        let vault = self.vault.ready()?;
        let (codes, _) = crate::codes::load(&vault);
        Some(
            codes
                .into_iter()
                .map(|e| Item {
                    otp: e.otp,
                    group: e.group,
                    sites: e.sites,
                })
                .collect(),
        )
    }

    /// Asks where to save, then writes what `make` gives there, off the
    /// window's thread (sealing derives a key).
    fn save_file(
        &mut self,
        name: String,
        plain: bool,
        make: impl FnOnce() -> Result<Vec<u8>, String> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        let dir = std::env::home_dir()
            .map(|h| {
                let docs = h.join("Documents");
                if docs.is_dir() { docs } else { h }
            })
            .unwrap_or_else(std::env::temp_dir);
        let path = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(path))) = path.await else {
                return;
            };
            let written = cx
                .background_spawn(async move {
                    let bytes = make()?;
                    write_private(&path, &bytes, plain).map_err(|e| e.to_string())?;
                    Ok::<_, String>(path)
                })
                .await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match written {
                    Ok(path) => {
                        view.problem = None;
                        cx.emit(SettingsEvent::Say(Notice::good(tr!(
                            "Saved: {path}",
                            path = path.display().to_string()
                        ))));
                    }
                    Err(e) => view.problem = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Saves every code, sealed with the passphrase in the two fields.
    fn save_backup(&mut self, cx: &mut Context<Self>) {
        let (p, again) = (
            self.backup_pass.read(cx).text().to_string(),
            self.backup_pass_again.read(cx).text().to_string(),
        );
        if p.chars().count() < authwing_backup::MIN_PASSPHRASE {
            return self.fail(tr!("A passphrase needs at least 10 characters."), cx);
        }
        if p != again {
            return self.fail(tr!("The two passphrases differ."), cx);
        }
        if authwing_vault::passphrase_strength(&p) < 3 {
            return self.fail(tr!("Too easy to guess: try several unrelated words."), cx);
        }
        let Some(items) = self.items() else {
            return self.fail(tr!("Open the vault first."), cx);
        };
        if items.is_empty() {
            return self.fail(tr!("There are no codes to save yet."), cx);
        }
        set_text(&self.backup_pass, "", cx);
        set_text(&self.backup_pass_again, "", cx);
        self.busy = true;
        let name = format!("authwing-{}.{}", today(), authwing_backup::EXTENSION);
        self.save_file(
            name,
            false,
            move || authwing_backup::seal(&items, &p).map_err(|e| backup_error(&e)),
            cx,
        );
    }

    /// Saves every code as plain otpauth links, after a question.
    fn save_plain(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(items) = self.items() else {
            return self.fail(tr!("Open the vault first."), cx);
        };
        if items.is_empty() {
            return self.fail(tr!("There are no codes to save yet."), cx);
        }
        let (yes, no) = (tr!("Save unencrypted"), tr!("Cancel"));
        let answer = window.prompt(
            PromptLevel::Warning,
            &tr!("Save the codes unencrypted?"),
            Some(&tr!(
                "Anyone who sees the file can make your codes. Use it to move to another app, then delete it."
            )),
            &[yes.as_str(), no.as_str()],
            cx,
        );
        cx.spawn(async move |this, cx| {
            if !matches!(answer.await, Ok(0)) {
                return;
            }
            let _ = this.update(cx, |view, cx| {
                let name = format!("authwing-{}.txt", today());
                view.save_file(
                    name,
                    true,
                    move || Ok(authwing_backup::plain(&items).into_bytes()),
                    cx,
                );
            });
        })
        .detach();
    }

    fn choose_import(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("Import").into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let read = std::fs::read(&path);
            let _ = this.update(cx, |view, cx| match read {
                Ok(bytes) => {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    view.import(name, bytes, None, cx)
                }
                Err(e) => view.fail(e.to_string(), cx),
            });
        })
        .detach();
    }

    /// Reads a file to import, off the window's thread (a password may need
    /// a slow key), and adds its codes; asks for its password when it has one.
    fn import(
        &mut self,
        name: String,
        bytes: Vec<u8>,
        password: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.busy = true;
        self.problem = None;
        let kept = bytes.clone();
        let read = cx
            .background_spawn(async move { authwing_backup::import(&bytes, password.as_deref()) });
        cx.spawn(async move |this, cx| {
            let result = read.await;
            let _ = this.update(cx, |view, cx| {
                view.busy = false;
                match result {
                    Ok(imported) => {
                        view.pending = None;
                        set_text(&view.import_pass, "", cx);
                        view.add_imported(imported, cx);
                    }
                    Err(authwing_backup::Error::PasswordNeeded(format)) => {
                        view.pending = Some(Pending {
                            name,
                            format,
                            bytes: kept,
                        });
                    }
                    Err(e) => {
                        set_text(&view.import_pass, "", cx);
                        view.problem = Some(backup_error(&e));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Adds the codes that are not in the vault already.
    fn add_imported(&mut self, imported: Imported, cx: &mut Context<Self>) {
        let Some(vault) = self.vault.ready() else {
            return self.fail(tr!("Open the vault first."), cx);
        };
        let (have, _) = crate::codes::load(&vault);
        let same = |a: &authwing_totp::Otp, b: &authwing_totp::Otp| {
            a.secret.as_bytes() == b.secret.as_bytes()
                && a.issuer == b.issuer
                && a.account == b.account
        };
        let (mut added, mut there) = (0usize, 0usize);
        for item in &imported.items {
            if have.iter().any(|e| same(&e.otp, &item.otp)) {
                there += 1;
                continue;
            }
            let filing = crate::codes::Filing {
                group: item.group.clone(),
                sites: item.sites.clone(),
                favorite: false,
            };
            match crate::codes::save(&vault, None, &item.otp, &filing) {
                Ok(_) => added += 1,
                Err(e) => self.problem = Some(e),
            }
        }
        let left_out = imported.skipped.len();
        let mut said = tr_n!(added, "{n} code added.", "{n} codes added.");
        if there > 0 {
            said.push(' ');
            said.push_str(&tr_n!(
                there,
                "{n} was here already.",
                "{n} were here already."
            ));
        }
        if left_out > 0 {
            said.push(' ');
            said.push_str(&tr_n!(
                left_out,
                "{n} left out: authwing cannot use it.",
                "{n} left out: authwing cannot use them."
            ));
        }
        cx.emit(SettingsEvent::CodesChanged);
        cx.emit(SettingsEvent::Say(if added > 0 {
            Notice::good(said)
        } else {
            Notice::warn(said)
        }));
    }

    fn look_at_browsers(&mut self) {
        let program = std::env::current_exe().ok();
        self.browsers = match (Dirs::for_user(), program) {
            (Ok(dirs), Some(program)) => install::status(Os::CURRENT, &dirs, &program),
            _ => vec![],
        };
    }

    fn install_browsers(&mut self, on: bool, cx: &mut Context<Self>) {
        let done = Dirs::for_user().and_then(|dirs| {
            if on {
                let program = std::env::current_exe()?;
                install::install(Os::CURRENT, &dirs, &program)
            } else {
                install::uninstall(Os::CURRENT, &dirs)
            }
        });
        match done {
            Ok(places) if on && places.is_empty() => {
                self.problem = Some(tr!("No browser the extension runs in was found."));
            }
            Ok(_) => self.problem = None,
            Err(e) => self.problem = Some(e.to_string()),
        }
        self.look_at_browsers();
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if ev.keystroke.key == "escape" {
            cx.emit(SettingsEvent::Close);
            cx.stop_propagation();
        }
    }
}

/// A section's heading on a page.
fn heading(text: String) -> gpui::Div {
    div()
        .pt(px(8.))
        .text_size(ui(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(colors().muted))
        .child(text)
}

/// A box on a page holding rows.
fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(10.))
        .bg(rgb(colors().surface))
        .border_1()
        .border_color(rgb(colors().line))
}

/// A setting: what it is, a line on it, and its control at the right.
fn row(label: String, hint: String, control: impl IntoElement) -> gpui::Div {
    let c = colors();
    div()
        .flex()
        .items_center()
        .gap(px(16.))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().child(label))
                .when(!hint.is_empty(), |d| {
                    d.child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.muted))
                            .child(hint),
                    )
                }),
        )
        .child(control)
}

fn hint(text: String) -> gpui::Div {
    div()
        .text_size(ui(12.))
        .text_color(rgb(colors().muted))
        .child(text)
}

/// The apps whose exports Settings › Backup reads, by kind: their own
/// names, in no language.
const IMPORT_AUTHENTICATORS: &str = "Aegis, 2FAS, andOTP, Ente Auth, FreeOTP+, Proton Authenticator, Raivo, Stratum (Authenticator Pro), TOTP Authenticator, Steam (maFile)";
const IMPORT_PASSWORD_MANAGERS: &str =
    "1Password, Apple Passwords, Bitwarden, Dashlane, KeePassXC, Proton Pass";

/// A kind of app, or an app, and a line on it.
fn apps(label: String, text: &str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(div().text_size(ui(12.)).child(label))
        .child(hint(text.to_string()))
}

impl SettingsView {
    fn general(&self, cx: &mut Context<Self>) -> gpui::Div {
        let s = &self.settings;
        let current = s.general.language.as_deref().filter(|l| *l != "auto");
        let mut languages = div().flex().flex_wrap().gap(px(6.)).child(
            choice("lang-auto", &tr!("As the system"), current.is_none()).on_click(
                cx.listener(|view, _, _, cx| view.set(&["general", "language"], None, cx)),
            ),
        );
        for (tag, name) in authwing_i18n::LANGUAGES {
            languages = languages.child(
                choice(&format!("lang-{tag}"), name, current == Some(*tag)).on_click(cx.listener(
                    move |view, _, _, cx| {
                        view.set(
                            &["general", "language"],
                            Some(SettingValue::Str((*tag).to_string())),
                            cx,
                        )
                    },
                )),
            );
        }
        let mode = s.appearance.mode;
        let modes = [
            (ColorMode::System, "system", tr!("As the system")),
            (ColorMode::Dark, "dark", tr!("Dark")),
            (ColorMode::Light, "light", tr!("Light")),
        ];
        let mut mode_row = div().flex().gap(px(6.));
        for (m, name, label) in modes {
            mode_row = mode_row.child(choice(&format!("mode-{name}"), &label, mode == m).on_click(
                cx.listener(move |view, _, _, cx| {
                    view.set(
                        &["appearance", "mode"],
                        Some(SettingValue::Str(name.to_string())),
                        cx,
                    )
                }),
            ));
        }
        let size = s.appearance.ui_font.size;
        let mut sizes = div().flex().gap(px(6.));
        for n in [12., 13., 14., 16., 18.] {
            sizes = sizes.child(
                choice(
                    &format!("size-{n}"),
                    &format!("{n}"),
                    (size - n).abs() < 0.1,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.set(
                        &["appearance", "ui_font", "size"],
                        Some(SettingValue::Float(f64::from(n))),
                        cx,
                    )
                })),
            );
        }
        let clear = s.codes.clear_clipboard;
        let mut clears = div().flex().gap(px(6.));
        for (secs, label) in [
            (10, tr!("10 s")),
            (20, tr!("20 s")),
            (45, tr!("45 s")),
            (90, tr!("90 s")),
            (0, tr!("Never")),
        ] {
            clears = clears.child(
                choice(&format!("clear-{secs}"), &label, clear == secs).on_click(cx.listener(
                    move |view, _, _, cx| {
                        view.set(
                            &["codes", "clear_clipboard"],
                            Some(SettingValue::Int(i64::from(secs))),
                            cx,
                        )
                    },
                )),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(tr!("Language")))
            .child(
                card()
                    .child(languages)
                    .child(hint(tr!("This device's own: it does not sync."))),
            )
            .child(heading(tr!("Appearance")))
            .child(
                card()
                    .child(row(tr!("Colors"), String::new(), mode_row))
                    .child(row(tr!("Text size"), String::new(), sizes)),
            )
            .child(heading(tr!("Quick panel")))
            .child(self.panel_card(cx))
            .child(heading(tr!("Codes")))
            .child(card().child(row(
                tr!("Clear a copied code from the clipboard after"),
                tr!("Only if the clipboard still holds it."),
                clears,
            )))
    }

    /// The quick panel's shortcut, and whether codes can be typed into
    /// other apps.
    fn panel_card(&self, cx: &mut Context<Self>) -> gpui::Div {
        let key = self.settings.panel.key.trim().to_string();
        let mac = cfg!(target_os = "macos");
        // Elsewhere Alt+Space opens the window's menu, and Windows keeps
        // Win+Space and Win+Shift+Space for the keyboard's language.
        let mut keys: Vec<(String, String)> = if mac {
            vec![
                ("alt+space".into(), "⌥Space".into()),
                ("ctrl+alt+space".into(), "⌃⌥Space".into()),
                ("cmd+shift+space".into(), "⌘⇧Space".into()),
            ]
        } else {
            vec![
                ("ctrl+alt+space".into(), "Ctrl+Alt+Space".into()),
                ("alt+shift+space".into(), "Alt+Shift+Space".into()),
                ("ctrl+shift+space".into(), "Ctrl+Shift+Space".into()),
            ]
        };
        // One written in config.toml by hand, shown as written.
        if !key.is_empty() && !keys.iter().any(|(k, _)| *k == key) {
            keys.insert(0, (key.clone(), key.clone()));
        }
        keys.push((String::new(), tr!("Off")));
        let mut choices = div().flex().flex_wrap().gap(px(6.));
        for (k, label) in keys {
            let on = key == k;
            choices = choices.child(choice(&format!("panel-{k}"), &label, on).on_click(
                cx.listener(move |view, _, _, cx| {
                    view.set(&["panel", "key"], Some(SettingValue::Str(k.clone())), cx)
                }),
            ));
        }
        let allowed = crate::typing::allowed();
        card()
            .child(row(
                tr!("Shortcut"),
                tr!("Opens it over any app, with that app's codes first. This device's own."),
                choices,
            ))
            .child(row(
                tr!("Type codes into other apps"),
                if allowed {
                    tr!("Allowed: Enter in the quick panel types the code where you were.")
                } else if mac {
                    tr!("macOS asks first (Accessibility). Until then, Enter copies the code.")
                } else {
                    tr!("Enter in the quick panel types the code where you were.")
                },
                div().when(mac && !allowed, |d| {
                    d.child(
                        outline_button("allow-typing", &tr!("Allow…")).on_click(cx.listener(
                            |_, _, _, cx| {
                                authwing_context::window::ask_title_access();
                                cx.notify();
                            },
                        )),
                    )
                }),
            ))
    }

    /// Voice in the quick panel: its model, downloaded here, and the
    /// language names are said in.
    fn voice_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let s = &self.settings;
        let chosen = authwing_voice::model::by_name(&s.voice.model);
        let dir = Store::models_dir();
        let ready = chosen.is_some_and(|m| dir.as_ref().is_some_and(|d| m.is_in(d)));
        let (downloading, failed) = cx
            .try_global::<crate::voice::Voice>()
            .map(|v| {
                (
                    v.download.as_ref().map(|d| (d.model, d.done())),
                    v.failed.clone(),
                )
            })
            .unwrap_or_default();
        let mb = |bytes: u64| (bytes + 500_000) / 1_000_000;

        let mut models = div().flex().flex_wrap().gap(px(6.));
        for model in authwing_voice::model::MODELS {
            let label = if *model == authwing_voice::model::RECOMMENDED {
                tr!(
                    "{name} · {mb} MB (recommended)",
                    name = model.name,
                    mb = mb(model.bytes)
                )
            } else {
                tr!("{name} · {mb} MB", name = model.name, mb = mb(model.bytes))
            };
            let name = model.name;
            models = models.child(
                choice(
                    &format!("voice-model-{name}"),
                    &label,
                    chosen.is_some_and(|m| m.name == name),
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.set(
                        &["voice", "model"],
                        Some(SettingValue::Str(name.to_string())),
                        cx,
                    )
                })),
            );
        }
        models = models.child(
            choice("voice-model-off", &tr!("Off"), chosen.is_none()).on_click(cx.listener(
                |view, _, _, cx| {
                    view.set(
                        &["voice", "model"],
                        Some(SettingValue::Str(String::new())),
                        cx,
                    )
                },
            )),
        );

        let state: gpui::AnyElement = match (chosen, downloading) {
            (_, Some((model, done))) => div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .flex_1()
                        .h(px(6.))
                        .rounded_full()
                        .bg(rgb(c.line))
                        .child(
                            div()
                                .h_full()
                                .rounded_full()
                                .bg(rgb(c.accent))
                                .w(gpui::relative(done.clamp(0.0, 1.0) as f32)),
                        ),
                )
                .child(div().text_size(ui(12.)).text_color(rgb(c.muted)).child(tr!(
                    "Downloading {name}: {done} of {mb} MB",
                    name = model.name,
                    done = mb((done * model.bytes as f64) as u64),
                    mb = mb(model.bytes)
                )))
                .child(
                    outline_button("voice-stop", &tr!("Stop")).on_click(cx.listener(
                        |_, _, _, cx| {
                            cx.update_global(|v: &mut crate::voice::Voice, _| v.stop_download());
                        },
                    )),
                )
                .into_any_element(),
            (Some(model), None) if ready => row(
                tr!(
                    "Ready: {name}, {mb} MB",
                    name = model.name,
                    mb = mb(model.bytes)
                ),
                tr!(
                    "On this device, in {dir}",
                    dir = dir
                        .as_ref()
                        .map(|d| d.display().to_string())
                        .unwrap_or_default()
                ),
                outline_button("voice-remove", &tr!("Remove")).on_click(cx.listener(
                    move |view, _, _, cx| {
                        if let Some(dir) = Store::models_dir()
                            && let Err(e) = std::fs::remove_file(model.path_in(&dir))
                        {
                            view.fail(e.to_string(), cx);
                        }
                        cx.notify();
                    },
                )),
            )
            .into_any_element(),
            (Some(model), None) => row(
                tr!("Not downloaded yet"),
                tr!(
                    "From Hugging Face, where whisper.cpp publishes it; checked before it is used."
                ),
                primary_button(
                    "voice-download",
                    &tr!("Download {mb} MB", mb = mb(model.bytes)),
                )
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.update_global(|v: &mut crate::voice::Voice, cx| v.start_download(model, cx));
                    cx.notify();
                })),
            )
            .into_any_element(),
            (None, None) => hint(tr!("Voice is off.")).into_any_element(),
        };

        let set = s.voice.language.trim();
        let mut languages = div().flex().flex_wrap().gap(px(6.)).child(
            choice(
                "voice-lang-auto",
                &tr!("As the app"),
                set == "auto" || set.is_empty(),
            )
            .on_click(cx.listener(|view, _, _, cx| view.set(&["voice", "language"], None, cx))),
        );
        for (tag, name) in authwing_i18n::LANGUAGES {
            languages = languages.child(
                choice(&format!("voice-lang-{tag}"), name, set == *tag).on_click(cx.listener(
                    move |view, _, _, cx| {
                        view.set(
                            &["voice", "language"],
                            Some(SettingValue::Str((*tag).to_string())),
                            cx,
                        )
                    },
                )),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(tr!("Find a code by voice")))
            .child(
                card()
                    .child(hint(if s.panel.key.trim().is_empty() {
                        tr!(
                            "The quick panel's shortcut is off (General › Quick panel): with one, hold it and say a code's name. Whisper listens on this device: nothing you say leaves it."
                        )
                    } else {
                        tr!(
                            "In the quick panel, hold {key} and say a code's name: “Open GitHub”, “PayPal”. Whisper listens on this device: nothing you say leaves it, and the microphone is on only while the key is held.",
                            key = crate::panel_key::label(&s.panel.key)
                        )
                    }))
                    .child(row(
                        tr!("Model"),
                        tr!("The larger ones are slower, and named no more codes right in our tests."),
                        div(),
                    ))
                    .child(models)
                    .child(state)
                    .when_some(failed, |d, why| {
                        d.child(
                            div()
                                .text_size(ui(12.))
                                .text_color(rgb(c.bad))
                                .child(tr!("Not downloaded: {why}", why = why)),
                        )
                    }),
            )
            .child(heading(tr!("The language you speak")))
            .child(
                card().child(languages).child(hint(tr!(
                    "Names said the English way are heard too. This device's own: it does not sync."
                ))),
            )
    }

    /// Where to sync, while not syncing: a folder, or a server's address
    /// and sign-in, as shellwing has it.
    fn sync_choice(&self, cx: &mut Context<Self>) -> gpui::Div {
        let f = &self.place;
        let kinds = [
            (Kind::Folder, "folder", tr!("A folder")),
            (Kind::WebDav, "webdav", "WebDAV".to_string()),
            (Kind::S3, "s3", tr!("S3-compatible")),
            (Kind::Git, "git", "Git".to_string()),
        ];
        let mut pick = div().flex().flex_wrap().gap(px(6.));
        for (k, name, label) in kinds {
            pick = pick.child(
                choice(&format!("place-{name}"), &label, f.kind == k).on_click(cx.listener(
                    move |view, _, _, cx| {
                        view.place.kind = k;
                        view.place.problem = None;
                        view.place.no_git = None;
                        cx.notify();
                    },
                )),
            );
        }
        let saved_secret = self
            .vault
            .ready()
            .and_then(|v| v.device_secret(PLACE_SECRET).ok().flatten())
            .is_some();
        let body = match f.kind {
            Kind::Folder => {
                let icloud = icloud_drive();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(hint(tr!(
                        "iCloud Drive, Syncthing, a Dropbox or OneDrive folder, a network share."
                    )))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(div().flex_1().child(input_box(&f.folder)))
                            .child(
                                outline_button("choose-folder", &tr!("Choose a folder…"))
                                    .on_click(cx.listener(|view, _, _, cx| view.choose_folder(cx))),
                            ),
                    )
                    .when_some(icloud, |d, dir| {
                        let path = dir.join("authwing").display().to_string();
                        d.child(div().flex().child(
                            text_button("use-icloud", &tr!("Use iCloud Drive"), colors().accent)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    set_text(&view.place.folder, &path, cx);
                                    cx.notify();
                                })),
                        ))
                    })
            }
            Kind::WebDav => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(hint(tr!(
                    "Nextcloud, ownCloud, Synology, Koofr, Fastmail: the address of a folder there."
                )))
                .child(labeled(tr!("Address"), &f.dav_url))
                .child(labeled(tr!("User"), &f.dav_user))
                .child(labeled(
                    if saved_secret {
                        tr!("Password (saved; type a new one to change it)")
                    } else {
                        tr!("Password")
                    },
                    &f.secret,
                )),
            Kind::S3 => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(hint(tr!(
                    "AWS S3, Cloudflare R2, Backblaze B2, MinIO, Wasabi: a bucket, and a key that can read and write it."
                )))
                .child(labeled(tr!("Endpoint"), &f.s3_endpoint))
                .child(labeled(tr!("Region (if it asks for one)"), &f.s3_region))
                .child(labeled(tr!("Bucket"), &f.s3_bucket))
                .child(labeled(tr!("Folder in the bucket"), &f.s3_prefix))
                .child(labeled(tr!("Access key"), &f.s3_access))
                .child(labeled(
                    if saved_secret {
                        tr!("Secret key (saved; type a new one to change it)")
                    } else {
                        tr!("Secret key")
                    },
                    &f.secret,
                )),
            Kind::Git => div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .child(hint(tr!(
                    "A private repository on GitHub, GitLab, Gitea or your own server, through this system's git: it signs in as it does for you (its credential manager, your SSH keys and agent). Each change is a commit."
                )))
                .child(labeled(tr!("Repository"), &f.git_url))
                .child(labeled(tr!("Branch"), &f.git_branch))
                .child(labeled(
                    tr!("Token (if git cannot sign in by itself)"),
                    &f.git_token,
                ))
                .child(labeled(tr!("The token's user"), &f.git_user))
                .when_some(f.no_git.clone(), |d, no| d.child(self.no_git(no, cx))),
        };
        let remote = f.kind != Kind::Folder;
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(hint(tr!(
                "Your codes travel through a folder another app syncs, a WebDAV server, S3-compatible storage or Git. Everything there is encrypted with your recovery key: the server never sees a code."
            )))
            .child(pick)
            .child(body)
            .when(remote, |d| {
                d.child(hint(tr!(
                    "The password, secret key or token stays on this device, in the vault; other devices sign in with their own."
                )))
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        primary_button("sync-connect", &tr!("Connect"))
                            .on_click(cx.listener(|view, _, window, cx| view.connect(window, cx))),
                    )
                    .when(self.busy, |d| {
                        d.child(hint(tr!("Asking the repository…")))
                    })
                    .when(f.installing, |d| d.child(hint(tr!("Installing git…"))))
                    .when_some(f.problem.clone(), |d, p| {
                        d.child(
                            div()
                                .text_size(ui(12.))
                                .text_color(rgb(colors().bad))
                                .child(p),
                        )
                    }),
            )
    }

    /// Why git cannot be used, and how to get it.
    fn no_git(&self, no: authwing_remote::NoGit, cx: &mut Context<Self>) -> gpui::Div {
        use authwing_remote::{Install, NoGit};
        let c = colors();
        let how = match &no {
            NoGit::Missing(how) => Some(how.clone()),
            _ => None,
        };
        let what = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(c.warn_edge))
            .bg(rgb(c.warn_bg))
            .text_size(ui(12.))
            .child(div().text_color(rgb(c.warn_text)).child(format!("{no}.")));
        match how {
            Some(Install::CommandLineTools) => what
                .child(hint(tr!(
                    "macOS installs it with Apple's Command Line Tools, in a dialog of its own."
                )))
                .child(div().flex().child(
                    outline_button("git-install", &tr!("Install git…")).on_click(cx.listener(
                        |view, _, _, cx| view.install_git(Install::CommandLineTools, cx),
                    )),
                )),
            Some(Install::Winget) => what
                .child(hint(tr!(
                    "Windows installs it with winget, and asks to allow it."
                )))
                .child(div().flex().child(
                    outline_button("git-install", &tr!("Install git…")).on_click(
                        cx.listener(|view, _, _, cx| view.install_git(Install::Winget, cx)),
                    ),
                )),
            Some(Install::Download(url)) => what.child(
                div().flex().child(
                    outline_button("git-download", &tr!("Download git"))
                        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url))),
                ),
            ),
            Some(Install::Command(cmd)) => {
                let copied = cmd.clone();
                what.child(hint(tr!("Install it in a terminal, then Connect again:")))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .px(px(8.))
                                    .py(px(4.))
                                    .rounded(px(5.))
                                    .bg(rgb(c.field))
                                    .font_family("monospace")
                                    .child(cmd),
                            )
                            .child(
                                outline_button("git-copy", &tr!("Copy")).on_click(cx.listener(
                                    move |_, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            copied.clone(),
                                        ));
                                    },
                                )),
                            ),
                    )
            }
            // Too old, or failing: what it said is above.
            None => what.child(hint(tr!(
                "Install a newer git (git-scm.com), then Connect again."
            ))),
        }
    }

    /// Settings › Sync, as shellwing's. Not syncing: authwing cloud, then a
    /// place of one's own to set up. Syncing: how it goes, the place (and
    /// joining the vault it holds), and authwing cloud beside it. Only the
    /// vault syncs: the settings are each device's own.
    fn sync_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let page = div().flex().flex_col().gap(px(12.));
        let Some(place) = self.settings.sync.place() else {
            return page
                .child(
                    div()
                        .p(px(22.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(rgb(c.line))
                        .bg(rgb(c.surface))
                        .child(crate::sync_panel::invitation()),
                )
                .child(heading(tr!("Or a place of your own")))
                .child(card().child(self.sync_choice(cx)));
        };
        let snap = &self.sync_snap;
        let shown = crate::syncing::describe(&place);
        let status = crate::sync_panel::status_card(
            &snap.standing,
            snap.place.as_deref().unwrap_or(&shown),
            Box::new(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::SyncNow))),
            Box::new(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Close))),
        );
        // A place that holds a vault this device is not in yet: join it.
        let joining = snap.other_vault;
        page.child(status)
            .when_some(snap.codes, |d, n| {
                d.child(
                    div()
                        .mt(px(-6.))
                        .px(px(4.))
                        .text_size(ui(12.5))
                        .text_color(rgb(c.soft))
                        .child(crate::sync_panel::codes_said(n)),
                )
            })
            .child(heading(tr!("Your place")))
            .child(
                card()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(240.))
                                    .child(tr!("With {place}", place = shown)),
                            )
                            .child(
                                outline_button("sync-key", &tr!("Show the recovery key")).on_click(
                                    cx.listener(|view, _, _, cx| view.set_page(Page::Security, cx)),
                                ),
                            )
                            .child(
                                outline_button("sync-stop", &tr!("Stop syncing"))
                                    .border_color(rgb(c.bad_edge))
                                    .text_color(rgb(c.bad_text))
                                    .on_click(cx.listener(|view, _, _, cx| view.stop_sync(cx))),
                            ),
                    )
                    .when(joining, |d| {
                        d.child(div().h(px(1.)).bg(rgb(c.line)))
                            .child(self.join.clone())
                    }),
            )
            .child(div().pt(px(8.)).child(crate::sync_panel::aside()))
    }

    fn security(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let vault = self.vault.ready();
        let has_pass = vault
            .as_ref()
            .is_some_and(|v| v.has_vault_passphrase().unwrap_or(false));
        let in_keychain = self.vault.in_keychain();
        let presence = self.vault.presence();
        let presence_on = vault
            .as_ref()
            .is_some_and(|v| v.has_presence().unwrap_or(false));
        let key =
            match &self.shown_key {
                None => div().flex().child(
                    outline_button("show-key", &tr!("Show the recovery key"))
                        .on_click(cx.listener(|view, _, _, cx| view.show_key(cx))),
                ),
                Some(k) => {
                    let copy = k.clone();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .p(px(10.))
                                .rounded(px(6.))
                                .bg(rgb(c.field))
                                .text_size(ui(15.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(k.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(8.))
                                .child(outline_button("copy-key", &tr!("Copy")).on_click(
                                    cx.listener(move |_, _, _, cx| {
                                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                            copy.clone(),
                                        ));
                                    }),
                                ))
                                .child(text_button("hide-key", &tr!("Hide"), c.soft).on_click(
                                    cx.listener(|view, _, _, cx| {
                                        view.shown_key = None;
                                        cx.notify();
                                    }),
                                )),
                        )
                }
            };
        let pass_title = if has_pass {
            tr!("Change the passphrase")
        } else {
            tr!("Set a passphrase")
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(tr!("Recovery key")))
            .child(card().child(hint(tr!(
                "It opens the vault when nothing else can: a lost keychain, a forgotten passphrase, a new device. Write it down and keep it apart from this computer."
            ))).child(key))
            .child(heading(tr!("Passphrase")))
            .child(
                card()
                    .child(hint(if has_pass {
                        tr!("A passphrase opens this vault. Set another below.")
                    } else {
                        tr!("No passphrase yet: the keychain alone opens the vault, and it cannot lock.")
                    }))
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(div().flex_1().child(labeled(pass_title, &self.pass)))
                            .child(div().flex_1().child(labeled(tr!("Again"), &self.pass_again))),
                    )
                    .child(
                        div().flex().justify_end().child(
                            primary_button(
                                "set-pass",
                                &if self.busy {
                                    tr!("Working…")
                                } else {
                                    tr!("Save the passphrase")
                                },
                            )
                            .on_click(cx.listener(|view, _, _, cx| {
                                if !view.busy {
                                    view.set_passphrase(cx)
                                }
                            })),
                        ),
                    ),
            )
            .child(heading(tr!("Opening and locking")))
            .child(
                card()
                    .child(row(
                        tr!("Open by itself"),
                        tr!("The vault's key stays in this computer's keychain. Off: the passphrase opens it, and Lock closes it."),
                        switch("keychain", in_keychain).on_click(cx.listener(move |view, _, _, cx| {
                            view.set_keychain(!in_keychain, cx)
                        })),
                    ))
                    .when_some(presence, |d, p| {
                        d.child(row(
                            tr!("Open with {what}", what = p.name().to_string()),
                            tr!("Needs a passphrase set. The system asks when you change it."),
                            switch("presence", presence_on).on_click(cx.listener(
                                move |view, _, _, cx| {
                                    if !view.busy {
                                        view.set_presence(!presence_on, cx)
                                    }
                                },
                            )),
                        ))
                    })
                    .child(row(
                        tr!("Lock now"),
                        tr!("Closes the vault until its passphrase is typed."),
                        outline_button("lock-now", &tr!("Lock"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Lock))),
                    )),
            )
    }

    fn backup_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let busy = self.busy;
        let pending =
            self.pending.as_ref().map(|p| {
                card()
                    .border_color(rgb(c.accent))
                    .child(div().child(match p.format {
                        // A file that says nothing of its app: tried as each.
                        Format::EncryptedFile => {
                            tr!("{file} is encrypted.", file = p.name.clone())
                        }
                        format => tr!(
                            "{file} is an encrypted {app} file.",
                            file = p.name.clone(),
                            app = format.name().to_string()
                        ),
                    }))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(div().flex_1().child(input_box(&self.import_pass)))
                            .child(primary_button("open-import", &tr!("Open")).on_click(
                                cx.listener(|view, _, _, cx| {
                                    // Kept until it opens: a wrong password is
                                    // typed again, not the file chosen again.
                                    let Some(p) = &view.pending else {
                                        return;
                                    };
                                    let (name, bytes) = (p.name.clone(), p.bytes.clone());
                                    let password = view.import_pass.read(cx).text().to_string();
                                    view.import(name, bytes, Some(password), cx);
                                }),
                            ))
                            .child(
                                text_button("cancel-import", &tr!("Cancel"), c.soft).on_click(
                                    cx.listener(|view, _, _, cx| {
                                        view.pending = None;
                                        cx.notify();
                                    }),
                                ),
                            ),
                    )
            });
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(tr!("A backup")))
            .child(
                card()
                    .child(hint(tr!(
                        "Every code, its group and its sites, sealed with a passphrase of its own: keep it somewhere else than this computer. Restoring it needs only the file and the passphrase."
                    )))
                    .child(
                        div()
                            .flex()
                            .gap(px(12.))
                            .child(div().flex_1().child(labeled(
                                tr!("Backup passphrase"),
                                &self.backup_pass,
                            )))
                            .child(div().flex_1().child(labeled(tr!("Again"), &self.backup_pass_again))),
                    )
                    .child(
                        div().flex().justify_end().child(
                            primary_button(
                                "save-backup",
                                &if busy { tr!("Working…") } else { tr!("Save a backup…") },
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                if !busy {
                                    view.save_backup(cx)
                                }
                            })),
                        ),
                    ),
            )
            .child(heading(tr!("Restore, or move from another app")))
            .child(
                card()
                    .child(hint(tr!(
                        "An authwing backup, another app's export, or a list of otpauth links. Codes already here are left as they are."
                    )))
                    .child(apps(tr!("Authenticators"), IMPORT_AUTHENTICATORS))
                    .child(apps(tr!("Password managers"), IMPORT_PASSWORD_MANAGERS))
                    .child(div().flex().child(
                        outline_button("import", &tr!("Choose a file…"))
                            .on_click(cx.listener(|view, _, _, cx| view.choose_import(cx))),
                    )),
            )
            .children(pending)
            .child(heading(tr!("Apps with no file to export")))
            .child(
                card()
                    .child(apps(
                        "Google Authenticator".into(),
                        &tr!("Transfer accounts › Export accounts shows QR codes: in Add a code, use the camera, or a screenshot, for many codes at once."),
                    ))
                    .child(apps(
                        "Microsoft Authenticator, Authy, Duo Mobile".into(),
                        &tr!("They keep their codes to themselves: at each site, set up two-step verification again, add the new QR code here, then remove it there."),
                    )),
            )
            .child(heading(tr!("Unencrypted")))
            .child(
                card().child(row(
                    tr!("Save as plain text"),
                    tr!("One otpauth link per code, which other apps import. Anyone who sees the file can make your codes."),
                    outline_button("save-plain", &tr!("Save…"))
                        .on_click(cx.listener(|view, _, window, cx| view.save_plain(window, cx))),
                )),
            )
    }

    fn browsers_page(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let mut list = card();
        let mut any_here = false;
        for e in self.browsers.iter().filter(|e| e.installed) {
            let (state, color) = match &e.found {
                Found::Here => {
                    any_here = true;
                    (tr!("Linked to this authwing"), c.good)
                }
                Found::Elsewhere(p) => (
                    tr!(
                        "Linked to another copy: {path}",
                        path = p.display().to_string()
                    ),
                    c.warn,
                ),
                Found::Missing => (tr!("Not linked"), c.muted),
            };
            list = list.child(row(
                e.browser.name().to_string(),
                e.place.manifest().display().to_string(),
                div().text_size(ui(12.)).text_color(rgb(color)).child(state),
            ));
        }
        if self.browsers.iter().all(|e| !e.installed) {
            list = list.child(hint(tr!("No browser the extension runs in was found.")));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(heading(tr!("The browser extension")))
            .child(card().child(hint(tr!(
                "With authwing's extension, the app knows the page in front exactly: a code saved with that site comes first and is typed in, and a page that only looks like it gets none. The extension keeps no codes."
            ))).child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        primary_button("link-browsers", &tr!("Link the browsers"))
                            .on_click(cx.listener(|view, _, _, cx| view.install_browsers(true, cx))),
                    )
                    .when(any_here, |d| {
                        d.child(
                            outline_button("unlink-browsers", &tr!("Unlink"))
                                .on_click(cx.listener(|view, _, _, cx| view.install_browsers(false, cx))),
                        )
                    }),
            ))
            .child(heading(tr!("Browsers on this computer")))
            .child(list)
            .child(hint(tr!(
                "The extension is not in the stores yet: build it from the repository's extension folder and load it unpacked."
            )))
    }
}

impl Render for SettingsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let header = div()
            .h(px(40.))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap(px(10.))
            .pl(px(14.))
            .pr(px(6.))
            .bg(rgb(c.surface))
            .border_b_1()
            .border_color(rgb(c.line))
            // The app icon's mark, as the title bar has it.
            .child(crate::mark::mark(px(16.), window.scale_factor(), cx))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr!("Settings")),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("settings-close")
                    .w(px(34.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(c.hover)))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Close)))
                    .child(
                        gpui::svg()
                            .path("icons/close.svg")
                            .size(px(14.))
                            .text_color(rgb(c.soft)),
                    ),
            );
        let mut nav = div()
            .w(px(200.))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(10.))
            .bg(rgb(c.panel))
            .border_r_1()
            .border_color(rgb(c.line));
        for page in Page::ALL {
            let on = page == self.page;
            nav = nav.child(
                div()
                    .id(SharedString::from(format!("page-{page:?}")))
                    .h(px(32.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(on, |d| {
                        d.bg(rgb(c.selected))
                            .text_color(rgb(c.text))
                            .border_l_2()
                            .border_color(rgb(c.accent))
                    })
                    .when(!on, |d| {
                        d.text_color(rgb(c.soft)).hover(|s| s.bg(rgb(c.hover)))
                    })
                    .on_click(cx.listener(move |view, _, _, cx| view.set_page(page, cx)))
                    .child(page.title()),
            );
        }
        let body = match self.page {
            Page::General => self.general(cx),
            Page::Voice => self.voice_page(cx),
            Page::Sync => self.sync_page(cx),
            Page::Security => self.security(cx),
            Page::Backup => self.backup_page(cx),
            Page::Browsers => self.browsers_page(cx),
            Page::About => self.about(window.scale_factor(), cx),
        };
        let page = div().flex_1().min_h_0().flex().child(nav).child(
            div()
                .id("settings-page")
                .flex_1()
                .h_full()
                .overflow_y_scroll()
                .child(
                    div()
                        .max_w(px(720.))
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .p(px(24.))
                        .child(
                            div()
                                .text_size(ui(18.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(self.page.title()),
                        )
                        .when_some(self.problem.clone(), |d, p| {
                            d.child(
                                crate::notice::colored(div(), crate::notice::Tone::Bad)
                                    .mt(px(8.))
                                    .px(px(12.))
                                    .py(px(8.))
                                    .rounded(px(6.))
                                    .text_size(ui(12.))
                                    .child(p),
                            )
                        })
                        .child(body),
                ),
        );
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(c.page))
            .text_color(rgb(c.text))
            .child(header)
            .child(page)
    }
}

/// Writes `bytes` to `path`; a plain export only its owner can read.
fn write_private(path: &std::path::Path, bytes: &[u8], plain: bool) -> std::io::Result<()> {
    std::fs::write(path, bytes)?;
    #[cfg(unix)]
    if plain {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = plain;
    Ok(())
}

/// Today as `2026-10-23`, for a backup's name.
fn today() -> String {
    let days = (crate::codes::now() / 86_400) as i64;
    // Howard Hinnant's days-to-civil, for the proleptic Gregorian calendar.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// What went wrong with a backup or an export, in the user's language.
fn backup_error(e: &authwing_backup::Error) -> String {
    use authwing_backup::Error;
    match e {
        Error::WrongPassword => tr!("That password does not open this file."),
        Error::Damaged(why) => tr!("The file is damaged: {why}", why = why.clone()),
        Error::Unknown => tr!("This is not a backup or an export authwing can read."),
        Error::WeakPassphrase(n) => tr!(
            "The passphrase is too short: at least {n} characters.",
            n = n.to_string()
        ),
        Error::PasswordNeeded(Format::EncryptedFile) => {
            tr!("This file is encrypted: enter its password.")
        }
        Error::PasswordNeeded(f) => tr!(
            "This {app} file is encrypted: enter its password.",
            app = f.name().to_string()
        ),
        // The app's or the format's own words, from the crate.
        Error::Unsupported(why) | Error::Seal(why) => why.clone(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn today_reads_as_a_date() {
        let t = super::today();
        assert_eq!(t.len(), 10, "{t}");
        assert!(t.starts_with("20"), "{t}");
        assert_eq!(&t[4..5], "-");
    }
}
