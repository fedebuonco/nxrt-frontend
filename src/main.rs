//! nxrt-frontend: one manager for the nxrt Switch recomps. A library of games (each one its own app, an nxrt runner),
//! how each game's files and game code stand, Play with the log alongside.
//!
//! Who a game is and whether its game code is current come from its app (`--describe`, `--status`); its files from
//! libnxrt_manage (manage.rs). The manager never edits a game's settings or saves: the game's app owns them.
mod library;
mod manage;
mod runner;

use eframe::egui::{self, Color32, RichText};
use library::{Entry, Library};
use runner::{Describe, Run, Status};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What the manager knows about one game, gathered off the UI thread.
#[derive(Clone, Default)]
struct Info {
    describe: Option<Result<Describe, String>>,
    status: Option<Result<Status, String>>,
    files: Option<manage::GameFiles>,
    romfs: Option<Result<manage::RomfsSummary, String>>,
}

impl Info {
    fn gather(e: &Entry) -> Info {
        let describe = Some(runner::describe(&e.exe));
        let status = Some(runner::status(&e.exe, e.data_root.as_deref()));
        let dir = status.as_ref().and_then(|s| s.as_ref().ok()).map(game_dir);
        let files = dir.as_deref().and_then(manage::game_files);
        let romfs = dir.as_deref().filter(|_| files.is_some()).map(manage::romfs_summary);
        Info { describe, status, files, romfs }
    }
}

/// The game folder --status names, absolute (a relative one is in the data root: "extracted").
fn game_dir(s: &Status) -> PathBuf {
    let g = PathBuf::from(&s.game_dir);
    if g.is_absolute() { g } else { PathBuf::from(&s.data_root).join(g) }
}

struct Game {
    entry: Entry,
    info: Arc<Mutex<Option<Info>>>,
    loading: Arc<Mutex<bool>>,
    run: Option<Run>,
    last_exit: Option<String>,
}

impl Game {
    fn new(entry: Entry) -> Game {
        Game { entry, info: Default::default(), loading: Default::default(), run: None, last_exit: None }
    }

    fn refresh(&self, ctx: &egui::Context) {
        let mut busy = self.loading.lock().unwrap();
        if *busy {
            return;
        }
        *busy = true;
        let (entry, info, loading, ctx) = (self.entry.clone(), self.info.clone(), self.loading.clone(), ctx.clone());
        std::thread::spawn(move || {
            let i = Info::gather(&entry);
            *info.lock().unwrap() = Some(i);
            *loading.lock().unwrap() = false;
            ctx.request_repaint();
        });
    }

    fn info(&self) -> Option<Info> {
        self.info.lock().unwrap().clone()
    }

    fn title(&self) -> String {
        match self.info().and_then(|i| i.describe).and_then(|d| d.ok()) {
            Some(d) => d.title,
            None => self.entry.exe.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }

    fn running(&self) -> bool {
        self.run.is_some()
    }
}

#[derive(Default)]
struct AddDialog {
    open: bool,
    exe: String,
    data_root: String,
    error: String,
}

struct App {
    library: Library,
    games: Vec<Game>,
    selected: usize,
    add: AddDialog,
    message: String,
}

impl App {
    fn new(cc: &eframe::CreationContext) -> App {
        let library = Library::load();
        let games: Vec<Game> = library.games.iter().cloned().map(Game::new).collect();
        for g in &games {
            g.refresh(&cc.egui_ctx);
        }
        let message = if manage::abi_ok() { String::new() } else { "libnxrt_manage: another ABI than this app's".into() };
        App { library, games, selected: 0, add: AddDialog::default(), message }
    }

    fn save(&mut self) {
        self.library.games = self.games.iter().map(|g| g.entry.clone()).collect();
        if let Err(e) = self.library.save() {
            self.message = format!("Couldn't save the library: {e}");
        }
    }

    fn play(&mut self, i: usize, play: bool, ctx: &egui::Context) {
        let g = &mut self.games[i];
        let id = g.info().and_then(|i| i.describe).and_then(|d| d.ok()).map(|d| d.id).unwrap_or_else(|| "game".into());
        let logs = library::dir().join("logs");
        let _ = std::fs::create_dir_all(&logs);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let log_file = logs.join(format!("{id}-{stamp}.log"));
        let c = ctx.clone();
        match Run::start(&g.entry.exe, g.entry.data_root.as_deref(), play, Some(log_file), move || c.request_repaint()) {
            Ok(r) => {
                g.run = Some(r);
                g.last_exit = None;
            }
            Err(e) => self.message = e,
        }
    }

