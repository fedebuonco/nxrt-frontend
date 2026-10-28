//! The operations both the window and the command line use: adding a game or an app, making a new recomp, showing
//! paths in Finder. No UI, no Tauri.
use crate::library::{self, Entry};
use crate::manage;
use crate::recomp;
use crate::runner;
use std::path::{Path, PathBuf};

/// The keys for a package: the chosen prod.keys, else one beside the package or in ~/.switch.
pub fn keys_for(chosen: Option<&Path>, package: &Path) -> Option<PathBuf> {
    chosen
        .filter(|k| k.is_file())
        .map(Path::to_path_buf)
        .or_else(|| manage::find_keys(package))
}

/// A game's own title id: its update's (…800) differs from it in the low bits.
pub fn base_title_id(t: &str) -> String {
    match u64::from_str_radix(t, 16) {
        Ok(v) => format!("{:016X}", v & !0xFFF),
        Err(_) => t.to_string(),
    }
}

/// Two title ids of the same game (the game, its update).
pub fn same_game(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && base_title_id(a).eq_ignore_ascii_case(&base_title_id(b))
}

/// What adding a package needs to know, read without the library locked (seconds: the package's control data, and
/// each app in the library asked which game it runs).
pub struct PackageRead {
    pub info: manage::Package,
    /// (app, the title id it runs) for the library's apps that have no package yet
    pub apps: Vec<(PathBuf, String)>,
}

pub fn read_package(games: &[Entry], package: &Path, keys: &Path) -> Result<PackageRead, String> {
    let info =
        manage::package_info(package, keys).map_err(|e| format!("{}: {e}", package.display()))?;
    if let Some(icon) = &info.icon {
        // (under the game's id: an update's icon is its game's)
        let path = library::icon_path(&base_title_id(&info.title_id));
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(&path, icon);
    }
    let apps = games
        .iter()
        .filter(|g| g.package.is_none())
        .filter_map(|g| g.exe.clone())
        .filter_map(|e| runner::describe(&e).ok().map(|d| (e, d.title_id)))
        .collect();
    Ok(PackageRead { info, apps })
}

