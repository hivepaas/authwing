# Configuration files

## Where

The config directory is the platform's, overridable with
`$AUTHWING_CONFIG_DIR`:

| OS | Directory |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/authwing` (`~/.config/authwing`) |
| macOS | `~/Library/Application Support/authwing` |
| Windows | `%APPDATA%\authwing` |

```
config.toml       settings; edited by people and by the app; some sync (configuration.md)
vault.db          SQLite: the codes, encrypted, wrapped keys, sync state (vault.md)
state/*.json      machine-written, never synced: the window's size and place
```

Files are split by **who writes them** and **whether they sync**:

- If people and the app both wrote one big file, every save from the app would
  risk reordering what a person wrote by hand.
- Settings hold per-device values (where this computer syncs, its shortcuts);
  codes must follow you to every device, and are secrets: they live only in
  the vault.

## Formats

TOML for every file a person may read or edit; JSON only under `state/`.

| | JSON | YAML | TOML |
| --- | --- | --- | --- |
| Comments | No (JSONC is non-standard) | Yes | Yes |
| App writes back, keeping comments (Rust) | Hard | No maintained crate | `toml_edit`, as cargo does |
| Hand-editing traps | Commas | Indentation, `no` → false, `1.10` → 1.1 | Few |
| Crate health | `serde_json` excellent | `serde_yaml` archived in 2024 | `toml`, `toml_edit` maintained |

Rules:

- **Reading:** `serde` structs from `toml`.
- **Writing:** the Settings screen edits through `toml_edit`, changing only the
  keys it touched, so comments and order survive.
- **Saving:** write a temp file and rename it, so a crash never leaves a
  half-written file.
- **Live reload:** the files' modification times are checked twice a second,
  and outside edits apply. Polling rather than a file watcher: editors that save
  by replacing the file confuse watchers, and a few `stat` calls cost nothing. A
  file that fails to parse keeps the last good values, and the error is shown
  with the line number.
