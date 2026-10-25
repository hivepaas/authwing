# Configuration

authwing keeps its codes in an encrypted vault (`vault.db`) and its settings in
TOML, both in its config directory:

| OS | Directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/authwing` (usually `~/.config/authwing`) |
| macOS | `~/Library/Application Support/authwing` |
| Windows | `%APPDATA%\authwing` |

`$AUTHWING_CONFIG_DIR` points it somewhere else: a vault of its own, whose
key has a keychain item of its own (`authwing (<that directory>)`).

How `config.toml` behaves:

- **Every key is optional**; anything missing takes its default, and a
  missing file is all defaults. **Settings** (the gear in the title bar, or
  Cmd/Ctrl+,) changes it for you, keeping your comments.
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

[panel]
key = "alt+space"            # opens the quick panel from any app; "": none; this device's own

[voice]                      # this device's own
model = "base"               # "tiny", "base", "small" or "large", once downloaded; "": voice off
language = "auto"            # the language you name codes in ("vi", "de"…); "auto": the app's

[sync]                       # see Syncing between devices; one place at a time
folder = "~/Library/Mobile Documents/com~apple~CloudDocs/authwing"
settings = true              # the settings that are not this device's own sync too
```

## Codes

The first time authwing opens on a device with no codes, a few steps
welcome it: bringing codes in (from a phone through the camera, a QR code on
the screen, another app's export, by hand), the quick panel and typing codes
in, voice, and the recovery key. Each can be skipped; *Show the welcome
again* in *Settings › About* shows it again. Seen, it is noted in
`state/welcome.json`, on this device.

- **Adding:** the **+** in the title bar (or Cmd/Ctrl+N).
  - *Scan the screen* reads the QR code a site shows, wherever it is on the
    screen; on macOS authwing steps aside while it does. macOS asks once
    for the Screen Recording permission, which applies when authwing is
    opened again; under Wayland the system asks to share the screen.
  - *Use the camera* reads a QR code held up to it: on a phone, on another
    computer, on paper. An export shown as several QR codes (Google
    Authenticator's *Transfer accounts*) is put together as each is shown,
    and all its codes are added at once. The camera is on only while its
    picture shows, and nothing it sees is kept; macOS asks for it the first
    time.
  - An image of the QR code works too: chosen, dropped on the dialog, or
    pasted (Cmd/Ctrl+V).
  - Or paste the link a QR code holds (`otpauth://totp/…`), a Google
    Authenticator export (`otpauth-migration://offline?data=…`, every code
    in it at once), or the key a site shows to type.

  A group and the sites the code belongs to can be given there.
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
- **Filing:** the sidebar shows every code, the favorites (the star on a
  card), those used on this device lately, and each group. **+** beside
  Groups makes one, of a name and a color; the pencil beside a group renames
  it (its codes go along), recolors or deletes it (its codes stay, under no
  group). Groups and stars sync with the codes; what was used when is this
  device's own (`state/usage.json`).
- **Editing:** the pencil on a card changes its service, account, group and
  sites; its key stays as it is.
- **Deleting:** the bin on a card, after a question. Turn two-factor
  sign-in off at the site first, or keep its recovery codes: without the
  code you cannot sign in there.

## The quick panel

A shortcut opens a small panel over the app you are in, with the codes for
that app or page first: Option+Space on macOS, Ctrl+Alt+Space on Windows and
Linux (*Settings › General*, or `key` under `[panel]`; `""` turns it off).

- **Enter** types the code into the app you were in, as the keyboard would;
  **Cmd/Ctrl+C** copies it; **Esc**, or a click elsewhere, closes the panel
  and gives the keyboard back to that app. Typing in the panel narrows the
  list, as the find field does.
- **On macOS, typing needs the Accessibility permission**, the one that
  reads window titles (*Allow…* in Settings asks for it); until then, Enter
  copies. On a page that only looks like a code's site, nothing is typed.
- **The key** is written as the window's shortcuts are (see Keyboard
  shortcuts), with Ctrl, Alt or Cmd: a key alone or with Shift alone is
  refused, and the window says so when another app holds the key already.
  Windows keeps Win+Space and Win+Shift+Space for the keyboard's language.
- **Under Wayland** an app gets no key over the others and cannot type into
  them: the shortcut and typing reach only apps that run through XWayland.

## Finding a code by voice

In the quick panel, hold its shortcut and say a code's name: "Open GitHub",
"PayPal". Let go, and the codes it names come first, the one named clearly
selected: Enter types it.

In the window, the microphone at the end of the find field does the same:
click it, say the name, click it again. The list shows the codes it names,
best first, and Enter copies the first; *Show every code* (or typing, or
Escape) brings the list back.

- **The model** is Whisper's, downloaded once from *Settings › Voice*
  (from Hugging Face, where whisper.cpp publishes it, and checked against
  its SHA-256). `base` (148 MB) is the one to have; the larger ones were
  slower and named no more codes right in our tests. Models are kept in
  `~/Library/Application Support/authwing/models` on macOS,
  `%LOCALAPPDATA%\authwing\models` on Windows and
  `~/.local/share/authwing/models` on Linux.
- **Everything stays on the device.** Whisper runs here, and the microphone
  is on only while the key is held (or the panel's microphone button is on):
  a tap opens the panel without it. macOS asks for the microphone the
  first time; with it refused, the panel says nothing was heard.
- **The language** is the one names are said in: the app's by default.
  When that names no code clearly, it listens again in English, as brand
  names are often said the English way.
- **It learns.** When voice is not sure, it offers the codes it may mean;
  the one you choose is found by what was heard next time. That, and how
  often each code is used, is kept in `state/usage.json`, on this device.

## Backup, and moving from another app

*Settings › Backup*:

- **A backup** holds every code with its group and sites, sealed with a
  passphrase of its own (Argon2id, then XChaCha20-Poly1305), in a
  `.authwing-backup` file. Keep it somewhere else than the computer:
  restoring needs only the file and the passphrase.
- **Restore, or move from another app**: an authwing backup, a list of
  `otpauth://` links (Google Authenticator's `otpauth-migration://` export
  among them), or another app's export. An encrypted file asks for its
  password. Codes already here are left as they are, and codes authwing
  cannot use (MD5, mOTP, an unknown kind) are counted and left out.

  | App | What to export | Encrypted |
  | --- | --- | --- |
  | Aegis | the vault (`.json`) | yes |
  | 2FAS | a backup (`.2fas`) | yes |
  | andOTP | a backup | yes, either kind |
  | Ente Auth | the plain-text export | no |
  | FreeOTP+ | the JSON export | no |
  | Proton Authenticator | the export | yes |
  | Raivo OTP | the export (`.zip`) | yes |
  | Stratum (Authenticator Pro) | a backup | yes |
  | TOTP Authenticator | a backup | yes |
  | Steam Desktop Authenticator | a maFile (`.maFile`) | no |
  | 1Password | `.1pux`, or CSV | no |
  | Apple Passwords | CSV | no |
  | Bitwarden | JSON, or CSV | yes (password-protected JSON) |
  | Dashlane | CSV (credentials) | no |
  | KeePassXC | CSV | no |
  | Proton Pass | the export (`.zip`), or CSV | no |

  From a password manager only the logins with a one-time code come in,
  with the login's site, so the code comes first there.

  Some apps have no file to export:

  - **Google Authenticator**: *Transfer accounts › Export accounts* shows QR
    codes; in *Add a code*, the camera or a screenshot of them brings many
    codes at once.
  - **Microsoft Authenticator, Authy, Duo Mobile** keep their codes to
    themselves: at each site, set up two-step verification again, add the
    new QR code to authwing, then remove it from the old app.
- **Save as plain text**: one `otpauth://` link per code, which other apps
  import, after a question. Anyone who sees the file can make your codes;
  on macOS and Linux only you can read it.

## The browser extension

With authwing's extension in the browser, the app knows the page in front
exactly, not just the window's title:

- A code saved with the page's site (or below it: `github.com` covers
  `gist.github.com`) comes first, and when you copy it on a page with a field
  for a code, it is typed in too.
- A page whose address only looks like a saved site
  (`github.com.login-verify.io`, `githuh.com`, look-alike letters) gets no code,
  and the window says why.
- The extension keeps no codes: it only says where the tab is and fills in
  what the app sends, into that page if it still shows the address the app
  checked.

To set it up, build it (`extension/README.md`), load it unpacked, and tell the
browsers where authwing is, once: *Settings › Browsers › Link the browsers*
(or `authwing --install-browser-host`; *Unlink* or `--uninstall-browser-host`
takes it back). The page lists each browser and whether it is linked. The app must be
running for the extension to link to it; its popup says whether it is.

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

*Settings › Security*:

- **The recovery key**: every vault has one from its first opening, synced
  or not. It opens the vault when nothing else can (a lost keychain, a
  forgotten passphrase, a new device): write it down.
- **The passphrase**: at least 10 characters and hard to guess (several
  unrelated words); it is turned into a key with Argon2id.
- **Open by itself**: on, the key stays in the keychain; off, only the
  passphrase (or Touch ID, Windows Hello) opens the vault, and Lock works.
- **Open with Touch ID / Windows Hello**, once a passphrase is set.

## Syncing between devices

Where the vault syncs is set in *Settings › Sync* (or under `[sync]`), one
place at a time:

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
- **WebDAV** (Nextcloud, Synology, Koofr) and **S3-compatible storage** (AWS,
  Cloudflare R2, Backblaze B2, MinIO): their password or secret key is typed
  in Settings and kept sealed in the vault, as this device's own; it never
  syncs, and each device types its own.

The cloud at the end of the title bar says how it stands (green synced,
orange with a dot failed, gray paused while the vault is locked, a light
orange not set up). It opens a panel with how the last sync went and *Sync
now*; *Settings › Sync* has the rest: authwing cloud (coming: Free up to 50
codes, Pro unlimited at $15 a year), and a place of your own.

- **The first device** creates the vault there and a **recovery key** (13
  groups of 4 characters). Keep it: it is the way in if every device is lost.
- **Another device** with the same place finds a vault it is not in, and
  joins it in *Settings › Sync*:
  - **Ask a device that syncs**: a device that syncs (with authwing open)
    asks whether to let it in. Both show six digits: approve only if they
    are the same. A request left 10 minutes is dropped.
  - **Join with the key**: type the recovery key.
- Syncing runs every minute, right after a code is added, changed or deleted,
  and on *Sync now*; quitting with Cmd/Ctrl+Q waits a moment for the last
  edits to go out. Edits merge field by field, so two devices offline never
  lose each other's changes; a deleted code is deleted everywhere.

Settings in `config.toml` sync too, unless `settings = false` under `[sync]`:

- **What syncs:** `[appearance]` (`mode`, `ui_font`) and `[codes]
  clear_clipboard`.
- **What each device keeps:** `[general] language`, `[sync]`, `[shortcuts]`,
  `[panel]` and `[voice]`, which differ between computers.
- **Merging** is per setting; the file keeps its comments and layout. A
  device whose settings sync for the first time takes the others' where they
  set one, and keeps its own where they set none.
