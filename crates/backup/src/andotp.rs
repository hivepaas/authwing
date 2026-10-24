//! andOTP's backup: a JSON list of its entries, in the clear or sealed with
//! AES-256-GCM. A sealed backup starts with how its key is made, the rounds
//! of PBKDF2-HMAC-SHA1 (four bytes, big-endian) and a 12-byte salt, then
//! has the nonce, the ciphertext and the tag. Older andOTP sealed it under
//! the SHA-256 of the password, and wrote only the nonce, the ciphertext
//! and the tag. Neither says what it is, so a sealed backup is detected as
//! [`Format::EncryptedFile`](crate::Format::EncryptedFile) and known by the
//! password that opens it: a tag that matches says the key is right, so
//! what does not read then is damage.
//!
//! Ported from Aegis's importer (https://github.com/beemdevelopment/Aegis,
//! `app/src/main/java/com/beemdevelopment/aegis/importers/AndOtpImporter.java`;
//! GPL-3.0). An entry's `label` is its account when it has an `issuer`;
//! older andOTP wrote `Issuer - label` instead, whose label may itself be
//! an otpauth URI's `Issuer:account`, so an issuer before a colon is taken
//! off it too. The first of its tags is its group. mOTP codes are skipped:
//! authwing does not make them.

use authwing_totp::fields::{ACCOUNT, ALGORITHM, COUNTER, DIGITS, ISSUER, KIND, PERIOD, SECRET};
use serde::Deserialize;
use serde_json::Value;
use sha1::Sha1;
use zeroize::Zeroizing;

use crate::crypto;
use crate::{Imported, Item, Result, damaged, label, otp_from};

/// The most rounds of PBKDF2 a backup may ask for: andOTP's own take about
/// 150,000. Bytes that ask for more are no backup of andOTP's, as Aegis
/// takes them, and should not keep the machine busy to find that out.
const MAX_ROUNDS: u32 = 10_000_000;
const SALT_LEN: usize = 12;
const NONCE_LEN: usize = 12;

#[derive(Deserialize)]
struct Entry {
    #[serde(default)]
    secret: String,
    issuer: Option<String>,
    #[serde(default)]
    label: String,
    /// `TOTP`, `HOTP`, `STEAM` or `MOTP`.
    #[serde(rename = "type")]
    kind: String,
    algorithm: Option<String>,
    digits: Option<u32>,
    period: Option<u64>,
    counter: Option<u64>,
    #[serde(default)]
    tags: Vec<String>,
}

/// Whether `json` looks like andOTP's backup: a list of entries, each with
/// a secret and a type.
pub(crate) fn is_backup(json: &Value) -> bool {
    let is_entry = |entry: &Value| entry.get("secret").is_some() && entry.get("type").is_some();
    json.as_array()
        .is_some_and(|entries| entries.first().is_some_and(is_entry))
}

