//! Links clang's runtime library on macOS, for whisper.cpp.
//!
//! whisper.cpp's Metal code asks which macOS it runs on (`@available`),
//! which clang turns into calls to `__isPlatformVersionAtLeast`, in clang's
//! own runtime library (`libclang_rt.osx.a`). clang links that into what it
//! links itself; rustc links without it. A build for a macOS older than
//! those checks (a release, for macOS 11) then fails to link, so it is
//! named here. A build for this Mac's own macOS answers them as it
//! compiles, and needs none of it.

use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    let macos = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos");
    let whisper = std::env::var_os("CARGO_FEATURE_WHISPER").is_some();
    if !(macos && whisper) {
        return;
    }
    match runtime_dir() {
        Some(dir) => {
            println!("cargo:rustc-link-search=native={dir}");
            println!("cargo:rustc-link-lib=static=clang_rt.osx");
        }
        None => println!(
            "cargo:warning=clang's runtime library was not found: a build for an older macOS may not link"
        ),
    }
}

/// Where `libclang_rt.osx.a` is: under the resource folder of the clang
/// that Xcode, or its Command Line Tools, has.
fn runtime_dir() -> Option<String> {
    let out = Command::new("xcrun")
        .args(["clang", "--print-resource-dir"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let dir = format!("{}/lib/darwin", String::from_utf8(out.stdout).ok()?.trim());
    Path::new(&dir)
        .join("libclang_rt.osx.a")
        .exists()
        .then_some(dir)
}
