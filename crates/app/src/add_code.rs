//! Adding codes: from the link a site's QR code holds (`otpauth://…`), from
//! Google Authenticator's export (`otpauth-migration://…`, many at once), or
//! from the key a site shows to type when its QR code cannot be scanned.
//!
//! The QR code itself is read where it is: on the screen, in an image file
//! chosen or dropped on the dialog, or in an image pasted into it. What it
//! holds goes into the link field, as if pasted there.
//!
//! The same dialog edits a code once added ([`AddCode::edit`]): its names,
//! its group and its sites; its key stays as it is.

use std::path::PathBuf;
use std::time::Duration;

use gpui::{
    App, ClipboardEntry, Context, Entity, EventEmitter, ExternalPaths, FocusHandle, Focusable,
    FontWeight, KeyDownEvent, PathPromptOptions, Render, SharedString, Window, div, prelude::*, px,
    rgb,
};

use authwing_i18n::{tr, tr_n};
use authwing_totp::{Kind, Migration, Otp, Secret};

use crate::colors::colors;
use crate::input::{Changed, TextInput, said};
use crate::look::ui;
use crate::widgets::{outline_button, primary_button};

pub enum AddEvent {
    /// Codes to save, under a group and shown first on these sites.
    Add {
        codes: Vec<Otp>,
        group: String,
        sites: Vec<String>,
    },
    /// The code `id`, its names, group and sites as now.
    Edit {
        id: String,
        issuer: String,
        account: String,
        group: String,
        sites: Vec<String>,
    },
    Close,
}

pub struct AddCode {
    link: Entity<TextInput>,
    issuer: Entity<TextInput>,
    account: Entity<TextInput>,
    group: Entity<TextInput>,
    sites: Entity<TextInput>,
    problem: Option<String>,
    /// What a QR code read put in the link field, and what to say of it
    /// while the field still holds that.
    note: Option<(String, String)>,
    /// A QR code being looked for, until the answer comes.
    scanning: bool,
    /// The code being edited; None: adding.
    editing: Option<String>,
    /// The groups there are, to pick one.
    groups: Vec<(String, u32)>,
    focus: FocusHandle,
}

/// Where a QR code was looked for, for what to say when there is none.
#[derive(Clone, Copy)]
enum Place {
    Screen,
    Image,
}

impl EventEmitter<AddEvent> for AddCode {}

impl Focusable for AddCode {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// What the link field holds, read.
enum Read {
    One(Otp),
    Many(Migration),
    /// A bare key: issuer and account come from their fields.
    Key(Secret),
}

impl AddCode {
    /// Adds codes, under `group` at first; `groups` (names and colors) are
    /// offered.
    pub fn new(group: Option<&str>, groups: Vec<(String, u32)>, cx: &mut Context<Self>) -> Self {
        let link = said(cx, || tr!("otpauth:// link, or the key the site shows"));
        // A link fills in what it says, so the fields show what will be saved.
        cx.subscribe(&link, |this: &mut AddCode, _, _: &Changed, cx| {
            this.problem = None;
            this.fill_from_link(cx);
            cx.notify();
        })
        .detach();
        let group_field = said(cx, || tr!("None"));
        if let Some(g) = group {
            group_field.update(cx, |f, cx| f.set_text(g, g.len()..g.len(), cx));
        }
        AddCode {
            link,
            issuer: said(cx, || "GitHub".to_string()),
            account: said(cx, || "you@example.com".to_string()),
            group: group_field,
            sites: said(cx, || "github.com, gist.github.com".to_string()),
            problem: None,
            note: None,
            scanning: false,
            editing: None,
            groups,
            focus: cx.focus_handle(),
        }
    }

    /// Edits the code `entry`: its names, group and sites.
    pub fn edit(
        entry: &crate::codes::Entry,
        groups: Vec<(String, u32)>,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::new(Some(&entry.group), groups, cx);
        this.editing = Some(entry.id.clone());
        let sites = entry.sites.join(", ");
        for (field, value) in [
            (&this.issuer, &entry.otp.issuer),
            (&this.account, &entry.otp.account),
            (&this.sites, &sites),
        ] {
            field.update(cx, |f, cx| f.set_text(value, value.len()..value.len(), cx));
        }
        this
    }

    /// Where the cursor goes first: the link when adding, the service when
    /// editing.
    pub fn first_focus(&self, cx: &App) -> FocusHandle {
        if self.editing.is_some() {
            self.issuer.focus_handle(cx)
        } else {
            self.link.focus_handle(cx)
        }
    }

