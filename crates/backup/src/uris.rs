//! `otpauth://` URIs, one per line: the plain export, and what most apps
//! export and import.
//!
//! Reading takes Google Authenticator's export among them
//! (`otpauth-migration://`, a line for each of its QR codes), and Ente
//! Auth's plain-text export, which is such a list with a parameter of its
//! own, `codeDisplay`: JSON of how Ente Auth shows the code. Its first tag
//! is taken as the group and a code in its trash is skipped; pins, notes
//! and icons are left behind. Ente Auth writes the label as typed, without
//! escaping it, so a `#` before the query is part of a name, as Ente Auth
//! reads it too, not the start of a fragment
//! (https://github.com/ente-io/ente/blob/main/mobile/apps/auth/lib/models/code.dart
//! and `code_display.dart`; the export, `ui/settings/data/export_widget.dart`).
//!
//! Steam codes are written as `otpauth://steam/`, the form Aegis
//! (`GoogleAuthInfo.parseUri`), 2FAS (the links in its backups) and Ente
//! Auth (`Code._getType`) read as Steam, keeping authwing's `encoder=steam`.

use std::borrow::Cow;

use authwing_totp::{Kind, Migration, Otp};
use percent_encoding::percent_decode_str;
use serde::Deserialize;

use crate::{Error, Imported, Item, Result, damaged, label};

const OTPAUTH: &str = "otpauth://";
const MIGRATION: &str = "otpauth-migration://";

/// The codes as `otpauth://` URIs, one per line, as Aegis, 2FAS, Ente Auth,
/// Bitwarden and most other apps import them. Groups and sites are left
/// out, and nothing is encrypted: whoever reads the file can make the codes.
pub fn plain(items: &[Item]) -> String {
    items
        .iter()
        .map(|item| portable(&item.otp) + "\n")
        .collect()
}

/// The URI other apps read best for `otp` (see the module docs).
pub(crate) fn portable(otp: &Otp) -> String {
    let mut uri = otp.to_uri();
    let totp = "otpauth://totp/";
    if otp.kind == Kind::Steam && uri.starts_with(totp) {
        uri.replace_range(..totp.len(), "otpauth://steam/");
    }
    uri
}

/// Whether `line` is a URI a code list holds.
pub(crate) fn is_uri(line: &str) -> bool {
    let line = line.trim();
    has_prefix(line, OTPAUTH) || has_prefix(line, MIGRATION)
}

/// The name a URI gives its code, its label (`Issuer:account`), to say
/// which one could not be read.
pub(crate) fn what(uri: &str) -> Option<String> {
    let uri = uri.trim();
    if !has_prefix(uri, OTPAUTH) {
        return None;
    }
    let path = uri[OTPAUTH.len()..].split(['?', '#']).next()?;
    let (_, label) = path.split_once('/')?;
    let label = percent_decode_str(label)
        .decode_utf8_lossy()
        .trim()
        .to_string();
    (!label.is_empty()).then_some(label)
}

/// What Ente Auth's `codeDisplay` says that authwing keeps.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Display {
    tags: Vec<String>,
    trashed: bool,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let text = std::str::from_utf8(bytes).map_err(damaged)?;
    if text.trim_start().starts_with('{') {
        return Err(Error::Unsupported(
            "this is Ente Auth's encrypted export, which authwing cannot read: \
             export the codes from Ente Auth as plain text"
                .into(),
        ));
    }
    let mut imported = Imported::default();
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let line_no = || format!("line {}", n + 1);
        if has_prefix(line, MIGRATION) {
            match Migration::parse(line) {
                Ok(migration) => {
                    imported
                        .items
                        .extend(migration.codes.into_iter().map(Item::new));
                    imported.skipped.extend(
                        migration
                            .skipped
                            .into_iter()
                            .map(|s| (label(&s.issuer, &s.account, line_no), s.reason.to_string())),
                    );
                }
                Err(e) => imported.skipped.push((line_no(), e.to_string())),
            }
            continue;
        }
        let name = || what(line).unwrap_or_else(line_no);
        let display = display(line);
        if display.trashed {
            imported
                .skipped
                .push((name(), "it is in Ente Auth's trash".into()));
            continue;
        }
        match Otp::from_uri(&label_hashes_escaped(line)) {
            Ok(otp) => imported.items.push(Item {
                otp,
                group: display
                    .tags
                    .iter()
                    .map(|tag| tag.trim())
                    .find(|tag| !tag.is_empty())
                    .unwrap_or_default()
                    .to_string(),
                sites: Vec::new(),
            }),
            Err(e) => imported.skipped.push((name(), e.to_string())),
        }
    }
    Ok(imported)
}

