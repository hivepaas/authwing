//! The camera: its frames, each read for QR codes, until stopped. For a QR
//! code that is not on this computer's screen: on a phone moving its codes
//! out (Google Authenticator's export), on another computer, on paper.
//!
//! A thread of its own opens the camera (AVFoundation on macOS, Media
//! Foundation on Windows, V4L2 on Linux, through `nokhwa`) and, for each
//! frame it reads, calls back with a small picture of it to show, and the
//! texts of the QR codes in it. The camera is on from [`open`] until the
//! [`Camera`] is dropped, and nothing it sees is kept.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use image::RgbaImage;
use image::imageops::{self, FilterType};
use nokhwa::pixel_format::RgbAFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};

use crate::{Error, decode};

/// The width frames are read at, at most: a code held up to a camera is
/// large in its frame, and a smaller frame is read sooner.
const READ_WIDTH: u32 = 960;

/// The preview's width, at most.
pub const PREVIEW_WIDTH: u32 = 480;

/// How long the system's question about the camera may wait for the user.
#[cfg(target_os = "macos")]
const ASKING: std::time::Duration = std::time::Duration::from_secs(120);

/// The camera, on until this is dropped.
pub struct Camera {
    stop: Arc<AtomicBool>,
}

impl Drop for Camera {
    /// Stops it: the thread lets the camera go after the frame it waits for.
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// What the camera saw.
pub enum Seen {
    /// A frame: a small picture of it, mirrored as a mirror shows the user,
    /// and the texts of the QR codes in it.
    Frame {
        preview: RgbaImage,
        texts: Vec<String>,
    },
    /// The camera could not be used, or stopped giving frames.
    Failed(Error),
}

/// The cameras there are, by name, in the order [`open`] takes them.
pub fn cameras() -> Vec<String> {
    nokhwa::query(ApiBackend::Auto)
        .map(|found| found.into_iter().map(|c| c.human_name()).collect())
        .unwrap_or_default()
}

/// Opens camera `index` (of [`cameras`]) and calls `seen` for each frame
/// read, on the camera's thread, until the [`Camera`] is dropped. On macOS
/// the system asks the user first, the first time.
pub fn open(index: usize, seen: impl FnMut(Seen) + Send + 'static) -> Result<Camera, Error> {
    let stop = Arc::new(AtomicBool::new(false));
    let running = stop.clone();
    let mut seen = seen;
    std::thread::Builder::new()
        .name("authwing-camera".into())
        .spawn(move || {
            if let Err(e) = run(index, &running, &mut seen)
                && !running.load(Ordering::Relaxed)
            {
                seen(Seen::Failed(e));
            }
        })
        .map_err(|e| Error::Capture(e.to_string()))?;
    Ok(Camera { stop })
}

fn run(index: usize, stop: &AtomicBool, seen: &mut impl FnMut(Seen)) -> Result<(), Error> {
    allowed()?;
    let index = u32::try_from(index).map_err(|_| Error::Capture("no such camera".into()))?;
    let failed = |e: nokhwa::NokhwaError| Error::Capture(e.to_string());
    let format = RequestedFormat::new::<RgbAFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = nokhwa::Camera::new(CameraIndex::Index(index), format).map_err(failed)?;
    camera.open_stream().map_err(failed)?;
    while !stop.load(Ordering::Relaxed) {
        let frame = camera.frame().map_err(failed)?;
        // A frame that does not decode (a dropped one) is passed over.
        let Ok(rgba) = frame.decode_image::<RgbAFormat>() else {
            continue;
        };
        let read = fit(&rgba, READ_WIDTH);
        let texts = decode(&read);
        let mut preview = fit(&read, PREVIEW_WIDTH);
        imageops::flip_horizontal_in_place(&mut preview);
        if stop.load(Ordering::Relaxed) {
            break;
        }
        seen(Seen::Frame { preview, texts });
    }
    let _ = camera.stop_stream();
    Ok(())
}

/// Whether the system lets authwing use the camera: on macOS, asking the
/// user the first time, and waiting for the answer.
fn allowed() -> Result<(), Error> {
    #[cfg(target_os = "macos")]
    {
        let (answer, answered) = std::sync::mpsc::channel();
        nokhwa::nokhwa_initialize(move |granted| {
            let _ = answer.send(granted);
        });
        return match answered.recv_timeout(ASKING) {
            Ok(true) => Ok(()),
            _ => Err(Error::NotAllowed),
        };
    }
    #[allow(unreachable_code)]
    Ok(())
}

/// `image` made no wider than `width`, its proportions kept.
fn fit(image: &RgbaImage, width: u32) -> RgbaImage {
    if image.width() <= width {
        return image.clone();
    }
    let height = (u64::from(image.height()) * u64::from(width) / u64::from(image.width())).max(1);
    imageops::resize(
        image,
        width,
        u32::try_from(height).unwrap_or(1),
        FilterType::Triangle,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_are_made_no_wider_than_asked() {
        let wide = RgbaImage::new(1920, 1080);
        let read = fit(&wide, READ_WIDTH);
        assert_eq!((read.width(), read.height()), (960, 540));
        let small = RgbaImage::new(640, 480);
        assert_eq!(fit(&small, READ_WIDTH).dimensions(), (640, 480));
    }

    /// A code held up to the camera, in a frame of its size: read.
    #[test]
    fn a_code_in_a_frame_is_read() {
        let code = crate::tests::qr(crate::tests::TOTP, 8);
        let mut frame = RgbaImage::from_pixel(1280, 720, image::Rgba([200, 200, 200, 255]));
        imageops::overlay(&mut frame, &code, 400, 100);
        let read = fit(&frame, READ_WIDTH);
        assert_eq!(decode(&read), [crate::tests::TOTP]);
    }
}
