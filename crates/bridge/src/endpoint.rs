//! Where the app listens for the host: a socket file on macOS and Linux, a
//! named pipe on Windows.
//!
//! Only the user may reach it, and that is the bridge's one boundary:
//!
//! - **macOS, Linux**: the socket is `bridge.sock` in a folder of the user's
//!   (`~/Library/Application Support/authwing` on macOS, which `~/Library`'s
//!   `0700` already keeps to the user; `$XDG_RUNTIME_DIR/authwing` on Linux,
//!   or `~/.local/share/authwing` without one). The folder is made `0700`
//!   and must not be writable by others, and the socket is `0600`. Each end
//!   then asks the system who the other is (`LOCAL_PEERCRED`,
//!   `SO_PEERCRED`) and drops it unless it runs as the same user.
//! - **Windows**: the pipe `\\.\pipe\authwing-bridge-<user>`, made so that
//!   only its owner may open it, refusing other machines, and failing to
//!   start if one of that name exists already (so no one can take the name
//!   first unnoticed). The system gives the other end's process id only,
//!   not its user, so the pipe's access list is the check; the host does
//!   not check who made the pipe. An app run as administrator owns its pipe
//!   as the Administrators group, and the browser's host cannot open it.

use std::io;
#[cfg(unix)]
use std::path::{Path, PathBuf};

use interprocess::local_socket::Name;

/// Where the app listens and the host connects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    #[cfg(unix)]
    path: PathBuf,
    #[cfg(windows)]
    pipe: String,
}

impl Endpoint {
    /// The user's: the one the app listens on and the host looks for.
    pub fn for_user() -> io::Result<Endpoint> {
        #[cfg(target_os = "macos")]
        let folder = dirs::data_local_dir();
        #[cfg(all(unix, not(target_os = "macos")))]
        let folder = dirs::runtime_dir().or_else(dirs::data_local_dir);
        #[cfg(unix)]
        {
            let folder = folder.ok_or_else(|| io::Error::other("no folder for the socket"))?;
            Ok(Endpoint::socket(
                folder.join("authwing").join("bridge.sock"),
            ))
        }
        #[cfg(windows)]
        {
            let user: String = std::env::var("USERNAME")
                .unwrap_or_default()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || "-_.".contains(*c))
                .collect();
            Ok(Endpoint::pipe(format!("authwing-bridge-{user}")))
        }
    }

    /// A socket at `path`; its folder is made when the app listens.
    #[cfg(unix)]
    pub fn socket(path: PathBuf) -> Endpoint {
        Endpoint { path }
    }

    /// The socket file.
    #[cfg(unix)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The named pipe `\\.\pipe\<name>`.
    #[cfg(windows)]
    pub fn pipe(name: String) -> Endpoint {
        Endpoint { pipe: name }
    }

    pub(crate) fn name(&self) -> io::Result<Name<'_>> {
        #[cfg(unix)]
        {
            use interprocess::local_socket::{GenericFilePath, ToFsName};
            self.path.as_path().to_fs_name::<GenericFilePath>()
        }
        #[cfg(windows)]
        {
            use interprocess::local_socket::{GenericNamespaced, ToNsName};
            self.pipe.as_str().to_ns_name::<GenericNamespaced>()
        }
    }

    /// Connects, as the host does, to the app if it is there; one of the
    /// same user only (see the module's notes).
    pub fn connect(&self) -> io::Result<interprocess::local_socket::Stream> {
        use interprocess::local_socket::Stream;
        use interprocess::local_socket::traits::Stream as _;
        let stream = Stream::connect(self.name()?)?;
        same_user(&stream)?;
        Ok(stream)
    }
}

/// Whether the other end of `stream` runs as this process's user, where the
/// system says; an error unless it does.
pub(crate) fn same_user(stream: &interprocess::local_socket::Stream) -> io::Result<()> {
    #[cfg(unix)]
    {
        use interprocess::local_socket::traits::StreamCommon;
        let theirs = stream.peer_creds()?.euid();
        let ours = nix::unistd::geteuid().as_raw();
        if theirs != Some(ours) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("the other end runs as user {theirs:?}, not {ours}"),
            ));
        }
    }
    #[cfg(not(unix))]
    let _ = stream;
    Ok(())
}
