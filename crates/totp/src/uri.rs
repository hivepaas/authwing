//! `otpauth://` URIs (Google's Key Uri Format), what QR codes carry.
//!
//! The format was never standardised, and the QR codes in the wild show it:
//! lower-case algorithm names, no issuer, spaces in the secret, everything in
//! upper case (QR codes' compact alphanumeric mode). Reading forgives all of
//! that and fills in the defaults (SHA-1, six digits, 30 seconds, counter 0),
//! but a secret that is missing or not base32 is an error, since it could
//! only give wrong codes. Parameters it does not know (`image`) are ignored.
//!
//! The issuer can be in the label (`Issuer:account`) and in the `issuer`
//! parameter. When both are there and differ, the parameter wins, as the
//! format recommends and Google Authenticator does; the label's prefix is
//! still taken off the account.
//!
//! Steam Guard has no type in the format. Steam codes come as `totp` URIs
//! with the issuer Steam or with `encoder=steam`, and some apps write
//! `otpauth://steam/`: all three read as [`Kind::Steam`]. Writing adds
//! `encoder=steam`, which still says Steam if the issuer is renamed.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};

use crate::{Error, Kind, Otp, Result};

const SCHEME: &str = "otpauth://";

/// What is escaped in a label or a parameter: all but letters, digits,
/// `-._~` and `@`, which keeps email addresses readable.
const ESCAPED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'@');

impl Otp {
    /// Reads an `otpauth://` URI; see the module docs for what is forgiven.
    pub fn from_uri(uri: &str) -> Result<Otp> {
        let rest = strip_prefix_ignore_case(uri.trim(), SCHEME).ok_or(Error::NotOtpauth)?;
        let rest = rest.split_once('#').map_or(rest, |(rest, _)| rest);
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (kind, label) = path.split_once('/').unwrap_or((path, ""));
        let params = params(query);
        let param = |name: &str| {
            params
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, value)| value.as_str())
        };

        let label = percent_decode_str(label).decode_utf8_lossy();
        let (issuer, account) = issuer_and_account(&label, param("issuer").map(str::trim));
        let steam = kind.eq_ignore_ascii_case("steam")
            || kind.eq_ignore_ascii_case("totp")
                && (param("encoder").is_some_and(|e| e.trim().eq_ignore_ascii_case("steam"))
                    || issuer.eq_ignore_ascii_case("steam"));
        let kind = if steam { "steam" } else { kind };
        Otp::read(issuer, account, kind, param)
    }

    /// The `otpauth://` URI for this code, to show as a QR code or to move it
    /// to another app. Every setting is written, defaults too.
    pub fn to_uri(&self) -> String {
        let issuer = utf8_percent_encode(&self.issuer, ESCAPED).to_string();
        let account = utf8_percent_encode(&self.account, ESCAPED);
        let kind = match self.kind {
            Kind::Hotp { .. } => "hotp",
            Kind::Totp { .. } | Kind::Steam => "totp",
        };
        let mut uri = format!("{SCHEME}{kind}/");
        if !issuer.is_empty() {
            uri.push_str(&format!("{issuer}:"));
        }
        uri.push_str(&format!("{account}?secret={}", self.secret.to_base32()));
        if !issuer.is_empty() {
            uri.push_str(&format!("&issuer={issuer}"));
        }
        if self.kind != Kind::Steam {
            uri.push_str(&format!(
                "&algorithm={}&digits={}",
                self.algorithm, self.digits
            ));
        }
        uri.push_str(&match self.kind {
            Kind::Totp { period } => format!("&period={period}"),
            Kind::Hotp { counter } => format!("&counter={counter}"),
            Kind::Steam => "&encoder=steam".to_string(),
        });
        uri
    }
}

/// The query's parameters: names in lower case, values decoded with `+` as
/// a space (as Android reads them). Empty values are left out, as apps write
/// `issuer=` for no issuer.
fn params(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let value = percent_decode_str(&value.replace('+', " "))
                .decode_utf8_lossy()
                .into_owned();
            (!value.trim().is_empty()).then(|| (name.to_ascii_lowercase(), value))
        })
        .collect()
}

