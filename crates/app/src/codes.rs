//! The codes in the vault, as the window shows them: each a record of kind
//! `code`, its fields those of [`authwing_totp::fields`] plus how it is filed
//! ([`Filing`]): the group it is under, the sites it is shown first on, and
//! whether it is a favorite.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use authwing_context::Site;
use authwing_i18n::tr;
use authwing_totp::Otp;
use authwing_vault::Vault;

/// The vault's record kind for a code.
pub const KIND: &str = "code";
/// The group a code is filed under; empty: none.
const GROUP: &str = "group";
/// The sites a code is shown first on, one per line (`github.com`).
const SITES: &str = "sites";
/// `1` for a favorite; absent otherwise.
const FAVORITE: &str = "favorite";

/// How a code is filed, beside the code itself.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filing {
    /// The group's name; empty: none.
    pub group: String,
    /// The sites it is shown first on.
    pub sites: Vec<String>,
    pub favorite: bool,
}

pub struct Entry {
    pub id: String,
    pub otp: Otp,
    pub group: String,
    /// The sites as saved.
    pub sites: Vec<String>,
    /// Those of them that read as sites, to match pages against.
    pub parsed: Vec<Site>,
    pub favorite: bool,
}

impl Entry {
    /// How it is filed, to save it again as it is.
    pub fn filing(&self) -> Filing {
        Filing {
            group: self.group.clone(),
            sites: self.sites.clone(),
            favorite: self.favorite,
        }
    }

    /// What the code is called: its service, else its account.
    pub fn title(&self) -> String {
        if self.otp.issuer.is_empty() {
            self.otp.account.clone()
        } else {
            self.otp.issuer.clone()
        }
    }

    /// The account under the title, when the title is not it already.
    pub fn detail(&self) -> &str {
        if self.otp.issuer.is_empty() {
            ""
        } else {
            &self.otp.account
        }
    }

    /// Whether every word of `words` (lowercase) is in its service, account,
    /// group or sites.
    pub fn matches(&self, words: &[String]) -> bool {
        let text = format!(
            "{} {} {} {}",
            self.otp.issuer,
            self.otp.account,
            self.group,
            self.sites.join(" ")
        )
        .to_lowercase();
        words.iter().all(|w| text.contains(w.as_str()))
    }

    /// Up to two letters for its tile: "GH" for GitHub, "AW" for AWS.
    pub fn initials(&self) -> String {
        let title = self.title();
        let words: Vec<&str> = title.split_whitespace().collect();
        let first = |w: &str| w.chars().find(|c| c.is_alphanumeric());
        let letters: String = match words.as_slice() {
            [] => String::new(),
            [one] => {
                // Capitals inside one word (GitHub, PayPal) say where it splits.
                let caps: String = one.chars().filter(|c| c.is_uppercase()).take(2).collect();
                if caps.chars().count() == 2 {
                    caps
                } else {
                    one.chars()
                        .filter(|c| c.is_alphanumeric())
                        .take(2)
                        .collect()
                }
            }
            [a, b, ..] => first(a).into_iter().chain(first(b)).collect(),
        };
        letters.to_uppercase()
    }
}

/// Seconds since 1970, as the codes are made from.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// The codes in the vault, by title; and the records that could not be read
/// as codes (written by a newer version, or damaged), said as problems.
pub fn load(vault: &Vault) -> (Vec<Entry>, Vec<String>) {
    let records = match vault.entries(KIND) {
        Ok(r) => r,
        Err(e) => {
            return (
                vec![],
                vec![tr!("Cannot read the codes: {e}", e = e.to_string())],
            );
        }
    };
    let mut problems = vec![];
    let mut codes: Vec<Entry> = records
        .into_iter()
        .filter_map(|(id, fields)| match Otp::from_fields(&fields) {
            Ok(otp) => {
                let sites: Vec<String> = fields
                    .get(SITES)
                    .map(|s| s.lines().map(str::to_string).collect())
                    .unwrap_or_default();
                let parsed = sites.iter().filter_map(|s| Site::parse(s).ok()).collect();
                Some(Entry {
                    id,
                    otp,
                    group: fields.get(GROUP).cloned().unwrap_or_default(),
                    sites,
                    parsed,
                    favorite: fields.get(FAVORITE).is_some_and(|f| f == "1"),
                })
            }
            Err(e) => {
                problems.push(tr!("A code cannot be read: {e}", e = e.to_string()));
                None
            }
        })
        .collect();
    codes.sort_by_key(|c| (c.title().to_lowercase(), c.otp.account.to_lowercase()));
    (codes, problems)
}

