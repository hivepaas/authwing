//! The app's end: it listens on the [`Endpoint`], takes each host that
//! connects (one for each browser the extension runs in), and hands the
//! app what the extension says.
//!
//! Each connection has a thread of its own, blocking on reads, so the app's
//! callback runs on those threads: it should pass the [`Event`] on (to a
//! channel the UI reads, say) and return. The listener answers `hello`
//! itself, with the version both then speak, before the app hears of it.

use std::io::{self, ErrorKind};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use interprocess::local_socket::traits::{Listener as _, Stream as _};
use interprocess::local_socket::{self, ListenerOptions, Stream};

use crate::endpoint::{Endpoint, same_user};
use crate::framing::{self, LIMIT};
use crate::protocol::{Caller, FromExtension, HostHello, ToExtension, negotiate};

/// How long a host has to say who started it once connected.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// What the app hears.
#[derive(Debug)]
pub enum Event {
    /// A host connected.
    Linked(Link),
    /// The extension said something; never [`FromExtension::Unknown`].
    Message(Link, FromExtension),
    /// The host went away: the browser closed, or the extension stopped.
    Unlinked(Link),
}

/// One host connected to the app: one browser's extension, to answer.
#[derive(Clone, Debug)]
pub struct Link(Arc<Shared>);

#[derive(Debug)]
struct Shared {
    id: u64,
    caller: Caller,
    host_version: String,
    stream: Stream,
    /// One message at a time, whole, whichever thread sends it.
    sending: Mutex<()>,
    /// The version both speak once the extension said hello; 0 before.
    protocol: AtomicU32,
}

impl Link {
    /// This connection's number, unique while the listener runs.
    pub fn id(&self) -> u64 {
        self.0.id
    }

    /// The browser that started the host, as the browser said.
    pub fn caller(&self) -> &Caller {
        &self.0.caller
    }

    /// The version of authwing that runs as the host.
    pub fn host_version(&self) -> &str {
        &self.0.host_version
    }

    /// The version both sides speak, once the extension said hello.
    pub fn protocol(&self) -> Option<u32> {
        match self.0.protocol.load(Ordering::Acquire) {
            0 => None,
            version => Some(version),
        }
    }

    /// Sends `message` to the extension.
    pub fn send(&self, message: &ToExtension) -> io::Result<()> {
        let framed = framing::frame(&serde_json::to_vec(message)?, LIMIT)?;
        let _one = self.0.sending.lock().unwrap_or_else(|e| e.into_inner());
        io::Write::write_all(&mut &self.0.stream, &framed)?;
        io::Write::flush(&mut &self.0.stream)
    }
}

/// The app listening; it stops listening when dropped. Hosts already
/// connected stay linked until they go.
pub struct Listener {
    endpoint: Endpoint,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Listener {
    /// Listens on `endpoint`, telling extensions the app is `app_version`
    /// and calling `on_event` with what they say. Fails if another authwing
    /// listens there already.
    pub fn bind(
        endpoint: Endpoint,
        app_version: impl Into<String>,
        on_event: impl Fn(Event) + Send + Sync + 'static,
    ) -> io::Result<Listener> {
        let listener = listen(&endpoint)?;
        let stop = Arc::new(AtomicBool::new(false));
        let app_version: Arc<str> = app_version.into().into();
        let on_event: Arc<dyn Fn(Event) + Send + Sync> = Arc::new(on_event);
        let thread = thread::Builder::new()
            .name("bridge-listener".into())
            .spawn({
                let stop = stop.clone();
                move || accept(listener, &stop, app_version, on_event)
            })?;
        Ok(Listener {
            endpoint,
            stop,
            thread: Some(thread),
        })
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Accepting blocks: a connection of our own wakes it to see `stop`.
        // It stays open until the thread has seen it: on Windows a
        // connection closed before the pipe waits for one is passed over
        // (ERROR_NO_DATA), and the wait, and so the join, would go on.
        let wake = self.endpoint.name().and_then(Stream::connect);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        drop(wake);
    }
}

/// The socket or pipe, made as the module [`crate::endpoint`] says.
fn listen(endpoint: &Endpoint) -> io::Result<local_socket::Listener> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
        let path = endpoint.path();
        if let Some(folder) = path.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(folder)?;
            let meta = std::fs::metadata(folder)?;
            if meta.uid() != nix::unistd::geteuid().as_raw() || meta.mode() & 0o022 != 0 {
                return Err(io::Error::new(
                    ErrorKind::PermissionDenied,
                    format!("{} is not the user's alone", folder.display()),
                ));
            }
        }
        // A socket file stays when an app ends without removing it: it is
        // taken over unless an app still answers on it.
        if std::fs::symlink_metadata(path).is_ok() {
            if Stream::connect(endpoint.name()?).is_ok() {
                return Err(io::Error::new(
                    ErrorKind::AddrInUse,
                    "another authwing listens for the extension",
                ));
            }
            std::fs::remove_file(path)?;
        }
        let listener = ListenerOptions::new()
            .name(endpoint.name()?)
            .create_sync()?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }
    #[cfg(windows)]
    {
        use interprocess::os::windows::local_socket::ListenerOptionsExt;
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        // Full access for the pipe's owner (OW), the user who made it, and
        // for no one else; not inherited from elsewhere (P).
        let only_owner = SecurityDescriptor::deserialize(widestring::u16cstr!("D:P(A;;GA;;;OW)"))?;
        ListenerOptions::new()
            .name(endpoint.name()?)
            .security_descriptor(only_owner)
            .create_sync()
            .map_err(|e| {
                if e.kind() == ErrorKind::PermissionDenied {
                    io::Error::new(
                        ErrorKind::AddrInUse,
                        "another authwing listens for the extension",
                    )
                } else {
                    e
                }
            })
    }
}

