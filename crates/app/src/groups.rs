//! Groups, as the sidebar files codes under them. Each is a record of kind
//! `group` in the vault, its name and color, synced as the codes are; a
//! code is filed under a group by the group's name (its `group` field).
//! A name codes are filed under with no record of its own is a group too,
//! of the color its name gives, so codes from an import or another device
//! are never left out. Renaming a group takes its codes along; deleting one
//! keeps its codes, under no group.

use std::collections::BTreeMap;

use authwing_vault::Vault;

use crate::codes::{self, Entry};

/// The vault's record kind for a group.
pub const KIND: &str = "group";
const NAME: &str = "name";
const COLOR: &str = "color";

/// The colors a group can have, by the name saved: the app's blue first.
pub const COLORS: &[(&str, u32)] = &[
    ("blue", 0x4cc4f9),
    ("amber", 0xf59e0b),
    ("violet", 0xa78bfa),
    ("green", 0x4ade80),
    ("red", 0xf87171),
    ("pink", 0xf472b6),
    ("teal", 0x2dd4bf),
    ("slate", 0x94a3b8),
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    /// Its records: none for a name codes use with no record, several when
    /// two devices made one of a name at once.
    pub ids: Vec<String>,
    pub name: String,
    /// A name from [`COLORS`].
    pub color: String,
    /// The codes filed under it.
    pub count: usize,
}

impl Group {
    pub fn rgb(&self) -> u32 {
        rgb(&self.color)
    }
}

/// The color `name` saves, or the first.
pub fn rgb(name: &str) -> u32 {
    COLORS
        .iter()
        .find(|(n, _)| *n == name)
        .map_or(COLORS[0].1, |(_, c)| *c)
}

/// The color a group of `name` has before one is chosen: one of
/// [`COLORS`], the same on every device.
pub fn color_for(name: &str) -> &'static str {
    let sum = name.to_lowercase().bytes().fold(0usize, |s, b| {
        s.wrapping_mul(31).wrapping_add(usize::from(b))
    });
    COLORS[sum % COLORS.len()].0
}

/// The groups there are, by name, each with how many codes it has.
pub fn load(vault: &Vault, codes: &[Entry]) -> Vec<Group> {
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    let key = |name: &str| name.trim().to_lowercase();
    for (id, fields) in vault.entries(KIND).unwrap_or_default() {
        let Some(name) = fields.get(NAME).map(|n| n.trim()).filter(|n| !n.is_empty()) else {
            continue;
        };
        let group = groups.entry(key(name)).or_insert_with(|| Group {
            ids: Vec::new(),
            name: name.to_string(),
            color: fields
                .get(COLOR)
                .filter(|c| COLORS.iter().any(|(n, _)| n == c))
                .cloned()
                .unwrap_or_else(|| color_for(name).to_string()),
            count: 0,
        });
        group.ids.push(id);
    }
    // How codes spell each name, for a group with no record: the most
    // used spelling, then the one that sorts first ("Work" before "work").
    let mut spellings: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for code in codes.iter().filter(|c| !c.group.trim().is_empty()) {
        let name = code.group.trim();
        *spellings
            .entry(key(name))
            .or_default()
            .entry(name.to_string())
            .or_default() += 1;
    }
    for (k, spelled) in spellings {
        let count = spelled.values().sum();
        let group = groups.entry(k).or_insert_with(|| {
            let name = spelled
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0)))
                .map(|(n, _)| n.clone())
                .unwrap_or_default();
            Group {
                ids: Vec::new(),
                color: color_for(&name).to_string(),
                name,
                count: 0,
            }
        });
        group.count = count;
    }
    groups.into_values().collect()
}

/// Whether `code` is filed under the group `name`.
pub fn holds(name: &str, code: &Entry) -> bool {
    code.group.trim().eq_ignore_ascii_case(name.trim())
}

