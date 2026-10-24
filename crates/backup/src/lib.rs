//! Codes in and out of authwing: its own backup, a plain export, and the
//! exports of other authenticator apps.
//!
//! Bytes in, bytes out: no files, no UI and no vault. The app reads a file
//! and hands its bytes over, or saves the bytes that come back.
//!
//! - authwing's backup ([`seal`], [`open`]) is JSON that says how it was
//!   sealed, around the codes sealed with a key from a passphrase, as the
//!   vault keeps its own key. Inside, each code is its `otpauth://` URI
//!   beside its group and sites, so once decrypted any app can read them.
//! - The plain export ([`plain`]) is one URI per line: what nearly every app
//!   imports, and what anyone who sees the file can use.
//! - [`import`] reads authwing's backup, the exports of Aegis, 2FAS,
//!   andOTP, Stratum (Authenticator Pro), TOTP Authenticator and Bitwarden,
//!   Ente Auth's plain-text export and lists of URIs (with Google
//!   Authenticator's `otpauth-migration://` among them); [`detect`] says
//!   which a file is. A code authwing cannot use (MD5, mOTP, no secret) is
//!   listed in [`Imported::skipped`] with the reason, and the rest come in:
//!   one bad entry does not cost the user the others.
//! - Encrypted backups that do not say what they are, andOTP's and TOTP
//!   Authenticator's, are [`Format::EncryptedFile`]: the password that opens
//!   one tells which app wrote it.

mod aegis;
mod andotp;
mod authwing;
mod bitwarden;
mod crypto;
mod stratum;
mod totp_authenticator;
mod twofas;
mod uris;

use std::fmt;

use authwing_totp::Otp;
use serde_json::Value;

pub use authwing::{EXTENSION, MIN_PASSPHRASE, open, seal};
pub use uris::plain;

/// A code as the app keeps it: the code itself, what it is filed under and
/// the sites it is shown first on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub otp: Otp,
    /// Empty: none.
    pub group: String,
    /// Hosts (`github.com`).
    pub sites: Vec<String>,
}

impl Item {
    /// A code with no group and no sites.
    pub fn new(otp: Otp) -> Item {
        Item {
            otp,
            group: String::new(),
            sites: Vec::new(),
        }
    }
}

/// What a file to import is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// authwing's own backup ([`seal`]).
    Authwing,
    /// `otpauth://` URIs, one per line, as [`plain`] and most apps write
    /// them; Google Authenticator's `otpauth-migration://` ones too.
    Uris,
    /// Aegis's vault export, plain or encrypted.
    Aegis,
    /// A 2FAS backup (`.2fas`), plain or encrypted.
    TwoFas,
    /// Bitwarden's JSON export.
    Bitwarden,
    /// Ente Auth's export.
    Ente,
    /// andOTP's backup in the clear. A sealed one is an
    /// [`EncryptedFile`](Format::EncryptedFile).
    AndOtp,
    /// The backup of Stratum (once Authenticator Pro), plain or encrypted.
    Stratum,
    /// An encrypted file that does not say what it is: andOTP's backup, or
    /// TOTP Authenticator's. Whichever opens with the password is read.
    EncryptedFile,
}

