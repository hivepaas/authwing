//! Speech to text with whisper.cpp.
//!
//! The settings are those that did best on short "open <service>" phrases
//! in 15 languages, on an M1 Pro with the base model. The language is set,
//! not detected. The account names are the initial prompt: 14 of 15 right
//! with them, 10 without. Decoding is one greedy pass over one segment,
//! without timestamps, earlier context or retries at higher temperatures,
//! and half a second of silence goes on each side, as when the user presses
//! the key a moment before speaking. The audio context stays at Whisper's
//! full 30 seconds: cut to the length of the clip, Whisper looped and made
//! things up.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
    WhisperState,
};

use crate::{Candidate, LANGUAGES, SAMPLE_RATE, Scored, Thresholds, rank, vocabulary};

/// The silence put before and after a recording: half a second.
const PADDING: usize = SAMPLE_RATE as usize / 2;

/// Why speech could not be written down.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot load the Whisper model {}: {source}", path.display())]
    Model { path: PathBuf, source: WhisperError },
    #[error("Whisper does not hear the language {0:?}")]
    Language(String),
    #[error("Whisper: {0}")]
    Whisper(#[from] WhisperError),
}

/// What was heard, and the candidates ranked by it.
#[derive(Clone, Debug, PartialEq)]
pub struct Heard<Id> {
    /// What Whisper wrote: what to teach the account the user then chooses,
    /// if they had to (see [`Candidate::taught`]).
    pub transcript: String,
    /// The language it was heard as.
    pub language: &'static str,
    /// The candidates, best first, as [`rank`] orders them.
    pub ranked: Vec<Scored<Id>>,
}

impl<Id> Heard<Id> {
    /// Whether the best candidate is named well enough to be picked, by
    /// its score alone.
    fn is_clear(&self, thresholds: &Thresholds) -> bool {
        self.ranked
            .first()
            .is_some_and(|best| best.taught || best.score > thresholds.pick)
    }

    /// Whether this names its best candidate better than `other` does.
    fn beats(&self, other: &Self) -> bool {
        let best = |heard: &Self| heard.ranked.first().map(|s| (s.taught, s.score));
        best(self) > best(other)
    }
}

/// A Whisper model, loaded once, that writes down one recording at a time.
/// It can live on a thread of its own: a phrase takes about 100 ms with
/// the base model on an M1 Pro, too long for the UI thread.
pub struct Transcriber {
    state: WhisperState,
}

impl Transcriber {
    /// Loads the ggml model at `model` (see [`crate::model`]), on the GPU
    /// where there is one. whisper.cpp's logging is silenced first: it
    /// writes to stderr otherwise. Loading takes a fraction of a second,
    /// but about 20 seconds the first time a new build of the app runs,
    /// while Metal compiles its shaders (macOS keeps them after), so load
    /// it ahead of the first key press.
    pub fn new(model: impl AsRef<Path>) -> Result<Self, Error> {
        whisper_rs::install_logging_hooks();
        let path = model.as_ref();
        let context = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|source| Error::Model {
                path: path.to_owned(),
                source,
            })?;
        Ok(Self {
            state: context.create_state()?,
        })
    }

    /// What was said in `pcm`, mono at [`SAMPLE_RATE`], heard as
    /// `language` (a code from [`LANGUAGES`]), expecting the names in
    /// `vocabulary` (see [`vocabulary`]).
    pub fn transcribe(
        &mut self,
        pcm: &[f32],
        language: &str,
        vocabulary: &str,
    ) -> Result<String, Error> {
        let language = known(language)?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(language));
        params.set_n_threads(threads());
        params.set_no_context(true);
        params.set_single_segment(true);
        params.set_no_timestamps(true);
        params.set_temperature_inc(0.0);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        let vocabulary = if vocabulary.contains('\0') {
            Cow::Owned(vocabulary.replace('\0', ""))
        } else {
            Cow::Borrowed(vocabulary)
        };
        if !vocabulary.is_empty() {
            params.set_initial_prompt(&vocabulary);
        }

        let mut padded = Vec::with_capacity(pcm.len() + 2 * PADDING);
        padded.resize(PADDING, 0.0);
        padded.extend_from_slice(pcm);
        padded.resize(pcm.len() + 2 * PADDING, 0.0);
        self.state.full(params, &padded)?;

        let mut text = String::new();
        for segment in self.state.as_iter() {
            text.push_str(&segment.to_str_lossy()?);
        }
        Ok(text.trim().to_owned())
    }

    /// Writes down `pcm` in `language` and ranks `candidates` by it, with
    /// their names as the vocabulary. When that names no candidate clearly
    /// and the language is not English, it listens again in English and
    /// keeps what named one better: brand names are often said the English
    /// way, whatever the language around them.
    pub fn recognize<Id: Clone>(
        &mut self,
        pcm: &[f32],
        language: &str,
        candidates: &[Candidate<Id>],
        thresholds: &Thresholds,
    ) -> Result<Heard<Id>, Error> {
        let language = known(language)?;
        let vocabulary = vocabulary(candidates);
        let mut hear = |language: &'static str| -> Result<Heard<Id>, Error> {
            let transcript = self.transcribe(pcm, language, &vocabulary)?;
            let ranked = rank(&transcript, candidates);
            Ok(Heard {
                transcript,
                language,
                ranked,
            })
        };
        let heard = hear(language)?;
        if language == "en" || heard.is_clear(thresholds) {
            return Ok(heard);
        }
        let english = hear("en")?;
        Ok(if english.beats(&heard) {
            english
        } else {
            heard
        })
    }
}

/// `language` as one of Whisper's, which it must be.
fn known(language: &str) -> Result<&'static str, Error> {
    LANGUAGES
        .iter()
        .map(|(code, _)| *code)
        .find(|code| *code == language)
        .ok_or_else(|| Error::Language(language.to_owned()))
}

/// The CPU threads to decode on: up to eight, as measured.
fn threads() -> i32 {
    std::thread::available_parallelism().map_or(4, |n| n.get().min(8) as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heard(best: f64, taught: bool) -> Heard<u8> {
        Heard {
            transcript: String::new(),
            language: "vi",
            ranked: vec![Scored {
                id: 0,
                score: best,
                taught,
            }],
        }
    }

    #[test]
    fn transcribers_can_move_to_a_thread() {
        fn sendable<T: Send>() {}
        sendable::<Transcriber>();
    }

    #[test]
    fn only_whispers_languages_are_heard() {
        assert_eq!(known("vi").ok(), Some("vi"));
        for language in ["auto", "yue", "VI", "", "v\0"] {
            assert!(matches!(known(language), Err(Error::Language(_))));
        }
    }

    #[test]
    fn the_better_hearing_is_kept() {
        let thresholds = Thresholds::default();
        assert!(heard(0.8, false).is_clear(&thresholds));
        assert!(!heard(0.75, false).is_clear(&thresholds));
        assert!(heard(0.6, false).beats(&heard(0.5, false)));
        assert!(!heard(0.5, false).beats(&heard(0.5, false)));
        assert!(heard(0.9, true).beats(&heard(1.0, false)));
    }
}
