//! Pictures in their own colors: the app icon's mark (`icons/app-mark.svg`),
//! the app icon, and shellwing's, which About shows.
//!
//! `img` of an SVG rasterizes it at the SVG's own size (184 units for the
//! mark, twice over) and leaves the GPU to shrink that to the 20 or so
//! points it is shown at, sampling a few pixels of hundreds: the edges come
//! out jagged. These are rasterized here for the pixels they cover instead,
//! as `svg` does the one-color icons.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, DevicePixels, ImageSource, Img, Pixels, RenderImage, SvgRenderer, SvgSize, img, prelude::*,
};

/// An SVG drawn into the binary, shown in its own colors.
pub(crate) struct Picture {
    /// Its name among the app's icons: what `img` falls back to.
    path: &'static str,
    svg: &'static [u8],
}

/// The app icon's mark, without its tile: the title bar's.
pub(crate) const MARK: Picture = Picture {
    path: "icons/app-mark.svg",
    svg: include_bytes!("../assets/icons/app-mark.svg"),
};

/// The app icon, its tile and all: the one the system shows.
pub(crate) const APP_ICON: Picture = Picture {
    path: "icons/authwing.svg",
    svg: include_bytes!("../../../packaging/icons/authwing.svg"),
};

/// shellwing's app icon, from its repository's `packaging/icons`.
pub(crate) const SHELLWING_ICON: Picture = Picture {
    path: "icons/shellwing.svg",
    svg: include_bytes!("../assets/icons/shellwing.svg"),
};

thread_local! {
    /// Each picture by its side in device pixels: rasterized once each, and
    /// the same image every frame, which the GPU keeps.
    static DRAWN: RefCell<HashMap<(&'static str, i32), Arc<RenderImage>>> = RefCell::default();
}

/// The mark, `side` square, in a window of `scale` device pixels a point.
pub(crate) fn mark(side: Pixels, scale: f32, cx: &App) -> Img {
    picture(&MARK, side, scale, cx)
}

/// `picture`, `side` square, in a window of `scale` device pixels a point.
pub(crate) fn picture(picture: &Picture, side: Pixels, scale: f32, cx: &App) -> Img {
    let pixels = ((f32::from(side) * scale).round() as i32).max(1);
    let key = (picture.path, pixels);
    let drawn = DRAWN.with_borrow_mut(|drawn| {
        if let Some(image) = drawn.get(&key) {
            return Some(image.clone());
        }
        let image = rasterize(&cx.svg_renderer(), picture, pixels)?;
        drawn.insert(key, image.clone());
        Some(image)
    });
    // Should it not rasterize (a test checks each does), GPUI's own way
    // stands in.
    match drawn {
        Some(image) => img(ImageSource::Render(image)),
        None => img(picture.path),
    }
    .size(side)
    .flex_shrink_0()
}

/// `picture`, `pixels` device pixels square.
fn rasterize(renderer: &SvgRenderer, picture: &Picture, pixels: i32) -> Option<Arc<RenderImage>> {
    let svg = renderer.parse_svg(picture.svg).ok()?;
    let square = gpui::size(DevicePixels(pixels), DevicePixels(pixels));
    renderer
        .render_parsed(&svg, SvgSize::ExactSize(square))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_picture_is_rasterized_for_its_pixels_with_smooth_edges() {
        let renderer = SvgRenderer::new(Arc::new(()));
        for picture in [&MARK, &APP_ICON, &SHELLWING_ICON] {
            let image = rasterize(&renderer, picture, 40).expect(picture.path);
            assert_eq!(
                image.size(0),
                gpui::size(DevicePixels(40), DevicePixels(40))
            );
            // BGRA: the edges are partly covered, not all or nothing.
            let alphas: Vec<u8> = image.as_bytes(0).expect("pixels")[3..]
                .iter()
                .step_by(4)
                .copied()
                .collect();
            assert!(
                alphas.contains(&0) && alphas.contains(&255),
                "{}",
                picture.path
            );
            assert!(
                alphas.iter().filter(|a| !matches!(a, 0 | 255)).count() > 20,
                "{}",
                picture.path
            );
        }
    }
}
