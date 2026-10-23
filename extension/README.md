# authwing's browser extension

This extension tells the authwing app the exact address of the tab in front.
The app can then offer that site's code, and refuse a look-alike site. The
extension also fills in a code when the app sends one. It keeps no codes and
no secrets. The messages it exchanges with the app are described in
[`crates/bridge/PROTOCOL.md`](../crates/bridge/PROTOCOL.md).

## Building

```sh
npm install
npm run check        # type-check (tsc --noEmit)
npm run build        # dist/chromium and dist/firefox, for loading unpacked
npm run build:store  # the same, without the development "key", for the stores
```

The manifests in `manifests/` contain `//` comments. The build removes them
and takes the version from `package.json`.

## Loading the unpacked build

The extension does nothing until authwing has written its host manifest for
the browser: `authwing --install-browser-host` (or
`cargo run -p authwing -- --install-browser-host` from a checkout), which
writes it for every browser found and says where. Only debug builds of
authwing allow the unpacked Chromium build's id.

- **Chrome, Edge, Brave, Vivaldi, Arc**: open `chrome://extensions` (or
  `edge://extensions`), turn on Developer mode, click "Load unpacked", and
  choose `dist/chromium`. The id is always `jabndagnjdpnmkblgpdjonlpjllpocbl`:
  the `key` in `manifests/chromium.json` fixes it. That key is only the
  public half, and the private half was not kept.
- **Firefox**: open `about:debugging#/runtime/this-firefox`, click "Load
  Temporary Add-on…", and choose `dist/firefox/manifest.json`. The add-on
  stays until Firefox quits. Firefox may hold back access to sites: if so,
  the popup has a button to allow it.

The popup shows "Linked to authwing 0.1.0", or why it is not linked:
authwing is not running, or authwing is not set up for this browser.

To try the extension without the app, use the bridge crate's
`cargo run --example stand_in -- --fill 123456`. It listens as the app does,
prints what the extension says, and fills `123456` into any tab with a code
field. Its doc comment explains how to point a browser at it.

## Permissions

- `nativeMessaging`: to talk to authwing.
- `tabs`: Chrome warns "Read your browsing history" for this permission. It
  lets the extension know the address of the tab in front without a click,
  even on sites where the user has withheld site access. That address is
  what the app checks a code's account against.
- `<all_urls>` (host permissions and content scripts): to find a code field,
  and fill it, on whatever site asks for a code. The content script reads
  only inputs' own attributes (`autocomplete`, `inputmode`, `maxlength`,
  `type`, `name`, `id`, `aria-label`, `placeholder`), never the page's text.
  It sends only whether there is a field.

For Firefox, `data_collection_permissions` is set to `none`, because tab
addresses go only to the authwing app on the same computer. Confirm this
reading of Mozilla's policy before the first submission to
addons.mozilla.org. `web-ext lint` reports no errors. Its one warning is
about Firefox for Android, which has no native messaging: publish the
extension for desktop only.

## Before publishing

- Replace `CHROME_WEB_STORE_ORIGIN` and `EDGE_ADDONS_ORIGIN` in the bridge
  crate's `install.rs` with the ids the two stores give on the first upload.
  Edge Add-ons gives an id of its own. Upload the `npm run build:store`
  output.
- The extension has no icons yet. The stores require them (16, 32, 48 and
  128 px PNGs under `icons` and `action.default_icon`).
- `authwing@hivepaas.com` is permanent once the extension is on
  addons.mozilla.org.

## Safari, later

Safari has no native messaging hosts. A Safari web extension ships inside a
macOS app as an app extension (`.appex`), built in Xcode from this same
`dist/` with `xcrun safari-web-extension-converter`. Its
`runtime.connectNative` / `sendNativeMessage` calls reach that app
extension's `SafariWebExtensionHandler` instead of a host program. For
authwing, this would mean:

- an app extension inside `authwing.app`, signed with the app and given an
  App Group;
- a handler that passes messages to the running app over the same socket as
  the host. The socket would need to live in the App Group's container,
  since the app extension is sandboxed;
- the extension's id coming from the Xcode project, and no host manifest at
  all.

None of this is in scope now. The protocol needs no change for it.
