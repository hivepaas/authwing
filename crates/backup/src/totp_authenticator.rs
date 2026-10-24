//! The backup of TOTP Authenticator (`com.authenticator.authservice2`):
//! JSON sealed with AES-256-CBC under the SHA-256 of the password and an IV of zeros, then
//! written out in base64. A backup made without a password of the user's
//! is sealed with the app's own, `TotpAuthenticator`, which is tried before
//! asking. Nothing says what the file is, so it is detected as
//! [`Format::EncryptedFile`](crate::Format::EncryptedFile) and known by the
//! password that opens it; a wrong key sometimes leaves padding that looks
//! whole, so it is the JSON that reads that tells.
//!
//! The JSON is an object whose key, not its value, is the list of codes,
//! as JSON again. Each code's `key` is written in base 16, 32 or 64, as its
//! `base` says; its `digits` and `period` are text. The app makes TOTP codes
//! with SHA-1 only.
//!
//! Ported from Aegis's importer (https://github.com/beemdevelopment/Aegis,
//! `app/src/main/java/com/beemdevelopment/aegis/importers/TotpAuthenticatorImporter.java`;
//! GPL-3.0), and checked against Stratum's (https://github.com/stratumauth/app,
//! `Stratum.Core/src/Converter/TotpAuthenticatorBackupConverter.cs`), which
//! also reads the digits and the period where Aegis takes the defaults, and
//! knows the issuer `Unknown` for none.

use authwing_totp::Secret;
use authwing_totp::fields::{ACCOUNT, DIGITS, ISSUER, PERIOD, SECRET};
use base64::Engine;
use serde_json::{Map, Value};

use crate::crypto::{self, BASE64};
use crate::{Imported, Item, Result, label, otp_from};

/// The password the app seals a backup with when the user gives none.
pub(crate) const APP_PASSWORD: &str = "TotpAuthenticator";
/// AES's block: the sealed bytes are whole blocks.
const BLOCK: usize = 16;
const IV: [u8; BLOCK] = [0; BLOCK];

/// The sealed bytes `bytes` spell, if they are base64 text of whole AES
/// blocks, as the app writes a backup.
pub(crate) fn sealed(bytes: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(bytes).ok()?;
    let text: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let sealed = BASE64.decode(text).ok()?;
    (!sealed.is_empty() && sealed.len().is_multiple_of(BLOCK)).then_some(sealed)
}

/// The codes in a backup, if `password` opens it; `None` if it does not, or
/// `bytes` are no such backup.
pub(crate) fn open(bytes: &[u8], password: &str) -> Option<Result<Imported>> {
    let sealed = sealed(bytes)?;
    let plain = crypto::aes_cbc_open(&crypto::sha256_key(password)[..], &IV, &sealed)?;
    let object: Map<String, Value> = serde_json::from_slice(&plain).ok()?;
    let entries = object
        .keys()
        .find_map(|key| serde_json::from_str::<Vec<Value>>(key).ok())?;
    Some(Ok(read(entries)))
}

