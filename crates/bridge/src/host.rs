//! The native messaging host: authwing itself, started by the browser, which
//! passes messages between the extension (on its stdin and stdout) and the
//! running app (on the [`Endpoint`]).
//!
//! A host manifest cannot give the program arguments of its own, so the
//! host is known by the ones the browser gives: Chrome, Edge and the other
//! Chromium browsers pass the extension's origin, `chrome-extension://<id>/`
//! (and on Windows `--parent-window=<n>` after it); Firefox passes the path
//! of the host manifest and the extension's id.
//! <https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging#native-messaging-host-protocol>
//! <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging#app_side>
//! `authwing --native-host` starts it by hand, to try it.
//!
//! The browser starts one host for each connection the extension makes and
//! ends it when the extension disconnects. The host does not end when the
//! app is not running, or quits: it tells the extension so (a `status` that
//! is not linked) and tries the app again every few seconds, saying the
//! extension's last `hello` again when it finds it, so the app answers with
//! a `welcome` as if the extension had just connected. The extension's
//! other messages, while there is no app, are dropped: the extension sends
//! the tab again on `welcome`.

use std::ffi::OsString;
use std::io::{self, ErrorKind, Read, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use interprocess::local_socket::Stream;
use interprocess::local_socket::traits::Stream as _;

use crate::endpoint::Endpoint;
use crate::framing::{self, LIMIT};
use crate::install::FIREFOX_EXTENSION_ID;
use crate::protocol::{Caller, HOST_NAME, HostHello, NOT_RUNNING, ToExtension};

/// The flag that starts the host by hand.
pub const NATIVE_HOST_FLAG: &str = "--native-host";

/// How often the host looks for the app while it is not there.
const RETRY: Duration = Duration::from_secs(2);

/// How soon the thread reading from the app sees it is no longer wanted.
const POLL: Duration = Duration::from_millis(250);

/// Whether authwing was started as the host, by a browser or with
/// `--native-host`; `args` are the arguments after the program's name.
/// `main` asks this first, before anything writes to stdout, which is the
/// extension's.
pub fn is_native_host_invocation(args: &[OsString]) -> bool {
    args.first().is_some_and(|a| a == NATIVE_HOST_FLAG) || caller(args).is_some()
}

/// The browser that started the host, from `args` (after the program's
/// name, and after `--native-host` if given).
pub fn caller(args: &[OsString]) -> Option<Caller> {
    let args = match args.first() {
        Some(flag) if flag == NATIVE_HOST_FLAG => &args[1..],
        _ => args,
    };
    let first = args.first()?.to_str()?;
    if let Some(rest) = first.strip_prefix("chrome-extension://") {
        let id = rest.trim_end_matches('/');
        if !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase()) {
            return Some(Caller::Chromium {
                extension_id: id.into(),
            });
        }
        return None;
    }
    let id = args.get(1)?.to_str()?;
    let manifest = Path::new(first).file_name()?;
    if manifest == format!("{HOST_NAME}.json").as_str() || id == FIREFOX_EXTENSION_ID {
        return Some(Caller::Firefox {
            extension_id: id.into(),
        });
    }
    None
}

/// Runs the host on stdin and stdout until the extension disconnects; what
/// the process should exit with.
pub fn run_native_host(args: &[OsString]) -> i32 {
    let caller = caller(args).unwrap_or(Caller::Unknown);
    let endpoint = match Endpoint::for_user() {
        Ok(endpoint) => endpoint,
        Err(e) => {
            eprintln!("authwing bridge: {e}");
            return 1;
        }
    };
    match relay(caller, &endpoint, io::stdin(), io::stdout().lock()) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("authwing bridge: {e}");
            1
        }
    }
}

/// What the relay waits on.
enum Event {
    /// A message from the extension; `None` once it is gone.
    Extension(Option<Vec<u8>>),
    /// A message from the app, on the connection with that number; `None`
    /// once it is gone.
    App(u64, Option<Vec<u8>>),
}

