//! The extension, the host and the app together, in one process: the app's
//! listener on a socket of the test's own, the host's relay on pipes the
//! test writes to and reads from as the browser would.

use std::io::PipeWriter;
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use authwing_bridge::endpoint::Endpoint;
use authwing_bridge::framing::{self, LIMIT};
use authwing_bridge::host::relay;
use authwing_bridge::listener::{Event, Link, Listener};
use authwing_bridge::protocol::{Caller, FromExtension, NOT_RUNNING, PROTOCOL, ToExtension};
use serde_json::json;

const WAIT: Duration = Duration::from_secs(10);

/// An endpoint no other test uses.
fn endpoint(name: &str) -> (Option<tempfile::TempDir>, Endpoint) {
    #[cfg(unix)]
    {
        // Short: a socket's path may have at most 104 bytes on macOS.
        let dir = tempfile::Builder::new()
            .prefix("aw")
            .tempdir()
            .expect("temporary folder");
        let endpoint = Endpoint::socket(dir.path().join(format!("{name}.sock")));
        (Some(dir), endpoint)
    }
    #[cfg(windows)]
    {
        let pipe = format!("authwing-bridge-test-{name}-{}", std::process::id());
        (None, Endpoint::pipe(pipe))
    }
}

/// The browser's end of a running host.
struct Browser {
    to_host: Option<PipeWriter>,
    from_host: Receiver<ToExtension>,
    host: JoinHandle<std::io::Result<()>>,
}

impl Browser {
    fn start(endpoint: &Endpoint) -> Browser {
        let (stdin, to_host) = std::io::pipe().expect("pipe");
        let (mut from_host_pipe, stdout) = std::io::pipe().expect("pipe");
        let caller = Caller::Chromium {
            extension_id: "abcdefghijklmnopabcdefghijklmnop".into(),
        };
        let endpoint = endpoint.clone();
        let host = thread::spawn(move || relay(caller, &endpoint, stdin, stdout));
        let (tx, from_host) = mpsc::channel();
        thread::spawn(move || {
            while let Ok(Some(message)) = framing::read_json(&mut from_host_pipe, LIMIT) {
                if tx.send(message).is_err() {
                    break;
                }
            }
        });
        Browser {
            to_host: Some(to_host),
            from_host,
            host,
        }
    }

    fn send(&mut self, message: serde_json::Value) {
        let to_host = self.to_host.as_mut().expect("open");
        framing::write_json(to_host, &message, LIMIT).expect("sent");
    }

    fn receive(&self) -> ToExtension {
        self.from_host
            .recv_timeout(WAIT)
            .expect("a message from the host")
    }

    /// Disconnects, as the browser does when the extension's port closes;
    /// the host ends.
    fn close(mut self) {
        drop(self.to_host.take());
        self.host
            .join()
            .expect("host ran")
            .expect("host ended well");
    }
}

fn listen(endpoint: &Endpoint) -> (Listener, Receiver<Event>) {
    let (tx, events) = mpsc::channel();
    let listener = Listener::bind(endpoint.clone(), "0.1.0", move |event| {
        let _ = tx.send(event);
    })
    .expect("listening");
    (listener, events)
}

fn hello() -> serde_json::Value {
    json!({"type": "hello", "protocol": PROTOCOL + 1, "extension_version": "0.1.0", "browser": "chrome"})
}

fn next(events: &Receiver<Event>) -> Event {
    events.recv_timeout(WAIT).expect("an event")
}

fn message(events: &Receiver<Event>) -> (Link, FromExtension) {
    match next(events) {
        Event::Message(link, message) => (link, message),
        other => panic!("not a message: {other:?}"),
    }
}

