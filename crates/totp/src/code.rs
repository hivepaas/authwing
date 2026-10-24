//! The codes: HOTP (RFC 4226), TOTP (RFC 6238) and Steam Guard.
//!
//! All three are one computation: an HMAC of an 8-byte counter, cut down to
//! a 31-bit number (RFC 4226's dynamic truncation), then written in decimal
//! or, for Steam, in Steam's letters. TOTP takes the counter from the time,
//! which comes in as an argument, so nothing here reads the clock.

use std::fmt;
use std::str::FromStr;

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use crate::{Error, Result};

/// Steam Guard's characters: digits and consonants hard to mistake for one
/// another (no 0, 1, vowels, L, S or Z).
const STEAM_ALPHABET: &[u8; 26] = b"23456789BCDFGHJKMNPQRTVWXY";
pub(crate) const STEAM_PERIOD: u32 = 30;
pub(crate) const STEAM_LENGTH: u8 = 5;

/// The hash the HMAC is built on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Algorithm {
    /// What nearly every site uses, and the default when none is named.
    #[default]
    Sha1,
    Sha256,
    Sha512,
}

impl Algorithm {
    /// The name URIs and vault records use: `SHA1`, `SHA256` or `SHA512`.
    pub fn name(self) -> &'static str {
        match self {
            Algorithm::Sha1 => "SHA1",
            Algorithm::Sha256 => "SHA256",
            Algorithm::Sha512 => "SHA512",
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Reads a name as written in the wild: any case, with or without a dash
/// (`sha-256`). MD5, which old exports still carry, is an error.
impl FromStr for Algorithm {
    type Err = Error;

    fn from_str(text: &str) -> Result<Algorithm> {
        match text.trim().replace('-', "").to_ascii_uppercase().as_str() {
            "SHA1" => Ok(Algorithm::Sha1),
            "SHA256" => Ok(Algorithm::Sha256),
            "SHA512" => Ok(Algorithm::Sha512),
            _ => Err(Error::Algorithm(text.trim().to_string())),
        }
    }
}

/// An HOTP code (RFC 4226): the code for `counter`, as `digits` decimal
/// digits with leading zeros. The RFC allows 6 to 8 digits; what this crate
/// reads may have 5 to 10 ([`DIGITS_RANGE`](crate::DIGITS_RANGE)).
pub fn hotp(key: &[u8], counter: u64, algorithm: Algorithm, digits: u8) -> String {
    let value = u64::from(truncated(key, counter, algorithm));
    // Past 10 digits a 31-bit number has only leading zeros to add.
    let modulus = 10u64.checked_pow(u32::from(digits)).unwrap_or(u64::MAX);
    format!("{:0width$}", value % modulus, width = usize::from(digits))
}

/// A TOTP code (RFC 6238): the HOTP code for the step `unix_seconds` falls
/// in, steps being `period` seconds long and counted from the Unix epoch.
pub fn totp(
    key: &[u8],
    unix_seconds: u64,
    period: u32,
    algorithm: Algorithm,
    digits: u8,
) -> String {
    hotp(key, time_step(unix_seconds, period), algorithm, digits)
}

/// RFC 6238's T, the counter TOTP uses at `unix_seconds`: whole periods
/// since T0, which is the Unix epoch.
pub fn time_step(unix_seconds: u64, period: u32) -> u64 {
    unix_seconds / step(period)
}

/// Seconds until the code at `unix_seconds` changes, from `period` down to 1.
pub fn seconds_left(unix_seconds: u64, period: u32) -> u32 {
    let step = step(period);
    (step - unix_seconds % step) as u32
}

/// A Steam Guard code at `unix_seconds`: SHA-1 and 30-second steps as in
/// TOTP, with the truncated number written as five characters of Steam's
/// alphabet, lowest base-26 digit first.
pub fn steam(key: &[u8], unix_seconds: u64) -> String {
    let mut value = truncated(key, time_step(unix_seconds, STEAM_PERIOD), Algorithm::Sha1);
    (0..STEAM_LENGTH)
        .map(|_| {
            let c = STEAM_ALPHABET[(value % 26) as usize];
            value /= 26;
            char::from(c)
        })
        .collect()
}

/// A period as a divisor. What reads periods rejects 0; here it counts as 1,
/// so that an [`Otp`](crate::Otp) built by hand cannot panic.
fn step(period: u32) -> u64 {
    u64::from(period.max(1))
}

/// RFC 4226's HOTP value before it is written out: the HMAC of `counter`,
/// truncated to 31 bits.
fn truncated(key: &[u8], counter: u64, algorithm: Algorithm) -> u32 {
    let counter = counter.to_be_bytes();
    match algorithm {
        Algorithm::Sha1 => truncate::<Hmac<Sha1>>(key, &counter),
        Algorithm::Sha256 => truncate::<Hmac<Sha256>>(key, &counter),
        Algorithm::Sha512 => truncate::<Hmac<Sha512>>(key, &counter),
    }
}

fn truncate<M: Mac + hmac::digest::KeyInit>(key: &[u8], counter: &[u8]) -> u32 {
    let mut mac = <M as Mac>::new_from_slice(key).expect("HMAC takes keys of any length");
    mac.update(counter);
    let hash = mac.finalize().into_bytes();
    // The last 4 bits say where to take 4 bytes from; the top bit is dropped
    // so the number reads the same signed or unsigned.
    let offset = usize::from(hash[hash.len() - 1] & 0x0f);
    let bytes = [
        hash[offset],
        hash[offset + 1],
        hash[offset + 2],
        hash[offset + 3],
    ];
    u32::from_be_bytes(bytes) & 0x7fff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED_20: &[u8] = b"12345678901234567890";
    const SEED_32: &[u8] = b"12345678901234567890123456789012";
    const SEED_64: &[u8] = b"1234567890123456789012345678901234567890123456789012345678901234";

    #[test]
    fn rfc_4226_appendix_d() {
        let expected = [
            "755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583",
            "399871", "520489",
        ];
        for (counter, code) in (0..).zip(expected) {
            assert_eq!(
                hotp(SEED_20, counter, Algorithm::Sha1, 6),
                code,
                "counter {counter}"
            );
        }
    }

    #[test]
    fn rfc_6238_appendix_b() {
        // Time, then the SHA-1, SHA-256 and SHA-512 codes, 8 digits each.
        let expected = [
            (59, ["94287082", "46119246", "90693936"]),
            (1111111109, ["07081804", "68084774", "25091201"]),
            (1111111111, ["14050471", "67062674", "99943326"]),
            (1234567890, ["89005924", "91819424", "93441116"]),
            (2000000000, ["69279037", "90698825", "38618901"]),
            (20000000000, ["65353130", "77737706", "47863826"]),
        ];
        let modes = [
            (Algorithm::Sha1, SEED_20),
            (Algorithm::Sha256, SEED_32),
            (Algorithm::Sha512, SEED_64),
        ];
        for (time, codes) in expected {
            for ((algorithm, seed), code) in modes.into_iter().zip(codes) {
                assert_eq!(
                    totp(seed, time, 30, algorithm, 8),
                    code,
                    "{algorithm} at {time}"
                );
            }
        }
    }

    #[test]
    fn steam_guard() {
        // From an independent Python implementation:
        //   v = int.from_bytes(hmac.new(key, (t // 30).to_bytes(8, "big"),
        //       "sha1").digest()[o:o + 4], "big") & 0x7fffffff, o = last byte & 15;
        //   then 5 times: alphabet[v % 26], v //= 26.
        assert_eq!(steam(SEED_20, 1234567890), "VHHQY");
        assert_eq!(steam(SEED_20, 59), "PV9M4");
        assert_eq!(steam(SEED_20, 1111111109), "PY4YB");
        // The same within a step, different in the next.
        assert_eq!(steam(SEED_20, 1234567860), steam(SEED_20, 1234567889));
        assert_ne!(steam(SEED_20, 1234567889), steam(SEED_20, 1234567890));
        for t in (0..3000).step_by(30) {
            let code = steam(SEED_20, t);
            assert_eq!(code.len(), 5);
            assert!(code.bytes().all(|c| STEAM_ALPHABET.contains(&c)), "{code}");
        }
    }

    #[test]
    fn digits_are_zero_padded() {
        // RFC 6238 at 1111111109 gives 07081804 with 8 digits, from the
        // truncated number 907081804.
        assert_eq!(totp(SEED_20, 1111111109, 30, Algorithm::Sha1, 6), "081804");
        assert_eq!(
            totp(SEED_20, 1111111109, 30, Algorithm::Sha1, 10),
            "0907081804"
        );
        assert_eq!(hotp(SEED_20, 0, Algorithm::Sha1, 5), "55224");
    }

    #[test]
    fn steps_and_seconds_left() {
        assert_eq!(time_step(59, 30), 1);
        assert_eq!(time_step(60, 30), 2);
        assert_eq!(seconds_left(60, 30), 30);
        assert_eq!(seconds_left(89, 30), 1);
        assert_eq!(seconds_left(61, 60), 59);
        assert_eq!(seconds_left(5, 0), 1, "a period of 0 does not panic");
    }

    #[test]
    fn algorithm_names() {
        for name in ["SHA1", "sha1", "Sha-1", " SHA1 "] {
            assert_eq!(name.parse(), Ok(Algorithm::Sha1), "{name}");
        }
        assert_eq!("sha256".parse(), Ok(Algorithm::Sha256));
        assert_eq!("SHA-512".parse(), Ok(Algorithm::Sha512));
        assert_eq!(
            "MD5".parse::<Algorithm>(),
            Err(Error::Algorithm("MD5".into()))
        );
        assert_eq!(Algorithm::Sha256.to_string(), "SHA256");
    }
}