/// The issuer and account a label names (`Issuer:account`, with optional
/// spaces after the colon, or just an account), given the issuer stated
/// apart from it, which wins when both are there.
pub(crate) fn issuer_and_account<'a>(
    label: &'a str,
    issuer: Option<&'a str>,
) -> (&'a str, &'a str) {
    let label = label.trim();
    if let Some(issuer) = issuer {
        // An issuer can hold a colon itself: the one after it splits.
        let rest = label
            .strip_prefix(issuer)
            .and_then(|rest| rest.strip_prefix(':'));
        if let Some(account) = rest {
            return (issuer, account.trim());
        }
    }
    let (label_issuer, account) = match label.split_once(':') {
        Some((label_issuer, account)) => (label_issuer.trim(), account.trim()),
        None => ("", label),
    };
    (issuer.unwrap_or(label_issuer), account)
}

/// `text` without `prefix`, matched in any case.
pub(crate) fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Algorithm, Secret};

    const SECRET: &str = "JBSWY3DPEHPK3PXP";

    fn secret() -> Secret {
        Secret::from_base32(SECRET).expect("secret")
    }

    fn read(uri: &str) -> Otp {
        Otp::from_uri(uri).expect(uri)
    }

    #[test]
    fn reads_google_example() {
        let otp =
            read("otpauth://totp/Example:alice@google.com?secret=JBSWY3DPEHPK3PXP&issuer=Example");
        assert_eq!(
            otp,
            Otp::new(
                "Example",
                "alice@google.com",
                secret(),
                Kind::Totp { period: 30 }
            )
        );
    }

    #[test]
    fn round_trips() {
        let mut totp = Otp::new(
            "ACME Co: R&D",
            "john.doe+2fa@example.com",
            secret(),
            Kind::Totp { period: 60 },
        );
        totp.algorithm = Algorithm::Sha512;
        totp.digits = 8;
        let mut hotp = Otp::new("", "bob", secret(), Kind::Hotp { counter: 12 });
        hotp.algorithm = Algorithm::Sha256;
        let steam = Otp::new("Steam", "gaben", secret(), Kind::Steam);
        let renamed_steam = Otp::new("My games", "gaben", secret(), Kind::Steam);
        let unicode = Otp::new("Ünïcødé", "名前", secret(), Kind::Totp { period: 30 });
        for otp in [totp, hotp, steam, renamed_steam, unicode] {
            let uri = otp.to_uri();
            assert_eq!(Otp::from_uri(&uri), Ok(otp), "{uri}");
        }
    }

    #[test]
    fn writes_escaped_labels() {
        let otp = Otp::new("ACME Co", "a:b@x.org", secret(), Kind::Totp { period: 30 });
        assert_eq!(
            otp.to_uri(),
            "otpauth://totp/ACME%20Co:a%3Ab@x.org?secret=JBSWY3DPEHPK3PXP\
             &issuer=ACME%20Co&algorithm=SHA1&digits=6&period=30"
        );
        let steam = Otp::new("Steam", "gaben", secret(), Kind::Steam);
        assert_eq!(
            steam.to_uri(),
            "otpauth://totp/Steam:gaben?secret=JBSWY3DPEHPK3PXP&issuer=Steam&encoder=steam"
        );
    }

    #[test]
    fn forgives_what_qr_codes_do() {
        // No issuer anywhere, lower-case algorithm, spaces in the secret.
        let otp = read("otpauth://totp/alice?secret=jbsw%20y3dp+ehpk-3pxp&algorithm=sha256");
        assert_eq!((otp.issuer.as_str(), otp.account.as_str()), ("", "alice"));
        assert_eq!(otp.algorithm, Algorithm::Sha256);
        assert_eq!(otp.secret, secret());
        // Upper case throughout, as QR alphanumeric mode has it.
        let otp = read("OTPAUTH://TOTP/ACME:BOB?SECRET=JBSWY3DPEHPK3PXP&DIGITS=8&PERIOD=60");
        assert_eq!((otp.issuer.as_str(), otp.account.as_str()), ("ACME", "BOB"));
        assert_eq!((otp.digits, otp.kind), (8, Kind::Totp { period: 60 }));
        // An encoded colon, a space after it, padding, empty and unknown
        // parameters, a fragment.
        let otp = read(
            "otpauth://totp/Big%20Corp%3A%20jo?secret=JBSWY3DPEHPK3PXP====&issuer=\
             &image=https%3A%2F%2Fx.org%2Fa.png#top",
        );
        assert_eq!(
            (otp.issuer.as_str(), otp.account.as_str()),
            ("Big Corp", "jo")
        );
        // `+` in a parameter is a space; no label at all.
        let otp = read("otpauth://totp?secret=JBSWY3DPEHPK3PXP&issuer=Big+Corp");
        assert_eq!(
            (otp.issuer.as_str(), otp.account.as_str()),
            ("Big Corp", "")
        );
        // HOTP without a counter starts at 0.
        assert_eq!(
            read("otpauth://hotp/x?secret=JBSWY3DPEHPK3PXP").kind,
            Kind::Hotp { counter: 0 }
        );
    }

    #[test]
    fn issuer_parameter_wins_over_label() {
        let otp = read("otpauth://totp/Old%20Name:alice?secret=JBSWY3DPEHPK3PXP&issuer=New");
        assert_eq!(
            (otp.issuer.as_str(), otp.account.as_str()),
            ("New", "alice")
        );
    }

    #[test]
    fn recognises_steam() {
        let by_issuer = read("otpauth://totp/Steam:gaben?secret=JBSWY3DPEHPK3PXP&issuer=Steam");
        let by_label = read("otpauth://totp/Steam:gaben?secret=JBSWY3DPEHPK3PXP");
        let by_encoder =
            read("otpauth://totp/Valve:gaben?secret=JBSWY3DPEHPK3PXP&encoder=steam&digits=5");
        let by_type = read("otpauth://steam/Steam:gaben?secret=JBSWY3DPEHPK3PXP&issuer=Steam");
        for otp in [&by_issuer, &by_label, &by_encoder, &by_type] {
            assert_eq!(otp.kind, Kind::Steam, "{otp:?}");
            assert_eq!(otp.account, "gaben");
        }
        assert_eq!(by_encoder.issuer, "Valve");
        // An HOTP code stays one whatever its issuer.
        let hotp = read("otpauth://hotp/Steam:x?secret=JBSWY3DPEHPK3PXP&counter=1");
        assert_eq!(hotp.kind, Kind::Hotp { counter: 1 });
    }

    #[test]
    fn rejects() {
        let error = |uri: &str| Otp::from_uri(uri).expect_err(uri);
        assert_eq!(error("otpauth://totp/x?issuer=A"), Error::NoSecret);
        assert_eq!(error("otpauth://totp/x?secret="), Error::NoSecret);
        assert_eq!(
            error("otpauth://totp/x?secret=JBSW1Y3DP"),
            Error::Base32Char('1')
        );
        assert_eq!(
            error("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&algorithm=MD5"),
            Error::Algorithm("MD5".into())
        );
        assert_eq!(
            error("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&digits=12"),
            Error::Digits("12".into())
        );
        assert_eq!(
            error("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP&period=soon"),
            Error::Period("soon".into())
        );
        assert_eq!(
            error("otpauth://motp/x?secret=JBSWY3DPEHPK3PXP"),
            Error::Kind("motp".into())
        );
        assert_eq!(
            error("https://example.com/?secret=JBSWY3DPEHPK3PXP"),
            Error::NotOtpauth
        );
        assert_eq!(
            error("otpauth-migration://offline?data=CgA"),
            Error::NotOtpauth
        );
    }
}
