//! macOS's menu bar: the app's menu, and Edit, whose Cut, Copy, Paste and
//! Select All are the system's own. authwing's views take these keys
//! themselves; the system's panels (choosing an image, saving a backup) get
//! them only through the menu, as macOS has it.

use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, actions};

use authwing_i18n::tr;

actions!(
    menu,
    [Hide, HideOthers, ShowAll, Quit, Cut, Copy, Paste, SelectAll]
);

/// A key context no element has: the bindings give the menu its keys, to
/// show and to pass to the system's panels, and never act in a window.
const MENU_ONLY: &str = "MenuOnly";

/// The menu, its keys and what its own items do. `quit` quits as the
/// window's shortcut does; the menu shows no key for it, as that key is
/// set in config.toml.
pub fn install(cx: &mut App, quit: impl Fn(&mut App) + 'static) {
    cx.bind_keys([
        KeyBinding::new("cmd-h", Hide, Some(MENU_ONLY)),
        KeyBinding::new("cmd-alt-h", HideOthers, Some(MENU_ONLY)),
        KeyBinding::new("cmd-x", Cut, Some(MENU_ONLY)),
        KeyBinding::new("cmd-c", Copy, Some(MENU_ONLY)),
        KeyBinding::new("cmd-v", Paste, Some(MENU_ONLY)),
        KeyBinding::new("cmd-a", SelectAll, Some(MENU_ONLY)),
    ]);
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(move |_: &Quit, cx| quit(cx));
    set_menus(cx);
}

/// The menus in the app's language; again when it changes.
pub fn set_menus(cx: &mut App) {
    cx.set_menus(vec![
        Menu::new("authwing").items([
            MenuItem::action(tr!("Hide authwing"), Hide),
            MenuItem::action(tr!("Hide Others"), HideOthers),
            MenuItem::action(tr!("Show All"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(tr!("Quit authwing"), Quit),
        ]),
        Menu::new(tr!("Edit")).items([
            MenuItem::os_action(tr!("Cut"), Cut, OsAction::Cut),
            MenuItem::os_action(tr!("Copy"), Copy, OsAction::Copy),
            MenuItem::os_action(tr!("Paste"), Paste, OsAction::Paste),
            MenuItem::os_action(tr!("Select All"), SelectAll, OsAction::SelectAll),
        ]),
    ]);
}