    fn read(text: &str) -> Result<Read, String> {
        let text = text.trim();
        if text.is_empty() {
            return Err(tr!("Paste the code's link, or type its key."));
        }
        let lower = text.to_ascii_lowercase();
        if lower.starts_with("otpauth-migration:") {
            return Migration::parse(text)
                .map(Read::Many)
                .map_err(|e| e.to_string());
        }
        if lower.starts_with("otpauth:") {
            return Otp::from_uri(text)
                .map(Read::One)
                .map_err(|e| e.to_string());
        }
        Secret::from_base32(text)
            .map(Read::Key)
            .map_err(|e| tr!("This is neither a link nor a key: {e}", e = e.to_string()))
    }

    fn fill_from_link(&mut self, cx: &mut Context<Self>) {
        let text = self.link.read(cx).text().to_string();
        let Ok(Read::One(otp)) = Self::read(&text) else {
            return;
        };
        for (field, value) in [(&self.issuer, otp.issuer), (&self.account, otp.account)] {
            if field.read(cx).text().is_empty() && !value.is_empty() {
                field.update(cx, |f, cx| f.set_text(&value, value.len()..value.len(), cx));
            }
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let text = |f: &Entity<TextInput>, cx: &Context<Self>| f.read(cx).text().trim().to_string();
        let (issuer, account) = (text(&self.issuer, cx), text(&self.account, cx));
        let group = text(&self.group, cx);
        let sites: Vec<String> = text(&self.sites, cx)
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|s| !s.is_empty())
            .map(|s| s.trim_end_matches('/').to_lowercase())
            .collect();
        if let Some(id) = self.editing.clone() {
            if issuer.is_empty() && account.is_empty() {
                self.problem = Some(tr!("Say whose code it is: the service, or the account."));
                cx.notify();
                return;
            }
            cx.emit(AddEvent::Edit {
                id,
                issuer,
                account,
                group,
                sites,
            });
            return;
        }
        let codes = match Self::read(&text(&self.link, cx)) {
            Err(e) => {
                self.problem = Some(e);
                cx.notify();
                return;
            }
            Ok(Read::One(mut otp)) => {
                // What was typed over the link's own names wins.
                if !issuer.is_empty() {
                    otp.issuer = issuer;
                }
                if !account.is_empty() {
                    otp.account = account;
                }
                vec![otp]
            }
            Ok(Read::Key(secret)) => {
                if issuer.is_empty() && account.is_empty() {
                    self.problem = Some(tr!("Say whose code it is: the service, or the account."));
                    cx.notify();
                    return;
                }
                vec![Otp::new(issuer, account, secret, Kind::Totp { period: 30 })]
            }
            Ok(Read::Many(m)) => {
                if m.codes.is_empty() {
                    self.problem = Some(tr_n!(
                        m.skipped.len(),
                        "The export holds no code authwing can use ({n} left out).",
                        "The export holds no code authwing can use ({n} left out)."
                    ));
                    cx.notify();
                    return;
                }
                m.codes
            }
        };
        cx.emit(AddEvent::Add {
            codes,
            group,
            sites,
        });
    }

    /// Reads the QR codes on the screen. On macOS authwing steps out of the
    /// way while it does, as its window may be over the code.
    pub fn scan_screen(&mut self, cx: &mut Context<Self>) {
        if self.scanning {
            return;
        }
        if authwing_scan::access() == authwing_scan::Access::NotGranted {
            // macOS's dialog the first time; it applies at the next start.
            authwing_scan::ask_access();
            self.problem = Some(tr!(
                "macOS asks first: turn authwing on in System Settings › Privacy & Security › Screen Recording, then open authwing again."
            ));
            cx.notify();
            return;
        }
        self.start_scan(cx);
        let step_out = cfg!(target_os = "macos");
        if step_out {
            cx.hide();
        }
        cx.spawn(async move |this, cx| {
            if step_out {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
            }
            let found = cx.background_spawn(async { authwing_scan::screen() }).await;
            if step_out {
                cx.update(|cx| cx.activate(true));
            }
            let _ = this.update(cx, |this, cx| {
                let found = found.map(|f| f.into_iter().map(|f| f.text).collect());
                this.scanned(found, Place::Screen, cx);
            });
        })
        .detach();
    }

