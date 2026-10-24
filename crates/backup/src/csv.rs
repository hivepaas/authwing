//! The CSV exports of password managers that keep a login's key for
//! one-time passwords in a column of its own. Which app wrote one shows in
//! its header row, by the names of its columns:
//!
//! - Apple Passwords (the Passwords app of macOS 15 and iOS 18, and Safari
//!   before it): `Title,URL,Username,Password,Notes,OTPAuth`.
//! - 1Password: `Title,Url,Username,Password,OTPAuth,Favorite,Archived,Tags,Notes`
//!   (https://support.1password.com/export/ lists the fields), the tags
//!   joined by `;`.
//! - Bitwarden: `folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp`,
//!   a login's addresses joined by commas in `login_uri`.
//! - KeePassXC: `Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created`,
//!   the group a path from the database's root group
//!   (https://github.com/keepassxreboot/keepassxc, `src/format/CsvExporter.cpp`).
//! - Proton Pass: `type,name,url,autofillUrls,email,username,password,note,totp,createTime,modifyTime,vault`
//!   (`autofillUrls` only in later versions), the addresses joined by `, `
//!   in `url` (https://github.com/ProtonMail/WebClients,
//!   `packages/pass/lib/export/csv.ts`).
//! - Dashlane: `username,username2,username3,title,password,note,url,category,otpUrl`,
//!   the key `otpSecret` in older exports and `otp` in its iOS app's, and
//!   the item's collections joined by `, ` in `category`
//!   (https://github.com/Dashlane/dashlane-web-extension,
//!   `packages/carbon/src/DataManagement/Export/Services/get-credentials-export.ts`).
//!   This is its `credentials.csv`, which Dashlane exports in a ZIP of CSV
//!   files: the ZIP reads too.
//!
//! Columns are found by name, in any order and case, so a version that adds
//! or moves one still reads, and so does another app's CSV with a column
//! of keys by one of these names ("a CSV export").
//!
//! A key is what Bitwarden takes ([`login::code`]): an `otpauth://` URI, or
//! a bare secret, which makes a 30-second, six-digit SHA-1 code. The row's
//! title stands in for an issuer the key does not give, and its username
//! (or its email) for an account. The hosts of its addresses become the
//! code's sites, and what the app files it under its group: the folder, the
//! group (KeePassXC's, without the root), the vault (Proton Pass), the
//! first collection (Dashlane) or the first tag (1Password). Rows without a
//! key are not codes, and are passed over without a word; 1Password's
//! archived ones come in.

use ::csv::{ReaderBuilder, StringRecord};

use crate::archive::{self, Archive};
use crate::{Format, Imported, Item, Result, damaged, label, login, without_bom};

/// The CSV of logins in Dashlane's ZIP.
pub(crate) const DASHLANE: &str = "credentials.csv";

/// The names, in lower case, of a column of keys, the first one there
/// taken; of the row's title; of its addresses; of its account, the first
/// one not empty taken; and of what it is filed under.
const KEYS: [&str; 6] = [
    "otpauth",
    "login_totp",
    "totp",
    "otpurl",
    "otpsecret",
    "otp",
];
const TITLES: [&str; 2] = ["title", "name"];
const ADDRESSES: [&str; 2] = ["url", "login_uri"];
const ACCOUNTS: [&str; 5] = [
    "username",
    "login_username",
    "email",
    "username2",
    "username3",
];
const GROUPS: [&str; 5] = ["folder", "group", "vault", "category", "tags"];

/// A password manager whose CSV export authwing knows by its header.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PasswordManager {
    /// Apple's Passwords app, and Safari's export before it.
    ApplePasswords,
    OnePassword,
    Bitwarden,
    KeePassXc,
    ProtonPass,
    Dashlane,
}

