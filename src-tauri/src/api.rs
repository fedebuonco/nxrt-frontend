//! What the window is shown: every game of the library, and how its own app says it stands.
//!
//! Gathering is process work (`--describe`, `--status`, counting a RomFS), so it runs once per refresh, one thread per
//! game, and the result is a plain serialisable view. Nothing here holds a lock or a Tauri type.
use crate::controls::Controls;
use crate::library::Entry;
use crate::manage;
use crate::runner;
use serde::Serialize;
use std::path::{Path, PathBuf};

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

/// A game's app and files, as its runner answers.
#[derive(Default, Clone)]
pub struct Gathered {
    pub describe: Option<Result<runner::Describe, String>>,
    pub status: Option<Result<runner::Status, String>>,
    pub files: Option<manage::GameFiles>,
    pub romfs: Option<Result<manage::RomfsSummary, String>>,
}

impl Gathered {
    pub fn gather(e: &Entry) -> Gathered {
        let Some(exe) = &e.exe else {
            return Gathered::default();
        };
        let describe = Some(runner::describe(exe));
        if let Some(Err(err)) = &describe {
            tracing::warn!(target: "api", exe = %exe.display(), error = %err, "describe failed");
        }
        let status = Some(runner::status(exe, e.data_root.as_deref()));
        if let Some(Err(err)) = &status {
            tracing::warn!(target: "api", exe = %exe.display(), error = %err, "status failed");
        }
        let dir = status.as_ref().and_then(|s| s.as_ref().ok()).map(game_dir);
        let files = dir.as_deref().and_then(manage::game_files);
        let romfs = dir
            .as_deref()
            .filter(|_| files.as_ref().is_some_and(|f| f.exefs))
            .map(manage::romfs_summary);
        Gathered {
            describe,
            status,
            files,
            romfs,
        }
    }
}

/// Every game's app asked at once: a refresh of four games then costs one.
pub fn gather_all(entries: &[Entry]) -> Vec<Gathered> {
    let mut out = Vec::with_capacity(entries.len());
    std::thread::scope(|scope| {
        let handles: Vec<_> = entries
            .iter()
            .map(|e| scope.spawn(move || Gathered::gather(e)))
            .collect();
        for h in handles {
            out.push(h.join().unwrap_or_default());
        }
    });
    out
}

