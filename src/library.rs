//! The manager's own list of games. A game is a Switch game package the player added (.nsp / .xci: what it is, read
//! from it), the nxrt app that runs it (its runner: fe3hrecomp, eowrecomp ...), or both. Kept in
//! <config>/nxrt-frontend/library.toml (macOS: ~/Library/Application Support/nxrt-frontend), the games' icons beside
//! it in icons/.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// the game's app (an nxrt runner); None: none found for this game yet
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exe: Option<PathBuf>,
    /// its data folder (settings, saves, mods); None: the app's own default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_root: Option<PathBuf>,

    /// the game package the player added, and the keys it is read with
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<PathBuf>,
    /// what the package is (its control data): shown before an app is found
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub title_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub build_id: String,

    /// the game-code id (the app's --status code_id) the Generate step last finished for: Compile is next
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub generated_for: String,

    /// an app built from source without a game-code builder: the command that builds its game code, run in build_dir
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_cmd: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default, rename = "game")]
    pub games: Vec<Entry>,
}

pub fn dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("nxrt-frontend")
}

pub fn icon_path(title_id: &str) -> PathBuf {
    dir().join("icons").join(format!("{title_id}.jpg"))
}

fn path() -> PathBuf {
    std::env::var_os("NXRT_FRONTEND_LIBRARY").map(PathBuf::from).unwrap_or_else(|| dir().join("library.toml"))
}

impl Library {
    pub fn load() -> Library {
        std::fs::read_to_string(path()).ok().and_then(|s| toml::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
    }

    pub fn add_app(&mut self, exe: &Path, data_root: Option<PathBuf>) -> bool {
        if self.games.iter().any(|g| g.exe.as_deref() == Some(exe)) {
            return false;
        }
        self.games.push(Entry { exe: Some(exe.to_path_buf()), data_root, ..Default::default() });
        true
    }
}
