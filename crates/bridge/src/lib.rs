//! The bridge between authwing and its browser extension.
//!
//! The extension tells the app the exact address of the tab in front, so the
//! app can offer that site's code and refuse a look-alike, and fills in a
//! code the app sends. It keeps no codes and no secrets.
//!
//! ```text
//! extension ──stdio (native messaging)──> authwing as the host ──socket──> authwing, running
//! ```
//!
//! A browser starts a native messaging host as a program of its own, and
//! talks to it on its stdin and stdout; the app is already running, and not
//! started by the browser. So authwing is both: started by the browser it is
//! the host ([`host`]), passing messages through to the running app, which
//! listens for it ([`listener`]). `main` asks
//! [`host::is_native_host_invocation`] first and, if so, calls
//! [`host::run_native_host`] and exits with what it returns.
//!
//! [`protocol`] has the messages, [`framing`] how they are written,
//! [`endpoint`] where the app listens and who may connect, and [`install`]
//! the manifests that tell each browser where the host is.

pub mod endpoint;
pub mod framing;
pub mod host;
pub mod install;
pub mod listener;
pub mod protocol;
