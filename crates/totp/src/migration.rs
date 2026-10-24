//! Google Authenticator's export: `otpauth-migration://offline?data=...`.
//!
//! The data is a protobuf message (`MigrationPayload`) in standard base64,
//! URL-encoded. It is read by hand: the message is small and fixed, and a
//! protobuf crate with its code generation would outweigh this module. An
//! export of many codes is spread over several QR codes, each a payload of
//! its own that says which one of how many it is.
//!
//! A code that cannot be imported (MD5, a kind or digit count Google added
//! later, a damaged entry) is listed as skipped, with the reason, rather than
//! failing the export: the rest are still worth having. Google Authenticator
//! has no Steam codes, so nothing here reads as one.

use base64::Engine;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use percent_encoding::percent_decode_str;
use zeroize::Zeroizing;

use crate::uri::{issuer_and_account, strip_prefix_ignore_case};
use crate::{Algorithm, DEFAULT_PERIOD, Error, Kind, Otp, Result, Secret};

const SCHEME: &str = "otpauth-migration://";

/// Standard base64, with or without its padding.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &base64::alphabet::STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// What one export QR code holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Migration {
    pub codes: Vec<Otp>,
    pub skipped: Vec<Skipped>,
    /// How many QR codes the export has (1 if it does not say).
    pub batch_size: i32,
    /// Which of them this is, from 0.
    pub batch_index: i32,
    /// The same in every QR code of one export, to tell exports apart.
    pub batch_id: i32,
}

/// A code in an export that cannot be imported, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub issuer: String,
    pub account: String,
    pub reason: Error,
}

impl Migration {
    /// Reads an `otpauth-migration://` URI, as scanned from the export's QR
    /// code. Only an export that cannot be read at all is an error.
    pub fn parse(uri: &str) -> Result<Migration> {
        let rest = strip_prefix_ignore_case(uri.trim(), SCHEME).ok_or(Error::NotMigration)?;
        let query = rest.split_once('?').map_or("", |(_, query)| query);
        let data = query
            .split('&')
            .find_map(|pair| {
                let (name, value) = pair.split_once('=')?;
                name.eq_ignore_ascii_case("data").then_some(value)
            })
            .ok_or(Error::Migration("no data"))?;
        // A `+` that something on the way read as a space is still a `+`.
        let data = percent_decode_str(data)
            .decode_utf8_lossy()
            .replace(' ', "+");
        let payload = Zeroizing::new(
            BASE64
                .decode(data)
                .map_err(|_| Error::Migration("not base64"))?,
        );
        read_payload(&payload)
    }
}

/// `MigrationPayload { repeated OtpParameters otp_parameters = 1; int32
/// version = 2; int32 batch_size = 3; int32 batch_index = 4; int32 batch_id
/// = 5; }`
fn read_payload(bytes: &[u8]) -> Result<Migration> {
    let mut migration = Migration {
        codes: Vec::new(),
        skipped: Vec::new(),
        batch_size: 1,
        batch_index: 0,
        batch_id: 0,
    };
    let mut fields = Reader(bytes);
    while let Some((number, value)) = fields.field()? {
        // int32 fields are varints of the value's 64-bit form, so the cast
        // brings back negative ones.
        match (number, value) {
            (1, Value::Bytes(entry)) => match read_entry(entry) {
                Ok(otp) => migration.codes.push(otp),
                Err(skipped) => migration.skipped.push(skipped),
            },
            (3, Value::Varint(n)) => migration.batch_size = n as i32,
            (4, Value::Varint(n)) => migration.batch_index = n as i32,
            (5, Value::Varint(n)) => migration.batch_id = n as i32,
            _ => {}
        }
    }
    Ok(migration)
}

/// One `OtpParameters` as read, its enums as numbers.
#[derive(Default)]
struct Entry<'a> {
    secret: &'a [u8],
    name: String,
    issuer: String,
    algorithm: u64,
    digits: u64,
    kind: u64,
    counter: u64,
}

