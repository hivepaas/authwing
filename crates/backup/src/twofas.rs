//! 2FAS's backup (`.2fas`): JSON with the services (its codes) and groups,
//! schema versions 1 to 4. Encrypted, the services are `servicesEncrypted`,
//! `ciphertext+tag:salt:iv` in base64, sealed with AES-256-GCM under a key
//! PBKDF2-HMAC-SHA256 makes from the password (10,000 rounds, 32 bytes).
//! `reference` is a known text sealed the same way: when it does not open,
//! the password is wrong; when it does and the services do not, the file is
//! damaged. Groups stay in the clear.
//!
//! As 2FAS's apps write it: Android's (https://github.com/twofas/2fas-android,
//! `core/cipher/src/main/java/com/twofasapp/cipher/backup/BackupCipherImpl.kt`,
//! `internal/BackupKeyGeneratorPbkdf.kt`, `cipher/internal/CipherAes.kt`,
//! `data/services/src/main/java/com/twofasapp/data/services/domain/BackupContent.kt`)
//! and iOS's (https://github.com/twofas/2fas-ios,
//! `TwoFAS/Protection/ExchangeFileEncryption.swift`,
//! `TwoFAS/Data/MainRepository/DataTypes/ExchangeData2.swift`), which differ
//! only in the salt's length.
//!
//! A service's `name` is what 2FAS shows, so it is the issuer, as Aegis
//! takes it too. Its `otp.link` is not read: 2FAS may write it with the
//! secret hidden.

use authwing_totp::fields::{ACCOUNT, ALGORITHM, COUNTER, DIGITS, ISSUER, KIND, PERIOD, SECRET};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::crypto::{self, BASE64};
use crate::{Error, Format, Imported, Item, Result, damaged, label, otp_from};

/// PBKDF2's rounds, in both apps.
const ROUNDS: u32 = 10_000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct File {
    #[serde(default)]
    services: Vec<Value>,
    groups: Option<Vec<Group>>,
    services_encrypted: Option<String>,
    reference: Option<String>,
}