impl PasswordManager {
    /// The app's name, to show.
    pub fn name(self) -> &'static str {
        match self {
            PasswordManager::ApplePasswords => "Apple Passwords",
            PasswordManager::OnePassword => "1Password",
            PasswordManager::Bitwarden => "Bitwarden",
            PasswordManager::KeePassXc => "KeePassXC",
            PasswordManager::ProtonPass => "Proton Pass",
            PasswordManager::Dashlane => "Dashlane",
        }
    }
}

/// Where a header puts what is read, and which app's it is.
struct Columns {
    app: Option<PasswordManager>,
    key: usize,
    title: Option<usize>,
    addresses: Option<usize>,
    accounts: Vec<usize>,
    group: Option<usize>,
}

impl Columns {
    /// The columns `header` names: `None` when none holds keys.
    fn of(header: &StringRecord) -> Option<Columns> {
        let names: Vec<String> = header
            .iter()
            .map(|name| name.trim().to_ascii_lowercase())
            .collect();
        let find = |name: &str| names.iter().position(|known| known == name);
        let first = |names: &[&str]| names.iter().find_map(|name| find(name));
        let has = |name: &str| find(name).is_some();
        let key = first(&KEYS)?;
        let app = if has("login_totp") {
            Some(PasswordManager::Bitwarden)
        } else if has("otpauth") && has("archived") {
            Some(PasswordManager::OnePassword)
        } else if has("otpauth") && has("title") && has("url") {
            Some(PasswordManager::ApplePasswords)
        } else if has("totp") && has("group") && has("title") {
            Some(PasswordManager::KeePassXc)
        } else if has("totp") && has("vault") && has("type") {
            Some(PasswordManager::ProtonPass)
        } else if has("username2") && (has("otpurl") || has("otpsecret") || has("otp")) {
            Some(PasswordManager::Dashlane)
        } else {
            None
        };
        Some(Columns {
            app,
            key,
            title: first(&TITLES),
            addresses: first(&ADDRESSES),
            accounts: ACCOUNTS.iter().filter_map(|name| find(name)).collect(),
            group: first(&GROUPS),
        })
    }

    /// The addresses in a row's `cell`, which some apps join.
    fn addresses<'a>(&self, cell: &'a str) -> Vec<&'a str> {
        match self.app {
            Some(PasswordManager::Bitwarden | PasswordManager::ProtonPass) => {
                cell.split(',').collect()
            }
            _ => vec![cell],
        }
    }

    /// The group a row's `cell` files its code under.
    fn group(&self, cell: &str) -> String {
        let group = match self.app {
            Some(PasswordManager::KeePassXc) => cell.split_once('/').map_or("", |(_, path)| path),
            Some(PasswordManager::OnePassword) => cell.split(';').next().unwrap_or_default(),
            Some(PasswordManager::Dashlane) => cell.split(", ").next().unwrap_or_default(),
            _ => cell,
        };
        group.trim().to_string()
    }
}

/// A reader of CSV as these apps write it: rows may be short, and quotes,
/// commas and line breaks in quoted fields are the field's.
fn reader(text: &str) -> ::csv::Reader<&[u8]> {
    ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes())
}

/// Which app's CSV export `text` is, if its header names a column of keys.
pub(crate) fn detect(text: &str) -> Option<Format> {
    let mut reader = reader(text);
    let columns = Columns::of(reader.headers().ok()?)?;
    Some(Format::Csv(columns.app))
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    if archive::is_zip(bytes) {
        let csv = Archive::open(bytes)?.read(DASHLANE, None)?;
        return read(without_bom(&csv));
    }
    read(bytes)
}