/// Saves `otp` as a new code, or over the code `id`, filed as `filing`; its
/// id.
pub fn save(vault: &Vault, id: Option<&str>, otp: &Otp, filing: &Filing) -> Result<String, String> {
    let (group, sites) = (&filing.group, &filing.sites);
    otp.check().map_err(|e| e.to_string())?;
    let id = match id {
        Some(id) => id.to_string(),
        None => authwing_vault::new_id().map_err(|e| e.to_string())?,
    };
    let mut fields: BTreeMap<String, String> = otp.to_fields();
    if !group.trim().is_empty() {
        fields.insert(GROUP.into(), group.trim().to_string());
    }
    if !sites.is_empty() {
        fields.insert(SITES.into(), sites.join("\n"));
    }
    if filing.favorite {
        fields.insert(FAVORITE.into(), "1".into());
    }
    let name = if otp.issuer.is_empty() {
        &otp.account
    } else {
        &otp.issuer
    };
    vault
        .update(&id, KIND, name, &fields)
        .map_err(|e| e.to_string())?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use authwing_totp::{Kind, Secret};
    use authwing_vault::MemoryKeys;

    fn otp(issuer: &str, account: &str) -> Otp {
        let secret = Secret::from_base32("JBSWY3DPEHPK3PXP").expect("secret");
        Otp::new(issuer, account, secret, Kind::Totp { period: 30 })
    }

    #[test]
    fn codes_are_saved_and_read_back_in_order() {
        let dir = std::env::temp_dir().join(format!("authwing-codes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let keys = MemoryKeys::default();
        let vault = Vault::open(&dir.join("vault.db"), &keys).expect("vault");
        let work = |sites: &[&str], favorite| Filing {
            group: "Work".into(),
            sites: sites.iter().map(|s| s.to_string()).collect(),
            favorite,
        };
        let gh = save(
            &vault,
            None,
            &otp("GitHub", "me@work"),
            &work(&["github.com"], true),
        )
        .expect("save");
        save(&vault, None, &otp("aws", "prod"), &work(&[], false)).expect("save");
        save(&vault, None, &otp("", "me@home"), &Filing::default()).expect("save");
        let (codes, problems) = load(&vault);
        assert!(problems.is_empty(), "{problems:?}");
        let titles: Vec<String> = codes.iter().map(Entry::title).collect();
        assert_eq!(titles, ["aws", "GitHub", "me@home"]);
        let github = codes.iter().find(|c| c.id == gh).expect("github");
        assert_eq!(github.filing(), work(&["github.com"], true));
        // Saved again unstarred, it is no longer a favorite.
        save(
            &vault,
            Some(&gh),
            &github.otp,
            &work(&["github.com"], false),
        )
        .expect("save");
        let (codes, _) = load(&vault);
        assert!(!codes.iter().any(|c| c.favorite));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_code_is_found_by_any_of_its_words() {
        let entry = Entry {
            id: "1".into(),
            otp: otp("Amazon Web Services", "prod-admin"),
            group: "Work".into(),
            sites: vec!["console.aws.amazon.com".into()],
            parsed: vec![],
            favorite: false,
        };
        let words = |s: &str| -> Vec<String> { s.split_whitespace().map(str::to_string).collect() };
        assert!(entry.matches(&words("amazon prod")));
        assert!(entry.matches(&words("console.aws")));
        assert!(entry.matches(&words("work")));
        assert!(!entry.matches(&words("github")));
        assert_eq!(entry.initials(), "AW");
        assert_eq!(otp_entry("GitHub").initials(), "GH");
        assert_eq!(otp_entry("Slack").initials(), "SL");
    }

    fn otp_entry(issuer: &str) -> Entry {
        Entry {
            id: "1".into(),
            otp: otp(issuer, ""),
            group: String::new(),
            sites: vec![],
            parsed: vec![],
            favorite: false,
        }
    }
}
