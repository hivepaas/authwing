//! Joining, from this device, the vault a place holds already: by asking a
//! device that syncs it, both showing the same six digits (see
//! `authwing_vault::Ask`), or with the vault's recovery key. Settings › Sync
//! shows it while the place is set and this device does not sync there yet.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, Render, SharedString,
    Window, div, prelude::*, px, rgb,
};

use authwing_config::SyncPlace;
use authwing_i18n::tr;
use authwing_vault::{AskState, transact};

use crate::colors::colors;
use crate::input::TextInput;
use crate::look::ui;
use crate::syncing::{self, SyncWorker};
use crate::vault_slot::VaultSlot;
use crate::widgets::{outline_button, text_button};

pub enum JoinEvent {
    /// This device joined the vault there: its codes are others now.
    Joined,
}

pub struct JoinView {
    sync: SyncWorker,
    vault: VaultSlot,
    asking: Option<AskState>,
    /// Whether the sync passphrase joins new devices there too.
    passphrase_joins: bool,
    problem: Option<String>,
    joining: bool,
    recovery: Entity<TextInput>,
    focus: FocusHandle,
}

impl EventEmitter<JoinEvent> for JoinView {}

impl Focusable for JoinView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl JoinView {
    pub fn new(sync: SyncWorker, vault: VaultSlot, cx: &mut Context<Self>) -> Self {
        JoinView {
            sync,
            vault,
            asking: None,
            passphrase_joins: false,
            problem: None,
            joining: false,
            recovery: cx.new(|cx| TextInput::said(|| tr!("XXXX-XXXX-…, 13 groups"), cx)),
            focus: cx.focus_handle(),
        }
    }

    pub fn set_asking(&mut self, state: AskState, cx: &mut Context<Self>) {
        if state == AskState::Joined {
            cx.emit(JoinEvent::Joined);
        }
        self.asking = Some(state);
        cx.notify();
    }

    pub fn set_passphrase_joins(&mut self, joins: bool, cx: &mut Context<Self>) {
        self.passphrase_joins = joins;
        cx.notify();
    }

    pub fn failed(&mut self, why: String, cx: &mut Context<Self>) {
        self.problem = Some(why);
        cx.notify();
    }

    /// Joins the vault at the place with the recovery key typed: this
    /// device's codes give way to that vault's.
    fn join(&mut self, cx: &mut Context<Self>) {
        let key = self.recovery.read(cx).text().trim().to_string();
        let (Some(place), Some(vault)) = (self.sync.place(), self.vault.get()) else {
            return;
        };
        if key.is_empty() || self.joining {
            return;
        }
        self.joining = true;
        self.problem = None;
        let joined = cx.background_spawn(async move {
            let backend = syncing::open(&place, &vault).map_err(|e| e.to_string())?;
            transact(backend.as_ref(), |b| vault.join(b, &key)).map_err(|e| e.to_string())
        });
        cx.spawn(async move |this, cx| {
            let result = joined.await;
            let _ = this.update(cx, |view, cx| {
                view.joining = false;
                match result {
                    Ok(()) => {
                        view.recovery.update(cx, |f, cx| f.set_text("", 0..0, cx));
                        view.sync.now();
                        cx.emit(JoinEvent::Joined);
                    }
                    Err(e) => view.problem = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn asking_view(&self, place: &SyncPlace, cx: &mut Context<Self>) -> gpui::Div {
        let c = colors();
        let line = |t: String| div().text_size(ui(13.)).text_color(rgb(c.soft)).child(t);
        let place = place.clone();
        match &self.asking {
            Some(AskState::Waiting) => div().flex().flex_col().gap(px(8.)).child(line(tr!(
                "Waiting for a device that syncs: open authwing there."
            ))),
            Some(AskState::Compare { device, code }) => div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(line(tr!(
                    "Approve on {device} if it shows the same code:",
                    device = device.clone()
                )))
                .child(
                    div()
                        .text_size(ui(24.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(c.warn))
                        .child(authwing_totp::grouped(code)),
                ),
            Some(AskState::Joined) => line(tr!("Joined. The vault's codes are coming.")),
            Some(AskState::Refused) => line(tr!("The other device refused.")),
            Some(AskState::Expired) => line(tr!("Nobody approved in time: ask again.")),
            // A row, so the button is as wide as its words.
            None => div().flex().child(
                outline_button("ask-join", &tr!("Ask a device that syncs")).on_click(cx.listener(
                    move |view, _, _, cx| {
                        view.problem = None;
                        view.asking = Some(AskState::Waiting);
                        view.sync.ask_to_join(place.clone());
                        cx.notify();
                    },
                )),
            ),
        }
        .when(
            matches!(
                self.asking,
                Some(AskState::Waiting | AskState::Compare { .. })
            ),
            |d| {
                d.child(div().flex().child(
                    text_button("ask-cancel", &tr!("Stop asking"), c.soft).on_click(cx.listener(
                        |view, _, _, cx| {
                            view.sync.cancel_ask();
                            view.asking = None;
                            cx.notify();
                        },
                    )),
                ))
            },
        )
    }
}

impl Render for JoinView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let Some(place) = self.sync.place() else {
            return div();
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(tr!("It holds a vault from another device: join it")),
            )
            .child(
                div()
                    .text_size(ui(12.))
                    .text_color(rgb(c.muted))
                    .child(if self.passphrase_joins {
                        tr!(
                            "Ask a device that syncs, or type the vault's recovery key. Its sync passphrase works too."
                        )
                    } else {
                        tr!("Ask a device that syncs, or type the vault's recovery key.")
                    }),
            )
            .child(self.asking_view(&place, cx))
            .child(
                div()
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
                            .child(self.recovery.clone()),
                    )
                    .child(
                        outline_button(
                            "join",
                            &if self.joining {
                                tr!("Joining…")
                            } else {
                                tr!("Join with the key")
                            },
                        )
                        .on_click(cx.listener(|view, _, _, cx| view.join(cx))),
                    ),
            )
            .when_some(self.problem.clone(), |d, p| {
                d.child(
                    div()
                        .text_size(ui(12.))
                        .text_color(rgb(c.bad))
                        .child(SharedString::from(p)),
                )
            })
    }
}
