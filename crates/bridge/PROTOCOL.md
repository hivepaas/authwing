# The extension's protocol

How authwing's browser extension and the authwing app talk. The Rust side
is `src/protocol.rs`; the extension's is `extension/src/protocol.ts`. This
file is what both follow.

```text
extension ──stdio (native messaging)──> authwing as the host ──socket──> authwing, running
```

The browser starts the host (the authwing program, recognised by the
arguments the browser gives it) when the extension calls
`runtime.connectNative("dev.authwing.bridge")`, and ends it when the
extension disconnects. The host passes messages on as they are, both ways.

## Framing

Each message is a 32-bit length in the machine's byte order, then that many
bytes of UTF-8 JSON. This holds on stdio, as the browsers require, and on the
socket. A message may be at most 1 MiB (1 048 576 bytes), whichever way it
goes. That is what Chrome and Firefox take from a host, and no message comes
near it. A longer length is an error, and the reader closes the connection.

- Chrome: <https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging#native-messaging-host-protocol>
  (1 MB from the host, 64 MiB to it)
- Firefox: <https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging#app_side>
  (1 MB from the app, 4 GB to it)

## Messages

Each message is a JSON object with a `"type"`. Fields are snake_case.
Optional fields may be left out.

### Extension to app

```json
{"type": "hello", "protocol": 1, "extension_version": "0.1.0", "browser": "chrome"}
{"type": "tab", "tab_id": 4, "url": "https://github.com/sessions/two-factor/app", "title": "GitHub",
 "frame_urls": ["https://login.example.com/otp"], "has_code_field": true}
{"type": "tab_closed", "tab_id": 4}
{"type": "filled", "tab_id": 4, "ok": false, "reason": "url_changed"}
```

- `hello` comes first on each connection. `browser` is only for display
  (`chrome`, `edge`, `brave`, `vivaldi`, `opera`, `chromium`, `firefox`).
  What the browser itself says about the extension's identity reaches the
  app separately, in the host's `host` message (see below).
- `tab` is sent when the tab in front changes, or its address, title or code
  fields change. It is not sent again if nothing changed. `url` is the
  address exactly as the browser has it. `frame_urls` lists the frames other
  than the page itself that have a code field, at most 16 of them.
  `has_code_field` is true if the page or any frame has one.
- `tab_closed` is sent for every tab that closes.
- `filled` answers `fill`. `reason` is present when `ok` is false:
  - `no_tab`: the tab is gone.
  - `url_changed`: the tab no longer shows the address the app checked.
  - `no_field`: the page has no code field.
  - `other_site`: the only code fields are in frames from another origin
    than the page's. These are never filled.
  - `length_mismatch`: the code does not fit the field (too long for its
    `maxlength`, or a different number of boxes).
  - `no_answer`: the page's script did not answer.

### App to extension

```json
{"type": "welcome", "protocol": 1, "app_version": "0.1.0"}
{"type": "fill", "tab_id": 4, "url": "https://github.com/sessions/two-factor/app", "code": "492039"}
{"type": "status", "linked": false, "message": "authwing is not running"}
```

- `welcome` answers `hello`. `protocol` is the version both sides now
  speak. The extension then sends the tab in front.
- `fill` asks the extension to fill `code` into tab `tab_id`, if that tab
  still shows exactly `url`, the address the app checked the code's account
  against. The extension fills the page itself, or else a frame of the same
  origin as `url`. It never fills a frame of another site, and it never
  submits the form. The frame checks its own address again just before
  filling. The extension does not keep the code.
- `status` says whether the extension is linked. The host sends
  `{"linked": false, "message": "authwing is not running"}` when it cannot
  reach the app.

### Host to app, on the socket only

```json
{"type": "host", "caller": {"family": "chromium", "extension_id": "jabndagnjdpnmkblgpdjonlpjllpocbl"}, "host_version": "0.1.0"}
```

The host sends this first, before passing on any of the extension's
messages. `caller` is what the browser put on the host's command line:

- `{"family": "chromium", "extension_id": "<id>"}`: from the origin
  `chrome-extension://<id>/`.
- `{"family": "firefox", "extension_id": "authwing@hivepaas.com"}`: from
  Firefox's arguments.
- `{"family": "unknown"}`: started by hand with `authwing --native-host`.

`caller` is a label, not proof of identity. The browser checks the extension
against the host manifest's `allowed_origins` or `allowed_extensions` before
starting the host. But any process of the same user could connect to the
socket and claim the same.

## Versions

`PROTOCOL` is 1. Each side says its version in its first message (`hello`,
`welcome`), and both keep to the lower of the two. A message of a type a
side does not know is skipped, not an error. So are fields it does not know,
and a known message that does not decode. The extension and the app update
on their own, so the version goes up only for a change an older side could
not just skip.

## When the app is not running

The host stays up. It answers each `hello` with the `status` above, drops the
extension's other messages, and looks for the app every 2 seconds. When it
finds the app, it sends the extension's last `hello` again. The app answers
`welcome`, as if the extension had just connected, and the extension sends
the tab again. If the app quits mid-session, the host sends the same `status`
and starts looking again.

If the host itself cannot be started (no manifest, or the extension's id is
not allowed), the browser closes the port. The extension then waits and tries
again, from 1 second up to 60, doubling each time.

## Where the app listens, and who may connect

- macOS: `~/Library/Application Support/authwing/bridge.sock`
- Linux: `$XDG_RUNTIME_DIR/authwing/bridge.sock`, or
  `~/.local/share/authwing/bridge.sock` when that variable is not set
- Windows: the named pipe `\\.\pipe\authwing-bridge-<user name>`

On macOS and Linux, the folder is made `0700` and must not be writable by
others. The socket is `0600`. Each end asks the system for the other end's
user (`LOCAL_PEERCRED` on macOS, `SO_PEERCRED` on Linux) and drops the
connection unless it is the same user. On Windows, the pipe's access list
allows its owner only. It refuses remote clients, and it is created as the
first instance, so a pipe someone else made first makes the app fail rather
than share. Windows gives the other end's process id but not its user, so
there the access list is the only check, and the host does not check who
made the pipe.

## In authwing's `main`

```rust
let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
// Before anything else, and before anything writes to stdout, which is the browser's.
if authwing_bridge::host::is_native_host_invocation(&args) {
    std::process::exit(authwing_bridge::host::run_native_host(&args));
}
```

The app then listens with
`listener::Listener::bind(Endpoint::for_user()?, version, on_event)`. It
writes the host manifests with `install::install(Os::CURRENT,
&Dirs::for_user()?, &std::env::current_exe()?)`, and shows
`install::status(...)` in its settings. `examples/stand_in.rs` takes the
app's place to try the extension: it branches as above, listens, and prints
what the extension says. Its `--fill <code>` option fills that code into any
tab with a code field.
