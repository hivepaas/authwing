//! The quick panel: a shortcut from any app opens it over that app, with the
//! codes for that app or page first. Enter types the code into the app the
//! user was in; Cmd/Ctrl+C copies it; Escape, or clicking elsewhere, closes.
//!
//! What the panel shows is worked out by the window before it opens (the app
//! in front then, its title, the page the extension says is open): once the
//! panel has the focus, the app in front is authwing.
//!
//! Holding the shortcut, or the microphone button, names a code aloud: the
//! window listens ([`crate::voice`]) and tells the panel what it heard
//! ([`VoiceLine`]), and the codes heard come first.

use std::time::Duration;

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Render,
    SharedString, Window, div, prelude::*, px, rgb, svg,
};

use authwing_i18n::tr;
use authwing_totp::Otp;

use crate::colors::colors;
use crate::input::{Changed, TextInput, said};
use crate::look::ui;

/// A code as the panel lists it.
#[derive(Clone)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub initials: String,
    pub otp: Otp,
    /// Why it matches the app or page, for those that do.
    pub why: Option<String>,
    /// What finding it by typing looks in, lowercase.
    pub search: String,
}

/// What the panel is opened over.
pub struct Over {
    /// The app and its window's title, as shown: "Slack · Sign in to acme".
    pub place: Option<String>,
    /// The app's name, for "Type into Slack".
    pub app: Option<String>,
    /// A page that only looks like a code's site: what it is, and the site.
    pub look_alike: Option<(String, String)>,
    /// Whether a code can be typed into the app: the system lets authwing
    /// send keys, and the app is not authwing itself.
    pub can_type: bool,
    /// How to name a code aloud ("hold ⌥Space and say a name"), when voice
    /// is set up.
    pub voice_hint: Option<String>,
}

/// What voice is doing, under the find field.
pub enum VoiceLine {
    Listening,
    Finding,
    /// What Whisper wrote, and the codes it names, best first; `sure` when
    /// the first was named clearly.
    Heard {
        said: String,
        codes: Vec<String>,
        sure: bool,
    },
    /// Why voice gave nothing, or how to set it up.
    Note(String),
}

pub enum PanelEvent {
    Type(String),
    Copy(String),
    /// The microphone button: listen, or stop listening.
    Listen,
    Close,
}

pub struct QuickPanel {
    over: Over,
    matched: Vec<Item>,
    others: Vec<Item>,
    filter: Entity<TextInput>,
    selected: usize,
    voice: Option<VoiceLine>,
    focus: FocusHandle,
}

impl EventEmitter<PanelEvent> for QuickPanel {}

