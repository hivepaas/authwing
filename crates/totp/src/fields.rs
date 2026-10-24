//! An [`Otp`] as the fields of a vault record.
//!
//! The vault keeps a record as flat string fields and merges edits from
//! other devices field by field, so each setting is a field of its own
//! rather than one URI: using an HOTP code changes `counter` and nothing
//! else. Names and values are those of otpauth URI parameters (a base32
//! secret, `SHA1`, decimal numbers), plus `account` and `kind`, and are read
//! as leniently. A field that does not apply to the kind is left out, and a
//! missing one reads as its default, as the vault drops empty fields. Fields
//! this crate does not know are ignored, so the app can keep its own (a note,
//! an icon) in the same record.

use std::collections::BTreeMap;

use crate::{Kind, Otp, Result};

/// The site or service.
pub const ISSUER: &str = "issuer";
/// The account at the issuer.
pub const ACCOUNT: &str = "account";
/// The key, in base32 without padding.
pub const SECRET: &str = "secret";
/// `totp` (the default), `hotp` or `steam`.
pub const KIND: &str = "kind";
/// `SHA1` (the default), `SHA256` or `SHA512`; not for Steam.
pub const ALGORITHM: &str = "algorithm";
/// How many digits a code has, 6 by default; not for Steam.
pub const DIGITS: &str = "digits";
/// Seconds each TOTP code lasts, 30 by default.
pub const PERIOD: &str = "period";
/// The current HOTP code's counter, 0 by default.
pub const COUNTER: &str = "counter";

impl Otp {
    /// The record fields that describe this code.
    pub fn to_fields(&self) -> BTreeMap<String, String> {
        let mut fields = BTreeMap::new();
        let mut set = |name: &str, value: String| {
            fields.insert(name.to_string(), value);
        };
        set(ISSUER, self.issuer.clone());
        set(ACCOUNT, self.account.clone());
        set(SECRET, self.secret.to_base32());
        set(KIND, self.kind.name().to_string());
        if self.kind != Kind::Steam {
            set(ALGORITHM, self.algorithm.name().to_string());
            set(DIGITS, self.digits.to_string());
        }
        match self.kind {
            Kind::Totp { period } => set(PERIOD, period.to_string()),
            Kind::Hotp { counter } => set(COUNTER, counter.to_string()),
            Kind::Steam => {}
        }
        fields
    }

    /// Reads a code back from record fields; see the module docs.
    pub fn from_fields(fields: &BTreeMap<String, String>) -> Result<Otp> {
        let text = |name: &str| fields.get(name).map(String::as_str).unwrap_or_default();
        let setting = |name: &str| Some(text(name).trim()).filter(|value| !value.is_empty());
        Otp::read(
            text(ISSUER),
            text(ACCOUNT),
            setting(KIND).unwrap_or("totp"),
            setting,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Algorithm, Error, Secret};

    fn fields(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn secret() -> Secret {
        Secret::from_base32("JBSWY3DPEHPK3PXP").expect("secret")
    }

    #[test]
    fn round_trips_every_kind() {
        let mut totp = Otp::new("GitHub", "alice", secret(), Kind::Totp { period: 60 });
        totp.algorithm = Algorithm::Sha256;
        totp.digits = 8;
        let mut hotp = Otp::new("", "bob", secret(), Kind::Hotp { counter: 42 });
        hotp.algorithm = Algorithm::Sha512;
        let steam = Otp::new("Steam", "gaben", secret(), Kind::Steam);
        for otp in [totp, hotp, steam] {
            assert_eq!(Otp::from_fields(&otp.to_fields()), Ok(otp));
        }
    }

    #[test]
    fn writes_only_what_applies() {
        let totp = Otp::new("GitHub", "alice", secret(), Kind::Totp { period: 30 });
        let expected = fields(&[
            ("issuer", "GitHub"),
            ("account", "alice"),
            ("secret", "JBSWY3DPEHPK3PXP"),
            ("kind", "totp"),
            ("algorithm", "SHA1"),
            ("digits", "6"),
            ("period", "30"),
        ]);
        assert_eq!(totp.to_fields(), expected);
        let hotp = Otp::new("", "", secret(), Kind::Hotp { counter: 7 });
        assert_eq!(hotp.to_fields().get(COUNTER).map(String::as_str), Some("7"));
        assert!(!hotp.to_fields().contains_key(PERIOD));
        let steam = Otp::new("Steam", "gaben", secret(), Kind::Steam);
        let names: Vec<_> = steam.to_fields().into_keys().collect();
        assert_eq!(names, ["account", "issuer", "kind", "secret"]);
    }

    #[test]
    fn missing_fields_take_defaults_and_others_are_ignored() {
        let otp = Otp::from_fields(&fields(&[
            ("secret", "jbsw y3dp ehpk 3pxp"),
            ("note", "backup codes in the safe"),
            ("digits", ""),
        ]))
        .expect("minimal");
        assert_eq!(otp, Otp::new("", "", secret(), Kind::Totp { period: 30 }));
    }

    #[test]
    fn rejects_bad_values() {
        let read = |pairs: &[(&str, &str)]| Otp::from_fields(&fields(pairs));
        assert_eq!(read(&[("issuer", "x")]), Err(Error::NoSecret));
        let s = ("secret", "JBSWY3DPEHPK3PXP");
        assert_eq!(
            read(&[s, ("kind", "motp")]),
            Err(Error::Kind("motp".into()))
        );
        assert_eq!(read(&[s, ("digits", "4")]), Err(Error::Digits("4".into())));
        assert_eq!(read(&[s, ("period", "0")]), Err(Error::Period("0".into())));
        assert_eq!(
            read(&[s, ("kind", "hotp"), ("counter", "-1")]),
            Err(Error::Counter("-1".into()))
        );
        assert_eq!(
            read(&[s, ("algorithm", "MD5")]),
            Err(Error::Algorithm("MD5".into()))
        );
    }
}