/// The codes in a backup in the clear.
pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let entries: Vec<Value> = serde_json::from_slice(bytes).map_err(damaged)?;

    let mut imported = Imported::default();
    for (n, value) in entries.into_iter().enumerate() {
        let text = |key| value.get(key).and_then(Value::as_str).unwrap_or_default();
        let what = label(text("issuer"), text("label"), || format!("entry {}", n + 1));
        let entry: Entry = match serde_json::from_value(value) {
            Ok(entry) => entry,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        let (issuer, account) = names(entry.issuer, entry.label);
        let what = label(&issuer, &account, || what);
        let otp = otp_from(&[
            (ISSUER, Some(issuer)),
            (ACCOUNT, Some(account)),
            (SECRET, Some(entry.secret)),
            (KIND, Some(entry.kind.to_ascii_lowercase())),
            (ALGORITHM, entry.algorithm),
            (DIGITS, entry.digits.map(|d| d.to_string())),
            (PERIOD, entry.period.map(|p| p.to_string())),
            (COUNTER, entry.counter.map(|c| c.to_string())),
        ]);
        match otp {
            Ok(otp) => imported.items.push(Item {
                otp,
                group: entry.tags.into_iter().next().unwrap_or_default(),
                sites: Vec::new(),
            }),
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    Ok(imported)
}

/// The issuer and the account an entry's `issuer` and `label` give (see the
/// module docs).
fn names(issuer: Option<String>, label: String) -> (String, String) {
    let (issuer, account) = match issuer {
        Some(issuer) => (issuer, label),
        None => match label.split_once(" - ") {
            Some((issuer, account)) => (issuer.to_string(), account.to_string()),
            None => (String::new(), label),
        },
    };
    let rest = account
        .strip_prefix(issuer.as_str())
        .and_then(|rest| rest.strip_prefix(':'));
    let account = match rest {
        Some(rest) if !issuer.is_empty() => rest.to_string(),
        _ => account,
    };
    (issuer, account)
}

/// The codes in a sealed backup, if `password` opens it as one andOTP
/// writes now; `None` if it does not, or `bytes` are no such backup.
pub(crate) fn open(bytes: &[u8], password: &str) -> Option<Result<Imported>> {
    let (rounds, rest) = bytes.split_first_chunk::<4>()?;
    let rounds = u32::from_be_bytes(*rounds);
    if !(1..=MAX_ROUNDS).contains(&rounds) {
        return None;
    }
    let (salt, sealed) = rest.split_first_chunk::<SALT_LEN>()?;
    let mut key = Zeroizing::new([0u8; 32]);
    pbkdf2::pbkdf2_hmac::<Sha1>(password.as_bytes(), salt, rounds, &mut key[..]);
    open_with(&key[..], sealed)
}

/// The codes in a sealed backup, if `password` opens it as one older
/// andOTP wrote; `None` if it does not.
pub(crate) fn open_old(bytes: &[u8], password: &str) -> Option<Result<Imported>> {
    open_with(&crypto::sha256_key(password)[..], bytes)
}

/// The codes `sealed` holds after its nonce, if `key` opens it.
fn open_with(key: &[u8], sealed: &[u8]) -> Option<Result<Imported>> {
    let (nonce, sealed) = sealed.split_first_chunk::<NONCE_LEN>()?;
    let plain = crypto::aes_gcm_open(key, nonce, sealed)?;
    Some(import(&plain))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use crate::{Error, Format};
    use aes_gcm::Aes256Gcm;
    use aes_gcm::aead::{Aead, KeyInit};
    use serde_json::json;

    const PLAIN: &[u8] = include_bytes!("../testdata/andotp_plain.json");
    const ENCRYPTED: &[u8] = include_bytes!("../testdata/andotp_encrypted.bin");
    /// Sealed under the password's SHA-256, as older andOTP sealed it.
    const ENCRYPTED_OLD: &[u8] = include_bytes!("../testdata/andotp_encrypted_old.bin");

    /// `plain` sealed as older andOTP sealed its backups.
    fn seal_old(plain: &[u8], password: &str) -> Vec<u8> {
        let nonce = [3u8; NONCE_LEN];
        let sealed = Aes256Gcm::new_from_slice(&crypto::sha256_key(password)[..])
            .expect("key")
            .encrypt((&nonce).into(), plain)
            .expect("seal");
        [&nonce[..], &sealed].concat()
    }

    #[test]
    fn reads_a_plain_backup() {
        assert_eq!(crate::detect(PLAIN), Some(Format::AndOtp));
        let imported = crate::import(PLAIN, None).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn reads_sealed_backups_by_their_password() {
        for file in [ENCRYPTED, ENCRYPTED_OLD] {
            assert_eq!(crate::detect(file), Some(Format::EncryptedFile));
            assert!(matches!(
                crate::import(file, None),
                Err(Error::PasswordNeeded(Format::EncryptedFile))
            ));
            assert!(matches!(
                crate::import(file, Some("tset")),
                Err(Error::WrongPassword)
            ));
        }
        let imported = crate::import(ENCRYPTED, Some("test")).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
        // The older one has no Steam code, and labels such as
        // `Deno - Deno:Mason`, which Aegis reads as the account `Deno:Mason`.
        let imported = crate::import(ENCRYPTED_OLD, Some("test")).expect("old");
        assert_codes(&imported.items, &URIS[..6]);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn what_decrypts_but_does_not_read_is_damage() {
        let file = seal_old(b"{\"not\": \"a list\"}", "pass");
        assert!(matches!(
            crate::import(&file, Some("pass")),
            Err(Error::Damaged(_))
        ));
        let file = seal_old(b"[]", "pass");
        assert_eq!(
            crate::import(&file, Some("pass")).ok(),
            Some(Imported::default())
        );
    }

    #[test]
    fn maps_labels_and_tags_and_skips_what_it_cannot_use() {
        let secret = "JBSWY3DPEHPK3PXP";
        let totp = |extra: Value| {
            let mut entry = json!({"secret": secret, "type": "TOTP", "algorithm": "SHA1",
                "digits": 6, "period": 30, "tags": []});
            if let (Some(entry), Some(extra)) = (entry.as_object_mut(), extra.as_object()) {
                entry.extend(extra.clone());
            }
            entry
        };
        let file = json!([
            totp(json!({"issuer": "GitHub", "label": "alice", "tags": ["Work", "Home"]})),
            totp(json!({"label": "GitLab - GitLab:bob"})),
            totp(json!({"label": "carol"})),
            totp(json!({"issuer": "", "label": "Dave - dave"})),
            totp(json!({"issuer": "Mobile", "label": "erin", "type": "MOTP"})),
            totp(json!({"issuer": "Hashed", "label": "frank", "algorithm": "MD5"})),
            {"secret": secret, "label": "untyped"},
        ]);
        let imported =
            crate::import(&serde_json::to_vec(&file).expect("json"), None).expect("import");
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
                ("GitHub", "alice", "Work"),
                ("GitLab", "bob", ""),
                ("", "carol", ""),
                ("", "Dave - dave", ""),
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
                ("Mobile (erin)", "unknown kind of code: motp"),
                ("Hashed (frank)", "unsupported algorithm: MD5"),
                ("untyped", "missing field `type`"),
            ]
        );
    }
}