/// `OtpParameters { bytes secret = 1; string name = 2; string issuer = 3;
/// Algorithm algorithm = 4; DigitCount digits = 5; OtpType type = 6; int64
/// counter = 7; }`, where the name is the label (`Issuer:account`).
fn read_entry(bytes: &[u8]) -> std::result::Result<Otp, Skipped> {
    let mut entry = Entry::default();
    let read = fill(&mut entry, bytes);
    let issuer = Some(entry.issuer.trim()).filter(|issuer| !issuer.is_empty());
    let (issuer, account) = issuer_and_account(&entry.name, issuer);
    let skipped = |reason| Skipped {
        issuer: issuer.to_string(),
        account: account.to_string(),
        reason,
    };
    read.map_err(skipped)?;
    // 0 is "unspecified" in each enum, which Google Authenticator reads as
    // its default.
    let algorithm = match entry.algorithm {
        0 | 1 => Algorithm::Sha1,
        2 => Algorithm::Sha256,
        3 => Algorithm::Sha512,
        4 => return Err(skipped(Error::Algorithm("MD5".into()))),
        n => return Err(skipped(Error::Algorithm(format!("unknown ({n})")))),
    };
    let digits = match entry.digits {
        0 | 1 => 6,
        2 => 8,
        n => return Err(skipped(Error::Digits(format!("unknown ({n})")))),
    };
    let kind = match entry.kind {
        0 | 2 => Kind::Totp {
            period: DEFAULT_PERIOD,
        },
        1 => Kind::Hotp {
            counter: entry.counter,
        },
        n => return Err(skipped(Error::Kind(format!("unknown ({n})")))),
    };
    if entry.secret.is_empty() {
        return Err(skipped(Error::NoSecret));
    }
    let secret = Secret::new(entry.secret.to_vec());
    let mut otp = Otp::new(issuer, account, secret, kind);
    otp.algorithm = algorithm;
    otp.digits = digits;
    Ok(otp)
}

fn fill<'a>(entry: &mut Entry<'a>, bytes: &'a [u8]) -> Result<()> {
    let text = |bytes| String::from_utf8_lossy(bytes).into_owned();
    let mut fields = Reader(bytes);
    while let Some((number, value)) = fields.field()? {
        match (number, value) {
            (1, Value::Bytes(secret)) => entry.secret = secret,
            (2, Value::Bytes(name)) => entry.name = text(name),
            (3, Value::Bytes(issuer)) => entry.issuer = text(issuer),
            (4, Value::Varint(n)) => entry.algorithm = n,
            (5, Value::Varint(n)) => entry.digits = n,
            (6, Value::Varint(n)) => entry.kind = n,
            (7, Value::Varint(n)) => entry.counter = n,
            _ => {}
        }
    }
    Ok(())
}

/// A field's value, as far as these messages need: fixed-size numbers are
/// skipped over.
enum Value<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
    Fixed,
}

/// Reads a protobuf message's fields in order.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    /// The next field's number and value, or `None` at the end.
    fn field(&mut self) -> Result<Option<(u64, Value<'a>)>> {
        if self.0.is_empty() {
            return Ok(None);
        }
        let key = self.varint()?;
        let value = match key & 7 {
            0 => Value::Varint(self.varint()?),
            1 => {
                self.take(8)?;
                Value::Fixed
            }
            2 => {
                let len = self.varint()?;
                Value::Bytes(self.take(usize::try_from(len).unwrap_or(usize::MAX))?)
            }
            5 => {
                self.take(4)?;
                Value::Fixed
            }
            _ => return Err(Error::Migration("unknown field type")),
        };
        Ok(Some((key >> 3, value)))
    }

    /// A base-128 number, low 7 bits first, the top bit of each byte saying
    /// whether another follows.
    fn varint(&mut self) -> Result<u64> {
        let mut value = 0;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(Error::Migration("number too long"))
    }

    fn byte(&mut self) -> Result<u8> {
        let (&byte, rest) = self.0.split_first().ok_or(Error::Migration("cut short"))?;
        self.0 = rest;
        Ok(byte)
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        if len > self.0.len() {
            return Err(Error::Migration("cut short"));
        }
        let (head, rest) = self.0.split_at(len);
        self.0 = rest;
        Ok(head)
    }
}

/// An export shown as several QR codes (Google Authenticator's, past a
/// few codes), put together as its codes are read, in any order.
#[derive(Debug, Default)]
pub struct Batches {
    id: i32,
    size: i32,
    read: std::collections::BTreeMap<i32, Vec<Otp>>,
    skipped: usize,
}

/// How far an export of several QR codes is read.
#[derive(Debug, PartialEq, Eq)]
pub enum Gathered {
    /// `read` of its `of` QR codes so far.
    Partly { read: usize, of: usize },
    /// All of them: their codes, in the export's order, and how many were
    /// left out (see [`Skipped`]).
    Whole { codes: Vec<Otp>, skipped: usize },
}