/// Saves a group: a new one (`old` None), or `old` renamed or recolored,
/// its codes taken along to its new name.
pub fn save(
    vault: &Vault,
    codes: &[Entry],
    old: Option<&Group>,
    name: &str,
    color: &str,
) -> Result<(), String> {
    let name = name.trim();
    let mut fields = BTreeMap::new();
    fields.insert(NAME.to_string(), name.to_string());
    fields.insert(COLOR.to_string(), color.to_string());
    let id = match old.and_then(|g| g.ids.first()) {
        Some(id) => id.clone(),
        None => authwing_vault::new_id().map_err(|e| e.to_string())?,
    };
    vault
        .update(&id, KIND, name, &fields)
        .map_err(|e| e.to_string())?;
    let Some(old) = old else {
        return Ok(());
    };
    // Other records of the old name: one group now.
    for extra in old.ids.iter().skip(1) {
        vault.delete(extra).map_err(|e| e.to_string())?;
    }
    if old.name != name {
        refile(vault, codes, &old.name, name)?;
    }
    Ok(())
}

/// Deletes the group: its records go, its codes stay under no group.
pub fn delete(vault: &Vault, codes: &[Entry], group: &Group) -> Result<(), String> {
    for id in &group.ids {
        vault.delete(id).map_err(|e| e.to_string())?;
    }
    refile(vault, codes, &group.name, "")
}

/// Files the codes under `from` under `to` (empty: none).
fn refile(vault: &Vault, codes: &[Entry], from: &str, to: &str) -> Result<(), String> {
    for code in codes.iter().filter(|c| holds(from, c)) {
        let filing = codes::Filing {
            group: to.to_string(),
            ..code.filing()
        };
        codes::save(vault, Some(&code.id), &code.otp, &filing)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use authwing_totp::{Kind, Otp, Secret};
    use authwing_vault::MemoryKeys;

    fn add(vault: &Vault, issuer: &str, group: &str) {
        let secret = Secret::from_base32("JBSWY3DPEHPK3PXP").expect("secret");
        let otp = Otp::new(issuer, "me", secret, Kind::Totp { period: 30 });
        let filing = codes::Filing {
            group: group.into(),
            ..Default::default()
        };
        codes::save(vault, None, &otp, &filing).expect("save");
    }

    fn names(groups: &[Group]) -> Vec<(String, usize)> {
        groups.iter().map(|g| (g.name.clone(), g.count)).collect()
    }

    #[test]
    fn groups_come_from_records_and_from_codes() {
        let dir = std::env::temp_dir().join(format!("authwing-groups-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let keys = MemoryKeys::default();
        let vault = Vault::open(&dir.join("vault.db"), &keys).expect("vault");
        add(&vault, "GitHub", "Work");
        add(&vault, "AWS", "work");
        add(&vault, "Vietcombank", "Money");
        add(&vault, "Steam", "");
        save(&vault, &[], None, "Personal", "violet").expect("new group");
        let (codes, _) = codes::load(&vault);
        let groups = load(&vault, &codes);
        assert_eq!(
            names(&groups),
            [
                ("Money".to_string(), 1),
                ("Personal".to_string(), 0),
                ("Work".to_string(), 2)
            ]
        );
        assert_eq!(groups[1].color, "violet");
        assert_eq!(groups[0].color, color_for("Money"));

        // Renamed, the codes go along; deleted, they stay, under none.
        let work = groups[2].clone();
        save(&vault, &codes, Some(&work), "Job", "amber").expect("rename");
        let (codes, _) = codes::load(&vault);
        let groups = load(&vault, &codes);
        assert_eq!(
            names(&groups),
            [
                ("Job".to_string(), 2),
                ("Money".to_string(), 1),
                ("Personal".to_string(), 0)
            ]
        );
        assert_eq!(groups[0].color, "amber");
        delete(&vault, &codes, &groups[0]).expect("delete");
        let (codes, _) = codes::load(&vault);
        assert_eq!(
            names(&load(&vault, &codes)),
            [("Money".to_string(), 1), ("Personal".to_string(), 0)]
        );
        assert_eq!(codes.len(), 4);
        assert_eq!(codes.iter().filter(|c| c.group.is_empty()).count(), 3);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_name_has_the_same_color_everywhere() {
        assert_eq!(color_for("Work"), color_for("work"));
        assert!(COLORS.iter().any(|(n, _)| *n == color_for("Ops")));
        assert_eq!(rgb("nope"), COLORS[0].1);
    }
}
