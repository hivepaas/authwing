//! Finding two-factor QR codes, so the user can add a code from the QR code
//! a site shows them, without a phone.
//!
//! Three places to look, each giving the text of every QR code found:
//!
//! - [`decode`]: an image already in memory.
//! - [`from_file_bytes`]: an image file, or an image from the clipboard.
//! - [`screen`]: every display, as it is now. On macOS this needs the
//!   user's permission ([`access`], [`ask_access`]).
//! - `camera` (feature `camera`): the camera's frames, as they come.
//!
//! [`two_factor`] then keeps the texts that are two-factor codes: an
//! `otpauth://` URI for one code, or an `otpauth-migration://` one for the
//! codes Google Authenticator exports.
//!
//! The codes are found and read by `rqrr`, in pure Rust. None of this has
//! UI, and the reading takes a moment on a large screen: call it off the
//! UI thread.

#[cfg(feature = "camera")]
pub mod camera;
mod decode;
mod screen;

pub use decode::decode;
pub use image::RgbaImage;
pub use screen::{Access, Found, access, ask_access, screen};

/// Why the screen or an image could not be read.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The user has not allowed authwing to capture the screen: on macOS,
    /// not yet in Privacy & Security, Screen Recording ([`ask_access`]); on
    /// Linux, they turned the desktop portal's request down.
    #[error("authwing is not allowed to capture the screen")]
    NotAllowed,
    /// The system would not give an image of the screen.
    #[error("the screen could not be captured: {0}")]
    Capture(String),
    /// The bytes are not an image in a format this reads.
    #[error("the image could not be read: {0}")]
    Unreadable(#[from] image::ImageError),
}

/// The text of every QR code in an image file's bytes, or a clipboard
/// image's: PNG and JPEG, and GIF, WebP, BMP and TIFF, told apart by how
/// they start.
pub fn from_file_bytes(bytes: &[u8]) -> Result<Vec<String>, Error> {
    let image = image::load_from_memory(bytes)?;
    Ok(decode(&image.into_rgba8()))
}

/// The two-factor texts among `texts`: those that start `otpauth://` or
/// `otpauth-migration://`, whatever the scheme's case, without surrounding
/// spaces, each once, in their order.
pub fn two_factor(texts: &[String]) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    for text in texts {
        let text = text.trim();
        if is_two_factor(text) && !kept.iter().any(|seen| seen == text) {
            kept.push(text.to_owned());
        }
    }
    kept
}

fn is_two_factor(text: &str) -> bool {
    ["otpauth://", "otpauth-migration://"].iter().any(|scheme| {
        text.get(..scheme.len())
            .is_some_and(|start| start.eq_ignore_ascii_case(scheme))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageFormat, Rgba};
    use std::io::Cursor;

    pub(crate) const TOTP: &str =
        "otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example";
    pub(crate) const MIGRATION: &str = "otpauth-migration://offline?data=CjEKCkhlbGxvId6tvu8SGEV4YW1wbGU6YWxpY2VAZXhhbXBsZS5jb20aB0V4YW1wbGUgASgBMAIQARgBIAA%3D";

    /// A QR code for `text`, black on white, `module` pixels to a module,
    /// with the four modules of white around it the standard asks for.
    pub(crate) fn qr(text: &str, module: u32) -> RgbaImage {
        let code = qrcode::QrCode::new(text.as_bytes()).expect("a QR code");
        let width = u32::try_from(code.width()).expect("width");
        let colors = code.to_colors();
        let side = (width + 8) * module;
        RgbaImage::from_fn(side, side, |x, y| {
            let (x, y) = (x / module, y / module);
            let dark = (4..width + 4).contains(&x)
                && (4..width + 4).contains(&y)
                && colors[((y - 4) * width + x - 4) as usize] == qrcode::Color::Dark;
            if dark {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        })
    }

    fn encoded(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
        let mut bytes = Vec::new();
        image
            .write_to(&mut Cursor::new(&mut bytes), format)
            .expect("encode");
        bytes
    }

    #[test]
    fn two_factor_keeps_otpauth_uris_once_in_order() {
        let texts: Vec<String> = [
            "https://example.com/otpauth://totp/x",
            "OTPAuth://totp/B?secret=BBBB",
            "  otpauth://totp/A?secret=AAAA\n",
            "otpauth-migration://offline?data=AA",
            "otpauth:totp/missing-slashes",
            "otpauth-migrationx://offline",
            "otpauth://totp/A?secret=AAAA",
            "OTPAUTH-MIGRATION://offline?data=BB",
            "otpaut",
            "",
        ]
        .map(String::from)
        .to_vec();
        assert_eq!(
            two_factor(&texts),
            [
                "OTPAuth://totp/B?secret=BBBB",
                "otpauth://totp/A?secret=AAAA",
                "otpauth-migration://offline?data=AA",
                "OTPAUTH-MIGRATION://offline?data=BB",
            ]
        );
        assert!(two_factor(&[]).is_empty());
    }

    #[test]
    fn png_bytes() {
        let bytes = encoded(DynamicImage::ImageRgba8(qr(TOTP, 4)), ImageFormat::Png);
        assert_eq!(from_file_bytes(&bytes).expect("png"), [TOTP]);
    }

    #[test]
    fn jpeg_bytes() {
        let image = DynamicImage::ImageRgba8(qr(MIGRATION, 6)).to_rgb8();
        let bytes = encoded(DynamicImage::ImageRgb8(image), ImageFormat::Jpeg);
        assert_eq!(from_file_bytes(&bytes).expect("jpeg"), [MIGRATION]);
    }

    /// Dark modules on a transparent ground, as a code copied from a web page
    /// can be: the ground is taken as white, not as the black it is stored as.
    #[test]
    fn png_with_a_transparent_ground() {
        let mut image = qr(TOTP, 4);
        for pixel in image.pixels_mut() {
            if pixel.0[0] == 255 {
                *pixel = Rgba([0, 0, 0, 0]);
            }
        }
        let bytes = encoded(DynamicImage::ImageRgba8(image), ImageFormat::Png);
        assert_eq!(from_file_bytes(&bytes).expect("png"), [TOTP]);
    }

    #[test]
    fn bytes_that_are_no_image() {
        let error = from_file_bytes(b"otpauth://totp/not-an-image").expect_err("not an image");
        assert!(matches!(error, Error::Unreadable(_)), "{error:?}");
        assert!(error.to_string().starts_with("the image could not be read"));
    }
}
