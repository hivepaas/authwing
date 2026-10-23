# authwing's English, from the code (scripts/i18n.py update writes it:
# do not edit). The source for translations.

tbfa1bb03 = otpauth:// link, or the key the site shows
t304ff7fb = None
te8e7a37a = Paste the code's link, or type its key.
t141e1cf7 = This is neither a link nor a key: { $e }
tdb7a122d = Say whose code it is: the service, or the account.
ta13f22fd =
    { $n ->
        [one] The export holds no code authwing can use ({ $n } left out).
       *[other] The export holds no code authwing can use ({ $n } left out).
    }
t82066414 = Add a code
t0378be46 = When a site turns on two-factor sign-in it shows a QR code, and a link or a key beside it. Paste either here; a QR code from your phone's camera gives the link too.
td5c6ff95 = Link or key
tb643c704 = Service
t59f8a2fc = Account
t0574902c = Group
te673b518 = Show it first on
t35afca3b = Cancel
t9dc3aa14 = Add
t4caf769c = Cannot read the codes: { $e }
t855061c0 = A code cannot be read: { $e }
t658f3664 = Copy
t58e25863 = Cut
tb34a0288 = Paste
t6af21072 = Select all
ta81ce4ca = { $key } alone is for typing: hold Cmd, Ctrl or Option with it.
t834f7024 = { $key } alone is for typing: hold Ctrl or Alt with it.
t67688898 = { $key } belongs to the system: it never reaches Authwing.
t01906ac3 = { $key } is { $what }, which cannot be changed.
t331b3467 = config.toml, [shortcuts]: no shortcut is named `{ $name }`.
tb915b010 = config.toml, [shortcuts] { $name }: `{ $key }`: { $error }
t71adda1f = config.toml, [shortcuts] { $name }: { $why }
te833da37 = config.toml, [shortcuts]: `{ $key }` is given to both { $kept } and { $lost }; { $kept } keeps it.
t87e7ceec = config.toml, [shortcuts] { $name }: `{ $key }` does not fit; only copy_code takes `digit` (1 to 9), and it takes nothing else.
tbb504d9f = XXXX-XXXX-…, 13 groups
tdd764294 = This vault has no recovery key yet.
t02904a59 = Recovery key
t82f98ccc = Keep it where you keep important papers: it opens the vault from a new device, and it is the way back if every device is lost.
t16f62979 = Show the recovery key
te10c7bcd = Hide
t33ac6aa2 = Waiting for a device that syncs: open authwing there.
ta89ba4d1 = Approve on { $device } if it shows the same code:
t775b89a0 = Joined. The vault's codes are coming.
t74419d8e = The other device refused.
t5c323bb6 = Nobody approved in time: ask again.
t2dc61136 = Ask a device that syncs
t296beec8 = Stop asking
t89a71bba = Sync
tc75c83a8 = Not syncing. Say where in config.toml, then come back here: a folder that syncs (iCloud Drive, Dropbox, Syncthing), WebDAV, S3 or Git.
tcd86acc3 = Close
td7b59080 = Only encrypted records are stored there: not even the services' names can be read.
tff44c4a4 = Devices asking to join
t8f54c79f = Approve only if it shows this code.
te6edf55a = Approve
t9bcada35 = Refuse
t54904649 = Join from this device
te7a28cec = When another vault syncs there already: ask a device that syncs, or type the vault's recovery key. Its sync passphrase works too.
t2d6ea251 = When another vault syncs there already: ask a device that syncs, or type the vault's recovery key.
taa884c51 = Joining…
t9f857a57 = Join with the key
ta34dd8fe = Sync now
td2fdfc60 = its password is not saved on this device: connect again in Settings › Sync
tf8592ea2 = settings not synced: { $e }
t60f438b7 = settings not written: { $e }
t8b2733e6 = Find a code
t4774284e = Passphrase or recovery key
tf1bf552a = A device asks to join: open Sync (the cloud) to approve it.
t97706593 = Typed into the page.
t709fb554 = Not typed into the page: { $why }
t0f346f03 = Not syncing
t8ad2ae06 = Syncing…
t4df6f903 = Synced just now
tc7922c74 =
    { $n ->
        [one] Synced { $n } minute ago
       *[other] Synced { $n } minutes ago
    }
t9dc23f47 = { $when }: { $pulled } in, { $pushed } out
tcc341778 = Sync failed: { $e }
t6b25e007 = Joined: this device syncs the vault now.
tb28eb5cc = The counter was not saved: { $e }
t6cd84653 = A code was not saved: { $e }
tcb4c07d0 =
    { $n ->
        [one] { $n } code added.
       *[other] { $n } codes added.
    }
t1690127f = Delete the code for { $name }?
t28110a8b = Turn two-factor sign-in off at { $name } first, or sign in there with a recovery code: without this code you cannot sign in. Your other devices lose it too.
t5797ea6a = Delete
t12e9b880 = Not deleted: { $e }
ta55f9fc6 = The vault's key is in this computer's keychain, so it cannot lock. Set a passphrase and take the key out of the keychain to lock it.
t2e0269c8 = Not opened: { $e }
tb78f406b = Cannot open config.toml: { $e }
t71b42d62 = Lock
t1fc961bb = Settings (config.toml)
t113331ee = All codes
tf57f458d = Groups
t3e02a97b = Not syncing: set [sync] in config.toml
tb1be48b7 = next { $code }
t8dc21305 = Copied
tfc0f33c3 = Counter
t2f4582ba = This page is not { $site }
t6ae50f4f = { $page } only looks like it. Pages like this copy a real sign-in to take your code: none is offered here.
tb8141570 = its site
tf5dd541a = a rule
td89d57b6 = chosen here before
t04583a8a = its name in the title
tf0c6b3bc = Matched by { $why }
t8f247c27 = For the window used last: { $place }
t33ff9c8a = No codes yet
t2d0ad3d3 = Turn on two-factor sign-in at a site, then paste the link of the QR code it shows.
t4a8c2b9a = Add your first code
tda76133f = Nothing matches
t289325a3 = Try fewer words, or another group.
t69755c3c = Set a passphrase
t911809a4 = This computer has no keychain to keep the vault's key in.
t78b7a6ee = The vault is locked
t2c0f4729 = Type its passphrase, or the recovery key you saved.
te97e7be1 = The vault cannot open
td46e3d6c = Opening the vault…
t99b658f7 = Open with Touch ID or Windows Hello
tc40f2321 = Opening…
t538b10e9 = Open