impl Format {
    /// The app's name, or what the file is, to show.
    pub fn name(self) -> &'static str {
        match self {
            Format::Authwing => "authwing",
            Format::Uris => "otpauth links",
            Format::Aegis => "Aegis",
            Format::TwoFas => "2FAS",
            Format::Bitwarden => "Bitwarden",
            Format::Ente => "Ente Auth",
            Format::AndOtp => "andOTP",
            Format::Stratum => "Stratum (Authenticator Pro)",
            Format::EncryptedFile => "an encrypted export",
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What an import brought in, and what it left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Imported {
    pub items: Vec<Item>,
    /// Each entry left out: what it is (its issuer and account, or where it
    /// is in the file) and why.
    pub skipped: Vec<(String, String)>,
}

/// Why a backup could not be made or a file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the passphrase is too short: at least {0} characters")]
    WeakPassphrase(usize),
    /// The file is encrypted: ask for its password and import it again.
    #[error("{}", password_needed(*.0))]
    PasswordNeeded(Format),
    #[error("that password does not open this file")]
    WrongPassword,
    /// The file is not whole: it does not read as what it says it is, or
    /// does not decrypt with the right password.
    #[error("the file is damaged ({0})")]
    Damaged(String),
    /// A kind of file, or a version of one, this version cannot read.
    #[error("{0}")]
    Unsupported(String),
    #[error("not a backup or an export authwing can read")]
    Unknown,
    #[error("the backup could not be sealed: {0}")]
    Seal(String),
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn damaged(why: impl fmt::Display) -> Error {
    Error::Damaged(why.to_string())
}

/// What [`Error::PasswordNeeded`] says: the app whose file it is, when
/// that is known.
fn password_needed(format: Format) -> String {
    match format {
        Format::EncryptedFile => "this file is encrypted: enter its password".into(),
        format => format!("this {format} file is encrypted: enter its password"),
    }
}

/// The fewest bytes an [`Format::EncryptedFile`] has: an AES-GCM nonce and
/// tag, around nothing.
const MIN_SEALED: usize = 12 + 16;

/// How files that hold no codes start (pictures, PDFs, ZIP archives, SQLite
/// databases): not text either, but not to be asked a password for.
const NOT_EXPORTS: [&[u8]; 6] = [
    b"\x89PNG",
    b"\xFF\xD8\xFF",
    b"GIF8",
    b"%PDF",
    b"PK\x03\x04",
    b"SQLite format 3\0",
];

/// Whether bytes that are not UTF-8 look encrypted: long enough, not a file
/// of another kind, and not text in another encoding either, where most
/// bytes are printable ASCII. In random bytes fewer than half are.
fn looks_sealed(bytes: &[u8]) -> bool {
    let printable = bytes
        .iter()
        .filter(|b| b.is_ascii_graphic() || b.is_ascii_whitespace())
        .count();
    bytes.len() >= MIN_SEALED
        && printable * 4 < bytes.len() * 3
        && !NOT_EXPORTS.iter().any(|magic| bytes.starts_with(magic))
}

/// Says which format `bytes` are in, from how they look: `None` when no
/// format authwing reads. Bytes that are not text, and base64 text of whole
/// AES blocks, are taken as an [`Format::EncryptedFile`] unless they say
/// what they are.
pub fn detect(bytes: &[u8]) -> Option<Format> {
    if stratum::is_sealed(bytes) {
        return Some(Format::Stratum);
    }
    let Ok(text) = std::str::from_utf8(without_bom(bytes)) else {
        return looks_sealed(bytes).then_some(Format::EncryptedFile);
    };
    let start = text.trim_start();
    if start.starts_with('[')
        && serde_json::from_str::<Value>(text).is_ok_and(|json| andotp::is_backup(&json))
    {
        return Some(Format::AndOtp);
    }
    if start.starts_with('{') {
        let Ok(json) = serde_json::from_str::<Value>(text) else {
            // A backup cut short still says what it is: damaged, not unknown.
            return text.contains(authwing::FORMAT).then_some(Format::Authwing);
        };
        let has = |key: &str| json.get(key).is_some();
        return if json.get("format").and_then(Value::as_str) == Some(authwing::FORMAT) {
            Some(Format::Authwing)
        } else if has("header") && has("db") {
            Some(Format::Aegis)
        } else if has("schemaVersion") && (has("services") || has("servicesEncrypted")) {
            Some(Format::TwoFas)
        } else if has("items") || has("passwordProtected") {
            Some(Format::Bitwarden)
        } else if has("kdfParams") && has("encryptedData") {
            Some(Format::Ente)
        } else if has("Authenticators") {
            Some(Format::Stratum)
        } else {
            None
        };
    }
    if text.lines().any(uris::is_uri) {
        return Some(if text.contains("codeDisplay=") {
            Format::Ente
        } else {
            Format::Uris
        });
    }
    // TOTP Authenticator writes its sealed backup in base64.
    totp_authenticator::sealed(text.as_bytes()).map(|_| Format::EncryptedFile)
}

/// Reads the codes in a file of any format [`detect`] knows. An encrypted
/// one needs its `password`: without it, [`Error::PasswordNeeded`] says to
/// ask for it.
pub fn import(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let format = detect(bytes).ok_or(Error::Unknown)?;
    let bytes = without_bom(bytes);
    match format {
        Format::Authwing => authwing::import(bytes, password),
        Format::Uris | Format::Ente => uris::import(bytes),
        Format::Aegis => aegis::import(bytes, password),
        Format::TwoFas => twofas::import(bytes, password),
        Format::Bitwarden => bitwarden::import(bytes),
        Format::AndOtp => andotp::import(bytes),
        Format::Stratum => stratum::import(bytes, password),
        Format::EncryptedFile => import_encrypted(bytes, password),
    }
}

/// Opens an app's encrypted backup with a password: `None` if it does not
/// open, or the bytes are no backup of that app's; once it opens, what it
/// holds, or why that does not read.
type Opener = fn(&[u8], &str) -> Option<Result<Imported>>;

/// The apps whose encrypted backups do not say what they are.
const OPENERS: [Opener; 3] = [andotp::open, andotp::open_old, totp_authenticator::open];

/// Reads an [`Format::EncryptedFile`] as the first app's backup `password`
/// opens. Without one, it may be TOTP Authenticator's sealed with the
/// app's own password; if not, the user is asked.
fn import_encrypted(bytes: &[u8], password: Option<&str>) -> Result<Imported> {
    let Some(password) = password else {
        return totp_authenticator::open(bytes, totp_authenticator::APP_PASSWORD)
            .unwrap_or(Err(Error::PasswordNeeded(Format::EncryptedFile)));
    };
    OPENERS
        .iter()
        .find_map(|open| open(bytes, password))
        .unwrap_or(Err(Error::WrongPassword))
}

/// `bytes` without the byte order mark some editors put first.
fn without_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)
}

