# authwing in Vietnamese. Each message is found by its English, the
# comment above it; scripts/i18n.py keeps this file in step with the code.

# otpauth:// link, or the key the site shows
tbfa1bb03 = Link otpauth://, hoặc khoá mà trang web hiện ra

# None
t304ff7fb = Không

# Paste the code's link, or type its key.
te8e7a37a = Dán link của mã, hoặc gõ khoá của nó.

# This is neither a link nor a key: {e}
t141e1cf7 = Đây không phải link cũng không phải khoá: { $e }

# Say whose code it is: the service, or the account.
tdb7a122d = Cho biết mã này của ai: dịch vụ, hoặc tài khoản.

# The export holds no code authwing can use ({n} left out). | The export holds no code authwing can use ({n} left out).
ta13f22fd =
    { $n ->
       *[other] File export không có mã nào authwing dùng được ({ $n } mã bị bỏ qua).
    }

# Add a code
t82066414 = Thêm mã

# When a site turns on two-factor sign-in it shows a QR code, and a link or a key beside it. Paste either here; a QR code from your phone's camera gives the link too.
t0378be46 = Khi bật đăng nhập hai lớp, trang web hiện một mã QR, kèm một link hoặc một khoá bên cạnh. Dán cái nào cũng được; quét mã QR bằng camera điện thoại cũng ra link đó.

# Link or key
td5c6ff95 = Link hoặc khoá

# Service
tb643c704 = Dịch vụ

# Account
t59f8a2fc = Tài khoản

# Group
t0574902c = Nhóm

# Show it first on
te673b518 = Hiện trước tiên trên

# Cancel
t35afca3b = Huỷ

# Add
t9dc3aa14 = Thêm

# Cannot read the codes: {e}
t4caf769c = Không đọc được các mã: { $e }

# A code cannot be read: {e}
t855061c0 = Một mã không đọc được: { $e }

# Copy
t658f3664 = Sao chép

# Cut
t58e25863 = Cắt

# Paste
tb34a0288 = Dán

# Select all
t6af21072 = Chọn tất cả

# {key} alone is for typing: hold Cmd, Ctrl or Option with it.
ta81ce4ca = { $key } riêng lẻ là để gõ chữ: hãy giữ kèm Cmd, Ctrl hoặc Option.

# {key} alone is for typing: hold Ctrl or Alt with it.
t834f7024 = { $key } riêng lẻ là để gõ chữ: hãy giữ kèm Ctrl hoặc Alt.

# {key} belongs to the system: it never reaches Authwing.
t67688898 = { $key } thuộc về hệ thống: nó không bao giờ tới được authwing.

# {key} is {what}, which cannot be changed.
t01906ac3 = { $key } là { $what }, phím này không đổi được.

# config.toml, [shortcuts]: no shortcut is named `{name}`.
t331b3467 = config.toml, [shortcuts]: không có phím tắt nào tên `{ $name }`.

# config.toml, [shortcuts] {name}: `{key}`: {error}
tb915b010 = config.toml, [shortcuts] { $name }: `{ $key }`: { $error }

# config.toml, [shortcuts] {name}: {why}
t71adda1f = config.toml, [shortcuts] { $name }: { $why }

# config.toml, [shortcuts]: `{key}` is given to both {kept} and {lost}; {kept} keeps it.
te833da37 = config.toml, [shortcuts]: `{ $key }` được gán cho cả { $kept } và { $lost }; { $kept } giữ nó.

# config.toml, [shortcuts] {name}: `{key}` does not fit; only copy_code takes `digit` (1 to 9), and it takes nothing else.
t87e7ceec = config.toml, [shortcuts] { $name }: `{ $key }` không hợp; chỉ copy_code nhận `digit` (1 đến 9), và nó không nhận gì khác.

# XXXX-XXXX-…, 13 groups
tbb504d9f = XXXX-XXXX-…, 13 nhóm

# This vault has no recovery key yet.
tdd764294 = Vault này chưa có khoá khôi phục.

# Recovery key
t02904a59 = Khoá khôi phục

# Keep it where you keep important papers: it opens the vault from a new device, and it is the way back if every device is lost.
t82f98ccc = Hãy cất nó cùng chỗ với giấy tờ quan trọng: nó mở vault từ một thiết bị mới, và là cách lấy lại khi mất hết mọi thiết bị.

