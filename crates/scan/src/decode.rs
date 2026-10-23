//! Reading the QR codes in an image, as real screens show them.
//!
//! `rqrr` looks for dark modules on a light ground, and tells them apart
//! pixel by pixel. A screen does not always draw a code so. A Retina
//! screenshot is large, and a code in it can be grainy (a photo, a
//! compressed image, dithering), with pixels on the wrong side of light;
//! halved, four pixels averaged into one, it reads clean again. A page in
//! dark mode can draw its code light on dark. So an image that shows
//! nothing as it is is read again at half its size and at a quarter, then
//! with its colours inverted, at each size. The first read that finds a
//! code gives the answer: an image with a code is mostly read once, one
//! without six times (two thirds of a second for a 5K display, in a
//! release build).
//!
//! Transparent pixels are taken as white, the ground a code copied from a
//! page is drawn on, not as the black they are stored as.

use image::imageops::{self, FilterType};
use image::{GrayImage, Luma, RgbaImage};

/// How many times an image that shows nothing is halved, at most.
const HALVINGS: usize = 2;

/// The shortest side an image is halved to: smaller, a code in it would
/// have too few pixels a module to read.
const SMALLEST: u32 = 200;

/// The text of every QR code in `image`, each once, in the order found.
pub fn decode(image: &RgbaImage) -> Vec<String> {
    let mut grey = grey(image);
    let found = at_each_size(&grey);
    if !found.is_empty() {
        return found;
    }
    imageops::invert(&mut grey);
    at_each_size(&grey)
}

/// What [`read`] finds in `grey`, or else in it halved, and halved again.
fn at_each_size(grey: &GrayImage) -> Vec<String> {
    let mut found = read(grey);
    let mut smaller = None;
    for _ in 0..HALVINGS {
        if !found.is_empty() {
            break;
        }
        let image = smaller.as_ref().unwrap_or(grey);
        let (width, height) = (image.width() / 2, image.height() / 2);
        if width.min(height) < SMALLEST {
            break;
        }
        let halved = imageops::resize(image, width, height, FilterType::Triangle);
        found = read(&halved);
        smaller = Some(halved);
    }
    found
}

/// The texts of the codes `rqrr` finds in `grey` and can read.
fn read(grey: &GrayImage) -> Vec<String> {
    let (width, height) = (grey.width() as usize, grey.height() as usize);
    let pixels = grey.as_raw();
    let mut prepared =
        rqrr::PreparedImage::prepare_from_greyscale(width, height, |x, y| pixels[y * width + x]);
    let mut found: Vec<String> = Vec::new();
    for grid in prepared.detect_grids() {
        if let Ok((_, text)) = grid.decode()
            && !found.contains(&text)
        {
            found.push(text);
        }
    }
    found
}

