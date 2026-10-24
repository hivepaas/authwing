# authwing in ru. Each message is found by its English, the
# comment above it; scripts/i18n.py keeps this file in step with the code.

# otpauth:// link, or the key the site shows
# tbfa1bb03 =

# None
t304ff7fb = Нет

# Paste the code's link, or type its key.
# te8e7a37a =

# This is neither a link nor a key: {e}
# t141e1cf7 =

# Say whose code it is: the service, or the account.
# tdb7a122d =

# The export holds no code authwing can use ({n} left out). | The export holds no code authwing can use ({n} left out).
ta13f22fd =
    { $n ->
        [one] В экспорте нет кодов, которые authwing может использовать ({ $n } пропущен).
        [few] В экспорте нет кодов, которые authwing может использовать ({ $n } пропущено).
        [many] В экспорте нет кодов, которые authwing может использовать ({ $n } пропущено).
       *[other] В экспорте нет кодов, которые authwing может использовать ({ $n } пропущено).
    }

# Add a code
# t82066414 =

# When a site turns on two-factor sign-in it shows a QR code, and a link or a key beside it. Paste either here; a QR code from your phone's camera gives the link too.
# t0378be46 =

# Link or key
# td5c6ff95 =

# Service
# tb643c704 =

# Account
# t59f8a2fc =

# Group
t0574902c = Группа

# Show it first on
# te673b518 =

# Cancel
t35afca3b = Отмена

# Add
# t9dc3aa14 =

# Cannot read the codes: {e}
# t4caf769c =

# A code cannot be read: {e}
# t855061c0 =

# Copy
t658f3664 = Копировать

# Cut
t58e25863 = Вырезать

# Paste
tb34a0288 = Вставить

# Select all
# t6af21072 =

# {key} alone is for typing: hold Cmd, Ctrl or Option with it.
ta81ce4ca = { $key } без модификаторов — это ввод текста: удерживайте вместе с ней Cmd, Ctrl или Option.

# {key} alone is for typing: hold Ctrl or Alt with it.
t834f7024 = { $key } без модификаторов — это ввод текста: удерживайте вместе с ней Ctrl или Alt.

# {key} belongs to the system: it never reaches Authwing.
# t67688898 =

# {key} is {what}, which cannot be changed.
t01906ac3 = { $key } — это «{ $what }»; изменить нельзя.

# config.toml, [shortcuts]: no shortcut is named `{name}`.
t331b3467 = config.toml, [shortcuts]: сочетания клавиш с именем `{ $name }` нет.

# config.toml, [shortcuts] {name}: `{key}`: {error}
tb915b010 = config.toml, [shortcuts] { $name }: `{ $key }`: { $error }

# config.toml, [shortcuts] {name}: {why}
t71adda1f = config.toml, [shortcuts] { $name }: { $why }

# config.toml, [shortcuts]: `{key}` is given to both {kept} and {lost}; {kept} keeps it.
te833da37 = config.toml, [shortcuts]: `{ $key }` используется и в { $kept }, и в { $lost }; остаётся за { $kept }.

# config.toml, [shortcuts] {name}: `{key}` does not fit; only copy_code takes `digit` (1 to 9), and it takes nothing else.
# t87e7ceec =

# XXXX-XXXX-…, 13 groups
# tbb504d9f =

# This vault has no recovery key yet.
# tdd764294 =

# Recovery key
# t02904a59 =

# Keep it where you keep important papers: it opens the vault from a new device, and it is the way back if every device is lost.
# t82f98ccc =

# Show the recovery key
# t16f62979 =

# Hide
# te10c7bcd =

# Waiting for a device that syncs: open authwing there.
# t33ac6aa2 =

# Approve on {device} if it shows the same code:
# ta89ba4d1 =

# Joined. The vault's codes are coming.
# t775b89a0 =

# The other device refused.
# t74419d8e =

# Nobody approved in time: ask again.
# t5c323bb6 =

# Ask a device that syncs
# t2dc61136 =

# Stop asking
# t296beec8 =

# Sync
t89a71bba = Синхронизация

# Not syncing. Say where in config.toml, then come back here: a folder that syncs (iCloud Drive, Dropbox, Syncthing), WebDAV, S3 or Git.
# tc75c83a8 =

# Close
tcd86acc3 = Закрыть

# Only encrypted records are stored there: not even the services' names can be read.
# td7b59080 =

# Devices asking to join
# tff44c4a4 =

