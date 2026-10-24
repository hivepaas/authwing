//! authwing: two-factor codes that know the window you are in.
//!
//! Usage: `authwing`, or `authwing --install-browser-host` (and
//! `--uninstall-browser-host`) to tell the browsers where authwing is, for
//! its extension. The codes live in the vault in the config directory
//! (`$AUTHWING_CONFIG_DIR` overrides it); settings in its `config.toml`
//! (see `docs/configuration.md`). Started by a browser, authwing is its
//! extension's native messaging host instead (see `authwing_bridge`).

// A GUI program on Windows: no console window opens beside it.
#![cfg_attr(windows, windows_subsystem = "windows")]

mod add_code;
mod brands;
mod browser;
mod chrome;
mod codes;
mod colors;
mod group_editor;
mod groups;
mod hotkeys;
mod input;
mod join;
mod look;
mod mark;
mod menu;
mod notice;
mod panel_key;
mod placement;
mod prompt;
mod quick_panel;
mod settings;
mod sync_panel;
mod syncing;
mod typing;
mod vault_slot;
mod voice;
mod welcome;
mod widgets;
mod workspace;

use std::time::Duration;

use authwing_config::Store;
use gpui::{App, AppContext as _, Focusable as _, WindowDecorations, WindowOptions};

/// Icons drawn into the binary: the app has no other files.
struct Assets;

const ICONS: &[(&str, &[u8])] = &[
    ("icons/logo.svg", include_bytes!("../assets/icons/logo.svg")),
    // The app icon's mark, without its tile: the title bar's (see mark.rs).
    (
        "icons/app-mark.svg",
        include_bytes!("../assets/icons/app-mark.svg"),
    ),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    (
        "icons/cloud.svg",
        include_bytes!("../assets/icons/cloud.svg"),
    ),
    ("icons/copy.svg", include_bytes!("../assets/icons/copy.svg")),
    ("icons/eye.svg", include_bytes!("../assets/icons/eye.svg")),
    (
        "icons/eye-off.svg",
        include_bytes!("../assets/icons/eye-off.svg"),
    ),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    (
        "icons/laptop.svg",
        include_bytes!("../assets/icons/laptop.svg"),
    ),
    (
        "icons/server.svg",
        include_bytes!("../assets/icons/server.svg"),
    ),
    ("icons/lock.svg", include_bytes!("../assets/icons/lock.svg")),
    ("icons/mic.svg", include_bytes!("../assets/icons/mic.svg")),
    ("icons/grid.svg", include_bytes!("../assets/icons/grid.svg")),
    (
        "icons/clock.svg",
        include_bytes!("../assets/icons/clock.svg"),
    ),
    ("icons/star.svg", include_bytes!("../assets/icons/star.svg")),
    (
        "icons/star-fill.svg",
        include_bytes!("../assets/icons/star-fill.svg"),
    ),
    (
        "icons/pencil.svg",
        include_bytes!("../assets/icons/pencil.svg"),
    ),
    ("icons/scan.svg", include_bytes!("../assets/icons/scan.svg")),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "icons/refresh.svg",
        include_bytes!("../assets/icons/refresh.svg"),
    ),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/settings.svg",
        include_bytes!("../assets/icons/settings.svg"),
    ),
    (
        "icons/trash.svg",
        include_bytes!("../assets/icons/trash.svg"),
    ),
    (
        "icons/minimize.svg",
        include_bytes!("../assets/icons/minimize.svg"),
    ),
    (
        "icons/maximize.svg",
        include_bytes!("../assets/icons/maximize.svg"),
    ),
    (
        "icons/close.svg",
        include_bytes!("../assets/icons/close.svg"),
    ),
    // The caption buttons as Windows draws them.
    (
        "icons/win-minimize.svg",
        include_bytes!("../assets/icons/win-minimize.svg"),
    ),
    (
        "icons/win-maximize.svg",
        include_bytes!("../assets/icons/win-maximize.svg"),
    ),
    (
        "icons/win-restore.svg",
        include_bytes!("../assets/icons/win-restore.svg"),
    ),
    (
        "icons/win-close.svg",
        include_bytes!("../assets/icons/win-close.svg"),
    ),
];

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        if path.starts_with(brands::PREFIX) {
            return Ok(brands::svg(path).map(std::borrow::Cow::Owned));
        }
        Ok(ICONS
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, bytes)| std::borrow::Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(p, _)| p.starts_with(path))
            .map(|(p, _)| (*p).into())
            .collect())
    }
}

/// How often config.toml is checked for edits.
const CONFIG_POLL: Duration = Duration::from_millis(500);

const USAGE: &str = "usage: authwing [--install-browser-host | --uninstall-browser-host]
  Two-factor codes. Settings: config.toml in the config directory.
  --install-browser-host    tell the browsers where authwing is, for its extension
  --uninstall-browser-host  take that back";

