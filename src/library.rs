//! The manager's own list of games: which runner (a game's app) and which data folder each one uses. Kept in
//! <config>/nxrt-frontend/library.toml (macOS: ~/Library/Application Support/nxrt-frontend).
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// the game's app: fe3hrecomp, eowrecomp ... (an nxrt runner)
    pub exe: PathBuf,
    /// its data folder (settings, saves, mods); None: the runner's own default
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Library {
    #[serde(default, rename = "game")]
    pub games: Vec<Entry>,
}

pub fn dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("nxrt-frontend")
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

    pub fn add(&mut self, exe: &Path, data_root: Option<PathBuf>) -> bool {
        if self.games.iter().any(|g| g.exe == exe) {
            return false;
        }
        self.games.push(Entry { exe: exe.to_path_buf(), data_root });
        true
    }
}