/// `image` in shades of grey, its transparent pixels over white.
fn grey(image: &RgbaImage) -> GrayImage {
    GrayImage::from_fn(image.width(), image.height(), |x, y| {
        let [r, g, b, a] = image.get_pixel(x, y).0;
        // ITU-R BT.601 luma, in thousandths.
        let luma = (299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b)) / 1000;
        let alpha = u32::from(a);
        let over_white = (luma * alpha + 255 * (255 - alpha)) / 255;
        Luma([u8::try_from(over_white).unwrap_or(u8::MAX)])
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{MIGRATION, TOTP, qr};
    use image::Rgba;

    fn inverted(mut image: RgbaImage) -> RgbaImage {
        for pixel in image.pixels_mut() {
            let [r, g, b, a] = pixel.0;
            *pixel = Rgba([255 - r, 255 - g, 255 - b, a]);
        }
        image
    }

    /// A light window of `width` by `height`, with what screens show around
    /// a code: a title bar, lines of text, a button, a photo-like gradient.
    fn screenshot(width: u32, height: u32) -> RgbaImage {
        RgbaImage::from_fn(width, height, |x, y| {
            let shade = if y < 40 {
                225
            } else if (120..400).contains(&y) && (60..700).contains(&x) && y % 24 < 12 {
                // Words: dark runs and gaps along each line.
                if (x * 7 + y * 3) % 53 < 31 { 30 } else { 255 }
            } else if (450..500).contains(&y) && (60..260).contains(&x) {
                70
            } else if (560..900).contains(&y) && (60..600).contains(&x) {
                u8::try_from((x + y) % 256).unwrap_or(0)
            } else {
                250
            };
            Rgba([shade, shade, shade, 255])
        })
    }

    /// One pixel a module, the smallest a code can be drawn.
    #[test]
    fn at_its_own_size() {
        assert_eq!(decode(&qr(TOTP, 1)), [TOTP]);
        assert_eq!(decode(&qr(MIGRATION, 1)), [MIGRATION]);
    }

    #[test]
    fn three_times_larger() {
        for text in [TOTP, MIGRATION] {
            let code = qr(text, 1);
            let large = imageops::resize(
                &code,
                code.width() * 3,
                code.height() * 3,
                FilterType::Nearest,
            );
            assert_eq!(decode(&large), [text]);
        }
    }

    #[test]
    fn white_on_black() {
        for text in [TOTP, MIGRATION] {
            assert_eq!(decode(&inverted(qr(text, 4))), [text]);
        }
    }

    #[test]
    fn in_a_screenshot() {
        let mut screen = screenshot(1440, 900);
        imageops::replace(&mut screen, &qr(TOTP, 4), 913, 377);
        assert_eq!(decode(&screen), [TOTP]);
    }

    #[test]
    fn two_codes_in_a_screenshot() {
        let mut screen = screenshot(1440, 900);
        imageops::replace(&mut screen, &qr(TOTP, 4), 760, 60);
        imageops::replace(&mut screen, &qr(MIGRATION, 3), 1050, 520);
        let mut found = decode(&screen);
        found.sort();
        assert_eq!(found, [MIGRATION, TOTP]);
    }

    /// A page in dark mode, captured on a Retina display: the whole
    /// screenshot at twice the size, light code on dark.
    #[test]
    fn in_a_dark_retina_screenshot() {
        let mut screen = screenshot(1440, 900);
        imageops::replace(&mut screen, &qr(MIGRATION, 4), 801, 433);
        let screen = inverted(screen);
        let retina = imageops::resize(&screen, 2880, 1800, FilterType::Nearest);
        assert!(at_each_size(&grey(&retina)).is_empty());
        assert_eq!(decode(&retina), [MIGRATION]);
    }

    /// A dithered screenshot: every other pixel off by a half, light and
    /// dark. Unreadable pixel by pixel, read once halved.
    #[test]
    fn in_a_dithered_screenshot() {
        let mut screen = RgbaImage::from_pixel(1200, 800, Rgba([250, 250, 250, 255]));
        imageops::replace(&mut screen, &qr(TOTP, 8), 400, 120);
        for (x, y, pixel) in screen.enumerate_pixels_mut() {
            if (x + y) % 2 == 1 {
                let shade = if pixel.0[0] > 127 { 130 } else { 120 };
                *pixel = Rgba([shade, shade, shade, 255]);
            }
        }
        assert!(read(&grey(&screen)).is_empty());
        assert_eq!(decode(&screen), [TOTP]);
    }

    #[test]
    fn nothing_to_find() {
        assert!(decode(&screenshot(800, 600)).is_empty());
        assert!(decode(&RgbaImage::new(0, 0)).is_empty());
        assert!(decode(&RgbaImage::new(1, 1)).is_empty());
    }

    #[test]
    fn transparent_is_white() {
        let clear = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));
        assert_eq!(grey(&clear).get_pixel(0, 0).0, [255]);
        let black = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        assert_eq!(grey(&black).get_pixel(0, 0).0, [0]);
    }
}
