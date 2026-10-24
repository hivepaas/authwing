//! Proton Authenticator's export: JSON with its codes (`entries`), each an
//! `otpauth://` URI, or `steam://` and the secret for Steam Guard, beside
//! the name Proton Authenticator shows and a note. Protected by a password,
//! the same JSON is sealed with AES-256-GCM, with
//! `proton.authenticator.export.v1` as associated data, under a key
//! Argon2id makes from the password and `salt` (19 MiB, two passes, one
//! lane); `content` is the nonce, then the ciphertext and tag, in base64.
//! Nothing else is sealed with the key, so a file that does not open is
//! taken as the wrong password: nothing tells it from a damaged one.
//!
//! The URI names the code; the entry's name is its account when the URI
//! has none. A Steam code's URI has nothing but the secret, in base32, or
//! in base64 when that is how it was given: the code is named `Steam` and
//! the entry's name. Proton Authenticator has no groups.
//!
//! As Proton Authenticator writes it
//! (https://github.com/protonpass/proton-pass-common,
//! `proton-authenticator/src/entry/exporter.rs`, `password_exporter.rs` and
//! `steam/mod.rs`) and Aegis reads it (https://github.com/beemdevelopment/Aegis,
//! `app/src/main/java/com/beemdevelopment/aegis/importers/ProtonAuthenticatorImporter.java`).

use argon2::{Algorithm, Argon2, Params, Version};
use authwing_totp::{Kind, Otp, Secret};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use zeroize::Zeroizing;

use crate::crypto::{self, BASE64};
use crate::uris::{self, has_prefix};
use crate::{Error, Format, Imported, Item, Result, damaged};

/// The export's version this reads.
const VERSION: u32 = 1;
/// What the sealed export vouches for besides the codes.
const AAD: &[u8] = b"proton.authenticator.export.v1";
/// Argon2id's costs: 19 MiB, two passes, one lane.
const ARGON2_M_KIB: u32 = 19 * 1024;
const ARGON2_T: u32 = 2;
const ARGON2_P: u32 = 1;
const KEY_LEN: usize = 32;
/// AES-GCM's nonce, before the ciphertext.
const NONCE_LEN: usize = 12;
/// The issuer of Steam codes, which their URIs do not name.
const STEAM: &str = "Steam";

#[derive(Deserialize)]
struct File {
    version: u32,
    entries: Option<Vec<Value>>,
    salt: Option<String>,
    content: Option<String>,
}

#[derive(Deserialize)]
struct Entry {
    content: Content,
}

