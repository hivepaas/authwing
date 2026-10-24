//! Proton Pass's export: a ZIP whose `Proton Pass/data.json` holds the
//! vaults by their id, each with its name and its items. A login keeps its
//! key in `content.totpUri`, and any item may keep more in extra fields of
//! the type `totp` (`extraFields[].data.totpUri`): `otpauth://` URIs, or a
//! secret as it was typed ([`login::code`]).
//!
//! As Proton Pass writes and reads it (https://github.com/ProtonMail/WebClients,
//! `packages/pass/lib/export/` and `lib/import/providers/protonpass/`): the
//! item's name stands in for an issuer the key does not give, and its
//! username, or else its email, for an account (exports before version
//! 1.18 have the email as `username`). The hosts of its addresses become
//! the code's sites: `autofillUrls` but those it is never to fill and the
//! patterns, or `urls` in exports from before them. Its vault's name is the
//! code's group. An item in the trash is skipped.
//!
//! An export encrypted with a passphrase holds `Proton Pass/data.pgp`
//! instead, sealed with OpenPGP, which authwing does not read: it is
//! refused with a word on exporting again.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::archive::Archive;
use crate::{Error, Imported, Item, Result, damaged, label, login};

/// The export's content in the archive, plain and encrypted.
pub(crate) const DATA: &str = "Proton Pass/data.json";
pub(crate) const SEALED: &str = "Proton Pass/data.pgp";
/// An item's state in the trash (in use, it is 1).
const TRASHED: u8 = 2;
/// The autofill modes whose address is no site: never to fill there, and
/// a pattern.
const NEVER: u8 = 2;
const PATTERN: u8 = 4;

#[derive(Deserialize)]
struct Export {
    vaults: BTreeMap<String, Vault>,
}

#[derive(Deserialize)]
struct Vault {
    name: Option<String>,
    items: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct Entry {
    data: Data,
    state: Option<u8>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    #[serde(rename = "type")]
    kind: String,
    metadata: Option<Metadata>,
    #[serde(default)]
    content: Value,
    extra_fields: Option<Vec<ExtraField>>,
}

#[derive(Default, Deserialize)]
struct Metadata {
    name: Option<String>,
}

/// What a login's content holds that is read.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Login {
    item_username: Option<String>,
    item_email: Option<String>,
    /// The email, in exports before version 1.18.
    username: Option<String>,
    urls: Option<Vec<String>>,
    autofill_urls: Option<Vec<AutofillUrl>>,
    totp_uri: Option<String>,
}

#[derive(Deserialize)]
struct AutofillUrl {
    url: Option<String>,
    mode: Option<u8>,
}

#[derive(Deserialize)]
struct ExtraField {
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    data: Value,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let mut archive = Archive::open(bytes)?;
    if !archive.has(DATA) && archive.has(SEALED) {
        return Err(Error::Unsupported(
            "this Proton Pass export is encrypted, which authwing cannot read: \
             export from Proton Pass again without encryption, or as CSV"
                .into(),
        ));
    }
    let data = archive.read(DATA, None)?;
    let export: Export = serde_json::from_slice(&data).map_err(damaged)?;

