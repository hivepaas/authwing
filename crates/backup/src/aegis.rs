//! Aegis's vault export, as
//! https://github.com/beemdevelopment/Aegis/blob/master/docs/vault.md has
//! it: JSON whose `db` is the content itself, or the content sealed with
//! AES-256-GCM under a master key. Each password slot keeps the master key
//! sealed the same way, under a key scrypt makes from the password (Aegis
//! writes N = 2^15, r = 8, p = 1; the slot says which). A password no slot
//! opens is the wrong one; a master key that does not open the content
//! means damage. A slot Aegis never repaired (`repaired: false`) from its
//! old encoding of passwords over 64 bytes does not open here.
//!
//! Groups come in two forms: content version 3 lists them by UUID and each
//! entry names the UUIDs of its own; version 2 gave an entry a `group` name
//! (Aegis's `Vault.fromJson` and `VaultEntry.fromJson`). An entry in several
//! groups goes in the first that is listed. mOTP and Yandex codes are
//! skipped: authwing does not make them.

use authwing_totp::fields::{ACCOUNT, ALGORITHM, COUNTER, DIGITS, ISSUER, KIND, PERIOD, SECRET};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use zeroize::Zeroizing;

use crate::crypto::{self, BASE64};
use crate::{Error, Format, Imported, Item, Result, damaged, label, otp_from};

/// The file format's version this reads.
const VERSION: u32 = 1;
/// The slot type of a password (biometric slots are 2).
const PASSWORD_SLOT: u8 = 1;
/// The most memory scrypt may take for a slot, 128 × r × N bytes: 1 GiB,
/// where Aegis's own takes 32 MiB. A file damaged, or made to, should not
/// take the machine; nor should its `p`, which repeats the work.
const MAX_SCRYPT_MEMORY: u64 = 1 << 30;
const MAX_SCRYPT_P: u32 = 16;

#[derive(Deserialize)]
struct File {
    version: u32,
    #[serde(default)]
    header: Header,
    db: Value,
}

#[derive(Default, Deserialize)]
struct Header {
    slots: Option<Vec<Slot>>,
    params: Option<Sealing>,
}

#[derive(Deserialize)]
struct Slot {
    #[serde(rename = "type")]
    kind: u8,
    key: String,
    key_params: Sealing,
    #[serde(default)]
    n: u64,
    #[serde(default)]
    r: u32,
    #[serde(default)]
    p: u32,
    #[serde(default)]
    salt: String,
}

/// AES-GCM's nonce and tag, in hexadecimal.
#[derive(Deserialize)]
struct Sealing {
    nonce: String,
    tag: String,
}

#[derive(Deserialize)]
struct Content {
    entries: Vec<Value>,
    #[serde(default)]
    groups: Vec<Group>,
}

#[derive(Deserialize)]
struct Group {
    uuid: String,
    name: String,
}

#[derive(Deserialize)]
struct Entry {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    issuer: String,
    info: Info,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    group: Option<String>,
}

