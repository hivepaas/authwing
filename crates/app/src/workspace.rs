//! The window: the codes in the vault, found by typing, filed by group, each
//! copied with a click or a key; the vault opened, locked and synced.

use std::time::{Duration, Instant};

use gpui::{
    AnyElement, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, FontWeight,
    KeyDownEvent, MouseButton, PromptLevel, Render, SharedString, Window, WindowControlArea,
    anchored, deferred, div, prelude::*, px, relative, rgb, svg,
};

use authwing_config::Hotkey;
use authwing_context::{Code, Context as Where, Page, Ranking, Reason, Warning};
use authwing_i18n::{tr, tr_n};
use authwing_totp::Kind;
use authwing_vault::{Lock, Request, Unlock};

use crate::add_code::{AddCode, AddEvent};
use crate::browser::{self, Bridge, Tab};
use crate::chrome;
use crate::codes::{self, Entry};
use crate::colors::colors;
use crate::group_editor::{GroupEditor, GroupEvent};
use crate::groups::{self, Group};
use crate::input::{Changed, TextInput, said};
use crate::look::{self, Live, ui};
use crate::notice::{self, Notice};
use crate::quick_panel::{self, Over, PanelEvent as QuickEvent, QuickPanel};
use crate::settings::{SettingsEvent, SettingsView};
use crate::sync_panel::{PanelEvent, SyncPanel};
use crate::syncing::{SyncEvent, SyncWorker};
use crate::vault_slot::VaultSlot;
use crate::widgets::{icon_button, outline_button, primary_button};

/// How long a notice stays.
mod listening;

/// Which codes the window shows, as chosen in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shown {
    All,
    Favorites,
    /// Those used on this device, the last used first.
    Recent,
    Group(String),
    /// Those in no group.
    NoGroup,
}

/// How many codes Recently used shows.
const RECENT: usize = 12;

const NOTICE: Duration = Duration::from_secs(6);
/// How long a card says "Copied".
const COPIED: Duration = Duration::from_secs(2);
/// Below this many seconds left, a code shows the next one beside it.
const SHOW_NEXT: u32 = 5;
const CARD_W: f32 = 260.;
const SIDEBAR_W: f32 = 220.;
/// Codes offered for the window used last, at most.
const FOR_WINDOW: usize = 3;
/// How long the window's size and place settle before they are saved.
const SAVE_BOUNDS: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq)]
enum SyncState {
    /// No place set in `[sync]`.
    Off,
    Syncing,
    /// When, and records in and out.
    Synced(Instant, usize, usize),
    Failed(String),
}

pub struct Workspace {
    live: Live,
    vault: VaultSlot,
    sync: Option<SyncWorker>,
    sync_state: SyncState,
    codes: Vec<Entry>,
    /// The codes were read since the vault opened.
    loaded: bool,
    filter: Entity<TextInput>,
    /// Which codes are shown.
    shown: Shown,
    /// The groups there are, with their colors.
    groups: Vec<Group>,
    group_editor: Option<Entity<GroupEditor>>,
    adding: Option<Entity<AddCode>>,
    panel: Option<Entity<SyncPanel>>,
    settings: Option<Entity<SettingsView>>,
    /// Devices asking to join, as the sync thread last saw them.
    requests: Vec<Request>,
    passphrase_joins: bool,
    /// The window of another app used last: its codes come first.
    elsewhere: Option<authwing_context::Window>,
    /// The quick panel, while it is open, and the window it is over.
    quick: Option<(
        gpui::WindowHandle<QuickPanel>,
        Option<authwing_context::window::Window>,
    )>,
    /// The quick panel's shortcut, while it is down.
    held: Option<listening::Held>,
    /// Presses of the shortcut, so that a hold's timer knows its own.
    presses: u64,
    /// The microphone records for the quick panel.
    listening: bool,
    /// What voice heard last in the quick panel.
    last_heard: Option<listening::LastHeard>,
    /// How the codes are used on this device: what voice has learnt, what
    /// was used when.
    usage: authwing_config::Usage,
    /// The browser extension, and the tab in front as it last said.
    _bridge: Bridge,
    tab: Option<Tab>,
    bounds_change: u64,
    /// The passphrase or recovery key typed to open a locked vault.
    unlock: Entity<TextInput>,
    unlocking: bool,
    notice: Option<(Notice, Instant)>,
    /// The card that was copied last, to say so for a moment.
    copied: Option<(String, Instant)>,
    /// The code put on the clipboard, cleared from it when due.
    clipboard: Option<(String, Instant)>,
    focus: FocusHandle,
    system_light: bool,
}

impl Workspace {
    pub fn new(live: Live, problems: Vec<String>, cx: &mut Context<Self>) -> Self {
        let vault = match live.store() {
            Some(store) => VaultSlot::open(store.dir().join("vault.db")),
            None => VaultSlot::none(),
        };
        let filter = said(cx, || tr!("Find a code"));
        cx.subscribe(&filter, |_, _, _: &Changed, cx| cx.notify())
            .detach();
        let unlock =
            cx.new(|cx| TextInput::said(|| tr!("Passphrase or recovery key"), cx).masked());
        let (events, received) = async_channel::unbounded();
        let sync = SyncWorker::start(vault.clone(), live.store().cloned(), events);
        sync.set_place(live.sync_place());
        let sync_state = if live.sync_place().is_some() {
            SyncState::Syncing
        } else {
            SyncState::Off
        };
        cx.spawn(async move |this, cx| {
            while let Ok(event) = received.recv().await {
                if this.update(cx, |ws, cx| ws.sync_event(event, cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        let (heard, said) = async_channel::unbounded();
        let usage = live.store().map(|s| s.usage()).unwrap_or_default();
        let bridge = Bridge::start(move |event| {
            let _ = heard.send_blocking(event);
        });
        cx.spawn(async move |this, cx| {
            while let Ok(event) = said.recv().await {
                if this
                    .update(cx, |ws, cx| ws.browser_event(event, cx))
                    .is_err()
                {
                    return;
                }
            }
        })
        .detach();
        // Once a second: the codes move on, the clipboard is cleared when due.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |ws, cx| ws.tick(cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        let mut ws = Workspace {
            live,
            vault,
            sync: Some(sync),
            sync_state,
            codes: vec![],
            loaded: false,
            filter,
            shown: Shown::All,
            groups: Vec::new(),
            group_editor: None,
            adding: None,
            panel: None,
            settings: None,
            requests: vec![],
            passphrase_joins: false,
            elsewhere: None,
            quick: None,
            held: None,
            presses: 0,
            listening: false,
            last_heard: None,
            usage,
            _bridge: bridge,
            tab: None,
            bounds_change: 0,
            unlock,
            unlocking: false,
            notice: None,
            copied: None,
            clipboard: None,
            focus: cx.focus_handle(),
            system_light: false,
        };
        if let Some(p) = problems.into_iter().next() {
            ws.say(Notice::warn(p));
        }
        cx.set_global(crate::hotkeys::Bound(ws.live.keymap()));
        ws
    }

    /// Follows the system's light or dark appearance, which the app takes
    /// with `mode = "system"`.
    pub fn track_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.system_light = Self::looks_light(window);
        self.restyle();
        cx.observe_window_appearance(window, |ws, window, cx| {
            ws.system_light = Self::looks_light(window);
            ws.restyle();
            cx.notify();
        })
        .detach();
    }

    /// Remembers the window's size and place for the next start, once they
    /// settle.
    pub fn track_bounds(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.observe_window_bounds(window, |ws, window, cx| {
            ws.bounds_change += 1;
            let change = ws.bounds_change;
            let state = crate::placement::state(window.window_bounds());
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(SAVE_BOUNDS).await;
                let _ = this.update(cx, |ws, _| {
                    if ws.bounds_change == change
                        && let Some(store) = ws.live.store()
                    {
                        let _ = store.save_window_state(&state);
                    }
                });
            })
            .detach();
        })
        .detach();
    }

    fn looks_light(window: &Window) -> bool {
        matches!(
            window.appearance(),
            gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight
        )
    }

    fn restyle(&mut self) {
        authwing_i18n::set_language(self.live.settings().general.language.as_deref());
        crate::colors::set_light(self.live.light_for(self.system_light));
    }

    /// Edits to config.toml apply while running.
    pub fn poll_config(&mut self, cx: &mut Context<Self>) {
        let Some(problems) = self.live.reload_if_changed() else {
            return;
        };
        self.restyle();
        self.apply_panel_key(cx);
        if cfg!(target_os = "macos") {
            crate::menu::set_menus(cx);
        }
        cx.set_global(crate::hotkeys::Bound(self.live.keymap()));
        if let Some(sync) = &self.sync {
            let place = self.live.sync_place();
            if place.is_none() {
                self.sync_state = SyncState::Off;
            }
            sync.set_place(place);
        }
        if let Some(p) = problems.into_iter().next() {
            self.say(Notice::warn(p));
        }
        cx.notify();
    }

    /// Registers the quick panel's shortcut as config.toml has it.
    pub fn apply_panel_key(&mut self, cx: &mut Context<Self>) {
        if !cx.has_global::<crate::panel_key::PanelKey>() {
            return;
        }
        let key = self.live.settings().panel.key.clone();
        let set = cx.update_global(|keys: &mut crate::panel_key::PanelKey, _| keys.set(&key));
        if let Err(e) = set {
            self.say(Notice::warn(tr!(
                "The quick panel's shortcut {key} cannot be used: {e}",
                key = key,
                e = e
            )));
        }
    }

    fn say(&mut self, notice: Notice) {
        self.notice = Some((notice, Instant::now()));
    }

    fn tick(&mut self, cx: &mut Context<Self>) {
        if !self.loaded && self.vault.ready().is_some() {
            self.reload();
        }
        // Another app's window: authwing's own is where the user looks the
        // codes up, not where they go.
        if let Some(w) = authwing_context::window::active()
            && w.pid != Some(std::process::id())
        {
            self.elsewhere = Some(w);
        }
        if self
            .notice
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= NOTICE)
        {
            self.notice = None;
        }
        if self
            .copied
            .as_ref()
            .is_some_and(|(_, at)| at.elapsed() >= COPIED)
        {
            self.copied = None;
        }
        let keep = u64::from(self.live.settings().codes.clear_clipboard);
        if keep > 0
            && let Some((code, at)) = &self.clipboard
            && at.elapsed() >= Duration::from_secs(keep)
        {
            // Only if it still holds the code: something copied since stays.
            if cx.read_from_clipboard().and_then(|c| c.text()).as_deref() == Some(code.as_str()) {
                cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
            }
            self.clipboard = None;
        }
        cx.notify();
    }

