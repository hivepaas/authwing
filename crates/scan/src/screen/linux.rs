//! Linux: the desktop portal under Wayland, the X server under X11.
//!
//! Under XWayland the X server sees only the windows of X apps, the rest
//! black: a Wayland session is captured through the portal, whatever
//! `DISPLAY` says. An X11 session the X server cannot capture, a remote or a
//! nested one, tries the portal after.

use ashpd::desktop::ResponseError;
use ashpd::desktop::screenshot::Screenshot;
use image::{Rgba, RgbaImage};
use x11rb::connection::Connection;
use x11rb::image::{Image, PixelLayout};
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{Screen, Visualid, Visualtype};

use crate::Error;

pub(super) fn displays() -> Result<Vec<RgbaImage>, Error> {
    if wayland() {
        return portal();
    }
    x11().or_else(|error| portal().map_err(|_| error))
}

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|kind| kind == "wayland")
}

/// The portal's screenshot of every display, as one image.
fn portal() -> Result<Vec<RgbaImage>, Error> {
    let shot = async_io::block_on(async {
        Screenshot::request()
            .interactive(false)
            .send()
            .await?
            .response()
    })
    .map_err(|error| match error {
        ashpd::Error::Response(ResponseError::Cancelled) => Error::NotAllowed,
        error => Error::Capture(format!("the desktop portal: {error}")),
    })?;
    let path = url::Url::parse(shot.uri().as_str())
        .ok()
        .and_then(|uri| uri.to_file_path().ok())
        .ok_or_else(|| Error::Capture(format!("the portal's screenshot is at {}", shot.uri())))?;
    let bytes = std::fs::read(&path);
    let _ = std::fs::remove_file(&path);
    let image = bytes
        .map_err(|error| error.to_string())
        .and_then(|bytes| image::load_from_memory(&bytes).map_err(|error| error.to_string()))
        .map_err(|error| Error::Capture(format!("the portal's screenshot: {error}")))?;
    Ok(vec![image.into_rgba8()])
}

/// Each monitor of each X screen, or each screen whole where RandR does not
/// list its monitors.
fn x11() -> Result<Vec<RgbaImage>, Error> {
    let failed = |error: &dyn std::fmt::Display| Error::Capture(format!("the X server: {error}"));
    let (conn, _) = x11rb::connect(None).map_err(|error| failed(&error))?;
    let mut images = Vec::new();
    for screen in &conn.setup().roots {
        let areas = monitors(&conn, screen)
            .unwrap_or_else(|| vec![(0, 0, screen.width_in_pixels, screen.height_in_pixels)]);
        for (x, y, width, height) in areas {
            let (image, visual) =
                Image::get(&conn, screen.root, x, y, width, height).map_err(|e| failed(&e))?;
            let layout = visual_type(screen, visual)
                .and_then(|visual| PixelLayout::from_visual_type(visual).ok())
                .ok_or_else(|| failed(&"a pixel format this does not read"))?;
            images.push(RgbaImage::from_fn(width.into(), height.into(), |x, y| {
                let pixel = image.get_pixel(x as u16, y as u16);
                let (r, g, b) = layout.decode(pixel);
                Rgba([(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8, 255])
            }));
        }
    }
    Ok(images)
}

/// The active monitors RandR lists on `screen`: where each is on the root
/// window, and its size.
fn monitors(conn: &impl Connection, screen: &Screen) -> Option<Vec<(i16, i16, u16, u16)>> {
    let reply = conn
        .randr_get_monitors(screen.root, true)
        .ok()?
        .reply()
        .ok()?;
    let areas: Vec<_> = reply
        .monitors
        .iter()
        .filter(|m| m.width > 0 && m.height > 0)
        .map(|m| (m.x, m.y, m.width, m.height))
        .collect();
    (!areas.is_empty()).then_some(areas)
}

fn visual_type(screen: &Screen, visual: Visualid) -> Option<Visualtype> {
    screen
        .allowed_depths
        .iter()
        .flat_map(|depth| &depth.visuals)
        .find(|candidate| candidate.visual_id == visual)
        .copied()
}
