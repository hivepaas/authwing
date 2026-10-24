//! What password managers keep beside a login: the key its one-time
//! passwords are made from, in the forms they write it, and the web
//! addresses the login is for. Bitwarden's export, its CSV and those of the
//! other password managers read them the same way.

use authwing_totp::{DEFAULT_PERIOD, Kind, Otp, Secret};
use url::Url;

use crate::uris::has_prefix;

/// The code `key` gives, in a form password managers write it
/// (https://bitwarden.com/help/integrated-authenticator/ lists Bitwarden's):
/// an `otpauth://` URI; `steam://` and the secret, Bitwarden's form for
/// Steam Guard; or a bare base32 secret, which makes a 30-second, six-digit
/// SHA-1 TOTP code. The login's `title` stands in for an issuer the key
/// does not give, and its `username` for an account.
pub(crate) fn code(key: &str, title: &str, username: &str) -> authwing_totp::Result<Otp> {
    let key = key.trim();
    let steam = "steam://";
    let mut otp = if has_prefix(key, "otpauth://") {
        Otp::from_uri(key)?
    } else if has_prefix(key, steam) {
        Otp::new(
            "",
            "",
            Secret::from_base32(&key[steam.len()..])?,
            Kind::Steam,
        )
    } else {
        let period = DEFAULT_PERIOD;
        Otp::new("", "", Secret::from_base32(key)?, Kind::Totp { period })
    };
    if otp.issuer.trim().is_empty() {
        otp.issuer = title.trim().to_string();
    }
    if otp.account.trim().is_empty() {
        otp.account = username.trim().to_string();
    }
    Ok(otp)
}

/// The hosts of the web addresses among `addresses`, each once, as the
/// browser has them: a login's sites. An address without a scheme is taken
/// as https, as Bitwarden does; one that is no web address (an app's, an
/// `ssh://` one) is passed over.
pub(crate) fn sites<'a>(addresses: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for address in addresses {
        let address = address.trim();
        let url = if address.contains("://") {
            Url::parse(address)
        } else {
            Url::parse(&format!("https://{address}"))
        };
        let Ok(url) = url else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https") {
            continue;
        }
        let host = url.host_str().unwrap_or_default().trim_end_matches('.');
        if !host.is_empty() && !hosts.iter().any(|known| known == host) {
            hosts.push(host.to_string());
        }
    }
    hosts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::code as uri_code;

    #[test]
    fn reads_each_form_of_key() {
        let deno = "otpauth://totp/Deno:Mason?secret=4SJHB4GSD43FZBAI7C2HLRJGPQ&issuer=Deno";
        assert_eq!(code(deno, "Title", "user"), Ok(uri_code(deno)));
        let steam = code(" steam://JRZCL47CMXVOQMNPZR2F7J4RGI ", "Steam", "gaben").expect("steam");
        assert_eq!(steam.kind, Kind::Steam);
        let bare = code("4sjh b4gs d43f zbai 7c2h lrjg pq", " Deno ", "Mason").expect("bare");
        assert_eq!(bare, uri_code(deno));
        assert_eq!(bare.kind, Kind::Totp { period: 30 });
        assert_eq!(bare.digits, 6);
        assert!(code("not a key!", "", "").is_err());
    }

    #[test]
    fn sites_are_the_hosts_of_web_addresses() {
        let sites = sites([
            "https://github.com/login",
            " gist.github.com ",
            "https://GitHub.com:443/settings",
            "androidapp://com.github.android",
            "",
            "https://example.com./",
        ]);
        assert_eq!(sites, ["github.com", "gist.github.com", "example.com"]);
    }
}
