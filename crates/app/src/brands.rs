//! Brand icons for the codes, from Simple Icons (`assets/brands.tsv.gz`,
//! made by `scripts/brands.py`): each a 24×24 path and the brand's color.
//!
//! A code finds its brand by its service's name (or another name the brand
//! goes by), else by one of its sites (`github` for `gist.github.com`), else
//! by its name's first word ("Google Workspace"). Its tile is the brand's
//! color with the mark on it, white or black, whichever reads; a code with
//! no brand has its initials on a color its name gives.

use std::collections::HashMap;
use std::io::Read as _;
use std::sync::OnceLock;

use authwing_context::Site;

const DATA: &[u8] = include_bytes!("../assets/brands.tsv.gz");

/// Where a brand's mark is served from, as an asset: `brands/<slug>.svg`.
pub const PREFIX: &str = "brands/";

pub struct Brand {
    pub slug: String,
    /// Its color, `0xRRGGBB`.
    pub color: u32,
    path: String,
}

impl Brand {
    /// The asset its mark is drawn from.
    pub fn asset(&self) -> String {
        format!("{PREFIX}{}.svg", self.slug)
    }

    /// The color its mark is drawn in on its own color: white or
    /// near-black, whichever stands out more from it (WCAG's contrast).
    pub fn ink(&self) -> u32 {
        const DARK: u32 = 0x111111;
        let ground = luminance(self.color);
        let against = |ink: u32| {
            let ink = luminance(ink);
            (ground.max(ink) + 0.05) / (ground.min(ink) + 0.05)
        };
        if against(DARK) > against(0xffffff) {
            DARK
        } else {
            0xffffff
        }
    }
}

struct Brands {
    list: Vec<Brand>,
    /// Every name a brand goes by, made a key, to its place in `list`.
    by_name: HashMap<String, usize>,
}

fn brands() -> &'static Brands {
    static BRANDS: OnceLock<Brands> = OnceLock::new();
    BRANDS.get_or_init(|| {
        let mut text = String::new();
        let _ = flate2::read::GzDecoder::new(DATA).read_to_string(&mut text);
        let mut list = Vec::new();
        let mut by_name = HashMap::new();
        for line in text.lines().filter(|l| !l.starts_with('#')) {
            let mut f = line.split('\t');
            let (Some(slug), Some(title), Some(hex), Some(others), Some(path)) =
                (f.next(), f.next(), f.next(), f.next(), f.next())
            else {
                continue;
            };
            let Ok(color) = u32::from_str_radix(hex, 16) else {
                continue;
            };
            let at = list.len();
            // The first brand to take a name keeps it.
            for name in [slug, title].into_iter().chain(others.split('|')) {
                let k = key(name);
                if !k.is_empty() {
                    by_name.entry(k).or_insert(at);
                }
            }
            list.push(Brand {
                slug: slug.to_string(),
                color,
                path: path.to_string(),
            });
        }
        Brands { list, by_name }
    })
}

/// A name as names are matched: lowercase letters and digits only, `+`
/// and `.` said as Simple Icons' slugs say them.
fn key(name: &str) -> String {
    name.to_lowercase()
        .replace('+', "plus")
        .replace('.', "dot")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect()
}

fn named(name: &str) -> Option<&'static Brand> {
    let b = brands();
    b.by_name.get(&key(name)).map(|&at| &b.list[at])
}

/// The brand of a code of the service `issuer` (or, with none, the account
/// `account`), shown first on `sites`.
pub fn find(issuer: &str, account: &str, sites: &[Site]) -> Option<&'static Brand> {
    let name = if issuer.trim().is_empty() {
        // An address's domain names its service: tien@proton.me.
        account
            .split_once('@')
            .map_or("", |(_, d)| d.split('.').next().unwrap_or(""))
    } else {
        issuer
    };
    if let Some(brand) = named(name) {
        return Some(brand);
    }
    let label = |s: &Site| {
        let owner = s.registrable();
        owner
            .split('.')
            .next()
            .filter(|_| owner.contains('.'))
            .map(str::to_string)
    };
    if let Some(brand) = sites.iter().filter_map(label).find_map(|l| named(&l)) {
        return Some(brand);
    }
    // "Google Workspace": Google; a first word too short to be a name is no
    // brand's.
    name.split_whitespace()
        .next()
        .filter(|w| key(w).len() >= 3 && name.split_whitespace().count() > 1)
        .and_then(named)
}