impl Focusable for QuickPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl QuickPanel {
    pub fn new(
        over: Over,
        matched: Vec<Item>,
        others: Vec<Item>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filter = said(cx, || tr!("Type to find, or press Enter"));
        cx.subscribe(&filter, |this: &mut QuickPanel, _, _: &Changed, cx| {
            this.selected = 0;
            cx.notify();
        })
        .detach();
        window.focus(&filter.focus_handle(cx), cx);
        // Clicking elsewhere closes it, as a menu would.
        cx.observe_window_activation(window, |_, window, cx| {
            if !window.is_window_active() {
                cx.emit(PanelEvent::Close);
            }
        })
        .detach();
        // The codes move on, and the seconds count down.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }
        })
        .detach();
        QuickPanel {
            over,
            matched,
            others,
            filter,
            selected: 0,
            voice: None,
            focus: cx.focus_handle(),
        }
    }

    /// What voice is doing, from the window.
    pub fn set_voice(&mut self, line: VoiceLine, cx: &mut Context<Self>) {
        if matches!(line, VoiceLine::Heard { .. }) {
            // The codes heard come first, unfiltered.
            self.filter.update(cx, |f, cx| f.set_text("", 0..0, cx));
        }
        self.selected = 0;
        self.voice = Some(line);
        cx.notify();
    }

    /// The codes shown: those heard, those that match the window, the
    /// others; all that match what is typed, when something is. With how
    /// many were heard and how many match.
    fn shown(&self, cx: &App) -> (Vec<&Item>, usize, usize) {
        let words: Vec<String> = self
            .filter
            .read(cx)
            .text()
            .to_lowercase()
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let heard: Vec<&Item> = match &self.voice {
            Some(VoiceLine::Heard { codes, .. }) if words.is_empty() => codes
                .iter()
                .filter_map(|id| {
                    self.matched
                        .iter()
                        .chain(&self.others)
                        .find(|i| i.id == *id)
                })
                .collect(),
            _ => Vec::new(),
        };
        let heard_ids: Vec<String> = heard.iter().map(|h| h.id.clone()).collect();
        let fits = |i: &&Item| {
            words.iter().all(|w| i.search.contains(w.as_str())) && !heard_ids.contains(&i.id)
        };
        let matched: Vec<&Item> = self.matched.iter().filter(fits).collect();
        let others: Vec<&Item> = self.others.iter().filter(fits).collect();
        let (h, m) = (heard.len(), matched.len());
        let mut all = heard;
        all.extend(matched);
        all.extend(others);
        (all, h, m)
    }

    fn chosen(&self, cx: &App) -> Option<String> {
        let (all, _, _) = self.shown(cx);
        all.get(self.selected).map(|i| i.id.clone())
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let cmd = if cfg!(target_os = "macos") {
            k.modifiers.platform
        } else {
            k.modifiers.control
        };
        let count = self.shown(cx).0.len();
        match k.key.as_str() {
            "escape" => cx.emit(PanelEvent::Close),
            "down" if count > 0 => self.selected = (self.selected + 1).min(count - 1),
            "up" => self.selected = self.selected.saturating_sub(1),
            "enter" => {
                if let Some(id) = self.chosen(cx) {
                    cx.emit(if self.over.can_type {
                        PanelEvent::Type(id)
                    } else {
                        PanelEvent::Copy(id)
                    });
                }
            }
            "c" if cmd => {
                if let Some(id) = self.chosen(cx) {
                    cx.emit(PanelEvent::Copy(id));
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
    }

    fn row(&self, item: &Item, index: usize, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let c = colors();
        let now = crate::codes::now();
        let on = index == self.selected;
        let code = authwing_totp::grouped(&item.otp.code_at(now));
        let left = item.otp.seconds_left(now);
        let ending = left.is_some_and(|l| l <= 5);
        let id = item.id.clone();
        let can_type = self.over.can_type;
        div()
            .id(SharedString::from(format!("quick-{}", item.id)))
            .h(px(48.))
            .px(px(14.))
            .flex()
            .items_center()
            .gap(px(12.))
            .cursor_pointer()
            .when(on, |d| d.bg(rgb(c.selected)))
            .when(!on, |d| d.hover(|s| s.bg(rgb(c.hover))))
            .on_click(cx.listener(move |_, _, _, cx| {
                cx.emit(if can_type {
                    PanelEvent::Type(id.clone())
                } else {
                    PanelEvent::Copy(id.clone())
                })
            }))
            .child(
                div()
                    .size(px(28.))
                    .flex_shrink_0()
                    .rounded(px(7.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(c.chip))
                    .text_size(ui(10.))
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(c.soft))
                    .child(item.initials.clone()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .truncate()
                            .when(on, |d| d.font_weight(FontWeight::SEMIBOLD))
                            .child(item.title.clone()),
                    )
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.muted))
                            .truncate()
                            .child(match &item.why {
                                Some(why) if !item.detail.is_empty() => {
                                    format!("{} · {why}", item.detail)
                                }
                                Some(why) => why.clone(),
                                None => item.detail.clone(),
                            }),
                    ),
            )
            .child(
                div()
                    .text_size(ui(19.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if ending { c.warn } else { c.text }))
                    .child(code),
            )
            .child(
                div()
                    .w(px(30.))
                    .text_size(ui(11.))
                    .text_color(rgb(c.faint))
                    .child(left.map(|l| format!("{l} s")).unwrap_or_default()),
            )
    }
}

/// What voice is doing, as said under the find field; whether it is
/// listening.
fn voice_line(line: &VoiceLine) -> (String, bool) {
    match line {
        VoiceLine::Listening => (tr!("Listening… say the code's name."), true),
        VoiceLine::Finding => (tr!("Finding what you said…"), false),
        VoiceLine::Heard { said, codes, sure } => (
            match (codes.is_empty(), sure) {
                (true, _) => tr!(
                    "Heard “{said}”, which names no code. Choose the one you meant: next time it will be found.",
                    said = said.clone()
                ),
                (false, true) => tr!("Heard “{said}”.", said = said.clone()),
                (false, false) => tr!(
                    "Heard “{said}”: which one? The one you choose is found by it next time.",
                    said = said.clone()
                ),
            },
            false,
        ),
        VoiceLine::Note(note) => (note.clone(), false),
    }
}

