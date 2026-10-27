//! The command line (scripts, tests): the same library and steps as the window, without opening it.
use crate::api;
use crate::issues;
use crate::library::{self, Library};
use crate::ops;
use crate::recomp;
use crate::runner::{self, Stage};
use std::path::PathBuf;
use std::time::Duration;

/// Handles a command-line switch; Some(exit code) when one was given, None to open the window.
pub fn run() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--keys") {
        let k = PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or(""));
        if let Err(e) = library::check_keys(&k) {
            eprintln!("{e}");
            return Some(1);
        }
        let mut lib = Library::load();
        lib.keys = Some(k.clone());
        if let Err(e) = lib.save() {
            eprintln!("{e}");
            return Some(1);
        }
        println!("keys: {}", k.display());
        return Some(0);
    }
    if let Some(i) = args.iter().position(|a| a == "--create") {
        return Some(create(&args, i));
    }
    if let Some(i) = args.iter().position(|a| a == "--issue") {
        let who = args.get(i + 1).map(String::as_str).unwrap_or("");
        return Some(issue(who));
    }
    if args.iter().any(|a| a == "--list") {
        list();
        return Some(0);
    }
    if let Some(i) = args.iter().position(|a| a == "--step") {
        let a = |k: usize| args.get(i + k).map(String::as_str);
        return Some(cli_step(a(1).unwrap_or(""), a(2).unwrap_or("all"), a(3)));
    }
    if let Some(i) = args.iter().position(|a| a == "--add") {
        return Some(add(&args, i));
    }
    None
}

/// `--issue <title id | app id | game key>`: the nxrt GitHub issue URL for a game, prefilled; printed, not opened
fn issue(who: &str) -> i32 {
    let lib = Library::load();
    let Some((index, e)) = lib.games.iter().enumerate().find(|(_, e)| {
        api::game_key(e) == who
            || e.title_id.eq_ignore_ascii_case(who)
            || e.exe.as_ref().is_some_and(|x| {
                runner::describe(x)
                    .is_ok_and(|d| d.id == who || d.title_id.eq_ignore_ascii_case(who))
            })
    }) else {
        eprintln!("{who}: not in the library");
        return 2;
    };
    let g = api::Gathered::gather(e);
    let v = api::view(index, e, &g, lib.keys.as_deref(), None, None);
    println!("{}", issues::for_game(&v, &recomp::default_nxrt()));
    0
}

/// `--list`: every game of the library and how it stands
fn list() {
    for e in Library::load().games {
        let g = api::Gathered::gather(&e);
        let v = api::view(0, &e, &g, None, None, None);
        println!("{} [{}] - {}", v.title, v.title_id, v.state);
        if let Some(p) = &v.package {
            println!("  package: {p}");
        }
        if let Some(app) = &v.describe {
            println!(
                "  app: {} v{} (code {})",
                app.title, app.version, app.gamelib_id
            );
        }
        if let Some(err) = &v.load_error {
            println!("  app: {err}");
        }
        if let Some(s) = &v.status {
            println!(
                "  steps: files {} generated {} compiled {} | data {} | game {}",
                v.steps.files,
                v.steps.generated,
                v.steps.code,
                s.data_root,
                v.game_dir.as_deref().unwrap_or("")
            );
        } else {
            // (nxnative: its steps are the build probe's, and its build is the work folder)
            println!(
                "  steps: files {} generated {} compiled {} | build {}",
                v.steps.files,
                v.steps.generated,
                v.steps.code,
                v.exe.as_deref().unwrap_or("")
            );
        }
        if v.backends.len() > 1 {
            let also: Vec<&str> = v
                .backends
                .iter()
                .map(String::as_str)
                .filter(|b| *b != v.backend)
                .collect();
            println!("  runs with: {} (also {})", v.backend, also.join(", "));
        }
        if let Some(r) = &v.romfs {
            println!(
                "  game data: {} files, {:.2} GB",
                r.files,
                r.bytes as f64 / 1e9
            );
        }
    }
}

