# Building and testing on macOS, Windows and Linux

CI builds and tests every push to main and every pull request into main on Linux
(x86_64); pull requests stacked on other branches run once retargeted to main. macOS
(arm64) and Windows (x86_64) run when started by hand, once a feature is ready:
Actions → CI → Run workflow, pick the branch and the platforms, or
`gh workflow run ci.yml --ref <branch> -f platforms=all`. Each run uploads the
binaries as workflow artifacts (`authwing-<platform>`), so a test machine does
not need a toolchain: download the artifact from the run's summary page.

Rendering and input (IME, keyboard layouts, HiDPI) still need a real desktop;
CI only proves that the code builds and the tests pass.

## Toolchain (to build locally)

All platforms need Rust (via [rustup](https://rustup.rs)) and CMake: voice
builds whisper.cpp with it. A CMake that does not run on this computer (an
Intel build on Apple silicon without Rosetta, say) fails the build of
`whisper-rs-sys` with "Bad CPU type"; `CMAKE=/path/to/cmake` picks another.

| Platform | Also install |
| --- | --- |
| macOS | Xcode Command Line Tools: `xcode-select --install`; CMake: `brew install cmake` or `pip install cmake` |
| Windows | Visual Studio Build Tools with the "Desktop development with C++" workload (MSVC, the Windows SDK, and its CMake) |
| Debian/Ubuntu | `sudo apt-get install build-essential pkg-config libfontconfig-dev libfreetype-dev libwayland-dev libxkbcommon-dev libxkbcommon-x11-dev libx11-xcb-dev libxcb1-dev libxcb-xkb-dev libxcb-shape0-dev libxcb-xfixes0-dev libvulkan-dev libdbus-1-dev libzstd-dev libpam0g-dev cmake libasound2-dev` |

`make build` builds a release for this computer into `dist/`; `make` alone lists
the other targets (`make run`, `make check`, `make doctor`). By hand:

```sh
cargo build --release          # Linux: Wayland and X11
cargo build --release -p authwing --no-default-features --features wayland   # Linux without the X11 libraries
```

Downloaded macOS binaries are quarantined by Gatekeeper; clear the flag once:

```sh
xattr -d com.apple.quarantine ./authwing && chmod +x ./authwing
```

## Windows in a virtual machine

GPUI renders with Direct3D 11 on Windows, so the VM's 3D support decides
whether rendering is fast:

| Host | VM | Notes |
| --- | --- | --- |
| Apple silicon Mac | VMware Fusion (free for personal use) or Parallels, Windows 11 ARM | Good Direct3D 11. Windows 11 ARM runs the x86_64 artifact through its built-in emulation. |
| Linux | VMware Workstation (free for personal use) | Usable 3D acceleration. |
| Linux | QEMU/KVM or VirtualBox | Little or no Direct3D; Windows falls back to software rendering (WARP). Fine for checking behaviour, not for measuring speed. |

Microsoft's Windows 11 Enterprise evaluation image works for testing.

## What to check by hand

Start it with a config directory of its own, so a test does not touch your
codes: `AUTHWING_CONFIG_DIR=/tmp/aw cargo run -p authwing`. The vault's key
goes into the system keychain (item `authwing`).

1. Add a code from a link: `otpauth://totp/Example:me@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example`.
   Its code matches another app's for the same link (or `oathtool --totp -b JBSWY3DPEHPK3PXP`).
2. Click a card: the code is on the clipboard, and gone from it after
   `[codes] clear_clipboard` seconds (20 by default) unless something else
   was copied since.
3. Type in the find field; Enter copies the first code shown; Cmd/Ctrl+1 to 9
   copy by place.
4. Switch to another app and back: a code whose name is in that window's
   title shows under "For the window used last". On macOS the title needs
   the Accessibility permission (System Settings › Privacy & Security).
5. Sync: set `[sync] folder = "/tmp/aw-sync"` in config.toml, add a code,
   then start a second copy with another config directory and the same
   folder; join from the cloud's panel with the first one's recovery key,
   or by asking and approving the six digits on the first.
6. HiDPI: text is sharp at the display's scale (e.g. 125 % or 150 % on Windows).
