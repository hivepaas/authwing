//! Raivo OTP's export (iOS): a ZIP sealed with WinZip's AES-256 under the
//! password given when exporting, holding `raivo-otp-export.json` beside an
//! HTML page of the same codes. The JSON is a list of the codes, every
//! value a string: `kind` (`TOTP` or `HOTP`), `issuer`, `account`,
//! `secret`, `algorithm`, `digits`, `timer` (the period) and `counter`
//! (https://github.com/raivo-otp/ios-application,
//! `Raivo/Features/DataExportFeature.swift` and `Raivo/Models/Password.swift`).
//! Its icons and pins are left behind.
//!
//! The entries' names are in the clear, so the file is known without its
//! password. AES checks the password before the codes are opened, and the
//! codes against what was sealed after: a password that fails the first
//! check is the wrong one, and codes that fail the second are damage (or,
//! once in 65,536, a wrong password that passed the first).

use authwing_totp::fields::{ACCOUNT, ALGORITHM, COUNTER, DIGITS, ISSUER, KIND, PERIOD, SECRET};
use serde_json::Value;

use crate::archive::Archive;
use crate::{Error, Format, Imported, Item, Result, damaged, label, otp_from};

/// The codes in the archive.
pub(crate) const DATA: &str = "raivo-otp-export.json";

pub(crate) fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let mut archive = Archive::open(bytes)?;
    if password.is_none() && archive.is_encrypted(DATA) {
        return Err(Error::PasswordNeeded(Format::Raivo));
    }
    let data = archive.read(DATA, password)?;
    let entries: Vec<Value> = serde_json::from_slice(&data).map_err(damaged)?;

    let mut imported = Imported::default();
    for (n, value) in entries.into_iter().enumerate() {
        // Raivo writes strings; numbers are taken as well.
        let text = |key: &str| match value.get(key) {
            Some(Value::String(text)) => Some(text.clone()),
            Some(Value::Number(number)) => Some(number.to_string()),
            _ => None,
        };
        let (issuer, account) = (text("issuer"), text("account"));
        let what = || {
            label(
                issuer.as_deref().unwrap_or_default(),
                account.as_deref().unwrap_or_default(),
                || format!("entry {}", n + 1),
            )
        };
        let otp = otp_from(&[
            (ISSUER, issuer.clone()),
            (ACCOUNT, account.clone()),
            (SECRET, text("secret")),
            (KIND, text("kind")),
            (ALGORITHM, text("algorithm")),
            (DIGITS, text("digits")),
            (PERIOD, text("timer")),
            (COUNTER, text("counter")),
        ]);
        match otp {
            Ok(otp) => imported.items.push(Item::new(otp)),
            Err(e) => imported.skipped.push((what(), e.to_string())),
        }
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    //! `raivo_otp.zip` is written from Raivo's export code (see the module
    //! docs), with the codes of Aegis's samples but Steam, which Raivo does
    //! not make (see `crate::tests`): its JSON and a page beside, sealed
    //! with 7-Zip as Raivo seals them, AES-256 and stored, under the
    //! password `test`:
    //! `7zz a -tzip -mem=AES256 -mx=0 -ptest raivo_otp.zip raivo-otp-export.json raivo-otp-export.html`.

    use super::*;
    use crate::testing::{URIS, assert_codes, zipped};

    const SAMPLE: &[u8] = include_bytes!("../testdata/raivo_otp.zip");

    #[test]
    fn reads_raivos_export_with_its_password() {
        assert_eq!(crate::detect(SAMPLE), Some(Format::Raivo));
        assert!(matches!(
            crate::import(SAMPLE, None),
            Err(Error::PasswordNeeded(Format::Raivo))
        ));
        assert!(matches!(
            crate::import(SAMPLE, Some("tset")),
            Err(Error::WrongPassword)
        ));
        let imported = crate::import(SAMPLE, Some("test")).expect("import");
        assert_codes(&imported.items, &URIS[..6]);
        assert!(imported.skipped.is_empty());
        assert!(imported.items.iter().all(|item| item.group.is_empty()));
    }

    #[test]
    fn skips_what_it_cannot_use_and_tells_damage() {
        let json = br#"[
            {"kind": "TOTP", "issuer": "GitHub", "account": "alice", "secret": "JBSWY3DPEHPK3PXP",
             "algorithm": "SHA1", "digits": 6, "timer": 60, "counter": "0"},
            {"kind": "TOTP", "issuer": "Bad", "account": "", "secret": "JBSWY3DPEHPK3PXP",
             "digits": "12"},
            {"kind": "MOTP", "secret": "JBSWY3DPEHPK3PXP"},
            7
        ]"#;
        let bytes = zipped(&[(DATA, json)], Some("pass"));
        let imported = crate::import(&bytes, Some("pass")).expect("import");
        assert_eq!(imported.items.len(), 1);
        assert_eq!(imported.items[0].otp.period(), Some(60));
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(skipped, ["Bad", "entry 3", "entry 4"]);

        let mut broken = SAMPLE.to_vec();
        // A byte of the sealed JSON: after its entry's name, the extra
        // fields and AES's salt take fewer than 200 bytes.
        let at = broken
            .windows(DATA.len())
            .position(|window| window == DATA.as_bytes())
            .expect("entry");
        broken[at + DATA.len() + 200] ^= 1;
        assert!(matches!(
            crate::import(&broken, Some("test")),
            Err(Error::Damaged(_))
        ));
    }
}