    let mut imported = Imported::default();
    let mut n = 0;
    for vault in export.vaults.into_values() {
        let group = vault.name.unwrap_or_default().trim().to_string();
        for value in vault.items.unwrap_or_default() {
            n += 1;
            let item_no = || format!("item {n}");
            let entry: Entry = match serde_json::from_value(value) {
                Ok(entry) => entry,
                Err(e) => {
                    imported.skipped.push((item_no(), e.to_string()));
                    continue;
                }
            };
            let data = entry.data;
            let content: Login = if data.kind == "login" {
                match serde_json::from_value(data.content) {
                    Ok(content) => content,
                    Err(e) => {
                        imported.skipped.push((item_no(), e.to_string()));
                        continue;
                    }
                }
            } else {
                Login::default()
            };
            let extra = data.extra_fields.iter().flatten().filter_map(|field| {
                let totp = field.kind.as_deref() == Some("totp");
                field
                    .data
                    .get("totpUri")
                    .and_then(Value::as_str)
                    .filter(|_| totp)
            });
            let keys: Vec<&str> = content
                .totp_uri
                .as_deref()
                .into_iter()
                .chain(extra)
                .map(str::trim)
                .filter(|key| !key.is_empty())
                .collect();
            if keys.is_empty() {
                continue;
            }
            let name = data.metadata.unwrap_or_default().name.unwrap_or_default();
            let account = [
                &content.item_username,
                &content.item_email,
                &content.username,
            ]
            .into_iter()
            .flatten()
            .map(|account| account.trim())
            .find(|account| !account.is_empty())
            .unwrap_or_default();
            let what = || label(&name, account, item_no);
            if entry.state == Some(TRASHED) {
                for _ in &keys {
                    imported
                        .skipped
                        .push((what(), "it is in Proton Pass's trash".into()));
                }
                continue;
            }
            let sites = match content.autofill_urls.as_deref() {
                Some(autofill) if !autofill.is_empty() => login::sites(
                    autofill
                        .iter()
                        .filter(|address| !matches!(address.mode, Some(NEVER | PATTERN)))
                        .filter_map(|address| address.url.as_deref()),
                ),
                _ => login::sites(content.urls.iter().flatten().map(String::as_str)),
            };
            for key in keys {
                match login::code(key, &name, account) {
                    Ok(otp) => imported.items.push(Item {
                        otp,
                        group: group.clone(),
                        sites: sites.clone(),
                    }),
                    Err(e) => imported.skipped.push((what(), e.to_string())),
                }
            }
        }
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    //! `proton_pass.zip` is written from Proton Pass's export code (see the
    //! module docs), its codes some of those of Aegis's samples (see
    //! `crate::tests`): a ZIP made with Python's `zipfile`, stored without
    //! compression as Proton Pass's are, of `Proton Pass/data.json`.

    use super::*;
    use crate::Format;
    use crate::testing::{URIS, assert_codes, zipped};

    #[test]
    fn reads_proton_passs_export() {
        let bytes = include_bytes!("../testdata/proton_pass.zip");
        assert_eq!(crate::detect(bytes), Some(Format::ProtonPass));
        let imported = crate::import(bytes, None).expect("import");
        // Airbnb's key is in an extra field, Deno's a bare secret.
        assert_codes(&imported.items, &[URIS[1], URIS[2], URIS[0]]);
        let filed: Vec<(&str, &str, &str, Vec<&str>)> = imported
            .items
            .iter()
            .map(|item| {
                let otp = &item.otp;
                let sites = item.sites.iter().map(String::as_str).collect();
                (
                    otp.issuer.as_str(),
                    otp.account.as_str(),
                    item.group.as_str(),
                    sites,
                )
            })
            .collect();
        assert_eq!(
            filed,
            [
                (
                    "SPDX",
                    "James",
                    "Personal",
                    vec!["spdx.org", "tools.spdx.org"]
                ),
                ("Airbnb", "Elijah", "Personal", vec!["www.airbnb.com"]),
                ("Deno", "Mason", "Work", vec!["deno.land"]),
            ]
        );
        let skipped: Vec<(&str, &str)> = imported
            .skipped
            .iter()
            .map(|(what, why)| (what.as_str(), why.as_str()))
            .collect();
        assert_eq!(
            skipped,
            [
                ("Old (bob)", "it is in Proton Pass's trash"),
                (
                    "Broken (erin)",
                    "'!' is not a base32 character (A to Z and 2 to 7)"
                ),
            ]
        );
    }

    #[test]
    fn older_exports_read_and_encrypted_ones_are_refused() {
        // Before version 1.18, the email was `username`.
        let data = br#"{"version": "1.17.0", "vaults": {"v": {"name": " Home ", "items": [
            {"state": 1, "data": {"type": "login", "metadata": {"name": "GitHub"},
                "content": {"username": "alice@example.com", "urls": ["https://github.com"],
                    "totpUri": "JBSWY3DPEHPK3PXP"}}},
            {"state": 1, "data": {"type": "login", "content": {"urls": 7, "totpUri": "x"}}},
            {"data": "not an item"}
        ]}}}"#;
        let bytes = zipped(&[(DATA, data)], None);
        let imported = crate::import(&bytes, None).expect("import");
        let item = &imported.items[0];
        assert_eq!(
            (
                item.otp.issuer.as_str(),
                item.otp.account.as_str(),
                item.group.as_str()
            ),
            ("GitHub", "alice@example.com", "Home")
        );
        assert_eq!(item.sites, ["github.com"]);
        let skipped: Vec<&str> = imported
            .skipped
            .iter()
            .map(|(what, _)| what.as_str())
            .collect();
        assert_eq!(skipped, ["item 2", "item 3"]);

        let sealed = zipped(&[(SEALED, b"-----BEGIN PGP MESSAGE-----\n")], None);
        assert_eq!(crate::detect(&sealed), Some(Format::ProtonPass));
        let Err(Error::Unsupported(why)) = crate::import(&sealed, None) else {
            panic!("an encrypted export read");
        };
        assert!(why.contains("without encryption, or as CSV"), "{why}");
    }
}
