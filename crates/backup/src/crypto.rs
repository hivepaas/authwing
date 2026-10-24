//! What the formats share: random bytes, AES-256-GCM as Aegis, 2FAS,
//! andOTP and Stratum use it, AES-256-CBC as older Stratum and TOTP
//! Authenticator do, and the text forms of bytes in their files.

use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockDecryptMut, KeyIvInit};
use aes_gcm::Aes256Gcm;
use aes_gcm::aead::{Aead, KeyInit};
use base64::alphabet::STANDARD;
use base64::engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{Error, Result};

/// Standard base64, with or without its padding, as other apps write it.
pub(crate) const BASE64: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// The nonce AES-GCM takes: 96 bits.
const GCM_NONCE: usize = 12;

pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    getrandom::fill(&mut bytes).map_err(|e| Error::Seal(format!("no randomness: {e}")))?;
    Ok(bytes)
}

/// What AES-256-GCM sealed under `key` and `nonce`, `sealed` being the
/// ciphertext with the tag after it. `None` if the key or the nonce has the
/// wrong length, or the tag does not match: the wrong key, or damage.
pub(crate) fn aes_gcm_open(key: &[u8], nonce: &[u8], sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    if nonce.len() != GCM_NONCE {
        return None;
    }
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    cipher
        .decrypt(nonce.into(), sealed)
        .ok()
        .map(Zeroizing::new)
}

/// What AES-256-CBC sealed under `key` and `iv`, its PKCS #7 padding taken
/// off. `None` if the key or the IV has the wrong length, or the padding
/// is not whole. A wrong key fails the padding most of the time, not
/// always: what comes out still needs reading to tell.
pub(crate) fn aes_cbc_open(key: &[u8], iv: &[u8], sealed: &[u8]) -> Option<Zeroizing<Vec<u8>>> {
    let cipher = cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv).ok()?;
    let mut plain = Zeroizing::new(sealed.to_vec());
    let len = cipher.decrypt_padded_mut::<Pkcs7>(&mut plain).ok()?.len();
    plain.truncate(len);
    Some(plain)
}

/// The SHA-256 of `password`: the key andOTP's older backups and TOTP
/// Authenticator's are sealed under, with no salt and no cost.
pub(crate) fn sha256_key(password: &str) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&Sha256::digest(password.as_bytes()));
    key
}

/// `plain` sealed with AES-256-CBC and PKCS #7 padding, as
/// [`aes_cbc_open`] opens it: to make the files tests read.
#[cfg(test)]
pub(crate) fn aes_cbc_seal(key: &[u8], iv: &[u8], plain: &[u8]) -> Vec<u8> {
    use aes::cipher::BlockEncryptMut;
    let cipher = cbc::Encryptor::<aes::Aes256>::new_from_slices(key, iv).expect("key and IV");
    let mut sealed = plain.to_vec();
    sealed.resize(plain.len() + 16 - plain.len() % 16, 0);
    let len = cipher
        .encrypt_padded_mut::<Pkcs7>(&mut sealed, plain.len())
        .expect("room for the padding")
        .len();
    sealed.truncate(len);
    sealed
}

/// The bytes a hexadecimal string spells, in either case.
pub(crate) fn hex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim().as_bytes();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    let digit = |c: u8| char::from(c).to_digit(16);
    text.chunks(2)
        .map(|pair| Some((digit(pair[0])? * 16 + digit(pair[1])?) as u8))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::Payload;

    #[test]
    fn opens_what_aes_gcm_sealed() {
        let (key, nonce) = ([7u8; 32], [9u8; GCM_NONCE]);
        let sealed = Aes256Gcm::new_from_slice(&key)
            .expect("key")
            .encrypt(
                (&nonce).into(),
                Payload {
                    msg: b"codes",
                    aad: b"",
                },
            )
            .expect("seal");
        assert_eq!(
            aes_gcm_open(&key, &nonce, &sealed)
                .as_deref()
                .map(Vec::as_slice),
            Some(&b"codes"[..])
        );
        assert_eq!(aes_gcm_open(&[8u8; 32], &nonce, &sealed), None);
        assert_eq!(aes_gcm_open(&key, &nonce[..8], &sealed), None);
        assert_eq!(aes_gcm_open(&key[..16], &nonce, &sealed), None);
    }

    #[test]
    fn opens_what_aes_cbc_sealed() {
        let (key, iv) = ([7u8; 32], [9u8; 16]);
        for plain in [&b""[..], b"codes", b"sixteen bytes!!!"] {
            let sealed = aes_cbc_seal(&key, &iv, plain);
            assert_eq!(sealed.len() % 16, 0);
            assert_eq!(
                aes_cbc_open(&key, &iv, &sealed)
                    .as_deref()
                    .map(Vec::as_slice),
                Some(plain)
            );
        }
        let sealed = aes_cbc_seal(&key, &iv, b"codes");
        assert_eq!(aes_cbc_open(&key, &iv, &sealed[..15]), None);
        assert_eq!(aes_cbc_open(&key, &iv[..8], &sealed), None);
        assert_eq!(aes_cbc_open(&key[..16], &iv, &sealed), None);
    }

    #[test]
    fn reads_hex() {
        assert_eq!(hex("00ff7A"), Some(vec![0, 255, 122]));
        assert_eq!(hex(""), Some(vec![]));
        assert_eq!(hex("abc"), None);
        assert_eq!(hex("zz"), None);
    }
}
