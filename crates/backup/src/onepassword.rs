//! 1Password's export (`.1pux`, https://support.1password.com/1pux-format/):
//! a ZIP whose `export.data` is JSON of the accounts, their vaults and the
//! items in each. A one-time password is a field in one of an item's
//! sections whose value is `{"totp": key}`, the key an `otpauth://` URI or
//! a bare secret ([`login::code`]); an item may have several.
//!
//! The item's title stands in for an issuer the key does not give, and the
//! username among its login fields for an account. The hosts of its web
//! addresses become the code's sites, and its first tag the code's group,
//! as the CSV export's are read. An item in 1Password's trash is skipped;
//! an archived one comes in. The attachments beside (`files/`) are not
//! read.

use serde::Deserialize;
use serde_json::Value;

use crate::archive::Archive;
use crate::{Imported, Item, Result, damaged, label, login};

/// The export's content in the archive.
pub(crate) const DATA: &str = "export.data";
/// The state of an item in the trash; others are `active` and `archived`.
const TRASHED: &str = "trashed";

#[derive(Deserialize)]
struct Export {
    accounts: Option<Vec<Account>>,
}

#[derive(Deserialize)]
struct Account {
    vaults: Option<Vec<Vault>>,
}

#[derive(Deserialize)]
struct Vault {
    items: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct Entry {
    state: Option<String>,
    overview: Option<Overview>,
    details: Option<Details>,
}

#[derive(Default, Deserialize)]
struct Overview {
    title: Option<String>,
    url: Option<String>,
    urls: Option<Vec<Address>>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Address {
    url: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Details {
    login_fields: Option<Vec<LoginField>>,
    sections: Option<Vec<Section>>,
}

#[derive(Deserialize)]
struct LoginField {
    #[serde(default)]
    value: Value,
    designation: Option<String>,
}

#[derive(Deserialize)]
struct Section {
    fields: Option<Vec<Field>>,
}

#[derive(Deserialize)]
struct Field {
    #[serde(default)]
    value: Value,
}

pub(crate) fn import(bytes: &[u8]) -> Result<Imported> {
    let data = Archive::open(bytes)?.read(DATA, None)?;
    let export: Export = serde_json::from_slice(&data).map_err(damaged)?;
    let items = export
        .accounts
        .into_iter()
        .flatten()
        .flat_map(|account| account.vaults.into_iter().flatten())
        .flat_map(|vault| vault.items.into_iter().flatten());

    let mut imported = Imported::default();
    for (n, value) in items.enumerate() {
        let item_no = || format!("item {}", n + 1);
        let entry: Entry = match serde_json::from_value(value) {
            Ok(entry) => entry,
            Err(e) => {
                imported.skipped.push((item_no(), e.to_string()));
                continue;
            }
        };
        let details = entry.details.unwrap_or_default();
        let keys: Vec<&str> = details
            .sections
            .iter()
            .flatten()
            .flat_map(|section| section.fields.iter().flatten())
            .filter_map(|field| field.value.get("totp").and_then(Value::as_str))
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .collect();
        if keys.is_empty() {
            continue;
        }
        let overview = entry.overview.unwrap_or_default();
        let title = overview.title.as_deref().unwrap_or_default();
        let username = details
            .login_fields
            .iter()
            .flatten()
            .find(|field| field.designation.as_deref() == Some("username"))
            .and_then(|field| field.value.as_str())
            .unwrap_or_default();
        let what = || label(title, username, item_no);
        if entry.state.as_deref() == Some(TRASHED) {
            for _ in &keys {
                imported
                    .skipped
                    .push((what(), "it is in 1Password's trash".into()));
            }
            continue;
        }
        let addresses = overview.url.iter().map(String::as_str).chain(
            overview
                .urls
                .iter()
                .flatten()
                .filter_map(|address| address.url.as_deref()),
        );
        let sites = login::sites(addresses);
        let group = overview
            .tags
            .iter()
            .flatten()
            .map(|tag| tag.trim())
            .find(|tag| !tag.is_empty())
            .unwrap_or_default();
        for key in keys {
            match login::code(key, title, username) {
                Ok(otp) => imported.items.push(Item {
                    otp,
                    group: group.to_string(),
                    sites: sites.clone(),
                }),
                Err(e) => imported.skipped.push((what(), e.to_string())),
            }
        }
    }
    Ok(imported)
}

#[cfg(test)]
mod tests {
    //! `1password.1pux` is written from the format's documentation, its
    //! codes some of those of Aegis's samples (see `crate::tests`): a ZIP
    //! made with Python's `zipfile`, deflated as 1Password's are, of
    //! `export.attributes` and `export.data`.

    use super::*;
    use crate::Format;
    use crate::testing::{URIS, assert_codes, zipped};

    const SAMPLE: &[u8] = include_bytes!("../testdata/1password.1pux");

    #[test]
    fn reads_1passwords_export() {
        assert_eq!(crate::detect(SAMPLE), Some(Format::OnePassword));
        let imported = crate::import(SAMPLE, None).expect("import");
        // Deno's key is a bare secret, and SPDX is archived.
        assert_codes(&imported.items, &URIS[..3]);
        let filed: Vec<(&str, &str, Vec<&str>)> = imported
            .items
            .iter()
            .map(|item| {
                let sites = item.sites.iter().map(String::as_str).collect();
                (item.otp.issuer.as_str(), item.group.as_str(), sites)
            })
            .collect();
        assert_eq!(
            filed,
            [
                ("Deno", "Work", vec!["deno.land", "dash.deno.com"]),
                ("SPDX", "", vec!["spdx.org"]),
                ("Airbnb", "", vec!["www.airbnb.com", "airbnb.co.uk"]),
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
                ("Old (bob)", "it is in 1Password's trash"),
                (
                    "Broken (erin)",
                    "'!' is not a base32 character (A to Z and 2 to 7)"
                ),
            ]
        );
    }

    #[test]
    fn a_broken_item_costs_only_itself() {
        let data = br#"{"accounts": [{"vaults": [{"items": [
            {"state": "active", "overview": {"title": "GitHub"}, "details": {"sections": [
                {"fields": [{"value": {"totp": "otpauth://totp/?secret=JBSWY3DPEHPK3PXP"}}]}]}},
            {"overview": "not an overview"},
            {"overview": null, "details": null}
        ]}, {"items": null}]}]}"#;
        let bytes = zipped(&[(DATA, data)], None);
        let imported = crate::import(&bytes, None).expect("import");
        assert_eq!(imported.items.len(), 1);
        assert_eq!(imported.items[0].otp.issuer, "GitHub");
        assert_eq!(imported.skipped.len(), 1);
        assert_eq!(imported.skipped[0].0, "item 2");
        let bytes = zipped(&[(DATA, b"{\"accounts\": 7}")], None);
        assert!(matches!(
            crate::import(&bytes, None),
            Err(crate::Error::Damaged(_))
        ));
    }
}
