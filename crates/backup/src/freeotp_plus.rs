//! FreeOTP+'s JSON export: its codes (`tokens`), each a token of FreeOTP's
//! own `Token` class as Gson writes it, beside the order they are shown in
//! (`tokenOrder`), which is not needed here. The secret is a list of bytes,
//! signed as Java's are.
//!
//! The issuer is the one the user gave (`issuerAlt`), else the one in the
//! code's label (`issuerExt`), else its `issuer` parameter (`issuerInt`);
//! the account is the one the user gave (`labelAlt`), else `label`: what
//! FreeOTP+ shows. A TOTP code whose label names `Steam` as its issuer
//! makes Steam Guard codes, as in FreeOTP+. A token's `category` is its
//! group.
//!
//! An HOTP code's `counter` is taken as it is, as Aegis takes it: FreeOTP+
//! takes one off the counter of a code it scans (and adds it back to the
//! `otpauth://` URIs it exports), and makes its next code from the counter
//! it keeps, so authwing shows the code FreeOTP+ would have shown next. A
//! code scanned at 0 and never used is kept at −1, and comes in at 0.
//!
//! As Aegis reads it (https://github.com/beemdevelopment/Aegis,
//! `app/src/main/java/com/beemdevelopment/aegis/importers/FreeOtpPlusImporter.java`
//! and `FreeOtpImporter.java`) and FreeOTP+ writes it
//! (https://github.com/helloworld1/FreeOTPPlus,
//! `token-data/src/main/java/org/fedorahosted/freeotp/data/legacy/Token.java`
//! and `ImportExportUtil.kt`).

use authwing_totp::Secret;
use authwing_totp::fields::{ACCOUNT, ALGORITHM, COUNTER, DIGITS, ISSUER, KIND, PERIOD, SECRET};
use serde::Deserialize;
use serde_json::Value;

use crate::{Imported, Item, Result, damaged, label, otp_from};

/// The issuer FreeOTP+ makes Steam Guard codes for.
const STEAM: &str = "Steam";

#[derive(Deserialize)]
struct File {
    tokens: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    /// `TOTP` or `HOTP`.
    #[serde(rename = "type")]
    kind: String,
    secret: Vec<i64>,
    issuer_alt: Option<String>,
    issuer_ext: Option<String>,
    issuer_int: Option<String>,
    label: Option<String>,
    label_alt: Option<String>,
    algo: Option<String>,
    digits: Option<u32>,
    period: Option<u64>,
    counter: Option<i64>,
    category: Option<String>,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let file: File = serde_json::from_slice(bytes).map_err(damaged)?;

    let mut imported = Imported::default();
    for (n, value) in file.tokens.into_iter().enumerate() {
        let token_no = || format!("token {}", n + 1);
        let text = |key| value.get(key).and_then(Value::as_str).unwrap_or_default();
        let what = label(text("issuerExt"), text("label"), token_no);
        let token: Token = match serde_json::from_value(value) {
            Ok(token) => token,
            Err(e) => {
                imported.skipped.push((what, e.to_string()));
                continue;
            }
        };
        let issuer = [&token.issuer_alt, &token.issuer_ext, &token.issuer_int]
            .into_iter()
            .flatten()
            .find(|issuer| !issuer.trim().is_empty())
            .cloned()
            .unwrap_or_default();
        let account = token.label_alt.or(token.label).unwrap_or_default();
        let what = label(&issuer, &account, token_no);
        let Some(secret) = bytes_of(&token.secret) else {
            imported
                .skipped
                .push((what, "the secret is not a list of bytes".into()));
            continue;
        };
        let mut kind = token.kind.to_ascii_lowercase();
        if kind == "totp" && token.issuer_ext.as_deref() == Some(STEAM) {
            kind = "steam".into();
        }
        let otp = otp_from(&[
            (ISSUER, Some(issuer)),
            (ACCOUNT, Some(account)),
            (SECRET, Some(Secret::new(secret).to_base32())),
            (KIND, Some(kind)),
            (ALGORITHM, token.algo),
            (DIGITS, token.digits.map(|d| d.to_string())),
            (PERIOD, token.period.map(|p| p.to_string())),
            (COUNTER, token.counter.map(|c| c.max(0).to_string())),
        ]);
        match otp {
            Ok(otp) => imported.items.push(Item {
                otp,
                group: token.category.unwrap_or_default().trim().to_string(),
                sites: Vec::new(),
            }),
            Err(e) => imported.skipped.push((what, e.to_string())),
        }
    }
    Ok(imported)
}

