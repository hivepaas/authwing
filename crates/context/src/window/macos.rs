//! macOS: the frontmost app from NSWorkspace, its focused window's title
//! from the Accessibility API.
//!
//! The Accessibility API is C functions that every binding marks unsafe;
//! the one safe wrapper on crates.io (`axuielement`) builds a Swift bridge
//! at build time. They are called here, each from a function of its own
//! that is allowed `unsafe` and says why the call is sound.

use std::ptr::NonNull;

use objc2_app_kit::NSWorkspace;
use objc2_application_services::{
    AXError, AXIsProcessTrusted, AXIsProcessTrustedWithOptions, AXUIElement,
    kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFRetained, CFString, CFType};

use super::Window;

/// How long to wait for an app to answer about its window: one that hangs
/// would otherwise hold the caller for the system's six seconds.
const ANSWER_WITHIN: f32 = 0.25;

pub(super) fn active() -> Option<Window> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let app_name = app
        .localizedName()
        .map(|name| name.to_string())
        .unwrap_or_default();
    let app_id = app
        .bundleIdentifier()
        .map_or_else(|| app_name.clone(), |id| id.to_string());
    let pid = app.processIdentifier();
    let title = if pid > 0 && trusted() {
        title(pid)
    } else {
        None
    };
    Some(Window {
        app_id,
        app_name,
        title: title.filter(|title| !title.is_empty()),
        pid: u32::try_from(pid).ok(),
    })
}

#[allow(unsafe_code)]
pub(super) fn trusted() -> bool {
    // SAFETY: takes nothing; reads whether this process is allowed.
    unsafe { AXIsProcessTrusted() }
}

#[allow(unsafe_code)]
pub(super) fn ask() -> bool {
    // SAFETY: a constant the framework sets when it loads.
    let prompt = unsafe { kAXTrustedCheckOptionPrompt };
    let options = CFDictionary::from_slices(&[prompt], &[CFBoolean::new(true)]);
    // SAFETY: the options map that key to a CFBoolean, the type it is
    // documented to take.
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}

/// The title of the focused window of the app `pid`, if it says.
#[allow(unsafe_code)]
fn title(pid: i32) -> Option<String> {
    // SAFETY: takes any pid; for one that is gone, or no app, the element's
    // calls fail with an error.
    let app = unsafe { AXUIElement::new_application(pid) };
    // SAFETY: a valid element and a positive timeout. Only for this element.
    unsafe { app.set_messaging_timeout(ANSWER_WITHIN) };
    let window = attribute(&app, "AXFocusedWindow")?
        .downcast::<AXUIElement>()
        .ok()?;
    let title = attribute(&window, "AXTitle")?.downcast::<CFString>().ok()?;
    Some(title.to_string())
}

/// An element's attribute (the values of `kAX…Attribute`).
#[allow(unsafe_code)]
fn attribute(element: &AXUIElement, name: &'static str) -> Option<CFRetained<CFType>> {
    let name = CFString::from_static_str(name);
    let mut value: *const CFType = std::ptr::null();
    // SAFETY: `value` is a valid place for the function to write a value
    // to, which it does only on success.
    let error = unsafe { element.copy_attribute_value(&name, NonNull::from(&mut value)) };
    if error != AXError::Success {
        return None;
    }
    let value = NonNull::new(value.cast_mut())?;
    // SAFETY: a Copy function's value is the caller's to release (Core
    // Foundation's Create Rule); CFRetained releases it when dropped.
    Some(unsafe { CFRetained::from_raw(value) })
}