# Show the recovery key
t16f62979 = Hiện khoá khôi phục

# Hide
te10c7bcd = Ẩn

# Waiting for a device that syncs: open authwing there.
t33ac6aa2 = Đang chờ một thiết bị đang đồng bộ: hãy mở authwing ở đó.

# Approve on {device} if it shows the same code:
ta89ba4d1 = Duyệt trên { $device } nếu nó hiện cùng mã này:

# Joined. The vault's codes are coming.
t775b89a0 = Đã tham gia. Các mã của vault đang được tải về.

# The other device refused.
t74419d8e = Thiết bị kia đã từ chối.

# Nobody approved in time: ask again.
t5c323bb6 = Không ai duyệt kịp: hãy xin lại.

# Ask a device that syncs
t2dc61136 = Xin một thiết bị đang đồng bộ

# Stop asking
t296beec8 = Thôi xin

# Sync
t89a71bba = Đồng bộ

# Not syncing. Say where in config.toml, then come back here: a folder that syncs (iCloud Drive, Dropbox, Syncthing), WebDAV, S3 or Git.
tc75c83a8 = Chưa đồng bộ. Khai báo nơi đồng bộ trong config.toml rồi quay lại đây: một thư mục được đồng bộ (iCloud Drive, Dropbox, Syncthing), WebDAV, S3 hoặc Git.

# Close
tcd86acc3 = Đóng

# Only encrypted records are stored there: not even the services' names can be read.
td7b59080 = Ở đó chỉ lưu các bản ghi đã mã hoá: ngay cả tên dịch vụ cũng không đọc được.

# Devices asking to join
tff44c4a4 = Thiết bị đang xin tham gia

# Approve only if it shows this code.
t8f54c79f = Chỉ duyệt nếu nó hiện đúng mã này.

# Approve
te6edf55a = Đồng ý

# Refuse
t9bcada35 = Từ chối

# Join from this device
t54904649 = Tham gia từ thiết bị này

# When another vault syncs there already: ask a device that syncs, or type the vault's recovery key. Its sync passphrase works too.
te7a28cec = Khi ở đó đã có một vault khác đồng bộ: xin một thiết bị đang đồng bộ, hoặc gõ khoá khôi phục của vault. Mật khẩu đồng bộ của nó cũng dùng được.

# When another vault syncs there already: ask a device that syncs, or type the vault's recovery key.
t2d6ea251 = Khi ở đó đã có một vault khác đồng bộ: xin một thiết bị đang đồng bộ, hoặc gõ khoá khôi phục của vault.

# Joining…
taa884c51 = Đang tham gia…

# Join with the key
t9f857a57 = Tham gia bằng khoá

# Sync now
ta34dd8fe = Đồng bộ ngay

# its password is not saved on this device: connect again in Settings › Sync
td2fdfc60 = mật khẩu của nó chưa được lưu trên thiết bị này: hãy kết nối lại trong Cài đặt › Đồng bộ

# settings not synced: {e}
tf8592ea2 = chưa đồng bộ được cài đặt: { $e }

# settings not written: {e}
t60f438b7 = chưa ghi được cài đặt: { $e }

# Find a code
t8b2733e6 = Tìm mã

# Passphrase or recovery key
t4774284e = Mật khẩu hoặc khoá khôi phục

# A device asks to join: open Sync (the cloud) to approve it.
tf1bf552a = Có thiết bị xin tham gia: mở Đồng bộ (biểu tượng đám mây) để duyệt.

# Typed into the page.
t97706593 = Đã gõ vào trang.

# Not typed into the page: {why}
t709fb554 = Chưa gõ được vào trang: { $why }

# Not syncing
t0f346f03 = Chưa đồng bộ

# Syncing…
t8ad2ae06 = Đang đồng bộ…

# Synced just now
t4df6f903 = Vừa đồng bộ xong

# Synced {n} minute ago | Synced {n} minutes ago
tc7922c74 =
    { $n ->
       *[other] Đã đồng bộ { $n } phút trước
    }

# {when}: {pulled} in, {pushed} out
t9dc23f47 = { $when }: nhận { $pulled }, gửi { $pushed }

