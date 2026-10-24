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

# macOS asks first: turn authwing on in System Settings › Privacy & Security › Screen Recording, then open authwing again.
t878591bf = macOS hỏi trước: bật authwing trong Cài đặt hệ thống › Quyền riêng tư & Bảo mật › Ghi màn hình, rồi mở lại authwing.

# Read
t4f0befe5 = Đọc

# This QR code is not a two-factor code.
t9c06ff6c = Mã QR này không phải mã hai yếu tố.

# No QR code on the screen: show the site's page with it, and scan again.
t3aeb1d3b = Không có mã QR nào trên màn hình: hãy mở trang của site có mã đó, rồi quét lại.

# No QR code in this image.
tb5e0504c = Không có mã QR nào trong ảnh này.

# One of {n} two-factor QR codes: add it, then scan again for the next. | One of {n} two-factor QR codes: add it, then scan again for the next.
t4a7417f5 =
    { $n ->
       *[other] Một trong { $n } mã QR hai yếu tố: hãy thêm mã này, rồi quét lại để lấy mã tiếp theo.
    }

# Read from the QR code. Check the names, then add it.
t494fced8 = Đã đọc từ mã QR. Kiểm tra tên, rồi thêm.

# The screen was not shared, so nothing was read.
t4c77286d = Màn hình không được chia sẻ nên không đọc được gì.

# Not read: {why}
t9133669c = Không đọc được: { $why }

# This is not an image authwing can read.
t5d87f9dd = Đây không phải ảnh authwing đọc được.

# Add a code
t82066414 = Thêm mã

# When a site turns on two-factor sign-in it shows a QR code, and often a key beside it. Read the QR code where it is, or paste its link or the key.
t33420425 = Khi một site bật đăng nhập hai yếu tố, nó hiện một mã QR, và thường kèm một khoá. Hãy đọc mã QR ngay tại chỗ, hoặc dán link của nó hay khoá.

# Reading…
t6aad8485 = Đang đọc…

# Scan the screen
t1eac14ed = Quét màn hình

# Choose an image…
t4b006b9a = Chọn ảnh…

# or paste or drop one here
t0591d55d = hoặc dán, kéo thả ảnh vào đây

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

# Hide authwing
tc980ec1e = Ẩn authwing

# Hide Others
tb1ea29d0 = Ẩn ứng dụng khác

# Show All
ta23c130f = Hiện tất cả

# Quit authwing
t92d4b253 = Thoát authwing

# Edit
tc2c76cb1 = Sửa

# Select All
tcd7e3b12 = Chọn tất cả

# a key alone, or with Shift alone, is for typing
t0d0135fa = một phím đơn, hoặc chỉ kèm Shift, là để gõ chữ

# Type to find, or press Enter
te54f3559 = Gõ để tìm, hoặc nhấn Enter

# Matches this window
t127fa712 = Khớp với cửa sổ này

# Others
t69bad712 = Mã khác

# No codes yet: the + in authwing's window adds them.
tdd6e3098 = Chưa có mã nào: nút + trong cửa sổ authwing để thêm.

# No code matches.
t2c168854 = Không có mã nào khớp.

# {page} is not {site}: no code is typed there.
t4b979ea8 = { $page } không phải { $site }: không gõ mã nào vào đó.

# type into {app}
tc5f5d5a6 = gõ vào { $app }

# copy
te562ea44 = sao chép

# close
t27cb3b23 = đóng

# Allow Accessibility to type codes in
t5e806dae = Cho phép Trợ năng để gõ mã vào

# General
t5561fd0b = Chung

# Sync
t89a71bba = Đồng bộ

# Security
tc2fe21db = Bảo mật

# Backup
t06bfc1ff = Sao lưu

# Browsers
td580efd4 = Trình duyệt

# About
t58e096de = Giới thiệu

# A folder another app keeps in sync
t27689c98 = Một thư mục được app khác đồng bộ

# The repository's default
t318b30d3 = Nhánh mặc định của repository