/// The SVG of the brand of `asset` (`brands/<slug>.svg`), drawn in one
/// color as the window's icons are.
pub fn svg(asset: &str) -> Option<Vec<u8>> {
    let slug = asset.strip_prefix(PREFIX)?.strip_suffix(".svg")?;
    let b = brands();
    let brand = b.by_name.get(&key(slug)).map(|&at| &b.list[at])?;
    (brand.slug == slug).then(|| {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="{}"/></svg>"#,
            brand.path
        )
        .into_bytes()
    })
}

/// The color a code with no brand has, by its name: one of a few soft
/// ones, the same on every device, its initials dark on it.
pub fn tint(name: &str) -> u32 {
    const TINTS: [u32; 9] = [
        0xe7ebf0, 0xfdba74, 0x93c5fd, 0xd8b4fe, 0xfbbf24, 0x86efac, 0xfde047, 0xc9d0d9, 0xfca5a5,
    ];
    let sum = name.to_lowercase().bytes().fold(0usize, |s, b| {
        s.wrapping_mul(31).wrapping_add(usize::from(b))
    });
    TINTS[sum % TINTS.len()]
}

/// The ink on a tint.
pub const TINT_INK: u32 = 0x0e1218;

/// A code's tile, `size` pixels square: its brand's mark on the brand's
/// color, else its initials on its tint. A color nearly black or white
/// has an edge, to stand out from the window's.
pub fn tile(brand: Option<&Brand>, title: &str, initials: &str, size: f32) -> gpui::Div {
    use gpui::{FontWeight, div, prelude::*, px, rgb, svg};
    let base = div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px((size * 0.27).round()))
        .flex()
        .items_center()
        .justify_center();
    match brand {
        Some(b) => {
            let l = luminance(b.color);
            base.bg(rgb(b.color))
                .when(!(0.03..=0.85).contains(&l), |d| {
                    d.border_1().border_color(rgb(crate::colors::colors().edge))
                })
                .child(
                    svg()
                        .path(b.asset())
                        .size(px((size * 0.56).round()))
                        .text_color(rgb(b.ink())),
                )
        }
        None => base
            .bg(rgb(tint(title)))
            .text_size(crate::look::ui((size * 0.37).round()))
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(TINT_INK))
            .child(initials.to_string()),
    }
}

/// Relative luminance of `0xRRGGBB`, 0 to 1.
fn luminance(color: u32) -> f64 {
    let channel = |shift: u32| {
        let c = f64::from((color >> shift) & 0xff) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sites(hosts: &[&str]) -> Vec<Site> {
        hosts
            .iter()
            .map(|h| Site::parse(h).expect("site"))
            .collect()
    }

    fn slug(b: Option<&Brand>) -> Option<&str> {
        b.map(|b| b.slug.as_str())
    }

    #[test]
    fn codes_find_their_brands() {
        assert_eq!(slug(find("GitHub", "me", &[])), Some("github"));
        assert_eq!(slug(find("github ", "me", &[])), Some("github"));
        assert_eq!(slug(find("PayPal", "me", &[])), Some("paypal"));
        // By a site, when the name is no brand's.
        assert_eq!(
            slug(find("My work account", "me", &sites(&["gist.github.com"]))),
            Some("github")
        );
        // By the name's first word.
        assert_eq!(slug(find("Google Workspace", "me", &[])), Some("google"));
        // By the account's domain, with no service.
        assert_eq!(slug(find("", "tien@proton.me", &[])), Some("proton"));
        // None: initials on a tint.
        assert!(find("Vietcombank", "tien", &[]).is_none());
        assert!(find("My bank", "me", &[]).is_none());
    }

    #[test]
    fn marks_are_served_and_inked_to_read() {
        let github = find("GitHub", "", &[]).expect("github");
        let drawn = String::from_utf8(svg(&github.asset()).expect("svg")).expect("utf-8");
        assert!(drawn.starts_with("<svg") && drawn.contains("<path d=\"M"));
        assert!(svg("brands/no-such-brand.svg").is_none());
        assert!(svg("icons/github.svg").is_none());
        // GitHub's near-black takes white; a light yellow, black.
        assert_eq!(github.ink(), 0xffffff);
        let binance = find("Binance", "", &[]).expect("binance");
        assert_eq!(binance.ink(), 0x111111);
        assert!(brands().list.len() > 3000);
    }

    #[test]
    fn a_name_has_the_same_tint_everywhere() {
        assert_eq!(tint("Vietcombank"), tint("vietcombank"));
    }
}
