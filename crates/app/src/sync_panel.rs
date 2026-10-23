//! The panel the cloud opens: where the vault syncs and how it went, and
//! devices joining it. A new device joins with the recovery key, or by asking
//! a device that syncs, both showing the same six digits (see
//! `authwing_vault::Ask`); a device that syncs approves those that ask.
//!
//! Where it syncs is set in config.toml's `[sync]` for now.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, FontWeight, KeyDownEvent, Render,
    SharedString, Window, div, prelude::*, px, rgb,
};

use authwing_config::SyncPlace;
use authwing_i18n::tr;
use authwing_vault::{AskState, Request, transact};

use crate::colors::colors;
use crate::input::TextInput;
use crate::look::ui;
use crate::syncing::{self, SyncWorker};
use crate::vault_slot::VaultSlot;
use crate::widgets::{outline_button, primary_button, text_button};

pub enum PanelEvent {
    Close,
    /// This device joined the vault there: its codes are others now.
    Joined,
}

pub struct SyncPanel {
    sync: SyncWorker,
    vault: VaultSlot,
    /// How the last sync went, as the window says it.
    status: String,
    asking: Option<AskState>,
    requests: Vec<Request>,
    /// Whether the sync passphrase joins new devices there too.
    passphrase_joins: bool,
    problem: Option<String>,
    joining: bool,
    recovery: Entity<TextInput>,
    focus: FocusHandle,
}

impl EventEmitter<PanelEvent> for SyncPanel {}

impl Focusable for SyncPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl SyncPanel {
    pub fn new(sync: SyncWorker, vault: VaultSlot, cx: &mut Context<Self>) -> Self {
        SyncPanel {
            sync,
            vault,
            status: String::new(),
            asking: None,
            requests: vec![],
            passphrase_joins: false,
            problem: None,
            joining: false,
            recovery: cx.new(|cx| TextInput::said(|| tr!("XXXX-XXXX-…, 13 groups"), cx)),
            focus: cx.focus_handle(),
        }
    }

    pub fn set_status(&mut self, status: String, cx: &mut Context<Self>) {
        self.status = status;
        cx.notify();
    }

    pub fn set_asking(&mut self, state: AskState, cx: &mut Context<Self>) {
        if state == AskState::Joined {
            cx.emit(PanelEvent::Joined);
        }
        self.asking = Some(state);
        cx.notify();
    }

    pub fn set_requests(&mut self, requests: Vec<Request>, cx: &mut Context<Self>) {
        self.requests = requests;
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
            let _ = this.update(cx, |panel, cx| {
                panel.joining = false;
                match result {
                    Ok(()) => {
                        panel.recovery.update(cx, |f, cx| f.set_text("", 0..0, cx));
                        panel.sync.now();
                        cx.emit(PanelEvent::Joined);
                    }
                    Err(e) => panel.problem = Some(e),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if ev.keystroke.key == "escape" {
            cx.emit(PanelEvent::Close);
            cx.stop_propagation();
        }
    }

    fn section(title: String) -> gpui::Div {
        div()
            .pt(px(6.))
            .text_size(ui(11.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(colors().muted))
            .child(title)
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
            None => div().child(
                outline_button("ask-join", &tr!("Ask a device that syncs")).on_click(cx.listener(
                    move |panel, _, _, cx| {
                        panel.problem = None;
                        panel.asking = Some(AskState::Waiting);
                        panel.sync.ask_to_join(place.clone());
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
                d.child(
                    text_button("ask-cancel", &tr!("Stop asking"), c.soft).on_click(cx.listener(
                        |panel, _, _, cx| {
                            panel.sync.cancel_ask();
                            panel.asking = None;
                            cx.notify();
                        },
                    )),
                )
            },
        )
    }
}

impl Render for SyncPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = colors();
        let place = self.sync.place();
        let mut panel = div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .w(px(520.))
            .flex()
            .flex_col()
            .gap(px(12.))
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
                    .child(tr!("Sync")),
            );
        let Some(place) = place else {
            return panel
                .child(
                    div()
                        .text_size(ui(13.))
                        .text_color(rgb(c.soft))
                        .child(tr!(
                            "Not syncing. Say where in config.toml, then come back here: a folder that syncs (iCloud Drive, Dropbox, Syncthing), WebDAV, S3 or Git."
                        )),
                )
                .child(
                    div()
                        .p(px(10.))
                        .rounded(px(6.))
                        .bg(rgb(c.field))
                        .text_size(ui(12.))
                        .text_color(rgb(c.soft))
                        .child("[sync]\nfolder = \"~/Library/Mobile Documents/com~apple~CloudDocs/authwing\"".to_string()),
                )
                .child(
                    div().flex().justify_end().child(
                        outline_button("sync-close", &tr!("Close"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(PanelEvent::Close))),
                    ),
                );
        };
        panel = panel
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(div().text_size(ui(13.)).child(syncing::describe(&place)))
                    .child(
                        div()
                            .text_size(ui(12.))
                            .text_color(rgb(c.muted))
                            .child(self.status.clone()),
                    ),
            )
            .child(div().text_size(ui(12.)).text_color(rgb(c.muted)).child(tr!(
                "Only encrypted records are stored there: not even the services' names can be read."
            )));
        if !self.requests.is_empty() {
            panel = panel.child(Self::section(tr!("Devices asking to join")));
            for r in &self.requests {
                let (yes, no) = (r.id.clone(), r.id.clone());
                panel =
                    panel.child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .child(r.device.clone())
                                    .child(
                                        div()
                                            .text_size(ui(12.))
                                            .text_color(rgb(c.muted))
                                            .child(tr!("Approve only if it shows this code.")),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(ui(20.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(rgb(c.warn))
                                    .child(authwing_totp::grouped(&r.code)),
                            )
                            .child(
                                primary_button(&format!("approve-{}", r.id), &tr!("Approve"))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.sync.approve(yes.clone());
                                        cx.notify();
                                    })),
                            )
                            .child(
                                outline_button(&format!("refuse-{}", r.id), &tr!("Refuse"))
                                    .on_click(cx.listener(move |panel, _, _, cx| {
                                        panel.sync.refuse(no.clone());
                                        cx.notify();
                                    })),
                            ),
                    );
            }
        }
        panel = panel
            .child(Self::section(tr!("Join from this device")))
            .child(
                div()
                    .text_size(ui(12.))
                    .text_color(rgb(c.muted))
                    .child(if self.passphrase_joins {
                        tr!(
                            "When another vault syncs there already: ask a device that syncs, or type the vault's recovery key. Its sync passphrase works too."
                        )
                    } else {
                        tr!(
                            "When another vault syncs there already: ask a device that syncs, or type the vault's recovery key."
                        )
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
                        .on_click(cx.listener(|panel, _, _, cx| panel.join(cx))),
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
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap(px(8.))
                    .child(
                        outline_button("sync-now", &tr!("Sync now")).on_click(cx.listener(
                            |panel, _, _, cx| {
                                panel.sync.now();
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        primary_button("sync-close", &tr!("Close"))
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(PanelEvent::Close))),
                    ),
            );
        panel
    }
}