# None: the bucket's top
tb62e6c7b = Để trống: gốc của bucket

# Access key
t837b43b8 = Access key

# Kept in the vault, on this device only
t4893a866 = Giữ trong vault, chỉ trên thiết bị này

# At least 10 characters, hard to guess
tea4f3495 = Ít nhất 10 ký tự, khó đoán

# The same again
t3d35edbe = Nhập lại lần nữa

# A passphrase for this backup
t8f817889 = Mật khẩu cho bản sao lưu này

# The file's password
teef70b9d = Mật khẩu của file

# Fill in {what}.
t0804c32b = Hãy điền { $what }.

# the folder
t48f6f260 = thư mục

# the repository's address
t5f845286 = địa chỉ repository

# the folder's address
t3f263e4e = địa chỉ thư mục

# the user
t9ba42c97 = tên người dùng

# the endpoint
t025e0e85 = endpoint

# the bucket
ta347465a = bucket

# the access key
t4e629821 = access key

# Open the vault first.
t18fc18b2 = Hãy mở vault trước.

# Fill in the password.
t396bf9b1 = Hãy điền mật khẩu.

# Fill in the secret key.
tfddf418b = Hãy điền secret key.

# Saved. Syncing now.
t3bd89f83 = Đã lưu. Đang đồng bộ.

# Sync is off. What is in the place stays there.
t9350814a = Đã tắt đồng bộ. Những gì ở nơi đồng bộ vẫn được giữ nguyên.

# Sync here
te998c2e0 = Đồng bộ ở đây

# This vault has no recovery key yet.
tdd764294 = Vault này chưa có khoá khôi phục.

# A passphrase needs at least 10 characters.
t848f2c32 = Mật khẩu cần ít nhất 10 ký tự.

# The two passphrases differ.
teb0402b1 = Hai mật khẩu không khớp nhau.

# Too easy to guess: try several unrelated words.
tc80e0d6d = Quá dễ đoán: hãy dùng vài từ không liên quan đến nhau.

# The passphrase is set.
t1c4d5a62 = Đã đặt mật khẩu.

# Saved: {path}
tac1104ef = Đã lưu: { $path }

# There are no codes to save yet.
t0e74f33a = Chưa có mã nào để lưu.

# Save unencrypted
td1b35071 = Lưu không mã hoá

# Save the codes unencrypted?
tfb35b299 = Lưu các mã mà không mã hoá?

# Anyone who sees the file can make your codes. Use it to move to another app, then delete it.
tf4321e21 = Ai thấy file này cũng tạo được mã của bạn. Chỉ dùng để chuyển sang app khác, rồi xoá nó đi.

# Import
t573e0af4 = Nhập

# {n} code added. | {n} codes added.
tcb4c07d0 =
    { $n ->
       *[other] Đã thêm { $n } mã.
    }

# {n} was here already. | {n} were here already.
tebfc485d =
    { $n ->
       *[other] { $n } mã đã có sẵn.
    }

# {n} left out: authwing cannot use it. | {n} left out: authwing cannot use them.
t9292bf58 =
    { $n ->
       *[other] { $n } mã bị bỏ qua: authwing không dùng được.
    }

# No browser the extension runs in was found.
tbdee3baa = Không tìm thấy trình duyệt nào chạy được tiện ích.

# As the system
t29f671f7 = Theo hệ thống

# Dark
t45570065 = Tối

# Light
t8ad2f8cf = Sáng

# 10 s
t656f91cd = 10 giây

# 20 s
t9356dfda = 20 giây

# 45 s
td029e48f = 45 giây

# 90 s
ted25e5d5 = 90 giây

# Never
tb2ff2b29 = Không bao giờ

# Language
t9a73db9b = Ngôn ngữ

# This device's own: it does not sync.
t075c5383 = Riêng của thiết bị này: không đồng bộ.

# Appearance
t31018b9f = Giao diện

# Colors
t25bb4ed1 = Màu