/// Passes messages between the extension, on `input` and `output`, and the
/// app on `endpoint`, until `input` ends. The host's whole work, apart from
/// the process's own stdio, so it can run in tests. (On Windows the
/// connection to the app outlives it, until the process ends: see `App`.)
pub fn relay(
    caller: Caller,
    endpoint: &Endpoint,
    input: impl Read + Send + 'static,
    output: impl Write,
) -> io::Result<()> {
    let (events, inbox) = mpsc::channel();
    thread::Builder::new().name("bridge-stdin".into()).spawn({
        let events = events.clone();
        move || read_extension(input, &events)
    })?;
    let mut relay = Relay {
        endpoint,
        hello: HostHello {
            caller,
            host_version: env!("CARGO_PKG_VERSION").into(),
        },
        extension_hello: None,
        app: None,
        generation: 0,
        next_try: Instant::now(),
        events,
        output,
        told_not_running: false,
    };
    relay.link();
    loop {
        // Messages that keep coming do not put off looking for the app.
        if relay.looking() && Instant::now() >= relay.next_try {
            relay.link();
        }
        let wait = if relay.looking() {
            relay.next_try.saturating_duration_since(Instant::now())
        } else {
            RETRY
        };
        let event = match inbox.recv_timeout(wait) {
            Ok(event) => event,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        match event {
            Event::Extension(None) => return Ok(()),
            Event::Extension(Some(bytes)) => relay.pass_on(bytes)?,
            Event::App(from, message) if from == relay.generation => match message {
                Some(bytes) => framing::write_message(&mut relay.output, &bytes, LIMIT)?,
                None => {
                    relay.app = None;
                    relay.not_running()?;
                }
            },
            // From a connection to the app given up already.
            Event::App(..) => {}
        }
    }
}

struct Relay<'a, W> {
    endpoint: &'a Endpoint,
    hello: HostHello,
    /// The extension's last hello, to say again to an app found later.
    extension_hello: Option<Vec<u8>>,
    app: Option<App>,
    /// Which connection to the app is the current one, so the end of one
    /// given up is not taken for the end of the next.
    generation: u64,
    /// When to look for the app again, while it is not there.
    next_try: Instant,
    events: Sender<Event>,
    output: W,
    /// Whether the extension knows the app is not there.
    told_not_running: bool,
}

impl<W: Write> Relay<'_, W> {
    /// Connects to the app if it is there, saying who started the host and
    /// the extension's hello.
    fn link(&mut self) {
        self.generation += 1;
        self.next_try = Instant::now() + RETRY;
        self.app = self.connect().ok();
        if self.app.is_some() {
            self.told_not_running = false;
        }
    }

    /// Whether there is an app to look for: the extension said hello, and
    /// there is none.
    fn looking(&self) -> bool {
        self.app.is_none() && self.extension_hello.is_some()
    }

    fn connect(&self) -> io::Result<App> {
        let stream = Arc::new(self.endpoint.connect()?);
        framing::write_json(&mut &*stream, &self.hello, LIMIT)?;
        if let Some(bytes) = &self.extension_hello {
            framing::write_message(&mut &*stream, bytes, LIMIT)?;
        }
        match stream.set_recv_timeout(Some(POLL)) {
            // Named pipes have none: see `App`.
            Err(e) if e.kind() == ErrorKind::Unsupported => {}
            done => done?,
        }
        let app = App {
            stream: stream.clone(),
            dropped: Arc::new(AtomicBool::new(false)),
        };
        let dropped = app.dropped.clone();
        let events = self.events.clone();
        let generation = self.generation;
        thread::Builder::new()
            .name("bridge-app".into())
            .spawn(move || read_app(&stream, &dropped, generation, &events))?;
        Ok(app)
    }

    fn pass_on(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        let is_hello = message_type(&bytes).as_deref() == Some("hello");
        if is_hello {
            self.extension_hello = Some(bytes.clone());
        }
        if let Some(app) = &self.app
            && framing::write_message(&mut &*app.stream, &bytes, LIMIT).is_err()
        {
            self.app = None;
        }
        if self.app.is_none() && is_hello {
            // Says the hello too.
            self.link();
        }
        if self.app.is_none() && (is_hello || !self.told_not_running) {
            self.not_running()?;
        }
        Ok(())
    }

    /// Tells the extension the app is not there.
    fn not_running(&mut self) -> io::Result<()> {
        self.told_not_running = true;
        let status = ToExtension::Status {
            linked: false,
            message: Some(NOT_RUNNING.into()),
        };
        framing::write_json(&mut self.output, &status, LIMIT)
    }
}

