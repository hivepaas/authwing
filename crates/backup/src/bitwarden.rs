//! Bitwarden's JSON export
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
//! Protected by a password (https://bitwarden.com/help/encrypted-export/),
//! the export is that JSON sealed as one of Bitwarden's "EncString"s of
//! type 2, `2.iv|ciphertext|mac` in base64: AES-256-CBC, then HMAC-SHA256
//! over the IV and the ciphertext. Its two keys are stretched with
//! HKDF-Expand (SHA-256, `enc` and `mac`) from the key the file's KDF makes
//! from the password and `salt`, the base64 text itself: PBKDF2-SHA256
//! (`kdfType` 0), or Argon2id (1) with the salt's SHA-256 and the memory in
//! MiB. Costs beyond those Bitwarden takes are refused before any key is
//! made. `encKeyValidation_DO_NOT_EDIT` is a known text sealed the same
//! way: when it does not open, the password is wrong; when it does and
//! `data` does not, the file is damaged. As Bitwarden's clients write and
//! read it (https://github.com/bitwarden/clients,
//! `libs/tools/export-vault-core/src/services/base-vault-export.service.ts`,
//! `libs/importer/src/importers/bitwarden/bitwarden-password-protected-importer.ts`
//! and `password-protected-kdf-config.ts`), with the KDF and the cipher of
//! their SDK (https://github.com/bitwarden/sdk-internal,
//! `crates/bitwarden-crypto/src/keys/kdf.rs`, `keys/utils.rs` and
//! `hazmat/symmetric_encryption/aes256_cbc_hmac_sha256_ae.rs`).
//!
//! An export restricted to the account, sealed with the account's own key,
//! is refused with a word on exporting again: only Bitwarden has that key.

use std::ops::RangeInclusive;

use argon2::{Algorithm, Argon2, Params, Version};
use authwing_totp::{DEFAULT_PERIOD, Kind, Otp, Secret};
use base64::Engine;
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use url::Url;
use zeroize::Zeroizing;

use crate::crypto::{self, BASE64};
use crate::uris::has_prefix;
use crate::{Error, Format, Imported, Item, Result, damaged, label};

/// Bitwarden's URI match types that are no address: a regular expression,
/// and never.
const REGEX_MATCH: u8 = 4;
const NEVER_MATCH: u8 = 5;

