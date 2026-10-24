//! A code as a site hands it over: whose it is, its secret, and how codes
//! are made from it.

use std::ops::RangeInclusive;

use crate::code::{self, Algorithm, STEAM_LENGTH, STEAM_PERIOD};
use crate::fields::{ALGORITHM, COUNTER, DIGITS, PERIOD, SECRET};
use crate::{Error, Result, Secret};

/// How long a TOTP code lasts when nothing says otherwise.
pub const DEFAULT_PERIOD: u32 = 30;
/// How many digits a code has when nothing says otherwise.
pub const DEFAULT_DIGITS: u8 = 6;
/// The numbers of digits accepted. RFC 4226 allows 6 to 8; some apps make
/// codes of 5 or up to 10, and their codes import here too.
pub const DIGITS_RANGE: RangeInclusive<u8> = 5..=10;

/// How codes are made, and what changes from one code to the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A new code every `period` seconds (RFC 6238).
    Totp { period: u32 },
    /// A new code each time one is used (RFC 4226); `counter` is the
    /// current code's, and moves on when it is used.
    Hotp { counter: u64 },
    /// Steam Guard: a new code every 30 seconds from SHA-1, as five
    /// letters and digits.
    Steam,
}

impl Kind {
    /// The name vault records use: `totp`, `hotp` or `steam`.
    pub fn name(self) -> &'static str {
        match self {
            Kind::Totp { .. } => "totp",
            Kind::Hotp { .. } => "hotp",
            Kind::Steam => "steam",
        }
    }
}

/// One two-factor code: what an otpauth URI or a vault record describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Otp {
    /// The site or service ("GitHub"); may be empty.
    pub issuer: String,
    /// The account there, often an email address; may be empty.
    pub account: String,
    pub secret: Secret,
    pub kind: Kind,
    /// Steam codes use SHA-1 whatever this says.
    pub algorithm: Algorithm,
    /// Steam codes have five characters whatever this says.
    pub digits: u8,
}

impl Otp {
    /// A code with the settings most sites use: SHA-1 and six digits (five
    /// characters for Steam).
    pub fn new(
        issuer: impl Into<String>,
        account: impl Into<String>,
        secret: Secret,
        kind: Kind,
    ) -> Otp {
        Otp {
            issuer: issuer.into(),
            account: account.into(),
            secret,
            kind,
            algorithm: Algorithm::Sha1,
            digits: if kind == Kind::Steam {
                STEAM_LENGTH
            } else {
                DEFAULT_DIGITS
            },
        }
    }

    /// The code to show at `unix_seconds`. An HOTP code does not change with
    /// time: it is the code for its counter.
    pub fn code_at(&self, unix_seconds: u64) -> String {
        let key = self.secret.as_bytes();
        match self.kind {
            Kind::Totp { period } => {
                code::totp(key, unix_seconds, period, self.algorithm, self.digits)
            }
            Kind::Hotp { counter } => code::hotp(key, counter, self.algorithm, self.digits),
            Kind::Steam => code::steam(key, unix_seconds),
        }
    }

    /// The code after the one at `unix_seconds`, to show beside it in the
    /// last seconds of its period, when there is no time left to type it.
    /// `None` for HOTP, whose next code comes when this one is used.
    pub fn next_code_at(&self, unix_seconds: u64) -> Option<String> {
        let left = self.seconds_left(unix_seconds)?;
        Some(self.code_at(unix_seconds.saturating_add(u64::from(left))))
    }

    /// Seconds until the code at `unix_seconds` changes, from the period down
    /// to 1; `None` for HOTP.
    pub fn seconds_left(&self, unix_seconds: u64) -> Option<u32> {
        self.period()
            .map(|period| code::seconds_left(unix_seconds, period))
    }

    /// How long each code lasts, in seconds; `None` for HOTP.
    pub fn period(&self) -> Option<u32> {
        match self.kind {
            Kind::Totp { period } => Some(period),
            Kind::Hotp { .. } => None,
            Kind::Steam => Some(STEAM_PERIOD),
        }
    }

    /// Whether other apps would make the same codes from this: a secret, 5 to
    /// 10 digits and a period of at least a second. Everything this crate
    /// reads is checked; an `Otp` built by hand (from a form) should be too.
    pub fn check(&self) -> Result<()> {
        if self.secret.as_bytes().is_empty() {
            return Err(Error::NoSecret);
        }
        match self.kind {
            Kind::Steam => Ok(()),
            Kind::Totp { period: 0 } => Err(Error::Period("0".into())),
            _ if !DIGITS_RANGE.contains(&self.digits) => {
                Err(Error::Digits(self.digits.to_string()))
            }
            _ => Ok(()),
        }
    }

