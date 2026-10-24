//! Finding the account a transcript names.
//!
//! Whisper writes brand names through the language it expects: "open
//! Vietcombank" said in Vietnamese comes back as "Mở viết cơm an", Microsoft
//! in Russian as "майкрасовд". So both sides are transliterated to ASCII and
//! cut to lowercase letters and digits, and every run of consecutive words
//! in the transcript, joined without spaces, is compared with each name,
//! spaces removed, by normalized Damerau-Levenshtein similarity. Joining
//! the words undoes Whisper's splits ("Adrop Box"); the command around the
//! name ("mở", "открой", "打開") is never part of the best run, so it costs
//! nothing. A candidate's score is its best run against its best name.
//!
//! Wrong names can score as well as right ones do on a bad transcript, so
//! a score alone does not pick: see [`Thresholds`]. What the user once had
//! to choose by hand is kept as a taught name, and hearing the same thing
//! again picks that account outright.

use std::cmp::Reverse;
use std::collections::HashSet;

/// How long [`vocabulary`] may be, in bytes. whisper.cpp keeps only the
/// last 223 tokens of an initial prompt, dropping the start, which holds
/// the most used names. A token is at least one byte, so this many bytes
/// always fit, whatever the script.
pub const PROMPT_BYTES: usize = 220;

/// How similar the whole transcript must be to a taught name for it to
/// count as heard again: one edit in seven letters, nothing in fewer.
const TAUGHT: f64 = 0.85;

/// An account the user may be naming.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate<Id> {
    /// What the caller knows the account by; handed back in the results.
    pub id: Id,
    /// What the account is called: its issuer first, then "Issuer account"
    /// and any other label it is shown with.
    pub names: Vec<String>,
    /// What Whisper wrote when the user said this account's name and then
    /// chose it by hand, kept whole, or a name the user typed for it ("ví
    /// mơ mơ"). Taught names count as names; a transcript that is (nearly)
    /// all a taught name picks the account over any other.
    pub taught: Vec<String>,
    /// How often the account has been opened, so that the most used names
    /// are the ones in Whisper's prompt when not all fit.
    pub uses: u32,
}

impl<Id> Candidate<Id> {
    /// An account called `names`, with nothing taught and no uses yet.
    pub fn new<S: Into<String>>(id: Id, names: impl IntoIterator<Item = S>) -> Self {
        Self {
            id,
            names: names.into_iter().map(Into::into).collect(),
            taught: Vec::new(),
            uses: 0,
        }
    }
}

/// How well a transcript names one candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct Scored<Id> {
    /// The candidate's id.
    pub id: Id,
    /// The similarity of the best run of words to the best name, from 0
    /// (nothing alike) to 1 (the same letters).
    pub score: f64,
    /// Whether the transcript was one of the candidate's taught names.
    pub taught: bool,
}

/// What to do about a ranking.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision<Id> {
    /// Show this account: it was named clearly.
    Pick(Id),
    /// Ask which of these was meant, best first; at most three.
    Choose(Vec<Id>),
    /// Nothing heard is like any account.
    Nothing,
}

/// When a ranking is clear enough to act on. The defaults come from what
/// Whisper wrote for the measured phrases (the tests hold them):
///
/// - `pick` 0.75, to be exceeded: one letter wrong in four is not enough
///   ("dalo" was Zalo, but "eine" was not LINE), one in five or two in
///   nine are.
/// - `margin` 0.2: the clear right answers led the next account by 0.32
///   or more (GitHub over GitLab by 0.33), while a letter in a name of six
///   or more is worth at most 0.17, so an account only a letter closer than
///   another is asked about, and two accounts of one issuer always are.
/// - `floor` 0.3: the lowest right answer scored 0.30 ("Ösnepeypey" for
///   PayPal); unrelated words score 0.2 to 0.4, so below that there is
///   nothing worth offering.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// The score the best candidate must exceed to be picked outright.
    pub pick: f64,
    /// How far ahead of the second the best must be to be picked outright.
    pub margin: f64,
    /// The score a candidate needs to be offered at all.
    pub floor: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            pick: 0.75,
            margin: 0.2,
            floor: 0.3,
        }
    }
}