/// How a skipped entry is named: `Issuer (account)`, or either alone, or
/// `fallback` when it has neither.
pub(crate) fn label(issuer: &str, account: &str, fallback: impl FnOnce() -> String) -> String {
    match (issuer.trim(), account.trim()) {
        ("", "") => fallback(),
        (issuer, "") => issuer.to_string(),
        ("", account) => account.to_string(),
        (issuer, account) => format!("{issuer} ({account})"),
    }
}

/// A code from the settings another app keeps, read and checked as a vault
/// record's fields are ([`authwing_totp::fields`]): what is `None` takes its
/// default.
pub(crate) fn otp_from(fields: &[(&str, Option<String>)]) -> authwing_totp::Result<Otp> {
    let fields = fields
        .iter()
        .filter_map(|(name, value)| Some((name.to_string(), value.clone()?)))
        .collect();
    Otp::from_fields(&fields)
}

#[cfg(test)]
pub(crate) mod testing {
    //! The codes in the sample exports of Aegis's test data (see
    //! `crate::tests`), to check every importer against.

    use authwing_totp::Otp;

    use crate::Item;

    /// The seven codes: TOTP, HOTP and Steam, SHA-1 to SHA-512, 6 to 8
    /// digits, periods other than 30 seconds.
    pub(crate) const URIS: [&str; 7] = [
        "otpauth://totp/Deno:Mason?secret=4SJHB4GSD43FZBAI7C2HLRJGPQ&issuer=Deno&algorithm=SHA1&digits=6&period=30",
        "otpauth://totp/SPDX:James?secret=5OM4WOOGPLQEF6UGN3CPEOOLWU&issuer=SPDX&algorithm=SHA256&digits=7&period=20",
        "otpauth://totp/Airbnb:Elijah?secret=7ELGJSGXNCCTV3O6LKJWYFV2RA&issuer=Airbnb&algorithm=SHA512&digits=8&period=50",
        "otpauth://hotp/Issuu:James?secret=YOOMIXWS5GN6RTBPUFFWKTW5M4&issuer=Issuu&algorithm=SHA1&digits=6&counter=1",
        "otpauth://hotp/Air%20Canada:Benjamin?secret=KUVJJOM753IHTNDSZVCNKL7GII&issuer=Air+Canada&algorithm=SHA256&digits=7&counter=50",
        "otpauth://hotp/WWE:Mason?secret=5VAML3X35THCEBVRLV24CGBKOY&issuer=WWE&algorithm=SHA512&digits=8&counter=10300",
        "otpauth://steam/Boeing:Sophia?secret=JRZCL47CMXVOQMNPZR2F7J4RGI&issuer=Boeing",
    ];

    pub(crate) fn code(uri: &str) -> Otp {
        Otp::from_uri(uri).expect(uri)
    }

    /// Checks `items` are the codes `uris` give, in any order.
    pub(crate) fn assert_codes(items: &[Item], uris: &[&str]) {
        let mut got: Vec<String> = items.iter().map(|item| item.otp.to_uri()).collect();
        let mut want: Vec<String> = uris.iter().map(|uri| code(uri).to_uri()).collect();
        got.sort();
        want.sort();
        assert_eq!(got, want);
    }
}