#[derive(Deserialize)]
struct Content {
    uri: String,
    name: Option<String>,
}

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let mut file: File = serde_json::from_slice(bytes).map_err(damaged)?;
    if let (Some(salt), Some(content)) = (&file.salt, &file.content) {
        check_version(file.version)?;
        let password = password.ok_or(Error::PasswordNeeded(Format::Proton))?;
        let plain = decrypt(salt, content, password)?;
        file = serde_json::from_slice(&plain).map_err(damaged)?;
    }
    check_version(file.version)?;
    let entries = file.entries.ok_or_else(|| damaged("it holds no codes"))?;

    let mut imported = Imported::default();
    for (n, value) in entries.into_iter().enumerate() {
        let content = value.get("content");
        let text = |key| content.and_then(|c| c.get(key)).and_then(Value::as_str);
        let what = text("uri")
            .and_then(uris::what)
            .or_else(|| {
                text("name")
                    .map(str::trim)
                    .filter(|n| !n.is_empty())
                    .map(String::from)
            })
            .unwrap_or_else(|| format!("entry {}", n + 1));
        let entry: Entry = match serde_json::from_value(value) {
            Ok(entry) => entry,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        match code(entry.content) {
            Ok(otp) => imported.items.push(Item::new(otp)),
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    Ok(imported)
}

/// Refuses an export of a version newer than this reads.
fn check_version(version: u32) -> Result<()> {
    if version > VERSION {
        return Err(Error::Unsupported(
            "this Proton Authenticator export is of a newer version than authwing reads".into(),
        ));
    }
    Ok(())
}

/// The code an entry's content gives (see the module docs).
fn code(content: Content) -> authwing_totp::Result<Otp> {
    let name = content.name.unwrap_or_default().trim().to_string();
    let uri = content.uri.trim();
    let steam = "steam://";
    if has_prefix(uri, steam) {
        return Ok(Otp::new(
            STEAM,
            name,
            steam_secret(&uri[steam.len()..])?,
            Kind::Steam,
        ));
    }
    let mut otp = Otp::from_uri(uri)?;
    if otp.account.trim().is_empty() && name != otp.issuer {
        otp.account = name;
    }
    Ok(otp)
}

/// The secret of a `steam://` URI: base32, or else base64.
fn steam_secret(text: &str) -> authwing_totp::Result<Secret> {
    Secret::from_base32(text).or_else(|e| match BASE64.decode(text.trim()) {
        Ok(bytes) if !bytes.is_empty() => Ok(Secret::new(bytes)),
        _ => Err(e),
    })
}

/// The export `content` seals, under the key `password` and `salt` make.
fn decrypt(salt: &str, content: &str, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let salt = BASE64.decode(salt.trim()).map_err(damaged)?;
    let content = BASE64.decode(content.trim()).map_err(damaged)?;
    if content.len() < NONCE_LEN {
        return Err(damaged("the encrypted codes are cut short"));
    }
    let params = Params::new(ARGON2_M_KIB, ARGON2_T, ARGON2_P, Some(KEY_LEN)).map_err(damaged)?;
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), &salt, &mut key[..])
        .map_err(damaged)?;
    let (nonce, sealed) = content.split_at(NONCE_LEN);
    crypto::aes_gcm_open_with(&key[..], nonce, AAD, sealed).ok_or(Error::WrongPassword)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use serde_json::json;

    const PLAIN: &[u8] = include_bytes!("../testdata/proton_authenticator.json");
    const ENCRYPTED: &[u8] = include_bytes!("../testdata/proton_authenticator_encrypted.json");

    #[test]
    fn reads_a_plain_export() {
        let imported = crate::import(PLAIN, None).expect("import");
        assert_codes(&imported.items, &URIS[..3]);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn reads_an_encrypted_export() {
        assert!(matches!(
            crate::import(ENCRYPTED, None),
            Err(Error::PasswordNeeded(Format::Proton))
        ));
        let imported = crate::import(ENCRYPTED, Some("test")).expect("import");
        // Its Steam code has lost its name, as Aegis's test notes.
        assert_codes(
            &imported.items,
            &[
                URIS[0],
                URIS[1],
                URIS[2],
                "otpauth://steam/Steam:?secret=JRZCL47CMXVOQMNPZR2F7J4RGI&issuer=Steam",
            ],
        );
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn a_file_that_does_not_open_is_the_wrong_password() {
        assert!(matches!(
            crate::import(ENCRYPTED, Some("tset")),
            Err(Error::WrongPassword)
        ));
        let mut file: Value = serde_json::from_slice(ENCRYPTED).expect("json");
        let mut content = BASE64
            .decode(file["content"].as_str().expect("content"))
            .expect("base64");
        content[NONCE_LEN + 3] ^= 1;
        file["content"] = BASE64.encode(content).into();
        let damaged_file = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&damaged_file, Some("test")),
            Err(Error::WrongPassword)
        ));
        file["version"] = 2.into();
        let newer = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&newer, Some("test")),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn names_codes_and_skips_what_it_cannot_use() {
        let entry = |uri: &str, name: Option<&str>| {
            json!({
                "id": "0", "content": {"uri": uri, "entry_type": "Totp", "name": name},
                "note": null,
            })
        };
        let file = json!({
            "version": 1,
            "entries": [
                entry("otpauth://totp/?secret=JBSWY3DPEHPK3PXP&issuer=Example", Some("alice")),
                entry("otpauth://totp/?secret=JBSWY3DPEHPK3PXP&issuer=ACME", Some("ACME")),
                entry("steam://THIl8+Jl6ugxr8x0X6eRMg", Some("sophia")),
                entry("steam://JRZCL47CMXVOQMNPZR2F7J4RGI", None),
                entry("otpauth://totp/Old:bob?secret=JBSWY3DPEHPK3PXP&algorithm=MD5", None),
                entry("steam://not+a+secret!", Some("mia")),
                {"id": "7", "note": "no content"},
            ],
        });
        let imported =
            crate::import(&serde_json::to_vec(&file).expect("json"), None).expect("import");
        let items: Vec<(&str, &str, Kind)> = imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                (otp.issuer.as_str(), otp.account.as_str(), otp.kind)
            })
            .collect();
        assert_eq!(
            items,
            [
                ("Example", "alice", Kind::Totp { period: 30 }),
                ("ACME", "", Kind::Totp { period: 30 }),
                ("Steam", "sophia", Kind::Steam),
                ("Steam", "", Kind::Steam),
            ]
        );
        // Base64 or base32, the same key.
        assert_eq!(imported.items[2].otp.secret, imported.items[3].otp.secret);
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(skipped, ["Old:bob", "mia", "entry 7"]);
    }
}
