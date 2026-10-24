//! The key codes are made from.
//!
//! Whoever holds it can make the codes, so it is wiped from memory when
//! dropped and `Debug` never shows it: a log line or a panic message that
//! includes an [`Otp`](crate::Otp) does not give it away. Its text forms
//! (base32, URIs, record fields) are plain strings, the caller's to look
//! after, as the vault encrypts what it keeps.

use std::fmt;

use zeroize::Zeroizing;

use crate::{Error, Result, base32};

/// The shared secret of a code.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    /// Takes `bytes` as the key, without copying them.
    pub fn new(bytes: Vec<u8>) -> Secret {
        Secret(Zeroizing::new(bytes))
    }

    /// Reads a key written in base32, as sites show it (see [`base32`]). An
    /// empty one is an error: anyone could make its codes.
    pub fn from_base32(text: &str) -> Result<Secret> {
        let bytes = base32::decode(text)?;
        if bytes.is_empty() {
            return Err(Error::NoSecret);
        }
        Ok(Secret(bytes))
    }

    /// The key in base32, as URIs and vault records hold it.
    pub fn to_base32(&self) -> String {
        base32::encode(&self.0)
    }

    /// The key itself, to make codes with.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_key() {
        let secret = Secret::from_base32("JBSWY3DPEHPK3PXP").expect("secret");
        let shown = format!("{secret:?}");
        assert_eq!(shown, "Secret(..)");
        assert!(!shown.contains("JBSW") && !shown.contains("72"));
    }

    #[test]
    fn empty_is_no_secret() {
        assert_eq!(Secret::from_base32(""), Err(Error::NoSecret));
        assert_eq!(Secret::from_base32(" = "), Err(Error::NoSecret));
        // A single character is fewer than 8 bits: nothing.
        assert_eq!(Secret::from_base32("A"), Err(Error::NoSecret));
    }

    #[test]
    fn round_trips_base32() {
        let secret = Secret::new(b"12345678901234567890".to_vec());
        let text = secret.to_base32();
        assert_eq!(text, "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ");
        assert_eq!(Secret::from_base32(&text), Ok(secret));
    }
}