impl Thresholds {
    /// What to do about `ranked`, as [`rank`] orders it. A taught name
    /// heard again is picked unless another account was taught the same.
    pub fn decide<Id: Clone>(&self, ranked: &[Scored<Id>]) -> Decision<Id> {
        let Some(best) = ranked.first() else {
            return Decision::Nothing;
        };
        let runner_up = ranked.get(1);
        let ahead = runner_up.is_none_or(|second| {
            best.taught != second.taught || best.score - second.score >= self.margin
        });
        if ahead && (best.taught || best.score > self.pick) {
            return Decision::Pick(best.id.clone());
        }
        let offered: Vec<Id> = ranked
            .iter()
            .take(3)
            .filter(|s| s.taught || s.score >= self.floor)
            .map(|s| s.id.clone())
            .collect();
        if offered.is_empty() {
            Decision::Nothing
        } else {
            Decision::Choose(offered)
        }
    }
}

/// Scores every candidate against `transcript`, best first: those whose
/// taught name it was, then by score; equals keep the candidates' order.
pub fn rank<Id: Clone>(transcript: &str, candidates: &[Candidate<Id>]) -> Vec<Scored<Id>> {
    let heard = words(transcript);
    let whole = heard.concat();
    let mut ranked: Vec<Scored<Id>> = candidates
        .iter()
        .map(|candidate| {
            let best = |names: &[String]| {
                names
                    .iter()
                    .map(|name| similarity(&heard, name))
                    .fold(0.0, f64::max)
            };
            let taught = !whole.is_empty()
                && candidate.taught.iter().any(|name| {
                    strsim::normalized_damerau_levenshtein(&whole, &key(name)) >= TAUGHT
                });
            Scored {
                id: candidate.id.clone(),
                score: best(&candidate.names).max(best(&candidate.taught)),
                taught,
            }
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.taught
            .cmp(&a.taught)
            .then_with(|| b.score.total_cmp(&a.score))
    });
    ranked
}

/// The candidates' names for Whisper's initial prompt, which makes it
/// spell them as the user's accounts do: comma separated, each once, at
/// most [`PROMPT_BYTES`] long. Every account's first name comes before any
/// account's second, and the most used accounts' before the others'.
/// Taught names are left out: they are Whisper's mishearings.
pub fn vocabulary<Id>(candidates: &[Candidate<Id>]) -> String {
    let mut by_use: Vec<&Candidate<Id>> = candidates.iter().collect();
    by_use.sort_by_key(|c| Reverse(c.uses));
    let rounds = candidates.iter().map(|c| c.names.len()).max().unwrap_or(0);
    let mut seen = HashSet::new();
    let mut prompt = String::new();
    for round in 0..rounds {
        for name in by_use.iter().filter_map(|c| c.names.get(round)) {
            let name = name
                .split(|c: char| c.is_whitespace() || c.is_control())
                .filter(|part| !part.is_empty())
                .collect::<Vec<_>>()
                .join(" ");
            let separator = if prompt.is_empty() { "" } else { ", " };
            if prompt.len() + separator.len() + name.len() > PROMPT_BYTES {
                continue;
            }
            let key = key(&name);
            if !key.is_empty() && seen.insert(key) {
                prompt.push_str(separator);
                prompt.push_str(&name);
            }
        }
    }
    prompt
}

/// The best similarity between `name` and a run of consecutive `heard`
/// words. Runs are up to two words longer than the name, for the words
/// Whisper splits it into.
fn similarity(heard: &[String], name: &str) -> f64 {
    let name = words(name);
    let target = name.concat();
    if target.is_empty() {
        return 0.0;
    }
    (1..=(name.len() + 2).min(heard.len()))
        .flat_map(|n| heard.windows(n))
        .map(|run| strsim::normalized_damerau_levenshtein(&run.concat(), &target))
        .fold(0.0, f64::max)
}

/// `name` as compared: its words joined.
fn key(name: &str) -> String {
    words(name).concat()
}