fn accept(
    listener: local_socket::Listener,
    stop: &AtomicBool,
    app_version: Arc<str>,
    on_event: Arc<dyn Fn(Event) + Send + Sync>,
) {
    let next_id = AtomicU64::new(1);
    loop {
        let stream = listener.accept();
        if stop.load(Ordering::Acquire) {
            return;
        }
        let stream = match stream {
            Ok(stream) => stream,
            Err(e) => {
                eprintln!("authwing bridge: accepting a host failed: {e}");
                // Not to spin if the failure lasts.
                thread::sleep(Duration::from_millis(200));
                continue;
            }
        };
        if let Err(e) = same_user(&stream) {
            eprintln!("authwing bridge: refused a connection: {e}");
            continue;
        }
        let id = next_id.fetch_add(1, Ordering::Relaxed);
        let app_version = app_version.clone();
        let on_event = on_event.clone();
        let spawned = thread::Builder::new()
            .name(format!("bridge-link-{id}"))
            .spawn(move || serve(id, stream, &app_version, &*on_event));
        if let Err(e) = spawned {
            eprintln!("authwing bridge: no thread for a host: {e}");
        }
    }
}

/// One host, until it goes.
fn serve(id: u64, stream: Stream, app_version: &str, on_event: &(dyn Fn(Event) + Send + Sync)) {
    // Not on Windows, where named pipes have no timeouts.
    let _ = stream.set_recv_timeout(Some(HELLO_TIMEOUT));
    let hello: HostHello = match framing::read_json(&mut &stream, LIMIT) {
        Ok(Some(hello)) => hello,
        Ok(None) => return,
        Err(e) => {
            eprintln!("authwing bridge: a host did not say who it is: {e}");
            return;
        }
    };
    let _ = stream.set_recv_timeout(None);
    let link = Link(Arc::new(Shared {
        id,
        caller: hello.caller,
        host_version: hello.host_version,
        stream,
        sending: Mutex::new(()),
        protocol: AtomicU32::new(0),
    }));
    on_event(Event::Linked(link.clone()));
    loop {
        let bytes = match framing::read_message(&mut &link.0.stream, LIMIT) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => break,
            Err(e) => {
                if e.kind() != ErrorKind::UnexpectedEof {
                    eprintln!("authwing bridge: reading from a host failed: {e}");
                }
                break;
            }
        };
        // A message that does not decode is skipped like one of a type not
        // known: the extension may be newer.
        let message = match serde_json::from_slice::<FromExtension>(&bytes) {
            Ok(FromExtension::Unknown) | Err(_) => continue,
            Ok(message) => message,
        };
        if let FromExtension::Hello { protocol, .. } = &message {
            let both = negotiate(*protocol);
            link.0.protocol.store(both, Ordering::Release);
            let welcome = ToExtension::Welcome {
                protocol: both,
                app_version: app_version.into(),
            };
            if link.send(&welcome).is_err() {
                break;
            }
        }
        on_event(Event::Message(link.clone(), message));
    }
    on_event(Event::Unlinked(link));
}
