//! The browser extension, as the window hears it: the tab in front and its
//! exact address, from a browser that has authwing's extension, and the code
//! typed into it when its page has a field for one (see `authwing_bridge`).

use authwing_bridge::endpoint::Endpoint;
use authwing_bridge::listener::{Event, Link, Listener};
use authwing_bridge::protocol::{FromExtension, ToExtension};

/// Apps that are browsers the extension runs in: on macOS their bundle ids,
/// on Windows their programs, on X11 their window classes. A page is taken
/// for the window's only when the window is one of these.
const BROWSERS: &[&str] = &[
    "com.google.chrome",
    "com.google.chrome.canary",
    "org.chromium.chromium",
    "com.microsoft.edgemac",
    "com.brave.browser",
    "com.vivaldi.vivaldi",
    "company.thebrowser.browser",
    "org.mozilla.firefox",
    "org.mozilla.firefoxdeveloperedition",
    "chrome.exe",
    "msedge.exe",
    "brave.exe",
    "vivaldi.exe",
    "firefox.exe",
    "google-chrome",
    "chromium",
    "chromium-browser",
    "microsoft-edge",
    "brave-browser",
    "vivaldi-stable",
    "firefox",
];

/// Whether `app_id` is a browser the extension runs in.
pub fn is_browser(app_id: &str) -> bool {
    let id = app_id.to_lowercase();
    BROWSERS.contains(&id.as_str())
}

/// The tab in front, as the extension last told it.
#[derive(Clone)]
pub struct Tab {
    pub link: Link,
    pub tab_id: authwing_bridge::protocol::TabId,
    pub url: String,
    pub has_code_field: bool,
}

/// The app's end of the bridge: listening for the extension's host, with
/// what it said handed to the window as events.
pub struct Bridge {
    _listener: Option<Listener>,
}

impl Bridge {
    /// Listens for the extension; `on_event` is called on the listener's
    /// thread. None listens when another authwing does already (its window
    /// gets the extension).
    pub fn start(on_event: impl Fn(Event) + Send + Sync + 'static) -> Bridge {
        let listener = Endpoint::for_user()
            .and_then(|endpoint| Listener::bind(endpoint, env!("CARGO_PKG_VERSION"), on_event));
        if let Err(e) = &listener {
            eprintln!("authwing: not listening for the browser extension: {e}");
        }
        Bridge {
            _listener: listener.ok(),
        }
    }
}

/// The tab an event leaves in front, from the one before.
pub fn tab_after(before: Option<Tab>, event: Event) -> (Option<Tab>, Option<Filled>) {
    match event {
        Event::Message(
            link,
            FromExtension::Tab {
                tab_id,
                url,
                has_code_field,
                ..
            },
        ) => (
            Some(Tab {
                link,
                tab_id,
                url,
                has_code_field,
            }),
            None,
        ),
        Event::Message(link, FromExtension::TabClosed { tab_id }) => (
            before.filter(|t| !(t.link.id() == link.id() && t.tab_id == tab_id)),
            None,
        ),
        Event::Message(_, FromExtension::Filled { ok, reason, .. }) => {
            (before, Some(Filled { ok, reason }))
        }
        Event::Unlinked(link) => (before.filter(|t| t.link.id() != link.id()), None),
        _ => (before, None),
    }
}

/// What came of a code sent to be typed in.
pub struct Filled {
    pub ok: bool,
    pub reason: Option<String>,
}

/// Sends `code` to be typed into `tab`, if it still shows the page checked.
pub fn fill(tab: &Tab, code: &str) -> std::io::Result<()> {
    tab.link.send(&ToExtension::Fill {
        tab_id: tab.tab_id,
        url: tab.url.clone(),
        code: code.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browsers_are_known_by_their_ids() {
        assert!(is_browser("com.google.Chrome"));
        assert!(is_browser("firefox.exe"));
        assert!(is_browser("Firefox"));
        assert!(!is_browser("com.tinyspeck.slackmacgap"));
        assert!(!is_browser("Code"));
    }
}
