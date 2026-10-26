//! The welcome: the first time authwing opens on a device with no codes,
//! a few steps in place of the empty list. What it does, bringing codes in
//! (from a phone through the camera, a QR code on the screen, another app's
//! export, by hand), the quick panel and typing codes in, voice, and the
//! recovery key. Each can be skipped; Settings › About shows it again.

use gpui::{
    Context, EventEmitter, FontWeight, Render, SharedString, Window, div, prelude::*, px, rgb, svg,
};

use authwing_config::Settings;
use authwing_i18n::{tr, tr_n};

use crate::colors::colors;
use crate::look::ui;
use crate::settings::Page;
use crate::vault_slot::VaultSlot;
use crate::voice::{self, Voice};
use crate::widgets::{outline_button, primary_button, text_button};

pub enum WelcomeEvent {
    /// Add a code by hand.
    Add,
    /// Read a QR code on the screen.
    Scan,
    /// Hold QR codes up to the camera: a phone's export.
    Camera,
    /// Open Settings at a page: Backup to import, Browsers, Sync.
    Settings(Page),
    /// Seen, or skipped: not shown again.
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Hello,
    Codes,
    SignIn,
    Voice,
    Key,
}

const STEPS: [Step; 5] = [
    Step::Hello,
    Step::Codes,
    Step::SignIn,
    Step::Voice,
    Step::Key,
];

pub struct Welcome {
    step: usize,
    /// How many codes there are, as the window last read them.
    codes: usize,
    settings: Settings,
    vault: VaultSlot,
    /// The recovery key, once shown.
    key: Option<String>,
}

impl EventEmitter<WelcomeEvent> for Welcome {}

impl Welcome {
    pub fn new(codes: usize, settings: Settings, vault: VaultSlot) -> Self {
        Welcome {
            step: 0,
            codes,
            settings,
            vault,
            key: None,
        }
    }

    /// What the window knows now: its codes, and the settings.
    pub fn update_from(&mut self, codes: usize, settings: &Settings, cx: &mut Context<Self>) {
        if self.codes != codes || &self.settings != settings {
            self.codes = codes;
            self.settings = settings.clone();
            cx.notify();
        }
    }

    fn go(&mut self, step: usize, cx: &mut Context<Self>) {
        if step >= STEPS.len() {
            cx.emit(WelcomeEvent::Done);
        } else {
            self.step = step;
        }
        cx.notify();
    }

    fn show_key(&mut self, cx: &mut Context<Self>) {
        self.key = match self.vault.ready().map(|v| v.recovery_key()) {
            Some(Ok(Some(k))) => Some(k.to_string()),
            _ => None,
        };
        cx.notify();
    }