/// The game folder --status names, absolute (a relative one is in the data root: "extracted").
pub fn game_dir(s: &runner::Status) -> PathBuf {
    let g = PathBuf::from(&s.game_dir);
    if g.is_absolute() {
        g
    } else {
        PathBuf::from(&s.data_root).join(g)
    }
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
    let d = g.describe.as_ref().and_then(|r| r.as_ref().ok());
    let s = g.status.as_ref().and_then(|r| r.as_ref().ok());
    let title = if !e.name.is_empty() {
        e.name.clone()
    } else if let Some(d) = d.filter(|d| !d.game_name.is_empty()) {
        d.game_name.clone()
    } else {
        e.exe
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let title_id = if !e.title_id.is_empty() {
        e.title_id.clone()
    } else {
        d.map(|d| d.title_id.clone()).unwrap_or_default()
    };
    let files = s.is_some_and(|s| s.files_ready);
    let code = s.is_some_and(|s| s.code_ready);
    let generated =
        code || s.is_some_and(|s| !s.code_id.is_empty() && s.code_id == e.generated_for);
    let builder = s.is_some_and(|s| s.builder);
    let dev = e.build_dir.is_some() && e.build_cmd.is_some();
    let ready = s.is_some_and(|s| s.ready);
    let steps = Steps {
        files,
        generated,
        code,
        builder,
        dev,
    };
    let (state, tone) = summary(e, g, running.as_ref(), files, generated);
    let next = if running.is_some() {
        None
    } else {
        next_action(e, s, files, generated, builder, dev)
    };
    let files_detail = if files {
        match (
            g.files.as_ref(),
            g.romfs.as_ref().and_then(|r| r.as_ref().ok()),
        ) {
            (Some(f), Some(r)) if f.romfs_package => format!(
                "Read from the package in place · {} files, {:.1} GB",
                r.files,
                r.bytes as f64 / 1e9
            ),
            (Some(f), Some(r)) if f.romfs_dir => format!(
                "An extracted copy · {} files, {:.1} GB",
                r.files,
                r.bytes as f64 / 1e9
            ),
            _ => "Ready".into(),
        }
    } else if e.package.is_none() {
        "From the game's package or an extracted folder".into()
    } else {
        "From its package, in seconds: nothing is copied".into()
    };
    let romfs = g
        .romfs
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|r| RomfsInfo {
            files: r.files,
            bytes: r.bytes,
        });
    let describe = d.map(|d| DescribeInfo {
        title: d.title.clone(),
        version: d.version.clone(),
        game_name: d.game_name.clone(),
        game_version: d.game_version.clone(),
        build_id: d.build_id.clone(),
        gamelib_id: d.gamelib_id.clone(),
        id: d.id.clone(),
        title_id: d.title_id.clone(),
    });
    let status = s.map(|s| StatusInfo {
        data_root: s.data_root.clone(),
        game_dir: s.game_dir.clone(),
        files: s.files.clone(),
        status: s.status.clone(),
        code_id: s.code_id.clone(),
        files_ready: s.files_ready,
        code_ready: s.code_ready,
        ready: s.ready,
        builder: s.builder,
    });
    let files_info = g.files.as_ref().map(|f| FilesInfo {
        exefs: f.exefs,
        romfs_dir: f.romfs_dir,
        romfs_package: f.romfs_package,
        build_id: f.build_id.clone(),
    });
    let load_error = match &g.describe {
        Some(Err(err)) => Some(err.clone()),
        _ => None,
    };
    let build_mismatch = d.is_some_and(|d| {
        !e.build_id.is_empty()
            && !d.build_id.is_empty()
            && !e.build_id.eq_ignore_ascii_case(&d.build_id)
    });
    let has_icon = !title_id.is_empty() && crate::library::icon_path(&title_id).is_file();
    Game {
        index,
        key: game_key(e),
        title_id,
        title,
        publisher: e.publisher.clone(),
        version: if !e.version.is_empty() {
            e.version.clone()
        } else {
            d.map(|d| d.game_version.clone()).unwrap_or_default()
        },
        app_name: d.map(|d| d.title.clone()).unwrap_or_default(),
        app_version: d.map(|d| d.version.clone()).unwrap_or_default(),
        exe: e.exe.as_deref().map(path_str),
        package: e.package.as_deref().map(path_str),
        update: e.update.as_deref().map(path_str),
        data_root: s
            .map(|s| s.data_root.clone())
            .or_else(|| e.data_root.as_deref().map(path_str)),
        game_dir: s.map(|s| game_dir(s).display().to_string()),
        recomp_dir: e.recomp_dir.as_deref().map(path_str),
        build_dir: e.build_dir.as_deref().map(path_str),
        build_cmd: e.build_cmd.clone(),
        keys: e.keys.as_deref().or(lib_keys).map(path_str),
        has_icon,
        state,
        tone,
        ready,
        steps,
        files_detail,
        romfs,
        describe,
        status,
        files: files_info,
        running,
        last_exit,
        next,
        load_error,
        build_mismatch,
        controls: e.controls.clone(),
    }
}

fn path_str(p: &Path) -> String {
    p.display().to_string()
}

fn summary(
    e: &Entry,
    g: &Gathered,
    running: Option<&RunInfo>,
    files: bool,
    generated: bool,
) -> (String, String) {
    if let Some(r) = running {
        return (
            if r.playing {
                "Playing".into()
            } else {
                "Working…".into()
            },
            "info".into(),
        );
    }
    if e.package.is_none() && e.update.is_some() {
        return ("An update: add its base game".into(), "warn".into());
    }
    if e.exe.is_none() {
        return ("No nxrt app for this game yet".into(), "warn".into());
    }
    match g.status.as_ref() {
        None => ("The app didn't answer".into(), "muted".into()),
        Some(Err(err)) => (err.clone(), "error".into()),
        Some(Ok(s)) if s.ready => ("Ready to play".into(), "ok".into()),
        Some(Ok(_)) => {
            let next = if !files {
                "Next: set up the game files"
            } else if !generated {
                "Next: generate the game code"
            } else {
                "Next: compile the game code"
            };
            (next.into(), "warn".into())
        }
    }
}

