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
    /// which backend runs this game: nxrt (its recomp app) or nxnative (its Mac build); the window switches it
    #[serde(default, skip_serializing_if = "Backend::is_nxrt")]
    pub backend: Backend,
    /// nxnative: the command that runs the game, as its Mac build printed it (nxnative.rs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_cmd: Option<String>,
}

/// What runs a game: an nxrt recomp (its own app, built from the game's code translated to C), or nxnative (the
/// game's own code run natively in nxnative's host, github.com/seedds/nxnative).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    #[default]
    Nxrt,
    Nxnative,
}

impl Backend {
    pub fn is_nxrt(&self) -> bool {
        *self == Backend::Nxrt
    }
    pub fn name(&self) -> &'static str {
        match self {
            Backend::Nxrt => "nxrt",
            Backend::Nxnative => "nxnative",
        }
    }
    pub fn parse(s: &str) -> Option<Backend> {
        match s {
            "nxrt" => Some(Backend::Nxrt),
            "nxnative" => Some(Backend::Nxnative),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Library {
    /// one root for the manager's games, per-game data and recomp builds. None: the default (~/nxrt).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<PathBuf>,
    /// the player's keys file (prod.keys) the toolbar chose: used before one beside a package or in ~/.switch
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<PathBuf>,
    /// controller defaults passed to every game (a game's own controls override them)
    #[serde(default, skip_serializing_if = "Controls::is_empty")]
    pub controls: Controls,
    #[serde(default, rename = "game")]
    pub games: Vec<Entry>,
    /// the nxnative checkout (nxnative.rs repo: NXNATIVE_DIR, else this, else the SSD's development volume)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nxnative_dir: Option<PathBuf>,
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

/// The root used when the library has none: `~/nxrt` (falls back to the config folder).
pub fn default_root() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("nxrt"))
        .unwrap_or_else(|| dir().join("nxrt"))
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
        match toml::from_str::<Library>(&text) {
            Ok(mut lib) => {
                // (a game added from its update first was once known by the update's id)
                for g in &mut lib.games {
                    if !g.title_id.is_empty() {
                        g.title_id = crate::ops::base_title_id(&g.title_id);
                    }
                }
                lib
            }
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

    /// The one root the manager works in. A stored root wins; otherwise `~/nxrt`.
    pub fn root_dir(&self) -> PathBuf {
        self.root.clone().unwrap_or_else(default_root)
    }
    pub fn games_dir(&self) -> PathBuf {
        self.root_dir().join("games")
    }
    pub fn data_dir(&self) -> PathBuf {
        self.root_dir().join("data")
    }
    pub fn recomps_dir(&self) -> PathBuf {
        self.root_dir().join("recomps")
    }

    /// Where a game's per-game data (game files, saves, settings, mods) lives under the central root.
    pub fn data_root_for(&self, entry: &Entry) -> PathBuf {
        let id = if !entry.title_id.is_empty() {
            entry.title_id.trim().to_ascii_lowercase()
        } else if !entry.name.is_empty() {
            entry
                .name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() {
                        c.to_ascii_lowercase()
                    } else {
                        '-'
                    }
                })
                .collect()
        } else {
            "game".to_string()
        };
        self.data_dir().join(id)
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
    fn one_root_derives_games_data_and_recomps() {
        let mut lib = Library::default();
        lib.root = Some(PathBuf::from("/central"));
        assert_eq!(lib.games_dir(), Path::new("/central/games"));
        assert_eq!(lib.data_dir(), Path::new("/central/data"));
        assert_eq!(lib.recomps_dir(), Path::new("/central/recomps"));
        let e = Entry {
            title_id: "0100ABCD".into(),
            ..Default::default()
        };
        assert_eq!(lib.data_root_for(&e), Path::new("/central/data/0100abcd"));
        let named = Entry {
            name: "My Game!".into(),
            ..Default::default()
        };
        assert_eq!(
            lib.data_root_for(&named),
            Path::new("/central/data/my-game-")
        );
    }

    #[test]
    fn backend_names_round_trip_and_unknown_names_are_rejected() {
        for b in [Backend::Nxrt, Backend::Nxnative] {
            assert_eq!(Backend::parse(b.name()), Some(b));
        }
        assert_eq!(Backend::parse(""), None);
        assert_eq!(Backend::parse("Metal"), None);
        assert!(Backend::Nxrt.is_nxrt());
        assert!(!Backend::Nxnative.is_nxrt());
    }

    #[test]
    fn an_old_library_has_no_controls() {
        let lib: Library = toml::from_str("[[game]]\nexe = \"/tmp/x\"\n").unwrap();
        assert!(lib.controls.is_empty());
        assert!(lib.games[0].controls.is_empty());
    }

    #[test]
    fn a_broken_library_is_set_aside_and_saves_are_whole() {
        // (the only test that points the library elsewhere: NXRT_FRONTEND_LIBRARY is the process's)
        let d = std::env::temp_dir().join(format!("nxrt-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("library.toml");
        unsafe { std::env::set_var("NXRT_FRONTEND_LIBRARY", &p) };

        // none yet: an empty library, no error
        let lib = Library::load();
        assert!(lib.games.is_empty() && lib.load_error.is_none());

        // saved and read back; no temporary file left
        let mut lib = Library::default();
        lib.add_app(Path::new("/apps/demo"), None);
        lib.save().unwrap();
        assert_eq!(Library::load().games.len(), 1);
        assert!(!d.join("library.toml.tmp").exists());

        // a file that doesn't parse: kept aside, reported, and the next save doesn't touch it
        std::fs::write(&p, "[[game]\nnot toml").unwrap();
        let lib = Library::load();
        assert!(lib.games.is_empty());
        let why = lib.load_error.clone().unwrap();
        assert!(why.contains("kept as"), "{why}");
        assert!(!p.exists());
        let aside: Vec<_> = std::fs::read_dir(&d)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("library.toml.broken-")
            })
            .collect();
        assert_eq!(aside.len(), 1);
        lib.save().unwrap();
        assert_eq!(
            std::fs::read_to_string(aside[0].path()).unwrap(),
            "[[game]\nnot toml"
        );
        unsafe { std::env::remove_var("NXRT_FRONTEND_LIBRARY") };
        let _ = std::fs::remove_dir_all(&d);
    }
}
