//! The operations both the window and the command line use: adding a game or an app, making a new recomp, showing
//! paths in Finder. No UI, no Tauri.
use crate::library::{self, Entry};
use crate::manage;
use crate::recomp;
use crate::runner;
use std::path::{Path, PathBuf};

/// The keys for a package: the chosen prod.keys, else one beside the package or in ~/.switch.
pub fn keys_for(chosen: Option<&Path>, package: &Path) -> Option<PathBuf> {
    chosen.filter(|k| k.is_file()).map(Path::to_path_buf).or_else(|| manage::find_keys(package))
}

/// The package's game into the list: its control data read, its icon kept, the app in the list that runs it (same
/// title id) given it. Returns the game's index.
pub fn add_package_to(games: &mut Vec<Entry>, package: &Path, keys: &Path) -> Result<usize, String> {
    let p = manage::package_info(package, keys).map_err(|e| format!("{}: {e}", package.display()))?;
    if let Some(icon) = &p.icon {
        let path = library::icon_path(&p.title_id);
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(&path, icon);
    }
    if let Some(i) = games.iter().position(|g| g.package.as_deref() == Some(package) || g.update.as_deref() == Some(package)) {
        return Ok(i); // (already here)
    }
    // base game and update pair up (the same title; an update's id differs in its low bits)
    let base_tid = |t: &str| u64::from_str_radix(t, 16).map(|v| v & !0xFFF).unwrap_or(0);
    let same = |t: &str| !t.is_empty() && base_tid(t) == base_tid(&p.title_id);
    let partner = games.iter().position(|g| same(&g.title_id) && if p.is_update { g.update.is_none() } else { g.package.is_none() && g.update.is_some() });
    if let Some(i) = partner {
        let e = &mut games[i];
        if p.is_update {
            // (the update's code is what runs: its version and build)
            e.update = Some(package.to_path_buf());
            e.version = p.version;
            e.build_id = p.build_id.unwrap_or_default();
        } else {
            e.package = Some(package.to_path_buf());
            if e.name.is_empty() {
                e.name = p.name;
            }
        }
        return Ok(i);
    }
    // the game's app: an entry whose app runs this title (and has no package yet)
    let app = games.iter().position(|g| {
        g.package.is_none()
            && g.exe.as_ref().is_some_and(|e| runner::describe(e).is_ok_and(|d| d.title_id.eq_ignore_ascii_case(&p.title_id)))
    });
    let i = match app {
        Some(i) => i,
        None => {
            games.push(Entry::default());
            games.len() - 1
        }
    };
    let e = &mut games[i];
    if p.is_update {
        e.update = Some(package.to_path_buf())
    } else {
        e.package = Some(package.to_path_buf())
    }
    e.keys = Some(keys.to_path_buf());
    e.title_id = p.title_id;
    e.name = p.name;
    e.publisher = p.publisher;
    e.version = p.version;
    e.build_id = p.build_id.unwrap_or_default();
    Ok(i)
}

/// A new recomp for a game added from its package (recomp.rs): its game files set up from the package into
/// <dir>/data/extracted, its folder written. Its app is built next by the caller.
pub fn create_recomp(e: &Entry, id: &str, dir: &Path, nxrt: &Path, keys: &Path) -> Result<(), String> {
    tracing::info!(target: "recomp", id = %id, dir = %dir.display(), "recomp: reading the package");
    if !recomp::valid_id(id) {
        return Err("The id is a short name in lower-case letters and digits (it names the app: <id>recomp)".into());
    }
    if !nxrt.join("nxrt.mk").is_file() {
        return Err(format!("{}: not an nxrt checkout (no nxrt.mk)", nxrt.display()));
    }
    let Some(pkg) = &e.package else {
        return Err(if e.update.is_some() { "This is the game's update: add its base game first".into() } else { "No package for this game".into() });
    };
    if dir.join("Makefile").exists() {
        return Err(format!("{} already has a recomp", dir.display()));
    }
    let files = dir.join("data").join("extracted");
    std::fs::create_dir_all(&files).map_err(|err| format!("{}: {err}", files.display()))?;
    manage::package_setup(pkg, e.update.as_deref(), keys, &files)?;
    let modules = recomp::layout(&files.join("exefs"))?;
    recomp::scaffold(dir, id, e, &modules, nxrt)
}

/// A macOS .app bundle picked: its executable inside.
pub fn resolve_app(p: &Path) -> PathBuf {
    if p.extension().is_some_and(|e| e == "app") {
        if let Ok(rd) = std::fs::read_dir(p.join("Contents/MacOS")) {
            if let Some(e) = rd.flatten().find(|e| e.path().is_file()) {
                return e.path();
            }
        }
    }
    p.to_path_buf()
}

pub fn reveal(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
}

/// The release page from the update manifest: only the project's GitHub pages.
pub fn open_update(url: &str) -> Result<(), String> {
    if !url.starts_with("https://github.com/fedebuonco/") {
        return Err("Refusing to open an unexpected update URL".into());
    }
    open_browser(url)
}

/// The little support link in the sidebar.
pub fn open_donate() -> Result<(), String> {
    open_browser("https://ko-fi.com/fedebuonco")
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn().map_err(|e| format!("open: {e}"))?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = url;
        Err("Opening a browser is only wired for macOS here".into())
    }
}

/// Opens the prefilled new-issue URL (only the nxrt issue form is allowed).
pub fn open_url(url: &str) -> Result<(), String> {
    let prefix = format!("{}?", crate::issues::NEW_ISSUE);
    if !url.starts_with(&prefix) {
        return Err("Refusing to open an unexpected URL".into());
    }
    open_browser(url)
}

/// Shows a file in Finder (selected), or opens a folder.
pub fn reveal_file(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(not(target_os = "macos"))]
    if let Some(d) = p.parent() {
        reveal(d);
    }
}