#[cfg(test)]
mod tests {
    //! The sample exports in `testdata/` come unchanged from Aegis's
    //! importer tests (https://github.com/beemdevelopment/Aegis, under
    //! `app/src/test/resources/com/beemdevelopment/aegis/importers/`;
    //! GPL-3.0, which authwing's GPL-3.0-or-later can carry): files the
    //! real apps wrote, the encrypted ones with the password `test`.

    use super::*;

    #[test]
    fn detects_each_format() {
        let cases: [(&[u8], Format); 14] = [
            (
                include_bytes!("../testdata/aegis_plain.json"),
                Format::Aegis,
            ),
            (
                include_bytes!("../testdata/aegis_encrypted.json"),
                Format::Aegis,
            ),
            (
                include_bytes!("../testdata/2fas_authenticator_plain_v4.2fas"),
                Format::TwoFas,
            ),
            (
                include_bytes!("../testdata/2fas_authenticator_encrypted_v4.2fas"),
                Format::TwoFas,
            ),
            (
                include_bytes!("../testdata/bitwarden.json"),
                Format::Bitwarden,
            ),
            (include_bytes!("../testdata/ente_auth.txt"), Format::Ente),
            (
                b"\xEF\xBB\xBF\n otpauth://totp/x?secret=JBSWY3DPEHPK3PXP\n",
                Format::Uris,
            ),
            (
                include_bytes!("../testdata/andotp_plain.json"),
                Format::AndOtp,
            ),
            (
                include_bytes!("../testdata/stratum_plain.json"),
                Format::Stratum,
            ),
            (
                include_bytes!("../testdata/stratum_encrypted.bin"),
                Format::Stratum,
            ),
            (
                include_bytes!("../testdata/stratum_encrypted_legacy.bin"),
                Format::Stratum,
            ),
            (
                include_bytes!("../testdata/andotp_encrypted.bin"),
                Format::EncryptedFile,
            ),
            (
                include_bytes!("../testdata/andotp_encrypted_old.bin"),
                Format::EncryptedFile,
            ),
            (
                include_bytes!("../testdata/totp_authenticator.bin"),
                Format::EncryptedFile,
            ),
        ];
        for (bytes, format) in cases {
            assert_eq!(detect(bytes), Some(format));
        }
        let sealed = seal(&[], "correct horse battery").expect("seal");
        assert_eq!(detect(&sealed), Some(Format::Authwing));
        let migration = b"otpauth-migration://offline?data=CgA";
        assert_eq!(detect(migration), Some(Format::Uris));
        // Not UTF-8, but no encrypted file: a picture, and text in Latin-1.
        let png = [&b"\x89PNG\r\n\x1A\n"[..], &[0xC8; 40]].concat();
        let latin1 = b"name;secret\nCaf\xE9 Ren\xE9;JBSWY3DPEHPK3PXP\n";
        // A list, but not of andOTP's entries; base64, but not of AES blocks.
        let list = b"[{\"name\": \"x\"}]";
        let short = b"JBSWY3DPEHPK3PXP\n";
        for other in [
            &b"{\"version\": 1}"[..],
            b"hello",
            b"\xFF\xFE",
            b"",
            &png,
            latin1,
            list,
            short,
            b"[]",
        ] {
            assert_eq!(detect(other), None);
            assert!(matches!(import(other, None), Err(Error::Unknown)));
        }
    }

    #[test]
    fn an_encrypted_file_no_password_opens_is_the_wrong_password() {
        let noise: Vec<u8> = (0..200u8).map(|n| n.wrapping_mul(151) ^ 0xA5).collect();
        assert_eq!(detect(&noise), Some(Format::EncryptedFile));
        assert!(matches!(
            import(&noise, None),
            Err(Error::PasswordNeeded(Format::EncryptedFile))
        ));
        assert!(matches!(
            import(&noise, Some("test")),
            Err(Error::WrongPassword)
        ));
        assert_eq!(
            Error::PasswordNeeded(Format::EncryptedFile).to_string(),
            "this file is encrypted: enter its password"
        );
        assert_eq!(
            Error::PasswordNeeded(Format::Stratum).to_string(),
            "this Stratum (Authenticator Pro) file is encrypted: enter its password"
        );
    }

    #[test]
    fn labels_name_what_there_is() {
        let none = || "entry 3".to_string();
        assert_eq!(label("GitHub", "alice", none), "GitHub (alice)");
        assert_eq!(label("GitHub", " ", none), "GitHub");
        assert_eq!(label("", "alice", none), "alice");
        assert_eq!(label("", "", none), "entry 3");
    }
}