# Text size
t2c041143 = Cỡ chữ

# Quick panel
t3fea61e2 = Bảng tra nhanh

# Codes
t758add25 = Mã

# Clear a copied code from the clipboard after
tcf387b5b = Xoá mã đã copy khỏi clipboard sau

# Only if the clipboard still holds it.
tedfee6ca = Chỉ khi clipboard vẫn còn giữ mã đó.

# Off
t0dc6b4aa = Tắt

# Shortcut
t97dcc893 = Phím tắt

# Opens it over any app, with that app's codes first. This device's own.
t5648409e = Mở bảng trên bất kỳ ứng dụng nào, mã của ứng dụng đó lên đầu. Riêng của thiết bị này.

# Type codes into other apps
t8562df4f = Gõ mã vào ứng dụng khác

# Allowed: Enter in the quick panel types the code where you were.
t9beca5b0 = Đã cho phép: Enter trong bảng tra nhanh gõ mã vào chỗ bạn vừa ở.

# macOS asks first (Accessibility). Until then, Enter copies the code.
t4fcdf7b5 = macOS hỏi trước (Trợ năng). Đến lúc đó, Enter sao chép mã.

# Enter in the quick panel types the code where you were.
tb88bea64 = Enter trong bảng tra nhanh gõ mã vào chỗ bạn vừa ở.

# Allow…
taa765d22 = Cho phép…

# Folder
t06357afd = Thư mục

# Git
tb6b5540b = Git

# WebDAV
t8e2d2e34 = WebDAV

# S3
t060203c3 = S3

# Saved on this device; type a new one to change it.
t6f6ad6e7 = Đã lưu trên thiết bị này; gõ cái mới để thay.

# Kept sealed in the vault, on this device only: it never syncs.
t5529829d = Được niêm phong trong vault, chỉ trên thiết bị này: không bao giờ đồng bộ.

# The codes stay on this device. Keep its recovery key (Security) and a backup.
t29381d6c = Các mã chỉ ở trên thiết bị này. Hãy giữ khoá khôi phục (Bảo mật) và một bản sao lưu.

# Choose…
t58daae9c = Chọn…

# Use iCloud Drive
t789420ac = Dùng iCloud Drive

# Any folder another app keeps in sync: iCloud Drive, Syncthing, Dropbox, OneDrive, a network share.
taadf45c8 = Bất kỳ thư mục nào được app khác đồng bộ: iCloud Drive, Syncthing, Dropbox, OneDrive, thư mục mạng.

# Repository
te763c8b5 = Repository

# Branch
ta3e27665 = Nhánh

# A private repository. This computer's git signs in as it does for you.
tcf6c891b = Một repository riêng tư. git của máy này đăng nhập như bình thường bạn vẫn dùng.

# Folder's address
t1331c053 = Địa chỉ thư mục

# User
te0a63e12 = Người dùng

# Password
t2cc30838 = Mật khẩu

# Endpoint
t27e7d69c = Endpoint

# Bucket
te34e3557 = Bucket

# Folder in it
tdfe63979 = Thư mục bên trong

# Region
t492e789d = Region

# Secret key
t47252d7a = Secret key

# Where the vault syncs
td7a153f7 = Nơi vault đồng bộ

# Save and sync
te0b44954 = Lưu và đồng bộ

# Stop syncing
t59fdbd70 = Dừng đồng bộ

# What syncs
ta2fbb093 = Những gì được đồng bộ

# Settings too
ta8aa81ba = Cả cài đặt

# Colors, text size and the clipboard's time. The language, shortcuts and the place stay this device's own.
te0e876b2 = Màu, cỡ chữ và thời gian giữ clipboard. Ngôn ngữ, phím tắt và nơi đồng bộ là riêng của từng thiết bị.

# Devices joining, and the recovery key, are in the cloud's panel in the title bar.
t674d2276 = Thiết bị xin tham gia và khoá khôi phục nằm trong bảng của biểu tượng đám mây trên thanh tiêu đề.