/// The words of `text`, transliterated to lowercase ASCII letters and
/// digits. Scripts written without spaces are set apart from the words
/// around them first, so that "打開PayPal" has a word "paypal".
fn words(text: &str) -> Vec<String> {
    let mut spaced = String::with_capacity(text.len() + 8);
    let mut was_unspaced = false;
    for c in text.chars() {
        let unspaced = is_unspaced(c);
        if unspaced != was_unspaced {
            spaced.push(' ');
        }
        was_unspaced = unspaced;
        spaced.push(c);
    }
    deunicode::deunicode(&spaced)
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Whether `c` belongs to a script written without spaces between words:
/// Chinese, Japanese, Korean, Thai, Lao, Khmer and Burmese.
fn is_unspaced(c: char) -> bool {
    matches!(
        c,
        '\u{1000}'..='\u{109F}'
            | '\u{0E00}'..='\u{0EFF}'
            | '\u{1100}'..='\u{11FF}'
            | '\u{1780}'..='\u{17FF}'
            | '\u{3000}'..='\u{30FF}'
            | '\u{3130}'..='\u{318F}'
            | '\u{31F0}'..='\u{31FF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{AC00}'..='\u{D7AF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{FF00}'..='\u{FFEF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    fn accounts() -> Vec<Candidate<&'static str>> {
        ACCOUNTS
            .iter()
            .map(|name| Candidate::new(*name, [*name]))
            .collect()
    }

    fn decide(transcript: &str, candidates: &[Candidate<&'static str>]) -> Decision<&'static str> {
        Thresholds::default().decide(&rank(transcript, candidates))
    }

    /// What Whisper wrote for "open <account>" in the measured run, the
    /// account meant, and whether it is clear enough to pick.
    const HEARD: &[(&str, &str, bool)] = &[
        ("Mở viết cơm an.", "Vietcombank", true),
        ("Mở Vietcoman", "Vietcombank", true),
        ("Movie Headcombran", "Vietcombank", false),
        ("Открой майкрасовд.", "Microsoft", false),
        ("Открой Microsoft", "Microsoft", true),
        ("打開PayPal", "PayPal", true),
        ("Book Adrop Box", "Dropbox", true),
        ("Asna PayPal", "PayPal", true),
        ("Esnepel-Paypal", "PayPal", true),
        ("abrir com em base", "Coinbase", false),
        ("GitHub, Yara Joe", "GitHub", true),
        ("Microsoft, Kholo", "Microsoft", true),
        ("Finance", "Binance", true),
        ("Dạ lo của công ty", "Zalo", false),
        ("Vì mơ mơ", "MoMo", true),
        ("Googleを開いて", "Google", true),
    ];

    #[test]
    fn heard_accounts_rank_first() {
        let accounts = accounts();
        for &(transcript, meant, clear) in HEARD {
            let ranked = rank(transcript, &accounts);
            assert_eq!(ranked[0].id, meant, "{transcript}: {:?}", &ranked[..3]);
            match decide(transcript, &accounts) {
                Decision::Pick(id) => assert!(clear && id == meant, "{transcript} picked {id}"),
                Decision::Choose(ids) => {
                    assert!(!clear, "{transcript} asked among {ids:?}");
                    assert_eq!(ids.first(), Some(&meant), "{transcript}");
                    assert!(ids.len() <= 3);
                }
                Decision::Nothing => panic!("{transcript} heard nothing"),
            }
        }
    }

    #[test]
    fn near_misses_are_not_picked() {
        let accounts = accounts();
        // Whisper without the names in its prompt, for PayPal both times.
        for transcript in ["Es ist eine Pepe.", "打開啞鈴鐺"] {
            assert!(
                matches!(decide(transcript, &accounts), Decision::Choose(_)),
                "{transcript}"
            );
        }
    }

    #[test]
    fn nothing_like_an_account_is_nothing() {
        let accounts = accounts();
        for transcript in ["", " ...", "open AWS", "افحبه دل"] {
            assert_eq!(
                decide(transcript, &accounts),
                Decision::Nothing,
                "{transcript}"
            );
        }
        assert_eq!(Thresholds::default().decide::<u32>(&[]), Decision::Nothing);
    }

    #[test]
    fn two_accounts_of_one_issuer_are_asked_about() {
        let accounts = [
            Candidate::new("personal", ["Google", "Google tien@gmail.com"]),
            Candidate::new("work", ["Google", "Google tien@hivepaas.com"]),
        ];
        assert_eq!(
            decide("Open Google", &accounts),
            Decision::Choose(vec!["personal", "work"])
        );
    }

    #[test]
    fn a_taught_name_heard_again_picks() {
        let mut accounts = accounts();
        accounts[8].taught.push("Movie Headcombran".into());
        assert_eq!(
            decide("movie headcombran!", &accounts),
            Decision::Pick("Vietcombank")
        );
        // One letter off in sixteen is still what was taught.
        assert_eq!(
            decide("Movie Headcombram", &accounts),
            Decision::Pick("Vietcombank")
        );

        let mut work = [
            Candidate::new("personal", ["Google"]),
            Candidate::new("work", ["Google"]),
        ];
        work[1].taught.push("Mở Google công ty".into());
        assert_eq!(decide("Mở Google công ty.", &work), Decision::Pick("work"));
        assert!(matches!(decide("Mở Google", &work), Decision::Choose(_)));
    }

    #[test]
    fn a_typed_name_is_a_name() {
        let mut accounts = accounts();
        accounts[14].taught.push("AWS".into());
        assert_eq!(
            decide("open AWS", &accounts),
            Decision::Pick("Amazon Web Services")
        );
    }

    #[test]
    fn a_taught_name_inside_a_transcript_does_not_pick() {
        let mut accounts = accounts();
        accounts[13].taught.push("Mở".into());
        assert_ne!(decide("Mở Zalo", &accounts), Decision::Pick("MoMo"));
    }

    #[test]
    fn one_name_taught_to_two_accounts_is_asked_about() {
        let mut accounts = accounts();
        accounts[0].taught.push("Git hap".into());
        accounts[1].taught.push("Git hap".into());
        let Decision::Choose(ids) = decide("Git hap", &accounts) else {
            panic!("picked one of two");
        };
        assert_eq!(ids[..2], ["GitHub", "GitLab"]);
    }

    #[test]
    fn vocabulary_names_each_account_once() {
        assert_eq!(
            vocabulary(&accounts()),
            "GitHub, GitLab, PayPal, Binance, Google, Dropbox, Coinbase, Microsoft, \
             Vietcombank, Facebook, LINE, Naver, Zalo, MoMo, Amazon Web Services"
        );
        let accounts = [
            Candidate::new(1, ["GitHub", "GitHub tien"]),
            Candidate::new(2, ["Github", "GitHub work"]),
            Candidate::new(3, ["git hub"]),
        ];
        assert_eq!(vocabulary(&accounts), "GitHub, GitHub tien, GitHub work");
    }

    #[test]
    fn vocabulary_puts_the_most_used_first() {
        let mut accounts = vec![
            Candidate::new(1, ["Zalo", "Zalo tien"]),
            Candidate::new(2, ["MoMo", "MoMo 0901"]),
            Candidate::new(3, ["LINE"]),
        ];
        accounts[2].uses = 7;
        accounts[1].uses = 3;
        assert_eq!(
            vocabulary(&accounts),
            "LINE, MoMo, Zalo, MoMo 0901, Zalo tien"
        );
    }

    #[test]
    fn vocabulary_fits_whispers_prompt() {
        let mut accounts: Vec<Candidate<usize>> = (0..100)
            .map(|i| Candidate::new(i, [format!("Service number {i}")]))
            .collect();
        accounts[99].uses = 1;
        accounts[42].taught.push("never in the prompt".into());
        let prompt = vocabulary(&accounts);
        assert!(prompt.len() <= PROMPT_BYTES, "{}", prompt.len());
        assert!(prompt.starts_with("Service number 99, Service number 0, "));
        assert!(!prompt.contains("never"));

        let odd = [Candidate::new(0, ["  Tab\tand\0nul  ", "", "!!!"])];
        assert_eq!(vocabulary(&odd), "Tab and nul");
        let wide = [
            Candidate::new(0, ["楽天".repeat(40)]),
            Candidate::new(1, ["LINE"]),
        ];
        assert_eq!(vocabulary(&wide), "LINE");
    }
}
