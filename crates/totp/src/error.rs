/// Why a secret, a URI or a record could not be read.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{0:?} is not a base32 character (A to Z and 2 to 7)")]
    Base32Char(char),
    #[error("base32 padding ('=') can only come at the end")]
    Base32Padding,
    /// No secret, or one that decodes to nothing.
    #[error("the secret is missing")]
    NoSecret,
    #[error("not an otpauth:// link")]
    NotOtpauth,
    #[error("not an otpauth-migration:// link")]
    NotMigration,
    /// Not TOTP, HOTP or Steam.
    #[error("unknown kind of code: {0}")]
    Kind(String),
    /// MD5, or a name that is no hash at all.
    #[error("unsupported algorithm: {0}")]
    Algorithm(String),
    #[error("unsupported number of digits: {0} (5 to 10 are supported)")]
    Digits(String),
    #[error("invalid period: {0} (whole seconds, at least 1)")]
    Period(String),
    #[error("invalid counter: {0}")]
    Counter(String),
    /// The export's data is missing, is not base64, or is not the protobuf
    /// message Google Authenticator writes.
    #[error("the export is damaged ({0})")]
    Migration(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
