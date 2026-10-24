//! The backup of Stratum, the app once named Authenticator Pro: JSON with
//! its authenticators (the codes), its categories and which code is in
//! which. Sealed, the file is `AUTHENTICATORPRO`, a 16-byte salt, a 12-byte
//! nonce, and the JSON sealed with AES-256-GCM under a key Argon2id makes
//! from the password (64 MiB, three passes, four lanes). Older versions
//! wrote `AuthenticatorPro`, a 20-byte salt, a 16-byte IV, and the JSON
//! sealed with AES-256-CBC under a key from PBKDF2-HMAC-SHA1 (64,000
//! rounds). Neither keeps anything to check the password against, so a
//! file that does not open is taken as the wrong password.
//!
//! Ported from Aegis's importer (https://github.com/beemdevelopment/Aegis,
//! `app/src/main/java/com/beemdevelopment/aegis/importers/StratumImporter.java`;
//! GPL-3.0), and checked against the app's own
//! (https://github.com/stratumauth/app, `Stratum.Core/src/Backup/`). A code's
//! first category is its group. mOTP and Yandex codes are skipped:
//! authwing does not make them.

use argon2::{Algorithm, Argon2, Params, Version};
use authwing_totp::fields::{ACCOUNT, ALGORITHM, COUNTER, DIGITS, ISSUER, KIND, PERIOD, SECRET};
use serde::Deserialize;
use serde_json::Value;
use sha1::Sha1;
use zeroize::Zeroizing;

use crate::crypto;
use crate::{Error, Format, Imported, Item, Result, damaged, label, otp_from};

/// What a sealed file starts with, and what an older one did.
const MAGIC: &[u8] = b"AUTHENTICATORPRO";
const LEGACY_MAGIC: &[u8] = b"AuthenticatorPro";
/// Argon2id's costs: 64 MiB, three passes, four lanes.
const ARGON2_M_KIB: u32 = 64 * 1024;
const ARGON2_T: u32 = 3;
const ARGON2_P: u32 = 4;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
/// PBKDF2's rounds and salt, and the IV, of an older file.
const LEGACY_ROUNDS: u32 = 64_000;
const LEGACY_SALT_LEN: usize = 20;
const LEGACY_IV_LEN: usize = 16;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Backup {
    authenticators: Vec<Value>,
    categories: Option<Vec<Category>>,
    authenticator_categories: Option<Vec<Binding>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Category {
    id: String,
    name: String,
}

/// A code in a category, the code named by its secret.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Binding {
    category_id: String,
    authenticator_secret: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Authenticator {
    #[serde(rename = "Type")]
    kind: u8,
    issuer: Option<String>,
    username: Option<String>,
    #[serde(default)]
    secret: String,
    algorithm: Option<u8>,
    digits: Option<u32>,
    period: Option<u64>,
    counter: Option<u64>,
}

/// Whether `bytes` are a sealed backup, by how it starts.
pub(crate) fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC) || bytes.starts_with(LEGACY_MAGIC)
}

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    if !is_sealed(bytes) {
        return read(bytes);
    }
    let password = password.ok_or(Error::PasswordNeeded(Format::Stratum))?;
    let plain = if let Some(rest) = bytes.strip_prefix(MAGIC) {
        decrypt(rest, password)?
    } else {
        decrypt_legacy(&bytes[LEGACY_MAGIC.len()..], password)?
    };
    read(&plain)
}

/// The JSON a sealed file holds after its magic.
fn decrypt(rest: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let cut_short = || damaged("the file is cut short");
    let (salt, rest) = rest.split_first_chunk::<SALT_LEN>().ok_or_else(cut_short)?;
    let (nonce, sealed) = rest
        .split_first_chunk::<NONCE_LEN>()
        .ok_or_else(cut_short)?;
    let params = Params::new(ARGON2_M_KIB, ARGON2_T, ARGON2_P, Some(32)).map_err(damaged)?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut key[..])
        .map_err(damaged)?;
    crypto::aes_gcm_open(&key[..], nonce, sealed).ok_or(Error::WrongPassword)
}

/// The JSON an older sealed file holds after its magic. A wrong key may
/// leave padding that looks whole: the JSON it gives does not read then,
/// which is also taken as the wrong password.
fn decrypt_legacy(rest: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let cut_short = || damaged("the file is cut short");
    let (salt, rest) = rest
        .split_first_chunk::<LEGACY_SALT_LEN>()
        .ok_or_else(cut_short)?;
    let (iv, sealed) = rest
        .split_first_chunk::<LEGACY_IV_LEN>()
        .ok_or_else(cut_short)?;
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<Sha1>(password.as_bytes(), salt, LEGACY_ROUNDS, &mut key[..]);
    crypto::aes_cbc_open(&key[..], iv, sealed)
        .filter(|plain| serde_json::from_slice::<Backup>(plain).is_ok())
        .ok_or(Error::WrongPassword)
}

