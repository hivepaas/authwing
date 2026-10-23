//! Finding the account the user names aloud.
//!
//! The user holds a key and says "open GitHub" in their own language: the
//! microphone records it (`mic` feature), Whisper writes it down
//! (`whisper` feature), and [`rank`] finds the account the words name.
//! Matching is fuzzy, because Whisper hears brand names through the
//! language it expects, and it is pure and always built, so that it can be
//! tested on transcripts alone; whisper.cpp needs CMake to build and the
//! microphone needs audio libraries, so the app opts into those.

mod audio;
mod language;
mod matching;
pub mod model;

#[cfg(feature = "download")]
pub mod download;
#[cfg(feature = "mic")]
pub mod mic;
#[cfg(feature = "whisper")]
pub mod whisper;

pub use audio::{SAMPLE_RATE, is_silent, resample};
pub use language::{LANGUAGES, language_for_locale, system_language};
pub use matching::{Candidate, Decision, PROMPT_BYTES, Scored, Thresholds, rank, vocabulary};