#[test]
fn hello_tab_fill() {
    let (_dir, endpoint) = endpoint("e2e");
    let (_listener, events) = listen(&endpoint);
    let mut browser = Browser::start(&endpoint);

    let Event::Linked(link) = next(&events) else {
        panic!("the host links first");
    };
    assert_eq!(
        link.caller(),
        &Caller::Chromium {
            extension_id: "abcdefghijklmnopabcdefghijklmnop".into()
        }
    );

    browser.send(hello());
    // The lower version of the two.
    assert_eq!(
        browser.receive(),
        ToExtension::Welcome {
            protocol: PROTOCOL,
            app_version: "0.1.0".into()
        }
    );
    let (_, said) = message(&events);
    assert!(matches!(said, FromExtension::Hello { protocol, .. } if protocol == PROTOCOL + 1));
    assert_eq!(link.protocol(), Some(PROTOCOL));

    // A newer extension's message is skipped, not an error.
    browser.send(json!({"type": "passkey_request", "tab_id": 4}));
    browser.send(json!({
        "type": "tab",
        "tab_id": 4,
        "url": "https://github.com/sessions/two-factor/app",
        "title": "GitHub",
        "has_code_field": true,
        "favicon": "ignored"
    }));
    let (link, said) = message(&events);
    assert_eq!(
        said,
        FromExtension::Tab {
            tab_id: 4,
            url: "https://github.com/sessions/two-factor/app".into(),
            title: "GitHub".into(),
            frame_urls: None,
            has_code_field: true
        }
    );

    let fill = ToExtension::Fill {
        tab_id: 4,
        url: "https://github.com/sessions/two-factor/app".into(),
        code: "492039".into(),
    };
    link.send(&fill).expect("sent");
    assert_eq!(browser.receive(), fill);

    browser.close();
    // On Windows the host's connection to the app lasts until its process
    // ends (see `host::relay`), which here is the test's.
    #[cfg(unix)]
    assert!(matches!(next(&events), Event::Unlinked(gone) if gone.id() == link.id()));
}

#[test]
fn app_not_running_then_started() {
    let (_dir, endpoint) = endpoint("late");
    let mut browser = Browser::start(&endpoint);
    browser.send(hello());
    assert_eq!(
        browser.receive(),
        ToExtension::Status {
            linked: false,
            message: Some(NOT_RUNNING.into())
        }
    );

    // The host finds the app once it runs, and says the extension's hello
    // again, so the app welcomes it.
    let (_listener, events) = listen(&endpoint);
    assert!(matches!(next(&events), Event::Linked(_)));
    assert!(matches!(message(&events).1, FromExtension::Hello { .. }));
    assert!(matches!(browser.receive(), ToExtension::Welcome { .. }));

    browser.send(json!({"type": "tab_closed", "tab_id": 9}));
    assert_eq!(message(&events).1, FromExtension::TabClosed { tab_id: 9 });
    browser.close();
}

#[test]
fn one_app_at_a_time() {
    let (_dir, endpoint) = endpoint("twice");
    let (first, _events) = listen(&endpoint);
    let second = Listener::bind(endpoint.clone(), "0.1.0", |_| {});
    assert_eq!(
        second.err().map(|e| e.kind()),
        Some(std::io::ErrorKind::AddrInUse)
    );
    // Once the first stops, its name can be taken again (soon: Windows may
    // keep a pipe a moment to flush it).
    drop(first);
    let again = (0..20).find_map(|_| {
        let bound = Listener::bind(endpoint.clone(), "0.1.0", |_| {});
        if bound.is_err() {
            thread::sleep(Duration::from_millis(50));
        }
        bound.ok()
    });
    assert!(again.is_some());
}

#[cfg(unix)]
#[test]
fn socket_is_the_users_alone() {
    use std::os::unix::fs::PermissionsExt;
    let (_dir, endpoint) = endpoint("mode");
    let (_listener, _events) = listen(&endpoint);
    let mode = std::fs::metadata(endpoint.path())
        .expect("there")
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

/// An app that ended without removing its socket does not keep the next
/// from listening.
#[cfg(unix)]
#[test]
fn stale_socket_is_taken_over() {
    let (_dir, endpoint) = endpoint("stale");
    drop(std::os::unix::net::UnixListener::bind(endpoint.path()).expect("bound"));
    assert!(endpoint.path().exists());
    let (_listener, events) = listen(&endpoint);
    let mut browser = Browser::start(&endpoint);
    browser.send(hello());
    assert!(matches!(browser.receive(), ToExtension::Welcome { .. }));
    assert!(matches!(next(&events), Event::Linked(_)));
    browser.close();
}