/// Ente Auth's `codeDisplay` in `uri`, decoded as Dart decodes a query
/// (`+` is a space); nothing if there is none or it is not its JSON.
fn display(uri: &str) -> Display {
    let query = uri.split_once('?').map_or("", |(_, query)| query);
    let query = query.split('#').next().unwrap_or_default();
    query
        .split('&')
        .find_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            name.eq_ignore_ascii_case("codeDisplay").then_some(value)
        })
        .and_then(|value| {
            let json = percent_decode_str(&value.replace('+', " "))
                .decode_utf8_lossy()
                .into_owned();
            serde_json::from_str(&json).ok()
        })
        .unwrap_or_default()
}

/// `uri` with any `#` in its label escaped, as part of the name.
fn label_hashes_escaped(uri: &str) -> Cow<'_, str> {
    let end = uri.find('?').unwrap_or(uri.len());
    if uri[..end].contains('#') {
        Cow::Owned(uri[..end].replace('#', "%23") + &uri[end..])
    } else {
        Cow::Borrowed(uri)
    }
}

/// Whether `text` starts with `prefix`, in any case (QR codes' upper case).
pub(crate) fn has_prefix(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes, code};
    use crate::{Format, crypto};
    use base64::Engine;

    #[test]
    fn plain_export_reads_back() {
        let items: Vec<Item> = URIS
            .iter()
            .map(|uri| Item {
                otp: code(uri),
                group: "Work".into(),
                sites: vec!["example.com".into()],
            })
            .collect();
        let text = plain(&items);
        assert_eq!(text.lines().count(), URIS.len());
        assert!(text.lines().all(|line| line.starts_with("otpauth://")));
        assert!(!text.contains("Work") && !text.contains("example.com"));
        let imported = crate::import(text.as_bytes(), None).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
        assert!(imported.items.iter().all(|item| item.group.is_empty()));
    }

    #[test]
    fn steam_is_written_as_other_apps_read_it() {
        let steam = code(URIS[6]);
        let uri = portable(&steam);
        assert_eq!(
            uri,
            "otpauth://steam/Boeing:Sophia?secret=JRZCL47CMXVOQMNPZR2F7J4RGI\
             &issuer=Boeing&encoder=steam"
        );
        assert_eq!(Otp::from_uri(&uri), Ok(steam));
        let totp = code(URIS[0]);
        assert_eq!(portable(&totp), totp.to_uri());
    }

    #[test]
    fn reads_ente_auth_export() {
        let bytes = include_bytes!("../testdata/ente_auth.txt");
        assert_eq!(crate::detect(bytes), Some(Format::Ente));
        let imported = crate::import(bytes, None).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn keeps_ente_tags_and_names_and_skips_its_trash() {
        let display = |json: &str| {
            percent_encoding::utf8_percent_encode(json, percent_encoding::NON_ALPHANUMERIC)
                .to_string()
        };
        // As Ente Auth writes them: the label unescaped, `#` and all.
        let text = format!(
            "otpauth://totp/My Bank:jo #2?algorithm=SHA1&digits=6&issuer=My+Bank&period=30\
             &secret=JBSWY3DPEHPK3PXP&codeDisplay={}\n\
             otpauth://totp/Old:bob?secret=JBSWY3DPEHPK3PXP&codeDisplay={}\n",
            display(r#"{"pinned":true,"tags":[" ","Bank","Home"],"note":"a, b"}"#),
            display(r#"{"trashed":true,"tags":[]}"#),
        );
        assert_eq!(crate::detect(text.as_bytes()), Some(Format::Ente));
        let imported = crate::import(text.as_bytes(), None).expect("import");
        assert_eq!(imported.items.len(), 1);
        let item = &imported.items[0];
        assert_eq!(
            (item.otp.issuer.as_str(), item.otp.account.as_str()),
            ("My Bank", "jo #2")
        );
        assert_eq!(item.group, "Bank");
        assert_eq!(
            imported.skipped,
            [("Old:bob".into(), "it is in Ente Auth's trash".into())]
        );
    }

    #[test]
    fn ente_encrypted_export_says_how_to_export() {
        let json = br#"{"version":1,"kdfParams":{"memLimit":1,"opsLimit":1,"salt":"AA=="},
            "encryptedData":"AA==","encryptionNonce":"AA=="}"#;
        assert_eq!(crate::detect(json), Some(Format::Ente));
        assert!(matches!(
            crate::import(json, None),
            Err(Error::Unsupported(_))
        ));
    }

    /// A Google Authenticator export of two codes, one with MD5, built as
    /// its protobuf (see `authwing_totp::Migration`).
    fn migration() -> String {
        let field = |number: u8, bytes: &[u8]| {
            let mut out = vec![number << 3 | 2, bytes.len() as u8];
            out.extend_from_slice(bytes);
            out
        };
        let entry = |secret: &[u8], name: &str, algorithm: u8| {
            let mut out = field(1, secret);
            out.extend(field(2, name.as_bytes()));
            out.extend(field(3, b"Example"));
            // algorithm, digits (six), type (TOTP)
            out.extend([4 << 3, algorithm, 5 << 3, 1, 6 << 3, 2]);
            out
        };
        let mut payload = field(1, &entry(b"Hello!\xde\xad\xbe\xef", "Example:alice", 1));
        payload.extend(field(1, &entry(b"0123456789", "Example:old", 4)));
        let data = crypto::BASE64.encode(payload);
        format!(
            "otpauth-migration://offline?data={}",
            data.replace('+', "%2B")
                .replace('/', "%2F")
                .replace('=', "%3D")
        )
    }

    #[test]
    fn reads_google_authenticator_export_and_reports_skipped() {
        let text = format!(
            "{}\n\n\
             otpauth://totp/Bad:md5?secret=JBSWY3DPEHPK3PXP&algorithm=MD5\n\
             otpauth://totp/Nothing:here?issuer=Nothing\n\
             otpauth://totp/?secret=JBSWY3DP&digits=12\n\
             hello\n\
             {}\n",
            migration(),
            URIS[0]
        );
        assert_eq!(crate::detect(text.as_bytes()), Some(Format::Uris));
        let imported = crate::import(text.as_bytes(), None).expect("import");
        assert_codes(
            &imported.items,
            &[
                "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example",
                URIS[0],
            ],
        );
        let skipped: Vec<(&str, &str)> = imported
            .skipped
            .iter()
            .map(|(what, why)| (what.as_str(), why.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [
                ("Example (old)", "unsupported algorithm: MD5"),
                ("Bad:md5", "unsupported algorithm: MD5"),
                ("Nothing:here", "the secret is missing"),
                (
                    "line 5",
                    "unsupported number of digits: 12 (5 to 10 are supported)"
                ),
                ("line 6", "not an otpauth:// link"),
            ]
        );
    }

    #[test]
    fn names_what_a_uri_is_for() {
        assert_eq!(
            what("otpauth://totp/ACME%20Co:jo?secret=X").as_deref(),
            Some("ACME Co:jo")
        );
        assert_eq!(what("OTPAUTH://TOTP/A#b").as_deref(), Some("A"));
        assert_eq!(what("otpauth://totp?secret=X"), None);
        assert_eq!(what("hello"), None);
    }
}
