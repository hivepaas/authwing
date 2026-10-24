//! Records from the default microphone as the app does, keeps it as a WAV
//! file, and says what Whisper heard: for trying voice out by hand.
//!
//! cargo run -p authwing-voice --features whisper,mic --example listen -- \
//!     <model.bin> <seconds> <out.wav> <language>... [-- <name>...]
//!
//! The names after `--` are the accounts to expect, as the app gives them.

use std::time::Duration;

use authwing_voice::mic::Recording;
use authwing_voice::whisper::Transcriber;
use authwing_voice::{Candidate, SAMPLE_RATE, Thresholds, is_silent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (head, names) = match args.iter().position(|a| a == "--") {
        Some(i) => (&args[..i], &args[i + 1..]),
        None => (&args[..], &[][..]),
    };
    let [model, seconds, out, languages @ ..] = head else {
        return Err("model.bin seconds out.wav language... [-- name...]".into());
    };
    let recording = Recording::start()?;
    eprintln!("recording {seconds} s…");
    std::thread::sleep(Duration::from_secs_f64(seconds.parse()?));
    let pcm = recording.stop();
    let peak = pcm.iter().fold(0f32, |m, s| m.max(s.abs()));
    let rms = (pcm.iter().map(|s| s * s).sum::<f32>() / pcm.len().max(1) as f32).sqrt();
    eprintln!(
        "{} samples, peak {peak:.4}, rms {rms:.5}, silent: {}",
        pcm.len(),
        is_silent(&pcm)
    );
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut wav = hound::WavWriter::create(out, spec)?;
    for s in &pcm {
        wav.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16)?;
    }
    wav.finalize()?;
    let candidates: Vec<Candidate<usize>> = names
        .iter()
        .enumerate()
        .map(|(i, n)| Candidate::new(i, [n.as_str()]))
        .collect();
    let mut whisper = Transcriber::new(model)?;
    for language in languages {
        let heard = whisper.recognize(&pcm, language, &candidates, &Thresholds::default())?;
        let best = heard
            .ranked
            .first()
            .map(|s| format!("{} {:.2}", names[s.id], s.score))
            .unwrap_or_default();
        println!(
            "{language}: “{}” (as {}) → {best}",
            heard.transcript, heard.language
        );
    }
    Ok(())
}
