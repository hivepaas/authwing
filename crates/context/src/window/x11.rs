//! Linux on X11: the window manager's `_NET_ACTIVE_WINDOW` (EWMH), read
//! with `x11rb`'s pure Rust client.
//!
//! The connection is kept between calls, and made again after an error;
//! under Wayland none is made at all (see [`super`]).

use std::sync::{Mutex, PoisonError};

use x11rb::connection::Connection;
use x11rb::errors::ReplyError;
use x11rb::protocol::xproto::{
    Atom, AtomEnum, ClientMessageEvent, ConnectionExt as _, EventMask, GetPropertyReply,
    Window as XWindow,
};
use x11rb::rust_connection::RustConnection;

use super::Window;

x11rb::atom_manager! {
    Atoms: AtomsCookie {
        _NET_ACTIVE_WINDOW,
        _NET_WM_NAME,
        _NET_WM_PID,
        UTF8_STRING,
    }
}

struct Display {
    connection: RustConnection,
    root: XWindow,
    atoms: Atoms,
}

static DISPLAY: Mutex<Option<Display>> = Mutex::new(None);

pub(super) fn active() -> Option<Window> {
    if wayland() {
        return None;
    }
    let mut display = DISPLAY.lock().unwrap_or_else(PoisonError::into_inner);
    if display.is_none() {
        *display = connect();
    }
    match display.as_ref()?.active() {
        Ok(window) => window,
        // A window that closed while being read, say.
        Err(ReplyError::X11Error(_)) => None,
        Err(ReplyError::ConnectionError(_)) => {
            *display = None;
            None
        }
    }
}

/// Asks the window manager to make `window` the active window again.
pub(super) fn bring_back(window: u64) -> bool {
    let Ok(window) = XWindow::try_from(window) else {
        return false;
    };
    let mut display = DISPLAY.lock().unwrap_or_else(PoisonError::into_inner);
    if display.is_none() {
        *display = connect();
    }
    display
        .as_ref()
        .is_some_and(|display| display.activate(window).is_ok())
}

fn wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|session| session == "wayland")
}

fn connect() -> Option<Display> {
    let (connection, screen) = x11rb::connect(None).ok()?;
    let root = connection.setup().roots.get(screen)?.root;
    let atoms = Atoms::new(&connection).ok()?.reply().ok()?;
    Some(Display {
        connection,
        root,
        atoms,
    })
}

impl Display {
    fn active(&self) -> Result<Option<Window>, ReplyError> {
        let atoms = &self.atoms;
        let active = self.property(self.root, atoms._NET_ACTIVE_WINDOW, AtomEnum::WINDOW.into())?;
        let Some(window) = first_u32(&active).filter(|&window| window != 0) else {
            return Ok(None);
        };
        // WM_CLASS is the instance's name then the class's, each ending in a nul.
        let class = self.property(window, AtomEnum::WM_CLASS.into(), AtomEnum::STRING.into())?;
        let mut names = class
            .value
            .split(|&b| b == 0)
            .map(|name| String::from_utf8_lossy(name).into_owned());
        let instance = names.next().unwrap_or_default();
        let class = names
            .next()
            .filter(|class| !class.is_empty())
            .unwrap_or(instance);
        let name = self.property(window, atoms._NET_WM_NAME, atoms.UTF8_STRING)?;
        let mut title = String::from_utf8_lossy(&name.value).into_owned();
        if title.is_empty() {
            // The ICCCM's own, in Latin-1.
            let name = self.property(window, AtomEnum::WM_NAME.into(), AtomEnum::STRING.into())?;
            title = name.value.iter().map(|&b| char::from(b)).collect();
        }
        let pid =
            first_u32(&self.property(window, atoms._NET_WM_PID, AtomEnum::CARDINAL.into())?);
        Ok(Some(Window {
            app_id: class.clone(),
            app_name: class,
            title: (!title.is_empty()).then_some(title),
            pid: pid.filter(|&pid| pid != 0),
            handle: Some(u64::from(window)),
        }))
    }

    /// A `_NET_ACTIVE_WINDOW` request, as EWMH has a pager send it (source
    /// 2): window managers that refuse apps the focus to keep them from
    /// stealing it still grant it to a pager.
    fn activate(&self, window: XWindow) -> Result<(), x11rb::errors::ConnectionError> {
        let request =
            ClientMessageEvent::new(32, window, self.atoms._NET_ACTIVE_WINDOW, [2, 0, 0, 0, 0]);
        self.connection.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            request,
        )?;
        self.connection.flush()
    }

    /// A window's property of the given type; empty if it has none.
    fn property(
        &self,
        window: XWindow,
        property: Atom,
        kind: Atom,
    ) -> Result<GetPropertyReply, ReplyError> {
        // Up to 64 KiB, in the 32-bit units the protocol counts in.
        self.connection
            .get_property(false, window, property, kind, 0, 16_384)?
            .reply()
    }
}

/// The first value of a property of 32-bit values.
fn first_u32(reply: &GetPropertyReply) -> Option<u32> {
    reply.value32()?.next()
}
