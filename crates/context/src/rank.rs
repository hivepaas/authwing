//! Which codes the user most likely wants where they are now.
//!
//! The context is the active window and, when the browser extension
//! reports one, the page in its focused tab. A code is put forward for the
//! strongest of these reasons, in this order:
//!
//! 1. [`Reason::Site`]: the page is one of its sites. Only this says the
//!    code may be typed: the address comes from the browser, which the page
//!    cannot make say another.
//! 2. [`Reason::Rule`]: one of its rules matches the window.
//! 3. [`Reason::Learned`]: the user chose it here before.
//! 4. [`Reason::IssuerInTitle`]: the window's title or app names its issuer.
//!    A page writes its own title, so this only orders the list.
//!
//! On a web page, a code with sites is put forward only where one of them
//! matches: its sites say where it belongs, whatever a page calls itself.
//! A code saved without sites is held to those its issuer suggests
//! ([`suggest`]), so it is kept off look-alikes too. A page that looks like
//! a code's site without being it puts forward no code at all, and gives a
//! warning instead.
//!
//! The user's choices are learned under a [`Key`], made the same way when
//! recorded and when looked up: the page's registrable domain, or else the
//! app and its window's title without what changes while it is the same
//! place (counts of unread items, the browser's name, a terminal's size).

use std::cmp::Reverse;
use std::fmt;

use crate::site::{Page, Resemblance, Site, Verdict, suggest};
use crate::window::Window;

/// A rule the user set on a code: put it forward in an app, or in windows
/// whose title has some words, or both.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rule {
    /// The app's id or name, in any case.
    pub app: Option<String>,
    /// Words the window's title has, in any case.
    pub title_contains: Option<String>,
}

impl Rule {
    /// Whether `window` is one this rule is for. A rule with neither part
    /// set (or set to blanks) is for none.
    pub fn matches(&self, window: &Window) -> bool {
        let app = given(&self.app);
        let words = given(&self.title_contains);
        if app.is_none() && words.is_none() {
            return false;
        }
        let app_matches = app.is_none_or(|app| {
            window.app_id.trim().eq_ignore_ascii_case(app)
                || window.app_name.trim().to_lowercase() == app.to_lowercase()
        });
        let title_matches = words.is_none_or(|words| {
            window
                .title
                .as_deref()
                .is_some_and(|title| title.to_lowercase().contains(&words.to_lowercase()))
        });
        app_matches && title_matches
    }
}

fn given(part: &Option<String>) -> Option<&str> {
    part.as_deref()
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

/// Where the user is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    /// The window they are in, from [`crate::window::active`].
    pub window: Option<Window>,
    /// The page the browser extension reports for its focused tab. Only
    /// while the window is that browser's: a tab in a window behind says
    /// nothing of where the user types.
    pub page: Option<Page>,
}

impl Context {
    /// The key to record the user's choice under here, and to look up how
    /// many times they made it. None when nothing is known of where they are.
    pub fn key(&self) -> Option<Key> {
        if let Some(domain) = self.page.as_ref().and_then(Page::registrable) {
            return Some(Key(format!("site:{domain}")));
        }
        let window = self.window.as_ref()?;
        let app = if window.app_id.is_empty() {
            &window.app_name
        } else {
            &window.app_id
        };
        let title = window
            .title
            .as_deref()
            .map(|title| title_key(title, &window.app_name))
            .unwrap_or_default();
        Some(Key(format!("app:{}\n{title}", app.to_lowercase())))
    }
}

/// Where a choice was made, for learning it: see [`Context::key`].
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Key(String);

impl Key {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<Key> for String {
    fn from(key: Key) -> String {
        key.0
    }
}

/// What ranking knows of a code.
#[derive(Clone, Copy, Debug)]
pub struct Code<'a> {
    pub issuer: &'a str,
    /// Its saved sites. When there are none, those its issuer suggests
    /// stand in.
    pub sites: &'a [Site],
    pub rules: &'a [Rule],
    /// How many times the user chose it under this context's [`Key`].
    pub times: u32,
}

