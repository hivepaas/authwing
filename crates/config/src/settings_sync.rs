//! The settings that sync between devices, as vault fields: `appearance.mode`
//! = `"dark"`, a key's dotted path and its value as JSON. The rest of
//! `config.toml` is this device's own: the sync place, the language, the
//! shortcuts, which differ between computers.

use std::collections::BTreeMap;

use toml_edit::DocumentMut;

use crate::Settings;
use crate::store::{SettingValue, set_in};

/// What syncs, by dotted path.
pub const SYNCED: &[&str] = &[
    "appearance.mode",
    "appearance.ui_font.family",
    "appearance.ui_font.size",
    "codes.clear_clipboard",
];

/// The synced settings `text` (config.toml) sets, by path, as JSON. Those it
/// leaves to their defaults are absent.
pub fn fields_of(text: &str) -> Result<BTreeMap<String, String>, toml::de::Error> {
    let doc: toml::Table = toml::from_str(text)?;
    let mut fields = BTreeMap::new();
    for path in SYNCED {
        let mut at: Option<&toml::Value> = None;
        let mut table = Some(&doc);
        for key in path.split('.') {
            at = table.and_then(|t| t.get(key));
            table = at.and_then(toml::Value::as_table);
        }
        let json = match at {
            Some(toml::Value::Boolean(b)) => b.to_string(),
            Some(toml::Value::Integer(i)) => i.to_string(),
            // 14, not 14.0: the same value reads the same on every device.
            Some(toml::Value::Float(f)) if f.fract() == 0. && f.abs() < 1e15 => {
                (*f as i64).to_string()
            }
            Some(toml::Value::Float(f)) => f.to_string(),
            Some(toml::Value::String(s)) => serde_json::to_string(s).unwrap_or_default(),
            _ => continue,
        };
        fields.insert((*path).to_string(), json);
    }
    Ok(fields)
}

fn value_of(json: &str) -> Option<SettingValue> {
    match serde_json::from_str::<serde_json::Value>(json).ok()? {
        serde_json::Value::Bool(b) => Some(SettingValue::Bool(b)),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Some(SettingValue::Int(i)),
            None => n.as_f64().map(SettingValue::Float),
        },
        serde_json::Value::String(s) => Some(SettingValue::Str(s)),
        _ => None,
    }
}

/// Sets the synced settings in `doc` to `fields` (a path absent there goes
/// back to its default), keeping comments and the rest. A value this version
/// cannot read (a newer one's) is left out rather than breaking the file.
/// Returns whether anything changed.
pub(crate) fn apply(doc: &mut DocumentMut, fields: &BTreeMap<String, String>) -> bool {
    let mut changed = false;
    for path in SYNCED {
        let keys: Vec<&str> = path.split('.').collect();
        let want = fields.get(*path).and_then(|j| value_of(j));
        let before = doc.clone();
        match set_in(doc, &keys, want) {
            Ok(true) if toml::from_str::<Settings>(&doc.to_string()).is_ok() => changed = true,
            Ok(true) => *doc = before,
            _ => {}
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_synced_settings_are_read_and_written_back_keeping_the_rest() {
        let mine = "# mine\n[appearance]\nmode = \"dark\" # at night too\nui_font = { family = \"Inter\", size = 14.0 }\n\n[sync]\nfolder = \"~/Sync\"\n\n[general]\nlanguage = \"vi\"\n";
        let fields = fields_of(mine).expect("read");
        assert_eq!(
            fields,
            BTreeMap::from([
                ("appearance.mode".into(), "\"dark\"".into()),
                ("appearance.ui_font.family".into(), "\"Inter\"".into()),
                ("appearance.ui_font.size".into(), "14".into()),
            ]),
            "this device's own (the folder, the language) are not among them"
        );

        let theirs = BTreeMap::from([
            ("appearance.mode".to_string(), "\"light\"".to_string()),
            ("appearance.ui_font.size".into(), "16".into()),
            ("codes.clear_clipboard".into(), "45".into()),
        ]);
        let mut doc: DocumentMut = mine.parse().expect("doc");
        assert!(apply(&mut doc, &theirs));
        let text = doc.to_string();
        assert!(text.starts_with("# mine\n"), "{text}");
        assert!(text.contains("mode = \"light\" # at night too"), "{text}");
        assert!(
            !text.contains("Inter"),
            "absent there: back to the default\n{text}"
        );
        assert!(text.contains("size = 16"), "{text}");
        assert!(text.contains("folder = \"~/Sync\""), "{text}");
        assert!(text.contains("language = \"vi\""), "{text}");
        let s: Settings = toml::from_str(&text).expect("still valid");
        assert_eq!(s.codes.clear_clipboard, 45);
        assert!(!apply(&mut doc, &theirs), "nothing more to change");

        // A newer version's mode: left out, not written.
        let newer = BTreeMap::from([("appearance.mode".to_string(), "\"sepia\"".to_string())]);
        assert!(!apply(&mut doc, &newer) || !doc.to_string().contains("sepia"));
        assert!(!doc.to_string().contains("sepia"));
    }
}
