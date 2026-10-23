//! Adding codes: from the link a site's QR code holds (`otpauth://…`), from
//! Google Authenticator's export (`otpauth-migration://…`, many at once), or
//! from the key a site shows to type when its QR code cannot be scanned.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Render,
    Window, div, prelude::*, px, rgb,
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
    Close,
}

pub struct AddCode {
    link: Entity<TextInput>,
    issuer: Entity<TextInput>,
    account: Entity<TextInput>,
    group: Entity<TextInput>,
    sites: Entity<TextInput>,
    problem: Option<String>,
    focus: FocusHandle,
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
    pub fn new(group: Option<&str>, cx: &mut Context<Self>) -> Self {
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
            focus: cx.focus_handle(),
        }
    }

    /// The link field's focus, where the cursor goes first.
    pub fn link_focus(&self, cx: &App) -> FocusHandle {
        self.link.focus_handle(cx)
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

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        match ev.keystroke.key.as_str() {
            "enter" => self.submit(cx),
            "escape" => cx.emit(AddEvent::Close),
            _ => return,
        }
        cx.stop_propagation();
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
        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
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
                    .child(tr!("Add a code")),
            )
            .child(
                div()
                    .text_size(ui(12.))
                    .text_color(rgb(c.muted))
                    .child(tr!(
                        "When a site turns on two-factor sign-in it shows a QR code, and a link or a key beside it. Paste either here; a QR code from your phone's camera gives the link too."
                    )),
            )
            .child(field(tr!("Link or key"), &self.link))
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
                        primary_button("add-save", &tr!("Add"))
                            .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
                    ),
            )
    }
}