/// A message's `"type"`, without decoding the rest.
fn message_type(bytes: &[u8]) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Typed {
        r#type: String,
    }
    serde_json::from_slice::<Typed>(bytes)
        .ok()
        .map(|t| t.r#type)
}

fn read_extension(mut input: impl Read, events: &Sender<Event>) {
    loop {
        let message = framing::read_message(&mut input, LIMIT).unwrap_or_else(|e| {
            eprintln!("authwing bridge: reading from the browser failed: {e}");
            None
        });
        let end = message.is_none();
        if events.send(Event::Extension(message)).is_err() || end {
            return;
        }
    }
}

/// The host's connection to the app. Its reader thread holds the stream
/// too, and only it can close it (a pipe cannot be shut while another
/// thread reads it), so dropping this tells the thread to stop, which it
/// sees at its next read timeout. Windows' named pipes have no timeouts:
/// there the connection lasts until the app ends it or the process ends,
/// which for the host comes right after the relay.
struct App {
    stream: Arc<Stream>,
    dropped: Arc<AtomicBool>,
}

impl Drop for App {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

/// A stream read with a timeout, read again on each until it is dropped:
/// a message is never cut where a timeout fell.
struct UntilDropped<'a> {
    stream: &'a Stream,
    dropped: &'a AtomicBool,
}

impl Read for UntilDropped<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match (&mut &*self.stream).read(buf) {
                Err(e)
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut)
                        && !self.dropped.load(Ordering::Acquire) => {}
                read => return read,
            }
        }
    }
}

fn read_app(stream: &Stream, dropped: &AtomicBool, generation: u64, events: &Sender<Event>) {
    let mut reader = UntilDropped { stream, dropped };
    loop {
        let message = match framing::read_message(&mut reader, LIMIT) {
            Ok(message) => message,
            Err(_) if dropped.load(Ordering::Acquire) => return,
            Err(e) => {
                if e.kind() != ErrorKind::UnexpectedEof {
                    eprintln!("authwing bridge: reading from the app failed: {e}");
                }
                None
            }
        };
        let end = message.is_none();
        if events.send(Event::App(generation, message)).is_err() || end {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn chromium_caller() {
        let id = "abcdefghijklmnopabcdefghijklmnop";
        let origin = format!("chrome-extension://{id}/");
        let expected = Some(Caller::Chromium {
            extension_id: id.into(),
        });
        assert_eq!(caller(&args(&[&origin])), expected);
        assert_eq!(caller(&args(&[&origin, "--parent-window=1234"])), expected);
        assert_eq!(caller(&args(&[NATIVE_HOST_FLAG, &origin])), expected);
        assert!(is_native_host_invocation(&args(&[&origin])));
        assert_eq!(caller(&args(&["chrome-extension://../"])), None);
    }

    #[test]
    fn firefox_caller() {
        let manifest = "/Users/me/Library/Application Support/Mozilla/NativeMessagingHosts/dev.authwing.bridge.json";
        let expected = Some(Caller::Firefox {
            extension_id: FIREFOX_EXTENSION_ID.into(),
        });
        assert_eq!(caller(&args(&[manifest, FIREFOX_EXTENSION_ID])), expected);
        assert_eq!(
            caller(&args(&[
                r"C:\Users\me\AppData\Local\authwing\NativeMessagingHosts\firefox\dev.authwing.bridge.json",
                FIREFOX_EXTENSION_ID
            ])),
            expected
        );
        assert!(is_native_host_invocation(&args(&[
            manifest,
            FIREFOX_EXTENSION_ID
        ])));
    }

    #[test]
    fn not_the_host() {
        assert!(!is_native_host_invocation(&args(&[])));
        assert!(!is_native_host_invocation(&args(&["--help"])));
        assert!(!is_native_host_invocation(&args(&["notes.json", "other"])));
        assert!(is_native_host_invocation(&args(&[NATIVE_HOST_FLAG])));
        assert_eq!(caller(&args(&[NATIVE_HOST_FLAG])), None);
    }

    #[test]
    fn type_without_the_rest() {
        assert_eq!(
            message_type(br#"{"type":"hello","protocol":9,"new":{}}"#).as_deref(),
            Some("hello")
        );
        assert_eq!(message_type(b"[]"), None);
    }
}
