//! Sites suggested from an issuer's name, for codes saved without any.
//!
//! A small table of common services, written by hand. 2FAS publishes a far
//! larger open-source list of services and their domains (the one behind
//! its icons) that could replace it, generated into this file or read at
//! build time; this one is enough to start with and to test against.

use super::Site;

/// A service: its names as issuers give them, normalised by [`name_key`],
/// and the sites its codes are typed into.
struct Service {
    names: &'static [&'static str],
    sites: &'static [&'static str],
}

const SERVICES: &[Service] = &[
    service(&["github"], &["github.com"]),
    service(&["gitlab"], &["gitlab.com"]),
    service(&["bitbucket"], &["bitbucket.org"]),
    service(&["google", "gmail", "googleworkspace"], &["google.com"]),
    service(
        &[
            "microsoft",
            "microsoftaccount",
            "outlook",
            "azure",
            "azuread",
            "entraid",
        ],
        &["microsoft.com", "live.com", "microsoftonline.com"],
    ),
    service(
        &["amazonwebservices", "aws", "awsconsole"],
        &["aws.amazon.com"],
    ),
    service(&["amazon"], &["amazon.com"]),
    service(&["cloudflare"], &["cloudflare.com"]),
    service(&["slack"], &["slack.com"]),
    service(&["discord"], &["discord.com"]),
    service(&["dropbox"], &["dropbox.com"]),
    service(&["paypal"], &["paypal.com"]),
    service(&["binance"], &["binance.com"]),
    service(&["coinbase"], &["coinbase.com"]),
    service(&["kraken"], &["kraken.com"]),
    service(&["facebook", "meta"], &["facebook.com"]),
    service(&["instagram"], &["instagram.com"]),
    service(&["x", "twitter", "xtwitter"], &["x.com", "twitter.com"]),
    service(&["linkedin"], &["linkedin.com"]),
    service(
        &["apple", "appleid", "icloud"],
        &["apple.com", "icloud.com"],
    ),
    service(&["atlassian", "jira", "confluence"], &["atlassian.com"]),
    service(&["digitalocean"], &["digitalocean.com"]),
    service(
        &["hetzner", "hetznercloud"],
        &["hetzner.com", "hetzner.cloud"],
    ),
    service(
        &["linode", "akamai", "akamaicloud"],
        &["linode.com", "akamai.com"],
    ),
    service(&["ovh", "ovhcloud"], &["ovh.com", "ovhcloud.com"]),
    service(
        &["proton", "protonmail", "protonag"],
        &["proton.me", "protonmail.com"],
    ),
    service(&["bitwarden"], &["bitwarden.com", "bitwarden.eu"]),
    service(&["1password"], &["1password.com"]),
    service(&["fastmail"], &["fastmail.com"]),
    service(&["npm", "npmjs"], &["npmjs.com"]),
    service(&["pypi", "pythonpackageindex"], &["pypi.org"]),
    service(&["dockerhub", "docker"], &["docker.com"]),
    service(&["reddit"], &["reddit.com"]),
    service(&["twitch"], &["twitch.tv"]),
    service(
        &["steam", "valve"],
        &["steampowered.com", "steamcommunity.com"],
    ),
    service(&["epicgames", "epic"], &["epicgames.com"]),
    service(&["okta"], &["okta.com"]),
    service(&["zoho"], &["zoho.com", "zoho.eu"]),
    service(&["stripe"], &["stripe.com"]),
    service(&["shopify"], &["shopify.com"]),
    service(&["namecheap"], &["namecheap.com"]),
    service(&["godaddy"], &["godaddy.com"]),
    service(&["porkbun"], &["porkbun.com"]),
    service(&["vercel"], &["vercel.com"]),
    service(&["netlify"], &["netlify.com"]),
    service(&["heroku"], &["heroku.com"]),
    service(&["tailscale"], &["tailscale.com"]),
    service(&["backblaze"], &["backblaze.com"]),
    service(&["openai", "chatgpt"], &["openai.com", "chatgpt.com"]),
    service(&["huggingface"], &["huggingface.co"]),
    service(
        &["mozilla", "firefox", "mozillaaccount"],
        &["mozilla.org", "firefox.com"],
    ),
    service(&["yahoo"], &["yahoo.com"]),
    service(&["zoom"], &["zoom.us"]),
    service(&["notion"], &["notion.so"]),
    service(&["figma"], &["figma.com"]),
    service(&["wise", "transferwise"], &["wise.com"]),
    service(&["sentry"], &["sentry.io"]),
    service(&["nintendo"], &["nintendo.com"]),
    service(&["battlenet", "blizzard"], &["battle.net", "blizzard.com"]),
];