    fn poll_runs(&mut self, ctx: &egui::Context) {
        for g in &mut self.games {
            if let Some(r) = &mut g.run {
                if let Some(how) = r.exited() {
                    g.last_exit = Some(how);
                    g.refresh(ctx);  // (it may have set its files up or built its game code)
                    // the run's log stays readable until the next one
                    g.run = None;
                    ctx.request_repaint();
                }
            }
        }
        if self.games.iter().any(|g| g.running()) {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
    }
}

fn status_line(info: &Option<Info>) -> (String, Color32) {
    let Some(i) = info else { return ("Checking...".into(), Color32::GRAY) };
    match &i.status {
        Some(Ok(s)) if s.ready => ("Ready to play".into(), Color32::from_rgb(90, 200, 120)),
        Some(Ok(s)) => (if s.status.is_empty() { "Not ready".into() } else { s.status.clone() }, Color32::from_rgb(240, 170, 60)),
        Some(Err(e)) => (e.clone(), Color32::from_rgb(230, 90, 90)),
        None => ("Checking...".into(), Color32::GRAY),
    }
}

fn reveal(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(p).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(p).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("explorer").arg(p).spawn();
}

fn row(ui: &mut egui::Ui, k: &str, v: impl Into<egui::WidgetText>) {
    ui.label(RichText::new(k).color(Color32::GRAY));
    ui.label(v);
    ui.end_row();
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_runs(ctx);

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("nxrt");
                ui.separator();
                if ui.button("Add game...").clicked() {
                    self.add = AddDialog { open: true, ..Default::default() };
                }
                if ui.button("Refresh").clicked() {
                    for g in &self.games {
                        g.refresh(ctx);
                    }
                }
                if !self.message.is_empty() {
                    ui.separator();
                    ui.colored_label(Color32::from_rgb(240, 170, 60), &self.message);
                    if ui.small_button("x").clicked() {
                        self.message.clear();
                    }
                }
            });
        });

        egui::SidePanel::left("games").resizable(true).default_width(240.0).show(ctx, |ui| {
            ui.add_space(6.0);
            if self.games.is_empty() {
                ui.label("No games yet. Add a game's app (fe3hrecomp, eowrecomp ...).");
            }
            for (i, g) in self.games.iter().enumerate() {
                let (line, col) = status_line(&g.info());
                let dot = if g.running() { RichText::new("●").color(Color32::from_rgb(90, 160, 240)) } else { RichText::new("●").color(col) };
                ui.horizontal(|ui| {
                    ui.label(dot);
                    let r = ui.selectable_label(self.selected == i, RichText::new(g.title()).strong());
                    if r.clicked() {
                        self.selected = i;
                    }
                    r.on_hover_text(line);
                });
            }
        });

        let mut remove = None;
        let mut start: Option<bool> = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(g) = self.games.get_mut(self.selected) else {
                ui.centered_and_justified(|ui| ui.label("Add a game to start."));
                return;
            };
            let info = g.info();
            let d = info.as_ref().and_then(|i| i.describe.clone()).and_then(|d| d.ok());
            let s = info.as_ref().and_then(|i| i.status.clone()).and_then(|s| s.ok());
            ui.add_space(4.0);
            ui.heading(RichText::new(d.as_ref().map(|d| d.headline.clone()).unwrap_or_else(|| g.title())).size(26.0));
            if let Some(d) = &d {
                ui.label(format!("{} - v{}", d.game_name, d.game_version));
            }
            let (line, col) = status_line(&info);
            ui.colored_label(col, line);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ready = s.as_ref().is_some_and(|s| s.ready);
                if g.running() {
                    if ui.button(RichText::new("Stop").size(18.0)).clicked() {
                        if let Some(r) = &mut g.run {
                            r.stop();
                        }
                    }
                } else {
                    if ui.add_enabled(ready, egui::Button::new(RichText::new("  Play  ").size(18.0))).clicked() {
                        start = Some(true);
                    }
                    let t = if ready { "Start screen" } else { "Set up in the game's app" };
                    if ui.button(t).on_hover_text("The game's own start screen: its settings and game-files setup").clicked() {
                        start = Some(false);
                    }
                }
                if let Some(s) = &s {
                    if ui.button("Data folder").clicked() {
                        reveal(Path::new(&s.data_root));
                    }
                    if ui.button("Game folder").clicked() {
                        reveal(&game_dir(s));
                    }
                }
                if !g.running() && ui.button("Remove").on_hover_text("From this list only: nothing is deleted").clicked() {
                    remove = Some(self.selected);
                }
            });
            if let Some(how) = &g.last_exit {
                ui.label(RichText::new(format!("Last run: {how}")).color(Color32::GRAY));
            }
            ui.add_space(8.0);

            egui::CollapsingHeader::new("Details").default_open(true).show(ui, |ui| {
                egui::Grid::new("details").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                    row(ui, "App", g.entry.exe.display().to_string());
                    if let Some(d) = &d {
                        row(ui, "Version", d.version.clone());
                        row(ui, "Title id", d.title_id.clone());
                        row(ui, "Game build", d.build_id.clone());
                        row(ui, "Game code id", d.gamelib_id.clone());
                    }
                    if let Some(Err(e)) = info.as_ref().and_then(|i| i.describe.clone()) {
                        row(ui, "App", RichText::new(e).color(Color32::from_rgb(230, 90, 90)));
                    }
                    if let Some(s) = &s {
                        row(ui, "Data folder", s.data_root.clone());
                        row(ui, "Game folder", game_dir(s).display().to_string());
                        row(ui, "Game files", if s.files_ready { "ready".into() } else { s.files.clone() });
                        row(ui, "Game code", if s.code_ready { "built for this version".to_string() } else { "not built for this version".into() });
                        row(ui, "Wants game code", s.code_id.clone());
                        if !s.code_ready {
                            row(ui, "", if s.builder { "the app builds it: Start screen > Setup game files" } else { "a dev build: make gen + the game library (no builder bundled)" });
                        }
                    }
                    if let Some(f) = info.as_ref().and_then(|i| i.files.clone()) {
                        let kind = if !f.exefs { "exefs/main missing" } else if f.romfs_package { "read from the game package in place" } else if f.romfs_dir { "extracted folder" } else { "RomFS missing" };
                        row(ui, "Game data", kind);
                        if let (Some(b), Some(d)) = (&f.build_id, &d) {
                            let ok = b == &d.build_id;
                            row(ui, "Files' build", RichText::new(format!("{b}{}", if ok { "" } else { "  (not the version this app is for)" }))
                                .color(if ok { Color32::GRAY } else { Color32::from_rgb(230, 90, 90) }));
                        }
                    }
                    if let Some(r) = info.as_ref().and_then(|i| i.romfs.clone()) {
                        match r {
                            Ok(r) => row(ui, "RomFS", format!("{} files, {:.1} GB", r.files, r.bytes as f64 / 1e9)),
                            Err(e) => row(ui, "RomFS", RichText::new(e).color(Color32::from_rgb(230, 90, 90))),
                        }
                    }
                    if *g.loading.lock().unwrap() {
                        row(ui, "", "checking...");
                    }
                });
            });

            ui.add_space(6.0);
            let log = g.run.as_ref().map(|r| (r.log.clone(), r.log_file.clone()));
            egui::CollapsingHeader::new("Log").default_open(true).show(ui, |ui| {
                match log {
                    None => {
                        ui.label(RichText::new("Shown while the game runs (each run's log is kept in the logs folder).").color(Color32::GRAY));
                        if ui.button("Logs folder").clicked() {
                            let p = library::dir().join("logs");
                            let _ = std::fs::create_dir_all(&p);
                            reveal(&p);
                        }
                    }
                    Some((log, file)) => {
                        if let Some(f) = file {
                            ui.label(RichText::new(f.display().to_string()).color(Color32::GRAY).small());
                        }
                        let lines = log.lock().unwrap();
                        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show_rows(
                            ui,
                            14.0,
                            lines.len(),
                            |ui, range| {
                                for l in lines.range(range) {
                                    ui.label(RichText::new(l).monospace().size(11.0));
                                }
                            },
                        );
                    }
                }
            });
        });

        if let Some(p) = start {
            self.play(self.selected, p, ctx);
        }
        if let Some(i) = remove {
            self.games.remove(i);
            self.selected = self.selected.min(self.games.len().saturating_sub(1));
            self.save();
        }

        self.add_dialog(ctx);
    }
}

