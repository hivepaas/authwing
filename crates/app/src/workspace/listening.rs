//! The quick panel's shortcut held down, or its microphone button, to name
//! a code aloud.
//!
//! A tap opens the panel, or closes it when it was open. Held past
//! [`HOLD`], the microphone records until the key comes up; then Whisper
//! writes down what was said and the codes it names come first in the
//! panel. The microphone is opened only then, not on every press: the
//! system shows when it is on, and it should be on only when asked.
//!
//! What the user then chooses by hand, when voice was not sure, is taught
//! what was heard (`state/usage.json`): heard again, it is found outright.

use std::time::Duration;

use gpui::Context;

use authwing_i18n::tr;
use authwing_voice::{Candidate, Decision};

use super::Workspace;
use crate::quick_panel::VoiceLine;
use crate::voice::{self, Voice};

/// A press held this long is speech, not a tap.
const HOLD: Duration = Duration::from_millis(250);

/// Taught names kept for one code, the newest.
const TAUGHT: usize = 8;

/// The quick panel's shortcut, while it is down.
pub(super) struct Held {
    /// The panel was open before: a tap closes it.
    was_open: bool,
    /// Held past [`HOLD`]: not a tap.
    long: bool,
}

/// What voice heard last in the quick panel: what Whisper wrote, and the
/// code it picked, if it was sure.
pub(super) struct LastHeard {
    said: String,
    picked: Option<String>,
}

impl Workspace {
    /// The quick panel's shortcut went down (`true`) or up.
    pub fn panel_key(&mut self, down: bool, cx: &mut Context<Self>) {
        if down {
            self.panel_key_down(cx);
        } else {
            self.panel_key_up(cx);
        }
    }