/// The bytes Java's signed bytes (or unsigned ones) stand for.
fn bytes_of(numbers: &[i64]) -> Option<Vec<u8>> {
    numbers
        .iter()
        .map(|&n| (-128..=255).contains(&n).then_some(n as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{URIS, assert_codes};
    use authwing_totp::Kind;
    use serde_json::json;

    #[test]
    fn reads_freeotp_plus_export() {
        let bytes = include_bytes!("../testdata/freeotp_plus.json");
        let imported = crate::import(bytes, None).expect("import");
        // No Steam code; HOTP counters one less, as FreeOTP+ keeps them.
        assert_codes(
            &imported.items,
            &[
                URIS[0],
                URIS[1],
                URIS[2],
                "otpauth://hotp/Issuu:James?secret=YOOMIXWS5GN6RTBPUFFWKTW5M4&issuer=Issuu\
                 &algorithm=SHA1&digits=6&counter=0",
                "otpauth://hotp/Air%20Canada:Benjamin?secret=KUVJJOM753IHTNDSZVCNKL7GII\
                 &issuer=Air+Canada&algorithm=SHA256&digits=7&counter=49",
                "otpauth://hotp/WWE:Mason?secret=5VAML3X35THCEBVRLV24CGBKOY&issuer=WWE\
                 &algorithm=SHA512&digits=8&counter=10299",
            ],
        );
        assert!(imported.skipped.is_empty());
    }

    #[test]
    fn names_steam_categories_and_skips_what_it_cannot_use() {
        // JBSWY3DPEHPK3PXP, as Java's signed bytes.
        let secret = json!([72, 101, 108, 108, 111, 33, -34, -83, -66, -17]);
        let token = |extra: Value| {
            let mut token = json!({
                "type": "TOTP", "secret": secret, "algo": "SHA1", "digits": 6, "period": 30,
                "counter": 0, "issuerExt": "GitHub", "issuerInt": "GitHub", "label": "alice",
            });
            if let (Some(token), Some(extra)) = (token.as_object_mut(), extra.as_object()) {
                token.extend(extra.clone());
            }
            token
        };
        let file = json!({
            "tokenOrder": [],
            "tokens": [
                token(json!({"issuerAlt": "Work GitHub", "labelAlt": "al", "category": "Work"})),
                token(json!({"issuerExt": "Steam", "issuerInt": "Steam", "digits": 5})),
                token(json!({"type": "HOTP", "issuerExt": "", "issuerInt": "Bank",
                    "counter": -1})),
                token(json!({"algo": "MD5"})),
                token(json!({"secret": [1, 2, 300]})),
                {"type": "TOTP"},
            ],
        });
        let imported = import(&serde_json::to_vec(&file).expect("json")).expect("import");
        let items: Vec<(&str, &str, Kind, &str)> = imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                (
                    otp.issuer.as_str(),
                    otp.account.as_str(),
                    otp.kind,
                    item.group.as_str(),
                )
            })
            .collect();
        assert_eq!(
            items,
            [
                ("Work GitHub", "al", Kind::Totp { period: 30 }, "Work"),
                ("Steam", "alice", Kind::Steam, ""),
                ("Bank", "alice", Kind::Hotp { counter: 0 }, ""),
            ]
        );
        assert_eq!(imported.items[0].otp.secret.to_base32(), "JBSWY3DPEHPK3PXP");
        let skipped: Vec<(&str, &str)> = imported
            .skipped
            .iter()
            .map(|(what, why)| (what.as_str(), &why[..why.find(':').unwrap_or(why.len())]))
            .collect();
        assert_eq!(
            skipped,
            [
                ("GitHub (alice)", "unsupported algorithm"),
                ("GitHub (alice)", "the secret is not a list of bytes"),
                ("token 6", "missing field `secret`"),
            ]
        );
    }
}