/// What [`rank`] makes of a context.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ranking {
    /// The codes to put forward, best first.
    Candidates(Vec<Candidate>),
    /// The page looks like a code's site and is not it: put forward
    /// nothing, type nothing, and say why. Saving the page's site to the
    /// code it is for, if it really is, makes it a match.
    LookAlike(Warning),
}

/// A code put forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Where the code is in the slice given to [`rank`].
    pub code: usize,
    pub why: Reason,
}

/// Why a code is put forward, the strongest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    /// The page the browser shows is one of its sites.
    Site,
    /// One of its rules matches the window.
    Rule,
    /// The user chose it here before, this many times.
    Learned { times: u32 },
    /// The window's title, or its app's name, names its issuer.
    IssuerInTitle,
}

impl Reason {
    /// Whether the code may be typed for this reason: only on a page that
    /// the browser says is one of its sites.
    pub fn verified(self) -> bool {
        self == Reason::Site
    }

    fn strength(self) -> u8 {
        match self {
            Reason::Site => 3,
            Reason::Rule => 2,
            Reason::Learned { .. } => 1,
            Reason::IssuerInTitle => 0,
        }
    }
}

/// A page that looks like a site it is not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    /// The page's host, in ASCII: punycode shows what a homoglyph hides.
    pub page: String,
    /// The site it looks like the most.
    pub like: Site,
    pub how: Resemblance,
    /// The codes whose sites it looks like, as [`Candidate::code`].
    pub codes: Vec<usize>,
}

/// Puts forward the codes for `context`, best first; or, on a page that
/// looks like a code's site, none and a warning.
pub fn rank(context: &Context, codes: &[Code<'_>]) -> Ranking {
    let mut candidates = Vec::new();
    let mut look_alikes: Vec<(Resemblance, Site, usize)> = Vec::new();
    for (index, code) in codes.iter().enumerate() {
        let suggested;
        let sites = if code.sites.is_empty() {
            suggested = suggest(code.issuer);
            &suggested
        } else {
            code.sites
        };
        if let Some(page) = &context.page
            && !sites.is_empty()
        {
            let mut like: Option<(Resemblance, &Site)> = None;
            let mut matched = false;
            for site in sites {
                match page.check(site) {
                    Verdict::Match => matched = true,
                    Verdict::LooksLike(how) if like.is_none_or(|(surest, _)| how < surest) => {
                        like = Some((how, site));
                    }
                    Verdict::LooksLike(_) | Verdict::Other => {}
                }
            }
            if matched {
                candidates.push(Candidate {
                    code: index,
                    why: Reason::Site,
                });
            } else if let Some((how, site)) = like {
                look_alikes.push((how, site.clone(), index));
            }
            continue;
        }
        if let Some(why) = window_reason(context.window.as_ref(), code) {
            candidates.push(Candidate { code: index, why });
        }
    }

    let matched = candidates.iter().any(|c| c.why == Reason::Site);
    if !matched
        && let Some(page) = context.page.as_ref().and_then(Page::host)
        && let Some((how, like, _)) = look_alikes.iter().min_by_key(|(how, _, _)| *how)
    {
        return Ranking::LookAlike(Warning {
            page: page.to_owned(),
            like: like.clone(),
            how: *how,
            codes: look_alikes.iter().map(|&(_, _, code)| code).collect(),
        });
    }
    candidates.sort_by_key(|c| {
        (
            Reverse(c.why.strength()),
            Reverse(codes[c.code].times),
            c.code,
        )
    });
    Ranking::Candidates(candidates)
}

/// The strongest reason the window alone gives to put `code` forward.
fn window_reason(window: Option<&Window>, code: &Code<'_>) -> Option<Reason> {
    if window.is_some_and(|w| code.rules.iter().any(|rule| rule.matches(w))) {
        return Some(Reason::Rule);
    }
    if code.times > 0 {
        return Some(Reason::Learned { times: code.times });
    }
    let issuer = code.issuer.trim().to_lowercase();
    let named = |text: &str| has_word(&text.to_lowercase(), &issuer);
    window
        .is_some_and(|w| w.title.as_deref().is_some_and(named) || named(&w.app_name))
        .then_some(Reason::IssuerInTitle)
}

/// Whether `word` is in `text` and not part of a longer word.
fn has_word(text: &str, word: &str) -> bool {
    let joined = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
    !word.is_empty()
        && text.match_indices(word).any(|(at, _)| {
            !joined(text[..at].chars().next_back())
                && !joined(text[at + word.len()..].chars().next())
        })
}

/// Browsers that end their windows' titles with their name.
const BROWSERS: &[&str] = &[
    "google chrome",
    "chromium",
    "mozilla firefox",
    "firefox developer edition",
    "firefox nightly",
    "microsoft edge",
    "brave",
    "vivaldi",
    "opera",
    "safari",
    "librewolf",
    "waterfox",
    "zen browser",
    "tor browser",
];

const SEPARATORS: &[&str] = &[" - ", " – ", " — ", " | ", " · "];

/// A window's title as it is learned: lowercase, without the app's or the
/// browser's name at the end, counts, sizes, unsaved marks or characters
/// that do not show.
fn title_key(title: &str, app_name: &str) -> String {
    let mut text: String = title
        .chars()
        .filter(|&c| !invisible(c))
        .flat_map(char::to_lowercase)
        .collect();
    let app_name = app_name.trim().to_lowercase();
    let names = std::iter::once(app_name.as_str()).chain(BROWSERS.iter().copied());
    for name in names.filter(|name| !name.is_empty()) {
        if text == name {
            text.clear();
        }
        for separator in SEPARATORS {
            if let Some(rest) = text.strip_suffix(name)
                && let Some(rest) = rest.strip_suffix(separator)
            {
                text.truncate(rest.len());
            }
        }
    }
    let text = without_counts(&text);
    let words: Vec<&str> = text.split_whitespace().collect();
    let edges: &[char] = &['-', '–', '—', '|', '·', '•', '●', '*', ' '];
    words.join(" ").trim_matches(edges).to_owned()
}

/// Characters that take no room: zero-width spaces and joiners,
/// direction marks (Edge writes "Microsoft\u{200B} Edge").
fn invisible(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2064}' | '\u{FEFF}')
}