    fn choose_image(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(tr!("Read").into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update(cx, |this, cx| this.scan_file(path, cx));
            }
        })
        .detach();
    }

    fn scan_file(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.scan_image(move || std::fs::read(path), cx);
    }

    /// Reads the QR codes in the image `load` gives, off the UI thread.
    fn scan_image(
        &mut self,
        load: impl FnOnce() -> std::io::Result<Vec<u8>> + Send + 'static,
        cx: &mut Context<Self>,
    ) {
        if self.scanning {
            return;
        }
        self.start_scan(cx);
        cx.spawn(async move |this, cx| {
            let found = cx
                .background_spawn(async move {
                    let bytes = load().map_err(|e| authwing_scan::Error::Capture(e.to_string()))?;
                    authwing_scan::from_file_bytes(&bytes)
                })
                .await;
            let _ = this.update(cx, |this, cx| this.scanned(found, Place::Image, cx));
        })
        .detach();
    }

    fn start_scan(&mut self, cx: &mut Context<Self>) {
        self.scanning = true;
        self.problem = None;
        self.note = None;
        cx.notify();
    }

    /// Puts the first two-factor code found into the link field, or says
    /// why there is none.
    fn scanned(
        &mut self,
        found: Result<Vec<String>, authwing_scan::Error>,
        place: Place,
        cx: &mut Context<Self>,
    ) {
        self.scanning = false;
        cx.notify();
        let texts = match found {
            Ok(texts) => texts,
            Err(e) => {
                self.problem = Some(scan_error(&e));
                return;
            }
        };
        let codes = authwing_scan::two_factor(&texts);
        let Some(first) = codes.first() else {
            self.problem = Some(match place {
                _ if !texts.is_empty() => tr!("This QR code is not a two-factor code."),
                Place::Screen => {
                    tr!("No QR code on the screen: show the site's page with it, and scan again.")
                }
                Place::Image => tr!("No QR code in this image."),
            });
            return;
        };
        // The names come from the code, not from one read before.
        for field in [&self.issuer, &self.account] {
            field.update(cx, |f, cx| f.set_text("", 0..0, cx));
        }
        self.link
            .update(cx, |f, cx| f.set_text(first, first.len()..first.len(), cx));
        let said = if codes.len() > 1 {
            tr_n!(
                codes.len(),
                "One of {n} two-factor QR codes: add it, then scan again for the next.",
                "One of {n} two-factor QR codes: add it, then scan again for the next."
            )
        } else {
            tr!("Read from the QR code. Check the names, then add it.")
        };
        self.note = Some((first.clone(), said));
    }

    /// Cmd/Ctrl+V with an image, or an image file, on the clipboard reads
    /// its QR code; with text, the field pastes it as usual.
    fn on_paste(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let cmd = if cfg!(target_os = "macos") {
            k.modifiers.platform
        } else {
            k.modifiers.control
        };
        if k.key != "v" || !cmd {
            return;
        }
        let Some(item) = cx.read_from_clipboard() else {
            return;
        };
        for entry in item.entries() {
            match entry {
                ClipboardEntry::Image(image) => {
                    let bytes = image.bytes.clone();
                    self.scan_image(move || Ok(bytes), cx);
                }
                ClipboardEntry::ExternalPaths(paths) if is_image(paths) => {
                    self.scan_file(paths.paths()[0].clone(), cx);
                }
                _ => continue,
            }
            return cx.stop_propagation();
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "enter" => self.submit(cx),
            "escape" => cx.emit(AddEvent::Close),
            _ => return,
        }
        cx.stop_propagation();
    }
}

/// One file, named as an image: Finder's Copy of a screenshot, say.
fn is_image(paths: &ExternalPaths) -> bool {
    let [path] = paths.paths() else {
        return false;
    };
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["png", "jpg", "jpeg", "gif", "webp", "bmp", "tif", "tiff"]
            .contains(&e.to_ascii_lowercase().as_str())
    })
}

fn scan_error(e: &authwing_scan::Error) -> String {
    match e {
        authwing_scan::Error::NotAllowed if cfg!(target_os = "macos") => tr!(
            "macOS asks first: turn authwing on in System Settings › Privacy & Security › Screen Recording, then open authwing again."
        ),
        authwing_scan::Error::NotAllowed => tr!("The screen was not shared, so nothing was read."),
        authwing_scan::Error::Capture(why) => tr!("Not read: {why}", why = why.clone()),
        authwing_scan::Error::Unreadable(_) => tr!("This is not an image authwing can read."),
    }
}