    /// Reads the codes from the vault again.
    fn reload(&mut self) {
        let Some(vault) = self.vault.ready() else {
            return;
        };
        let (codes, problems) = codes::load(&vault);
        self.groups = groups::load(&vault, &codes);
        self.codes = codes;
        self.loaded = true;
        if let Some(p) = problems.into_iter().next() {
            self.say(Notice::warn(p));
        }
        // A group deleted or renamed elsewhere: every code instead.
        if let Shown::Group(name) = &self.shown
            && !self
                .groups
                .iter()
                .any(|g| g.name.eq_ignore_ascii_case(name))
        {
            self.shown = Shown::All;
        }
    }

    fn sync_event(&mut self, event: SyncEvent, cx: &mut Context<Self>) {
        match event {
            SyncEvent::Started => {
                if self.sync_state == SyncState::Off {
                    return;
                }
                self.sync_state = SyncState::Syncing;
            }
            SyncEvent::Done(r) => {
                self.sync_state = SyncState::Synced(Instant::now(), r.pulled, r.pushed);
                self.reload();
            }
            SyncEvent::Failed(e) => self.sync_state = SyncState::Failed(e),
            SyncEvent::Asking(state) => {
                if let Some(p) = &self.panel {
                    p.update(cx, |p, cx| p.set_asking(state, cx));
                }
            }
            SyncEvent::Requests(requests) => {
                if self.panel.is_none() && requests.len() > self.requests.len() {
                    self.say(Notice::warn(tr!(
                        "A device asks to join: open Sync (the cloud) to approve it."
                    )));
                }
                if let Some(p) = &self.panel {
                    let r = requests.clone();
                    p.update(cx, |p, cx| p.set_requests(r, cx));
                }
                self.requests = requests;
            }
            SyncEvent::PairingFailed(e) => match &self.panel {
                Some(p) => p.update(cx, |p, cx| p.failed(e, cx)),
                None => self.say(Notice::bad(e)),
            },
            SyncEvent::PassphraseJoins(joins) => {
                self.passphrase_joins = joins;
                if let Some(p) = &self.panel {
                    p.update(cx, |p, cx| p.set_passphrase_joins(joins, cx));
                }
            }
        }
        if let Some(s) = &self.settings {
            let status = self.sync_status();
            s.update(cx, |s, cx| s.set_sync_status(status, cx));
        }
        if let Some(p) = &self.panel {
            let (status, synced) = (self.sync_status(), self.synced());
            p.update(cx, |p, cx| {
                p.set_status(status, cx);
                p.set_synced(synced, cx);
            });
        }
        cx.notify();
    }

    fn browser_event(&mut self, event: authwing_bridge::listener::Event, cx: &mut Context<Self>) {
        let (tab, filled) = browser::tab_after(self.tab.take(), event);
        self.tab = tab;
        if let Some(f) = filled {
            if f.ok {
                self.say(Notice::good(tr!("Typed into the page.")));
            } else {
                self.say(Notice::warn(tr!(
                    "Not typed into the page: {why}",
                    why = f.reason.unwrap_or_default()
                )));
            }
        }
        cx.notify();
    }

    /// The page in the window used last, when that window is a browser
    /// with the extension.
    fn page(&self) -> Option<&Tab> {
        let window = self.elsewhere.as_ref()?;
        browser::is_browser(&window.app_id)
            .then_some(self.tab.as_ref())
            .flatten()
    }

    /// The last sync went through: this device syncs the vault there.
    fn synced(&self) -> bool {
        matches!(self.sync_state, SyncState::Synced(..))
    }

    /// How syncing goes, in a line.
    fn sync_status(&self) -> String {
        match &self.sync_state {
            SyncState::Off => tr!("Not syncing"),
            SyncState::Syncing => tr!("Syncing…"),
            SyncState::Synced(at, pulled, pushed) => {
                let ago = at.elapsed().as_secs() / 60;
                let when = if ago == 0 {
                    tr!("Synced just now")
                } else {
                    tr_n!(
                        ago as usize,
                        "Synced {n} minute ago",
                        "Synced {n} minutes ago"
                    )
                };
                if pulled + pushed == 0 {
                    when
                } else {
                    tr!(
                        "{when}: {pulled} in, {pushed} out",
                        when = when,
                        pulled = pulled.to_string(),
                        pushed = pushed.to_string()
                    )
                }
            }
            SyncState::Failed(e) => tr!("Sync failed: {e}", e = e.clone()),
        }
    }

