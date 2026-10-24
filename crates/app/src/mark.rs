//! The app icon's mark (`icons/app-mark.svg`), in its colors.
//!
//! `img` of an SVG rasterizes it at the SVG's own size (184 units, twice
//! over) and leaves the GPU to shrink that to the 20 or so points it is
//! shown at, sampling a few pixels of hundreds: the edges come out jagged.
//! The mark is rasterized here for the pixels it covers instead, as `svg`
//! does the one-color icons.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui::{
    App, DevicePixels, ImageSource, Img, Pixels, RenderImage, SvgRenderer, SvgSize, img, prelude::*,
};

const PATH: &str = "icons/app-mark.svg";
const SVG: &[u8] = include_bytes!("../assets/icons/app-mark.svg");

thread_local! {
    /// The mark by its side in device pixels: rasterized once each, and the
    /// same image every frame, which the GPU keeps.
    static DRAWN: RefCell<HashMap<i32, Arc<RenderImage>>> = RefCell::default();
}

/// The mark, `side` square, in a window of `scale` device pixels a point.
pub(crate) fn mark(side: Pixels, scale: f32, cx: &App) -> Img {
    let pixels = ((f32::from(side) * scale).round() as i32).max(1);
    let drawn = DRAWN.with_borrow_mut(|drawn| {
        if let Some(image) = drawn.get(&pixels) {
            return Some(image.clone());
        }
        let image = rasterize(&cx.svg_renderer(), pixels)?;
        drawn.insert(pixels, image.clone());
        Some(image)
    });
    // Should it not rasterize (a test checks it does), GPUI's own way
    // stands in.
    match drawn {
        Some(image) => img(ImageSource::Render(image)),
        None => img(PATH),
    }
    .size(side)
    .flex_shrink_0()
}

/// The mark, `pixels` device pixels square.
fn rasterize(renderer: &SvgRenderer, pixels: i32) -> Option<Arc<RenderImage>> {
    let svg = renderer.parse_svg(SVG).ok()?;
    let square = gpui::size(DevicePixels(pixels), DevicePixels(pixels));
    renderer
        .render_parsed(&svg, SvgSize::ExactSize(square))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mark_is_rasterized_for_its_pixels_with_smooth_edges() {
        let image = rasterize(&SvgRenderer::new(Arc::new(())), 40).expect("the mark");
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
        assert!(alphas.contains(&0) && alphas.contains(&255));
        assert!(alphas.iter().filter(|a| !matches!(a, 0 | 255)).count() > 20);
    }
}
