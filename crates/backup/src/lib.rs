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
//! - [`import`] reads authwing's backup; the exports of Aegis, 2FAS,
//!   andOTP, FreeOTP+, Proton Authenticator, Raivo OTP, Stratum
//!   (Authenticator Pro) and TOTP Authenticator, Ente Auth's plain-text one
//!   and Steam Desktop Authenticator's maFiles; those of the password
//!   managers that keep codes beside logins: Bitwarden's JSON (protected by
//!   a password too), 1Password's `.1pux`, Proton Pass's ZIP, and the CSV
//!   of Apple Passwords, 1Password, Bitwarden, KeePassXC, Proton Pass or
//!   Dashlane; and lists of URIs (with Google Authenticator's
//!   `otpauth-migration://` among them). [`detect`] says which a file is. A
//!   code authwing cannot use (MD5, mOTP, no secret) is listed in
//!   [`Imported::skipped`] with the reason, and the rest come in: one bad
//!   entry does not cost the user the others.
//! - Encrypted backups that do not say what they are, andOTP's and TOTP
//!   Authenticator's, are [`Format::EncryptedFile`]: the password that opens
//!   one tells which app wrote it.

mod aegis;
mod andotp;
mod archive;
mod authwing;
mod bitwarden;
mod crypto;
mod csv;
mod freeotp_plus;
mod login;
mod onepassword;
mod proton;
mod protonpass;
mod raivo;
mod steam;
mod stratum;
mod totp_authenticator;
mod twofas;
mod uris;

use std::fmt;

use authwing_totp::Otp;
use serde_json::Value;

