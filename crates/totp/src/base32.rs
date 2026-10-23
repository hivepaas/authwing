//! Base32 (RFC 4648), the text form of TOTP secrets.
//!
//! Decoding forgives what sites and people do to a secret: lower case,
//! spaces or dashes between groups, `=` padding. Bits left over at the end
//! that do not make a whole byte are dropped, as other authenticator apps
//! drop them, so a secret of an odd length gives the codes the site expects.
//! Anything else is an error rather than a guess: a secret read wrong gives
//! wrong codes with no other sign.

use zeroize::Zeroizing;

use crate::{Error, Result};

const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// The bytes `text` spells, wiped when dropped, as they are usually a key.
pub fn decode(text: &str) -> Result<Zeroizing<Vec<u8>>> {
    // Room for every byte up front: a vector that grew would leave copies of
    // the key behind in freed memory.
    let mut out = Zeroizing::new(Vec::with_capacity(text.len() * 5 / 8));
    let (mut bits, mut n) = (0u32, 0u32);
    let mut padded = false;
    for c in text.chars() {
        if c.is_whitespace() || c == '-' {
            continue;
        }
        if c == '=' {
            padded = true;
            continue;
        }
        if padded {
            return Err(Error::Base32Padding);
        }
        let value = match c.to_ascii_uppercase() {
            u @ 'A'..='Z' => u32::from(u) - u32::from('A'),
            d @ '2'..='7' => u32::from(d) - u32::from('2') + 26,
            _ => return Err(Error::Base32Char(c)),
        };
        bits = (bits << 5) | value;
        n += 5;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
            bits &= (1 << n) - 1;
        }
    }
    Ok(out)
}

/// `bytes` in upper case without padding, as otpauth URIs carry secrets.
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    let (mut bits, mut n) = (0u32, 0u32);
    for &b in bytes {
        bits = (bits << 8) | u32::from(b);
        n += 8;
        while n >= 5 {
            n -= 5;
            out.push(char::from(ALPHABET[((bits >> n) & 31) as usize]));
        }
        bits &= (1 << n) - 1;
    }
    if n > 0 {
        out.push(char::from(ALPHABET[((bits << (5 - n)) & 31) as usize]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4648 section 10, without the padding.
    const VECTORS: [(&str, &str); 7] = [
        ("", ""),
        ("f", "MY"),
        ("fo", "MZXQ"),
        ("foo", "MZXW6"),
        ("foob", "MZXW6YQ"),
        ("fooba", "MZXW6YTB"),
        ("foobar", "MZXW6YTBOI"),
    ];

    #[test]
    fn rfc_4648_vectors() {
        for (plain, encoded) in VECTORS {
            assert_eq!(encode(plain.as_bytes()), encoded);
            assert_eq!(*decode(encoded).expect(encoded), plain.as_bytes());
        }
        assert_eq!(*decode("MZXW6YTBOI======").expect("padded"), b"foobar");
    }

    #[test]
    fn forgives_case_spaces_and_dashes() {
        let key = decode("jbsw y3dp-ehpk 3pxp\n").expect("sloppy");
        assert_eq!(*key, *decode("JBSWY3DPEHPK3PXP").expect("clean"));
        assert_eq!(*key, b"Hello!\xde\xad\xbe\xef");
    }

    #[test]
    fn drops_leftover_bits() {
        // 3 characters are 15 bits: one byte and 7 bits that make none.
        assert_eq!(*decode("MZX").expect("odd"), b"f");
    }

    #[test]
    fn rejects_what_is_not_base32() {
        assert_eq!(decode("JBSW1"), Err(Error::Base32Char('1')));
        assert_eq!(decode("JBSW0"), Err(Error::Base32Char('0')));
        assert_eq!(decode("JBSW_Y3DP"), Err(Error::Base32Char('_')));
        assert_eq!(decode("JBSWé"), Err(Error::Base32Char('é')));
        assert_eq!(decode("MY==MY"), Err(Error::Base32Padding));
    }
}