    fn open_sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(sync) = self.sync.clone() else {
            return;
        };
        let panel = cx.new(|cx| SyncPanel::new(sync, self.vault.clone(), cx));
        let (status, requests, joins, synced) = (
            self.sync_status(),
            self.requests.clone(),
            self.passphrase_joins,
            self.synced(),
        );
        panel.update(cx, |p, cx| {
            p.set_status(status, cx);
            p.set_synced(synced, cx);
            p.set_requests(requests, cx);
            p.set_passphrase_joins(joins, cx);
        });
        cx.subscribe_in(&panel, window, |ws, _, event: &PanelEvent, window, cx| {
            if let PanelEvent::Joined = event {
                ws.reload();
                ws.say(Notice::good(tr!(
                    "Joined: this device syncs the vault now."
                )));
            }
            ws.panel = None;
            window.focus(&ws.focus, cx);
            cx.notify();
        })
        .detach();
        window.focus(&panel.focus_handle(cx), cx);
        self.panel = Some(panel);
        cx.notify();
    }

    /// The codes shown: those the sidebar chose, matching what is typed.
    fn shown(&self, cx: &App) -> Vec<&Entry> {
        let words: Vec<String> = self
            .filter
            .read(cx)
            .text()
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let last = |c: &Entry| self.usage.codes.get(&c.id).map_or(0, |u| u.last);
        let mut shown: Vec<&Entry> = self
            .codes
            .iter()
            .filter(|c| match &self.shown {
                Shown::All => true,
                Shown::Favorites => c.favorite,
                Shown::Recent => last(c) > 0,
                Shown::Group(name) => groups::holds(name, c),
                Shown::NoGroup => c.group.trim().is_empty(),
            })
            .filter(|c| c.matches(&words))
            .collect();
        if self.shown == Shown::Recent {
            shown.sort_by_key(|c| std::cmp::Reverse(last(c)));
            shown.truncate(RECENT);
        }
        shown
    }

    fn show(&mut self, shown: Shown, cx: &mut Context<Self>) {
        self.shown = shown;
        cx.notify();
    }

    /// The groups' names and colors, to offer them.
    fn group_choices(&self) -> Vec<(String, u32)> {
        self.groups
            .iter()
            .map(|g| (g.name.clone(), g.rgb()))
            .collect()
    }

    /// Stars the code `id`, or unstars it.
    fn toggle_favorite(&mut self, id: &str, cx: &mut Context<Self>) {
        let (Some(vault), Some(entry)) =
            (self.vault.ready(), self.codes.iter().find(|c| c.id == id))
        else {
            return;
        };
        let filing = codes::Filing {
            favorite: !entry.favorite,
            ..entry.filing()
        };
        if let Err(e) = codes::save(&vault, Some(id), &entry.otp, &filing) {
            self.say(Notice::bad(tr!("Not saved: {e}", e = e)));
        }
        self.reload();
        self.sync_now();
        cx.notify();
    }

    /// Opens the group editor, for `group` or a new one.
    fn open_group_editor(
        &mut self,
        group: Option<Group>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.vault.ready().is_none() {
            return;
        }
        let all = self.groups.clone();
        let editor = cx.new(|cx| GroupEditor::new(group, &all, cx));
        cx.subscribe_in(&editor, window, |ws, _, event: &GroupEvent, window, cx| {
            ws.group_event(event, cx);
            ws.group_editor = None;
            window.focus(&ws.focus, cx);
            cx.notify();
        })
        .detach();
        let name = editor.read(cx).name_focus(cx);
        window.focus(&name, cx);
        self.group_editor = Some(editor);
        cx.notify();
    }

    fn group_event(&mut self, event: &GroupEvent, cx: &mut Context<Self>) {
        let Some(vault) = self.vault.ready() else {
            return;
        };
        let done = match event {
            GroupEvent::Close => return,
            GroupEvent::Save { old, name, color } => {
                let saved = groups::save(&vault, &self.codes, old.as_ref(), name, color);
                if saved.is_ok() {
                    // The group shown, renamed: still shown.
                    let was_shown = matches!((&self.shown, old), (Shown::Group(s), Some(o)) if s.eq_ignore_ascii_case(&o.name));
                    if was_shown || old.is_none() {
                        self.shown = Shown::Group(name.clone());
                    }
                }
                saved
            }
            GroupEvent::Delete(group) => groups::delete(&vault, &self.codes, group),
        };
        if let Err(e) = done {
            self.say(Notice::bad(tr!("Not saved: {e}", e = e)));
        }
        self.reload();
        self.sync_now();
        cx.notify();
    }

    /// Opens the dialog that edits the code `id`.
    fn open_editing(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.codes.iter().find(|c| c.id == id) else {
            return;
        };
        let choices = self.group_choices();
        let edit = cx.new(|cx| AddCode::edit(entry, choices, cx));
        self.show_adding(edit, window, cx);
    }

    /// Saves what the edit dialog says of the code `id`.
    fn edit_code(&mut self, id: &str, issuer: &str, account: &str, group: &str, sites: &[String]) {
        let (Some(vault), Some(entry)) =
            (self.vault.ready(), self.codes.iter().find(|c| c.id == id))
        else {
            return;
        };
        let mut otp = entry.otp.clone();
        otp.issuer = issuer.to_string();
        otp.account = account.to_string();
        let filing = codes::Filing {
            group: group.to_string(),
            sites: sites.to_vec(),
            favorite: entry.favorite,
        };
        match codes::save(&vault, Some(id), &otp, &filing) {
            Ok(_) => self.say(Notice::good(tr!("Saved."))),
            Err(e) => self.say(Notice::bad(tr!("Not saved: {e}", e = e))),
        }
        self.reload();
        self.sync_now();
    }

    /// The code `id` shows now, for use: an HOTP code moves on to its next,
    /// as using one does.
    fn take_code(&mut self, id: &str) -> Option<String> {
        let entry = self.codes.iter().find(|c| c.id == id)?;
        let code = entry.otp.code_at(codes::now());
        if let Kind::Hotp { counter } = entry.otp.kind {
            let mut next = entry.otp.clone();
            next.kind = Kind::Hotp {
                counter: counter.saturating_add(1),
            };
            let filing = entry.filing();
            if let Some(vault) = self.vault.ready() {
                if let Err(e) = codes::save(&vault, Some(id), &next, &filing) {
                    self.say(Notice::bad(tr!("The counter was not saved: {e}", e = e)));
                }
                self.reload();
                self.sync_now();
            }
        }
        self.used(id);
        Some(code)
    }

    /// Puts the code `id` shows now on the clipboard, and types it into the
    /// page in front when that is the code's site and has a field for it.
    fn copy(&mut self, id: &str, cx: &mut Context<Self>) {
        let verified = self
            .for_window()
            .1
            .iter()
            .any(|(e, why)| e.id == id && why.verified());
        let Some(code) = self.take_code(id) else {
            return;
        };
        let typed = verified
            .then(|| self.page().filter(|t| t.has_code_field))
            .flatten()
            .map(|tab| browser::fill(tab, &code));
        if let Some(Err(e)) = typed {
            self.say(Notice::warn(tr!(
                "Not typed into the page: {why}",
                why = e.to_string()
            )));
        }
        cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
        self.clipboard = Some((code, Instant::now()));
        self.copied = Some((id.to_string(), Instant::now()));
        cx.notify();
    }

    /// Opens the quick panel over the app in front, or closes it if open.
    pub fn toggle_quick(&mut self, cx: &mut Context<Self>) {
        if self.quick.is_some() {
            return self.close_quick(false, cx);
        }
        if self.vault.ready().is_none() {
            // Locked: the window asks for the passphrase.
            cx.activate(true);
            return;
        }
        // Before authwing comes to the front: the app the user is in.
        let own = Some(std::process::id());
        let target = authwing_context::window::active().filter(|w| w.pid != own);
        let page = target
            .as_ref()
            .filter(|w| browser::is_browser(&w.app_id))
            .and(self.tab.as_ref());
        let (matched, warning) = match &target {
            Some(w) => self.rank_for(w, page, 5),
            None => (vec![], None),
        };
        let item = |e: &Entry, why: Option<Reason>| quick_panel::Item {
            id: e.id.clone(),
            title: e.title(),
            detail: e.detail().to_string(),
            initials: e.initials(),
            otp: e.otp.clone(),
            why: why.map(reason_label),
            search: format!(
                "{} {} {} {}",
                e.otp.issuer,
                e.otp.account,
                e.group,
                e.sites.join(" ")
            )
            .to_lowercase(),
        };
        let matched_items: Vec<quick_panel::Item> =
            matched.iter().map(|(e, why)| item(e, Some(*why))).collect();
        let others: Vec<quick_panel::Item> = self
            .codes
            .iter()
            .filter(|e| !matched.iter().any(|(m, _)| m.id == e.id))
            .map(|e| item(e, None))
            .collect();
        let over = Over {
            place: target.as_ref().map(|w| match &w.title {
                Some(t) if !t.is_empty() => format!("{} · {t}", w.app_name),
                _ => w.app_name.clone(),
            }),
            app: target.as_ref().map(|w| w.app_name.clone()),
            look_alike: warning
                .as_ref()
                .map(|w| (w.page.clone(), w.like.to_string())),
            can_type: target.is_some() && warning.is_none() && crate::typing::allowed(),
            voice_hint: crate::voice::ready_model(self.live.settings()).map(|_| {
                tr!(
                    "hold {key} and say a name",
                    key = crate::panel_key::label(&self.live.settings().panel.key)
                )
            }),
        };
        let bounds = gpui::Bounds::centered(None, gpui::size(px(560.), px(440.)), cx);
        let opened = cx.open_window(
            gpui::WindowOptions {
                window_bounds: Some(gpui::WindowBounds::Windowed(bounds)),
                titlebar: None,
                kind: gpui::WindowKind::PopUp,
                focus: true,
                show: true,
                is_movable: false,
                is_resizable: false,
                is_minimizable: false,
                ..Default::default()
            },
            |window, cx| cx.new(|cx| QuickPanel::new(over, matched_items, others, window, cx)),
        );
        let Ok(handle) = opened else {
            return;
        };
        if let Ok(panel) = handle.update(cx, |_, window, cx| {
            window.activate_window();
            cx.entity()
        }) {
            cx.subscribe(&panel, |ws, _, event: &QuickEvent, cx| match event {
                QuickEvent::Close => ws.close_quick(true, cx),
                QuickEvent::Copy(id) => {
                    ws.copy(id, cx);
                    ws.close_quick(true, cx);
                }
                QuickEvent::Type(id) => {
                    let id = id.clone();
                    ws.type_code(&id, cx);
                }
                QuickEvent::Listen => ws.toggle_listening(cx),
            })
            .detach();
        }
        cx.activate(true);
        self.quick = Some((handle, target));
    }

    /// Closes the quick panel; `back` gives the keyboard back to the window
    /// it was over, leaving authwing's own window where it was.
    fn close_quick(&mut self, back: bool, cx: &mut Context<Self>) {
        self.cancel_listening(cx);
        let Some((handle, over)) = self.quick.take() else {
            return;
        };
        // While authwing still has the keyboard, which the system asks.
        let returned = back && over.is_some_and(|w| authwing_context::window::bring_back(&w));
        let _ = handle.update(cx, |_, window, _| window.remove_window());
        if back && !returned && cfg!(target_os = "macos") {
            // To whichever app was in front before, as menus do.
            cx.hide();
        }
    }

    /// Types the code `id` into the app the quick panel was over, once it
    /// is in front again.
    fn type_code(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(code) = self.take_code(id) else {
            return;
        };
        self.close_quick(true, cx);
        cx.spawn(async move |this, cx| {
            // The other app takes the keyboard back first.
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            let typed = cx
                .background_spawn(async move { crate::typing::type_text(&code) })
                .await;
            if let Err(e) = typed {
                let _ = this.update(cx, |ws, cx| {
                    ws.say(Notice::bad(tr!("Not typed in: {e}", e = e)));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn sync_now(&self) {
        if let Some(s) = &self.sync {
            s.now();
        }
    }

    fn open_adding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.vault.ready().is_none() {
            return;
        }
        let group = match &self.shown {
            Shown::Group(name) => Some(name.clone()),
            _ => None,
        };
        let choices = self.group_choices();
        let add = cx.new(|cx| AddCode::new(group.as_deref(), choices, cx));
        self.show_adding(add, window, cx);
    }

    /// Adding, then reading the screen at once: the sidebar's scan button.
    fn open_scanning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_adding(window, cx);
        if let Some(add) = &self.adding {
            add.update(cx, |add, cx| add.scan_screen(cx));
        }
    }

    /// Shows the add (or edit) dialog over the window.
    fn show_adding(&mut self, add: Entity<AddCode>, window: &mut Window, cx: &mut Context<Self>) {
        cx.subscribe_in(&add, window, |ws, _, event: &AddEvent, window, cx| {
            match event {
                AddEvent::Close => {}
                AddEvent::Add {
                    codes,
                    group,
                    sites,
                } => ws.add(codes, group, sites),
                AddEvent::Edit {
                    id,
                    issuer,
                    account,
                    group,
                    sites,
                } => ws.edit_code(id, issuer, account, group, sites),
            }
            ws.adding = None;
            window.focus(&ws.focus, cx);
            cx.notify();
        })
        .detach();
        let first = add.read(cx).first_focus(cx);
        window.focus(&first, cx);
        self.adding = Some(add);
        cx.notify();
    }

    fn add(&mut self, new: &[authwing_totp::Otp], group: &str, sites: &[String]) {
        let Some(vault) = self.vault.ready() else {
            return;
        };
        let mut saved = 0;
        let filing = codes::Filing {
            group: group.to_string(),
            sites: sites.to_vec(),
            favorite: false,
        };
        for otp in new {
            match codes::save(&vault, None, otp, &filing) {
                Ok(_) => saved += 1,
                Err(e) => self.say(Notice::bad(tr!("A code was not saved: {e}", e = e))),
            }
        }
        self.reload();
        self.sync_now();
        if saved > 0 {
            self.say(Notice::good(tr_n!(
                saved,
                "{n} code added.",
                "{n} codes added."
            )));
        }
    }

    fn delete(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.codes.iter().find(|c| c.id == id) else {
            return;
        };
        let title = entry.title();
        let question = tr!("Delete the code for {name}?", name = title.clone());
        let detail = tr!(
            "Turn two-factor sign-in off at {name} first, or sign in there with a recovery code: without this code you cannot sign in. Your other devices lose it too.",
            name = title
        );
        let (yes, no) = (tr!("Delete"), tr!("Cancel"));
        let answer = window.prompt(
            PromptLevel::Warning,
            &question,
            Some(&detail),
            &[yes.as_str(), no.as_str()],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if !matches!(answer.await, Ok(0)) {
                return;
            }
            let _ = this.update(cx, |ws, cx| {
                if let Some(vault) = ws.vault.ready()
                    && let Err(e) = vault.delete(&id)
                {
                    ws.say(Notice::bad(tr!("Not deleted: {e}", e = e.to_string())));
                }
                ws.reload();
                ws.sync_now();
                cx.notify();
            });
        })
        .detach();
    }

    fn lock(&mut self, cx: &mut Context<Self>) {
        if self.vault.lock() {
            self.codes.clear();
            self.loaded = false;
        } else {
            self.say(Notice::warn(tr!(
                "The vault's key is in this computer's keychain, so it cannot lock. Set a passphrase and take the key out of the keychain to lock it."
            )));
        }
        cx.notify();
    }

    fn try_unlock(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let typed = self.unlock.read(cx).text().trim().to_string();
        if typed.is_empty() || self.unlocking {
            return;
        }
        self.unlocking = true;
        let slot = self.vault.clone();
        // Argon2 takes half a second: off the window's thread.
        let opened = cx.background_spawn(async move {
            let by_passphrase = slot.unlock(Unlock::Passphrase(&typed));
            match by_passphrase {
                Ok(_) => Ok(()),
                Err(e) => slot
                    .unlock(Unlock::RecoveryKey(&typed))
                    .map(|_| ())
                    .map_err(|_| e.to_string()),
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = opened.await;
            let _ = this.update_in(cx, |ws, window, cx| {
                ws.unlocking = false;
                match result {
                    Ok(()) => {
                        ws.unlock.update(cx, |f, cx| f.set_text("", 0..0, cx));
                        ws.reload();
                        window.focus(&ws.focus, cx);
                    }
                    Err(e) => ws.say(Notice::bad(tr!("Not opened: {e}", e = e))),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens the locked vault with Touch ID or Windows Hello: the system
    /// asks, off the window's thread.
    fn unlock_with_presence(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.unlocking {
            return;
        }
        self.unlocking = true;
        let slot = self.vault.clone();
        let opened = cx.background_spawn(async move {
            slot.unlock_with_presence()
                .map(|_| ())
                .map_err(|e| e.to_string())
        });
        cx.spawn_in(window, async move |this, cx| {
            let result = opened.await;
            let _ = this.update_in(cx, |ws, window, cx| {
                ws.unlocking = false;
                match result {
                    Ok(()) => {
                        ws.reload();
                        window.focus(&ws.focus, cx);
                    }
                    Err(e) => ws.say(Notice::bad(tr!("Not opened: {e}", e = e))),
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Opens Settings over the window, or closes it.
    fn toggle_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
            return;
        }
        let Some(sync) = self.sync.clone() else {
            return;
        };
        let store = self.live.store().cloned();
        let view = cx.new(|cx| SettingsView::new(store, self.vault.clone(), sync, cx));
        let status = self.sync_status();
        view.update(cx, |v, cx| v.set_sync_status(status, cx));
        cx.subscribe_in(&view, window, |ws, _, event: &SettingsEvent, window, cx| {
            match event {
                SettingsEvent::Close => {
                    ws.settings = None;
                    window.focus(&ws.focus, cx);
                }
                SettingsEvent::Changed => ws.poll_config(cx),
                SettingsEvent::Say(n) => ws.say(n.clone()),
                SettingsEvent::CodesChanged => {
                    ws.reload();
                    ws.sync_now();
                }
                SettingsEvent::Lock => {
                    ws.lock(cx);
                    if ws.vault.ready().is_none() {
                        ws.settings = None;
                    }
                }
            }
            cx.notify();
        })
        .detach();
        window.focus(&view.focus_handle(cx), cx);
        self.adding = None;
        self.panel = None;
        self.settings = Some(view);
        cx.notify();
    }

    /// The window's shortcuts, before the field with the focus sees the key.
    fn shortcut(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.adding.is_some() || self.panel.is_some() || self.group_editor.is_some() {
            return;
        }
        let Some(chord) = crate::hotkeys::pressed(&ev.keystroke) else {
            return;
        };
        let Some((hotkey, place)) = self.live.keymap().find(&chord) else {
            return;
        };
        // Over Settings, only Settings (to close it) and Quit.
        if self.settings.is_some() && !matches!(hotkey, Hotkey::Settings | Hotkey::Quit) {
            return;
        }
        match hotkey {
            Hotkey::Find => window.focus(&self.filter.focus_handle(cx), cx),
            Hotkey::AddCode => self.open_adding(window, cx),
            Hotkey::CopyCode => {
                let id = place.and_then(|n| self.shown(cx).get(n).map(|c| c.id.clone()));
                if let Some(id) = id {
                    self.copy(&id, cx);
                }
            }
            Hotkey::Lock => self.lock(cx),
            Hotkey::Settings => self.toggle_settings(window, cx),
            Hotkey::Quit => self.quit(cx),
        }
        cx.stop_propagation();
    }

    /// Quits, the last edits sent first if a sync is quick.
    pub fn quit(&mut self, cx: &mut Context<Self>) {
        if let Some(s) = &self.sync
            && s.place().is_some()
        {
            s.sync_and_wait(Duration::from_secs(3));
        }
        cx.quit()
    }

    /// Keys in the find field: Enter copies the first code shown, Escape
    /// empties the field.
    fn filter_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "enter" => {
                if let Some(id) = self.shown(cx).first().map(|c| c.id.clone()) {
                    self.copy(&id, cx);
                }
            }
            "escape" => self.filter.update(cx, |f, cx| f.set_text("", 0..0, cx)),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// `scale`: the window's device pixels a point.
    fn title_bar(&self, scale: f32, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let cloud = match &self.sync_state {
            SyncState::Off => c.sync_off,
            SyncState::Syncing | SyncState::Synced(..) => c.sync_ok,
            SyncState::Failed(_) => c.sync_bad,
        };
        let cloud_tip = self.sync_status();
        let open = self.vault.ready().is_some();
        // A dialog covers the bar: a tooltip left from the click that opened
        // it would show over the dialog's shade.
        let tips = self.adding.is_none() && self.panel.is_none();
        div()
            .size_full()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .window_control_area(WindowControlArea::Drag)
                    // The app icon's mark, in its colors; no name beside it.
                    .child(crate::mark::mark(px(20.), scale, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .when(open, |d| {
                d.child(
                    div()
                        .id("find")
                        .w(px(340.))
                        .h(px(28.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .rounded(px(6.))
                        .bg(rgb(c.field))
                        .border_1()
                        .border_color(rgb(c.field_edge))
                        .on_key_down(cx.listener(Self::filter_key))
                        .child(
                            svg()
                                .path("icons/search.svg")
                                .size(px(14.))
                                .text_color(rgb(c.muted)),
                        )
                        .child(div().flex_1().child(self.filter.clone()))
                        .child(
                            div()
                                .text_size(ui(11.))
                                .text_color(rgb(c.faint))
                                .child(crate::hotkeys::hint(cx, Hotkey::Find)),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                icon_button("sync", "icons/cloud.svg", cloud)
                    .when(tips, |b| b.tooltip(move |_, cx| tip(cloud_tip.clone(), cx)))
                    .on_click(cx.listener(|ws, _, window, cx| ws.open_sync(window, cx))),
            )
            .when(open, |d| {
                d.child(
                    icon_button("add", "icons/plus.svg", c.soft)
                        .when(tips, |b| b.tooltip(|_, cx| tip(tr!("Add a code"), cx)))
                        .on_click(cx.listener(|ws, _, window, cx| ws.open_adding(window, cx))),
                )
                .child(
                    icon_button("lock", "icons/lock.svg", c.soft)
                        .when(tips, |b| b.tooltip(|_, cx| tip(tr!("Lock"), cx)))
                        .on_click(cx.listener(|ws, _, _, cx| ws.lock(cx))),
                )
            })
            .child(
                icon_button("settings", "icons/settings.svg", c.soft)
                    .when(tips, |b| b.tooltip(|_, cx| tip(tr!("Settings"), cx)))
                    .on_click(cx.listener(|ws, _, window, cx| ws.toggle_settings(window, cx))),
            )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        // A row of the list: its mark (an icon, a group's color), its name,
        // how many codes it has.
        let row =
            |id: SharedString, mark: AnyElement, label: String, count: Option<usize>, on: bool| {
                div()
                    .id(id)
                    .group("side-row")
                    .h(px(30.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(9.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(on, |d| d.bg(rgb(c.selected)).text_color(rgb(c.text)))
                    .when(!on, |d| {
                        d.text_color(rgb(c.soft)).hover(|s| s.bg(rgb(c.hover)))
                    })
                    .child(mark)
                    .child(div().flex_1().min_w_0().truncate().child(label))
                    .when_some(count, |d, n| {
                        d.child(
                            div()
                                .flex_shrink_0()
                                .text_size(ui(11.))
                                .text_color(rgb(c.muted))
                                .child(n.to_string()),
                        )
                    })
            };
        let icon = |path: &'static str| {
            svg()
                .path(path)
                .size(px(14.))
                .flex_shrink_0()
                .text_color(rgb(c.muted))
                .into_any_element()
        };
        let dot = |color: u32| {
            div()
                .size(px(8.))
                .flex_shrink_0()
                .rounded(px(2.))
                .bg(rgb(color))
                .into_any_element()
        };
        let section = |title: String| {
            div()
                .pt(px(14.))
                .pb(px(4.))
                .px(px(10.))
                .flex()
                .items_center()
                .text_size(ui(11.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(c.muted))
                .child(div().flex_1().child(title.to_uppercase()))
        };
        let favorites = self.codes.iter().filter(|e| e.favorite).count();
        let mut list = div()
            .id("sidebar-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap(px(1.))
            .p(px(8.))
            .child(
                row(
                    "show-all".into(),
                    icon("icons/grid.svg"),
                    tr!("All codes"),
                    Some(self.codes.len()),
                    self.shown == Shown::All,
                )
                .on_click(cx.listener(|ws, _, _, cx| ws.show(Shown::All, cx))),
            )
            .child(
                row(
                    "show-favorites".into(),
                    icon("icons/star.svg"),
                    tr!("Favorites"),
                    (favorites > 0).then_some(favorites),
                    self.shown == Shown::Favorites,
                )
                .on_click(cx.listener(|ws, _, _, cx| ws.show(Shown::Favorites, cx))),
            )
            .child(
                row(
                    "show-recent".into(),
                    icon("icons/clock.svg"),
                    tr!("Recently used"),
                    None,
                    self.shown == Shown::Recent,
                )
                .on_click(cx.listener(|ws, _, _, cx| ws.show(Shown::Recent, cx))),
            )
            .child(
                section(tr!("Groups")).child(
                    div()
                        .id("new-group")
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.))
                        .cursor_pointer()
                        .hover(|s| s.bg(rgb(c.hover)))
                        .on_click(
                            cx.listener(|ws, _, window, cx| ws.open_group_editor(None, window, cx)),
                        )
                        .child(
                            svg()
                                .path("icons/plus.svg")
                                .size(px(12.))
                                .text_color(rgb(c.muted)),
                        ),
                ),
            );
        if self.groups.is_empty() {
            list = list.child(
                div()
                    .px(px(10.))
                    .py(px(4.))
                    .text_size(ui(12.))
                    .text_color(rgb(c.faint))
                    .child(tr!("Groups file codes: Work, Money, Family…")),
            );
        }
        for (n, group) in self.groups.iter().enumerate() {
            let on = matches!(&self.shown, Shown::Group(g) if g.eq_ignore_ascii_case(&group.name));
            let (pick, edit) = (group.name.clone(), group.clone());
            list = list.child(
                row(
                    SharedString::from(format!("group-{n}")),
                    dot(group.rgb()),
                    group.name.clone(),
                    Some(group.count),
                    on,
                )
                .on_click(cx.listener(move |ws, _, _, cx| ws.show(Shown::Group(pick.clone()), cx)))
                .child(
                    div()
                        .id(SharedString::from(format!("edit-group-{n}")))
                        .invisible()
                        .group_hover("side-row", |s| s.visible())
                        .size(px(20.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded(px(5.))
                        .hover(|s| s.bg(rgb(c.hover)))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(cx.listener(move |ws, _, window, cx| {
                            cx.stop_propagation();
                            ws.open_group_editor(Some(edit.clone()), window, cx);
                        }))
                        .child(
                            svg()
                                .path("icons/pencil.svg")
                                .size(px(12.))
                                .text_color(rgb(c.muted)),
                        ),
                ),
            );
        }
        let ungrouped = self
            .codes
            .iter()
            .filter(|e| e.group.trim().is_empty())
            .count();
        if !self.groups.is_empty() && ungrouped > 0 {
            list = list.child(
                row(
                    "show-no-group".into(),
                    div()
                        .size(px(8.))
                        .flex_shrink_0()
                        .rounded(px(2.))
                        .border_1()
                        .border_color(rgb(c.muted))
                        .into_any_element(),
                    tr!("No group"),
                    Some(ungrouped),
                    self.shown == Shown::NoGroup,
                )
                .on_click(cx.listener(|ws, _, _, cx| ws.show(Shown::NoGroup, cx))),
            );
        }
        let status = match &self.sync_state {
            SyncState::Off => tr!("Not syncing"),
            _ => self.sync_status(),
        };
        div()
            .w(px(SIDEBAR_W))
            .flex_shrink_0()
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(c.panel))
            .border_r_1()
            .border_color(rgb(c.line))
            .text_size(ui(13.))
            .child(list)
            .child(
                div()
                    .px(px(12.))
                    .pt(px(10.))
                    .border_t_1()
                    .border_color(rgb(c.line))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .text_size(ui(12.))
                    .text_color(rgb(c.muted))
                    .child(div().flex_1().min_w_0().truncate().child(status))
                    .child(
                        div()
                            .id("sync-manage")
                            .flex_shrink_0()
                            .text_color(rgb(c.accent))
                            .cursor_pointer()
                            .hover(|s| s.text_color(rgb(c.accent_hover)))
                            .on_click(cx.listener(|ws, _, window, cx| ws.open_sync(window, cx)))
                            .child(tr!("Manage")),
                    ),
            )
            .child(
                div()
                    .p(px(10.))
                    .flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("side-add")
                            .flex_1()
                            .h(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(6.))
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(c.edge))
                            .bg(rgb(c.button))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(c.button_hover)))
                            .on_click(cx.listener(|ws, _, window, cx| ws.open_adding(window, cx)))
                            .child(
                                svg()
                                    .path("icons/plus.svg")
                                    .size(px(13.))
                                    .text_color(rgb(c.soft)),
                            )
                            .child(tr!("Add code")),
                    )
                    .child(
                        div()
                            .id("side-scan")
                            .size(px(32.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(rgb(c.edge))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(c.hover)))
                            .on_click(cx.listener(|ws, _, window, cx| ws.open_scanning(window, cx)))
                            .child(
                                svg()
                                    .path("icons/scan.svg")
                                    .size(px(15.))
                                    .text_color(rgb(c.soft)),
                            ),
                    ),
            )
    }

    fn card(&self, entry: &Entry, place: usize, now: u64, cx: &mut Context<Self>) -> AnyElement {
        let c = colors();
        let code = authwing_totp::grouped(&entry.otp.code_at(now));
        let left = entry.otp.seconds_left(now);
        let period = entry.otp.period().unwrap_or(30).max(1);
        let ending = left.is_some_and(|l| l <= SHOW_NEXT);
        let next = left
            .filter(|_| ending)
            .and_then(|_| entry.otp.next_code_at(now))
            .map(|n| authwing_totp::grouped(&n));
        let copied = self.copied.as_ref().is_some_and(|(id, _)| id == &entry.id);
        let (id, delete) = (entry.id.clone(), entry.id.clone());
        let (star, edit) = (entry.id.clone(), entry.id.clone());
        let hint = (place < 9)
            .then(|| keymap_hint(cx, place))
            .filter(|h| !h.is_empty());
        div()
            .id(SharedString::from(format!("card-{}", entry.id)))
            .group("card")
            .w(px(CARD_W))
            .relative()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(14.))
            .rounded(px(10.))
            .bg(rgb(c.surface))
            .border_1()
            .border_color(rgb(if ending { c.warn_edge } else { c.line }))
            .cursor_pointer()
            .hover(|s| s.border_color(rgb(c.edge)))
            .on_click(cx.listener(move |ws, _, _, cx| ws.copy(&id, cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .size(px(30.))
                            .flex_shrink_0()
                            .rounded(px(8.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(rgb(c.chip))
                            .text_size(ui(11.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(c.soft))
                            .child(entry.initials()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(5.))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .truncate()
                                            .child(entry.title()),
                                    )
                                    .when(entry.favorite, |d| {
                                        d.child(
                                            svg()
                                                .path("icons/star-fill.svg")
                                                .size(px(11.))
                                                .flex_shrink_0()
                                                .text_color(rgb(c.amber)),
                                        )
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(ui(12.))
                                    .text_color(rgb(c.muted))
                                    .truncate()
                                    .child(entry.detail().to_string()),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(ui(26.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(if ending { c.warn } else { c.text }))
                            .child(code),
                    )
                    .when_some(next, |d, n| {
                        d.child(
                            div()
                                .text_size(ui(12.))
                                .text_color(rgb(c.muted))
                                .child(tr!("next {code}", code = n)),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .h(px(3.))
                            .rounded(px(2.))
                            .bg(rgb(c.line))
                            .when_some(left, |d, l| {
                                d.child(
                                    div()
                                        .h_full()
                                        .rounded(px(2.))
                                        .w(relative(l as f32 / period as f32))
                                        .bg(rgb(if ending { c.amber } else { c.accent })),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(ui(11.))
                            .text_color(rgb(if copied { c.good } else { c.faint }))
                            .child(if copied {
                                tr!("Copied")
                            } else if let Some(l) = left {
                                format!("{l} s")
                            } else {
                                tr!("Counter")
                            }),
                    )
                    .when_some(hint, |d, h| {
                        d.child(div().text_size(ui(11.)).text_color(rgb(c.faint)).child(h))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .top(px(10.))
                    .right(px(10.))
                    .flex()
                    .gap(px(2.))
                    .p(px(2.))
                    .rounded(px(7.))
                    .bg(rgb(c.surface))
                    .invisible()
                    .group_hover("card", |s| s.visible())
                    .child(card_button(
                        format!("star-{}", entry.id),
                        if entry.favorite {
                            "icons/star-fill.svg"
                        } else {
                            "icons/star.svg"
                        },
                        if entry.favorite { c.amber } else { c.muted },
                        cx.listener(move |ws, _, _, cx| {
                            cx.stop_propagation();
                            ws.toggle_favorite(&star, cx);
                        }),
                    ))
                    .child(card_button(
                        format!("edit-{}", entry.id),
                        "icons/pencil.svg",
                        c.muted,
                        cx.listener(move |ws, _, window, cx| {
                            cx.stop_propagation();
                            ws.open_editing(&edit, window, cx);
                        }),
                    ))
                    .child(card_button(
                        format!("delete-{}", entry.id),
                        "icons/trash.svg",
                        c.muted,
                        cx.listener(move |ws, _, window, cx| {
                            cx.stop_propagation();
                            ws.delete(delete.clone(), window, cx);
                        }),
                    )),
            )
            .into_any_element()
    }

    /// The codes for `window` (and `page`, the browser's tab in it), surest
    /// first, `most` of them, with why each; or, on a page that only looks
    /// like a code's site, why none is.
    fn rank_for(
        &self,
        window: &authwing_context::Window,
        page: Option<&Tab>,
        most: usize,
    ) -> (Vec<(&Entry, Reason)>, Option<Warning>) {
        let place = Where {
            window: Some(window.clone()),
            page: page.map(|t| Page::parse(&t.url)),
        };
        let list: Vec<Code> = self
            .codes
            .iter()
            .map(|e| Code {
                issuer: &e.otp.issuer,
                sites: &e.parsed,
                rules: &[],
                times: 0,
            })
            .collect();
        match authwing_context::rank(&place, &list) {
            Ranking::Candidates(found) => (
                found
                    .into_iter()
                    .take(most)
                    .filter_map(|c| Some((self.codes.get(c.code)?, c.why)))
                    .collect(),
                None,
            ),
            Ranking::LookAlike(w) => (vec![], Some(w)),
        }
    }

    /// The codes for the window used last; see [`Workspace::rank_for`].
    fn for_window(
        &self,
    ) -> (
        Option<&authwing_context::Window>,
        Vec<(&Entry, Reason)>,
        Option<Warning>,
    ) {
        let Some(window) = &self.elsewhere else {
            return (None, vec![], None);
        };
        let (found, warning) = self.rank_for(window, self.page(), FOR_WINDOW);
        (Some(window), found, warning)
    }

    fn for_window_view(&self, now: u64, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let c = colors();
        let (window, found, warning) = self.for_window();
        let window = window?;
        if let Some(w) = warning {
            return Some(
                notice::colored(div(), notice::Tone::Bad)
                    .mb(px(18.))
                    .p(px(14.))
                    .rounded(px(12.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr!("This page is not {site}", site = w.like.to_string())),
                    )
                    .child(div().text_size(ui(12.)).child(tr!(
                        "{page} only looks like it. Pages like this copy a real sign-in to take your code: none is offered here.",
                        page = w.page.clone()
                    ))),
            );
        }
        if found.is_empty() {
            return None;
        }
        let place = match &window.title {
            Some(t) if !t.is_empty() => format!("{} · {}", window.app_name, t),
            _ => window.app_name.clone(),
        };
        let cards: Vec<AnyElement> = found
            .iter()
            .map(|(e, why)| {
                let reason = reason_label(*why);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(self.card(e, usize::MAX, now, cx))
                    .child(
                        div()
                            .px(px(4.))
                            .text_size(ui(11.))
                            .text_color(rgb(c.info_text))
                            .child(tr!("Matched by {why}", why = reason)),
                    )
                    .into_any_element()
            })
            .collect();
        Some(
            div()
                .mb(px(18.))
                .p(px(14.))
                .flex()
                .flex_col()
                .gap(px(10.))
                .rounded(px(12.))
                .bg(rgb(c.info_bg))
                .border_1()
                .border_color(rgb(c.info_edge))
                .child(
                    div()
                        .text_size(ui(12.))
                        .text_color(rgb(c.info_text))
                        .child(tr!("For the window used last: {place}", place = place)),
                )
                .child(div().flex().flex_wrap().gap(px(12.)).children(cards)),
        )
    }

    fn codes_view(&self, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let c = colors();
        let now = codes::now();
        let filtering = !self.filter.read(cx).text().trim().is_empty();
        let strip = (!filtering)
            .then(|| self.for_window_view(now, cx))
            .flatten();
        let shown: Vec<String> = self.shown(cx).iter().map(|e| e.id.clone()).collect();
        let cards: Vec<AnyElement> = shown
            .iter()
            .enumerate()
            .filter_map(|(i, id)| {
                let entry = self.codes.iter().find(|e| &e.id == id)?;
                Some(self.card(entry, i, now, cx))
            })
            .collect();
        let body = if self.codes.is_empty() {
            empty(
                tr!("No codes yet"),
                tr!("Turn on two-factor sign-in at a site, then read the QR code it shows."),
            )
            .child(
                primary_button("first-code", &tr!("Add your first code"))
                    .on_click(cx.listener(|ws, _, window, cx| ws.open_adding(window, cx))),
            )
            .into_any_element()
        } else if cards.is_empty() {
            let typed = !self.filter.read(cx).text().trim().is_empty();
            let (title, detail) = match &self.shown {
                _ if typed => (
                    tr!("Nothing matches"),
                    tr!("Try fewer words, or another group."),
                ),
                Shown::Favorites => (
                    tr!("No favorites yet"),
                    tr!("The star on a code makes it one."),
                ),
                Shown::Recent => (
                    tr!("Nothing used yet"),
                    tr!("The codes you copy or type show up here, the last first."),
                ),
                Shown::Group(name) => (
                    tr!("{group} is empty", group = name.clone()),
                    tr!(
                        "Edit a code (the pencil on its card) to file it here, or add one while this group is shown."
                    ),
                ),
                _ => (
                    tr!("Nothing matches"),
                    tr!("Try fewer words, or another group."),
                ),
            };
            empty(title, detail).into_any_element()
        } else {
            div()
                .flex()
                .flex_wrap()
                .gap(px(12.))
                .children(cards)
                .into_any_element()
        };
        div()
            .id("codes")
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .p(px(20.))
            .bg(rgb(c.page))
            .children(strip)
            .child(body)
    }

    fn locked_view(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let (title, detail) = match self.vault.locked() {
            Some(Lock::New) => (
                tr!("Set a passphrase"),
                tr!("This computer has no keychain to keep the vault's key in."),
            ),
            Some(_) => (
                tr!("The vault is locked"),
                tr!("Type its passphrase, or the recovery key you saved."),
            ),
            None => match self.vault.problem() {
                Some(p) => (tr!("The vault cannot open"), p),
                None => (tr!("Opening the vault…"), String::new()),
            },
        };
        let can_type = self.vault.locked().is_some();
        let mut body = empty(title, detail);
        if self.vault.opens_with_presence() {
            body = body.child(
                outline_button("presence", &tr!("Open with Touch ID or Windows Hello"))
                    .on_click(cx.listener(|ws, _, window, cx| ws.unlock_with_presence(window, cx))),
            );
        }
        if can_type {
            body = body.child(
                div()
                    .w(px(320.))
                    .flex()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex_1()
                            .h(px(32.))
                            .px(px(10.))
                            .flex()
                            .items_center()
                            .rounded(px(6.))
                            .bg(rgb(c.field))
                            .border_1()
                            .border_color(rgb(c.field_edge))
                            .on_key_down(cx.listener(|ws, ev: &KeyDownEvent, window, cx| {
                                if ev.keystroke.key == "enter" {
                                    ws.try_unlock(window, cx);
                                    cx.stop_propagation();
                                }
                            }))
                            .child(self.unlock.clone()),
                    )
                    .child(
                        primary_button(
                            "unlock",
                            &if self.unlocking {
                                tr!("Opening…")
                            } else {
                                tr!("Open")
                            },
                        )
                        .on_click(cx.listener(|ws, _, window, cx| ws.try_unlock(window, cx))),
                    ),
            );
        }
        div().flex_1().h_full().bg(rgb(c.page)).child(body)
    }
}

/// A heading and a line under it, centred: what a page shows with nothing on it.
/// A small button on a card, among those shown on hover.
fn card_button(
    id: String,
    icon: &'static str,
    color: u32,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> gpui::Stateful<gpui::Div> {
    let c = colors();
    div()
        .id(SharedString::from(id))
        .size(px(24.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(5.))
        .hover(|s| s.bg(rgb(c.hover)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(svg().path(icon).size(px(14.)).text_color(rgb(color)))
}

fn empty(title: String, detail: String) -> gpui::Div {
    let c = colors();
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(10.))
        .p(px(40.))
        .child(
            div()
                .text_size(ui(16.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(c.text))
                .child(title),
        )
        .when(!detail.is_empty(), |d| {
            d.child(
                div()
                    .max_w(px(420.))
                    .text_center()
                    .text_size(ui(13.))
                    .text_color(rgb(c.muted))
                    .child(detail),
            )
        })
}

/// The key that copies the code at `place` (0 for 1), as shown.
fn keymap_hint(cx: &App, place: usize) -> String {
    let keys = crate::hotkeys::keymap(cx);
    keys.chords(Hotkey::CopyCode)
        .first()
        .map(|c| c.label(keys.os()).replace("1…9", &(place + 1).to_string()))
        .unwrap_or_default()
}

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(6.))
            .rounded(px(6.))
            .bg(rgb(colors().chip))
            .border_1()
            .border_color(rgb(colors().edge))
            .text_size(ui(12.))
            .text_color(rgb(colors().text))
            .child(self.0.clone())
    }
}

fn tip(text: String, cx: &mut App) -> gpui::AnyView {
    cx.new(|_| Tip(text.into())).into()
}

impl Focusable for Workspace {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let ui_font = self.live.settings().appearance.ui_font.clone();
        window.set_rem_size(look::rem_for(ui_font.size));
        let family = ui_font.family.clone().map(SharedString::from);
        if cx
            .try_global::<crate::prompt::PromptFont>()
            .is_none_or(|f| f.0 != family)
        {
            cx.set_global(crate::prompt::PromptFont(family.clone()));
        }
        let bottom = chrome::BottomCorners::of(window);
        if chrome::BottomCorners::current(cx) != bottom {
            cx.set_global(bottom);
        }
        let c = colors();
        let main: AnyElement = if let Some(settings) = &self.settings {
            bottom
                .round(div().size_full().child(settings.clone()))
                .into_any_element()
        } else if self.vault.ready().is_some() {
            div()
                .size_full()
                .flex()
                .child(bottom.left().round(self.sidebar(cx)))
                .child(bottom.right().round(self.codes_view(cx)))
                .into_any_element()
        } else {
            bottom.round(self.locked_view(cx)).into_any_element()
        };
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .relative()
            .child(main)
            .when_some(self.notice.clone(), |d, (n, _)| {
                d.child(
                    div()
                        .absolute()
                        .bottom(px(16.))
                        .right(px(16.))
                        .max_w(px(420.))
                        .child(
                            notice::colored(div(), n.tone)
                                .px(px(14.))
                                .py(px(10.))
                                .rounded(px(8.))
                                .text_size(ui(13.))
                                .child(n.text),
                        ),
                )
            });
        let title = self.title_bar(window.scale_factor(), cx);
        let root = if chrome::draws_title_bar(window) {
            chrome::window_chrome(title, body, rgb(c.page).into(), window)
        } else {
            // A native title bar (X11): the bar's own row under it.
            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(rgb(c.page))
                .child(
                    div()
                        .h(px(chrome::TITLEBAR_H))
                        .flex_shrink_0()
                        .bg(rgb(c.chrome))
                        .border_b_1()
                        .border_color(rgb(c.line))
                        .child(title),
                )
                .child(body)
        };
        let dialog: Option<AnyElement> = match (&self.adding, &self.panel, &self.group_editor) {
            (Some(add), _, _) => Some(add.clone().into_any_element()),
            (None, Some(panel), _) => Some(panel.clone().into_any_element()),
            (None, None, Some(editor)) => Some(editor.clone().into_any_element()),
            (None, None, None) => None,
        };
        let dialog = dialog.map(|dialog| {
            let (at, shade) = chrome::shade(window);
            deferred(
                anchored().position(at).child(
                    shade
                        .occlude()
                        .flex()
                        .justify_center()
                        .items_start()
                        .pt(px(72.))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|ws, _, window, cx| {
                                ws.adding = None;
                                ws.panel = None;
                                ws.group_editor = None;
                                window.focus(&ws.focus, cx);
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .child(dialog),
                        ),
                ),
            )
            .with_priority(10)
        });
        root.when_some(ui_font.family, |d, f| d.font_family(f))
            .text_size(ui(13.))
            .text_color(rgb(c.text))
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::shortcut))
            .children(dialog)
    }
}

/// The drawn title bar's close button: as the system's.
pub fn close_window(window: &mut Window, _: &mut App) {
    window.remove_window();
}

/// Why a code is offered for a window or page, in a few words.
fn reason_label(why: Reason) -> String {
    match why {
        Reason::Site => tr!("its site"),
        Reason::Rule => tr!("a rule"),
        Reason::Learned { .. } => tr!("chosen here before"),
        Reason::IssuerInTitle => tr!("its name in the title"),
    }
}
