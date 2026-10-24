//! The About page, as shellwing's: the version, where the code is and how
//! to take part, updates, and the author's other projects, shellwing and
//! HivePaaS.

use gpui::{ClipboardItem, Context, FontWeight, div, prelude::*, px, rgb, svg};

use authwing_i18n::tr;

use super::{SettingsEvent, SettingsView, card, heading, hint};
use crate::colors::colors;
use crate::look::ui;
use crate::mark::{self, APP_ICON, SHELLWING_ICON};
use crate::notice::Notice;
use crate::widgets::outline_button;

const SITE: &str = "https://authwing.app";
const REPO: &str = "https://github.com/hivepaas/authwing";
const SHELLWING: &str = "https://shellwing.app";
const SHELLWING_REPO: &str = "https://github.com/hivepaas/shellwing";
const HIVEPAAS: &str = "https://hivepaas.com";
const HIVEPAAS_REPO: &str = "https://github.com/hivepaas/hivepaas";
/// The year in the copyright line.
const YEAR: u32 = 2026;

/// A brand's button: its fill, lighter at the top, the fill under the
/// pointer, and the text's color, the same in either mode.
type Brand = (u32, u32, u32, u32, u32);
/// authwing's: the blue of the icon's wing, the tile's slate on it.
const AUTHWING_BRAND: Brand = (0x7dd3fc, 0x4cc4f9, 0x9ddefd, 0x6ccffa, 0x1c2129);
/// shellwing's: the blue of its logo.
const SHELLWING_BRAND: Brand = (0x38b6f3, 0x0f87c9, 0x5cc6f7, 0x1a95d6, 0xffffff);
/// HivePaaS's: its amber.
const HIVEPAAS_BRAND: Brand = (0xfbbd2a, 0xf49a12, 0xfcca4d, 0xf6a823, 0x0b0c10);