fn read(bytes: &[u8]) -> Result<Imported> {
    let text = std::str::from_utf8(bytes).map_err(damaged)?;
    let mut reader = reader(text);
    let columns = Columns::of(reader.headers().map_err(damaged)?)
        .ok_or_else(|| damaged("no column of one-time passwords"))?;

    let mut imported = Imported::default();
    for (n, record) in reader.records().enumerate() {
        // The header is row 1.
        let row_no = || format!("row {}", n + 2);
        let record = match record {
            Ok(record) => record,
            Err(e) => {
                imported.skipped.push((row_no(), e.to_string()));
                continue;
            }
        };
        let cell = |column: Option<usize>| {
            column
                .and_then(|column| record.get(column))
                .unwrap_or_default()
                .trim()
        };
        let key = cell(Some(columns.key));
        if key.is_empty() {
            continue;
        }
        let title = cell(columns.title);
        let account = columns
            .accounts
            .iter()
            .map(|&column| cell(Some(column)))
            .find(|account| !account.is_empty())
            .unwrap_or_default();
        match login::code(key, title, account) {
            Ok(otp) => imported.items.push(Item {
                otp,
                group: columns.group(cell(columns.group)),
                sites: login::sites(columns.addresses(cell(columns.addresses))),
            }),
            Err(e) => imported
                .skipped
                .push((label(title, account, row_no), e.to_string())),
        }
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    //! `bitwarden.csv` is Aegis's (see `crate::tests`); the other samples
    //! are written from each app's header, as the module docs give it, with
    //! some of the codes of Aegis's samples.

    use authwing_totp::Kind;

    use super::*;
    use crate::testing::{URIS, assert_codes, code, zipped};

    /// What each item is named and filed under, and its sites.
    fn filed(imported: &Imported) -> Vec<(String, String, String, Vec<String>)> {
        imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                (
                    otp.issuer.clone(),
                    otp.account.clone(),
                    item.group.clone(),
                    item.sites.clone(),
                )
            })
            .collect()
    }

    fn row(
        issuer: &str,
        account: &str,
        group: &str,
        sites: &[&str],
    ) -> (String, String, String, Vec<String>) {
        let sites = sites.iter().map(|site| site.to_string()).collect();
        (issuer.into(), account.into(), group.into(), sites)
    }

    fn import(bytes: &[u8], app: PasswordManager) -> Imported {
        assert_eq!(crate::detect(bytes), Some(Format::Csv(Some(app))));
        let imported = crate::import(bytes, None).expect("import");
        assert!(imported.skipped.is_empty(), "{:?}", imported.skipped);
        imported
    }

    #[test]
    fn reads_bitwardens_csv() {
        let bytes = include_bytes!("../testdata/bitwarden.csv");
        let imported = import(bytes, PasswordManager::Bitwarden);
        assert_codes(&imported.items[..3], &URIS[..3]);
        let steam = &imported.items[3].otp;
        assert_eq!(steam.kind, Kind::Steam);
        assert_eq!(steam.secret, code(URIS[6]).secret);
        // The key gives no names: the login's stand in.
        assert_eq!(
            (steam.issuer.as_str(), steam.account.as_str()),
            ("Test 4", "")
        );
    }

    #[test]
    fn reads_apple_passwords_csv() {
        let imported = import(
            include_bytes!("../testdata/apple_passwords.csv"),
            PasswordManager::ApplePasswords,
        );
        assert_codes(&imported.items, &URIS[..2]);
        assert_eq!(
            filed(&imported),
            [
                row("Deno", "Mason", "", &["deno.land"]),
                row("SPDX", "James", "", &["spdx.org"]),
            ]
        );
    }

    #[test]
    fn reads_1passwords_csv() {
        let imported = import(
            include_bytes!("../testdata/1password.csv"),
            PasswordManager::OnePassword,
        );
        // Deno's key is a bare secret, archived: the same code as Aegis's.
        assert_codes(&imported.items, &[URIS[2], URIS[0]]);
        assert_eq!(
            filed(&imported),
            [
                row("Airbnb", "Elijah", "Travel", &["airbnb.com"]),
                row("Deno", "Mason", "", &["deno.land"]),
            ]
        );
    }

    #[test]
    fn reads_keepassxcs_csv() {
        let imported = import(
            include_bytes!("../testdata/keepassxc.csv"),
            PasswordManager::KeePassXc,
        );
        assert_codes(&imported.items, &[URIS[1], URIS[6]]);
        assert_eq!(
            filed(&imported),
            [
                row("SPDX", "James", "", &["spdx.org"]),
                row(
                    "Boeing",
                    "Sophia",
                    "Games/Steam",
                    &["store.steampowered.com"]
                ),
            ]
        );
    }

    #[test]
    fn reads_proton_passs_csv() {
        let imported = import(
            include_bytes!("../testdata/proton_pass.csv"),
            PasswordManager::ProtonPass,
        );
        assert_codes(&imported.items, &[URIS[2], URIS[0]]);
        assert_eq!(
            filed(&imported),
            [
                row(
                    "Airbnb",
                    "Elijah",
                    "Travel",
                    &["www.airbnb.com", "airbnb.co.uk"]
                ),
                row("Deno", "Mason", "Personal", &["deno.land"]),
            ]
        );
    }

    #[test]
    fn reads_dashlanes_csv_and_its_zip() {
        let csv = include_bytes!("../testdata/dashlane_credentials.csv");
        let imported = import(csv, PasswordManager::Dashlane);
        // Deno's key is a URI with only the secret, as Dashlane writes it.
        assert_codes(&imported.items, &URIS[..2]);
        assert_eq!(
            filed(&imported),
            [
                row("Deno", "Mason", "Work", &["deno.land"]),
                row("SPDX", "James", "", &["spdx.org"]),
            ]
        );
        let zip = zipped(
            &[
                (DASHLANE, csv),
                (
                    "securenotes.csv",
                    b"title,note,category\r\nA note,text,\r\n",
                ),
            ],
            None,
        );
        assert_eq!(import(&zip, PasswordManager::Dashlane), imported);
        let older = b"username,username2,username3,title,password,note,url,category,otpSecret\n\
            Mason,,,Deno,pw,,https://deno.land,,4SJHB4GSD43FZBAI7C2HLRJGPQ\n";
        assert_codes(&import(older, PasswordManager::Dashlane).items, &URIS[..1]);
    }

    #[test]
    fn reads_csv_as_written() {
        // A byte order mark, CRLF, quotes doubled, commas and line breaks
        // in quoted fields, a short row, and the columns in another order.
        let text = "\u{feff}Notes,OTPAuth,Title,URL,Username\r\n\
            \"a \"\"note\"\", with a comma\r\nand a line\",otpauth://totp/?secret=JBSWY3DPEHPK3PXP,\
            \"ACME, Inc.\",acme.example,alice\r\n\
            only a note\r\n\
            ,,,,\r\n\
            ,\"not a key!\",Broken,,bob\r\n\
            ,JBSWY3DPEHPK3PXP\r\n";
        let bytes = text.as_bytes();
        assert_eq!(
            crate::detect(bytes),
            Some(Format::Csv(Some(PasswordManager::ApplePasswords)))
        );
        let imported = crate::import(bytes, None).expect("import");
        assert_eq!(
            filed(&imported),
            [
                row("ACME, Inc.", "alice", "", &["acme.example"]),
                row("", "", "", &[])
            ]
        );
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(skipped, ["Broken (bob)"]);
    }

    #[test]
    fn other_apps_csv_reads_and_csv_without_keys_is_not_one() {
        let other = b"Name,Login,TOTP\nGitHub,alice,JBSWY3DPEHPK3PXP\n";
        assert_eq!(crate::detect(other), Some(Format::Csv(None)));
        assert_eq!(Format::Csv(None).name(), "a CSV export");
        let imported = crate::import(other, None).expect("import");
        assert_eq!(filed(&imported), [row("GitHub", "", "", &[])]);
        for not_ours in [
            &b"name,url,username,password\nGitHub,,alice,pw\n"[..],
            b"totp codes\n",
        ] {
            assert_eq!(crate::detect(not_ours), None);
        }
    }
}