# Show the recovery key
t16f62979 = Hiện khoá khôi phục

# Hide
te10c7bcd = Ẩn

# Change the passphrase
t02c0bf0a = Đổi mật khẩu

# Set a passphrase
t69755c3c = Đặt mật khẩu

# Recovery key
t02904a59 = Khoá khôi phục

# It opens the vault when nothing else can: a lost keychain, a forgotten passphrase, a new device. Write it down and keep it apart from this computer.
tc3c501b7 = Nó mở vault khi không còn cách nào khác: mất keychain, quên mật khẩu, thiết bị mới. Hãy chép lại và cất ở nơi tách biệt với máy này.

# Passphrase
t191a2dc5 = Mật khẩu

# A passphrase opens this vault. Set another below.
tc2369f61 = Vault này được mở bằng mật khẩu. Đặt mật khẩu khác ở dưới.

# No passphrase yet: the keychain alone opens the vault, and it cannot lock.
t4373fdd8 = Chưa có mật khẩu: chỉ keychain mở được vault, và vault không thể khoá.

# Again
t58fb56ef = Nhập lại

# Working…
td5507854 = Đang xử lý…

# Save the passphrase
t014d0621 = Lưu mật khẩu

# Opening and locking
tb34987b3 = Mở và khoá

# Open by itself
t680c6531 = Tự mở

# The vault's key stays in this computer's keychain. Off: the passphrase opens it, and Lock closes it.
ta2c3bb9f = Khoá của vault nằm trong keychain của máy này. Tắt: mật khẩu mở vault, và nút Khoá đóng nó lại.

# Open with {what}
td5998b93 = Mở bằng { $what }

# Needs a passphrase set. The system asks when you change it.
t5b1f668d = Cần đặt mật khẩu trước. Hệ thống sẽ hỏi khi bạn thay đổi.

# Lock now
t1aa78686 = Khoá ngay

# Closes the vault until its passphrase is typed.
tba4f5365 = Đóng vault cho tới khi gõ mật khẩu.

# Lock
t71b42d62 = Khoá

# {file} is an encrypted {app} file.
t3635feb7 = { $file } là file { $app } đã mã hoá.

# Open
t538b10e9 = Mở

# A backup
t2e1d133a = Bản sao lưu

# Every code, its group and its sites, sealed with a passphrase of its own: keep it somewhere else than this computer. Restoring it needs only the file and the passphrase.
t4d83bdf7 = Mọi mã, cùng nhóm và các trang web của nó, được niêm phong bằng một mật khẩu riêng: hãy cất ở nơi khác ngoài máy này. Khôi phục chỉ cần file và mật khẩu.

# Backup passphrase
teeca199d = Mật khẩu bản sao lưu

# Save a backup…
tbc59ad3d = Lưu bản sao lưu…

# Restore, or move from another app
tea7de02f = Khôi phục, hoặc chuyển từ app khác

# An authwing backup, or an export from Aegis, 2FAS, Ente Auth or Bitwarden, or a list of otpauth links. Codes already here are left as they are. Authy cannot export its codes.
t4f923eb7 = Một bản sao lưu authwing, file export từ Aegis, 2FAS, Ente Auth hoặc Bitwarden, hoặc một danh sách link otpauth. Mã đã có sẵn được giữ nguyên. Authy không xuất được mã.

# Choose a file…
t28a576c9 = Chọn file…

# Unencrypted
t39b38862 = Không mã hoá

# Save as plain text
tc2fb9143 = Lưu dạng văn bản thường

# One otpauth link per code, which other apps import. Anyone who sees the file can make your codes.
te3b8adae = Mỗi mã một link otpauth, các app khác nhập được. Ai thấy file này cũng tạo được mã của bạn.

# Save…
t6dec18f4 = Lưu…

# Linked to this authwing
ta9f58dac = Đã liên kết với authwing này

# Linked to another copy: {path}
t92cd5320 = Đã liên kết với một bản authwing khác: { $path }

