//! Voice in the quick panel: hold its shortcut and say a code's name.
//!
//! Two threads do the work, so the window never waits on it. One keeps the
//! microphone: a recording is started and stopped on the thread that made
//! it, as some systems want. The other keeps Whisper's model, loaded when
//! first needed (it is warmed as the user starts speaking) and let go after
//! [`IDLE`] unused, as it holds a few hundred MB.
//!
//! The model is downloaded from Settings › Voice, into
//! [`Store::models_dir`]; [`Voice`] keeps the download's progress for the
//! views to show.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Duration;

use authwing_config::{Settings, Store};
use authwing_voice::model::Model;
use authwing_voice::whisper::{Heard, Transcriber};
use authwing_voice::{Candidate, LANGUAGES, Thresholds, language_for_locale, system_language};
use gpui::BorrowAppContext as _;

/// A model unused this long is let go.
const IDLE: Duration = Duration::from_secs(10 * 60);

/// How often a download's progress is drawn.
const DRAW_EVERY: Duration = Duration::from_millis(250);

enum MicJob {
    Start(async_channel::Sender<Result<(), String>>),
    Stop(async_channel::Sender<Vec<f32>>),
    Drop,
}

enum EarJob {
    Warm(PathBuf),
    Hear {
        model: PathBuf,
        pcm: Vec<f32>,
        language: String,
        candidates: Vec<Candidate<String>>,
        reply: async_channel::Sender<Result<Heard<String>, String>>,
    },
}

/// A model on its way.
pub struct Download {
    pub model: Model,
    /// Bytes so far.
    got: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
}

impl Download {
    /// How far it is, from 0 to 1.
    pub fn done(&self) -> f64 {
        self.got.load(Ordering::Relaxed) as f64 / self.model.bytes.max(1) as f64
    }
}

pub struct Voice {
    mic: mpsc::Sender<MicJob>,
    ear: mpsc::Sender<EarJob>,
    pub download: Option<Download>,
    /// Why the last download failed, until the next one.
    pub failed: Option<String>,
}

impl gpui::Global for Voice {}

impl Voice {
    pub fn new() -> Voice {
        let (mic, jobs) = mpsc::channel();
        std::thread::Builder::new()
            .name("authwing-mic".into())
            .spawn(move || mic_thread(jobs))
            .ok();
        let (ear, jobs) = mpsc::channel();
        std::thread::Builder::new()
            .name("authwing-whisper".into())
            .spawn(move || ear_thread(jobs))
            .ok();
        Voice {
            mic,
            ear,
            download: None,
            failed: None,
        }
    }

    /// Starts recording; says whether the microphone could be opened.
    pub fn listen(&self) -> async_channel::Receiver<Result<(), String>> {
        let (reply, answer) = async_channel::bounded(1);
        let _ = self.mic.send(MicJob::Start(reply));
        answer
    }

    /// Stops recording; what was heard, at Whisper's rate.
    pub fn stop(&self) -> async_channel::Receiver<Vec<f32>> {
        let (reply, answer) = async_channel::bounded(1);
        let _ = self.mic.send(MicJob::Stop(reply));
        answer
    }

    /// Stops recording, keeping nothing.
    pub fn cancel(&self) {
        let _ = self.mic.send(MicJob::Drop);
    }

    /// Loads `model` now, so that it is ready when the user stops speaking.
    pub fn warm(&self, model: &Path) {
        let _ = self.ear.send(EarJob::Warm(model.to_owned()));
    }

    /// What `pcm` names among `candidates`.
    pub fn hear(
        &self,
        model: &Path,
        pcm: Vec<f32>,
        language: String,
        candidates: Vec<Candidate<String>>,
    ) -> async_channel::Receiver<Result<Heard<String>, String>> {
        let (reply, answer) = async_channel::bounded(1);
        let _ = self.ear.send(EarJob::Hear {
            model: model.to_owned(),
            pcm,
            language,
            candidates,
            reply,
        });
        answer
    }