/// The package's game into the list (the list as it is now): a game is one entry, known by its base title id - its
/// package and its update join it (a newer update, or the game from another file, takes the old one's place); a game
/// not in the list yet joins the app that runs it, else it is a new entry. Returns its index. No I/O: the reading is
/// read_package's.
pub fn merge_package(games: &mut Vec<Entry>, package: &Path, keys: &Path, r: PackageRead) -> usize {
    let p = r.info;
    let tid = base_title_id(&p.title_id);
    if let Some(i) = games
        .iter()
        .position(|g| g.package.as_deref() == Some(package) || g.update.as_deref() == Some(package))
    {
        return i; // (already here)
    }
    let partner = games
        .iter()
        .position(|g| same_game(&g.title_id, &tid) && (g.package.is_some() || g.update.is_some()));
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
            if e.update.is_none() {
                e.version = p.version;
                e.build_id = p.build_id.unwrap_or_default();
            }
        }
        e.keys.get_or_insert_with(|| keys.to_path_buf());
        return i;
    }
    // the game's app: an entry whose app runs this game (and has no package yet)
    let app = games.iter().position(|g| {
        g.package.is_none()
            && g.exe
                .as_ref()
                .is_some_and(|e| r.apps.iter().any(|(x, t)| x == e && same_game(t, &tid)))
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
    e.title_id = tid;
    e.name = p.name;
    e.publisher = p.publisher;
    e.version = p.version;
    e.build_id = p.build_id.unwrap_or_default();
    i
}

/// The package's game into the list: read, then merged. Returns the game's index.
pub fn add_package_to(
    games: &mut Vec<Entry>,
    package: &Path,
    keys: &Path,
) -> Result<usize, String> {
    let r = read_package(games, package, keys)?;
    Ok(merge_package(games, package, keys, r))
}

/// A new recomp for a game added from its package (recomp.rs): its game files set up from the package into
/// <dir>/data/extracted, its folder written. Its app is built next by the caller.
pub fn create_recomp(
    e: &Entry,
    id: &str,
    dir: &Path,
    nxrt: &Path,
    keys: &Path,
) -> Result<(), String> {
    tracing::info!(target: "recomp", id = %id, dir = %dir.display(), "recomp: reading the package");
    if !recomp::valid_id(id) {
        return Err("The id is a short name in lower-case letters and digits (it names the app: <id>recomp)".into());
    }
    if !nxrt.join("nxrt.mk").is_file() {
        return Err(format!(
            "{}: not an nxrt checkout (no nxrt.mk)",
            nxrt.display()
        ));
    }
    let Some(pkg) = &e.package else {
        return Err(if e.update.is_some() {
            "This is the game's update: add its base game first".into()
        } else {
            "No package for this game".into()
        });
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
/// Packages under a games root, recursively (`.nsp`/`.xci`, case-insensitive), sorted. Hidden directories are skipped.
pub fn find_packages(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
        if depth > 8 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name();
            if name.to_string_lossy().starts_with('.') {
                continue;
            }
            if p.is_dir() {
                walk(&p, depth + 1, out);
            } else if p
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("nsp") || x.eq_ignore_ascii_case("xci"))
            {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort();
    out
}

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
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("open: {e}"))?;
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pkg(title_id: &str, update: bool) -> manage::Package {
        manage::Package {
            title_id: title_id.into(),
            build_id: Some(if update { "bb02" } else { "bb01" }.into()),
            name: "Demo".into(),
            publisher: "Pub".into(),
            version: if update { "1.2.0" } else { "1.0.0" }.into(),
            icon: None,
            is_update: update,
        }
    }

    fn read(p: manage::Package, apps: &[(&str, &str)]) -> PackageRead {
        PackageRead {
            info: p,
            apps: apps
                .iter()
                .map(|(e, t)| (PathBuf::from(e), t.to_string()))
                .collect(),
        }
    }

    const KEYS: &str = "/k/prod.keys";

    #[test]
    fn a_new_game_is_a_new_entry() {
        let mut games = vec![];
        let i = merge_package(
            &mut games,
            Path::new("/g/base.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        assert_eq!(i, 0);
        let e = &games[0];
        assert_eq!(e.package.as_deref(), Some(Path::new("/g/base.nsp")));
        assert_eq!(e.keys.as_deref(), Some(Path::new(KEYS)));
        assert_eq!(e.title_id, "0100000000010000");
        assert_eq!(e.version, "1.0.0");
        assert_eq!(e.build_id, "bb01");
    }

    #[test]
    fn the_same_package_twice_is_one_game() {
        let mut games = vec![];
        let p = Path::new("/g/base.nsp");
        merge_package(
            &mut games,
            p,
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        let i = merge_package(
            &mut games,
            p,
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        assert_eq!((i, games.len()), (0, 1));
    }

    #[test]
    fn an_update_joins_its_base_game_and_its_code_is_what_runs() {
        let mut games = vec![];
        merge_package(
            &mut games,
            Path::new("/g/base.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        let i = merge_package(
            &mut games,
            Path::new("/g/upd.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010800", true), &[]),
        );
        assert_eq!((i, games.len()), (0, 1));
        let e = &games[0];
        assert_eq!(e.update.as_deref(), Some(Path::new("/g/upd.nsp")));
        assert_eq!(e.package.as_deref(), Some(Path::new("/g/base.nsp")));
        assert_eq!((e.version.as_str(), e.build_id.as_str()), ("1.2.0", "bb02"));
    }

    #[test]
    fn a_base_game_added_after_its_update_joins_it() {
        let mut games = vec![];
        merge_package(
            &mut games,
            Path::new("/g/upd.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010800", true), &[]),
        );
        let i = merge_package(
            &mut games,
            Path::new("/g/base.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        assert_eq!((i, games.len()), (0, 1));
        assert_eq!(games[0].package.as_deref(), Some(Path::new("/g/base.nsp")));
        assert_eq!(games[0].version, "1.2.0", "the update's version stays");
    }

    #[test]
    fn another_game_is_not_paired() {
        let mut games = vec![];
        merge_package(
            &mut games,
            Path::new("/g/a.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        merge_package(
            &mut games,
            Path::new("/g/b.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000020800", true), &[]),
        );
        assert_eq!(games.len(), 2);
    }

    #[test]
    fn a_game_joins_the_app_that_runs_it() {
        let mut games = vec![
            Entry {
                exe: Some("/apps/other".into()),
                ..Default::default()
            },
            Entry {
                exe: Some("/apps/demo".into()),
                ..Default::default()
            },
        ];
        let r = read(
            pkg("0100000000010000", false),
            &[
                ("/apps/other", "0100000000099000"),
                ("/apps/demo", "0100000000010000"),
            ],
        );
        let i = merge_package(&mut games, Path::new("/g/base.nsp"), Path::new(KEYS), r);
        assert_eq!((i, games.len()), (1, 2));
        assert_eq!(games[1].package.as_deref(), Some(Path::new("/g/base.nsp")));
        assert_eq!(games[1].exe.as_deref(), Some(Path::new("/apps/demo")));
    }

    #[test]
    fn a_game_added_meanwhile_is_kept() {
        // (read against the list as it was, merged into the list as it is: the race add_package had)
        let r = read(pkg("0100000000010000", false), &[]);
        let mut now = vec![Entry {
            exe: Some("/apps/new".into()),
            name: "Added meanwhile".into(),
            ..Default::default()
        }];
        merge_package(&mut now, Path::new("/g/base.nsp"), Path::new(KEYS), r);
        assert_eq!(now.len(), 2);
        assert_eq!(now[0].name, "Added meanwhile");
    }

    #[test]
    fn a_newer_update_replaces_the_older_one() {
        let mut games = vec![];
        merge_package(
            &mut games,
            Path::new("/g/base.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        merge_package(
            &mut games,
            Path::new("/g/upd1.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010800", true), &[]),
        );
        let mut newer = pkg("0100000000010800", true);
        newer.version = "1.3.0".into();
        let i = merge_package(
            &mut games,
            Path::new("/g/upd2.nsp"),
            Path::new(KEYS),
            read(newer, &[]),
        );
        assert_eq!(
            (i, games.len()),
            (0, 1),
            "one game, not two with the same key"
        );
        assert_eq!(games[0].update.as_deref(), Some(Path::new("/g/upd2.nsp")));
        assert_eq!(games[0].version, "1.3.0");
    }

    #[test]
    fn the_same_game_from_another_file_is_the_same_game() {
        let mut games = vec![];
        merge_package(
            &mut games,
            Path::new("/old/base.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        let i = merge_package(
            &mut games,
            Path::new("/new/base.xci"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        assert_eq!((i, games.len()), (0, 1));
        assert_eq!(
            games[0].package.as_deref(),
            Some(Path::new("/new/base.xci"))
        );
    }

    #[test]
    fn a_game_is_known_by_its_base_title_id() {
        // (an update added first: the game is still the base game - its app, its key, its icon)
        let mut games = vec![];
        merge_package(
            &mut games,
            Path::new("/g/upd.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010800", true), &[]),
        );
        assert_eq!(games[0].title_id, "0100000000010000");
        merge_package(
            &mut games,
            Path::new("/g/base.nsp"),
            Path::new(KEYS),
            read(pkg("0100000000010000", false), &[]),
        );
        assert_eq!(games[0].title_id, "0100000000010000");
    }

    #[test]
    fn an_update_joins_the_app_of_its_game() {
        let mut games = vec![Entry {
            exe: Some("/apps/demo".into()),
            ..Default::default()
        }];
        let r = read(
            pkg("0100000000010800", true),
            &[("/apps/demo", "0100000000010000")],
        );
        let i = merge_package(&mut games, Path::new("/g/upd.nsp"), Path::new(KEYS), r);
        assert_eq!((i, games.len()), (0, 1));
        assert_eq!(games[0].update.as_deref(), Some(Path::new("/g/upd.nsp")));
    }

    #[test]
    fn base_title_ids() {
        assert_eq!(base_title_id("0100000000010800"), "0100000000010000");
        assert_eq!(base_title_id("0100000000010000"), "0100000000010000");
        assert_eq!(base_title_id("01007ef00011e800"), "01007EF00011E000");
        assert_eq!(base_title_id("not hex"), "not hex");
        assert!(same_game("0100000000010800", "0100000000010000"));
        assert!(!same_game("", ""));
        assert!(!same_game("0100000000020000", "0100000000010000"));
    }

    #[test]
    fn packages_are_found_recursively_and_sorted() {
        let d = std::env::temp_dir().join(format!("nxrt-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::write(d.join("z.NSP"), b"x").unwrap();
        std::fs::write(d.join("a/b/game.xci"), b"x").unwrap();
        std::fs::write(d.join("a/readme.txt"), b"x").unwrap();
        std::fs::create_dir_all(d.join(".hidden")).unwrap();
        std::fs::write(d.join(".hidden/ignored.nsp"), b"x").unwrap();
        let got = find_packages(&d);
        assert_eq!(got, vec![d.join("a/b/game.xci"), d.join("z.NSP")]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