# Not linked
ta8dc0817 = Chưa liên kết

# The browser extension
tdcceb399 = Tiện ích trình duyệt

# With authwing's extension, the app knows the page in front exactly: a code saved with that site comes first and is typed in, and a page that only looks like it gets none. The extension keeps no codes.
t9ca9ffff = Với tiện ích của authwing, app biết chính xác trang đang mở: mã đã lưu cho trang đó được đưa lên đầu và gõ vào trang, còn trang chỉ trông giống thì không nhận được mã nào. Tiện ích không giữ mã nào.

# Link the browsers
t813a0fa7 = Liên kết các trình duyệt

# Unlink
ta3742172 = Bỏ liên kết

# Browsers on this computer
td2a9db60 = Trình duyệt trên máy này

# The extension is not in the stores yet: build it from the repository's extension folder and load it unpacked.
t49b23b02 = Tiện ích chưa có trên các store: hãy build từ thư mục extension của repository và nạp dạng unpacked.

# Version
t5dcdd537 = Phiên bản

# License
t38e70ee4 = Giấy phép

# Free software: you may share and change it under the GPL.
tef971997 = Phần mềm tự do: bạn có thể chia sẻ và sửa đổi theo giấy phép GPL.

# Source
t61e2a3f8 = Mã nguồn

# Updates
t483075a5 = Cập nhật

# Not looked for: this build has no release keys to check them with.
t208f65bc = Không tìm cập nhật: bản build này chưa có khoá release để kiểm tra.

# Open config.toml
t07a52335 = Mở config.toml

# Done
t8dd31791 = Xong

# That password does not open this file.
t90cd5647 = Mật khẩu đó không mở được file này.

# The file is damaged: {why}
t95bba871 = File bị hỏng: { $why }

# This is not a backup or an export authwing can read.
td41d3da6 = Đây không phải bản sao lưu hay file export mà authwing đọc được.

# The passphrase is too short: at least {n} characters.
t67c24024 = Mật khẩu quá ngắn: cần ít nhất { $n } ký tự.

# This {app} file is encrypted: enter its password.
t1aa6da9b = File { $app } này đã được mã hoá: hãy nhập mật khẩu.

# XXXX-XXXX-…, 13 groups
tbb504d9f = XXXX-XXXX-…, 13 nhóm

# Keep it where you keep important papers: it opens the vault from a new device, and it is the way back if every device is lost.
t82f98ccc = Hãy cất nó cùng chỗ với giấy tờ quan trọng: nó mở vault từ một thiết bị mới, và là cách lấy lại khi mất hết mọi thiết bị.

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

# The quick panel's shortcut {key} cannot be used: {e}
td4cbf886 = Không dùng được phím tắt { $key } cho bảng tra nhanh: { $e }

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

# Not typed in: {e}
tf4e0ecfa = Chưa gõ được: { $e }

# A code was not saved: {e}
t6cd84653 = Một mã chưa được lưu: { $e }

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

# Settings
t4b058728 = Cài đặt

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

# Matched by {why}
tf0c6b3bc = Khớp theo { $why }

# For the window used last: {place}
t8f247c27 = Cho cửa sổ vừa dùng: { $place }

# No codes yet
t33ff9c8a = Chưa có mã nào

# Turn on two-factor sign-in at a site, then read the QR code it shows.
t34aa395e = Bật đăng nhập hai yếu tố ở một site, rồi đọc mã QR nó hiện.

# Add your first code
t4a8c2b9a = Thêm mã đầu tiên

# Nothing matches
tda76133f = Không có kết quả

# Try fewer words, or another group.
t289325a3 = Thử ít từ hơn, hoặc nhóm khác.

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

# its site
tb8141570 = trang web của nó

# a rule
tf5dd541a = một quy tắc

# chosen here before
td89d57b6 = đã chọn ở đây trước đó

# its name in the title
t04583a8a = tên của nó trong tiêu đề
