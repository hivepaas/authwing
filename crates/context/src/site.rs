//! Sites: whether a page is one a code belongs to, and whether it only
//! looks like one.
//!
//! A code is saved with the sites it is for, as hosts. A page belongs to a
//! site when its host is the site's or below it, over https; plain http is
//! taken only from `localhost` and IP addresses, which are the user's own
//! machine or network. Hosts are compared as the browser has them: through
//! IDNA (UTS 46) to ASCII, lowercased, without a trailing dot or the port.
//! A site that is itself a public suffix (`github.io`, `vercel.app`) covers
//! only itself: the hosts below it are other people's.
//!
//! A page that is not a code's site may look like it, and is then one to
//! warn about and type nothing into. Hosts are taken apart with the Public
//! Suffix List (the `psl` crate, which has the list compiled in): the
//! registrable domain says who owns a host, and its first label is the name
//! a phisher imitates. On another registrable domain, a page looks like a
//! site when:
//!
//! - its name is drawn the same: equal once both are reduced to their UTS 39
//!   skeletons (`gіthub.com` with a Cyrillic і, `rnicrosoft.com`,
//!   `paypa1.com`);
//! - it carries the site's name or domain in its own labels
//!   (`github.com.login-verify.io`, `github-login.com`);
//! - its name, or its whole domain, is a letter or two off (`githuh.com`,
//!   `gihtub.com`, `github.co`).
//!
//! The match is exact and these are guesses. They miss a name run into
//! other words (`githubsecurity.com`), other words for the brand, and look-
//! alikes UTS 39's table does not have; they flag some innocent pages
//! (`github-tips.io`, a company's own `github.example.com`), which is the
//! safe way to be wrong, and is undone by saving the page's site to its
//! code. The same name under another suffix (`amazon.de` for `amazon.com`)
//! is not flagged: it is as often the same company.

mod services;

pub use services::suggest;

use std::fmt;
use std::str::FromStr;

use url::{Host, Url};

/// A host a code is for, normalised as the browser would have it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Site {
    host: String,
    ip: bool,
}

/// Why text could not be taken as a site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteError {
    Empty,
    /// It is neither a host nor an http(s) address with one.
    Invalid,
}

impl fmt::Display for SiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SiteError::Empty => "no site given",
            SiteError::Invalid => "not a host name or web address",
        })
    }
}

impl std::error::Error for SiteError {}

impl Site {
    /// Takes what the user typed or an issuer gave: a host (`github.com`,
    /// `bücher.de`, `*.example.com`) or a web address, of which only the
    /// host is kept.
    pub fn parse(text: &str) -> Result<Site, SiteError> {
        let text = text.trim();
        let text = text
            .strip_prefix("*.")
            .unwrap_or(text)
            .trim_start_matches('.');
        if text.is_empty() {
            return Err(SiteError::Empty);
        }
        let url = if text.contains("://") {
            Url::parse(text)
        } else {
            Url::parse(&format!("https://{text}"))
        }
        .map_err(|_| SiteError::Invalid)?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(SiteError::Invalid);
        }
        let (host, ip) = host_of(&url).ok_or(SiteError::Invalid)?;
        Ok(Site { host, ip })
    }

    /// The host, in ASCII (punycode for international names).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// The host as people read it, international names in their own script.
    pub fn unicode(&self) -> String {
        idna::domain_to_unicode(&self.host).0
    }

    /// Who owns the host: its registrable domain (`amazon.com` for
    /// `signin.aws.amazon.com`), or the host itself for an IP address or a
    /// name with none (`localhost`).
    pub fn registrable(&self) -> &str {
        if self.ip {
            &self.host
        } else {
            registrable(&self.host)
        }
    }
}

impl fmt::Display for Site {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.host)
    }
}

impl FromStr for Site {
    type Err = SiteError;

    fn from_str(text: &str) -> Result<Site, SiteError> {
        Site::parse(text)
    }
}

/// The page a browser tab shows, as its address says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// None for what is no web page: the browser's own pages, files.
    host: Option<String>,
    ip: bool,
    /// Over https, or plain http where it stays on the user's own machine
    /// or network.
    secure: bool,
}

impl Page {
    /// Reads the address of a page. Anything but an http(s) address with a
    /// host is a page no site matches.
    pub fn parse(url: &str) -> Page {
        let none = Page {
            host: None,
            ip: false,
            secure: false,
        };
        let Ok(url) = Url::parse(url.trim()) else {
            return none;
        };
        let https = match url.scheme() {
            "https" => true,
            "http" => false,
            _ => return none,
        };
        let Some((host, ip)) = host_of(&url) else {
            return none;
        };
        let local = ip || host == "localhost" || host.ends_with(".localhost");
        Page {
            secure: https || local,
            host: Some(host),
            ip,
        }
    }

