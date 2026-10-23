# Configuration

authwing keeps its codes in an encrypted vault (`vault.db`) and its settings in
TOML, both in its config directory:

| OS | Directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/authwing` (usually `~/.config/authwing`) |
| macOS | `~/Library/Application Support/authwing` |
| Windows | `%APPDATA%\authwing` |

`$AUTHWING_CONFIG_DIR` points it somewhere else.

How `config.toml` behaves:

- **Every key is optional**; anything missing takes its default, and a
  missing file is all defaults. The gear in the title bar opens it in your
  editor, created with a commented example first.
- **Edits apply to a running window** within half a second.
- **A file with a mistake is reported** with its line number, and the window
  keeps the last good values.
- **Unknown keys are ignored**, so a file written by a newer version still loads.

The `state/` folder holds what authwing remembers by itself: the window's size
and place (`state/window.json`). It is not meant for editing, and deleting it
only forgets that.

## config.toml

```toml
[general]
language = "auto"            # "auto" (the system's) or a language, see Language; this device's own

[appearance]
mode = "system"              # "system" (as the desktop is), "dark" or "light"
ui_font = { family = "Inter", size = 14 }   # no family: the system's; size 10 to 20

[codes]
clear_clipboard = 20         # seconds a copied code stays on the clipboard; 0: it stays

[sync]                       # see Syncing between devices; one place at a time
folder = "~/Library/Mobile Documents/com~apple~CloudDocs/authwing"
settings = true              # the settings that are not this device's own sync too
```

## Codes

- **Adding:** the **+** in the title bar (or Cmd/Ctrl+N) takes the link a
  site's QR code holds (`otpauth://totp/…`), a Google Authenticator export
  (`otpauth-migration://offline?data=…`, every code in it at once), or the
  key a site shows to type when its QR code cannot be scanned. A group and
  the sites the code belongs to can be given there.
- **Copying:** a click on a card, Enter in the find field (the first code
  shown), or Cmd/Ctrl+1 to 9 by place. The clipboard is cleared after
  `clear_clipboard` seconds, unless something else was copied since. An HOTP
  code moves on to the next when copied.
- **Finding:** type in the find field (Cmd/Ctrl+K or F): every word must be in
  the service, the account, the group or a site.
- **For the window used last:** codes for the app you were in before
  authwing come first, by the sites saved with them or their name in the
  window's title. On macOS the title needs the Accessibility permission
  (System Settings › Privacy & Security › Accessibility); without it only
  the app is known. On Wayland no app may read other windows.
- **Deleting:** the bin on a card, after a question. Turn two-factor
  sign-in off at the site first, or keep its recovery codes: without the
  code you cannot sign in there.

## Language

authwing speaks the system's language where it has it, else English;
`language` under `[general]` picks another. Each device keeps its own: it does
not sync.

| `language` | |
| --- | --- |
| `en` | English |
| `de` | Deutsch |
| `es` | Español |
| `fr` | Français |
| `pt` | Português (Brasil) |
| `vi` | Tiếng Việt |
| `ru` | Русский |
| `ja` | 日本語 |
| `ko` | 한국어 |
| `zh` | 简体中文 (simplified Chinese) |

Translations live in `crates/i18n/locales/<language>.ftl` (Fluent), each message
under the English it translates, which is in the code. `scripts/i18n.py update`
brings them in step with the code; `scripts/i18n.py todo de` lists what is left.

## Keyboard shortcuts

The keys are kept in `config.toml`, under `[shortcuts]`, only for the shortcuts
whose keys differ from their defaults; each device keeps its own (they do not
sync, as keys differ between systems). A shortcut takes one key or a list, and
`[]` leaves it without one:

```toml
[shortcuts]
find = ["ctrl+k", "ctrl+shift+space"]
lock = []
copy_code = "alt+digit"      # Alt+1…9
```

A key is its modifiers and then the key, joined by `+`: `ctrl`, `alt`
(`option`), `shift`, `cmd` (`super`, `win`: Cmd on macOS, the Windows key
elsewhere), then a letter, digit or symbol, or `left`, `right`, `up`, `down`,
`home`, `end`, `pageup`, `pagedown`, `tab`, `enter`, `escape`, `space`,
`backspace`, `delete`, `insert`, `f1`…`f24`. `digit` stands for 1 to 9, and
only `copy_code` takes it.

| Name | Linux, Windows | macOS |
| --- | --- | --- |
| `find` | Ctrl+K, Ctrl+F | Cmd+K, Cmd+F |
| `add_code` | Ctrl+N | Cmd+N |
| `copy_code` | Ctrl+1…9 | Cmd+1…9 |
| `lock` | Ctrl+L | Cmd+L |
| `settings` | Ctrl+, | Cmd+, |
| `quit` | Ctrl+Q | Cmd+Q |

Refused: a key alone or with Shift alone (it is for typing), keys the system
keeps (Alt+Tab, Alt+F4, the Windows key, Cmd+Tab, Cmd+H), and the text fields'
own keys (copy, cut, paste, select all). A name or key that does not read is
reported and left out.

## The vault

The vault opens by itself at start with its key from the system keychain
(macOS Keychain, Windows Credential Manager, Secret Service on Linux). Where
there is none, or the key was taken out of it, it asks for its passphrase or
the recovery key; Touch ID or Windows Hello opens it where they were set up
for it. **Lock** (Cmd/Ctrl+L) closes it until then; with its key in the
keychain there is nothing to lock, and the window says so.

## Syncing between devices

Where the vault syncs is set under `[sync]`, one place at a time:

```toml
[sync]
folder = "~/Library/Mobile Documents/com~apple~CloudDocs/authwing"   # any synced folder
# git = { url = "git@github.com:me/vault.git" }                      # or a private Git repository
```

- **A folder** another tool keeps in sync: iCloud Drive, Syncthing, a Dropbox
  or OneDrive folder, a network share. authwing writes `authwing/format.json`
  and one encrypted file per code in `authwing/records/`; nothing in them is
  readable without the key, the services' names included.
- **A Git repository** goes through this system's git (2.31 or newer), which
  signs in as it does for you (Git Credential Manager, the keychain, your SSH
  keys and agent).
- **WebDAV and S3-compatible storage** (`webdav = { url, user }`, `s3 = {
  endpoint, bucket, prefix, access_key }`) are read too, but their password or
  secret key is kept sealed in the vault, and the window cannot save it yet:
  until it can, use a folder or Git.

The cloud in the title bar says how it stands (green synced, orange failed, a
light orange not set up) and opens the Sync panel:

- **The first device** creates the vault there and a **recovery key** (13
  groups of 4 characters). Keep it: it is the way in if every device is lost.
- **Another device** with the same place joins from the panel:
  - **Ask a device that syncs**: on a device that syncs (with authwing open),
    the request shows up in its panel. Both show six digits: approve only if
    they are the same. A request left 10 minutes is dropped.
  - **Join with the key**: type the recovery key.
- Syncing runs every minute, right after a code is added, changed or deleted,
  and on *Sync now*; quitting with Cmd/Ctrl+Q waits a moment for the last
  edits to go out. Edits merge field by field, so two devices offline never
  lose each other's changes; a deleted code is deleted everywhere.

Settings in `config.toml` sync too, unless `settings = false` under `[sync]`:

- **What syncs:** `[appearance]` (`mode`, `ui_font`) and `[codes]
  clear_clipboard`.
- **What each device keeps:** `[general] language`, `[sync]` and
  `[shortcuts]`, which differ between computers.
- **Merging** is per setting; the file keeps its comments and layout. A
  device whose settings sync for the first time takes the others' where they
  set one, and keeps its own where they set none.