fn section(title: String) -> gpui::Div {
    div()
        .px(px(14.))
        .pt(px(10.))
        .pb(px(4.))
        .text_size(ui(11.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(colors().muted))
        .child(title)
}

impl Render for QuickPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let (all, heard, matched) = self.shown(cx);
        let ids: Vec<Item> = all.into_iter().cloned().collect();
        if self.selected >= ids.len() {
            self.selected = ids.len().saturating_sub(1);
        }
        let typing = !self.filter.read(cx).text().trim().is_empty();
        let said = match &self.voice {
            Some(VoiceLine::Heard { said, .. }) => said.clone(),
            _ => String::new(),
        };
        let mut list = div().flex().flex_col();
        for (i, item) in ids.iter().enumerate() {
            if i == 0 && heard > 0 {
                list = list.child(section(tr!("You said “{said}”", said = said.clone())));
            }
            if i == heard && matched > 0 && !typing {
                list = list.child(section(tr!("Matches this window")));
            }
            if i == heard + matched && heard + matched > 0 && !typing {
                list = list.child(section(tr!("Others")));
            }
            list = list.child(self.row(item, i, cx));
        }
        if ids.is_empty() {
            let none = if self.matched.is_empty() && self.others.is_empty() {
                tr!("No codes yet: the + in authwing's window adds them.")
            } else {
                tr!("No code matches.")
            };
            list = list.child(div().p(px(18.)).text_color(rgb(c.muted)).child(none));
        }
        let context = match (&self.over.look_alike, &self.over.place) {
            (Some((page, like)), _) => Some(
                crate::notice::colored(div(), crate::notice::Tone::Bad)
                    .border_0()
                    .border_b_1()
                    .px(px(14.))
                    .py(px(9.))
                    .text_size(ui(12.))
                    .child(tr!(
                        "{page} is not {site}: no code is typed there.",
                        page = page.clone(),
                        site = like.clone()
                    )),
            ),
            (None, Some(place)) => Some(
                div()
                    .px(px(14.))
                    .py(px(9.))
                    .text_size(ui(12.))
                    .bg(rgb(c.info_bg))
                    .text_color(rgb(c.info_text))
                    .border_b_1()
                    .border_color(rgb(c.line))
                    .child(place.clone()),
            ),
            (None, None) => None,
        };
        let listening = matches!(self.voice, Some(VoiceLine::Listening));
        let enter = match (&self.over.app, self.over.can_type) {
            (Some(app), true) => tr!("type into {app}", app = app.clone()),
            _ => tr!("copy"),
        };
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(c.raised))
            .border_1()
            .border_color(rgb(c.edge))
            .rounded(px(12.))
            .overflow_hidden()
            .text_size(ui(13.))
            .text_color(rgb(c.text))
            .children(context)
            .child(
                div()
                    .h(px(46.))
                    .flex_shrink_0()
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(rgb(c.line))
                    .child(
                        svg()
                            .path("icons/search.svg")
                            .size(px(16.))
                            .text_color(rgb(c.muted)),
                    )
                    .child(div().flex_1().text_size(ui(15.)).child(self.filter.clone()))
                    .child(
                        div()
                            .id("quick-mic")
                            .size(px(28.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(rgb(c.hover)))
                            .when(listening, |d| d.bg(rgb(c.selected)))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(PanelEvent::Listen)))
                            .child(
                                svg()
                                    .path("icons/mic.svg")
                                    .size(px(16.))
                                    .text_color(rgb(if listening { c.accent } else { c.muted })),
                            ),
                    ),
            )
            .when_some(self.voice.as_ref().map(voice_line), |d, (text, on)| {
                d.child(
                    div()
                        .px(px(14.))
                        .py(px(7.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .border_b_1()
                        .border_color(rgb(c.line))
                        .text_size(ui(12.))
                        .text_color(rgb(if on { c.text } else { c.muted }))
                        .when(on, |d| {
                            d.child(div().size(px(8.)).rounded_full().bg(rgb(c.bad)))
                        })
                        .child(div().flex_1().child(text)),
                )
            })
            .child(
                div()
                    .id("quick-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(list),
            )
            .child(
                div()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px(px(14.))
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .border_t_1()
                    .border_color(rgb(c.line))
                    .bg(rgb(c.surface))
                    .text_size(ui(11.))
                    .text_color(rgb(c.muted))
                    .child(format!("⏎ {enter}"))
                    .child(format!(
                        "{} {}",
                        if cfg!(target_os = "macos") {
                            "⌘C"
                        } else {
                            "Ctrl+C"
                        },
                        tr!("copy")
                    ))
                    .child(format!("Esc {}", tr!("close")))
                    .when(!self.over.can_type && self.over.app.is_some(), |d| {
                        d.child(div().flex_1())
                            .child(tr!("Allow Accessibility to type codes in"))
                    })
                    .when(self.over.can_type || self.over.app.is_none(), |d| {
                        d.when_some(self.over.voice_hint.clone(), |d, hint| {
                            d.child(div().flex_1()).child(hint)
                        })
                    }),
            )
    }
}