    fn hello(&self) -> gpui::Div {
        let c = colors();
        let point = |icon: &'static str, title: String, text: String| {
            div()
                .flex()
                .gap(px(12.))
                .child(
                    div()
                        .size(px(32.))
                        .flex_shrink_0()
                        .rounded(px(8.))
                        .bg(rgb(c.chip))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(svg().path(icon).size(px(16.)).text_color(rgb(c.accent))),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(
                            div()
                                .text_size(ui(12.))
                                .text_color(rgb(c.muted))
                                .child(text),
                        ),
                )
        };
        let key = crate::panel_key::label(&self.settings.panel.key);
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(point(
                "icons/search.svg",
                tr!("The code for the window you are in"),
                tr!("The codes for the app or site in front come first: by its sites, its window's title, or the browser extension."),
            ))
            .child(point(
                "icons/grid.svg",
                if key.is_empty() {
                    tr!("From any app")
                } else {
                    tr!("From any app, with {key}", key = key)
                },
                tr!("A small panel over the app you are in, and Enter types the code into it."),
            ))
            .child(point(
                "icons/mic.svg",
                tr!("Say its name"),
                tr!("“Open GitHub”, “PayPal”: Whisper hears it on this device, in your language."),
            ))
            .child(point(
                "icons/lock.svg",
                tr!("Yours alone"),
                tr!("The codes are encrypted on this device, and synced, if you choose to, only as ciphertext."),
            ))
    }

    fn codes_step(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let way = |id: &'static str,
                   title: String,
                   text: String,
                   button: String,
                   event: fn() -> WelcomeEvent,
                   cx: &mut Context<Self>| {
            div()
                .flex()
                .items_center()
                .gap(px(14.))
                .p(px(14.))
                .rounded(px(10.))
                .border_1()
                .border_color(rgb(c.line))
                .bg(rgb(c.surface))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                        .child(
                            div()
                                .text_size(ui(12.))
                                .text_color(rgb(c.muted))
                                .child(text),
                        ),
                )
                .child(
                    outline_button(id, &button)
                        .on_click(cx.listener(move |_, _, _, cx| cx.emit(event()))),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(way(
                "welcome-camera",
                tr!("From your phone"),
                tr!("Google Authenticator: Transfer accounts › Export accounts, then hold each QR code it shows up to the camera."),
                tr!("Use the camera"),
                || WelcomeEvent::Camera,
                cx,
            ))
            .child(way(
                "welcome-scan",
                tr!("From a site's QR code"),
                tr!("Turning on two-factor sign-in, a site shows one: authwing reads it on the screen."),
                tr!("Scan the screen"),
                || WelcomeEvent::Scan,
                cx,
            ))
            .child(way(
                "welcome-import",
                tr!("From another app"),
                tr!("An export from Aegis, 2FAS, Ente Auth or Bitwarden, or an authwing backup."),
                tr!("Import a file…"),
                || WelcomeEvent::Settings(Page::Backup),
                cx,
            ))
            .child(way(
                "welcome-add",
                tr!("By hand"),
                tr!("The link or the key a site shows beside its QR code."),
                tr!("Add a code"),
                || WelcomeEvent::Add,
                cx,
            ))
            .when(self.codes > 0, |d| {
                d.child(
                    div()
                        .text_size(ui(12.))
                        .text_color(rgb(c.good))
                        .child(tr_n!(
                            self.codes,
                            "{n} code here so far.",
                            "{n} codes here so far."
                        )),
                )
            })
    }

    fn sign_in(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let key = crate::panel_key::label(&self.settings.panel.key);
        let typing = crate::typing::allowed();
        let mac = cfg!(target_os = "macos");
        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(if key.is_empty() {
                        tr!("The quick panel's shortcut is off")
                    } else {
                        tr!("Press {key} in any app", key = key)
                    }))
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.muted))
                            .child(tr!(
                                "A panel opens over it with its codes first; Enter types the code where you were. Its key is in Settings › General."
                            )),
                    ),
            )
            .when(mac, |d| {
                d.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(ui(12.))
                                .text_color(rgb(if typing { c.good } else { c.soft }))
                                .child(if typing {
                                    tr!("Typing codes into other apps is allowed.")
                                } else {
                                    tr!("To type codes into other apps, and to see their windows' titles, macOS asks first (Accessibility).")
                                }),
                        )
                        .when(!typing, |d| {
                            d.child(outline_button("welcome-allow", &tr!("Allow…")).on_click(
                                cx.listener(|_, _, _, cx| {
                                    authwing_context::window::ask_title_access();
                                    cx.notify();
                                }),
                            ))
                        }),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(ui(12.))
                            .text_color(rgb(c.soft))
                            .child(tr!(
                                "In a browser, the extension tells authwing the page exactly, and fills the code in."
                            )),
                    )
                    .child(
                        outline_button("welcome-browsers", &tr!("Link the browsers…")).on_click(
                            cx.listener(|_, _, _, cx| {
                                cx.emit(WelcomeEvent::Settings(Page::Browsers))
                            }),
                        ),
                    ),
            )
    }

    fn voice_step(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let model = authwing_voice::model::by_name(&self.settings.voice.model)
            .unwrap_or(authwing_voice::model::RECOMMENDED);
        let ready = voice::ready_model(&self.settings).is_some();
        let downloading = cx
            .try_global::<Voice>()
            .and_then(|v| v.download.as_ref().map(|d| d.done()));
        let mb = (model.bytes + 500_000) / 1_000_000;
        let state: gpui::AnyElement = match (ready, downloading) {
            (true, _) => div()
                .text_size(ui(12.))
                .text_color(rgb(c.good))
                .child(tr!("Ready: hold the quick panel's key and say a name."))
                .into_any_element(),
            (false, Some(done)) => div()
                .flex()
                .items_center()
                .gap(px(10.))
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
                .child(
                    div()
                        .text_size(ui(12.))
                        .text_color(rgb(c.muted))
                        .child(format!("{:.0} %", done * 100.0)),
                )
                .into_any_element(),
            (false, None) => div()
                .flex()
                .child(
                    primary_button("welcome-voice", &tr!("Download {mb} MB", mb = mb)).on_click(
                        cx.listener(move |_, _, _, cx| {
                            cx.update_global(|v: &mut Voice, cx| v.start_download(model, cx));
                            cx.notify();
                        }),
                    ),
                )
                .into_any_element(),
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(ui(12.)).text_color(rgb(c.soft)).child(tr!(
                "Whisper, the model that hears you, is downloaded once ({mb} MB) and runs on this device: nothing you say leaves it. You can do it later in Settings › Voice.",
                mb = mb
            )))
            .child(state)
    }

    fn key_step(&self, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(div().text_size(ui(12.)).text_color(rgb(c.soft)).child(tr!(
                "The recovery key opens the vault when nothing else can: a lost keychain, a new computer. Write it down, or keep it in a password manager, away from this computer."
            )))
            .child(match &self.key {
                Some(key) => {
                    let copied = key.clone();
                    div()
                        .flex()
                        .flex_col()
                        .items_start()
                        .gap(px(8.))
                        .child(
                            div()
                                .w_full()
                                .px(px(12.))
                                .py(px(10.))
                                .rounded(px(8.))
                                .bg(rgb(c.field))
                                .border_1()
                                .border_color(rgb(c.field_edge))
                                .font_family(crate::look::mono_font(cx))
                                .child(key.clone()),
                        )
                        .child(outline_button("welcome-copy-key", &tr!("Copy")).on_click(
                            cx.listener(move |_, _, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    copied.clone(),
                                ));
                            }),
                        ))
                }
                None => div().flex().child(
                    outline_button("welcome-show-key", &tr!("Show the recovery key"))
                        .on_click(cx.listener(|this, _, _, cx| this.show_key(cx))),
                ),
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(ui(12.))
                            .text_color(rgb(c.soft))
                            .child(tr!(
                                "On your other computers too: the vault syncs through a folder, WebDAV, S3 or Git, encrypted."
                            )),
                    )
                    .child(outline_button("welcome-sync", &tr!("Set up sync…")).on_click(
                        cx.listener(|_, _, _, cx| cx.emit(WelcomeEvent::Settings(Page::Sync))),
                    )),
            )
    }
}