    /// Builds a code from text, as URIs and vault records give it: `setting`
    /// looks up a parameter or field by name. Settings that are missing take
    /// their defaults; Steam has none.
    pub(crate) fn read<'a>(
        issuer: &str,
        account: &str,
        kind: &str,
        setting: impl Fn(&str) -> Option<&'a str>,
    ) -> Result<Otp> {
        let kind = match kind.to_ascii_lowercase().as_str() {
            "totp" => Kind::Totp {
                period: setting(PERIOD)
                    .map(parse_period)
                    .transpose()?
                    .unwrap_or(DEFAULT_PERIOD),
            },
            "hotp" => Kind::Hotp {
                counter: setting(COUNTER)
                    .map(parse_counter)
                    .transpose()?
                    .unwrap_or(0),
            },
            "steam" => Kind::Steam,
            _ => return Err(Error::Kind(kind.to_string())),
        };
        let secret = Secret::from_base32(setting(SECRET).ok_or(Error::NoSecret)?)?;
        let mut otp = Otp::new(issuer, account, secret, kind);
        if kind != Kind::Steam {
            if let Some(name) = setting(ALGORITHM) {
                otp.algorithm = name.parse()?;
            }
            if let Some(digits) = setting(DIGITS) {
                otp.digits = parse_digits(digits)?;
            }
        }
        otp.check()?;
        Ok(otp)
    }
}

fn parse_digits(text: &str) -> Result<u8> {
    text.trim()
        .parse()
        .ok()
        .filter(|digits| DIGITS_RANGE.contains(digits))
        .ok_or_else(|| Error::Digits(text.trim().to_string()))
}

fn parse_period(text: &str) -> Result<u32> {
    text.trim()
        .parse()
        .ok()
        .filter(|period| *period > 0)
        .ok_or_else(|| Error::Period(text.trim().to_string()))
}

fn parse_counter(text: &str) -> Result<u64> {
    text.trim()
        .parse()
        .map_err(|_| Error::Counter(text.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rfc_seed() -> Secret {
        Secret::new(b"12345678901234567890".to_vec())
    }

    #[test]
    fn totp_codes_seconds_left_and_next() {
        let mut otp = Otp::new("Example", "alice", rfc_seed(), Kind::Totp { period: 30 });
        otp.digits = 8;
        assert_eq!(otp.code_at(59), "94287082");
        assert_eq!(otp.seconds_left(59), Some(1));
        assert_eq!(otp.seconds_left(60), Some(30));
        // 1111111109 is 1 s before the step of 1111111111 (RFC 6238).
        assert_eq!(otp.code_at(1111111109), "07081804");
        assert_eq!(otp.next_code_at(1111111109).as_deref(), Some("14050471"));
        assert_eq!(otp.period(), Some(30));
    }

    #[test]
    fn hotp_ignores_time() {
        let otp = Otp::new("", "", rfc_seed(), Kind::Hotp { counter: 3 });
        assert_eq!(otp.code_at(0), "969429");
        assert_eq!(otp.code_at(1234567890), "969429");
        assert_eq!(otp.seconds_left(0), None);
        assert_eq!(otp.next_code_at(0), None);
        assert_eq!(otp.period(), None);
    }

    #[test]
    fn steam_ignores_algorithm_and_digits() {
        let mut otp = Otp::new("Steam", "gaben", rfc_seed(), Kind::Steam);
        assert_eq!(otp.digits, 5);
        assert_eq!(otp.code_at(1234567890), "VHHQY");
        otp.algorithm = Algorithm::Sha512;
        otp.digits = 8;
        assert_eq!(otp.code_at(1234567890), "VHHQY");
        assert_eq!(otp.seconds_left(1234567890), Some(30));
        assert_eq!(otp.check(), Ok(()));
    }

    #[test]
    fn check_catches_what_parsers_reject() {
        let mut otp = Otp::new("", "", rfc_seed(), Kind::Totp { period: 0 });
        assert_eq!(otp.check(), Err(Error::Period("0".into())));
        otp.kind = Kind::Totp { period: 30 };
        otp.digits = 11;
        assert_eq!(otp.check(), Err(Error::Digits("11".into())));
        otp.digits = 6;
        otp.secret = Secret::new(Vec::new());
        assert_eq!(otp.check(), Err(Error::NoSecret));
    }

    #[test]
    fn debug_hides_the_secret() {
        let otp = Otp::new("Example", "alice", rfc_seed(), Kind::Steam);
        let shown = format!("{otp:?}");
        assert!(shown.contains("Secret(..)"), "{shown}");
        assert!(
            !shown.contains("GEZD") && !shown.contains("49, 50"),
            "{shown}"
        );
    }
}
