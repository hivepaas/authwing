//! Bitwarden's JSON export, unencrypted
//! (https://bitwarden.com/help/condition-bitwarden-import/): logins whose
//! `login.totp` holds a code's key in a form Bitwarden takes
//! (https://bitwarden.com/help/integrated-authenticator/): an `otpauth://`
//! URI, a bare base32 secret, or `steam://` and the secret for Steam Guard.
//! Bitwarden Authenticator exports the same JSON.
//!
//! The login's name stands in for an issuer the key does not give, and its
//! username for an account. The hosts of its web addresses become the
//! code's sites (not those matched as a regular expression, or never), and
//! its folder the code's group. Logins without a key are not codes, and are
//! passed over without a word.
//!
//! An encrypted export (account-restricted, or protected by a password) is
//! refused with a word on exporting again: authwing does not decrypt them.

use authwing_totp::{DEFAULT_PERIOD, Kind, Otp, Secret};
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::uris::has_prefix;
use crate::{Error, Imported, Item, Result, damaged, label};

/// Bitwarden's URI match types that are no address: a regular expression,
/// and never.
const REGEX_MATCH: u8 = 4;
const NEVER_MATCH: u8 = 5;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    #[serde(default)]
    encrypted: bool,
    #[serde(default)]
    password_protected: bool,
    folders: Option<Vec<Folder>>,
    items: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct Folder {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: Option<String>,
    folder_id: Option<String>,
    login: Option<Login>,
}

#[derive(Deserialize)]
struct Login {
    username: Option<String>,
    uris: Option<Vec<LoginUri>>,
    totp: Option<String>,
}

#[derive(Deserialize)]
struct LoginUri {
    uri: Option<String>,
    #[serde(rename = "match")]
    matching: Option<u8>,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let file: File = serde_json::from_slice(bytes).map_err(damaged)?;
    if file.encrypted || file.password_protected {
        return Err(Error::Unsupported(
            "this Bitwarden export is encrypted, which authwing cannot read: \
             export the vault from Bitwarden again as .json, not encrypted"
                .into(),
        ));
    }
    let folders = file.folders.unwrap_or_default();

    let mut imported = Imported::default();
    for (n, value) in file.items.unwrap_or_default().into_iter().enumerate() {
        let item_no = || format!("item {}", n + 1);
        let entry: Entry = match serde_json::from_value(value) {
            Ok(entry) => entry,
            Err(e) => {
                imported.skipped.push((item_no(), e.to_string()));
                continue;
            }
        };
        let Some(login) = entry.login else {
            continue;
        };
        let Some(key) = login
            .totp
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
        else {
            continue;
        };
        let name = entry.name.unwrap_or_default();
        let username = login.username.unwrap_or_default();
        match code(key, &name, &username) {
            Ok(otp) => {
                let group = entry
                    .folder_id
                    .and_then(|id| folders.iter().find(|folder| folder.id == id))
                    .map(|folder| folder.name.clone());
                imported.items.push(Item {
                    otp,
                    group: group.unwrap_or_default(),
                    sites: hosts(login.uris.unwrap_or_default()),
                });
            }
            Err(e) => imported
                .skipped
                .push((label(&name, &username, item_no), e.to_string())),
        }
    }
    Ok(imported)
}

/// The code `key` gives, named by the login where it does not say.
fn code(key: &str, name: &str, username: &str) -> authwing_totp::Result<Otp> {
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
        otp.issuer = name.trim().to_string();
    }
    if otp.account.trim().is_empty() {
        otp.account = username.trim().to_string();
    }
    Ok(otp)
}

