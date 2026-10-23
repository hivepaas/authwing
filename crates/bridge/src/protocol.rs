//! The messages between the browser extension and the app, as JSON.
//!
//! Each message is a JSON object whose `"type"` says what it is; the fields
//! are snake_case. PROTOCOL.md shows every shape, for the extension's side.
//!
//! The extension and the app update on their own, from different stores, so
//! neither can expect the other to be as new: each says the version it speaks
//! in its first message and both keep to the lower of the two ([`negotiate`]).
//! A message of a type one side does not know is skipped, and fields it does
//! not know are ignored, so a newer side can add both without an older one
//! failing.

use serde::{Deserialize, Serialize};

/// The version of this protocol: one more for each change an older side
/// could not just skip.
pub const PROTOCOL: u32 = 1;

/// The native messaging host's name, as the extension asks for it and as
/// the host manifests are named.
pub const HOST_NAME: &str = "dev.authwing.bridge";

/// What the host tells the extension when the app cannot be reached.
pub const NOT_RUNNING: &str = "authwing is not running";

/// A browser tab's id, as the browser gives it to the extension.
pub type TabId = i64;

/// What the extension sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FromExtension {
    /// First, on each connection: the version it speaks, its own, and the
    /// browser's name as it sees it (shown to the user; [`Caller`] is what
    /// the browser itself said).
    Hello {
        protocol: u32,
        extension_version: String,
        browser: String,
    },
    /// The tab in front changed, or what it shows: its address exactly as
    /// the browser has it, the addresses of its frames where the extension
    /// knows them, and whether the page has a field for a code.
    Tab {
        tab_id: TabId,
        url: String,
        #[serde(default)]
        title: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        frame_urls: Option<Vec<String>>,
        #[serde(default)]
        has_code_field: bool,
    },
    /// The tab is gone.
    TabClosed { tab_id: TabId },
    /// What came of a [`ToExtension::Fill`]; `reason` says why it was not
    /// filled (see PROTOCOL.md for the ones the extension sends).
    Filled {
        tab_id: TabId,
        ok: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// A type this version does not know: skipped.
    #[serde(other, skip_serializing)]
    Unknown,
}

/// What the app sends; the host sends [`ToExtension::Status`] too.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToExtension {
    /// The answer to [`FromExtension::Hello`]: the version both now speak
    /// and the app's.
    Welcome { protocol: u32, app_version: String },
    /// Fill `code` into the tab, if it still shows `url`: the address the
    /// app checked the code's account against. The extension fills nothing
    /// if the tab has moved on since.
    Fill {
        tab_id: TabId,
        url: String,
        code: String,
    },
    /// Whether the extension is linked to the app, and why not.
    Status {
        linked: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    /// A type this version does not know: skipped.
    #[serde(other, skip_serializing)]
    Unknown,
}

/// The browser that started the host, as it said on the host's command
/// line. A label, not a proof: any process of the same user could connect
/// to the app and say the same.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "family", rename_all = "snake_case")]
pub enum Caller {
    /// Chrome, Edge, Brave, Vivaldi, Arc, Chromium: the id from the
    /// extension's origin, `chrome-extension://<id>/`.
    Chromium { extension_id: String },
    /// Firefox: the extension's id, as `authwing@hivepaas.com`.
    Firefox { extension_id: String },
    /// Started by hand (`authwing --native-host`), not by a browser.
    Unknown,
}

/// What the host sends the app first, before passing on the extension's
/// messages; only on the socket, never to the extension.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename = "host")]
pub struct HostHello {
    pub caller: Caller,
    /// The authwing that runs as the host, which may not be the app's.
    pub host_version: String,
}

/// The version two sides speak: the lower of theirs and ours.
pub fn negotiate(theirs: u32) -> u32 {
    theirs.min(PROTOCOL)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn from(value: serde_json::Value) -> FromExtension {
        serde_json::from_value(value).expect("decodes")
    }

    #[test]
    fn shapes_from_the_extension() {
        assert_eq!(
            from(
                json!({"type": "hello", "protocol": 1, "extension_version": "0.1.0", "browser": "firefox"})
            ),
            FromExtension::Hello {
                protocol: 1,
                extension_version: "0.1.0".into(),
                browser: "firefox".into()
            }
        );
        assert_eq!(
            from(
                json!({"type": "tab", "tab_id": 7, "url": "https://github.com/login", "title": "Sign in", "has_code_field": true})
            ),
            FromExtension::Tab {
                tab_id: 7,
                url: "https://github.com/login".into(),
                title: "Sign in".into(),
                frame_urls: None,
                has_code_field: true
            }
        );
        assert_eq!(
            from(json!({"type": "tab_closed", "tab_id": 7})),
            FromExtension::TabClosed { tab_id: 7 }
        );
        assert_eq!(
            from(json!({"type": "filled", "tab_id": 7, "ok": false, "reason": "url_changed"})),
            FromExtension::Filled {
                tab_id: 7,
                ok: false,
                reason: Some("url_changed".into())
            }
        );
    }

    #[test]
    fn shapes_to_the_extension() {
        let fill = ToExtension::Fill {
            tab_id: 7,
            url: "https://github.com/login".into(),
            code: "123456".into(),
        };
        assert_eq!(
            serde_json::to_value(&fill).expect("encodes"),
            json!({"type": "fill", "tab_id": 7, "url": "https://github.com/login", "code": "123456"})
        );
        let status = ToExtension::Status {
            linked: false,
            message: Some(NOT_RUNNING.into()),
        };
        assert_eq!(
            serde_json::to_value(&status).expect("encodes"),
            json!({"type": "status", "linked": false, "message": "authwing is not running"})
        );
        let welcome = ToExtension::Welcome {
            protocol: 1,
            app_version: "0.1.0".into(),
        };
        assert_eq!(
            serde_json::to_value(&welcome).expect("encodes"),
            json!({"type": "welcome", "protocol": 1, "app_version": "0.1.0"})
        );
    }

    #[test]
    fn host_hello_shape() {
        let hello = HostHello {
            caller: Caller::Chromium {
                extension_id: "abc".into(),
            },
            host_version: "0.1.0".into(),
        };
        assert_eq!(
            serde_json::to_value(&hello).expect("encodes"),
            json!({"type": "host", "caller": {"family": "chromium", "extension_id": "abc"}, "host_version": "0.1.0"})
        );
    }

    /// A newer side's messages and fields do not break an older one.
    #[test]
    fn newer_messages_are_skipped() {
        assert_eq!(
            from(json!({"type": "passkey", "anything": [1, 2]})),
            FromExtension::Unknown
        );
        assert_eq!(
            from(json!({"type": "tab_closed", "tab_id": 3, "window_id": 9})),
            FromExtension::TabClosed { tab_id: 3 }
        );
        let to: ToExtension =
            serde_json::from_value(json!({"type": "theme", "dark": true})).expect("decodes");
        assert_eq!(to, ToExtension::Unknown);
    }

    #[test]
    fn lower_version_wins() {
        assert_eq!(negotiate(PROTOCOL + 3), PROTOCOL);
        assert_eq!(negotiate(0), 0);
    }
}