#[derive(Deserialize)]
struct Info {
    #[serde(default)]
    secret: String,
    algo: Option<String>,
    digits: Option<u32>,
    period: Option<u64>,
    counter: Option<u64>,
}

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let file: File = serde_json::from_slice(bytes).map_err(damaged)?;
    if file.version > VERSION {
        return Err(Error::Unsupported(
            "this Aegis vault is of a newer version than authwing reads".into(),
        ));
    }
    let content: Content = match file.db {
        Value::String(sealed) => {
            let password = password.ok_or(Error::PasswordNeeded(Format::Aegis))?;
            let plain = decrypt(&file.header, &sealed, password)?;
            serde_json::from_slice(&plain).map_err(damaged)?
        }
        db => serde_json::from_value(db).map_err(damaged)?,
    };

    let mut imported = Imported::default();
    for (n, value) in content.entries.into_iter().enumerate() {
        let text = |key| value.get(key).and_then(Value::as_str).unwrap_or_default();
        let what = label(text("issuer"), text("name"), || format!("entry {}", n + 1));
        let entry: Entry = match serde_json::from_value(value) {
            Ok(entry) => entry,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        let info = entry.info;
        let otp = otp_from(&[
            (ISSUER, Some(entry.issuer)),
            (ACCOUNT, Some(entry.name)),
            (SECRET, Some(info.secret)),
            (KIND, Some(entry.kind)),
            (ALGORITHM, info.algo),
            (DIGITS, info.digits.map(|d| d.to_string())),
            (PERIOD, info.period.map(|p| p.to_string())),
            (COUNTER, info.counter.map(|c| c.to_string())),
        ]);
        match otp {
            Ok(otp) => {
                let by_uuid = entry.groups.iter().find_map(|uuid| {
                    let group = content.groups.iter().find(|g| g.uuid == *uuid)?;
                    Some(group.name.clone())
                });
                imported.items.push(Item {
                    otp,
                    group: by_uuid.or(entry.group).unwrap_or_default(),
                    sites: Vec::new(),
                });
            }
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    Ok(imported)
}

/// The content `db` holds, sealed with the master key a password slot
/// keeps for `password`.
fn decrypt(header: &Header, db: &str, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let sealing = header
        .params
        .as_ref()
        .ok_or_else(|| damaged("no encryption parameters"))?;
    let slots: Vec<&Slot> = header
        .slots
        .iter()
        .flatten()
        .filter(|slot| slot.kind == PASSWORD_SLOT)
        .collect();
    if slots.is_empty() {
        return Err(Error::Unsupported(
            "this Aegis vault has no password to open it with".into(),
        ));
    }
    // Aegis may keep a second password for backups: any slot will do.
    let (mut master, mut damage) = (None, None);
    for slot in slots {
        match open_slot(slot, password) {
            Ok(Some(key)) => {
                master = Some(key);
                break;
            }
            Ok(None) => {}
            Err(e) => damage = Some(e),
        }
    }
    let Some(master) = master else {
        return Err(damage.unwrap_or(Error::WrongPassword));
    };
    let sealed = BASE64.decode(db.trim()).map_err(damaged)?;
    open(&master, sealing, sealed).ok_or_else(|| damaged("the vault does not decrypt"))
}

/// The master key `slot` keeps, if `password` is the slot's.
fn open_slot(slot: &Slot, password: &str) -> Result<Option<Zeroizing<Vec<u8>>>> {
    if !slot.n.is_power_of_two() || slot.n < 2 || slot.r == 0 || slot.p == 0 {
        return Err(damaged("a password slot has invalid scrypt parameters"));
    }
    let memory = 128u64
        .saturating_mul(u64::from(slot.r))
        .saturating_mul(slot.n);
    if memory > MAX_SCRYPT_MEMORY || slot.p > MAX_SCRYPT_P {
        return Err(damaged(
            "a password slot would take too much memory or time",
        ));
    }
    let salt = crypto::hex(&slot.salt).ok_or_else(|| damaged("a slot's salt is not hex"))?;
    let params =
        scrypt::Params::new(slot.n.trailing_zeros() as u8, slot.r, slot.p, 32).map_err(damaged)?;
    let mut key = Zeroizing::new([0u8; 32]);
    scrypt::scrypt(password.as_bytes(), &salt, &params, &mut key[..]).map_err(damaged)?;
    let wrapped = crypto::hex(&slot.key).ok_or_else(|| damaged("a slot's key is not hex"))?;
    Ok(open(&key[..], &slot.key_params, wrapped))
}

/// What AES-GCM sealed as `sealed`, with the nonce and tag `sealing` gives.
fn open(key: &[u8], sealing: &Sealing, mut sealed: Vec<u8>) -> Option<Zeroizing<Vec<u8>>> {
    let nonce = crypto::hex(&sealing.nonce)?;
    sealed.extend(crypto::hex(&sealing.tag)?);
    crypto::aes_gcm_open(key, &nonce, &sealed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use serde_json::json;

    const PLAIN: &[u8] = include_bytes!("../testdata/aegis_plain.json");
    const ENCRYPTED: &[u8] = include_bytes!("../testdata/aegis_encrypted.json");

    #[test]
    fn reads_a_plain_vault() {
        let imported = crate::import(PLAIN, None).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn reads_an_encrypted_vault() {
        assert!(matches!(
            crate::import(ENCRYPTED, None),
            Err(Error::PasswordNeeded(Format::Aegis))
        ));
        let imported = crate::import(ENCRYPTED, Some("test")).expect("import");
        assert_codes(&imported.items, &URIS);
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn wrong_password_is_told_from_damage() {
        assert!(matches!(
            crate::import(ENCRYPTED, Some("tset")),
            Err(Error::WrongPassword)
        ));
        let mut file: Value = serde_json::from_slice(ENCRYPTED).expect("json");
        let mut db = BASE64
            .decode(file["db"].as_str().expect("db"))
            .expect("base64");
        db[10] ^= 1;
        file["db"] = BASE64.encode(db).into();
        let damaged_file = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&damaged_file, Some("test")),
            Err(Error::Damaged(_))
        ));
    }

    #[test]
    fn maps_both_kinds_of_group_and_skips_what_it_cannot_use() {
        let secret = "JBSWY3DPEHPK3PXP";
        let entry = |kind: &str, issuer: &str, info: Value, extra: Value| {
            let mut entry = json!({
                "type": kind, "uuid": "0", "name": "alice", "issuer": issuer, "info": info,
            });
            if let (Some(entry), Some(extra)) = (entry.as_object_mut(), extra.as_object()) {
                entry.extend(extra.clone());
            }
            entry
        };
        let totp = json!({"secret": secret, "algo": "SHA1", "digits": 6, "period": 30});
        let file = json!({
            "version": 1,
            "header": {"slots": null, "params": null},
            "db": {
                "version": 3,
                "groups": [
                    {"uuid": "11111111-0000-4000-8000-000000000000", "name": "Work"},
                    {"uuid": "22222222-0000-4000-8000-000000000000", "name": "Home"},
                ],
                "entries": [
                    // An unknown group first, then a listed one.
                    entry("totp", "Version 3", totp.clone(), json!({"groups": [
                        "99999999-0000-4000-8000-000000000000",
                        "22222222-0000-4000-8000-000000000000",
                    ]})),
                    entry("totp", "Version 2", totp.clone(), json!({"group": "Old"})),
                    entry("totp", "None", totp.clone(), json!({})),
                    entry("motp", "Mobile", json!({"secret": secret, "algo": "MD5", "digits": 6,
                        "period": 10, "pin": "1234"}), json!({})),
                    entry("totp", "Hashed", json!({"secret": secret, "algo": "MD5"}), json!({})),
                    entry("hotp", "Empty", json!({"secret": "", "counter": 1}), json!({})),
                    json!({"type": "totp", "name": "broken"}),
                ],
            },
        });
        let imported =
            crate::import(&serde_json::to_vec(&file).expect("json"), None).expect("import");
        let groups: Vec<(&str, &str)> = imported
            .items
            .iter()
            .map(|item| (item.otp.issuer.as_str(), item.group.as_str()))
            .collect();
        assert_eq!(
            groups,
            [("Version 3", "Home"), ("Version 2", "Old"), ("None", "")]
        );
        let skipped: Vec<(&str, &str)> = imported
            .skipped
            .iter()
            .map(|(what, why)| (what.as_str(), &why[..why.find(':').unwrap_or(why.len())]))
            .collect();
        assert_eq!(
            skipped,
            [
                ("Mobile (alice)", "unknown kind of code"),
                ("Hashed (alice)", "unsupported algorithm"),
                ("Empty (alice)", "the secret is missing"),
                ("broken", "missing field `info`"),
            ]
        );
    }

    #[test]
    fn greedy_or_odd_slots_are_damage() {
        let mut file: Value = serde_json::from_slice(ENCRYPTED).expect("json");
        file["header"]["slots"][0]["n"] = (1u64 << 40).into();
        let greedy = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&greedy, Some("test")),
            Err(Error::Damaged(_))
        ));
        file["header"]["slots"][0]["n"] = 1000.into();
        let odd = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&odd, Some("test")),
            Err(Error::Damaged(_))
        ));
        file["header"]["slots"][0]["type"] = 2.into();
        let biometric_only = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&biometric_only, Some("test")),
            Err(Error::Unsupported(_))
        ));
    }
}