impl Batches {
    /// Takes one QR code's part: a part of another export starts over.
    pub fn add(&mut self, part: Migration) -> Gathered {
        if part.batch_id != self.id || part.batch_size != self.size {
            *self = Batches {
                id: part.batch_id,
                size: part.batch_size,
                ..Batches::default()
            };
        }
        if let std::collections::btree_map::Entry::Vacant(slot) = self.read.entry(part.batch_index)
        {
            self.skipped += part.skipped.len();
            slot.insert(part.codes);
        }
        let of = usize::try_from(self.size.max(1)).unwrap_or(1);
        if self.read.len() < of {
            return Gathered::Partly {
                read: self.read.len(),
                of,
            };
        }
        Gathered::Whole {
            codes: self.read.values().flatten().cloned().collect(),
            skipped: self.skipped,
        }
    }
}

#[cfg(test)]
mod tests {
    use base64::engine::general_purpose::STANDARD;
    use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};

    use super::*;

    /// Writes protobuf fields, to build exports to read.
    #[derive(Default)]
    struct Writer(Vec<u8>);

    impl Writer {
        fn varint(&mut self, mut n: u64) {
            while n >= 0x80 {
                self.0.push((n as u8) | 0x80);
                n >>= 7;
            }
            self.0.push(n as u8);
        }

        fn number(mut self, field: u64, n: u64) -> Self {
            self.varint(field << 3);
            self.varint(n);
            self
        }

        fn bytes(mut self, field: u64, bytes: &[u8]) -> Self {
            self.varint((field << 3) | 2);
            self.varint(bytes.len() as u64);
            self.0.extend_from_slice(bytes);
            self
        }
    }

    fn uri(payload: &[u8]) -> String {
        let data = STANDARD.encode(payload);
        format!(
            "otpauth-migration://offline?data={}",
            utf8_percent_encode(&data, NON_ALPHANUMERIC)
        )
    }

    fn rfc_seed() -> Secret {
        Secret::new(b"12345678901234567890".to_vec())
    }

    #[test]
    fn reads_a_single_code() {
        // One TOTP code, as Google Authenticator writes it: secret
        // "Hello!\xde\xad\xbe\xef", name "Example:alice@google.com", issuer
        // "Example", type TOTP, the rest unspecified.
        let migration = Migration::parse(
            "otpauth-migration://offline?data=CjEKCkhlbGxvId6tvu8SGEV4YW1wbGU6YWxpY2VAZ29vZ2xl\
             LmNvbRoHRXhhbXBsZTAC",
        )
        .expect("export");
        let secret = Secret::from_base32("JBSWY3DPEHPK3PXP").expect("secret");
        let expected = Otp::new(
            "Example",
            "alice@google.com",
            secret,
            Kind::Totp { period: 30 },
        );
        assert_eq!(migration.codes, [expected]);
        assert!(migration.skipped.is_empty());
        assert_eq!((migration.batch_size, migration.batch_index), (1, 0));
    }

    #[test]
    fn reads_a_batch_and_skips_md5() {
        let totp = Writer::default()
            .bytes(1, b"12345678901234567890")
            .bytes(2, b"GitHub:alice")
            .bytes(3, b"GitHub")
            .number(4, 2)
            .number(5, 2)
            .number(6, 2);
        let hotp = Writer::default()
            .bytes(1, b"12345678901234567890")
            .bytes(2, b"bob@example.com")
            .number(6, 1)
            .number(7, 5);
        let md5 = Writer::default()
            .bytes(1, b"12345678901234567890")
            .bytes(2, b"Old Bank:carol")
            .number(4, 4)
            .number(6, 2);
        let payload = Writer::default()
            .bytes(1, &totp.0)
            .bytes(1, &md5.0)
            .bytes(1, &hotp.0)
            .number(2, 1)
            .number(3, 3)
            .number(4, 1)
            .number(5, (-123456789i64) as u64);
        let migration = Migration::parse(&uri(&payload.0)).expect("export");

        let mut github = Otp::new("GitHub", "alice", rfc_seed(), Kind::Totp { period: 30 });
        github.algorithm = Algorithm::Sha256;
        github.digits = 8;
        let bob = Otp::new("", "bob@example.com", rfc_seed(), Kind::Hotp { counter: 5 });
        assert_eq!(migration.codes, [github, bob]);
        assert_eq!(
            migration.skipped,
            [Skipped {
                issuer: "Old Bank".into(),
                account: "carol".into(),
                reason: Error::Algorithm("MD5".into()),
            }]
        );
        assert_eq!(migration.batch_size, 3);
        assert_eq!(migration.batch_index, 1);
        assert_eq!(migration.batch_id, -123456789);
        assert_eq!(
            migration.codes[1].code_at(0),
            "254676",
            "RFC 4226 counter 5"
        );
    }

    #[test]
    fn skips_damaged_and_unknown_entries() {
        let unknown_kind = Writer::default()
            .bytes(1, b"key")
            .bytes(2, b"x")
            .number(6, 9);
        let no_secret = Writer::default().bytes(2, b"y");
        // A secret said to be 20 bytes long, of which 3 follow.
        let mut damaged = Writer::default().bytes(2, b"z").0;
        damaged.extend_from_slice(&[0x0a, 20, 1, 2, 3]);
        let payload = Writer::default()
            .bytes(1, &unknown_kind.0)
            .bytes(1, &no_secret.0)
            .bytes(1, &damaged);
        let migration = Migration::parse(&uri(&payload.0)).expect("export");
        let reasons: Vec<_> = migration
            .skipped
            .iter()
            .map(|s| (s.account.as_str(), s.reason.clone()))
            .collect();
        assert_eq!(
            reasons,
            [
                ("x", Error::Kind("unknown (9)".into())),
                ("y", Error::NoSecret),
                ("z", Error::Migration("cut short")),
            ]
        );
        assert!(migration.codes.is_empty());
    }

    #[test]
    fn forgives_unescaped_data_and_missing_padding() {
        // Raw base64 with its `+` turned into a space and no padding.
        let payload = Writer::default().bytes(
            1,
            &Writer::default()
                .bytes(1, &[0xf8, 0xf8, 0xf8])
                .bytes(2, b"a")
                .0,
        );
        let data = STANDARD.encode(&payload.0);
        assert!(data.contains('+') && data.ends_with('='), "{data}");
        let sloppy = data.trim_end_matches('=').replace('+', " ");
        let migration = Migration::parse(&format!("OTPAUTH-MIGRATION://offline?data={sloppy}"))
            .expect("sloppy");
        assert_eq!(migration.codes.len(), 1);
        assert_eq!(migration.codes[0].secret.as_bytes(), [0xf8, 0xf8, 0xf8]);
    }

    #[test]
    fn rejects_what_is_not_an_export() {
        let parse = |uri: &str| Migration::parse(uri).expect_err(uri);
        assert_eq!(
            parse("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP"),
            Error::NotMigration
        );
        assert_eq!(
            parse("otpauth-migration://offline"),
            Error::Migration("no data")
        );
        assert_eq!(
            parse("otpauth-migration://offline?data=!!!!"),
            Error::Migration("not base64")
        );
        // A field that says 50 bytes follow, then ends.
        assert_eq!(parse(&uri(&[0x0a, 50, 1])), Error::Migration("cut short"));
        assert_eq!(
            parse(&uri(&[
                0x08, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01
            ])),
            Error::Migration("number too long")
        );
    }

    #[test]
    fn an_export_of_several_codes_is_put_together_in_any_order() {
        let part = |id: i32, index: i32, issuer: &str| Migration {
            codes: vec![Otp::new(
                issuer,
                "me",
                crate::Secret::from_base32("JBSWY3DPEHPK3PXP").expect("secret"),
                crate::Kind::Totp { period: 30 },
            )],
            skipped: Vec::new(),
            batch_size: 3,
            batch_index: index,
            batch_id: id,
        };
        let mut batches = Batches::default();
        assert_eq!(
            batches.add(part(7, 1, "B")),
            Gathered::Partly { read: 1, of: 3 }
        );
        // The same part again counts once.
        assert_eq!(
            batches.add(part(7, 1, "B")),
            Gathered::Partly { read: 1, of: 3 }
        );
        // A part of another export starts over.
        assert_eq!(
            batches.add(part(8, 0, "X")),
            Gathered::Partly { read: 1, of: 3 }
        );
        assert_eq!(
            batches.add(part(7, 2, "C")),
            Gathered::Partly { read: 1, of: 3 }
        );
        batches.add(part(7, 0, "A"));
        let Gathered::Whole { codes, skipped } = batches.add(part(7, 1, "B")) else {
            panic!("not whole");
        };
        let issuers: Vec<&str> = codes.iter().map(|c| c.issuer.as_str()).collect();
        assert_eq!((issuers, skipped), (vec!["A", "B", "C"], 0));
    }
}