impl App {
    fn add_dialog(&mut self, ctx: &egui::Context) {
        if !self.add.open {
            return;
        }
        let mut open = true;
        let mut done = false;
        egui::Window::new("Add a game").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label("The game's app: an nxrt recomp (fe3hrecomp, eowrecomp, lasrecomp ...).");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.add.exe).desired_width(380.0).hint_text("path to the app"));
                if ui.button("Browse...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("The game's app").pick_file() {
                        self.add.exe = p.display().to_string();
                    }
                }
            });
            ui.add_space(4.0);
            ui.label("Its data folder (settings, saves): leave empty for the app's own.");
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.add.data_root).desired_width(380.0).hint_text("default"));
                if ui.button("Browse...").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Data folder").pick_folder() {
                        self.add.data_root = p.display().to_string();
                    }
                }
            });
            if !self.add.error.is_empty() {
                ui.colored_label(Color32::from_rgb(230, 90, 90), &self.add.error);
            }
            ui.add_space(6.0);
            if ui.button("Add").clicked() {
                let exe = resolve_app(Path::new(self.add.exe.trim()));
                match runner::describe(&exe) {
                    Err(e) => self.add.error = e,
                    Ok(_) => {
                        let dr = self.add.data_root.trim();
                        let entry = Entry { exe: exe.clone(), data_root: (!dr.is_empty()).then(|| PathBuf::from(dr)) };
                        if self.games.iter().any(|g| g.entry.exe == exe) {
                            self.add.error = "Already in the list".into();
                        } else {
                            let g = Game::new(entry);
                            g.refresh(ctx);
                            self.games.push(g);
                            self.selected = self.games.len() - 1;
                            self.save();
                            done = true;
                        }
                    }
                }
            }
        });
        if !open || done {
            self.add.open = false;
        }
    }
}