fn field(label: String, input: &Entity<TextInput>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .text_size(ui(12.))
                .text_color(rgb(colors().label))
                .child(label),
        )
        .child(
            div()
                .h(px(32.))
                .px(px(10.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .bg(rgb(colors().field))
                .border_1()
                .border_color(rgb(colors().field_edge))
                .child(input.clone()),
        )
}

impl Render for AddCode {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let editing = self.editing.is_some();
        let typed_group = self.group.read(cx).text().trim().to_lowercase();
        let mut group_chips = div().flex().flex_wrap().gap(px(6.));
        for (name, color) in &self.groups {
            let on = typed_group == name.to_lowercase();
            let pick = name.clone();
            group_chips = group_chips.child(
                div()
                    .id(SharedString::from(format!("pick-group-{name}")))
                    .h(px(24.))
                    .px(px(8.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(6.))
                    .border_1()
                    .border_color(rgb(if on { c.accent } else { c.edge }))
                    .when(on, |d| d.bg(rgb(c.selected)))
                    .cursor_pointer()
                    .hover(|s| s.bg(rgb(c.hover)))
                    .text_size(ui(12.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let name = if this.group.read(cx).text().trim() == pick {
                            String::new()
                        } else {
                            pick.clone()
                        };
                        this.group
                            .update(cx, |f, cx| f.set_text(&name, name.len()..name.len(), cx));
                        cx.notify();
                    }))
                    .child(div().size(px(8.)).rounded(px(2.)).bg(rgb(*color)))
                    .child(name.clone()),
            );
        }
        div()
            .track_focus(&self.focus)
            .when(!editing, |d| d.capture_key_down(cx.listener(Self::on_paste)))
            .on_key_down(cx.listener(Self::on_key))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.scan_file(path.clone(), cx);
                }
            }))
            .drag_over::<ExternalPaths>(|style, _, _, _| style.border_color(rgb(colors().accent)))
            .w(px(520.))
            .flex()
            .flex_col()
            .gap(px(14.))
            .p(px(20.))
            .rounded(px(12.))
            .bg(rgb(c.surface))
            .border_1()
            .border_color(rgb(c.edge))
            .text_color(rgb(c.text))
            .child(
                div()
                    .text_size(ui(16.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(if editing {
                        tr!("Edit the code")
                    } else {
                        tr!("Add a code")
                    }),
            )
            .when(!editing, |d| {
                d.child(
                div()
                    .text_size(ui(12.))
                    .text_color(rgb(c.muted))
                    .child(tr!(
                        "When a site turns on two-factor sign-in it shows a QR code, and often a key beside it. Read the QR code where it is, or paste its link or the key."
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        outline_button(
                            "scan-screen",
                            &if self.scanning {
                                tr!("Reading…")
                            } else {
                                tr!("Scan the screen")
                            },
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.scan_screen(cx))),
                    )
                    .child(
                        outline_button("scan-image", &tr!("Choose an image…"))
                            .on_click(cx.listener(|this, _, _, cx| this.choose_image(cx))),
                    )
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.muted))
                            .child(tr!("or paste or drop one here")),
                    ),
            )
            .child(field(tr!("Link or key"), &self.link))
            })
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(div().flex_1().child(field(tr!("Service"), &self.issuer)))
                    .child(div().flex_1().child(field(tr!("Account"), &self.account))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(div().w(px(160.)).child(field(tr!("Group"), &self.group)))
                    .child(div().flex_1().child(field(tr!("Show it first on"), &self.sites))),
            )
            .when(!self.groups.is_empty(), |d| d.child(group_chips))
            .when(self.problem.is_none(), |d| {
                let link = self.link.read(cx).text();
                let note = self.note.as_ref().filter(|(text, _)| link == text.as_str());
                d.when_some(note.map(|(_, said)| said.clone()), |d, note| {
                    d.child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.good))
                            .child(note),
                    )
                })
            })
            .when_some(self.problem.clone(), |d, p| {
                d.child(
                    div()
                        .text_size(ui(12.))
                        .text_color(rgb(c.bad))
                        .child(p),
                )
            })
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        outline_button("add-cancel", &tr!("Cancel"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(AddEvent::Close))),
                    )
                    .child(
                        primary_button(
                            "add-save",
                            &if editing { tr!("Save") } else { tr!("Add") },
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
    }
}