    fn panel_key_down(&mut self, cx: &mut Context<Self>) {
        // The system repeats a held key.
        if self.held.is_some() {
            return;
        }
        let was_open = self.quick.is_some();
        if !was_open {
            self.toggle_quick(cx);
            if self.quick.is_none() {
                return;
            }
        }
        self.held = Some(Held {
            was_open,
            long: false,
        });
        self.presses += 1;
        let press = self.presses;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(HOLD).await;
            let _ = this.update(cx, |ws, cx| {
                if ws.presses != press {
                    return;
                }
                if let Some(held) = &mut ws.held {
                    held.long = true;
                    ws.start_listening(cx);
                }
            });
        })
        .detach();
    }

    fn panel_key_up(&mut self, cx: &mut Context<Self>) {
        let Some(held) = self.held.take() else {
            return;
        };
        if self.listening {
            self.stop_listening(cx);
        } else if held.was_open && !held.long {
            self.close_quick(true, cx);
        }
    }

    /// The panel's microphone button.
    pub(super) fn toggle_listening(&mut self, cx: &mut Context<Self>) {
        if self.listening {
            self.stop_listening(cx);
        } else {
            self.start_listening(cx);
        }
    }

    fn start_listening(&mut self, cx: &mut Context<Self>) {
        if self.listening || self.quick.is_none() || !cx.has_global::<Voice>() {
            return;
        }
        let Some(model) = voice::ready_model(self.live.settings()) else {
            let note = if cx.global::<Voice>().download.is_some() {
                tr!("Voice's model is still downloading (Settings › Voice).")
            } else {
                tr!("Voice is off: download its model in Settings › Voice.")
            };
            self.tell_panel(VoiceLine::Note(note), cx);
            return;
        };
        let voice = cx.global::<Voice>();
        // Loaded while the user speaks.
        voice.warm(&model);
        let started = voice.listen();
        self.listening = true;
        self.tell_panel(VoiceLine::Listening, cx);
        cx.spawn(async move |this, cx| {
            if let Ok(Err(e)) = started.recv().await {
                let _ = this.update(cx, |ws, cx| {
                    ws.listening = false;
                    ws.tell_panel(
                        VoiceLine::Note(tr!("The microphone could not be used: {e}", e = e)),
                        cx,
                    );
                });
            }
        })
        .detach();
    }

    fn stop_listening(&mut self, cx: &mut Context<Self>) {
        if !self.listening {
            return;
        }
        self.listening = false;
        let settings = self.live.settings();
        let Some(model) = voice::ready_model(settings) else {
            cx.global::<Voice>().cancel();
            return;
        };
        let language = voice::language(settings);
        let candidates = self.voice_candidates();
        self.tell_panel(VoiceLine::Finding, cx);
        let recorded = cx.global::<Voice>().stop();
        cx.spawn(async move |this, cx| {
            let pcm = recorded.recv().await.unwrap_or_default();
            if authwing_voice::is_silent(&pcm) {
                let _ = this.update(cx, |ws, cx| {
                    ws.tell_panel(VoiceLine::Note(nothing_heard()), cx);
                });
                return;
            }
            let answer =
                cx.update(|cx| cx.global::<Voice>().hear(&model, pcm, language, candidates));
            let Ok(heard) = answer.recv().await else {
                return;
            };
            let _ = this.update(cx, |ws, cx| {
                let line = match heard {
                    Ok(heard) => {
                        let said = heard.transcript.clone();
                        let (codes, sure) = match voice::decide(&heard) {
                            Decision::Pick(id) => (vec![id], true),
                            Decision::Choose(ids) => (ids, false),
                            Decision::Nothing => (Vec::new(), false),
                        };
                        ws.last_heard = Some(LastHeard {
                            said: said.clone(),
                            picked: sure.then(|| codes[0].clone()),
                        });
                        VoiceLine::Heard { said, codes, sure }
                    }
                    Err(e) => VoiceLine::Note(tr!("Voice: {e}", e = e)),
                };
                ws.tell_panel(line, cx);
            });
        })
        .detach();
    }

    /// Stops listening, keeping nothing: the panel closed.
    pub(super) fn cancel_listening(&mut self, cx: &mut Context<Self>) {
        self.last_heard = None;
        if self.listening {
            self.listening = false;
            if cx.has_global::<Voice>() {
                cx.global::<Voice>().cancel();
            }
        }
    }

    fn tell_panel(&self, line: VoiceLine, cx: &mut Context<Self>) {
        if let Some((panel, _)) = &self.quick {
            let _ = panel.update(cx, |panel, _, cx| panel.set_voice(line, cx));
        }
    }

    /// Every code, as voice can name it.
    fn voice_candidates(&self) -> Vec<Candidate<String>> {
        self.codes
            .iter()
            .map(|e| {
                let mut c = Candidate::new(
                    e.id.clone(),
                    voice::names(&e.otp.issuer, &e.otp.account, &e.parsed),
                );
                if let Some(known) = self.usage.codes.get(&e.id) {
                    c.taught = known.taught.clone();
                    c.uses = known.uses;
                }
                c
            })
            .collect()
    }

    /// The code `id` was used: counted and timed, and taught what voice
    /// heard last if the user chose it over what voice picked.
    pub(super) fn used(&mut self, id: &str) {
        let known = self.usage.codes.entry(id.to_string()).or_default();
        known.uses = known.uses.saturating_add(1);
        known.last = crate::codes::now();
        if let Some(heard) = self.last_heard.take()
            && heard.picked.as_deref() != Some(id)
            && !heard.said.trim().is_empty()
        {
            known.taught.retain(|t| t != &heard.said);
            known.taught.push(heard.said);
            let over = known.taught.len().saturating_sub(TAUGHT);
            known.taught.drain(..over);
        }
        // Codes deleted since are forgotten.
        self.usage
            .codes
            .retain(|id, _| self.codes.iter().any(|e| e.id == *id));
        if let Some(store) = self.live.store()
            && let Err(e) = store.save_usage(&self.usage)
        {
            eprintln!("authwing: usage not saved: {e}");
        }
    }
}

/// What to say when the microphone gave silence: on macOS, most likely
/// not allowed yet.
fn nothing_heard() -> String {
    if cfg!(target_os = "macos") {
        tr!(
            "Nothing was heard. Is the microphone allowed? System Settings › Privacy & Security › Microphone."
        )
    } else {
        tr!("Nothing was heard: is the microphone on?")
    }
}