/// A macOS .app bundle picked: its executable inside.
fn resolve_app(p: &Path) -> PathBuf {
    if p.extension().is_some_and(|e| e == "app") {
        if let Ok(rd) = std::fs::read_dir(p.join("Contents/MacOS")) {
            if let Some(e) = rd.flatten().find(|e| e.path().is_file()) {
                return e.path();
            }
        }
    }
    p.to_path_buf()
}

/// `--list`: every game of the library and how it stands, as the window would show it (scripts, tests)
fn list() {
    for e in Library::load().games {
        let i = Info::gather(&e);
        let name = match &i.describe {
            Some(Ok(d)) => format!("{} ({} v{}, code {})", d.title, d.game_name, d.game_version, d.gamelib_id),
            Some(Err(err)) => format!("{}: {err}", e.exe.display()),
            None => e.exe.display().to_string(),
        };
        println!("{name}");
        match &i.status {
            Some(Ok(s)) => println!("  {} | data {} | game {}", if s.ready { "ready" } else { s.status.as_str() }, s.data_root, game_dir(s).display()),
            Some(Err(err)) => println!("  status: {err}"),
            None => {}
        }
        if let Some(f) = &i.files {
            println!("  files: exefs {} romfs dir {} package {} build {}", f.exefs, f.romfs_dir, f.romfs_package, f.build_id.clone().unwrap_or_default());
        }
        match &i.romfs {
            Some(Ok(r)) => println!("  romfs: {} files, {:.2} GB", r.files, r.bytes as f64 / 1e9),
            Some(Err(err)) => println!("  romfs: {err}"),
            None => {}
        }
    }
}

fn main() -> eframe::Result<()> {
    // nxrt-frontend --add <exe> [--data-root <dir>]: add a game from the command line and exit (scripts, tests)
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--list") {
        list();
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--add") {
        let exe = resolve_app(Path::new(args.get(i + 1).map(String::as_str).unwrap_or("")));
        let dr = args.iter().position(|a| a == "--data-root").and_then(|j| args.get(j + 1)).map(PathBuf::from);
        match runner::describe(&exe) {
            Ok(d) => {
                let mut lib = Library::load();
                let added = lib.add(&exe, dr);
                if let Err(e) = lib.save() {
                    eprintln!("{e}");
                    std::process::exit(1);
                }
                println!("{}: {}", d.title, if added { "added" } else { "already in the list" });
                return Ok(());
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("nxrt").with_inner_size([1000.0, 680.0]).with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native("nxrt-frontend", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