const fn service(names: &'static [&'static str], sites: &'static [&'static str]) -> Service {
    Service { names, sites }
}

/// Sites a code from `issuer` is likely typed into: the table's, or the
/// issuer itself when it is a domain name (`github.com`, `login.example.org`).
/// None when it says nothing known.
pub fn suggest(issuer: &str) -> Vec<Site> {
    let key = name_key(issuer);
    if let Some(service) = SERVICES.iter().find(|s| s.names.contains(&key.as_str())) {
        return service
            .sites
            .iter()
            .filter_map(|s| Site::parse(s).ok())
            .collect();
    }
    domain_name(issuer).into_iter().collect()
}

/// `issuer` as the table has names: lowercase letters and digits only, and
/// without what is in parentheses ("Google (work)").
fn name_key(issuer: &str) -> String {
    let name = issuer.split('(').next().unwrap_or(issuer);
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The issuer as a site, when it is a domain name under a suffix the list
/// knows: not "Acme Inc." nor "intranet".
fn domain_name(issuer: &str) -> Option<Site> {
    let issuer = issuer.trim();
    if !issuer.contains('.') || issuer.contains(char::is_whitespace) {
        return None;
    }
    let site = Site::parse(issuer).ok()?;
    let known = !site.ip && psl::suffix(site.host.as_bytes()).is_some_and(|s| s.is_known());
    (known && psl::domain_str(&site.host).is_some()).then_some(site)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hosts(issuer: &str) -> Vec<String> {
        suggest(issuer)
            .iter()
            .map(|s| s.host().to_owned())
            .collect()
    }

    #[test]
    fn names_as_issuers_write_them() {
        assert_eq!(hosts("GitHub"), ["github.com"]);
        assert_eq!(hosts("github"), ["github.com"]);
        assert_eq!(hosts("Amazon Web Services"), ["aws.amazon.com"]);
        assert_eq!(hosts("AWS"), ["aws.amazon.com"]);
        assert_eq!(hosts("Google (work)"), ["google.com"]);
        assert_eq!(hosts("X"), ["x.com", "twitter.com"]);
        assert_eq!(hosts("Epic Games"), ["epicgames.com"]);
        assert_eq!(hosts("Docker Hub"), ["docker.com"]);
        assert_eq!(hosts("Battle.net"), ["battle.net", "blizzard.com"]);
    }

    #[test]
    fn an_issuer_that_is_a_domain_is_its_site() {
        assert_eq!(hosts("login.example.org"), ["login.example.org"]);
        assert_eq!(hosts("Example.COM"), ["example.com"]);
    }

    #[test]
    fn nothing_for_unknown_issuers() {
        assert!(suggest("Acme Inc.").is_empty());
        assert!(suggest("Acme").is_empty());
        assert!(suggest("intranet.local").is_empty());
        assert!(suggest("github.io").is_empty());
        assert!(suggest("10.0.0.1").is_empty());
        assert!(suggest("").is_empty());
        // Only whole names: not a service whose name starts the same.
        assert!(suggest("Applebee's").is_empty());
    }

    #[test]
    fn the_table_holds_together() {
        let mut names = std::collections::HashSet::new();
        for service in SERVICES {
            for name in service.names {
                assert_eq!(*name, name_key(name), "{name} is not normalised");
                assert!(names.insert(*name), "{name} is in the table twice");
            }
            for text in service.sites {
                let site = Site::parse(text).expect("site");
                assert_eq!(site.host(), *text);
                // A public suffix would cover only itself, not the service's hosts.
                assert!(psl::domain_str(text).is_some(), "{text} is a public suffix");
            }
            assert!(!service.sites.is_empty());
        }
        assert!(SERVICES.len() >= 40);
    }
}