/// `text` without counts, `(3)`, `[12]`, `(99+)`, and sizes, `80×24`.
fn without_counts(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    let mut previous = None;
    while let Some(c) = rest.chars().next() {
        let after_digit = previous.is_some_and(|p: char| p.is_ascii_digit());
        let skipped = count(rest).or_else(|| if after_digit { None } else { size(rest) });
        if let Some(after) = skipped {
            kept.push(' ');
            rest = after;
            previous = None;
            continue;
        }
        kept.push(c);
        rest = &rest[c.len_utf8()..];
        previous = Some(c);
    }
    kept
}

/// What follows a count in brackets at the start of `text`.
fn count(text: &str) -> Option<&str> {
    let close = match text.chars().next()? {
        '(' => ')',
        '[' => ']',
        _ => return None,
    };
    let after = digits(&text[1..])?;
    after.strip_prefix('+').unwrap_or(after).strip_prefix(close)
}

/// What follows a size, columns × rows, at the start of `text`.
fn size(text: &str) -> Option<&str> {
    let after = digits(digits(text)?.strip_prefix('×')?)?;
    after
        .chars()
        .next()
        .is_none_or(|c| !c.is_alphanumeric())
        .then_some(after)
}

/// What follows one or more ASCII digits at the start of `text`.
fn digits(text: &str) -> Option<&str> {
    let after = text.trim_start_matches(|c: char| c.is_ascii_digit());
    (after.len() < text.len()).then_some(after)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(app_id: &str, app_name: &str, title: Option<&str>) -> Window {
        Window {
            app_id: app_id.to_owned(),
            app_name: app_name.to_owned(),
            title: title.map(str::to_owned),
            pid: Some(42),
        }
    }

    fn chrome(title: &str) -> Window {
        window("com.google.Chrome", "Google Chrome", Some(title))
    }

    fn terminal() -> Window {
        window("com.apple.Terminal", "Terminal", Some("ssh prod — 80×24"))
    }

    fn sites(hosts: &[&str]) -> &'static [Site] {
        let sites: Vec<Site> = hosts
            .iter()
            .map(|h| Site::parse(h).expect("site"))
            .collect();
        sites.leak()
    }

    fn rule(app: Option<&str>, title: Option<&str>) -> Rule {
        Rule {
            app: app.map(str::to_owned),
            title_contains: title.map(str::to_owned),
        }
    }

    fn context(window: Option<Window>, url: Option<&str>) -> Context {
        Context {
            window,
            page: url.map(Page::parse),
        }
    }

    fn code<'a>(issuer: &'a str, sites: &'a [Site], rules: &'a [Rule]) -> Code<'a> {
        Code {
            issuer,
            sites,
            rules,
            times: 0,
        }
    }

    fn candidates(ranking: Ranking) -> Vec<(usize, Reason)> {
        match ranking {
            Ranking::Candidates(candidates) => candidates.iter().map(|c| (c.code, c.why)).collect(),
            Ranking::LookAlike(warning) => panic!("warned: {warning:?}"),
        }
    }

    fn warning(ranking: Ranking) -> Warning {
        match ranking {
            Ranking::LookAlike(warning) => warning,
            Ranking::Candidates(candidates) => panic!("no warning: {candidates:?}"),
        }
    }

    #[test]
    fn a_site_match_ranks_above_everything() {
        let github = sites(&["github.com"]);
        let aws = sites(&["signin.aws.amazon.com"]);
        let always = [rule(Some("Google Chrome"), None)];
        let mut codes = [
            code("AWS", aws, &always),
            code("Personal", &[], &always),
            code("GitHub", github, &[]),
        ];
        codes[1].times = 50;
        let here = context(
            Some(chrome("Sign in to GitHub")),
            Some("https://github.com/login"),
        );
        assert_eq!(
            candidates(rank(&here, &codes)),
            [(2, Reason::Site), (1, Reason::Rule)],
            "AWS has sites, none of them this page: not put forward"
        );
    }

    #[test]
    fn every_code_for_the_site_more_chosen_first() {
        let github = sites(&["github.com"]);
        let gitlab = sites(&["gitlab.com"]);
        let mut codes = [
            code("GitHub", github, &[]),
            code("GitHub (work)", github, &[]),
            code("GitLab", gitlab, &[]),
        ];
        codes[1].times = 3;
        let here = context(Some(chrome("GitHub")), Some("https://gist.github.com/"));
        assert_eq!(
            candidates(rank(&here, &codes)),
            [(1, Reason::Site), (0, Reason::Site)]
        );
    }

    #[test]
    fn a_look_alike_page_puts_nothing_forward() {
        let github = sites(&["github.com"]);
        let always = [rule(Some("com.google.chrome"), None)];
        let mut codes = [code("GitHub", github, &[]), code("Personal", &[], &always)];
        codes[1].times = 9;
        let here = context(
            Some(chrome("Sign in to GitHub · GitHub")),
            Some("https://github.com.login-verify.io/session"),
        );
        let warning = warning(rank(&here, &codes));
        assert_eq!(warning.page, "github.com.login-verify.io");
        assert_eq!(warning.like.host(), "github.com");
        assert_eq!(warning.how, Resemblance::Embedded);
        assert_eq!(warning.codes, [0]);
    }

    #[test]
    fn the_surest_look_alike_is_the_one_warned_of() {
        let codes = [
            code("GitHub", sites(&["github.com"]), &[]),
            code("Gihub", sites(&["gihub.com"]), &[]),
        ];
        let here = context(None, Some("https://gіthub.com/"));
        let warning = warning(rank(&here, &codes));
        assert_eq!(warning.page, "xn--gthub-n2e.com");
        assert_eq!(warning.like.host(), "github.com");
        assert_eq!(warning.how, Resemblance::Homoglyph);
        assert_eq!(warning.codes, [0, 1]);
    }

    #[test]
    fn plain_http_on_the_site_is_warned_of() {
        let codes = [code("GitHub", sites(&["github.com"]), &[])];
        let warning = warning(rank(&context(None, Some("http://github.com/")), &codes));
        assert_eq!(warning.how, Resemblance::PlainHttp);
    }

    #[test]
    fn a_match_outweighs_a_look_alike() {
        // The company's own GitHub, saved: the page is it, not a fake github.com.
        let codes = [
            code("GitHub", sites(&["github.com"]), &[]),
            code("Acme GitHub", sites(&["github.acme.com"]), &[]),
        ];
        let here = context(None, Some("https://github.acme.com/login"));
        assert_eq!(candidates(rank(&here, &codes)), [(1, Reason::Site)]);
    }

    #[test]
    fn codes_without_sites_are_held_to_their_issuers() {
        let codes = [code("GitHub", &[], &[]), code("Acme VPN", &[], &[])];
        let real = context(Some(chrome("GitHub")), Some("https://github.com/"));
        assert_eq!(candidates(rank(&real, &codes)), [(0, Reason::Site)]);

        let fake = context(Some(chrome("GitHub")), Some("https://githuh.com/"));
        let warning = warning(rank(&fake, &codes));
        assert_eq!((warning.how, warning.codes), (Resemblance::Typo, vec![0]));

        // Neither the site nor like it: a code with sites stays back, even
        // where the title names it.
        let elsewhere = context(
            Some(chrome("GitHub login — Acme VPN")),
            Some("https://vpn.acme.example/"),
        );
        assert_eq!(
            candidates(rank(&elsewhere, &codes)),
            [(1, Reason::IssuerInTitle)]
        );
    }

    #[test]
    fn a_page_that_is_no_web_page_keeps_codes_with_sites_back() {
        let always = [rule(Some("Google Chrome"), None)];
        let codes = [code("GitHub", &[], &always), code("Acme", &[], &always)];
        let here = context(Some(chrome("Settings")), Some("chrome://settings"));
        assert_eq!(candidates(rank(&here, &codes)), [(1, Reason::Rule)]);
    }

    #[test]
    fn without_a_page_the_window_decides() {
        let aws = sites(&["signin.aws.amazon.com"]);
        let in_terminal = [rule(Some("terminal"), None)];
        let in_prod = [rule(None, Some("SSH PROD"))];
        let mut codes = [
            code("Amazon Web Services", aws, &in_terminal),
            code("Hetzner", &[], &[]),
            code("Prod", &[], &in_prod),
            code("Terminal", &[], &[]),
            code("GitHub", &[], &[]),
        ];
        codes[1].times = 2;
        codes[2].times = 7;
        assert_eq!(
            candidates(rank(&context(Some(terminal()), None), &codes)),
            [
                (2, Reason::Rule),
                (0, Reason::Rule),
                (1, Reason::Learned { times: 2 }),
                (3, Reason::IssuerInTitle),
            ]
        );
    }

    #[test]
    fn nothing_known_puts_forward_only_what_was_learned() {
        let mut codes = [code("GitHub", &[], &[]), code("Acme", &[], &[])];
        codes[1].times = 1;
        assert_eq!(
            candidates(rank(&Context::default(), &codes)),
            [(1, Reason::Learned { times: 1 })]
        );
    }

    #[test]
    fn rules() {
        let chrome_window = chrome("Jenkins — Build #12");
        assert!(rule(Some("Google Chrome"), None).matches(&chrome_window));
        assert!(rule(Some("COM.GOOGLE.CHROME"), None).matches(&chrome_window));
        assert!(rule(Some(" google chrome "), Some("jenkins")).matches(&chrome_window));
        assert!(!rule(Some("Firefox"), Some("jenkins")).matches(&chrome_window));
        assert!(!rule(Some("Google Chrome"), Some("Grafana")).matches(&chrome_window));
        assert!(!rule(None, None).matches(&chrome_window));
        assert!(!rule(Some(" "), Some("")).matches(&chrome_window));
        // Without the title (no permission to read it), title rules match nothing.
        let untitled = window("com.google.Chrome", "Google Chrome", None);
        assert!(!rule(None, Some("jenkins")).matches(&untitled));
        assert!(rule(Some("google chrome"), None).matches(&untitled));
        let windows_app = window("chrome.exe", "chrome", Some("x"));
        assert!(rule(Some("Chrome.exe"), None).matches(&windows_app));
        assert!(rule(Some("chrome"), None).matches(&windows_app));
    }

    #[test]
    fn issuers_named_as_whole_words() {
        let codes = [
            code("Git", &[], &[]),
            code("GitHub", &[], &[]),
            code("X", &[], &[]),
            code("Slack", &[], &[]),
            code("", &[], &[]),
        ];
        let here = context(
            Some(window(
                "com.tinyspeck.slackmacgap",
                "Slack",
                Some("GitHub: Box"),
            )),
            None,
        );
        assert_eq!(
            candidates(rank(&here, &codes)),
            [(1, Reason::IssuerInTitle), (3, Reason::IssuerInTitle)]
        );
    }

    #[test]
    fn keys_for_pages_are_their_registrable_domains() {
        let key = |url| context(Some(chrome("anything")), Some(url)).key();
        assert_eq!(
            key("https://gist.github.com/x").as_ref().map(Key::as_str),
            Some("site:github.com")
        );
        assert_eq!(
            key("https://github.com/"),
            key("https://www.github.com/login")
        );
        assert_eq!(
            key("http://192.168.1.10:5000/").as_ref().map(Key::as_str),
            Some("site:192.168.1.10")
        );
        // No web page: the window instead.
        assert_eq!(
            key("chrome://settings").as_ref().map(Key::as_str),
            Some("app:com.google.chrome\nanything")
        );
    }

    #[test]
    fn keys_for_windows_leave_out_what_changes() {
        let key = |w: Window| context(Some(w), None).key().map(String::from);
        let gmail = |title| key(chrome(title));
        assert_eq!(
            gmail("(3) Inbox - alice@example.com - Gmail - Google Chrome"),
            Some("app:com.google.chrome\ninbox - alice@example.com - gmail".to_owned())
        );
        assert_eq!(
            gmail("(3) Inbox - alice@example.com - Gmail - Google Chrome"),
            gmail("Inbox (12) - alice@example.com - Gmail - Google Chrome")
        );
        assert_eq!(gmail("Inbox [99+] - Gmail"), gmail("inbox - gmail"));
        assert_eq!(
            key(window(
                "firefox.exe",
                "firefox",
                Some("GitHub — Mozilla Firefox")
            )),
            Some("app:firefox.exe\ngithub".to_owned())
        );
        assert_eq!(
            key(window(
                "msedge.exe",
                "msedge",
                Some("Sign in - Work - Microsoft\u{200B} Edge")
            )),
            Some("app:msedge.exe\nsign in - work".to_owned())
        );
        assert_eq!(
            key(window(
                "com.apple.Terminal",
                "Terminal",
                Some("me@prod: ~ — -zsh — 80×24")
            )),
            key(window(
                "com.apple.Terminal",
                "Terminal",
                Some("me@prod: ~ — -zsh — 120×40")
            ))
        );
        assert_eq!(
            key(window(
                "com.apple.Terminal",
                "Terminal",
                Some("me@prod: ~ — -zsh — 80×24")
            )),
            Some("app:com.apple.terminal\nme@prod: ~ — -zsh".to_owned())
        );
        assert_eq!(
            key(window(
                "com.microsoft.VSCode",
                "Code",
                Some("● main.rs — authwing")
            )),
            key(window(
                "com.microsoft.VSCode",
                "Code",
                Some("main.rs — authwing")
            ))
        );
        assert_eq!(
            key(window("com.tinyspeck.slackmacgap", "Slack", Some("Slack"))),
            Some("app:com.tinyspeck.slackmacgap\n".to_owned())
        );
        assert_eq!(
            key(window("", "Some App", None)),
            Some("app:some app\n".to_owned())
        );
        // Numbers that are not counts stay: they may be what tells two places apart.
        assert_ne!(
            key(window(
                "com.apple.Terminal",
                "Terminal",
                Some("account 1234")
            )),
            key(window(
                "com.apple.Terminal",
                "Terminal",
                Some("account 5678")
            ))
        );
        assert_ne!(gmail("Order (2024-01) - Shop"), gmail("Order - Shop"));
        assert_eq!(Context::default().key(), None);
    }

    #[test]
    fn verified_only_by_site() {
        assert!(Reason::Site.verified());
        assert!(!Reason::Rule.verified());
        assert!(!Reason::Learned { times: 100 }.verified());
        assert!(!Reason::IssuerInTitle.verified());
    }
}