/// `--step <title id or app id> <files|generate|compile|all> [src]`: a step, progress on stdout
fn cli_step(who: &str, stage: &str, src: Option<&str>) -> i32 {
    let lib = Library::load();
    let Some(e) = lib.games.iter().find(|e| {
        e.title_id.eq_ignore_ascii_case(who)
            || e.exe.as_ref().is_some_and(|x| {
                runner::describe(x)
                    .is_ok_and(|d| d.id == who || d.title_id.eq_ignore_ascii_case(who))
            })
    }) else {
        eprintln!("{who}: not in the library");
        return 2;
    };
    let Some(exe) = &e.exe else {
        eprintln!("{who}: no nxrt app for it");
        return 2;
    };
    let stage = match stage {
        "files" => Stage::Files,
        "generate" => Stage::Generate,
        "compile" => Stage::Compile,
        _ => Stage::All,
    };
    let src = src
        .map(str::to_string)
        .or_else(|| e.package.as_ref().map(|p| p.display().to_string()))
        .unwrap_or_default();
    let code_id = runner::status(exe, e.data_root.as_deref())
        .map(|s| s.code_id)
        .unwrap_or_default();
    let keys = lib
        .keys
        .clone()
        .filter(|k| k.is_file())
        .or_else(|| e.keys.clone());
    let mut r = match runner::Run::setup(
        exe,
        e.data_root.as_deref(),
        stage,
        &src,
        keys.as_deref(),
        e.update.as_deref(),
        None,
        || {},
    ) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("{err}");
            return 2;
        }
    };
    let mut last = String::new();
    let how = loop {
        if let Some(how) = r.exited() {
            break how;
        }
        let p = r.progress.lock().unwrap().clone();
        let line = match p.fraction {
            Some(f) => format!("{:5.1}% {}", f * 100.0, p.step),
            None => p.step.clone(),
        };
        if line != last {
            println!("{line}");
            last = line;
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    let res = r.progress.lock().unwrap().result.clone();
    let ok = res.as_ref().is_some_and(|r| r.ok);
    if ok && stage == Stage::Generate {
        // (recorded as the window does: Compile is next)
        let mut lib = Library::load();
        if let Some(x) = lib
            .games
            .iter_mut()
            .find(|x| x.exe == e.exe && x.package == e.package)
        {
            x.generated_for = runner::status(exe, e.data_root.as_deref())
                .map(|s| s.code_id)
                .unwrap_or(code_id);
            let _ = lib.save();
        }
    }
    match res {
        Some(r) if r.ok => println!("done ({how})"),
        Some(r) => println!("failed: {} {} ({how})", r.step, r.hint),
        None => println!("no result ({how})"),
    }
    if ok {
        0
    } else {
        1
    }
}

/// `--add <package | app> [--data-root <dir>]`
fn add(args: &[String], i: usize) -> i32 {
    let what = PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or(""));
    let dr = args
        .iter()
        .position(|a| a == "--data-root")
        .and_then(|j| args.get(j + 1))
        .map(PathBuf::from);
    let mut lib = Library::load();
    let is_pkg = what
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("nsp") || e.eq_ignore_ascii_case("xci"));
    let msg = if is_pkg {
        let Some(keys) = ops::keys_for(lib.keys.as_deref(), &what) else {
            eprintln!("no prod.keys beside the package or in ~/.switch");
            return 1;
        };
        let mut games = lib.games.clone();
        match ops::add_package_to(&mut games, &what, &keys) {
            Ok(i) => {
                lib.games = games;
                let e = &lib.games[i];
                format!(
                    "{} [{}]: added{}",
                    e.name,
                    e.title_id,
                    if e.exe.is_some() {
                        ", with its app"
                    } else {
                        " (no nxrt app for it yet)"
                    }
                )
            }
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    } else {
        let exe = ops::resolve_app(&what);
        match runner::describe(&exe) {
            Ok(d) => {
                if let Some(g) = lib
                    .games
                    .iter_mut()
                    .find(|g| g.exe.is_none() && ops::same_game(&g.title_id, &d.title_id))
                {
                    g.exe = Some(exe);
                    g.data_root = dr;
                    format!("{}: given to {}", d.title, g.name)
                } else {
                    format!(
                        "{}: {}",
                        d.title,
                        if lib.add_app(&exe, dr) {
                            "added"
                        } else {
                            "already in the list"
                        }
                    )
                }
            }
            Err(e) => {
                eprintln!("{e}");
                return 1;
            }
        }
    };
    if let Err(e) = lib.save() {
        eprintln!("{e}");
        return 1;
    }
    println!("{msg}");
    0
}

/// `--create <title id> [--id <id>] [--dir <folder>]`: a new recomp, its app built
fn create(args: &[String], i: usize) -> i32 {
    let who = args.get(i + 1).cloned().unwrap_or_default();
    let opt = |k: &str| {
        args.iter()
            .position(|a| a == k)
            .and_then(|j| args.get(j + 1))
            .cloned()
    };
    let mut lib = Library::load();
    let Some(gi) = lib
        .games
        .iter()
        .position(|e| e.title_id.eq_ignore_ascii_case(&who))
    else {
        eprintln!("{who}: not in the library");
        return 2;
    };
    let e = lib.games[gi].clone();
    let id = opt("--id").unwrap_or_else(|| recomp::default_id(&e.name));
    let dir = opt("--dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| recomp::default_dir(&id));
    let keys = lib
        .keys
        .clone()
        .filter(|k| k.is_file())
        .or(e.keys.clone())
        .unwrap_or_default();
    if let Err(err) = ops::create_recomp(&e, &id, &dir, &recomp::default_nxrt(), &keys) {
        eprintln!("{err}");
        return 1;
    }
    println!(
        "{}: recomp at {} (modules in its Makefile); building its app...",
        e.name,
        dir.display()
    );
    let mut r = runner::Run::create(&dir, None, || {}).unwrap_or_else(|err| {
        eprintln!("{err}");
        std::process::exit(1)
    });
    let mut last = String::new();
    let how = loop {
        if let Some(how) = r.exited() {
            break how;
        }
        let st = r.progress.lock().unwrap().step.clone();
        if st != last {
            println!("{st}");
            last = st;
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    let app = recomp::app_path(&dir, &id);
    if how != "closed" || !app.is_file() {
        for l in r
            .log
            .lock()
            .unwrap()
            .iter()
            .rev()
            .take(15)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            eprintln!("  {l}");
        }
        eprintln!("the build {how}");
        return 1;
    }
    let g = &mut lib.games[gi];
    g.exe = Some(app.clone());
    g.data_root = Some(dir.join("data"));
    g.recomp_dir = Some(dir);
    if let Err(err) = lib.save() {
        eprintln!("{err}");
        return 1;
    }
    println!(
        "app: {} - next: --step {who} generate, then compile",
        app.display()
    );
    0
}
