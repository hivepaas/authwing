//! authwing's own backup: every code with its group and sites, sealed with a
//! passphrase.
//!
//! The file is JSON that says what it is and how it was sealed:
//!
//! ```json
//! {
//!   "format": "authwing-backup",
//!   "version": 1,
//!   "kdf": { "alg": "argon2id", "m_kib": 65536, "t": 3, "p": 1, "salt": "…" },
//!   "key_id": "…",
//!   "cipher": 1,
//!   "nonce": "…",
//!   "sealed": "…"
//! }
//! ```
//!
//! The key comes from the passphrase as the vault's does (Argon2id, 64 MiB,
//! three passes), and the parameters are kept with the salt, so a later
//! version can raise them and still open the files made before. `cipher` is
//! the vault's id for XChaCha20-Poly1305, whose 192-bit nonce is safe to
//! pick at random, and `sealed` its ciphertext and tag. `key_id` is a hash
//! of the key: a passphrase whose key does not match it is the wrong one,
//! and a file that matches but does not decrypt is damaged. It gives a
//! guesser nothing the ciphertext does not: each guess still costs Argon2.
//!
//! Sealed inside is JSON with the time of the backup and the codes, each as
//! its `otpauth://` URI with its group and sites: whoever decrypts the file
//! can take the codes to any app.

use std::time::{SystemTime, UNIX_EPOCH};

use argon2::{Algorithm, Argon2, Params, Version};
use authwing_totp::Otp;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{Error, Format, Imported, Item, Result, crypto, damaged, uris};

/// What the file says it is.
pub(crate) const FORMAT: &str = "authwing-backup";
/// The file name's extension, without the dot.
pub const EXTENSION: &str = "authwing-backup";
/// The shortest passphrase a backup is sealed with, as the vault's.
pub const MIN_PASSPHRASE: usize = 10;
/// The format's version. A file of a newer one is refused, not guessed at.
const VERSION: u32 = 1;
/// XChaCha20-Poly1305, by the vault's id for it.
const XCHACHA20_POLY1305: u8 = 1;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
const AAD: &[u8] = b"authwing backup v1";
/// The most memory (1 GiB) and passes a file may ask Argon2 for: one that
/// is damaged, or made to, should not take the machine.
const MAX_M_KIB: u32 = 1024 * 1024;
const MAX_T: u32 = 100;
const MAX_P: u32 = 64;

#[derive(Serialize, Deserialize)]
struct Envelope {
    format: String,
    version: u32,
    kdf: Kdf,
    key_id: String,
    cipher: u8,
    nonce: String,
    sealed: String,
}

/// How the passphrase becomes the key: Argon2id with these costs and salt.
#[derive(Serialize, Deserialize)]
struct Kdf {
    alg: String,
    m_kib: u32,
    t: u32,
    p: u32,
    salt: String,
}

impl Kdf {
    fn new() -> Result<Kdf> {
        // Tests run unoptimised: a small cost there, the real one otherwise.
        let (m_kib, t) = if cfg!(test) { (256, 1) } else { (64 * 1024, 3) };
        Ok(Kdf {
            alg: "argon2id".into(),
            m_kib,
            t,
            p: 1,
            salt: B64.encode(crypto::random::<16>()?),
        })
    }

    fn key(&self, passphrase: &str) -> Result<Zeroizing<[u8; KEY_LEN]>> {
        if self.alg != "argon2id" {
            return Err(Error::Unsupported(format!(
                "this backup's key is made with {}, which this version does not know: update authwing",
                self.alg
            )));
        }
        if self.m_kib > MAX_M_KIB || self.t > MAX_T || self.p > MAX_P {
            return Err(damaged(
                "its key would take more memory or time than allowed",
            ));
        }
        let salt = B64.decode(&self.salt).map_err(damaged)?;
        let params = Params::new(self.m_kib, self.t, self.p, Some(KEY_LEN)).map_err(damaged)?;
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(passphrase.as_bytes(), &salt, &mut key[..])
            .map_err(damaged)?;
        Ok(key)
    }
}

/// What is sealed, as written.
#[derive(Serialize)]
struct Contents<'a> {
    /// When the backup was made, in RFC 3339 (UTC).
    exported: String,
    items: Vec<Entry<'a>>,
}

#[derive(Serialize)]
struct Entry<'a> {
    uri: &'a str,
    #[serde(skip_serializing_if = "str::is_empty")]
    group: &'a str,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    sites: &'a [String],
}

/// What is sealed, as read.
#[derive(Deserialize)]
struct Read {
    items: Vec<ReadEntry>,
}