    /// Downloads `model`, drawing its progress until it is there.
    pub fn start_download(&mut self, model: Model, cx: &mut gpui::App) {
        let Some(dir) = Store::models_dir() else {
            self.failed = Some("no folder for models".into());
            return;
        };
        if self.download.is_some() {
            return;
        }
        let got = Arc::new(AtomicU64::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        self.failed = None;
        self.download = Some(Download {
            model,
            got: got.clone(),
            stop: stop.clone(),
        });
        // A thread of its own: the download blocks it for minutes.
        let (done, finished) = async_channel::bounded(1);
        std::thread::Builder::new()
            .name("authwing-model-download".into())
            .spawn(move || {
                let fetched = authwing_voice::download::download(&model, &dir, &stop, |n| {
                    got.store(n, Ordering::Relaxed);
                });
                let _ = done.send_blocking(fetched);
            })
            .ok();
        cx.spawn(async move |cx| {
            let result = loop {
                cx.background_executor().timer(DRAW_EVERY).await;
                match finished.try_recv() {
                    Ok(result) => break result.map(|_| ()),
                    Err(async_channel::TryRecvError::Empty) => cx.update(|cx| cx.refresh_windows()),
                    Err(async_channel::TryRecvError::Closed) => {
                        break Err(authwing_voice::download::Error::Fetch(
                            "the download stopped".into(),
                        ));
                    }
                }
            };
            cx.update(|cx| {
                cx.update_global(|voice: &mut Voice, _| {
                    voice.download = None;
                    voice.failed = match result {
                        Ok(()) | Err(authwing_voice::download::Error::Stopped) => None,
                        Err(e) => Some(e.to_string()),
                    };
                });
                cx.refresh_windows();
            });
        })
        .detach();
    }

    pub fn stop_download(&mut self) {
        if let Some(d) = &self.download {
            d.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// The model the settings name, if it is downloaded.
pub fn ready_model(settings: &Settings) -> Option<PathBuf> {
    let model = authwing_voice::model::by_name(&settings.voice.model)?;
    let dir = Store::models_dir()?;
    model.is_in(&dir).then(|| model.path_in(&dir))
}

/// The language names are said in: the one set for voice, else the app's,
/// else the system's; one Whisper knows.
pub fn language(settings: &Settings) -> String {
    let set = settings.voice.language.trim();
    let known = |code: &str| LANGUAGES.iter().any(|(c, _)| *c == code);
    if set != "auto" && known(set) {
        return set.to_string();
    }
    settings
        .general
        .language
        .as_deref()
        .and_then(language_for_locale)
        .unwrap_or_else(system_language)
        .to_string()
}

/// What the user may call a code: its service, the service with the
/// account, the account alone when there is no service, and its sites'
/// names (`github` for `gist.github.com`). An address is called by its
/// name before the `@`: nobody says the rest, and in Whisper's prompt it
/// is what Whisper writes when it hears nothing clearly.
pub fn names(issuer: &str, account: &str, sites: &[authwing_context::Site]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |name: String| {
        let name = name.trim().to_string();
        if !name.is_empty() && !names.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            names.push(name);
        }
    };
    let (user, domain) = match account.split_once('@') {
        Some((user, domain)) => (user, Some(domain)),
        None => (account, None),
    };
    if issuer.trim().is_empty() {
        add(user.to_string());
        // tien@acme.com: Acme.
        if let Some(label) = domain.and_then(|d| d.split('.').next()) {
            add(label.to_string());
        }
    } else {
        add(issuer.to_string());
        if !user.trim().is_empty() {
            add(format!("{issuer} {user}"));
        }
    }
    for site in sites {
        let owner = site.registrable();
        if let Some(label) = owner.split('.').next().filter(|_| owner.contains('.')) {
            add(label.to_string());
        }
    }
    names
}

/// Picked, offered, or nothing, as voice decides on what was heard.
pub fn decide(heard: &Heard<String>) -> authwing_voice::Decision<String> {
    Thresholds::default().decide(&heard.ranked)
}

fn mic_thread(jobs: mpsc::Receiver<MicJob>) {
    let mut recording: Option<authwing_voice::mic::Recording> = None;
    for job in jobs {
        match job {
            MicJob::Start(reply) => {
                drop(recording.take());
                let started = authwing_voice::mic::Recording::start();
                let _ =
                    reply.send_blocking(started.as_ref().map(|_| ()).map_err(|e| e.to_string()));
                recording = started.ok();
            }
            MicJob::Stop(reply) => {
                let heard = recording.take().map(|r| r.stop()).unwrap_or_default();
                let _ = reply.send_blocking(heard);
            }
            MicJob::Drop => drop(recording.take()),
        }
    }
}

fn ear_thread(jobs: mpsc::Receiver<EarJob>) {
    let mut loaded: Option<(PathBuf, Transcriber)> = None;
    let load = |loaded: &mut Option<(PathBuf, Transcriber)>, model: &Path| {
        if loaded.as_ref().is_some_and(|(p, _)| p == model) {
            return Ok(());
        }
        *loaded = None;
        let transcriber = Transcriber::new(model).map_err(|e| e.to_string())?;
        *loaded = Some((model.to_owned(), transcriber));
        Ok::<(), String>(())
    };
    loop {
        let job = match jobs.recv_timeout(IDLE) {
            Ok(job) => job,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                loaded = None;
                continue;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        };
        match job {
            EarJob::Warm(model) => {
                let _ = load(&mut loaded, &model);
            }
            EarJob::Hear {
                model,
                pcm,
                language,
                candidates,
                reply,
            } => {
                let heard = load(&mut loaded, &model).and_then(|()| {
                    let (_, transcriber) = loaded.as_mut().ok_or("no model")?;
                    transcriber
                        .recognize(&pcm, &language, &candidates, &Thresholds::default())
                        .map_err(|e| e.to_string())
                });
                let _ = reply.send_blocking(heard);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sites(hosts: &[&str]) -> Vec<authwing_context::Site> {
        hosts
            .iter()
            .map(|h| authwing_context::Site::parse(h).expect("site"))
            .collect()
    }

    #[test]
    fn a_code_is_called_by_its_service_account_and_sites() {
        assert_eq!(
            names(
                "GitHub",
                "tien@example.com",
                &sites(&["gist.github.com", "github.com"])
            ),
            ["GitHub", "GitHub tien"]
        );
        assert_eq!(
            names("", "tien@acme.com", &sites(&["signin.aws.amazon.com"])),
            ["tien", "acme", "amazon"]
        );
        assert_eq!(
            names("Vietcombank", "0123456", &[]),
            ["Vietcombank", "Vietcombank 0123456"]
        );
        assert_eq!(names("Zalo", "", &sites(&["localhost"])), ["Zalo"]);
    }

    #[test]
    fn the_language_is_one_whisper_knows() {
        let mut settings = Settings::default();
        settings.voice.language = "vi".into();
        assert_eq!(language(&settings), "vi");
        settings.voice.language = "auto".into();
        settings.general.language = Some("de".into());
        assert_eq!(language(&settings), "de");
        settings.voice.language = "klingon".into();
        assert_eq!(language(&settings), "de");
        settings.general.language = None;
        assert!(LANGUAGES.iter().any(|(c, _)| *c == language(&settings)));
    }
}