# Sync failed: {e}
tcc341778 = Đồng bộ thất bại: { $e }

# Joined: this device syncs the vault now.
t6b25e007 = Đã tham gia: thiết bị này giờ đồng bộ vault.

# The counter was not saved: {e}
tb28eb5cc = Bộ đếm chưa được lưu: { $e }

# A code was not saved: {e}
t6cd84653 = Một mã chưa được lưu: { $e }

# {n} code added. | {n} codes added.
tcb4c07d0 =
    { $n ->
       *[other] Đã thêm { $n } mã.
    }

# Delete the code for {name}?
t1690127f = Xoá mã của { $name }?

# Turn two-factor sign-in off at {name} first, or sign in there with a recovery code: without this code you cannot sign in. Your other devices lose it too.
t28110a8b = Hãy tắt đăng nhập hai lớp ở { $name } trước, hoặc đăng nhập ở đó bằng mã khôi phục: thiếu mã này bạn sẽ không đăng nhập được. Các thiết bị khác của bạn cũng mất mã này.

# Delete
t5797ea6a = Xoá

# Not deleted: {e}
t12e9b880 = Chưa xoá: { $e }

# The vault's key is in this computer's keychain, so it cannot lock. Set a passphrase and take the key out of the keychain to lock it.
ta55f9fc6 = Khoá của vault đang nằm trong keychain của máy này nên không khoá được. Hãy đặt mật khẩu và lấy khoá ra khỏi keychain để có thể khoá.

# Not opened: {e}
t2e0269c8 = Chưa mở được: { $e }

# Cannot open config.toml: {e}
tb78f406b = Không mở được config.toml: { $e }

# Lock
t71b42d62 = Khoá

# Settings (config.toml)
t1fc961bb = Cài đặt (config.toml)

# All codes
t113331ee = Tất cả mã

# Groups
tf57f458d = Nhóm

# Not syncing: set [sync] in config.toml
t3e02a97b = Chưa đồng bộ: khai báo [sync] trong config.toml

# next {code}
tb1be48b7 = tiếp { $code }

# Copied
t8dc21305 = Đã copy

# Counter
tfc0f33c3 = Bộ đếm

# This page is not {site}
t2f4582ba = Trang này không phải { $site }

# {page} only looks like it. Pages like this copy a real sign-in to take your code: none is offered here.
t6ae50f4f = { $page } chỉ trông giống thôi. Những trang như vậy bắt chước trang đăng nhập thật để lấy mã của bạn: ở đây không có mã nào được đưa ra.

# its site
tb8141570 = trang web của nó

# a rule
tf5dd541a = một quy tắc

# chosen here before
td89d57b6 = đã chọn ở đây trước đó

# its name in the title
t04583a8a = tên của nó trong tiêu đề

# Matched by {why}
tf0c6b3bc = Khớp theo { $why }

# For the window used last: {place}
t8f247c27 = Cho cửa sổ vừa dùng: { $place }

# No codes yet
t33ff9c8a = Chưa có mã nào

# Turn on two-factor sign-in at a site, then paste the link of the QR code it shows.
t2d0ad3d3 = Bật đăng nhập hai lớp ở một trang web, rồi dán link của mã QR mà trang đó hiện ra.

# Add your first code
t4a8c2b9a = Thêm mã đầu tiên

# Nothing matches
tda76133f = Không có kết quả

# Try fewer words, or another group.
t289325a3 = Thử ít từ hơn, hoặc nhóm khác.

# Set a passphrase
t69755c3c = Đặt mật khẩu

# This computer has no keychain to keep the vault's key in.
t911809a4 = Máy này không có keychain để giữ khoá của vault.

# The vault is locked
t78b7a6ee = Kho đang khoá

# Type its passphrase, or the recovery key you saved.
t2c0f4729 = Gõ mật khẩu của nó, hoặc khoá khôi phục bạn đã lưu.

# The vault cannot open
te97e7be1 = Không mở được vault

# Opening the vault…
td46e3d6c = Đang mở vault…

# Open with Touch ID or Windows Hello
t99b658f7 = Mở bằng Touch ID hoặc Windows Hello

# Opening…
tc40f2321 = Đang mở…

# Open
t538b10e9 = Mở