    /// The host, in ASCII, if this is a web page.
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    /// Who owns the host, as [`Site::registrable`].
    pub fn registrable(&self) -> Option<&str> {
        let host = self.host.as_deref()?;
        Some(if self.ip { host } else { registrable(host) })
    }

    /// Whether this page is `site`'s, or looks like it without being it.
    pub fn check(&self, site: &Site) -> Verdict {
        let Some(host) = self.host.as_deref() else {
            return Verdict::Other;
        };
        if covers(site, host) {
            return if self.secure {
                Verdict::Match
            } else {
                Verdict::LooksLike(Resemblance::PlainHttp)
            };
        }
        if self.ip || site.ip {
            return Verdict::Other;
        }
        resemblance(host, &site.host).map_or(Verdict::Other, Verdict::LooksLike)
    }
}

/// What a page is to a site.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The page is the site's, or below it, over a connection safe for it.
    Match,
    /// The page is not the site's but looks like it: no code goes there.
    LooksLike(Resemblance),
    /// The page has nothing to do with the site.
    Other,
}

/// How a page looks like a site it is not, the surest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Resemblance {
    /// It is the site's host, over plain http: anyone on the way could
    /// have written the page.
    PlainHttp,
    /// Its name is drawn with characters that look like the site's.
    Homoglyph,
    /// It carries the site's name or domain in its own labels.
    Embedded,
    /// Its name is a letter or two off the site's.
    Typo,
}

/// A URL's host as compared, and whether it is an IP address.
fn host_of(url: &Url) -> Option<(String, bool)> {
    match url.host()? {
        Host::Domain(name) => {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            (!name.is_empty()).then_some((name, false))
        }
        Host::Ipv4(address) => Some((address.to_string(), true)),
        Host::Ipv6(address) => Some((format!("[{address}]"), true)),
    }
}

/// `host`'s registrable domain, or `host` when it has none.
fn registrable(host: &str) -> &str {
    psl::domain_str(host).unwrap_or(host)
}

/// Whether `host` is the site's host or below it. Not below a public
/// suffix: `github.io` saved does not cover `mallory.github.io`.
fn covers(site: &Site, host: &str) -> bool {
    if host == site.host {
        return true;
    }
    !site.ip
        && host
            .strip_suffix(site.host.as_str())
            .is_some_and(|below| below.ends_with('.'))
        && psl::domain_str(&site.host).is_some()
}

/// How the domain `page` looks like the domain `site` without being it.
fn resemblance(page: &str, site: &str) -> Option<Resemblance> {
    let page_domain = registrable(page);
    let site_domain = registrable(site);
    if page_domain == site_domain {
        // The same owner: another of its hosts, not a look-alike.
        return None;
    }
    let page_name = first_label(page_domain);
    let site_name = first_label(site_domain);
    let renamed = page_name != site_name;
    let below = page[..page.len() - page_domain.len()].trim_end_matches('.');

    let skeleton_of = |ascii: &str| skeleton(&unicode(ascii));
    let site_name_skeleton = skeleton_of(site_name);
    if (renamed && skeleton_of(page_name) == site_name_skeleton)
        || skeleton_of(page_domain) == skeleton_of(site_domain)
    {
        return Some(Resemblance::Homoglyph);
    }

    let site_domain_skeleton = skeleton_of(site_domain);
    let below_skeleton = skeleton_of(below);
    let embedded = has_token(&below_skeleton, &site_domain_skeleton)
        || (renamed && has_token(&skeleton_of(page), &site_domain_skeleton))
        // A short name (`x`, `ea`) is in too many hosts to mean anything.
        || (site_name_skeleton.chars().count() >= 4
            && (has_token(&below_skeleton, &site_name_skeleton)
                || (renamed && has_token(&skeleton_of(page_name), &site_name_skeleton))));
    if embedded {
        return Some(Resemblance::Embedded);
    }

    let near_names = renamed && edits(page_name, site_name) <= typos_allowed(&unicode(site_name));
    let near_domains =
        unicode(site_domain).chars().count() >= 6 && edits(page_domain, site_domain) <= 1;
    (near_names || near_domains).then_some(Resemblance::Typo)
}

