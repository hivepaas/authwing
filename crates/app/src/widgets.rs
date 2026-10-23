//! Buttons, drawn alike everywhere in the window.

use gpui::{FontWeight, SharedString, div, prelude::*, px, rgb};

use crate::colors::colors;

/// A button that is only its label, in `color`.
pub fn text_button(id: &str, label: &str, color: u32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(|| id.to_string())
        .h(px(32.))
        .px(px(12.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .text_color(rgb(color))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(colors().hover)))
        .child(label.to_string())
}

pub fn outline_button(id: &str, label: &str) -> gpui::Stateful<gpui::Div> {
    text_button(id, label, colors().text)
        .px(px(14.))
        .border_1()
        .border_color(rgb(colors().field_edge))
}

/// The one button that does what the place is for.
pub fn primary_button(id: &str, label: &str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(|| id.to_string())
        .h(px(32.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .bg(rgb(colors().accent))
        .hover(|s| s.bg(rgb(colors().accent_hover)))
        .text_color(rgb(colors().on_accent))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .child(label.to_string())
}

/// A square button with an icon (`icons/….svg`) in the title bar.
pub fn icon_button(id: &str, icon: &'static str, color: u32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(|| id.to_string())
        .size(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(rgb(colors().hover)))
        .child(gpui::svg().path(icon).size(px(16.)).text_color(rgb(color)))
}