/// The hosts of the web addresses among `uris`, each once, as the browser
/// has them. An address without a scheme is taken as https, as Bitwarden
/// does.
fn hosts(uris: Vec<LoginUri>) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for login_uri in uris {
        if matches!(login_uri.matching, Some(REGEX_MATCH | NEVER_MATCH)) {
            continue;
        }
        let Some(uri) = login_uri.uri else {
            continue;
        };
        let uri = uri.trim();
        let url = if uri.contains("://") {
            Url::parse(uri)
        } else {
            Url::parse(&format!("https://{uri}"))
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
    use crate::Format;
    use crate::testing::{URIS, assert_codes, code as uri_code};
    use serde_json::json;

    #[test]
    fn reads_bitwardens_export() {
        let bytes = include_bytes!("../testdata/bitwarden.json");
        let imported = crate::import(bytes, None).expect("import");
        assert_codes(&imported.items[..3], &URIS[..3]);
        let steam = &imported.items[3].otp;
        assert_eq!(steam.kind, Kind::Steam);
        assert_eq!(steam.secret, uri_code(URIS[6]).secret);
        // The key gives no names: the login's stand in.
        assert_eq!(
            (steam.issuer.as_str(), steam.account.as_str()),
            ("Test Steam", "")
        );
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn names_files_and_places_codes_by_their_login() {
        let file = json!({
            "encrypted": false,
            "folders": [{"id": "F1", "name": "Work"}],
            "items": [
                {
                    "type": 1, "name": "GitHub", "folderId": "F1",
                    "login": {
                        "username": "alice",
                        "uris": [
                            {"match": null, "uri": "https://github.com/login"},
                            {"match": null, "uri": "gist.github.com"},
                            {"match": 1, "uri": "https://GitHub.com:443/settings"},
                            {"match": null, "uri": "androidapp://com.github.android"},
                            {"match": 4, "uri": "^https://.*\\.github\\.com/"},
                            {"match": null, "uri": null},
                        ],
                        "totp": "jbsw y3dp ehpk 3pxp",
                    },
                },
                {
                    "type": 1, "name": "ACME", "folderId": "gone",
                    "login": {"username": "bob", "uris": null,
                        "totp": "otpauth://totp/?secret=JBSWY3DPEHPK3PXP&digits=8"},
                },
                {
                    "type": 1, "name": "Kept", "folderId": null,
                    "login": {"username": "x", "totp": "otpauth://totp/Theirs:carol?secret=JBSWY3DPEHPK3PXP"},
                },
                {"type": 1, "name": "No code", "login": {"username": "dave", "totp": null}},
                {"type": 2, "name": "A note", "secureNote": {"type": 0}},
                {"type": 1, "name": "Broken", "login": {"username": "erin", "totp": "not a key!"}},
                {"type": 1, "name": 7},
            ],
        });
        let imported =
            crate::import(&serde_json::to_vec(&file).expect("json"), None).expect("import");
        let items: Vec<(&str, &str, &str, Vec<&str>)> = imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                let sites = item.sites.iter().map(String::as_str).collect();
                (
                    otp.issuer.as_str(),
                    otp.account.as_str(),
                    item.group.as_str(),
                    sites,
                )
            })
            .collect();
        assert_eq!(
            items,
            [
                (
                    "GitHub",
                    "alice",
                    "Work",
                    vec!["github.com", "gist.github.com"]
                ),
                ("ACME", "bob", "", vec![]),
                ("Theirs", "carol", "", vec![]),
            ]
        );
        assert_eq!(imported.items[0].otp.kind, Kind::Totp { period: 30 });
        assert_eq!(imported.items[1].otp.digits, 8);
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(skipped, ["Broken (erin)", "item 7"]);
        assert_eq!(
            imported.skipped[0].1,
            "'!' is not a base32 character (A to Z and 2 to 7)"
        );
    }

    #[test]
    fn encrypted_exports_say_how_to_export() {
        let password_protected = json!({
            "encrypted": true, "passwordProtected": true, "salt": "c2FsdA==",
            "kdfType": 0, "kdfIterations": 600000,
            "encKeyValidation_DO_NOT_EDIT": "2.x|y|z", "data": "2.x|y|z",
        });
        let account_restricted = json!({
            "encrypted": true, "encKeyValidation_DO_NOT_EDIT": "2.x|y|z",
            "folders": [], "items": [{"name": "2.x|y|z", "login": {"totp": "2.x|y|z"}}],
        });
        for file in [password_protected, account_restricted] {
            let bytes = serde_json::to_vec(&file).expect("json");
            assert_eq!(crate::detect(&bytes), Some(Format::Bitwarden));
            assert!(matches!(
                crate::import(&bytes, None),
                Err(Error::Unsupported(_))
            ));
        }
    }
}
