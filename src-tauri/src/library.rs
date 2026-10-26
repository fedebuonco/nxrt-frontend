//! The manager's own list of games. A game is a Switch game package the player added (.nsp / .xci: what it is, read
//! from it), the nxrt app that runs it (its runner: fe3hrecomp, eowrecomp ...), or both. Kept in
//! <config>/nxrt-frontend/library.toml (macOS: ~/Library/Application Support/nxrt-frontend), the games' icons beside
//! it in icons/.
use crate::controls::Controls;
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
    /// the game's update (an .nsp), played with the base package: its code, and its changes to the game's data
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub update: Option<PathBuf>,
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

    /// a recomp nxrt-frontend made for this game (recomp.rs): its folder
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recomp_dir: Option<PathBuf>,

    /// an app built from source without a game-code builder: the command that builds its game code, run in build_dir
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_dir: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_cmd: Option<String>,

    /// controller settings for this game, layered over the library's defaults (controls.rs)
    #[serde(default, skip_serializing_if = "Controls::is_empty")]
    pub controls: Controls,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Library {
    /// the player's keys file (prod.keys) the toolbar chose: used before one beside a package or in ~/.switch
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<PathBuf>,
    /// controller defaults passed to every game (a game's own controls override them)
    #[serde(default, skip_serializing_if = "Controls::is_empty")]
    pub controls: Controls,
    #[serde(default, rename = "game")]
    pub games: Vec<Entry>,
    /// (not saved) the library file could not be read: where it was moved, so nothing overwrites it
    #[serde(skip)]
    pub load_error: Option<String>,
}

/// A keys file that looks like one (prod.keys: has the header key). Err: why not.
pub fn check_keys(p: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
    if text
        .lines()
        .any(|l| l.trim_start().starts_with("header_key"))
    {
        Ok(())
    } else {
        Err(format!(
            "{} has no header_key: not a prod.keys file",
            p.display()
        ))
    }
}

pub fn dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("nxrt-frontend")
}

pub fn icon_path(title_id: &str) -> PathBuf {
    dir().join("icons").join(format!("{title_id}.jpg"))
}

fn path() -> PathBuf {
    std::env::var_os("NXRT_FRONTEND_LIBRARY")
        .map(PathBuf::from)
        .unwrap_or_else(|| dir().join("library.toml"))
}

impl Library {
    /// The library. A file that is there but can't be read is moved aside (library.toml.broken-<time>) before an
    /// empty library starts, so the next save can't overwrite it; load_error says where it went.
    pub fn load() -> Library {
        let p = path();
        let text = match std::fs::read_to_string(&p) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Library::default(),
            Err(e) => return Library::broken(&p, &e.to_string()),
        };
        match toml::from_str(&text) {
            Ok(lib) => lib,
            Err(e) => Library::broken(&p, &e.to_string()),
        }
    }

    fn broken(p: &Path, why: &str) -> Library {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let aside = p.with_file_name(format!(
            "{}.broken-{stamp}",
            p.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "library.toml".into())
        ));
        let moved = std::fs::rename(p, &aside).is_ok();
        tracing::error!(target: "library", path = %p.display(), error = %why, moved_to = %aside.display(), moved,
            "the library could not be read");
        Library {
            load_error: Some(if moved {
                format!(
                    "The library could not be read ({why}); it was kept as {}",
                    aside.display()
                )
            } else {
                format!("The library could not be read ({why})")
            }),
            ..Default::default()
        }
    }

    /// Written to a temporary file and renamed over the library, so a save cut short leaves the old one whole.
    pub fn save(&self) -> Result<(), String> {
        let p = path();
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = p.with_extension("toml.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {e}", p.display()))
    }

    pub fn add_app(&mut self, exe: &Path, data_root: Option<PathBuf>) -> bool {
        if self.games.iter().any(|g| g.exe.as_deref() == Some(exe)) {
            return false;
        }
        self.games.push(Entry {
            exe: Some(exe.to_path_buf()),
            data_root,
            ..Default::default()
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controls_round_trip_in_the_library_file() {
        let controls = Controls {
            prompts: Some("nintendo".into()),
            bind_pad: [("a".to_string(), "south".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        };
        let game_controls = Controls {
            dpad_move: Some(false),
            ..Default::default()
        };
        let lib = Library {
            keys: None,
            controls: controls.clone(),
            games: vec![Entry {
                exe: Some(PathBuf::from("/tmp/nxrt-test-app")),
                controls: game_controls.clone(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let text = toml::to_string_pretty(&lib).unwrap();
        assert!(text.contains("[controls]"), "{text}");
        assert!(text.contains("[game.controls]"), "{text}");
        let back: Library = toml::from_str(&text).unwrap();
        assert_eq!(back.controls, controls);
        assert_eq!(back.games[0].controls, game_controls);
        assert_eq!(
            back.games[0].exe.as_deref(),
            Some(Path::new("/tmp/nxrt-test-app"))
        );
    }

    #[test]
    fn an_old_library_has_no_controls() {
        let lib: Library = toml::from_str("[[game]]\nexe = \"/tmp/x\"\n").unwrap();
        assert!(lib.controls.is_empty());
        assert!(lib.games[0].controls.is_empty());
    }
}
