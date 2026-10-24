//! One-time codes for two-factor sign-in, and the data that describes them.
//!
//! A site that turns on two-factor sign-in hands over an [`Otp`]: a secret
//! and how codes are made from it (HOTP, TOTP or Steam Guard). It comes as an
//! `otpauth://` URI, usually in a QR code, or many at once in Google
//! Authenticator's `otpauth-migration://` export ([`Migration`]), and the app
//! keeps it as the string fields of a vault record ([`fields`]).
//!
//! This crate has the formats and the arithmetic only: no UI, no storage and
//! no clock. Callers pass the time in Unix seconds, so a code depends on its
//! arguments alone and the tests check exact values.

pub mod base32;
mod code;
mod display;
mod error;
pub mod fields;
mod migration;
mod otp;
mod secret;
mod uri;

pub use code::{Algorithm, hotp, seconds_left, steam, time_step, totp};
pub use display::grouped;
pub use error::{Error, Result};
pub use migration::{Migration, Skipped};
pub use otp::{DEFAULT_DIGITS, DEFAULT_PERIOD, DIGITS_RANGE, Kind, Otp};
pub use secret::Secret;