#[derive(Deserialize)]
struct ReadEntry {
    uri: String,
    #[serde(default)]
    group: String,
    #[serde(default)]
    sites: Vec<String>,
}

/// A backup of `items`, sealed with `passphrase`: the bytes of a
/// `.authwing-backup` file. The passphrase must have at least
/// [`MIN_PASSPHRASE`] characters; how guessable it is, the app checks.
pub fn seal(items: &[Item], passphrase: &str) -> Result<Vec<u8>> {
    if passphrase.chars().count() < MIN_PASSPHRASE {
        return Err(Error::WeakPassphrase(MIN_PASSPHRASE));
    }
    let uris: Vec<Zeroizing<String>> = items
        .iter()
        .map(|item| Zeroizing::new(uris::portable(&item.otp)))
        .collect();
    let contents = Contents {
        exported: rfc3339(now()),
        items: items
            .iter()
            .zip(&uris)
            .map(|(item, uri)| Entry {
                uri,
                group: &item.group,
                sites: &item.sites,
            })
            .collect(),
    };
    // Room for it all up front: a buffer that grew would leave copies of the
    // secrets behind in freed memory.
    let text: usize = items
        .iter()
        .zip(&uris)
        .map(|(item, uri)| {
            uri.len() + item.group.len() + item.sites.iter().map(String::len).sum::<usize>()
        })
        .sum();
    let mut plain = Zeroizing::new(Vec::with_capacity(2 * text + 64 * (items.len() + 1)));
    serde_json::to_writer(&mut *plain, &contents).map_err(|e| Error::Seal(e.to_string()))?;

    let kdf = Kdf::new()?;
    let key = kdf.key(passphrase)?;
    let nonce: [u8; NONCE_LEN] = crypto::random()?;
    let sealed = XChaCha20Poly1305::new((&*key).into())
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: &plain,
                aad: AAD,
            },
        )
        .map_err(|e| Error::Seal(e.to_string()))?;
    let envelope = Envelope {
        format: FORMAT.into(),
        version: VERSION,
        kdf,
        key_id: key_id(&key),
        cipher: XCHACHA20_POLY1305,
        nonce: B64.encode(nonce),
        sealed: B64.encode(sealed),
    };
    serde_json::to_vec_pretty(&envelope).map_err(|e| Error::Seal(e.to_string()))
}

/// The codes in a backup [`seal`] made. All or nothing: a code that cannot
/// be read makes the file damaged ([`crate::import`] lists it as skipped
/// instead). A wrong passphrase is [`Error::WrongPassword`].
pub fn open(bytes: &[u8], passphrase: &str) -> Result<Vec<Item>> {
    let imported = read(bytes, passphrase)?;
    match imported.skipped.into_iter().next() {
        Some((what, why)) => Err(damaged(format!("{what}: {why}"))),
        None => Ok(imported.items),
    }
}

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    read(
        bytes,
        password.ok_or(Error::PasswordNeeded(Format::Authwing))?,
    )
}

fn read(bytes: &[u8], passphrase: &str) -> Result<Imported> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(damaged)?;
    if envelope.format != FORMAT {
        return Err(Error::Unknown);
    }
    if envelope.version > VERSION {
        return Err(Error::Unsupported(
            "this backup was made by a newer authwing: update authwing to open it".into(),
        ));
    }
    if envelope.cipher != XCHACHA20_POLY1305 {
        return Err(Error::Unsupported(format!(
            "this backup is sealed with a cipher this version does not know ({}): update authwing",
            envelope.cipher
        )));
    }
    let key = envelope.kdf.key(passphrase)?;
    if key_id(&key) != envelope.key_id {
        return Err(Error::WrongPassword);
    }
    let nonce = B64.decode(&envelope.nonce).map_err(damaged)?;
    if nonce.len() != NONCE_LEN {
        return Err(damaged("its nonce has the wrong length"));
    }
    let sealed = B64.decode(&envelope.sealed).map_err(damaged)?;
    let plain = XChaCha20Poly1305::new((&*key).into())
        .decrypt(
            nonce.as_slice().into(),
            Payload {
                msg: &sealed,
                aad: AAD,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| damaged("it does not decrypt"))?;
    let contents: Read = serde_json::from_slice(&plain).map_err(damaged)?;

    let mut imported = Imported::default();
    for (n, entry) in contents.items.into_iter().enumerate() {
        match Otp::from_uri(&entry.uri) {
            Ok(otp) => imported.items.push(Item {
                otp,
                group: entry.group,
                sites: entry.sites,
            }),
            Err(e) => {
                let what = uris::what(&entry.uri).unwrap_or_else(|| format!("code {}", n + 1));
                imported.skipped.push((what, e.to_string()));
            }
        }
    }
    Ok(imported)
}

