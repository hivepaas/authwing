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

/// A text field's box: the field sunk in the page.
pub fn input_box(input: &gpui::Entity<crate::input::TextInput>) -> gpui::Div {
    div()
        .h(px(32.))
        .px(px(10.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .bg(rgb(colors().field))
        .border_1()
        .border_color(rgb(colors().field_edge))
        .child(input.clone())
}

/// A field with its label above.
pub fn labeled(label: String, input: &gpui::Entity<crate::input::TextInput>) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .text_size(crate::look::ui(12.))
                .text_color(rgb(colors().label))
                .child(label),
        )
        .child(input_box(input))
}

/// One choice of a row of them, the one that is on drawn as chosen.
pub fn choice(id: &str, label: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    let c = colors();
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(|| id.to_string())
        .h(px(28.))
        .px(px(12.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .border_1()
        .cursor_pointer()
        .when(on, |d| {
            d.bg(rgb(c.selected))
                .border_color(rgb(c.accent))
                .text_color(rgb(c.text))
        })
        .when(!on, |d| {
            d.border_color(rgb(c.field_edge))
                .text_color(rgb(c.soft))
                .hover(|s| s.bg(rgb(c.hover)))
        })
        .child(label.to_string())
}

/// A switch, on or off.
pub fn switch(id: &str, on: bool) -> gpui::Stateful<gpui::Div> {
    let c = colors();
    div()
        .id(SharedString::from(id.to_string()))
        .debug_selector(|| id.to_string())
        .w(px(34.))
        .h(px(20.))
        .flex_shrink_0()
        .p(px(2.))
        .flex()
        .rounded_full()
        .cursor_pointer()
        .bg(rgb(if on { c.accent } else { c.chip }))
        .when(on, |d| d.justify_end())
        .child(
            div()
                .size(px(16.))
                .rounded_full()
                .bg(rgb(if on { c.on_accent } else { c.soft })),
        )
}