/// The codes `entries` describe.
fn read(entries: Vec<Value>) -> Imported {
    let mut imported = Imported::default();
    for (n, entry) in entries.into_iter().enumerate() {
        let text = |key| entry.get(key).and_then(Value::as_str).unwrap_or_default();
        let issuer = match text("issuer") {
            "Unknown" => "",
            issuer => issuer,
        };
        let what = label(issuer, text("name"), || format!("entry {}", n + 1));
        let secret = match secret(&entry) {
            Ok(secret) => secret,
            Err(why) => {
                imported.skipped.push((what, why));
                continue;
            }
        };
        let otp = otp_from(&[
            (ISSUER, Some(issuer.to_string())),
            (ACCOUNT, Some(text("name").to_string())),
            (SECRET, Some(secret)),
            (DIGITS, setting(&entry, "digits")),
            (PERIOD, setting(&entry, "period")),
        ]);
        match otp {
            Ok(otp) => imported.items.push(Item::new(otp)),
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    imported
}

/// An entry's key in base32, from the base it is written in.
fn secret(entry: &Value) -> std::result::Result<String, String> {
    let key = entry.get("key").and_then(Value::as_str).unwrap_or_default();
    let bytes = match entry.get("base").and_then(number) {
        Some(32) => return Ok(key.to_string()),
        Some(16) => crypto::hex(key).ok_or("the secret is not hexadecimal")?,
        Some(64) => BASE64
            .decode(key.trim())
            .map_err(|_| "the secret is not base64")?,
        Some(base) => return Err(format!("the secret is written in base {base}")),
        None => return Err("the secret does not say how it is written".into()),
    };
    Ok(Secret::new(bytes).to_base32())
}

/// A setting written as text or as a number; `None` when it is empty.
fn setting(entry: &Value, key: &str) -> Option<String> {
    let value = match entry.get(key)? {
        Value::String(text) => text.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!value.is_empty()).then_some(value)
}

/// A number written as one, or as text.
fn number(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str()?.trim().parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::URIS;
    use crate::{Error, Format};
    use serde_json::json;

    const BACKUP: &[u8] = include_bytes!("../testdata/totp_authenticator.bin");
    /// The sample backup's password.
    const PASSWORD: &str = "Testtest1";

    /// `entries` written and sealed as the app writes a backup.
    fn backup(entries: Value, password: &str) -> Vec<u8> {
        let mut object = Map::new();
        object.insert(entries.to_string(), "1608293023".into());
        let plain = serde_json::to_vec(&object).expect("json");
        let sealed = crypto::aes_cbc_seal(&crypto::sha256_key(password)[..], &IV, &plain);
        BASE64.encode(sealed).into_bytes()
    }

    #[test]
    fn reads_a_backup_with_its_password() {
        assert_eq!(crate::detect(BACKUP), Some(Format::EncryptedFile));
        assert!(matches!(
            crate::import(BACKUP, None),
            Err(Error::PasswordNeeded(Format::EncryptedFile))
        ));
        assert!(matches!(
            crate::import(BACKUP, Some("Testtest2")),
            Err(Error::WrongPassword)
        ));
        let imported = crate::import(BACKUP, Some(PASSWORD)).expect("import");
        // Three of Aegis's codes, with the names in lower case, and SHA-1
        // for SHA-256 and SHA-512, as the app keeps them.
        let uris: Vec<String> = imported
            .items
            .iter()
            .map(|item| item.otp.to_uri())
            .collect();
        let want = [
            URIS[0].replace("Mason", "mason"),
            "otpauth://totp/SPDX:james?secret=5OM4WOOGPLQEF6UGN3CPEOOLWU&issuer=SPDX\
             &digits=7&period=20"
                .into(),
            "otpauth://totp/Airbnb:elijah?secret=7ELGJSGXNCCTV3O6LKJWYFV2RA&issuer=Airbnb\
             &digits=8&period=50"
                .into(),
        ];
        let want: Vec<String> = want
            .iter()
            .map(|uri| crate::testing::code(uri).to_uri())
            .collect();
        assert_eq!(uris, want);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn opens_a_backup_made_without_a_password() {
        let file = backup(
            json!([
                {"issuer": "Hex", "name": "a", "key": "48656c6c6f21deadbeef", "base": 16,
                 "digits": "6", "period": "30"},
                {"issuer": "Base32", "name": "b", "key": "JBSWY3DPEHPK3PXP", "base": 32,
                 "digits": "", "period": ""},
                {"issuer": "Base64", "name": "c", "key": "SGVsbG8h3q2+7w==", "base": "64"},
                {"issuer": "Unknown", "name": "d", "key": "JBSWY3DPEHPK3PXP", "base": 32},
                {"issuer": "Base10", "name": "e", "key": "1234", "base": 10},
                {"issuer": "Odd", "name": "f", "key": "JBSWY3DPEHPK3PXP", "base": 32,
                 "digits": "4"},
            ]),
            APP_PASSWORD,
        );
        assert_eq!(crate::detect(&file), Some(Format::EncryptedFile));
        let imported = crate::import(&file, None).expect("import");
        let items: Vec<(&str, &str, String)> = imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                (
                    otp.issuer.as_str(),
                    otp.account.as_str(),
                    otp.secret.to_base32(),
                )
            })
            .collect();
        assert_eq!(
            items,
            [
                ("Hex", "a", "JBSWY3DPEHPK3PXP".to_string()),
                ("Base32", "b", "JBSWY3DPEHPK3PXP".to_string()),
                ("Base64", "c", "JBSWY3DPEHPK3PXP".to_string()),
                ("", "d", "JBSWY3DPEHPK3PXP".to_string()),
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
                ("Base10 (e)", "the secret is written in base 10"),
                (
                    "Odd (f)",
                    "unsupported number of digits: 4 (5 to 10 are supported)"
                ),
            ]
        );
        // With a password given, it is that password the file must open with.
        assert!(matches!(
            crate::import(&file, Some("test")),
            Err(Error::WrongPassword)
        ));
    }
}