impl Render for Welcome {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let step = STEPS[self.step];
        let (title, lead) = match step {
            Step::Hello => (
                tr!("Welcome to authwing"),
                tr!("Two-factor codes that know the window you are in."),
            ),
            Step::Codes => (
                tr!("Bring your codes"),
                tr!("However many you have: they can all come at once."),
            ),
            Step::SignIn => (
                tr!("Codes where you sign in"),
                tr!("Without opening this window."),
            ),
            Step::Voice => (
                tr!("Find a code by voice"),
                tr!("If you like: it can wait."),
            ),
            Step::Key => (
                tr!("Keep the recovery key"),
                tr!("The one thing to keep safe."),
            ),
        };
        let body = match step {
            Step::Hello => self.hello(),
            Step::Codes => self.codes_step(cx),
            Step::SignIn => self.sign_in(cx),
            Step::Voice => self.voice_step(cx),
            Step::Key => self.key_step(cx),
        };
        let mut dots = div().flex().gap(px(6.));
        for (i, _) in STEPS.iter().enumerate() {
            dots = dots.child(
                div()
                    .id(SharedString::from(format!("welcome-dot-{i}")))
                    .size(px(8.))
                    .rounded_full()
                    .cursor_pointer()
                    .bg(rgb(if i == self.step { c.accent } else { c.edge }))
                    .on_click(cx.listener(move |this, _, _, cx| this.go(i, cx))),
            );
        }
        let last = self.step + 1 == STEPS.len();
        div()
            .size_full()
            .flex()
            .justify_center()
            .items_start()
            .pt(px(40.))
            .child(
                div()
                    .w(px(560.))
                    .flex()
                    .flex_col()
                    .gap(px(18.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(crate::mark::mark(px(28.), window.scale_factor(), cx))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(ui(20.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_size(ui(13.))
                                            .text_color(rgb(c.muted))
                                            .child(lead),
                                    ),
                            ),
                    )
                    .child(body)
                    .child(
                        div()
                            .pt(px(8.))
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(dots)
                            .child(div().flex_1())
                            .when(!last, |d| {
                                d.child(
                                    text_button("welcome-skip", &tr!("Skip"), c.muted).on_click(
                                        cx.listener(|_, _, _, cx| cx.emit(WelcomeEvent::Done)),
                                    ),
                                )
                            })
                            .when(self.step > 0, |d| {
                                d.child(outline_button("welcome-back", &tr!("Back")).on_click(
                                    cx.listener(|this, _, _, cx| this.go(this.step - 1, cx)),
                                ))
                            })
                            .child(
                                primary_button(
                                    "welcome-next",
                                    &if last {
                                        tr!("Done")
                                    } else if self.step == 0 {
                                        tr!("Get started")
                                    } else {
                                        tr!("Next")
                                    },
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.go(this.step + 1, cx))),
                            ),
                    ),
            )
    }
}

/// Whether the welcome has been seen on this device.
pub fn seen(store: Option<&authwing_config::Store>) -> bool {
    store.is_none_or(|s| s.welcome_seen())
}