#[derive(Deserialize)]
struct Group {
    id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Service {
    #[serde(default)]
    name: String,
    #[serde(default)]
    secret: String,
    otp: Option<ServiceOtp>,
    group_id: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ServiceOtp {
    account: Option<String>,
    issuer: Option<String>,
    digits: Option<u32>,
    period: Option<u64>,
    algorithm: Option<String>,
    counter: Option<u64>,
    /// `TOTP` (also when missing), `HOTP` or `STEAM`.
    token_type: Option<String>,
}

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let file: File = serde_json::from_slice(bytes).map_err(damaged)?;
    let services = match file.services_encrypted.as_deref().map(str::trim) {
        Some(sealed) if !sealed.is_empty() => {
            let password = password.ok_or(Error::PasswordNeeded(Format::TwoFas))?;
            let plain = decrypt(sealed, file.reference.as_deref(), password)?;
            serde_json::from_slice(&plain).map_err(damaged)?
        }
        _ => file.services,
    };
    let groups = file.groups.unwrap_or_default();

    let mut imported = Imported::default();
    for (n, value) in services.into_iter().enumerate() {
        let text = |key| value.get(key).and_then(Value::as_str).unwrap_or_default();
        let what = label(text("name"), "", || format!("service {}", n + 1));
        let service: Service = match serde_json::from_value(value) {
            Ok(service) => service,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        let otp = service.otp.unwrap_or_default();
        let issuer = Some(service.name)
            .filter(|name| !name.trim().is_empty())
            .or(otp.issuer);
        let what = label(
            issuer.as_deref().unwrap_or_default(),
            otp.account.as_deref().unwrap_or_default(),
            || what,
        );
        let kind = otp
            .token_type
            .map_or("totp".to_string(), |kind| kind.to_ascii_lowercase());
        let code = otp_from(&[
            (ISSUER, issuer),
            (ACCOUNT, otp.account),
            (SECRET, Some(service.secret)),
            (KIND, Some(kind)),
            (ALGORITHM, otp.algorithm),
            (DIGITS, otp.digits.map(|d| d.to_string())),
            (PERIOD, otp.period.map(|p| p.to_string())),
            (COUNTER, otp.counter.map(|c| c.to_string())),
        ]);
        match code {
            Ok(code) => {
                let group = service
                    .group_id
                    .and_then(|id| groups.iter().find(|group| group.id == id))
                    .map(|group| group.name.clone());
                imported.items.push(Item {
                    otp: code,
                    group: group.unwrap_or_default(),
                    sites: Vec::new(),
                });
            }
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    Ok(imported)
}

/// The services JSON `sealed` holds, checked with `reference` first when
/// there is one.
fn decrypt(sealed: &str, reference: Option<&str>, password: &str) -> Result<Zeroizing<Vec<u8>>> {
    let services = Part::parse(sealed)?;
    let key = services.key(password);
    let reference = reference.map(str::trim).filter(|text| !text.is_empty());
    let Some(reference) = reference.map(Part::parse).transpose()? else {
        return services.open(&key).ok_or(Error::WrongPassword);
    };
    let opened = if reference.salt == services.salt {
        reference.open(&key)
    } else {
        reference.open(&reference.key(password))
    };
    if opened.is_none() {
        return Err(Error::WrongPassword);
    }
    services
        .open(&key)
        .ok_or_else(|| damaged("the codes do not decrypt"))
}

/// One sealed value: `ciphertext+tag:salt:iv`.
struct Part {
    sealed: Vec<u8>,
    salt: Vec<u8>,
    iv: Vec<u8>,
}

impl Part {
    fn parse(text: &str) -> Result<Part> {
        let mut parts = text.split(':').map(|part| {
            let part: String = part.chars().filter(|c| !c.is_whitespace()).collect();
            BASE64.decode(part).map_err(damaged)
        });
        let (Some(sealed), Some(salt), Some(iv)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(damaged("the encrypted codes are not in three parts"));
        };
        Ok(Part {
            sealed: sealed?,
            salt: salt?,
            iv: iv?,
        })
    }

    fn key(&self, password: &str) -> Zeroizing<[u8; 32]> {
        let mut key = Zeroizing::new([0u8; 32]);
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &self.salt, ROUNDS, &mut key[..]);
        key
    }

    fn open(&self, key: &[u8; 32]) -> Option<Zeroizing<Vec<u8>>> {
        crypto::aes_gcm_open(key, &self.iv, &self.sealed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use serde_json::json;

    const PLAIN_V4: &[u8] = include_bytes!("../testdata/2fas_authenticator_plain_v4.2fas");
    const ENCRYPTED_V4: &[u8] = include_bytes!("../testdata/2fas_authenticator_encrypted_v4.2fas");
    /// Schema 2, from 2FAS 3.8 for Android.
    const ENCRYPTED_V2: &[u8] = include_bytes!("../testdata/2fas_authenticator_encrypted.2fas");

    #[test]
    fn reads_a_plain_backup() {
        let imported = crate::import(PLAIN_V4, None).expect("import");
        assert_codes(
            &imported.items,
            &[URIS[0], URIS[3], URIS[4], URIS[5], URIS[6]],
        );
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn reads_encrypted_backups() {
        assert!(matches!(
            crate::import(ENCRYPTED_V4, None),
            Err(Error::PasswordNeeded(Format::TwoFas))
        ));
        let imported = crate::import(ENCRYPTED_V4, Some("test")).expect("v4");
        assert_codes(
            &imported.items,
            &[URIS[0], URIS[3], URIS[4], URIS[5], URIS[6]],
        );
        assert!(imported.skipped.is_empty());
        // Schema 2 had TOTP only: Aegis's sample has its HOTP codes as TOTP.
        let imported = crate::import(ENCRYPTED_V2, Some("test")).expect("v2");
        assert_codes(
            &imported.items,
            &[
                URIS[0],
                URIS[2],
                "otpauth://totp/Issuu:James?secret=YOOMIXWS5GN6RTBPUFFWKTW5M4&issuer=Issuu",
                "otpauth://totp/WWE:Mason?secret=5VAML3X35THCEBVRLV24CGBKOY&issuer=WWE\
                 &algorithm=SHA512&digits=8",
            ],
        );
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn wrong_password_is_told_from_damage() {
        assert!(matches!(
            crate::import(ENCRYPTED_V4, Some("tset")),
            Err(Error::WrongPassword)
        ));
        let mut file: Value = serde_json::from_slice(ENCRYPTED_V4).expect("json");
        let sealed = file["servicesEncrypted"]
            .as_str()
            .expect("sealed")
            .to_string();
        let (data, rest) = sealed.split_once(':').expect("parts");
        let mut data = BASE64.decode(data).expect("base64");
        data[3] ^= 1;
        file["servicesEncrypted"] = format!("{}:{rest}", BASE64.encode(data)).into();
        let damaged_file = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&damaged_file, Some("test")),
            Err(Error::Damaged(_))
        ));
        // Without the reference, a file that does not open is taken as
        // the wrong password: nothing tells the two apart.
        file["reference"] = Value::Null;
        let no_reference = serde_json::to_vec(&file).expect("json");
        assert!(matches!(
            crate::import(&no_reference, Some("test")),
            Err(Error::WrongPassword)
        ));
    }

    #[test]
    fn maps_groups_and_skips_what_it_cannot_use() {
        let service = |name: &str, otp: Value, group: Option<&str>| {
            json!({
                "name": name, "secret": "JBSWY3DPEHPK3PXP", "updatedAt": 0,
                "otp": otp, "order": {"position": 0}, "groupId": group,
            })
        };
        let file = json!({
            "schemaVersion": 4,
            "appOrigin": "ios",
            "groups": [{"id": "G1", "name": "Work", "isExpanded": true}],
            "services": [
                service("GitHub", json!({"account": "alice", "tokenType": "TOTP"}), Some("G1")),
                service("", json!({"issuer": "From otp", "account": "bob"}), Some("gone")),
                service("Yandex", json!({"account": "c", "tokenType": "YAOTP"}), None),
                service("Old", json!({"account": "d", "algorithm": "MD5"}), None),
                {"name": "No secret", "otp": {"account": "e"}},
                {"secret": 7},
            ],
        });
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
            [("GitHub", "alice", "Work"), ("From otp", "bob", "")]
        );
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(
            skipped,
            ["Yandex (c)", "Old (d)", "No secret (e)", "service 6"]
        );
        assert_eq!(imported.skipped[0].1, "unknown kind of code: yaotp");
    }
}