# Approve only if it shows this code.
# t8f54c79f =

# Approve
te6edf55a = Подтвердить

# Refuse
t9bcada35 = Отклонять

# Join from this device
# t54904649 =

# When another vault syncs there already: ask a device that syncs, or type the vault's recovery key. Its sync passphrase works too.
# te7a28cec =

# When another vault syncs there already: ask a device that syncs, or type the vault's recovery key.
# t2d6ea251 =

# Joining…
taa884c51 = Подключение…

# Join with the key
# t9f857a57 =

# Sync now
ta34dd8fe = Синхронизировать

# its password is not saved on this device: connect again in Settings › Sync
td2fdfc60 = его пароль не сохранён на этом устройстве: подключитесь снова в Настройки › Синхронизация

# settings not synced: {e}
tf8592ea2 = настройки не синхронизированы: { $e }

# settings not written: {e}
t60f438b7 = настройки не записаны: { $e }

# Find a code
# t8b2733e6 =

# Passphrase or recovery key
# t4774284e =

# A device asks to join: open Sync (the cloud) to approve it.
# tf1bf552a =

# Typed into the page.
# t97706593 =

# Not typed into the page: {why}
# t709fb554 =

# Not syncing
# t0f346f03 =

# Syncing…
t8ad2ae06 = Синхронизация…

# Synced just now
# t4df6f903 =

# Synced {n} minute ago | Synced {n} minutes ago
tc7922c74 =
    { $n ->
        [one] Синхронизировано { $n } минуту назад
        [few] Синхронизировано { $n } минуты назад
        [many] Синхронизировано { $n } минут назад
       *[other] Синхронизировано { $n } минуты назад
    }

# {when}: {pulled} in, {pushed} out
# t9dc23f47 =

# Sync failed: {e}
# tcc341778 =

# Joined: this device syncs the vault now.
# t6b25e007 =

# The counter was not saved: {e}
# tb28eb5cc =

# A code was not saved: {e}
# t6cd84653 =

# {n} code added. | {n} codes added.
tcb4c07d0 =
    { $n ->
        [one] Добавлен { $n } код.
        [few] Добавлено { $n } кода.
        [many] Добавлено { $n } кодов.
       *[other] Добавлено { $n } кода.
    }

# Delete the code for {name}?
# t1690127f =

# Turn two-factor sign-in off at {name} first, or sign in there with a recovery code: without this code you cannot sign in. Your other devices lose it too.
# t28110a8b =

# Delete
t5797ea6a = Удалить

# Not deleted: {e}
t12e9b880 = Не удалено: { $e }

# The vault's key is in this computer's keychain, so it cannot lock. Set a passphrase and take the key out of the keychain to lock it.
# ta55f9fc6 =

# Not opened: {e}
# t2e0269c8 =

# Cannot open config.toml: {e}
# tb78f406b =

# Lock
# t71b42d62 =

# Settings (config.toml)
# t1fc961bb =

# All codes
# t113331ee =

# Groups
# tf57f458d =

# Not syncing: set [sync] in config.toml
# t3e02a97b =

# next {code}
# tb1be48b7 =

# Copied
# t8dc21305 =

# Counter
# tfc0f33c3 =

# This page is not {site}
# t2f4582ba =

# {page} only looks like it. Pages like this copy a real sign-in to take your code: none is offered here.
# t6ae50f4f =

# its site
# tb8141570 =

# a rule
# tf5dd541a =

# chosen here before
# td89d57b6 =

# its name in the title
# t04583a8a =

# Matched by {why}
# tf0c6b3bc =

# For the window used last: {place}
# t8f247c27 =

# No codes yet
# t33ff9c8a =

# Turn on two-factor sign-in at a site, then paste the link of the QR code it shows.
# t2d0ad3d3 =

# Add your first code
# t4a8c2b9a =

# Nothing matches
# tda76133f =

# Try fewer words, or another group.
# t289325a3 =

# Set a passphrase
# t69755c3c =

# This computer has no keychain to keep the vault's key in.
# t911809a4 =

# The vault is locked
t78b7a6ee = Хранилище заблокировано

# Type its passphrase, or the recovery key you saved.
# t2c0f4729 =

# The vault cannot open
# te97e7be1 =

# Opening the vault…
# td46e3d6c =

# Open with Touch ID or Windows Hello
# t99b658f7 =

# Opening…
# tc40f2321 =

# Open
t538b10e9 = Открыть