/// What a problem report needs to say about this build.
fn version_info() -> String {
    format!(
        "authwing {} ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// A link to a page of the authwing repository.
fn repo(path: &str) -> String {
    format!("{REPO}/{path}")
}

impl SettingsView {
    /// `scale`: the window's device pixels a point, for the icons.
    pub(super) fn about(&self, scale: f32, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(identity(scale, cx))
            .child(open_source(cx))
            .child(section(
                &tr!("Updates"),
                card().child(hint(tr!(
                    "This build has no release keys, so it cannot tell of updates."
                ))),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(heading(tr!("From the same author")))
                    .child(shellwing(scale, cx))
                    .child(hivepaas(cx)),
            )
            .child(
                div()
                    .pt(px(12.))
                    .border_t_1()
                    .border_color(rgb(colors().line))
                    .child(hint(tr!(
                        "© {year} HivePaaS. The authwing name and logo are trademarks of HivePaaS.",
                        year = YEAR.to_string()
                    ))),
            )
    }
}

/// The icon, the name, what it is and which build this is.
fn identity(scale: f32, cx: &mut Context<SettingsView>) -> gpui::Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(16.))
        .child(mark::picture(&APP_ICON, px(64.), scale, cx))
        .child(
            div()
                .flex_1()
                .min_w(px(220.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(ui(22.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("authwing"),
                )
                .child(
                    div()
                        .text_size(ui(14.))
                        .text_color(rgb(colors().soft))
                        .child(tr!("Two-factor codes that know the window you are in.")),
                )
                .child(
                    div()
                        .pt(px(2.))
                        .text_size(ui(12.))
                        .text_color(rgb(colors().muted))
                        .child(tr!(
                            "Version {version} · {os} {arch}",
                            version = env!("CARGO_PKG_VERSION"),
                            os = std::env::consts::OS,
                            arch = std::env::consts::ARCH
                        )),
                ),
        )
        .child(
            outline_button("about-copy-version", &tr!("Copy version info")).on_click(cx.listener(
                |_, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(version_info()));
                    cx.emit(SettingsEvent::Say(Notice::good(tr!(
                        "Copied: {info}",
                        info = version_info()
                    ))));
                },
            )),
        )
}

/// Where the code is, under which license, and the ways to take part.
fn open_source(cx: &mut Context<SettingsView>) -> gpui::Div {
    let actions = div()
        .flex()
        .flex_wrap()
        .gap(px(8.))
        .child(primary_link(
            "about-site",
            &tr!("Visit authwing.app"),
            SITE,
            AUTHWING_BRAND,
            cx,
        ))
        .child(link_button("about-github", "GitHub", REPO.to_string(), cx))
        .child(link_button(
            "about-issue",
            &tr!("Report a problem"),
            repo("issues/new"),
            cx,
        ))
        .child(link_button(
            "about-releases",
            &tr!("Release notes"),
            repo("releases"),
            cx,
        ));
    section(
        &tr!("Open source"),
        card()
            .child(body_text(tr!(
                "authwing is free and open source, under the GPL-3.0-or-later license. Read the code, build it yourself, report a problem or send a fix."
            )))
            .child(actions),
    )
}

/// shellwing: what it is, what it does, and where to see more.
fn shellwing(scale: f32, cx: &mut Context<SettingsView>) -> gpui::Div {
    let c = colors();
    let features = [
        tr!("SSH as OpenSSH does it"),
        tr!("Tabs and split panes"),
        tr!("SFTP"),
        tr!("Port forwarding"),
        tr!("Encrypted vault"),
        tr!("Sync between devices"),
    ];
    let actions = div()
        .flex()
        .flex_wrap()
        .gap(px(8.))
        .child(primary_link(
            "about-shellwing",
            &tr!("Visit shellwing.app"),
            SHELLWING,
            SHELLWING_BRAND,
            cx,
        ))
        .child(link_button(
            "about-shellwing-github",
            "GitHub",
            SHELLWING_REPO.to_string(),
            cx,
        ));
    card()
        .gap(px(16.))
        .child(project_title(
            mark::picture(&SHELLWING_ICON, px(52.), scale, cx).into_any_element(),
            "shellwing",
            tr!("Open source · GPL-3.0"),
            (c.info_edge, c.info_bg, c.info_text),
            tr!("A fast, native SSH client for macOS, Windows and Linux"),
        ))
        .child(
            div()
                .text_size(ui(14.))
                .text_color(rgb(c.text))
                .child(tr!(
                    "Tabs, split panes, SFTP, port forwarding and an encrypted vault that syncs between your devices, in a GPU-rendered terminal built in Rust."
                )),
        )
        .child(chips(features))
        .child(actions)
}

/// HivePaaS: what it is, where it runs, and where to see more.
fn hivepaas(cx: &mut Context<SettingsView>) -> gpui::Div {
    let c = colors();
    let place = |icon: &'static str, name: String, what: String| {
        div()
            .flex_1()
            .min_w(px(240.))
            .flex()
            .flex_col()
            .gap(px(6.))
            .py(px(12.))
            .px(px(14.))
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(c.line))
            .bg(rgb(c.page))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(svg().path(icon).size(px(16.)).text_color(rgb(c.amber)))
                    .child(name),
            )
            .child(
                div()
                    .text_size(ui(12.5))
                    .text_color(rgb(c.soft))
                    .child(what),
            )
    };
    let places = div()
        .flex()
        .flex_wrap()
        .gap(px(12.))
        .child(place(
            "icons/server.svg",
            tr!("On your servers"),
            tr!(
                "Deploy from Git or a Docker image, with domains and automatic TLS, logs, metrics and scaling across a cluster."
            ),
        ))
        .child(place(
            "icons/laptop.svg",
            tr!("On your own computer"),
            tr!(
                "Install it on your own computer in place of Docker Desktop or Portainer, and still get full app management on top."
            ),
        ));
    let features = [
        tr!("300+ one-click apps"),
        tr!("Deploy on push"),
        tr!("Pull request previews"),
        tr!("Automatic TLS"),
        tr!("Live logs and metrics"),
        tr!("Functions"),
    ];
    let actions = div()
        .flex()
        .flex_wrap()
        .gap(px(8.))
        .child(primary_link(
            "about-hivepaas",
            &tr!("Visit hivepaas.com"),
            HIVEPAAS,
            HIVEPAAS_BRAND,
            cx,
        ))
        .child(link_button(
            "about-hivepaas-github",
            "GitHub",
            HIVEPAAS_REPO.to_string(),
            cx,
        ));
    card()
        .gap(px(16.))
        .child(project_title(
            hivepaas_logo().into_any_element(),
            "HivePaaS",
            tr!("Open source · Apache-2.0"),
            (c.warn_edge, c.warn_bg, c.warn_text),
            tr!("A self-hosted platform for your apps, built on Docker Swarm"),
        ))
        .child(div().text_size(ui(14.)).text_color(rgb(c.text)).child(tr!(
            "An open-source, self-hosted alternative to Heroku, Render and Coolify."
        )))
        .child(places)
        .child(chips(features))
        .child(actions)
}

/// A project's logo, its name with its license beside it in a pill of
/// `(edge, fill, text)`, and a line on what it is.
fn project_title(
    logo: gpui::AnyElement,
    name: &'static str,
    license: String,
    (edge, fill, text): (u32, u32, u32),
    what: String,
) -> gpui::Div {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap(px(14.))
        .child(logo)
        .child(
            div()
                .flex_1()
                .min_w(px(220.))
                .flex()
                .flex_col()
                .gap(px(3.))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(ui(17.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(name),
                        )
                        .child(
                            div()
                                .px(px(8.))
                                .rounded_full()
                                .border_1()
                                .border_color(rgb(edge))
                                .bg(rgb(fill))
                                .text_size(ui(11.))
                                .text_color(rgb(text))
                                .child(license),
                        ),
                )
                .child(
                    div()
                        .text_size(ui(12.5))
                        .text_color(rgb(colors().muted))
                        .child(what),
                ),
        )
}