/// The fewest edits (a letter in, out, changed, or two swapped) between
/// two names, as written or as their skeletons, so that a typo and a
/// homoglyph together are seen as well.
fn edits(page: &str, site: &str) -> usize {
    let (page, site) = (unicode(page), unicode(site));
    let written = strsim::osa_distance(&page, &site);
    written.min(strsim::osa_distance(&skeleton(&page), &skeleton(&site)))
}

/// How many edits make a name a typo of `name`: none for short names, any
/// of which is a letter off many others.
fn typos_allowed(name: &str) -> usize {
    match name.chars().count() {
        0..5 => 0,
        5..9 => 1,
        _ => 2,
    }
}

fn first_label(domain: &str) -> &str {
    domain.split_once('.').map_or(domain, |(label, _)| label)
}

fn unicode(ascii: &str) -> String {
    idna::domain_to_unicode(ascii).0
}

/// UTS 39's skeleton, lowercased, so `0` and `o` meet as well.
fn skeleton(text: &str) -> String {
    unicode_security::skeleton(text)
        .collect::<String>()
        .to_lowercase()
}

/// Whether `needle` is in `hay` as whole labels or hyphenated words.
fn has_token(hay: &str, needle: &str) -> bool {
    let edge = |c: Option<char>| c.is_none_or(|c| c == '.' || c == '-');
    !needle.is_empty()
        && hay.match_indices(needle).any(|(at, _)| {
            edge(hay[..at].chars().next_back()) && edge(hay[at + needle.len()..].chars().next())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(text: &str) -> Site {
        Site::parse(text).expect("site")
    }

    fn check(url: &str, saved: &str) -> Verdict {
        Page::parse(url).check(&site(saved))
    }

    #[test]
    fn sites_are_hosts_normalised() {
        assert_eq!(site("github.com").host(), "github.com");
        assert_eq!(site(" GitHub.COM. ").host(), "github.com");
        assert_eq!(site("https://github.com/login?x=1").host(), "github.com");
        assert_eq!(site("github.com:8443/path").host(), "github.com");
        assert_eq!(site("*.example.com").host(), "example.com");
        assert_eq!(site("bücher.de").host(), "xn--bcher-kva.de");
        assert_eq!(site("bücher.de").unicode(), "bücher.de");
        assert_eq!(site("192.168.1.10:5000").host(), "192.168.1.10");
        assert_eq!(site("[::1]").host(), "[::1]");
        assert_eq!(Site::parse("  "), Err(SiteError::Empty));
        assert_eq!(Site::parse("ftp://github.com"), Err(SiteError::Invalid));
        assert_eq!(Site::parse("not a host"), Err(SiteError::Invalid));
        assert_eq!("github.com".parse::<Site>(), Ok(site("GITHUB.com")));
    }

    #[test]
    fn registrable_domains() {
        assert_eq!(site("signin.aws.amazon.com").registrable(), "amazon.com");
        assert_eq!(site("www.example.co.uk").registrable(), "example.co.uk");
        assert_eq!(site("alice.github.io").registrable(), "alice.github.io");
        assert_eq!(site("localhost").registrable(), "localhost");
        assert_eq!(site("10.0.0.1").registrable(), "10.0.0.1");
        let page = Page::parse("https://www.github.com.login-verify.io/x");
        assert_eq!(page.registrable(), Some("login-verify.io"));
        assert_eq!(Page::parse("about:blank").registrable(), None);
    }

    #[test]
    fn the_host_and_below_match_over_https() {
        assert_eq!(
            check("https://github.com/login", "github.com"),
            Verdict::Match
        );
        assert_eq!(
            check("https://gist.github.com/", "github.com"),
            Verdict::Match
        );
        assert_eq!(
            check("https://a.b.github.com/", "github.com"),
            Verdict::Match
        );
        assert_eq!(
            check(
                "https://signin.aws.amazon.com/oauth",
                "signin.aws.amazon.com"
            ),
            Verdict::Match
        );
        assert_eq!(
            check(
                "https://us-east-1.signin.aws.amazon.com/",
                "signin.aws.amazon.com"
            ),
            Verdict::Match
        );
    }

    #[test]
    fn hosts_compare_as_the_browser_has_them() {
        assert_eq!(check("https://GitHub.COM/", "github.com"), Verdict::Match);
        assert_eq!(check("https://github.com./", "github.com"), Verdict::Match);
        assert_eq!(
            check("https://github.com:8443/", "github.com"),
            Verdict::Match
        );
        assert_eq!(check("https://github.com/", "github.com."), Verdict::Match);
        assert_eq!(
            check("https://bücher.de/", "xn--bcher-kva.de"),
            Verdict::Match
        );
        assert_eq!(
            check("https://xn--bcher-kva.de/", "BÜCHER.de"),
            Verdict::Match
        );
        assert_eq!(
            check("https://shop.bücher.de/", "bücher.de"),
            Verdict::Match
        );
    }

    #[test]
    fn other_hosts_do_not_match() {
        // Above the saved host, beside it, or only ending in the same letters.
        assert_ne!(
            check("https://github.com/", "gist.github.com"),
            Verdict::Match
        );
        assert_ne!(
            check("https://api.github.com/", "gist.github.com"),
            Verdict::Match
        );
        assert_ne!(
            check("https://notgithub.com/", "github.com"),
            Verdict::Match
        );
        assert_ne!(
            check("https://github.com.evil.io/", "github.com"),
            Verdict::Match
        );
        assert_eq!(check("https://example.org/", "github.com"), Verdict::Other);
        // The user name is not the host.
        assert_ne!(
            check("https://github.com@evil.io/", "github.com"),
            Verdict::Match
        );
        assert_ne!(
            check("https://evil.io/github.com", "github.com"),
            Verdict::Match
        );
        assert_ne!(
            check("https://evil.io/?u=https://github.com", "github.com"),
            Verdict::Match
        );
    }

    #[test]
    fn a_public_suffix_covers_only_itself() {
        assert_ne!(
            check("https://mallory.github.io/", "github.io"),
            Verdict::Match
        );
        assert_ne!(
            check("https://evil.vercel.app/", "vercel.app"),
            Verdict::Match
        );
        assert_ne!(check("https://example.com/", "com"), Verdict::Match);
        assert_eq!(
            check("https://alice.github.io/", "alice.github.io"),
            Verdict::Match
        );
        assert_eq!(
            check("https://x.alice.github.io/", "alice.github.io"),
            Verdict::Match
        );
        assert_ne!(
            check("https://bob.github.io/", "alice.github.io"),
            Verdict::Match
        );
    }

    #[test]
    fn plain_http_only_where_it_stays_local() {
        assert_eq!(
            check("http://github.com/", "github.com"),
            Verdict::LooksLike(Resemblance::PlainHttp)
        );
        assert_eq!(
            check("http://gist.github.com/", "github.com"),
            Verdict::LooksLike(Resemblance::PlainHttp)
        );
        assert_eq!(check("http://localhost:8080/", "localhost"), Verdict::Match);
        assert_eq!(
            check("http://app.localhost/", "app.localhost"),
            Verdict::Match
        );
        assert_eq!(
            check("http://192.168.1.10:5000/", "192.168.1.10"),
            Verdict::Match
        );
        assert_eq!(
            check("https://192.168.1.10/", "192.168.1.10"),
            Verdict::Match
        );
        assert_eq!(check("http://[::1]:3000/", "[::1]"), Verdict::Match);
        // The same address written otherwise.
        assert_eq!(check("http://0xc0a8010a/", "192.168.1.10"), Verdict::Match);
        assert_eq!(
            check("http://192.168.1.11/", "192.168.1.10"),
            Verdict::Other
        );
        // Names that only point at local addresses are still names.
        assert_eq!(
            check("http://127.0.0.1.nip.io/", "127.0.0.1.nip.io"),
            Verdict::LooksLike(Resemblance::PlainHttp)
        );
    }

    #[test]
    fn what_is_no_web_page_matches_nothing() {
        for url in [
            "chrome://settings",
            "about:blank",
            "file:///Users/me/github.com",
            "ftp://github.com/",
            "wss://github.com/",
            "data:text/html,github.com",
            "",
            "github.com",
            "https://xn--zz/",
        ] {
            let page = Page::parse(url);
            assert_eq!(page.host(), None, "{url}");
            assert_eq!(page.check(&site("github.com")), Verdict::Other, "{url}");
        }
    }

    #[test]
    fn the_site_inside_another_domain_looks_like_it() {
        for url in [
            "https://github.com.login-verify.io/session",
            "https://www.github.com.login-verify.io/",
            "https://github.com-login.verify.io/",
            "https://github.evil.io/",
            "https://github-login.com/",
            "https://secure-github.com/",
            "https://login.github-sso.net/",
            "https://github-com.vercel.app/",
        ] {
            assert_eq!(
                check(url, "github.com"),
                Verdict::LooksLike(Resemblance::Embedded),
                "{url}"
            );
        }
        assert_eq!(
            check(
                "https://signin.aws.amazon.com.evil.io/",
                "signin.aws.amazon.com"
            ),
            Verdict::LooksLike(Resemblance::Embedded)
        );
        assert_eq!(
            check("https://x.com.verify-account.io/", "x.com"),
            Verdict::LooksLike(Resemblance::Embedded)
        );
        assert_eq!(
            check("https://x.com-security.io/", "x.com"),
            Verdict::LooksLike(Resemblance::Embedded)
        );
    }

    #[test]
    fn homoglyphs_look_like_it() {
        for url in [
            // Cyrillic і, Latin dotless ı, Cyrillic о, ASCII look-alikes.
            "https://gіthub.com/",
            "https://xn--gthub-n4a.com/",
            "https://www.gіthub.com/",
            "https://gіthub.io/",
            "https://github.corn/",
        ] {
            assert_eq!(
                check(url, "github.com"),
                Verdict::LooksLike(Resemblance::Homoglyph),
                "{url}"
            );
        }
        assert_eq!(
            check("https://rnicrosoft.com/", "microsoft.com"),
            Verdict::LooksLike(Resemblance::Homoglyph)
        );
        assert_eq!(
            check("https://paypa1.com/", "paypal.com"),
            Verdict::LooksLike(Resemblance::Homoglyph)
        );
        assert_eq!(
            check("https://g00gle.com/", "google.com"),
            Verdict::LooksLike(Resemblance::Homoglyph)
        );
        // Cyrillic р, а and у, with a Latin l.
        assert_eq!(
            check("https://раураl.com/", "paypal.com"),
            Verdict::LooksLike(Resemblance::Homoglyph)
        );
        // UTS 39 takes the Cyrillic ӏ for an i, not an l: a letter off.
        assert_eq!(
            check("https://аррӏе.com/", "apple.com"),
            Verdict::LooksLike(Resemblance::Typo)
        );
    }

    #[test]
    fn typos_look_like_it() {
        for url in [
            "https://githuh.com/",
            "https://gihtub.com/",
            "https://githubb.com/",
            "https://gthub.com/",
            "https://git-hub.com/",
            "https://github.co/",
            "https://githuh.io/",
        ] {
            assert_eq!(
                check(url, "github.com"),
                Verdict::LooksLike(Resemblance::Typo),
                "{url}"
            );
        }
        assert_eq!(
            check("https://micorsoft.com/", "microsoft.com"),
            Verdict::LooksLike(Resemblance::Typo)
        );
        assert_eq!(
            check("https://digitalocaen.com/", "digitalocean.com"),
            Verdict::LooksLike(Resemblance::Typo)
        );
    }

    #[test]
    fn neighbours_that_are_not_look_alikes() {
        // Another site of the same owner, the same name under another
        // suffix, different names, short names a letter apart.
        for (url, saved) in [
            ("https://www.amazon.com/", "signin.aws.amazon.com"),
            ("https://github.com/", "gist.github.com"),
            ("https://amazon.de/", "amazon.com"),
            ("https://www.amazon.com.au/", "amazon.com"),
            ("https://accounts.google.co.uk/", "google.com"),
            ("https://npmjs.org/", "npmjs.com"),
            ("https://github.blog/", "github.com"),
            ("https://gitlab.com/", "github.com"),
            ("https://githubstatus.com/", "github.com"),
            ("https://githubusercontent.com/", "github.com"),
            ("https://mallory.github.io/", "github.com"),
            ("https://microsoftonline.com/", "microsoft.com"),
            ("https://t.co/", "x.com"),
            ("https://xyz.com/", "x.com"),
            ("https://ea.org/", "ea.com"),
            ("https://bitwarden.eu/", "bitwarden.com"),
            ("https://example.org/", "github.com"),
            ("https://10.0.0.1/", "github.com"),
            ("https://github.com/", "10.0.0.1"),
        ] {
            assert_eq!(check(url, saved), Verdict::Other, "{url} for {saved}");
        }
    }

    #[test]
    fn surest_resemblance_first() {
        assert!(Resemblance::PlainHttp < Resemblance::Homoglyph);
        assert!(Resemblance::Homoglyph < Resemblance::Embedded);
        assert!(Resemblance::Embedded < Resemblance::Typo);
    }

    #[test]
    fn tokens_are_whole_labels_or_words() {
        assert!(has_token("github", "github"));
        assert!(has_token("www.github.com", "github"));
        assert!(has_token("github-login", "github"));
        assert!(!has_token("mygithub", "github"));
        assert!(!has_token("githubs", "github"));
        assert!(!has_token("github", ""));
    }
}
