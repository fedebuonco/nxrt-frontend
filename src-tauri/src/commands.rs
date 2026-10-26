//! The window's side of the app: everything the web UI can ask for, plus the one thread per run that keeps it posted.
use crate::api::{self, ExitInfo, RunInfo, Snapshot};
use crate::controls::{self, Controls};
use crate::issues;
use crate::library::{self, Entry, Library};
use crate::manage;
use crate::ops;
use crate::recomp;
use crate::runner::{self, Kind, Stage};
use base64::Engine;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, State};

/// What the commands share: the library, the runs alive now, the last word of each finished run, and the icons read.
#[derive(Serialize, Clone, Default)]
pub struct PadStatus {
    pub connected: bool,
    pub name: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone)]
pub struct Shared {
    pub library: Arc<Mutex<Library>>,
    pub runs: Arc<Mutex<HashMap<String, RunHandle>>>,
    pub last: Arc<Mutex<HashMap<String, ExitInfo>>>,
    pub creating: Arc<Mutex<HashMap<String, (PathBuf, PathBuf)>>>,
    pub icons: Arc<Mutex<HashMap<String, Option<String>>>>,
    pub pad: Arc<Mutex<PadStatus>>,
}

impl Shared {
    pub fn new() -> Shared {
        Shared {
            library: Arc::new(Mutex::new(Library::load())),
            runs: Default::default(),
            last: Default::default(),
            creating: Default::default(),
            icons: Default::default(),
            pad: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct RunHandle {
    pub run: Arc<Mutex<runner::Run>>,
}

#[derive(Serialize, Clone)]
struct RunState {
    key: String,
    kind: String,
    playing: bool,
    progress: Option<f32>,
    step: String,
    log: Vec<String>,
}

#[derive(Serialize, Clone)]
struct RunFinished {
    key: String,
    text: String,
    ok: bool,
}

// ---- what the window reads ---------------------------------------------------------------------------------------

pub fn snapshot(sh: &Shared) -> Snapshot {
    let lib = sh.library.lock().unwrap().clone();
    let infos = api::gather_all(&lib.games);
    let runs = sh.runs.lock().unwrap();
    let last = sh.last.lock().unwrap();
    let mut games = Vec::with_capacity(lib.games.len());
    for (i, e) in lib.games.iter().enumerate() {
        let key = api::game_key(e);
        let running = runs.get(&key).map(run_info);
        games.push(api::view(i, e, &infos[i], lib.keys.as_deref(), running, last.get(&key).cloned()));
    }
    Snapshot {
        games,
        keys: lib.keys.as_ref().map(|k| k.display().to_string()),
        keys_ok: lib.keys.as_ref().is_some_and(|k| k.is_file()),
        keys_home: dirs::home_dir().map(|h| h.join(".switch/prod.keys")).filter(|p| p.is_file()).map(|p| p.display().to_string()),
        config_dir: library::dir().display().to_string(),
        recomps_dir: library::dir().join("recomps").display().to_string(),
        nxrt_dir: recomp::default_nxrt().display().to_string(),
        version: env!("CARGO_PKG_VERSION").into(),
        running: runs.len(),
        message: (!manage::abi_ok()).then(|| "libnxrt_manage: another ABI than this app's".to_string()),
        controls: lib.controls.clone(),
    }
}

fn run_info(h: &RunHandle) -> RunInfo {
    let r = h.run.lock().unwrap();
    let p = r.progress.lock().unwrap().clone();
    let log = r.log.lock().unwrap();
    let tail: Vec<String> = log.iter().rev().take(400).rev().cloned().collect();
    RunInfo {
        kind: kind_label(r.kind),
        playing: r.kind == Kind::Game,
        progress: p.fraction,
        step: p.step,
        log_file: r.log_file.as_ref().and_then(|f| f.file_name()).map(|n| n.to_string_lossy().into_owned()),
        log: tail,
    }
}

fn kind_label(k: Kind) -> String {
    match k {
        Kind::Game => "play",
        Kind::Setup(Stage::Files) => "files",
        Kind::Setup(Stage::Generate) => "generate",
        Kind::Setup(Stage::Compile) => "compile",
        Kind::Setup(Stage::All) => "all",
        Kind::DevBuild => "build",
        Kind::Create => "create",
    }
    .into()
}

fn here() -> String {
    library::dir().display().to_string()
}

fn log_file(id: &str, what: &str) -> Option<PathBuf> {
    let logs = library::dir().join("logs");
    let _ = std::fs::create_dir_all(&logs);
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    Some(logs.join(format!("{id}{what}-{stamp}.log")))
}

// ---- runs --------------------------------------------------------------------------------------------------------

fn start(sh: &Shared, app: &AppHandle, key: &str, run: runner::Run, code_id: String) -> Result<(), String> {
    let kind = run.kind;
    let handle = RunHandle { run: Arc::new(Mutex::new(run)) };
    {
        let mut runs = sh.runs.lock().unwrap();
        if runs.contains_key(key) {
            return Err("That game is already running".into());
        }
        runs.insert(key.to_string(), handle.clone());
    }
    sh.last.lock().unwrap().remove(key);
    let (sh, app, key) = (sh.clone(), app.clone(), key.to_string());
    let _ = app.emit("library-changed", ());
    std::thread::spawn(move || monitor(sh, app, key, kind, code_id, handle));
    Ok(())
}

fn monitor(sh: Shared, app: AppHandle, key: String, kind: Kind, code_id: String, handle: RunHandle) {
    let mut sig: Option<(String, Option<f32>, usize, String)> = None;
    loop {
        let (done, info) = {
            let mut r = handle.run.lock().unwrap();
            let done = r.exited();
            let p = r.progress.lock().unwrap().clone();
            let log = r.log.lock().unwrap();
            let tail: Vec<String> = log.iter().rev().take(400).rev().cloned().collect();
            let last_line = tail.last().cloned().unwrap_or_default();
            (done, (r.kind, p.fraction, p.step, tail, last_line))
        };
        if let Some(how) = done {
            let exit = finish(&sh, &key, kind, code_id, &handle, how);
            let _ = app.emit("run-finished", RunFinished { key: key.clone(), text: exit.text, ok: exit.ok });
            let _ = app.emit("library-changed", ());
            break;
        }
        let (k, progress, step, log, last_line) = info;
        let s = (step.clone(), progress, log.len(), last_line);
        if sig.as_ref() != Some(&s) {
            sig = Some(s);
            let _ = app.emit(
                "run-state",
                RunState { key: key.clone(), kind: kind_label(k), playing: k == Kind::Game, progress, step, log },
            );
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn finish(sh: &Shared, key: &str, kind: Kind, code_id: String, handle: &RunHandle, how: String) -> ExitInfo {
    let result = handle.run.lock().unwrap().progress.lock().unwrap().result.clone();
    let created = sh.creating.lock().unwrap().get(key).cloned();
    let mut exit = ExitInfo { text: format!("Last run: {how}"), ok: how == "closed" };
    let mut save = false;
    {
        let mut lib = sh.library.lock().unwrap();
        if let Some(e) = lib.games.iter_mut().find(|e| api::game_key(e) == key) {
            let (text, ok) = match (kind, result) {
                (Kind::Setup(stage), Some(res)) if res.ok => {
                    if stage == Stage::Generate && !code_id.is_empty() {
                        e.generated_for = code_id;
                        save = true;
                    }
                    (
                        match stage {
                            Stage::Files => "Game files set up".into(),
                            Stage::Generate => "Game code generated".into(),
                            Stage::Compile | Stage::All => "Game code compiled: ready to play".into(),
                        },
                        true,
                    )
                }
                (Kind::Setup(_), Some(res)) => {
                    let what = if res.step.is_empty() { res.status.clone() } else { res.step.clone() };
                    let hint = if !res.builder {
                        " - this app has no game-code builder: set its Dev build command".to_string()
                    } else if res.hint.is_empty() {
                        String::new()
                    } else {
                        format!(" - {}", res.hint)
                    };
                    (format!("{what}{hint}"), false)
                }
                (Kind::Setup(_), None) => (format!("The step {how}"), false),
                (Kind::DevBuild, _) => (format!("Dev build {}", if how == "closed" { "finished" } else { how.as_str() }), how == "closed"),
                (Kind::Create, _) => match created {
                    Some((exe, data)) if how == "closed" && exe.is_file() => {
                        e.exe = Some(exe);
                        e.data_root = Some(data);
                        save = true;
                        ("Recomp created: next, generate its game code".into(), true)
                    }
                    _ => ("Creating the recomp failed: see the log in the logs folder".into(), false),
                },
                (Kind::Game, _) => (format!("Last run: {how}"), how == "closed"),
            };
            exit = ExitInfo { text, ok };
        }
    }
    if save {
        let _ = sh.library.lock().unwrap().save();
    }
    sh.last.lock().unwrap().insert(key.to_string(), exit.clone());
    sh.runs.lock().unwrap().remove(key);
    exit
}

// ---- commands ----------------------------------------------------------------------------------------------------

#[tauri::command]
pub async fn snapshot_command(state: State<'_, Shared>) -> Result<Snapshot, String> {
    let sh = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || snapshot(&sh)).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn pad_status(state: State<Shared>) -> PadStatus {
    state.pad.lock().unwrap().clone()
}

#[tauri::command]
pub fn icon(state: State<Shared>, title_id: String) -> Option<String> {
    let mut cache = state.icons.lock().unwrap();
    if let Some(v) = cache.get(&title_id) {
        return v.clone();
    }
    let v = std::fs::read(library::icon_path(&title_id))
        .ok()
        .map(|b| format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(b)));
    cache.insert(title_id, v.clone());
    v
}

#[tauri::command]
pub fn pick_package() -> Option<String> {
    rfd::FileDialog::new().set_title("A Switch game (.nsp / .xci)").add_filter("Switch game", &["nsp", "xci"]).pick_file().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn pick_app() -> Option<String> {
    rfd::FileDialog::new().set_title("The game's nxrt app").pick_file().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn pick_keys() -> Option<String> {
    rfd::FileDialog::new().set_title("Your Switch's keys file (prod.keys)").add_filter("Keys", &["keys"]).pick_file().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn pick_folder(title: Option<String>) -> Option<String> {
    let mut d = rfd::FileDialog::new();
    if let Some(t) = &title {
        d = d.set_title(t);
    }
    d.pick_folder().map(|p| p.display().to_string())
}

#[tauri::command]
pub async fn add_package(state: State<'_, Shared>, path: String) -> Result<Snapshot, String> {
    let sh = state.inner().clone();
    let pkg = PathBuf::from(path);
    let chosen = sh.library.lock().unwrap().keys.clone();
    let keys = ops::keys_for(chosen.as_deref(), &pkg).ok_or("A game package is read with the keys from your own Switch (prod.keys): none chosen")?;
    let sh2 = sh.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut games = sh2.library.lock().unwrap().games.clone();
        ops::add_package_to(&mut games, &pkg, &keys)?;
        let mut lib = sh2.library.lock().unwrap();
        lib.games = games;
        lib.save()
    })
    .await
    .map_err(|e| e.to_string())??;
    Ok(snapshot(&sh))
}

#[tauri::command]
pub async fn add_app(state: State<'_, Shared>, path: String, data_root: Option<String>) -> Result<Snapshot, String> {
    let sh = state.inner().clone();
    let exe = ops::resolve_app(Path::new(&path));
    let d = runner::describe(&exe)?;
    let dr = data_root.filter(|s| !s.trim().is_empty()).map(PathBuf::from);
    {
        let mut lib = sh.library.lock().unwrap();
        if lib.games.iter().any(|g| g.exe.as_deref() == Some(exe.as_path())) {
            return Err("Already in the list".into());
        }
        if let Some(g) = lib.games.iter_mut().find(|g| g.exe.is_none() && g.title_id.eq_ignore_ascii_case(&d.title_id)) {
            g.exe = Some(exe);
            g.data_root = dr;
        } else {
            lib.games.push(Entry { exe: Some(exe), data_root: dr, ..Default::default() });
        }
        lib.save()?;
    }
    Ok(snapshot(&sh))
}

#[tauri::command]
pub async fn choose_app(state: State<'_, Shared>, index: usize, path: String) -> Result<Snapshot, String> {
    let sh = state.inner().clone();
    let exe = ops::resolve_app(Path::new(&path));
    let d = runner::describe(&exe)?;
    {
        let mut lib = sh.library.lock().unwrap();
        let Some(g) = lib.games.get(index) else { return Err("No such game".into()) };
        if !g.title_id.is_empty() && !d.title_id.eq_ignore_ascii_case(&g.title_id) {
            return Err(format!("{} is for {}, not this game", d.title, d.game_name));
        }
        // (an entry for that app alone gives way to this one, its data folder kept)
        let mut i = index;
        let dup = lib.games.iter().position(|g| g.exe.as_deref() == Some(exe.as_path()) && g.package.is_none() && g.update.is_none());
        if let Some(j) = dup.filter(|j| *j != index) {
            let dr = lib.games[j].data_root.clone();
            lib.games[index].data_root = dr;
            lib.games.remove(j);
            if j < i {
                i -= 1;
            }
        }
        lib.games[i].exe = Some(exe);
        lib.save()?;
    }
    Ok(snapshot(&sh))
}

#[tauri::command]
pub async fn remove(state: State<'_, Shared>, index: usize) -> Result<Snapshot, String> {
    let sh = state.inner().clone();
    {
        let mut lib = sh.library.lock().unwrap();
        if index >= lib.games.len() {
            return Err("No such game".into());
        }
        let key = api::game_key(&lib.games[index]);
        if let Some(h) = sh.runs.lock().unwrap().get(&key) {
            h.run.lock().unwrap().stop();
        }
        lib.games.remove(index);
        lib.save()?;
    }
    Ok(snapshot(&sh))
}

#[tauri::command]
pub fn set_keys(state: State<Shared>, path: String) -> Result<Snapshot, String> {
    let p = PathBuf::from(path);
    library::check_keys(&p)?;
    state.library.lock().unwrap().keys = Some(p);
    state.library.lock().unwrap().save()?;
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn forget_keys(state: State<Shared>) -> Result<Snapshot, String> {
    state.library.lock().unwrap().keys = None;
    state.library.lock().unwrap().save()?;
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn start_game(state: State<Shared>, app: AppHandle, key: String, mode: String) -> Result<(), String> {
    let (entry, library_controls) = {
        let lib = state.library.lock().unwrap();
        let e = lib.games.iter().find(|e| api::game_key(e) == key).cloned().ok_or("No such game")?;
        (e, lib.controls.clone())
    };
    let exe = entry.exe.clone().ok_or("This game has no nxrt app yet")?;
    // (the game's own controls over the library's defaults; only what is set is passed)
    let effective = library_controls.merged(&entry.controls);
    let overlay = if effective.is_empty() {
        None
    } else {
        let path = controls::overlay_path(&key);
        effective.write_overlay(&path)?;
        Some(path)
    };
    let id = runner::describe(&exe).map(|d| d.id).unwrap_or_else(|_| "game".into());
    let run = runner::Run::game(&exe, entry.data_root.as_deref(), mode == "play", log_file(&id, ""), || {}, overlay.as_deref())?;
    start(&state, &app, &key, run, String::new())
}

/// Opens the nxrt GitHub new-issue form with this game, its build, its latest log and the environment filled in.
#[tauri::command]
pub fn open_issue(state: State<Shared>, key: String) -> Result<(), String> {
    let (index, entry) = {
        let lib = state.library.lock().unwrap();
        let (index, e) = lib.games.iter().enumerate().find(|(_, e)| api::game_key(e) == key).ok_or("No such game")?;
        (index, e.clone())
    };
    let last = state.last.lock().unwrap().get(&key).cloned();
    let g = api::Gathered::gather(&entry);
    let view = api::view(index, &entry, &g, None, None, last);
    let url = issues::for_game(&view, &recomp::default_nxrt(), &library::dir().join("logs"));
    ops::open_url(&url)
}

/// Save the controller defaults (`scope = "defaults"`) or one game's overrides (`scope = "game"`, its key).
#[tauri::command]
pub fn save_controls(state: State<Shared>, scope: String, key: Option<String>, controls: Controls) -> Result<Snapshot, String> {
    controls.validate()?;
    {
        let mut lib = state.library.lock().unwrap();
        match scope.as_str() {
            "defaults" => lib.controls = controls,
            "game" => {
                let k = key.ok_or("No game given")?;
                let g = lib.games.iter_mut().find(|e| api::game_key(e) == k).ok_or("No such game")?;
                g.controls = controls;
            }
            other => return Err(format!("Unknown controls scope: {other}")),
        }
        lib.save()?;
    }
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn start_step(state: State<Shared>, app: AppHandle, key: String, stage: String, src: Option<String>) -> Result<(), String> {
    let (entry, keys) = {
        let lib = state.library.lock().unwrap();
        let e = lib.games.iter().find(|e| api::game_key(e) == key).cloned().ok_or("No such game")?;
        let k = lib.keys.clone().filter(|k| k.is_file()).or_else(|| e.keys.clone());
        (e, k)
    };
    let exe = entry.exe.clone().ok_or("This game has no nxrt app yet")?;
    let stage = match stage.as_str() {
        "files" => Stage::Files,
        "generate" => Stage::Generate,
        "compile" => Stage::Compile,
        _ => Stage::All,
    };
    let src = src.or_else(|| entry.package.as_ref().map(|p| p.display().to_string())).unwrap_or_default();
    if stage == Stage::Files && src.is_empty() {
        return Err("No package or folder for the game files".into());
    }
    let code_id = runner::status(&exe, entry.data_root.as_deref()).map(|s| s.code_id).unwrap_or_default();
    let id = runner::describe(&exe).map(|d| d.id).unwrap_or_else(|_| "game".into());
    let what = match stage {
        Stage::Files => "-files",
        Stage::Generate => "-generate",
        Stage::Compile => "-compile",
        Stage::All => "-build",
    };
    let run = runner::Run::setup(&exe, entry.data_root.as_deref(), stage, &src, keys.as_deref(), entry.update.as_deref(), log_file(&id, what), || {})?;
    start(&state, &app, &key, run, code_id)
}

#[tauri::command]
pub fn start_dev(state: State<Shared>, app: AppHandle, key: String) -> Result<(), String> {
    let entry = {
        let lib = state.library.lock().unwrap();
        lib.games.iter().find(|e| api::game_key(e) == key).cloned().ok_or("No such game")?
    };
    let (dir, cmd) = match (&entry.build_dir, &entry.build_cmd) {
        (Some(d), Some(c)) => (d.clone(), c.clone()),
        _ => return Err("No Dev build command set".into()),
    };
    let run = runner::Run::dev_build(&cmd, &dir, log_file("dev", "-build"), || {})?;
    start(&state, &app, &key, run, String::new())
}

#[tauri::command]
pub fn stop(state: State<Shared>, key: String) {
    if let Some(h) = state.runs.lock().unwrap().get(&key) {
        h.run.lock().unwrap().stop();
    }
}

#[tauri::command]
pub async fn create_recomp(state: State<'_, Shared>, app: AppHandle, index: usize, id: String, dir: String) -> Result<(), String> {
    let sh = state.inner().clone();
    let (entry, keys) = {
        let lib = sh.library.lock().unwrap();
        let e = lib.games.get(index).cloned().ok_or("No such game")?;
        let k = lib
            .keys
            .clone()
            .filter(|k| k.is_file())
            .or_else(|| e.keys.clone())
            .or_else(|| e.package.as_deref().and_then(manage::find_keys))
            .ok_or("No keys: choose your prod.keys first")?;
        (e, k)
    };
    let (id, dir) = (id.trim().to_string(), PathBuf::from(dir.trim()));
    let nxrt = recomp::default_nxrt();
    let (e2, id2, dir2, nxrt2, keys2) = (entry.clone(), id.clone(), dir.clone(), nxrt, keys.clone());
    tauri::async_runtime::spawn_blocking(move || ops::create_recomp(&e2, &id2, &dir2, &nxrt2, &keys2)).await.map_err(|e| e.to_string())??;
    let key = api::game_key(&entry);
    let app_path = recomp::app_path(&dir, &id);
    sh.creating.lock().unwrap().insert(key.clone(), (app_path, dir.join("data")));
    {
        let mut lib = sh.library.lock().unwrap();
        if let Some(g) = lib.games.iter_mut().find(|e| api::game_key(e) == key) {
            g.recomp_dir = Some(dir.clone());
        }
        lib.save()?;
    }
    let run = runner::Run::create(&dir, log_file(&id, "-create"), || {})?;
    start(&sh, &app, &key, run, String::new())
}

#[tauri::command]
pub fn save_dev(state: State<Shared>, index: usize, dir: String, cmd: String) -> Result<Snapshot, String> {
    {
        let mut lib = state.library.lock().unwrap();
        let Some(g) = lib.games.get_mut(index) else { return Err("No such game".into()) };
        let d = dir.trim();
        let c = cmd.trim();
        g.build_dir = (!d.is_empty()).then(|| PathBuf::from(d));
        g.build_cmd = (!c.is_empty()).then_some(c.to_string());
        lib.save()?;
    }
    Ok(snapshot(&state))
}

#[tauri::command]
pub fn open_logs() {
    let dir = library::dir().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    ops::reveal(&dir);
}

#[tauri::command]
pub fn reveal(path: String, file: bool) {
    let p = Path::new(&path);
    if file {
        ops::reveal_file(p)
    } else {
        ops::reveal(p)
    }
}

#[derive(Serialize, Clone)]
pub struct LogRow {
    pub name: String,
    pub path: String,
    pub bytes: u64,
    pub mtime: u64,
}

#[tauri::command]
pub fn logs() -> Vec<LogRow> {
    let dir = library::dir().join("logs");
    let mut out: Vec<LogRow> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.flatten()
                .filter_map(|e| {
                    let m = e.metadata().ok()?;
                    let mtime = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_secs();
                    Some(LogRow {
                        name: e.file_name().to_string_lossy().into_owned(),
                        path: e.path().display().to_string(),
                        bytes: m.len(),
                        mtime,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    out.truncate(80);
    out
}

#[tauri::command]
pub fn config_dir() -> String {
    here()
}