pub use authwing::{EXTENSION, MIN_PASSPHRASE, open, seal};
pub use csv::PasswordManager;
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
    /// Bitwarden's JSON export, plain or protected by a password.
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
    /// FreeOTP+'s JSON export.
    FreeOtpPlus,
    /// Proton Authenticator's export, plain or encrypted.
    Proton,
    /// A Steam Guard maFile, from Steam Desktop Authenticator, or the Steam
    /// app's own file of its accounts.
    Steam,
    /// A password manager's CSV export with a column of one-time passwords:
    /// which app's, when its header says.
    Csv(Option<PasswordManager>),
    /// 1Password's export (`.1pux`).
    OnePassword,
    /// Proton Pass's export, a ZIP; the one encrypted with a passphrase is
    /// known, but not read.
    ProtonPass,
    /// Raivo OTP's export: a ZIP encrypted with its password.
    Raivo,
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
            Format::FreeOtpPlus => "FreeOTP+",
            Format::Proton => "Proton Authenticator",
            Format::Steam => "Steam (maFile)",
            Format::Csv(Some(app)) => app.name(),
            Format::Csv(None) => "a CSV export",
            Format::OnePassword => "1Password",
            Format::ProtonPass => "Proton Pass",
            Format::Raivo => "Raivo OTP",
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
    if archive::is_zip(bytes) {
        return detect_archive(bytes);
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
        let steam_accounts = || {
            let accounts = json.get("accounts").and_then(Value::as_object);
            accounts.is_some_and(|a| a.values().any(|a| a.get("shared_secret").is_some()))
        };
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
        } else if json.get("tokens").is_some_and(Value::is_array) {
            Some(Format::FreeOtpPlus)
        } else if has("version") && (has("entries") || (has("salt") && has("content"))) {
            Some(Format::Proton)
        } else if has("shared_secret") || steam_accounts() {
            Some(Format::Steam)
        } else {
            None
        };
    }
    if let Some(format) = csv::detect(text) {
        return Some(format);
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

/// Which export a ZIP archive is, from the entries it holds.
fn detect_archive(bytes: &[u8]) -> Option<Format> {
    let mut archive = archive::Archive::open(bytes).ok()?;
    if archive.has(onepassword::DATA) {
        Some(Format::OnePassword)
    } else if archive.has(protonpass::DATA) || archive.has(protonpass::SEALED) {
        Some(Format::ProtonPass)
    } else if archive.has(raivo::DATA) {
        Some(Format::Raivo)
    } else if archive.has(csv::DASHLANE) {
        let csv = archive.read(csv::DASHLANE, None).ok()?;
        csv::detect(std::str::from_utf8(without_bom(&csv)).ok()?)
    } else {
        None
    }
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
        Format::Bitwarden => bitwarden::import(bytes, password),
        Format::AndOtp => andotp::import(bytes),
        Format::Stratum => stratum::import(bytes, password),
        Format::EncryptedFile => import_encrypted(bytes, password),
        Format::FreeOtpPlus => freeotp_plus::import(bytes),
        Format::Proton => proton::import(bytes, password),
        Format::Steam => steam::import(bytes),
        Format::Csv(_) => csv::import(bytes),
        Format::OnePassword => onepassword::import(bytes),
        Format::ProtonPass => protonpass::import(bytes),
        Format::Raivo => raivo::import(bytes, password),
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
pub(crate) fn without_bom(bytes: &[u8]) -> &[u8] {
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

    /// A ZIP archive of `entries`, deflated and, with a password, sealed
    /// with AES-256 as WinZip seals them.
    pub(crate) fn zipped(entries: &[(&str, &[u8])], password: Option<&str>) -> Vec<u8> {
        use std::io::Write;

        use zip::write::SimpleFileOptions;
        use zip::{AesMode, CompressionMethod, ZipWriter};

        let mut zip = ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let mut options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        if let Some(password) = password {
            options = options.with_aes_encryption(AesMode::Aes256, password);
        }
        for (name, content) in entries {
            zip.start_file(*name, options).expect("entry");
            zip.write_all(content).expect("write");
        }
        zip.finish().expect("zip").into_inner()
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
    //! The sample exports in `testdata/` of Aegis, 2FAS, Bitwarden (its
    //! JSON and its CSV) and Ente Auth come unchanged from Aegis's importer
    //! tests (https://github.com/beemdevelopment/Aegis, under
    //! `app/src/test/resources/com/beemdevelopment/aegis/importers/`;
    //! GPL-3.0, which authwing's GPL-3.0-or-later can carry): files the
    //! real apps wrote, the encrypted ones with the password `test`. Those
    //! of the other password managers and of Raivo OTP are authwing's own,
    //! written from each app's format with some of the same codes, as their
    //! importers' tests say.

    use super::*;

    #[test]
    fn detects_each_format() {
        let csv = |app| Format::Csv(Some(app));
        let cases: [(&[u8], Format); 28] = [
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
                include_bytes!("../testdata/freeotp_plus.json"),
                Format::FreeOtpPlus,
            ),
            (
                include_bytes!("../testdata/proton_authenticator.json"),
                Format::Proton,
            ),
            (
                include_bytes!("../testdata/proton_authenticator_encrypted.json"),
                Format::Proton,
            ),
            (include_bytes!("../testdata/steam.json"), Format::Steam),
            (include_bytes!("../testdata/steam_old.json"), Format::Steam),
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
            (
                include_bytes!("../testdata/bitwarden.csv"),
                csv(PasswordManager::Bitwarden),
            ),
            (
                include_bytes!("../testdata/apple_passwords.csv"),
                csv(PasswordManager::ApplePasswords),
            ),
            (
                include_bytes!("../testdata/1password.csv"),
                csv(PasswordManager::OnePassword),
            ),
            (
                include_bytes!("../testdata/keepassxc.csv"),
                csv(PasswordManager::KeePassXc),
            ),
            (
                include_bytes!("../testdata/proton_pass.csv"),
                csv(PasswordManager::ProtonPass),
            ),
            (
                include_bytes!("../testdata/dashlane_credentials.csv"),
                csv(PasswordManager::Dashlane),
            ),
            (
                include_bytes!("../testdata/1password.1pux"),
                Format::OnePassword,
            ),
            (
                include_bytes!("../testdata/proton_pass.zip"),
                Format::ProtonPass,
            ),
            (include_bytes!("../testdata/raivo_otp.zip"), Format::Raivo),
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
        // A ZIP, but of no export; a CSV, but with no one-time passwords.
        let other_zip = testing::zipped(&[("readme.txt", b"hello")], None);
        for other in [
            &b"{\"version\": 1}"[..],
            b"{\"tokens\": 1}",
            b"{\"accounts\": {\"1\": {\"account_name\": \"x\"}}}",
            b"hello",
            b"\xFF\xFE",
            b"",
            &png,
            latin1,
            list,
            short,
            b"[]",
            b"name,url,password\nGitHub,github.com,pw\n",
            &other_zip,
            b"PK\x03\x04 cut short",
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
    fn names_say_which_app() {
        let names = [
            Format::Csv(Some(PasswordManager::ApplePasswords)),
            Format::Csv(Some(PasswordManager::OnePassword)),
            Format::Csv(Some(PasswordManager::Bitwarden)),
            Format::Csv(Some(PasswordManager::KeePassXc)),
            Format::Csv(Some(PasswordManager::ProtonPass)),
            Format::Csv(Some(PasswordManager::Dashlane)),
            Format::Csv(None),
            Format::OnePassword,
            Format::ProtonPass,
            Format::Raivo,
        ]
        .map(Format::name);
        assert_eq!(
            names,
            [
                "Apple Passwords",
                "1Password",
                "Bitwarden",
                "KeePassXC",
                "Proton Pass",
                "Dashlane",
                "a CSV export",
                "1Password",
                "Proton Pass",
                "Raivo OTP",
            ]
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