/// Writes or removes the browsers' manifests for the extension's host:
/// what the process should exit with.
fn browser_host(install: bool) -> i32 {
    use authwing_bridge::install::{self, Dirs, Os};
    let dirs = match Dirs::for_user() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("authwing: {e}");
            return 1;
        }
    };
    let done = if install {
        match std::env::current_exe() {
            Ok(program) => install::install(Os::CURRENT, &dirs, &program),
            Err(e) => Err(e),
        }
    } else {
        install::uninstall(Os::CURRENT, &dirs)
    };
    match done {
        Ok(places) if places.is_empty() => {
            println!("No browser the extension runs in was found.");
            0
        }
        Ok(places) => {
            for (browser, place) in places {
                println!("{}: {}", browser.name(), place.manifest().display());
            }
            0
        }
        Err(e) => {
            eprintln!("authwing: {e}");
            1
        }
    }
}

/// Windows starts a GUI program without a console. Started from a terminal,
/// the app takes that one, so `--help` and errors are written there.
#[cfg(windows)]
#[allow(unsafe_code)]
fn attach_console() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    // SAFETY: no pointers or handles go in. Without a parent console it
    // fails and changes nothing, which is what a start from Explorer wants.
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

fn main() -> anyhow::Result<()> {
    // Started by a browser: its stdout is the extension's, so nothing else
    // may be written first.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if authwing_bridge::host::is_native_host_invocation(&args) {
        std::process::exit(authwing_bridge::host::run_native_host(&args));
    }
    #[cfg(windows)]
    attach_console();
    if let Some(a) = std::env::args().nth(1) {
        match a.as_str() {
            "-h" | "--help" => println!("{USAGE}"),
            "--install-browser-host" => std::process::exit(browser_host(true)),
            "--uninstall-browser-host" => std::process::exit(browser_host(false)),
            _ => {
                eprintln!("authwing: unknown argument {a}\n{USAGE}");
                std::process::exit(2);
            }
        }
        return Ok(());
    }
    let (live, problems) = look::Live::open(Store::default_dir().map(Store::new));
    // The app's language, before anything is said.
    authwing_i18n::set_language(live.settings().general.language.as_deref());

    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            // Questions are drawn in the app's look on every system.
            cx.set_prompt_builder(prompt::render);
            // The quick panel's shortcut, over any app.
            match panel_key::PanelKey::new() {
                Ok(keys) => cx.set_global(keys),
                Err(e) => eprintln!("authwing: no shortcut over other apps: {e}"),
            }
            let window_bounds = placement::initial(live.store().and_then(|s| s.window_state()), cx);
            // GNOME's Wayland compositor draws no window frame, so Linux gets
            // client-side decorations. macOS and Windows keep the system's
            // frame but hide its title bar, and the app draws its own in its
            // place (see chrome::window_chrome).
            let client_side = cfg!(any(target_os = "linux", target_os = "freebsd"));
            let options = WindowOptions {
                window_bounds: Some(window_bounds),
                window_decorations: client_side.then_some(WindowDecorations::Client),
                titlebar: (!client_side).then(|| gpui::TitlebarOptions {
                    title: Some("authwing".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(chrome::TRAFFIC_LIGHTS),
                }),
                app_id: Some("authwing".into()),
                ..Default::default()
            };
            let mut setup = Some((live, problems));
            let Ok(window) = cx.open_window(options, |_, cx| {
                cx.new(|cx| {
                    let (live, problems) = setup
                        .take()
                        .unwrap_or_else(|| (look::Live::open(None).0, vec![]));
                    workspace::Workspace::new(live, problems, cx)
                })
            }) else {
                eprintln!("cannot open a window");
                cx.quit();
                return;
            };
            let Ok(ws) = window.update(cx, |_, _, cx| cx.entity()) else {
                return;
            };
            let _ = window.update(cx, |ws, window, cx| {
                window.focus(&ws.focus_handle(cx), cx);
                cx.activate(true);
                ws.track_appearance(window, cx);
                ws.track_bounds(window, cx);
                ws.apply_panel_key(cx);
            });
            if cfg!(target_os = "macos") {
                let quitting = ws.clone();
                menu::install(cx, move |cx| quitting.update(cx, |ws, cx| ws.quit(cx)));
            }
            // The shortcut opens the quick panel, and held down listens;
            // from the system's thread.
            cx.set_global(voice::Voice::new());
            let (pressed, presses) = async_channel::unbounded::<bool>();
            panel_key::listen(move |down| {
                let _ = pressed.send_blocking(down);
            });
            let quick = ws.clone();
            cx.spawn(async move |cx| {
                while let Ok(down) = presses.recv().await {
                    quick.update(cx, |ws, cx| ws.panel_key(down, cx));
                }
            })
            .detach();
            // Closing the main window quits, on macOS too; the quick panel's
            // closing does not.
            cx.on_window_closed(|cx, _| {
                let main = cx
                    .windows()
                    .iter()
                    .any(|w| w.downcast::<workspace::Workspace>().is_some());
                if !main {
                    cx.quit();
                }
            })
            .detach();
            // Edits to config.toml apply while running.
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor().timer(CONFIG_POLL).await;
                    ws.update(cx, |ws, cx| ws.poll_config(cx));
                }
            })
            .detach();
        });
    Ok(())
}