fn next_action(
    e: &Entry,
    s: Option<&runner::Status>,
    files: bool,
    generated: bool,
    builder: bool,
    dev: bool,
) -> Option<Next> {
    if e.package.is_none() && e.update.is_some() {
        return Some(Next {
            label: "Add its base game".into(),
            kind: "add-base".into(),
            primary: false,
        });
    }
    if e.exe.is_none() {
        return e.package.is_some().then(|| Next {
            label: "Create a recomp".into(),
            kind: "create".into(),
            primary: false,
        });
    }
    let s = s?;
    if s.ready {
        return Some(Next {
            label: "Play".into(),
            kind: "play".into(),
            primary: true,
        });
    }
    if !files {
        return Some(Next {
            label: "Set up the game files".into(),
            kind: "files".into(),
            primary: true,
        });
    }
    if !builder && dev {
        return Some(Next {
            label: "Build the game code".into(),
            kind: "build".into(),
            primary: true,
        });
    }
    if !generated {
        return Some(Next {
            label: "Generate".into(),
            kind: "generate".into(),
            primary: true,
        });
    }
    Some(Next {
        label: "Compile".into(),
        kind: "compile".into(),
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

    #[test]
    fn a_relative_game_dir_is_resolved_in_the_data_root() {
        let st = runner::Status {
            data_root: "/data".into(),
            game_dir: "extracted".into(),
            files: String::new(),
            status: String::new(),
            code_id: String::new(),
            files_ready: false,
            code_ready: false,
            ready: false,
            builder: false,
        };
        assert_eq!(game_dir(&st), PathBuf::from("/data/extracted"));
        let abs = runner::Status {
            game_dir: "/elsewhere/game".into(),
            ..st
        };
        assert_eq!(game_dir(&abs), PathBuf::from("/elsewhere/game"));
    }

    fn gathered(s: runner::Status) -> Gathered {
        Gathered {
            status: Some(Ok(s)),
            ..Default::default()
        }
    }

    fn app_entry() -> Entry {
        Entry {
            exe: Some("/apps/demo".into()),
            package: Some("/g/demo.nsp".into()),
            title_id: "0100000000010000".into(),
            ..Default::default()
        }
    }

    fn next_of(e: &Entry, g: &Gathered) -> Option<String> {
        view(0, e, g, None, None, None).next.map(|n| n.kind)
    }

    #[test]
    fn the_steps_come_in_order() {
        let e = app_entry();
        let mut s = runner::Status {
            builder: true,
            code_id: "c1".into(),
            ..Default::default()
        };
        assert_eq!(next_of(&e, &gathered(s.clone())).as_deref(), Some("files"));
        s.files_ready = true;
        assert_eq!(
            next_of(&e, &gathered(s.clone())).as_deref(),
            Some("generate")
        );
        let generated = Entry {
            generated_for: "c1".into(),
            ..app_entry()
        };
        assert_eq!(
            next_of(&generated, &gathered(s.clone())).as_deref(),
            Some("compile")
        );
        // (generated for other game code: generate again)
        let stale = Entry {
            generated_for: "c0".into(),
            ..app_entry()
        };
        assert_eq!(
            next_of(&stale, &gathered(s.clone())).as_deref(),
            Some("generate")
        );
        s.code_ready = true;
        s.ready = true;
        let v = view(0, &e, &gathered(s), None, None, None);
        assert_eq!(v.next.unwrap().kind, "play");
        assert_eq!((v.state.as_str(), v.tone.as_str()), ("Ready to play", "ok"));
    }

    #[test]
    fn a_source_checkout_builds_its_code_itself() {
        let e = Entry {
            build_dir: Some("/src/demo".into()),
            build_cmd: Some("make".into()),
            ..app_entry()
        };
        let s = runner::Status {
            files_ready: true,
            ..Default::default()
        };
        assert_eq!(next_of(&e, &gathered(s)).as_deref(), Some("build"));
    }

    #[test]
    fn a_game_without_its_app_or_its_base() {
        let no_app = Entry {
            exe: None,
            ..app_entry()
        };
        assert_eq!(
            next_of(&no_app, &Gathered::default()).as_deref(),
            Some("create")
        );
        let update_only = Entry {
            package: None,
            update: Some("/g/upd.nsp".into()),
            ..app_entry()
        };
        let v = view(0, &update_only, &Gathered::default(), None, None, None);
        assert_eq!(v.next.unwrap().kind, "add-base");
        assert_eq!(v.tone, "warn");
    }

    #[test]
    fn a_running_game_has_no_next_step_and_an_app_that_fails_says_why() {
        let e = app_entry();
        let running = RunInfo {
            playing: true,
            ..Default::default()
        };
        let v = view(
            0,
            &e,
            &gathered(runner::Status::default()),
            None,
            Some(running),
            None,
        );
        assert!(v.next.is_none());
        assert_eq!(v.state, "Playing");
        let failed = Gathered {
            describe: Some(Err("doesn't answer --describe".into())),
            status: Some(Err("doesn't answer --status".into())),
            ..Default::default()
        };
        let v = view(0, &e, &failed, None, None, None);
        assert_eq!(v.tone, "error");
        assert_eq!(v.load_error.as_deref(), Some("doesn't answer --describe"));
        assert!(v.next.is_none());
    }
}
