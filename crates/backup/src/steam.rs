//! Steam Guard's keys as Steam Desktop Authenticator keeps them: a maFile,
//! JSON with an account's `shared_secret` (in base64), which Steam Guard's
//! codes are made from, beside its `account_name` and keys authwing has no
//! use for (`identity_secret` confirms trades, `revocation_code` removes
//! the authenticator). The Steam app's own file (`Steamguard-…`) holds the
//! same for each of its accounts, under `accounts`. The codes are named
//! `Steam` and the account.
//!
//! As Aegis reads them (https://github.com/beemdevelopment/Aegis,
//! `app/src/main/java/com/beemdevelopment/aegis/importers/SteamImporter.java`).
//! A maFile Steam Desktop Authenticator encrypted is not JSON, and is not
//! read: it needs the passkey and the salts in its `manifest.json`.

use authwing_totp::{Kind, Otp, Secret};
use base64::Engine;
use serde::Deserialize;
use serde_json::Value;

use crate::crypto::BASE64;
use crate::{Imported, Item, Result, damaged};

/// The issuer every code is named by.
const STEAM: &str = "Steam";

#[derive(Deserialize)]
struct Account {
    shared_secret: String,
    #[serde(default)]
    account_name: String,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let file: Value = serde_json::from_slice(bytes).map_err(damaged)?;
    let accounts = match file.get("accounts") {
        Some(Value::Object(accounts)) => accounts.values().cloned().collect(),
        Some(_) => return Err(damaged("its accounts are not a list of accounts")),
        None => vec![file],
    };

    let mut imported = Imported::default();
    for (n, value) in accounts.into_iter().enumerate() {
        let name = value.get("account_name").and_then(Value::as_str);
        let what = match name.map(str::trim) {
            Some(name) if !name.is_empty() => format!("{STEAM} ({name})"),
            _ => format!("{STEAM} account {}", n + 1),
        };
        let account: Account = match serde_json::from_value(value) {
            Ok(account) => account,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        match secret(&account.shared_secret) {
            Ok(secret) => imported.items.push(Item::new(Otp::new(
                STEAM,
                account.account_name.trim(),
                secret,
                Kind::Steam,
            ))),
            Err(why) => imported.skipped.push((what, why)),
        }
    }
    Ok(imported)
}

/// The key `shared_secret` holds in base64.
fn secret(text: &str) -> std::result::Result<Secret, String> {
    let bytes = BASE64
        .decode(text.trim())
        .map_err(|e| format!("the secret is not base64: {e}"))?;
    if bytes.is_empty() {
        return Err(authwing_totp::Error::NoSecret.to_string());
    }
    Ok(Secret::new(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use serde_json::json;

    /// Aegis's Steam code, named as a maFile names it.
    const STEAM_CODE: &str =
        "otpauth://steam/Steam:Sophia?secret=JRZCL47CMXVOQMNPZR2F7J4RGI&issuer=Steam";

    #[test]
    fn reads_both_kinds_of_file() {
        for bytes in [
            &include_bytes!("../testdata/steam.json")[..],
            include_bytes!("../testdata/steam_old.json"),
        ] {
            let imported = crate::import(bytes, None).expect("import");
            assert_codes(&imported.items, &[STEAM_CODE]);
            // Only the issuer differs from Aegis's code: a maFile has none.
            let steam = crate::testing::code(URIS[6]);
            assert_eq!(imported.items[0].otp.secret, steam.secret);
            assert!(imported.skipped.is_empty());
        }
    }

    #[test]
    fn skips_accounts_it_cannot_use() {
        let file = json!({"accounts": {
            "1": {"shared_secret": "THIl8+Jl6ugxr8x0X6eRMg==", "account_name": "sophia"},
            "2": {"shared_secret": "not base64!", "account_name": "mia"},
            "3": {"shared_secret": "", "account_name": "ava"},
            "4": {"account_name": "zoe"},
            "5": {"shared_secret": 7},
        }});
        let imported = import(&serde_json::to_vec(&file).expect("json")).expect("import");
        assert_eq!(imported.items.len(), 1);
        assert_eq!(imported.items[0].otp.account, "sophia");
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(
            skipped,
            [
                "Steam (mia)",
                "Steam (ava)",
                "Steam (zoe)",
                "Steam account 5"
            ]
        );
        assert_eq!(imported.skipped[1].1, "the secret is missing");
        let odd = json!({"accounts": ["THIl8+Jl6ugxr8x0X6eRMg=="]});
        assert!(matches!(
            import(&serde_json::to_vec(&odd).expect("json")),
            Err(crate::Error::Damaged(_))
        ));
    }
}