/// What a project does, a pill each.
fn chips(features: [String; 6]) -> gpui::Div {
    let c = colors();
    div()
        .flex()
        .flex_wrap()
        .gap(px(6.))
        .children(features.map(|f| {
            div()
                .px(px(10.))
                .py(px(2.))
                .rounded_full()
                .bg(rgb(c.chip))
                .text_size(ui(12.))
                .text_color(rgb(c.soft))
                .child(f)
        }))
}

/// HivePaaS's mark, seven cells of a hive in its brown and amber, on a pale
/// tile (the brown does not show on a dark one), as shellwing draws it.
fn hivepaas_logo() -> gpui::Div {
    const BROWN: u32 = 0x543d2b;
    const AMBER: u32 = 0xf59e0b;
    // Centers in the mark's 36-unit box.
    const CELLS: [(f32, f32, u32); 7] = [
        (12.98, 9.30, BROWN),
        (23.02, 9.30, AMBER),
        (7.95, 18.0, BROWN),
        (18.0, 18.0, AMBER),
        (28.05, 18.0, AMBER),
        (12.98, 26.70, BROWN),
        (23.02, 26.70, AMBER),
    ];
    let mark = div()
        .relative()
        .size(px(36.))
        .children(CELLS.map(|(x, y, c)| {
            div()
                .absolute()
                .left(px(x - 4.3))
                .top(px(y - 4.3))
                .size(px(8.6))
                .rounded_full()
                .bg(rgb(c))
        }));
    div()
        .size(px(52.))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(12.))
        .border_1()
        .border_color(rgb(0xd9cbb5))
        .bg(rgb(0xfbf7f0))
        .child(mark)
}

fn section(title: &str, body: gpui::Div) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .child(heading(title.to_string()))
        .child(body)
}

fn body_text(s: String) -> gpui::Div {
    div()
        .text_size(ui(13.))
        .text_color(rgb(colors().soft))
        .child(s)
}

/// A button that opens a web page.
fn link_button(
    id: &'static str,
    text: &str,
    url: String,
    cx: &mut Context<SettingsView>,
) -> gpui::Stateful<gpui::Div> {
    outline_button(id, text).on_click(cx.listener(move |_, _, _, cx| cx.open_url(&url)))
}

/// A fill lighter at the top.
fn brand_fill(top: u32, bottom: u32) -> gpui::Background {
    gpui::linear_gradient(
        180.,
        gpui::linear_color_stop(rgb(top), 0.),
        gpui::linear_color_stop(rgb(bottom), 1.),
    )
}

/// A filled button in a brand's colors that opens a web page: the
/// section's main way on.
fn primary_link(
    id: &'static str,
    text: &str,
    url: &'static str,
    (top, bottom, hover_top, hover_bottom, fg): Brand,
    cx: &mut Context<SettingsView>,
) -> gpui::Stateful<gpui::Div> {
    let hover = brand_fill(hover_top, hover_bottom);
    div()
        .id(id)
        .debug_selector(|| id.into())
        .h(px(32.))
        .px(px(14.))
        .flex()
        .items_center()
        .rounded(px(6.))
        .bg(brand_fill(top, bottom))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(fg))
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(text.to_string())
        .on_click(cx.listener(move |_, _, _, cx| cx.open_url(url)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_info_names_the_build() {
        let info = version_info();
        assert!(info.starts_with(&format!("authwing {} (", env!("CARGO_PKG_VERSION"))));
        assert!(info.ends_with(&format!("{})", std::env::consts::ARCH)));
    }

    #[test]
    fn links_stay_in_the_repository() {
        assert_eq!(
            repo("issues/new"),
            "https://github.com/hivepaas/authwing/issues/new"
        );
    }

    /// The text on authwing's and HivePaaS's buttons reads against both ends
    /// of their fill (WCAG's 4.5:1), under the pointer too. shellwing's is
    /// as shellwing draws it.
    #[test]
    fn brand_buttons_read() {
        fn luminance(color: u32) -> f64 {
            let channel = |shift: u32| {
                let c = f64::from((color >> shift) & 0xff) / 255.;
                if c <= 0.03928 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
        }
        fn contrast(a: u32, b: u32) -> f64 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
        for (top, bottom, hover_top, hover_bottom, fg) in [AUTHWING_BRAND, HIVEPAAS_BRAND] {
            for fill in [top, bottom, hover_top, hover_bottom] {
                assert!(contrast(fill, fg) >= 4.5, "{fg:06x} on {fill:06x}");
            }
        }
    }
}