/// The codes in the backup's JSON.
fn read(bytes: &[u8]) -> Result<Imported> {
    let backup: Backup = serde_json::from_slice(bytes).map_err(damaged)?;
    let categories = backup.categories.unwrap_or_default();
    let bindings = backup.authenticator_categories.unwrap_or_default();
    let group = |secret: &str| {
        let binding = bindings.iter().find(|b| b.authenticator_secret == secret)?;
        let category = categories.iter().find(|c| c.id == binding.category_id)?;
        Some(category.name.clone())
    };

    let mut imported = Imported::default();
    for (n, value) in backup.authenticators.into_iter().enumerate() {
        let text = |key| value.get(key).and_then(Value::as_str).unwrap_or_default();
        let what = label(text("Issuer"), text("Username"), || {
            format!("authenticator {}", n + 1)
        });
        let entry: Authenticator = match serde_json::from_value(value) {
            Ok(entry) => entry,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        let otp = otp_from(&[
            (ISSUER, entry.issuer),
            (ACCOUNT, entry.username),
            (SECRET, Some(entry.secret.clone())),
            (KIND, Some(kind(entry.kind))),
            (ALGORITHM, entry.algorithm.map(algorithm)),
            (DIGITS, entry.digits.map(|d| d.to_string())),
            (PERIOD, entry.period.map(|p| p.to_string())),
            (COUNTER, entry.counter.map(|c| c.to_string())),
        ]);
        match otp {
            Ok(otp) => imported.items.push(Item {
                otp,
                group: group(&entry.secret).unwrap_or_default(),
                sites: Vec::new(),
            }),
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    Ok(imported)
}

/// The kind of code Stratum numbers `number` (its `AuthenticatorType`), as
/// a vault record names it; mOTP and Yandex's are named only to be refused.
fn kind(number: u8) -> String {
    match number {
        1 => "hotp".into(),
        2 => "totp".into(),
        3 => "motp".into(),
        4 => "steam".into(),
        5 => "yandex".into(),
        other => other.to_string(),
    }
}

/// The hash Stratum numbers `number`, as a vault record names it.
fn algorithm(number: u8) -> String {
    match number {
        0 => "SHA1".into(),
        1 => "SHA256".into(),
        2 => "SHA512".into(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use serde_json::json;

    const PLAIN: &[u8] = include_bytes!("../testdata/stratum_plain.json");
    const ENCRYPTED: &[u8] = include_bytes!("../testdata/stratum_encrypted.bin");
    const ENCRYPTED_LEGACY: &[u8] = include_bytes!("../testdata/stratum_encrypted_legacy.bin");

    #[test]
    fn reads_a_plain_backup() {
        let imported = crate::import(PLAIN, None).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn reads_sealed_backups() {
        for file in [ENCRYPTED, ENCRYPTED_LEGACY] {
            assert!(matches!(
                crate::import(file, None),
                Err(Error::PasswordNeeded(Format::Stratum))
            ));
            let imported = crate::import(file, Some("test")).expect("import");
            assert_codes(&imported.items, &URIS);
            assert!(imported.skipped.is_empty());
        }
    }

    #[test]
    fn wrong_password_is_told_from_a_file_cut_short() {
        for file in [ENCRYPTED, ENCRYPTED_LEGACY] {
            assert!(matches!(
                crate::import(file, Some("tset")),
                Err(Error::WrongPassword)
            ));
            assert!(matches!(
                crate::import(&file[..30], Some("test")),
                Err(Error::Damaged(_))
            ));
        }
    }

    #[test]
    fn maps_categories_and_skips_what_it_cannot_use() {
        let code = |kind: u8, issuer: &str, secret: &str, algorithm: u8| {
            json!({
                "Type": kind, "Icon": null, "Issuer": issuer, "Username": "alice",
                "Secret": secret, "Pin": null, "Algorithm": algorithm, "Digits": 6,
                "Period": 30, "Counter": 0, "Ranking": 0,
            })
        };
        let file = json!({
            "Authenticators": [
                code(2, "GitHub", "JBSWY3DPEHPK3PXP", 0),
                code(1, "GitLab", "KRUGS4ZANFZSAYJA", 1),
                {"Type": 2, "Issuer": "Bare", "Username": null, "Secret": "GEZDGNBVGY3TQOJQ"},
                code(3, "Mobile", "1234567890abcdef", 0),
                code(5, "Yandex", "JBSWY3DPEHPK3PXP", 0),
                code(2, "Hashed", "JBSWY3DPEHPK3PXP", 7),
                {"Issuer": "Typeless"},
            ],
            "Categories": [
                {"Id": "a1", "Name": "Work", "Ranking": 0},
                {"Id": "b2", "Name": "Home", "Ranking": 1},
            ],
            "AuthenticatorCategories": [
                {"CategoryId": "b2", "AuthenticatorSecret": "JBSWY3DPEHPK3PXP", "Ranking": 0},
                {"CategoryId": "a1", "AuthenticatorSecret": "JBSWY3DPEHPK3PXP", "Ranking": 1},
                {"CategoryId": "gone", "AuthenticatorSecret": "KRUGS4ZANFZSAYJA", "Ranking": 0},
            ],
            "CustomIcons": [],
        });
        let bytes = serde_json::to_vec(&file).expect("json");
        assert_eq!(crate::detect(&bytes), Some(Format::Stratum));
        let imported = crate::import(&bytes, None).expect("import");
        let items: Vec<(&str, &str, &str)> = imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                (
                    otp.issuer.as_str(),
                    otp.account.as_str(),
                    item.group.as_str(),
                )
            })
            .collect();
        assert_eq!(
            items,
            [
                ("GitHub", "alice", "Home"),
                ("GitLab", "alice", ""),
                ("Bare", "", ""),
            ]
        );
        let skipped: Vec<(&str, &str)> = imported
            .skipped
            .iter()
            .map(|(what, why)| (what.as_str(), why.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [
                ("Mobile (alice)", "unknown kind of code: motp"),
                ("Yandex (alice)", "unknown kind of code: yandex"),
                ("Hashed (alice)", "unsupported algorithm: 7"),
                ("Typeless", "missing field `Type`"),
            ]
        );
    }
}