/// The KDFs, by `kdfType`.
const PBKDF2_SHA256: u32 = 0;
const ARGON2ID: u32 = 1;
/// The costs Bitwarden takes in an export it imports: a file damaged, or
/// made to, should not take the machine (Argon2's memory is in MiB).
const PBKDF2_ITERATIONS: RangeInclusive<u32> = 5_000..=2_000_000;
const ARGON2_ITERATIONS: RangeInclusive<u32> = 2..=10;
const ARGON2_MEMORY: RangeInclusive<u32> = 16..=1024;
const ARGON2_PARALLELISM: RangeInclusive<u32> = 1..=16;
const OUT_OF_BOUNDS: &str = "its key's costs are not ones Bitwarden makes";
const KEY_LEN: usize = 32;

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

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let mut file: File = serde_json::from_slice(bytes).map_err(damaged)?;
    if file.encrypted && file.password_protected {
        let password = password.ok_or(Error::PasswordNeeded(Format::Bitwarden))?;
        let sealed: Protected = serde_json::from_slice(bytes).map_err(damaged)?;
        file = serde_json::from_slice(&sealed.open(password)?).map_err(damaged)?;
    }
    if file.encrypted || file.password_protected {
        return Err(Error::Unsupported(
            "this Bitwarden export is encrypted with the account's own key, which \
             authwing cannot read: export the vault from Bitwarden again as .json, \
             or as .json (Encrypted) protected by a password"
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

/// A password-protected export, as sealed (see the module docs).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Protected {
    salt: String,
    kdf_type: u32,
    kdf_iterations: u32,
    kdf_memory: Option<u32>,
    kdf_parallelism: Option<u32>,
    #[serde(rename = "encKeyValidation_DO_NOT_EDIT")]
    key_check: String,
    data: String,
}

impl Protected {
    /// The plain export `data` seals, if `password` is the file's.
    fn open(&self, password: &str) -> Result<Zeroizing<Vec<u8>>> {
        let (check, data) = (
            EncString::parse(&self.key_check)?,
            EncString::parse(&self.data)?,
        );
        let key = self.key(password)?;
        let keys = Keys::stretched(&key)?;
        if check.open(&keys).is_none() {
            return Err(Error::WrongPassword);
        }
        data.open(&keys)
            .ok_or_else(|| damaged("the vault does not decrypt"))
    }

    /// The key the file's KDF makes from `password`, before it is stretched.
    fn key(&self, password: &str) -> Result<Zeroizing<[u8; KEY_LEN]>> {
        let (password, salt) = (password.as_bytes(), self.salt.as_bytes());
        let iterations = self.kdf_iterations;
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        match self.kdf_type {
            PBKDF2_SHA256 => {
                if !PBKDF2_ITERATIONS.contains(&iterations) {
                    return Err(damaged(OUT_OF_BOUNDS));
                }
                pbkdf2::pbkdf2_hmac::<Sha256>(password, salt, iterations, &mut key[..]);
            }
            ARGON2ID => {
                let memory = self.kdf_memory.unwrap_or_default();
                let parallelism = self.kdf_parallelism.unwrap_or_default();
                if !ARGON2_ITERATIONS.contains(&iterations)
                    || !ARGON2_MEMORY.contains(&memory)
                    || !ARGON2_PARALLELISM.contains(&parallelism)
                {
                    return Err(damaged(OUT_OF_BOUNDS));
                }
                let params = Params::new(memory * 1024, iterations, parallelism, Some(KEY_LEN))
                    .map_err(damaged)?;
                Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
                    .hash_password_into(password, &Sha256::digest(salt), &mut key[..])
                    .map_err(damaged)?;
            }
            other => {
                return Err(Error::Unsupported(format!(
                    "this Bitwarden export's key is made in a way authwing does not know \
                     (kdfType {other})"
                )));
            }
        }
        Ok(key)
    }
}

/// The keys a password-protected export is sealed with: one for AES-256-CBC
/// and one for HMAC-SHA256.
struct Keys {
    encryption: Zeroizing<[u8; KEY_LEN]>,
    mac: Zeroizing<[u8; KEY_LEN]>,
}

impl Keys {
    /// The keys Bitwarden stretches `key` into, with HKDF-Expand.
    fn stretched(key: &[u8; KEY_LEN]) -> Result<Keys> {
        let hkdf = Hkdf::<Sha256>::from_prk(key).map_err(damaged)?;
        let mut keys = Keys {
            encryption: Zeroizing::new([0u8; KEY_LEN]),
            mac: Zeroizing::new([0u8; KEY_LEN]),
        };
        hkdf.expand(b"enc", &mut keys.encryption[..])
            .map_err(damaged)?;
        hkdf.expand(b"mac", &mut keys.mac[..]).map_err(damaged)?;
        Ok(keys)
    }
}

/// One of Bitwarden's sealed values, an "EncString" of type 2.
struct EncString {
    iv: Vec<u8>,
    ciphertext: Vec<u8>,
    mac: Vec<u8>,
}

impl EncString {
    /// Reads `2.iv|ciphertext|mac`.
    fn parse(text: &str) -> Result<EncString> {
        let Some((kind, parts)) = text.trim().split_once('.') else {
            return Err(damaged("an encrypted value does not say how it is sealed"));
        };
        if kind != "2" {
            return Err(Error::Unsupported(format!(
                "this Bitwarden export is sealed in a way authwing does not know (type {kind})"
            )));
        }
        let parts: Vec<&str> = parts.split('|').collect();
        let [iv, ciphertext, mac] = parts[..] else {
            return Err(damaged("an encrypted value is not in three parts"));
        };
        let decode = |part: &str| BASE64.decode(part.trim()).map_err(damaged);
        Ok(EncString {
            iv: decode(iv)?,
            ciphertext: decode(ciphertext)?,
            mac: decode(mac)?,
        })
    }

    /// What it seals, if `keys` sealed it: `None` for other keys, or damage.
    fn open(&self, keys: &Keys) -> Option<Zeroizing<Vec<u8>>> {
        let mut mac = Hmac::<Sha256>::new_from_slice(&keys.mac[..]).ok()?;
        mac.update(&self.iv);
        mac.update(&self.ciphertext);
        mac.verify_slice(&self.mac).ok()?;
        crypto::aes_cbc_open(&keys.encryption[..], &self.iv, &self.ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes, code as uri_code};
    use serde_json::json;

    const PLAIN: &[u8] = include_bytes!("../testdata/bitwarden.json");

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
    fn account_restricted_exports_say_how_to_export() {
        let account_restricted = json!({
            "encrypted": true, "encKeyValidation_DO_NOT_EDIT": "2.x|y|z",
            "folders": [], "items": [{"name": "2.x|y|z", "login": {"totp": "2.x|y|z"}}],
        });
        let bytes = serde_json::to_vec(&account_restricted).expect("json");
        assert_eq!(crate::detect(&bytes), Some(Format::Bitwarden));
        for password in [None, Some("test")] {
            assert!(matches!(
                crate::import(&bytes, password),
                Err(Error::Unsupported(_))
            ));
        }
    }

    /// Seals `plain` as an EncString under `keys`, as Bitwarden does.
    fn seal(keys: &Keys, iv: [u8; 16], plain: &[u8]) -> String {
        let ciphertext = crypto::aes_cbc_seal(&keys.encryption[..], &iv, plain);
        let mut mac = Hmac::<Sha256>::new_from_slice(&keys.mac[..]).expect("key");
        mac.update(&iv);
        mac.update(&ciphertext);
        let mac = mac.finalize().into_bytes();
        format!(
            "2.{}|{}|{}",
            BASE64.encode(iv),
            BASE64.encode(&ciphertext),
            BASE64.encode(mac)
        )
    }

    /// `plain` protected by `password` as Bitwarden exports it
    /// (`buildPasswordExport` in its clients), with the KDF settings `kdf`
    /// gives: a salt of 16 bytes in base64, a GUID sealed to check the
    /// password with, and the export sealed after it.
    fn protect(plain: &[u8], password: &str, kdf: Value) -> Vec<u8> {
        let fields = json!({
            "encrypted": true,
            "passwordProtected": true,
            "salt": BASE64.encode(*b"sixteen bytes!!!"),
            "encKeyValidation_DO_NOT_EDIT": "",
            "data": "",
        });
        let mut file = merged(fields, kdf);
        let sealed: Protected = serde_json::from_value(file.clone()).expect("fields");
        let keys = Keys::stretched(&sealed.key(password).expect("key")).expect("keys");
        let guid = b"0f6b1a52-6c1d-4a3e-9d2b-3f1f8e0c7a41";
        file["encKeyValidation_DO_NOT_EDIT"] = seal(&keys, [1; 16], guid).into();
        file["data"] = seal(&keys, [2; 16], plain).into();
        serde_json::to_vec_pretty(&file).expect("json")
    }

    /// `object` with the fields of `more` added, or put in place.
    fn merged(mut object: Value, more: Value) -> Value {
        if let (Some(object), Value::Object(more)) = (object.as_object_mut(), more) {
            object.extend(more);
        }
        object
    }

    /// Bitwarden's least PBKDF2, and its least Argon2id: quick to test.
    fn pbkdf2() -> Value {
        json!({"kdfType": 0, "kdfIterations": 5000})
    }

    fn argon2id() -> Value {
        json!({"kdfType": 1, "kdfIterations": 2, "kdfMemory": 16, "kdfParallelism": 1})
    }

    #[test]
    fn reads_a_password_protected_export() {
        let plain = crate::import(PLAIN, None).expect("plain");
        for kdf in [pbkdf2(), argon2id()] {
            let bytes = protect(PLAIN, "correct horse", kdf);
            assert_eq!(crate::detect(&bytes), Some(Format::Bitwarden));
            assert!(matches!(
                crate::import(&bytes, None),
                Err(Error::PasswordNeeded(Format::Bitwarden))
            ));
            let imported = crate::import(&bytes, Some("correct horse")).expect("import");
            assert_eq!(imported, plain);
        }
    }

    #[test]
    fn wrong_password_is_told_from_damage() {
        let bytes = protect(PLAIN, "correct horse", pbkdf2());
        assert!(matches!(
            crate::import(&bytes, Some("correct horses")),
            Err(Error::WrongPassword)
        ));
        let mut file: Value = serde_json::from_slice(&bytes).expect("json");
        let to_bytes = |file: &Value| serde_json::to_vec(file).expect("json");
        let data = file["data"].as_str().expect("data").to_string();
        let (iv, rest) = data.split_once('|').expect("parts");
        let (ciphertext, mac) = rest.split_once('|').expect("parts");
        let mut ciphertext = BASE64.decode(ciphertext).expect("base64");
        ciphertext[5] ^= 1;
        file["data"] = format!("{iv}|{}|{mac}", BASE64.encode(ciphertext)).into();
        assert!(matches!(
            crate::import(&to_bytes(&file), Some("correct horse")),
            Err(Error::Damaged(_))
        ));
        file["data"] = "2.AAAA|AAAA".into();
        assert!(matches!(
            crate::import(&to_bytes(&file), Some("correct horse")),
            Err(Error::Damaged(_))
        ));
        file["data"] = "7.AAAA".into();
        assert!(matches!(
            crate::import(&to_bytes(&file), Some("correct horse")),
            Err(Error::Unsupported(_))
        ));
    }

    #[test]
    fn costs_beyond_bitwardens_are_refused() {
        for kdf in [
            json!({"kdfType": 0, "kdfIterations": 4999}),
            json!({"kdfType": 0, "kdfIterations": 2_000_001}),
            json!({"kdfType": 1, "kdfIterations": 2, "kdfMemory": 1025, "kdfParallelism": 1}),
            json!({"kdfType": 1, "kdfIterations": 11, "kdfMemory": 16, "kdfParallelism": 1}),
            json!({"kdfType": 1, "kdfIterations": 2, "kdfMemory": 16, "kdfParallelism": 17}),
            json!({"kdfType": 1, "kdfIterations": 2}),
        ] {
            let file: Value =
                serde_json::from_slice(&protect(PLAIN, "pw", pbkdf2())).expect("json");
            let bytes = serde_json::to_vec(&merged(file, kdf.clone())).expect("json");
            assert!(
                matches!(crate::import(&bytes, Some("pw")), Err(Error::Damaged(_))),
                "{kdf}"
            );
        }
        let mut file: Value =
            serde_json::from_slice(&protect(PLAIN, "pw", pbkdf2())).expect("json");
        file["kdfType"] = 2.into();
        let bytes = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&bytes, Some("pw")),
            Err(Error::Unsupported(_))
        ));
    }

    /// Known answers from Bitwarden's SDK (https://github.com/bitwarden/sdk-internal,
    /// `crates/bitwarden-crypto/src/keys/master_key.rs`, `keys/utils.rs` and
    /// `hazmat/symmetric_encryption/aes256_cbc_hmac_sha256_ae.rs`): the
    /// round trips above check the parts fit; these, that each is Bitwarden's.
    #[test]
    fn keys_and_ciphers_are_bitwardens() {
        // The KDFs, checked by the hash Bitwarden's server is sent: one
        // round of PBKDF2 over the key, with the password as the salt.
        let server_hash = |kdf: Value, salt: &str| {
            let fields = json!({"salt": salt, "encKeyValidation_DO_NOT_EDIT": "", "data": ""});
            let sealed: Protected = serde_json::from_value(merged(fields, kdf)).expect("fields");
            let key = sealed.key("asdfasdf").expect("key");
            let mut hash = [0u8; KEY_LEN];
            pbkdf2::pbkdf2_hmac::<Sha256>(&key[..], b"asdfasdf", 1, &mut hash);
            BASE64.encode(hash)
        };
        assert_eq!(
            server_hash(
                json!({"kdfType": 0, "kdfIterations": 100_000}),
                "test@bitwarden.com"
            ),
            "wmyadRMyBZOH7P/a/ucTCbSghKgdzDpPqUnu/DAVtSw="
        );
        assert_eq!(
            server_hash(
                json!({"kdfType": 1, "kdfIterations": 4, "kdfMemory": 32, "kdfParallelism": 2}),
                "test_salt"
            ),
            "PR6UjYmjmppTYcdyTiNbAhPJuQQOmynKbdEl1oyi/iQ="
        );

        let hex = |text: &str| crate::crypto::hex(text).expect("hex");
        let key: [u8; KEY_LEN] =
            hex("1f4f68e29647b15ac250acd1118184518aa745a7fe95021b27c5402a16c3564b")
                .try_into()
                .expect("32 bytes");
        let keys = Keys::stretched(&key).expect("keys");
        assert_eq!(
            keys.encryption[..],
            hex("6f1fb22dee9825728fd77c5387adc3178e8678f93d84a3b671c5bdccbc15ed60")
        );
        assert_eq!(
            keys.mac[..],
            hex("dd7fceea651bca265634221c4e1cb910303d7fa6d1f7c257e81a3055c1f9b39b")
        );

        let bytes: Vec<u8> = (0..64).collect();
        let keys = Keys {
            encryption: Zeroizing::new(bytes[..32].try_into().expect("32 bytes")),
            mac: Zeroizing::new(bytes[32..].try_into().expect("32 bytes")),
        };
        let sealed = EncString::parse(
            "2.2NokAMS6llUxk26oueMqrA==|6k0QD71SJLy2WEBDkV4esiTrgkP/z7eoSedSesGLGYE=\
             |PE4sb0jpAwZW+tnyPuW43eeWvSxjvdw3xMJlPGbDlYI=",
        )
        .expect("EncString");
        assert_eq!(
            sealed.open(&keys).as_deref().map(Vec::as_slice),
            Some(&b"Bitwarden SDK test vector"[..])
        );
        let mut tampered = sealed;
        tampered.mac[0] ^= 1;
        assert_eq!(tampered.open(&keys), None);
    }
}
