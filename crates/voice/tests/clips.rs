//! Whisper on recorded phrases. `AUTHWING_WHISPER_MODEL` names a ggml model
//! and `AUTHWING_VOICE_CLIPS` a folder of 16 kHz mono WAV files named
//! `<language>_<account>.wav`, each saying "open <account>" in that
//! language; without both there is nothing to test, and the test passes.
#![cfg(feature = "whisper")]

use std::path::Path;
use std::time::{Duration, Instant};

use authwing_voice::whisper::Transcriber;
use authwing_voice::{Candidate, Decision, SAMPLE_RATE, Thresholds, rank, vocabulary};

/// The accounts of the measured run, names that look alike included.
const ACCOUNTS: &[&str] = &[
    "GitHub",
    "GitLab",
    "PayPal",
    "Binance",
    "Google",
    "Dropbox",
    "Coinbase",
    "Microsoft",
    "Vietcombank",
    "Facebook",
    "LINE",
    "Naver",
    "Zalo",
    "MoMo",
    "Amazon Web Services",
];

fn read(path: &Path) -> Vec<f32> {
    let mut wav = hound::WavReader::open(path).expect("a WAV file");
    let spec = wav.spec();
    assert_eq!(
        (spec.sample_rate, spec.channels),
        (SAMPLE_RATE, 1),
        "{path:?}"
    );
    wav.samples::<i16>()
        .map(|s| f32::from(s.expect("a sample")) / 32768.0)
        .collect()
}

fn ms(time: Duration) -> f64 {
    time.as_secs_f64() * 1000.0
}

#[test]
fn clips_name_their_accounts() {
    let (Some(model), Some(clips)) = (
        std::env::var_os("AUTHWING_WHISPER_MODEL"),
        std::env::var_os("AUTHWING_VOICE_CLIPS"),
    ) else {
        eprintln!("AUTHWING_WHISPER_MODEL and AUTHWING_VOICE_CLIPS are not set: nothing to test");
        return;
    };
    let accounts: Vec<Candidate<&str>> = ACCOUNTS
        .iter()
        .map(|name| Candidate::new(*name, [*name]))
        .collect();
    let thresholds = Thresholds::default();
    let names = vocabulary(&accounts);

    let started = Instant::now();
    let mut transcriber = Transcriber::new(&model).expect("the model loads");
    println!("loaded in {:.0} ms", ms(started.elapsed()));

    let mut clips: Vec<_> = std::fs::read_dir(&clips)
        .expect("the clips folder")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "wav"))
        .collect();
    clips.sort();
    assert!(!clips.is_empty(), "no clips");

    let first = read(&clips[0]);
    let started = Instant::now();
    transcriber
        .transcribe(&first, "en", &names)
        .expect("a transcript");
    println!("first transcript in {:.0} ms", ms(started.elapsed()));

    let (mut right, mut right_alone, mut picked, mut wrong_picks) = (0, 0, 0, 0);
    let (mut alone_time, mut time) = (Duration::ZERO, Duration::ZERO);
    for clip in &clips {
        let stem = clip.file_stem().and_then(|s| s.to_str()).expect("a name");
        let (language, meant) = stem.split_once('_').expect("<language>_<account>");
        let pcm = read(clip);

        let started = Instant::now();
        let alone = transcriber
            .transcribe(&pcm, language, &names)
            .expect("a transcript");
        alone_time += started.elapsed();
        let alone_best = rank(&alone, &accounts)[0].id;
        right_alone += usize::from(alone_best.eq_ignore_ascii_case(meant));

        let started = Instant::now();
        let heard = transcriber
            .recognize(&pcm, language, &accounts, &thresholds)
            .expect("a transcript");
        let took = started.elapsed();
        time += took;
        let best = &heard.ranked[0];
        let is_right = best.id.eq_ignore_ascii_case(meant);
        right += usize::from(is_right);
        let decision = thresholds.decide(&heard.ranked);
        if let Decision::Pick(id) = decision {
            picked += 1;
            wrong_picks += usize::from(!id.eq_ignore_ascii_case(meant));
        }
        println!(
            "{language:<3} {:>5.0} ms  {:<2} {:<24} {:<12} {:.2} {} {decision:?}",
            ms(took),
            heard.language,
            heard.transcript,
            best.id,
            best.score,
            if is_right { "right" } else { "WRONG" },
        );
    }
    let n = clips.len();
    println!(
        "in the language alone: {right_alone}/{n} right, {:.0} ms each",
        ms(alone_time) / n as f64
    );
    println!(
        "with English to fall back on: {right}/{n} right, {:.0} ms each; {picked} picked, {wrong_picks} wrongly",
        ms(time) / n as f64
    );
    // The measured run had 14 of 15 right.
    assert!(right * 10 >= n * 8, "{right}/{n} right");
    assert_eq!(wrong_picks, 0);
}