/// A hash of the key, safe to keep beside what it seals, to tell a wrong
/// passphrase from damage.
fn key_id(key: &[u8; KEY_LEN]) -> String {
    Sha256::digest(key)[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// `unix_seconds` in RFC 3339, in UTC: `2026-10-23T14:05:09Z`.
fn rfc3339(unix_seconds: u64) -> String {
    let (days, second) = (unix_seconds / 86_400, unix_seconds % 86_400);
    // The civil date of a day count (Howard Hinnant's `civil_from_days`), in
    // 400-year eras that start on 1 March, so leap days come last.
    let z = days as i64 + 719_468;
    let (era, day_of_era) = (z.div_euclid(146_097), z.rem_euclid(146_097));
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        second / 3600,
        second / 60 % 60,
        second % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, code};
    use serde_json::Value;

    const PASSPHRASE: &str = "correct horse battery";

    fn items() -> Vec<Item> {
        URIS.iter()
            .enumerate()
            .map(|(i, uri)| Item {
                otp: code(uri),
                group: if i % 2 == 0 {
                    "Work".into()
                } else {
                    String::new()
                },
                sites: if i == 0 {
                    vec!["deno.com".into(), "deno.land".into()]
                } else {
                    vec![]
                },
            })
            .collect()
    }

    fn envelope(bytes: &[u8]) -> Value {
        serde_json::from_slice(bytes).expect("json")
    }

    #[test]
    fn round_trips() {
        let items = items();
        let sealed = seal(&items, PASSPHRASE).expect("seal");
        assert_eq!(open(&sealed, PASSPHRASE).expect("open"), items);
        let imported = crate::import(&sealed, Some(PASSPHRASE)).expect("import");
        assert_eq!((imported.items, imported.skipped), (items, vec![]));
        assert_eq!(
            open(&seal(&[], PASSPHRASE).expect("seal"), PASSPHRASE).expect("open"),
            []
        );
    }

    #[test]
    fn says_how_it_was_sealed_and_hides_the_rest() {
        let sealed = seal(&items(), PASSPHRASE).expect("seal");
        let json = envelope(&sealed);
        assert_eq!(json["format"], "authwing-backup");
        assert_eq!(json["version"], 1);
        assert_eq!(json["kdf"]["alg"], "argon2id");
        assert_eq!(json["kdf"]["p"], 1);
        assert_eq!(json["cipher"], 1);
        let text = String::from_utf8(sealed).expect("utf-8");
        for hidden in [
            "Deno",
            "Mason",
            "4SJHB4GSD43FZBAI7C2HLRJGPQ",
            "Work",
            "deno.com",
        ] {
            assert!(!text.contains(hidden), "{hidden} shows in {text}");
        }
    }

    #[test]
    fn inside_are_uris_any_app_reads() {
        let items = items();
        let sealed = seal(&items, PASSPHRASE).expect("seal");
        let json = envelope(&sealed);
        let kdf: Kdf = serde_json::from_value(json["kdf"].clone()).expect("kdf");
        let key = kdf.key(PASSPHRASE).expect("key");
        let nonce = B64
            .decode(json["nonce"].as_str().expect("nonce"))
            .expect("b64");
        let data = B64
            .decode(json["sealed"].as_str().expect("sealed"))
            .expect("b64");
        let plain = XChaCha20Poly1305::new((&*key).into())
            .decrypt(
                nonce.as_slice().into(),
                Payload {
                    msg: &data,
                    aad: AAD,
                },
            )
            .expect("decrypt");
        let contents: Value = serde_json::from_slice(&plain).expect("json");
        let exported = contents["exported"].as_str().expect("time");
        assert_eq!((exported.len(), &exported[10..11]), (20, "T"));
        let first = &contents["items"][0];
        assert_eq!(first["uri"], items[0].otp.to_uri());
        assert_eq!(first["group"], "Work");
        assert_eq!(first["sites"], serde_json::json!(["deno.com", "deno.land"]));
        // Empty groups and sites are left out; Steam is written as Aegis,
        // 2FAS and Ente Auth read it.
        let steam = &contents["items"][6];
        assert!(steam.get("sites").is_none());
        assert!(
            steam["uri"]
                .as_str()
                .expect("uri")
                .starts_with("otpauth://steam/")
        );
        assert!(contents["items"][1].get("group").is_none());
    }

    #[test]
    fn a_short_passphrase_is_refused() {
        assert!(matches!(
            seal(&items(), "123456789"),
            Err(Error::WeakPassphrase(10))
        ));
        // Characters, not bytes.
        assert!(seal(&[], "ñññññññññ").is_err());
        assert!(seal(&[], "ññññññññññ").is_ok());
    }

    #[test]
    fn wrong_passphrase_is_told_from_damage() {
        let sealed = seal(&items(), PASSPHRASE).expect("seal");
        assert!(matches!(
            open(&sealed, "correct horse battery!"),
            Err(Error::WrongPassword)
        ));
        assert!(matches!(
            crate::import(&sealed, None),
            Err(Error::PasswordNeeded(Format::Authwing))
        ));

        // A changed byte in the ciphertext, with the right passphrase.
        let mut json = envelope(&sealed);
        let mut data = B64
            .decode(json["sealed"].as_str().expect("sealed"))
            .expect("b64");
        data[5] ^= 1;
        json["sealed"] = B64.encode(&data).into();
        let damaged_file = serde_json::to_vec(&json).expect("json");
        assert!(matches!(
            open(&damaged_file, PASSPHRASE),
            Err(Error::Damaged(_))
        ));
        // A file cut short is no JSON, and still says what it is.
        let cut = &sealed[..sealed.len() / 2];
        assert!(matches!(open(cut, PASSPHRASE), Err(Error::Damaged(_))));
        assert!(matches!(
            crate::import(cut, Some(PASSPHRASE)),
            Err(Error::Damaged(_))
        ));
    }

    #[test]
    fn newer_files_and_greedy_ones_are_refused() {
        let sealed = seal(&[], PASSPHRASE).expect("seal");
        let mut json = envelope(&sealed);
        json["version"] = 2.into();
        let newer = serde_json::to_vec(&json).expect("json");
        assert!(matches!(
            open(&newer, PASSPHRASE),
            Err(Error::Unsupported(_))
        ));

        let mut json = envelope(&sealed);
        json["cipher"] = 9.into();
        let other_cipher = serde_json::to_vec(&json).expect("json");
        assert!(matches!(
            open(&other_cipher, PASSPHRASE),
            Err(Error::Unsupported(_))
        ));

        let mut json = envelope(&sealed);
        json["kdf"]["m_kib"] = (64 * 1024 * 1024).into();
        let greedy = serde_json::to_vec(&json).expect("json");
        assert!(matches!(open(&greedy, PASSPHRASE), Err(Error::Damaged(_))));
    }

    #[test]
    fn a_code_it_cannot_read_is_skipped_by_import_and_fails_open() {
        // As a later version might write one: a kind this one does not know.
        let mut items = items();
        items.truncate(1);
        let kdf = Kdf::new().expect("kdf");
        let key = kdf.key(PASSPHRASE).expect("key");
        let plain = serde_json::json!({
            "exported": "2026-10-23T00:00:00Z",
            "items": [
                {"uri": items[0].otp.to_uri(), "group": "Work", "sites": ["deno.com", "deno.land"]},
                {"uri": "otpauth://motp/Old:bob?secret=JBSWY3DPEHPK3PXP"},
            ],
        });
        let nonce = [3u8; NONCE_LEN];
        let data = XChaCha20Poly1305::new((&*key).into())
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: &serde_json::to_vec(&plain).expect("json"),
                    aad: AAD,
                },
            )
            .expect("encrypt");
        let file = serde_json::to_vec(&Envelope {
            format: FORMAT.into(),
            version: VERSION,
            key_id: key_id(&key),
            kdf,
            cipher: XCHACHA20_POLY1305,
            nonce: B64.encode(nonce),
            sealed: B64.encode(data),
        })
        .expect("json");
        let imported = crate::import(&file, Some(PASSPHRASE)).expect("import");
        items[0].sites = vec!["deno.com".into(), "deno.land".into()];
        assert_eq!(imported.items, items);
        assert_eq!(
            imported.skipped,
            [(
                "Old:bob".to_string(),
                "unknown kind of code: motp".to_string()
            )]
        );
        assert!(matches!(open(&file, PASSPHRASE), Err(Error::Damaged(_))));
    }

    #[test]
    fn writes_utc_times() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_761_228_309), "2025-10-23T14:05:09Z");
        assert_eq!(rfc3339(4_107_542_399), "2100-02-28T23:59:59Z");
    }
}
