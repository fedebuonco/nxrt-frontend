//! What the window is shown: every game of the library, and how its own app says it stands.
//!
//! Gathering is process work (`--describe`, `--status`, counting a RomFS), so it runs once per refresh, one thread per
//! game, and the result is a plain serialisable view. Nothing here holds a lock or a Tauri type.
use crate::controls::Controls;
use crate::library::Entry;
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Clone, Default)]
pub struct Steps {
    pub files: bool,
    pub generated: bool,
    pub code: bool,
    pub builder: bool,
    pub dev: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct DescribeInfo {
    pub title: String,
    pub version: String,
    pub game_name: String,
    pub game_version: String,
    pub build_id: String,
    pub gamelib_id: String,
    pub id: String,
    pub title_id: String,
}

#[derive(Serialize, Clone, Default)]
pub struct StatusInfo {
    pub data_root: String,
    pub game_dir: String,
    pub files: String,
    pub status: String,
    pub code_id: String,
    pub files_ready: bool,
    pub code_ready: bool,
    pub ready: bool,
    pub builder: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct FilesInfo {
    pub exefs: bool,
    pub romfs_dir: bool,
    pub romfs_package: bool,
    pub build_id: Option<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct RomfsInfo {
    pub files: usize,
    pub bytes: u64,
}

#[derive(Serialize, Clone, Default)]
pub struct RunInfo {
    /// play, start, files, generate, compile, all, build, create
    pub kind: String,
    pub playing: bool,
    pub progress: Option<f32>,
    pub step: String,
    pub log_file: Option<String>,
    pub log: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
pub struct ExitInfo {
    pub text: String,
    pub ok: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct Next {
    pub label: String,
    /// play, files, generate, compile, build, add-base, create
    pub kind: String,
    pub primary: bool,
}

#[derive(Serialize, Clone, Default)]
pub struct Game {
    pub index: usize,
    pub key: String,
    pub title: String,
    pub title_id: String,
    pub publisher: String,
    pub version: String,
    pub app_name: String,
    pub app_version: String,
    pub exe: Option<String>,
    pub package: Option<String>,
    pub update: Option<String>,
    pub data_root: Option<String>,
    pub game_dir: Option<String>,
    pub recomp_dir: Option<String>,
    pub build_dir: Option<String>,
    pub build_cmd: Option<String>,
    pub keys: Option<String>,
    pub has_icon: bool,
    pub state: String,
    /// ok, warn, info, error, muted
    pub tone: String,
    pub ready: bool,
    pub steps: Steps,
    pub files_detail: String,
    pub romfs: Option<RomfsInfo>,
    pub describe: Option<DescribeInfo>,
    pub status: Option<StatusInfo>,
    pub files: Option<FilesInfo>,
    pub running: Option<RunInfo>,
    pub last_exit: Option<ExitInfo>,
    pub next: Option<Next>,
    pub load_error: Option<String>,
    pub build_mismatch: bool,
    pub controls: Controls,
}

#[derive(Serialize, Clone, Default)]
pub struct Snapshot {
    pub games: Vec<Game>,
    pub keys: Option<String>,
    pub keys_ok: bool,
    pub keys_home: Option<String>,
    pub config_dir: String,
    pub recomps_dir: String,
    pub nxrt_dir: String,
    pub version: String,
    pub running: usize,
    pub message: Option<String>,
    pub controls: Controls,
    pub log: LogInfo,
}

/// Where the manager's own log lives and the filter it was started with (docs/LOGGING.md).
#[derive(Serialize, Clone, Default)]
pub struct LogInfo {
    pub dir: String,
    pub path: Option<String>,
    pub filter: String,
}

/// What a game's nxnative build has made so far (nxnative.rs probe): read from its NSP folder, no process started.
#[derive(Default, Clone)]
pub struct Gathered {
    pub nx: crate::nxnative::Probe,
}

impl Gathered {
    pub fn gather(e: &Entry) -> Gathered {
        Gathered::gather_in(e, &crate::nxnative::repo(None))
    }
    pub fn gather_in(e: &Entry, repo: &Path) -> Gathered {
        Gathered {
            nx: crate::nxnative::probe(e, repo),
        }
    }
}

/// Every game's build probed (folders only: cheap).
pub fn gather_all(entries: &[Entry], repo: &Path) -> Vec<Gathered> {
    entries
        .iter()
        .map(|e| Gathered::gather_in(e, repo))
        .collect()
}

/// A stable name for a game across refreshes and library edits.
pub fn game_key(e: &Entry) -> String {
    if !e.title_id.is_empty() {
        format!("tid:{}", e.title_id.to_lowercase())
    } else if let Some(x) = &e.exe {
        format!("exe:{}", x.display())
    } else if let Some(p) = &e.package {
        format!("pkg:{}", p.display())
    } else if let Some(u) = &e.update {
        format!("upd:{}", u.display())
    } else {
        "entry".into()
    }
}

pub fn view(
    index: usize,
    e: &Entry,
    g: &Gathered,
    lib_keys: Option<&Path>,
    running: Option<RunInfo>,
    last_exit: Option<ExitInfo>,
) -> Game {
    let nx = &g.nx;
    let ready = e.run_cmd.is_some() && nx.host;
    let steps = Steps {
        files: nx.extracted,
        generated: nx.patched,
        code: ready,
        builder: false,
        dev: true,
    };
    let (state, tone) = summary(e, nx, running.as_ref(), ready);
    let next = if running.is_some() {
        None
    } else {
        next_action(e, nx, ready)
    };
    let files_detail = if nx.extracted {
        "Extracted from the NSPs into the build folder".to_string()
    } else if e.package.is_none() && e.update.is_none() {
        "From the game's NSP folder".to_string()
    } else {
        "From its NSP folder: extracted by the build".to_string()
    };
    let title_id = crate::nxnative::title_id(e).unwrap_or_else(|| e.title_id.clone());
    let has_icon = !e.title_id.is_empty()
        && (crate::library::icon_path(&e.title_id).is_file()
            || crate::library::icon_path(&title_id).is_file());
    let work = nx.work.as_deref().map(path_str);
    Game {
        index,
        key: game_key(e),
        title_id: e.title_id.clone(),
        title: if e.name.is_empty() {
            title_id.clone()
        } else {
            e.name.clone()
        },
        publisher: e.publisher.clone(),
        version: e.version.clone(),
        app_name: if nx.profile {
            "nxnative".into()
        } else {
            "nxnative (no profile)".into()
        },
        app_version: nx.note.clone(),
        // (the window shows its setup steps for a game that has somewhere to build: its build folder)
        exe: work.clone(),
        package: e.package.as_deref().map(path_str),
        update: e.update.as_deref().map(path_str),
        data_root: nx.work.as_deref().map(|w| path_str(&w.join("run"))),
        game_dir: nx.work.as_deref().map(|w| path_str(&w.join("extract"))),
        recomp_dir: work,
        build_dir: None,
        build_cmd: None,
        keys: e.keys.as_deref().or(lib_keys).map(path_str),
        has_icon,
        state,
        tone,
        ready,
        steps,
        files_detail,
        romfs: None,
        describe: None,
        status: None,
        files: None,
        running,
        last_exit,
        next,
        load_error: None,
        build_mismatch: false,
        controls: e.controls.clone(),
    }
}

fn path_str(p: &Path) -> String {
    p.display().to_string()
}

fn summary(
    e: &Entry,
    nx: &crate::nxnative::Probe,
    running: Option<&RunInfo>,
    ready: bool,
) -> (String, String) {
    if let Some(r) = running {
        return (
            if r.playing {
                "Playing".into()
            } else {
                "Building…".into()
            },
            "info".into(),
        );
    }
    if e.package.is_none() && e.update.is_none() {
        return ("No NSP folder for this game".into(), "warn".into());
    }
    if ready {
        return ("Ready to play".into(), "ok".into());
    }
    if !nx.profile {
        return (
            "nxnative has no profile for this game: a build is a best guess".into(),
            "warn".into(),
        );
    }
    ("Next: build it for this Mac".into(), "warn".into())
}

fn next_action(e: &Entry, nx: &crate::nxnative::Probe, ready: bool) -> Option<Next> {
    if e.package.is_none() && e.update.is_none() {
        return None;
    }
    if ready {
        return Some(Next {
            label: "Play".into(),
            kind: "play".into(),
            primary: true,
        });
    }
    Some(Next {
        label: if nx.profile {
            "Build for Mac".into()
        } else {
            "Build for Mac (best guess)".into()
        },
        kind: "build".into(),
        primary: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Entry;
    use std::path::PathBuf;

    #[test]
    fn game_keys_are_stable_across_library_edits() {
        let mut e = Entry::default();
        assert_eq!(game_key(&e), "entry");
        e.title_id = "0100ABC".into();
        assert_eq!(game_key(&e), "tid:0100abc");
        e.title_id.clear();
        e.exe = Some(PathBuf::from("/apps/game"));
        assert_eq!(game_key(&e), "exe:/apps/game");
        e.exe = None;
        e.package = Some(PathBuf::from("/games/a.nsp"));
        assert_eq!(game_key(&e), "pkg:/games/a.nsp");
        e.package = None;
        e.update = Some(PathBuf::from("/games/a-update.nsp"));
        assert_eq!(game_key(&e), "upd:/games/a-update.nsp");
    }

    fn nsp_entry() -> Entry {
        Entry {
            package: Some("/Volumes/SSD/Games/Demo/demo.nsp".into()),
            title_id: "0100000000010000".into(),
            name: "Demo".into(),
            ..Default::default()
        }
    }

    fn probe(extracted: bool, patched: bool, host: bool, profile: bool) -> Gathered {
        Gathered {
            nx: crate::nxnative::Probe {
                profile,
                extracted,
                patched,
                host,
                work: Some("/Volumes/SSD/Games/Demo/0100000000010000_build".into()),
                ..Default::default()
            },
        }
    }

    #[test]
    fn a_game_is_built_for_mac_then_played() {
        let e = nsp_entry();
        let v = view(0, &e, &probe(false, false, false, true), None, None, None);
        assert_eq!(v.next.as_ref().map(|n| n.kind.as_str()), Some("build"));
        assert_eq!(v.next.unwrap().label, "Build for Mac");
        assert!(!v.ready);
        assert_eq!(
            v.exe.as_deref(),
            Some("/Volumes/SSD/Games/Demo/0100000000010000_build")
        );
        // built, but no run command kept: not ready
        let v = view(0, &e, &probe(true, true, true, true), None, None, None);
        assert!(v.steps.files && v.steps.generated && !v.ready);
        let built = Entry {
            run_cmd: Some("cd run && dmhost patched".into()),
            ..nsp_entry()
        };
        let v = view(0, &built, &probe(true, true, true, true), None, None, None);
        assert!(v.ready);
        assert_eq!(v.next.unwrap().kind, "play");
        assert_eq!((v.state.as_str(), v.tone.as_str()), ("Ready to play", "ok"));
    }

    #[test]
    fn a_game_without_a_profile_is_a_best_guess() {
        let v = view(
            0,
            &nsp_entry(),
            &probe(false, false, false, false),
            None,
            None,
            None,
        );
        assert_eq!(v.next.unwrap().label, "Build for Mac (best guess)");
        assert_eq!(v.tone, "warn");
        assert_eq!(v.app_name, "nxnative (no profile)");
    }

    #[test]
    fn a_running_build_has_no_next_step() {
        let r = RunInfo {
            kind: "build".into(),
            ..Default::default()
        };
        let v = view(
            0,
            &nsp_entry(),
            &probe(false, false, false, true),
            None,
            Some(r),
            None,
        );
        assert!(v.next.is_none());
        assert_eq!(v.state, "Building…");
    }

    #[test]
    fn the_build_folder_is_named_after_the_base_title() {
        let e = Entry {
            title_id: "0100000000010800".into(), // (an update's id: the base game's folder)
            ..nsp_entry()
        };
        assert_eq!(
            crate::nxnative::work_dir(&e).as_deref(),
            Some(Path::new("/Volumes/SSD/Games/Demo/0100000000010000_build"))
        );
    }
}
