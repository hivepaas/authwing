use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// How the codes are used on this device (`state/usage.json`), by the id of
/// the code: what voice has learnt, and what was used when.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    #[serde(default)]
    pub codes: BTreeMap<String, Used>,
}

/// How one code is used.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Used {
    /// What Whisper wrote when this code was named and the user then chose
    /// it by hand: heard again, it picks this code.
    #[serde(default)]
    pub taught: Vec<String>,
    /// How often the code was used, so the most used names are the ones
    /// Whisper is told to expect.
    #[serde(default)]
    pub uses: u32,
    /// When it was last used, in seconds since 1970; 0: never.
    #[serde(default)]
    pub last: u64,
}

/// What the last look for a newer authwing found (`state/updates.json`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct UpdateState {
    /// When it looked, in seconds since 1970.
    #[serde(default)]
    pub checked_at: Option<u64>,
    /// The release it was offered, if any (`v0.2.0`).
    #[serde(default)]
    pub offered: Option<String>,
    /// A release the user said to skip.
    #[serde(default)]
    pub skipped: Option<String>,
    /// A release downloaded and checked, waiting to be installed as the app
    /// quits: its version, and its file.
    #[serde(default)]
    pub staged: Option<(String, String)>,
}

/// Where the window was when last seen, in logical pixels. For a maximised or
/// fullscreen window, the bounds it returns to.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    #[serde(default)]
    pub mode: WindowMode,
    /// The group list's width as dragged; None: the default.
    #[serde(default)]
    pub sidebar_width: Option<f32>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowMode {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}
