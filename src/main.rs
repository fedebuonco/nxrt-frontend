//! nxrt-frontend: one manager for the nxrt Switch recomps. The player adds a Switch game (its .nsp / .xci): the
//! manager shows what it is (name, icon, version, from the package itself), finds the nxrt app that runs it, and walks
//! through the steps to play it - the game files set up, the game code generated (its ARM code translated to C), then
//! compiled - asking before each long one. Then Play, with the log alongside.
//!
//! Who an app's game is and how its files and game code stand come from the app itself (`--describe`, `--status`);
//! the steps are run by the app (`--setup-files`, `--generate`, `--compile`); packages and game folders are read with
//! libnxrt_manage (manage.rs). The manager never edits a game's settings or saves: the game's app owns them.
mod library;
mod manage;
mod runner;

use eframe::egui::{self, Color32, RichText};
use library::{Entry, Library};
use runner::{Describe, Kind, Run, Stage, Status};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const GREEN: Color32 = Color32::from_rgb(90, 200, 120);
const AMBER: Color32 = Color32::from_rgb(240, 170, 60);
const RED: Color32 = Color32::from_rgb(230, 90, 90);
const BLUE: Color32 = Color32::from_rgb(90, 160, 240);

/// What the manager knows about one game's app and files, gathered off the UI thread.
#[derive(Clone, Default)]
struct Info {
    describe: Option<Result<Describe, String>>,
    status: Option<Result<Status, String>>,
    files: Option<manage::GameFiles>,
    romfs: Option<Result<manage::RomfsSummary, String>>,
}

impl Info {
    fn gather(e: &Entry) -> Info {
        let Some(exe) = &e.exe else { return Info::default() };
        let describe = Some(runner::describe(exe));
        let status = Some(runner::status(exe, e.data_root.as_deref()));
        let dir = status.as_ref().and_then(|s| s.as_ref().ok()).map(game_dir);
        let files = dir.as_deref().and_then(manage::game_files);
        let romfs = dir.as_deref().filter(|_| files.as_ref().is_some_and(|f| f.exefs)).map(manage::romfs_summary);
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
    /// the game-code id a running Generate is for (its result is recorded against it)
    run_code_id: String,
    last_exit: Option<(String, bool)>,
    edit_dir: String,
    edit_cmd: String,
}

impl Game {
    fn new(entry: Entry) -> Game {
        let edit_dir = entry.build_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let edit_cmd = entry.build_cmd.clone().unwrap_or_default();
        Game { entry, info: Default::default(), loading: Default::default(), run: None, run_code_id: String::new(), last_exit: None, edit_dir, edit_cmd }
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

    fn describe(&self) -> Option<Describe> {
        self.info().and_then(|i| i.describe).and_then(|d| d.ok())
    }

    fn status(&self) -> Option<Status> {
        self.info().and_then(|i| i.status).and_then(|s| s.ok())
    }

    /// The game's name: from its package, else its app
    fn title(&self) -> String {
        if !self.entry.name.is_empty() {
            return self.entry.name.clone();
        }
        match self.describe() {
            Some(d) => d.game_name,
            None => self.entry.exe.as_ref().and_then(|e| e.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
        }
    }

    fn title_id(&self) -> String {
        if !self.entry.title_id.is_empty() { self.entry.title_id.clone() } else { self.describe().map(|d| d.title_id).unwrap_or_default() }
    }

    fn running(&self) -> bool {
        self.run.is_some()
    }

    fn steps(&self) -> Steps {
        let s = self.status();
        let files = s.as_ref().is_some_and(|s| s.files_ready);
        let code = s.as_ref().is_some_and(|s| s.code_ready);
        let generated = code || s.as_ref().is_some_and(|s| !s.code_id.is_empty() && s.code_id == self.entry.generated_for);
        let builder = s.as_ref().is_some_and(|s| s.builder);
        let dev = self.entry.build_dir.is_some() && self.entry.build_cmd.is_some();
        Steps { files, generated, code, builder, dev }
    }
}

#[derive(Clone, Copy)]
struct Steps {
    files: bool,
    generated: bool,
    code: bool,
    builder: bool,
    dev: bool,
}

/// What a click asked for (done after the panels are drawn).
enum Action {
    Start(usize, Job),
    Remove(usize),
    SaveDev(usize),
    ChooseApp(usize),
}

enum Job {
    Play,
    StartScreen,
    Step(Stage, String),
    DevBuild,
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
    icons: HashMap<String, Option<egui::TextureHandle>>,
}

/// The keys for a package: the toolbar's, else prod.keys beside it or in ~/.switch.
fn keys_for(chosen: Option<&Path>, package: &Path) -> Option<PathBuf> {
    chosen.filter(|k| k.is_file()).map(Path::to_path_buf).or_else(|| manage::find_keys(package))
}

impl App {
    fn new(cc: &eframe::CreationContext) -> App {
        let library = Library::load();
        let games: Vec<Game> = library.games.iter().cloned().map(Game::new).collect();
        for g in &games {
            g.refresh(&cc.egui_ctx);
        }
        let message = if manage::abi_ok() { String::new() } else { "libnxrt_manage: another ABI than this app's".into() };
        App { library, games, selected: 0, add: AddDialog::default(), message, icons: HashMap::new() }
    }

    fn pick_keys(&mut self) {
        let Some(p) = rfd::FileDialog::new().set_title("Your Switch's keys file (prod.keys)").add_filter("Keys", &["keys"]).pick_file() else { return };
        match library::check_keys(&p) {
            Ok(()) => {
                self.library.keys = Some(p);
                self.save();
            }
            Err(e) => self.message = e,
        }
    }

    fn save(&mut self) {
        self.library.games = self.games.iter().map(|g| g.entry.clone()).collect();
        if let Err(e) = self.library.save() {
            self.message = format!("Couldn't save the library: {e}");
        }
    }

    fn icon(&mut self, ctx: &egui::Context, title_id: &str) -> Option<egui::TextureHandle> {
        if title_id.is_empty() {
            return None;
        }
        self.icons
            .entry(title_id.to_string())
            .or_insert_with(|| {
                let bytes = std::fs::read(library::icon_path(title_id)).ok()?;
                let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
                let size = [img.width() as usize, img.height() as usize];
                let ci = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
                Some(ctx.load_texture(format!("icon-{title_id}"), ci, egui::TextureOptions::LINEAR))
            })
            .clone()
    }

    /// Add a Switch game from its package: what it is (its control data), and the nxrt app in the library that runs it.
    fn add_package(&mut self, ctx: &egui::Context, package: PathBuf) {
        if keys_for(self.library.keys.as_deref(), &package).is_none() {
            self.pick_keys();  // (none set, none beside the package: asked for once, then kept in the toolbar)
        }
        let keys = keys_for(self.library.keys.as_deref(), &package);
        let Some(keys) = keys else {
            self.message = "A game package is read with the keys from your own Switch (prod.keys): none chosen".into();
            return;
        };
        match add_package_to(&mut self.games, &package, &keys) {
            Ok(i) => {
                self.games[i].refresh(ctx);
                self.selected = i;
                self.icons.remove(&self.games[i].entry.title_id);
                self.save();
            }
            Err(e) => self.message = e,
        }
    }

    fn start(&mut self, i: usize, job: Job, ctx: &egui::Context) {
        let g = &mut self.games[i];
        let Some(exe) = g.entry.exe.clone() else { return };
        let id = g.describe().map(|d| d.id).unwrap_or_else(|| "game".into());
        let logs = library::dir().join("logs");
        let _ = std::fs::create_dir_all(&logs);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let what = match &job {
            Job::Play | Job::StartScreen => "",
            Job::Step(Stage::Files, _) => "-files",
            Job::Step(Stage::Generate, _) => "-generate",
            Job::Step(Stage::Compile, _) => "-compile",
            Job::Step(Stage::All, _) | Job::DevBuild => "-build",
        };
        let log_file = Some(logs.join(format!("{id}{what}-{stamp}.log")));
        let c = ctx.clone();
        let repaint = move || c.request_repaint();
        let dr = g.entry.data_root.clone();
        let keys = self.library.keys.clone().filter(|k| k.is_file()).or_else(|| g.entry.keys.clone());
        g.run_code_id = g.status().map(|s| s.code_id).unwrap_or_default();
        let r = match job {
            Job::Play => Run::game(&exe, dr.as_deref(), true, log_file, repaint),
            Job::StartScreen => Run::game(&exe, dr.as_deref(), false, log_file, repaint),
            Job::Step(stage, src) => Run::setup(&exe, dr.as_deref(), stage, &src, keys.as_deref(), log_file, repaint),
            Job::DevBuild => match (&g.entry.build_dir, &g.entry.build_cmd) {
                (Some(d), Some(cmd)) => Run::dev_build(cmd, d, log_file, repaint),
                _ => Err("No Dev build command set".into()),
            },
        };
        match r {
            Ok(r) => {
                g.run = Some(r);
                g.last_exit = None;
            }
            Err(e) => self.message = e,
        }
    }

    fn poll_runs(&mut self, ctx: &egui::Context) {
        let mut changed = false;
        for g in &mut self.games {
            let Some(r) = &mut g.run else { continue };
            let Some(how) = r.exited() else { continue };
            let result = r.progress.lock().unwrap().result.clone();
            let (text, ok) = match (r.kind, result) {
                (Kind::Setup(stage), Some(res)) if res.ok => {
                    if stage == Stage::Generate && !g.run_code_id.is_empty() {
                        g.entry.generated_for = g.run_code_id.clone();
                        changed = true;
                    }
                    (match stage {
                        Stage::Files => "Game files set up".into(),
                        Stage::Generate => "Game code generated".into(),
                        Stage::Compile | Stage::All => "Game code compiled: ready to play".into(),
                    }, true)
                }
                (Kind::Setup(_), Some(res)) => {
                    let what = if res.step.is_empty() { res.status.clone() } else { res.step.clone() };
                    let hint = if !res.builder { " - this app has no game-code builder: set its Dev build command".to_string() }
                        else if res.hint.is_empty() { String::new() } else { format!(" - {}", res.hint) };
                    (format!("{what}{hint}"), false)
                }
                (Kind::Setup(_), None) => (format!("The step {how}"), false),
                (Kind::DevBuild, _) => (format!("Dev build {}", if how == "closed" { "finished" } else { how.as_str() }), how == "closed"),
                (Kind::Game, _) => (format!("Last run: {how}"), how == "closed"),
            };
            g.last_exit = Some((text, ok));
            g.run = None;  // (its log file stays in the logs folder)
            g.refresh(ctx);
            ctx.request_repaint();
        }
        if changed {
            self.save();
        }
        if self.games.iter().any(|g| g.running()) {
            ctx.request_repaint_after(Duration::from_millis(400));
        }
    }
}

/// The package's game into the list: its control data read, its icon kept, the app in the list that runs it (same
/// title id) given it. Returns the game's index.
fn add_package_to(games: &mut Vec<Game>, package: &Path, keys: &Path) -> Result<usize, String> {
    let p = manage::package_info(package, keys).map_err(|e| format!("{}: {e}", package.display()))?;
    if let Some(icon) = &p.icon {
        let path = library::icon_path(&p.title_id);
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(&path, icon);
    }
    if let Some(i) = games.iter().position(|g| g.entry.package.as_deref() == Some(package)) {
        return Ok(i);  // (already here)
    }
    // the game's app: an entry whose app runs this title (and has no package yet)
    let app = games.iter().position(|g| {
        g.entry.package.is_none()
            && g.entry.exe.as_ref().is_some_and(|e| runner::describe(e).is_ok_and(|d| d.title_id.eq_ignore_ascii_case(&p.title_id)))
    });
    let i = match app {
        Some(i) => i,
        None => {
            games.push(Game::new(Entry::default()));
            games.len() - 1
        }
    };
    let e = &mut games[i].entry;
    e.package = Some(package.to_path_buf());
    e.keys = Some(keys.to_path_buf());
    e.title_id = p.title_id;
    e.name = p.name;
    e.publisher = p.publisher;
    e.version = p.version;
    e.build_id = p.build_id.unwrap_or_default();
    Ok(i)
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

/// The game's state in a few words, and its colour (the list's dot)
fn summary(g: &Game) -> (String, Color32) {
    if g.running() {
        return (if g.run.as_ref().is_some_and(|r| r.kind == Kind::Game) { "Playing" } else { "Working..." }.into(), BLUE);
    }
    if g.entry.exe.is_none() {
        return ("No nxrt app for this game yet".into(), Color32::GRAY);
    }
    match g.info().and_then(|i| i.status) {
        None => ("Checking...".into(), Color32::GRAY),
        Some(Err(e)) => (e, RED),
        Some(Ok(s)) if s.ready => ("Ready to play".into(), GREEN),
        Some(Ok(_)) => {
            let st = g.steps();
            (if !st.files { "Next: set up the game files" } else if !st.generated { "Next: generate the game code" } else { "Next: compile the game code" }.into(), AMBER)
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_runs(ctx);
        let mut action: Option<Action> = None;

        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("nxrt");
                ui.separator();
                if ui.button("Add a Switch game...").on_hover_text("Its .nsp or .xci (with your prod.keys beside it)").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("A Switch game (.nsp / .xci)").add_filter("Switch game", &["nsp", "xci"]).pick_file() {
                        self.add_package(ctx, p);
                    }
                }
                if ui.button("Add an nxrt app...").on_hover_text("A recomp's app: fe3hrecomp, eowrecomp ...").clicked() {
                    self.add = AddDialog { open: true, ..Default::default() };
                }
                if ui.button("Refresh").clicked() {
                    for g in &self.games {
                        g.refresh(ctx);
                    }
                }
                ui.separator();
                // the keys games are read with: the one chosen here, else each package's own (beside it, ~/.switch)
                let home_keys = dirs::home_dir().map(|h| h.join(".switch/prod.keys")).filter(|p| p.is_file());
                let (label, col, tip) = match (&self.library.keys, &home_keys) {
                    (Some(k), _) if k.is_file() => (k.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), GREEN, k.display().to_string()),
                    (Some(k), _) => ("missing".to_string(), RED, format!("{} is gone: choose it again", k.display())),
                    (None, Some(h)) => ("~/.switch/prod.keys".into(), Color32::GRAY, format!("{} (none chosen: also prod.keys beside each package)", h.display())),
                    (None, None) => ("beside each package".into(), AMBER, "No keys chosen: prod.keys beside each package is used".into()),
                };
                ui.label("Keys:");
                if ui.button(RichText::new(label).color(col)).on_hover_text(format!("{tip}\n\nClick to choose your prod.keys")).clicked() {
                    self.pick_keys();
                }
                if self.library.keys.is_some() && ui.small_button("x").on_hover_text("Forget the chosen keys").clicked() {
                    self.library.keys = None;
                    self.save();
                }
                if !self.message.is_empty() {
                    ui.separator();
                    ui.colored_label(AMBER, &self.message);
                    if ui.small_button("x").clicked() {
                        self.message.clear();
                    }
                }
            });
        });

        // the list: icon, name, state
        let rows: Vec<(String, String, String, Color32)> = self.games.iter().map(|g| { let (t, c) = summary(g); (g.title_id(), g.title(), t, c) }).collect();
        let icons: Vec<Option<egui::TextureHandle>> = rows.iter().map(|r| self.icon(ctx, &r.0)).collect();
        egui::SidePanel::left("games").resizable(true).default_width(280.0).show(ctx, |ui| {
            ui.add_space(6.0);
            if self.games.is_empty() {
                ui.label("No games yet: Add a Switch game.");
            }
            for (i, (_, name, line, col)) in rows.iter().enumerate() {
                let r = ui.horizontal(|ui| {
                    match &icons[i] {
                        Some(t) => { ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(40.0, 40.0)).corner_radius(4.0)); }
                        None => { ui.add_sized([40.0, 40.0], egui::Label::new(RichText::new("?").size(22.0).color(Color32::GRAY))); }
                    }
                    ui.vertical(|ui| {
                        let r = ui.selectable_label(self.selected == i, RichText::new(name).strong());
                        if r.clicked() {
                            self.selected = i;
                        }
                        ui.label(RichText::new(line).color(*col).small());
                    });
                });
                if r.response.interact(egui::Sense::click()).clicked() {
                    self.selected = i;
                }
                ui.add_space(4.0);
            }
        });

        let sel = self.selected;
        let sel_tid = self.games.get(sel).map(|g| g.title_id()).unwrap_or_default();
        let big_icon = self.icon(ctx, &sel_tid);
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(g) = self.games.get_mut(sel) else {
                ui.centered_and_justified(|ui| ui.label("Add a Switch game (its .nsp or .xci) to start."));
                return;
            };
            let info = g.info();
            let d = g.describe();
            let s = g.status();

            // ---- what the game is
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if let Some(t) = &big_icon {
                    ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(112.0, 112.0)).corner_radius(8.0));
                }
                ui.vertical(|ui| {
                    ui.heading(RichText::new(g.title()).size(26.0));
                    let mut sub = Vec::new();
                    if !g.entry.publisher.is_empty() { sub.push(g.entry.publisher.clone()); }
                    let ver = if !g.entry.version.is_empty() { g.entry.version.clone() } else { d.as_ref().map(|d| d.game_version.clone()).unwrap_or_default() };
                    if !ver.is_empty() { sub.push(format!("v{ver}")); }
                    let tid = g.title_id();
                    if !tid.is_empty() { sub.push(tid.to_uppercase()); }
                    ui.label(RichText::new(sub.join("  ·  ")).color(Color32::GRAY));
                    if let Some(d) = &d {
                        ui.label(RichText::new(format!("Runs with {} ({})", d.headline, d.version)).color(Color32::GRAY));
                    }
                    let (line, col) = summary(g);
                    ui.colored_label(col, line);
                });
            });
            ui.add_space(8.0);

            // ---- no app for it yet
            if g.entry.exe.is_none() {
                ui.label("This game has no nxrt app in the library yet. Add the recomp's app (for example fe3hrecomp), \
                          or choose it here:");
                ui.horizontal(|ui| {
                    if ui.button("Choose its app...").clicked() {
                        action = Some(Action::ChooseApp(sel));
                    }
                    if ui.button("Remove").on_hover_text("From this list only: nothing is deleted").clicked() {
                        action = Some(Action::Remove(sel));
                    }
                });
                return;
            }
            if let Some(d) = &d {
                if !g.entry.build_id.is_empty() && !d.build_id.is_empty() && !g.entry.build_id.eq_ignore_ascii_case(&d.build_id) {
                    ui.colored_label(RED, format!("This package is another version of the game than {} is for (v{}).", d.title, d.game_version));
                }
            }

            // ---- progress of a running step
            if let Some(r) = &g.run {
                if r.kind != Kind::Game {
                    let p = r.progress.lock().unwrap().clone();
                    let label = if p.step.is_empty() { "Starting...".to_string() } else { p.step.clone() };
                    match p.fraction {
                        Some(f) => { ui.add(egui::ProgressBar::new(f).text(label).animate(true)); }
                        None => { ui.horizontal(|ui| { ui.spinner(); ui.label(label); }); }
                    }
                    if ui.button("Cancel").clicked() {
                        if let Some(r) = &mut g.run { r.stop(); }
                    }
                    ui.add_space(6.0);
                }
            }

            // ---- the steps: files, generate, compile, play - each asked for when it is next
            let st = g.steps();
            let busy = g.running();
            let step_row = |ui: &mut egui::Ui, n: usize, done: bool, title: &str, detail: &str, next: bool, button: Option<(&str, Job)>| -> Option<Job> {
                let mut clicked = None;
                ui.horizontal(|ui| {
                    let mark = if done { RichText::new("✔").color(GREEN) } else if next { RichText::new("➜").color(AMBER) } else { RichText::new("○").color(Color32::GRAY) };
                    ui.label(mark.size(18.0));
                    ui.vertical(|ui| {
                        ui.label(RichText::new(format!("{n}. {title}")).strong());
                        ui.label(RichText::new(detail).color(Color32::GRAY).small());
                    });
                    if let Some((label, job)) = button {
                        if !done && !busy {
                            let b = egui::Button::new(if next { RichText::new(label).strong() } else { RichText::new(label) });
                            if ui.add_enabled(next, b).clicked() {
                                clicked = Some(job);
                            }
                        }
                    }
                });
                ui.add_space(4.0);
                clicked
            };
            let mut picked: Option<Job> = None;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let pkg = g.entry.package.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                let files_detail = if st.files {
                    match info.as_ref().and_then(|i| i.files.clone()) {
                        Some(f) if f.romfs_package => "Ready: the game data is read from the package in place".to_string(),
                        Some(f) if f.romfs_dir => "Ready: an extracted copy".into(),
                        _ => "Ready".into(),
                    }
                } else if pkg.is_empty() {
                    "From the game's package or an extracted folder (Set up game files below)".into()
                } else {
                    format!("From {pkg} (a few seconds: nothing is copied)")
                };
                picked = picked.take().or(step_row(ui, 1, st.files, "Game files", &files_detail, !st.files,
                         (!pkg.is_empty()).then(|| ("Set up", Job::Step(Stage::Files, pkg.clone())))));
                if st.builder || !st.dev {
                    picked = picked.take().or(step_row(ui, 2, st.generated, "Generate the game code",
                             if st.generated { "Done: the game's ARM code is C now" } else { "Translates the game's ARM code to C (a few minutes, all cores)" },
                             st.files && !st.generated, Some(("Generate", Job::Step(Stage::Generate, String::new())))));
                    picked = picked.take().or(step_row(ui, 3, st.code, "Compile the game code",
                             if st.code { "Done: built for this version of the app" } else { "Compiles that C into the game's library (a few minutes)" },
                             st.files && st.generated && !st.code, Some(("Compile", Job::Step(Stage::Compile, String::new())))));
                } else {
                    picked = picked.take().or(step_row(ui, 2, st.code, "Build the game code (dev command)", "This app has no game-code builder: its Dev build command",
                             st.files && !st.code, Some(("Build", Job::DevBuild))));
                }
                let ready = s.as_ref().is_some_and(|s| s.ready);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if ready { "✔" } else { "○" }).size(18.0).color(if ready { GREEN } else { Color32::GRAY }));
                    ui.label(RichText::new(format!("{}. Play", if st.builder || !st.dev { 4 } else { 3 })).strong());
                    if g.running() && g.run.as_ref().is_some_and(|r| r.kind == Kind::Game) {
                        if ui.button("Stop").clicked() {
                            if let Some(r) = &mut g.run { r.stop(); }
                        }
                    } else if ui.add_enabled(ready && !busy, egui::Button::new(RichText::new("  Play  ").size(18.0).strong())).clicked() {
                        action = Some(Action::Start(sel, Job::Play));
                    }
                });
            });
            if let Some(job) = picked {
                action = Some(Action::Start(sel, job));
            }
            if let Some((text, ok)) = &g.last_exit {
                ui.colored_label(if *ok { GREEN } else { RED }, text);
            }

            // ---- other things
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if !busy {
                    if ui.button("Start screen").on_hover_text("The game's own start screen: its settings, controls, setup").clicked() {
                        action = Some(Action::Start(sel, Job::StartScreen));
                    }
                    ui.menu_button("Set up game files", |ui| {
                        if ui.button("From a game package (.nsp / .xci)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new().set_title("The game package (prod.keys beside it)").add_filter("Game package", &["nsp", "xci", "nca"]).pick_file() {
                                action = Some(Action::Start(sel, Job::Step(Stage::Files, p.display().to_string())));
                            }
                        }
                        if ui.button("From a folder (exefs + romfs)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new().set_title("The game files' folder").pick_folder() {
                                action = Some(Action::Start(sel, Job::Step(Stage::Files, p.display().to_string())));
                            }
                        }
                        if st.files && ui.button("Generate and compile again").clicked() {
                            ui.close_menu();
                            action = Some(Action::Start(sel, Job::Step(Stage::All, String::new())));
                        }
                    });
                }
                if let Some(s) = &s {
                    if ui.button("Data folder").clicked() { reveal(Path::new(&s.data_root)); }
                    if ui.button("Game folder").clicked() { reveal(&game_dir(s)); }
                }
                if !busy && ui.button("Remove").on_hover_text("From this list only: nothing is deleted").clicked() {
                    action = Some(Action::Remove(sel));
                }
            });
            ui.add_space(6.0);

            egui::CollapsingHeader::new("Details").default_open(false).show(ui, |ui| {
                egui::Grid::new("details").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                    if let Some(p) = &g.entry.package { row(ui, "Package", p.display().to_string()); }
                    if let Some(e) = &g.entry.exe { row(ui, "App", e.display().to_string()); }
                    if let Some(d) = &d {
                        row(ui, "App version", d.version.clone());
                        row(ui, "Game build", d.build_id.clone());
                        row(ui, "Game code id", d.gamelib_id.clone());
                    }
                    if let Some(Err(e)) = info.as_ref().and_then(|i| i.describe.clone()) {
                        row(ui, "App", RichText::new(e).color(RED));
                    }
                    if let Some(s) = &s {
                        row(ui, "Data folder", s.data_root.clone());
                        row(ui, "Game folder", game_dir(s).display().to_string());
                        if !s.files_ready { row(ui, "Game files", s.files.clone()); }
                        if !s.ready && !s.status.is_empty() { row(ui, "The app says", s.status.clone()); }
                        row(ui, "Game-code builder", if s.builder { "in the app" } else { "none (a dev build: Dev build below)" });
                    }
                    if let Some(f) = info.as_ref().and_then(|i| i.files.clone()) {
                        if let (Some(b), Some(d)) = (&f.build_id, &d) {
                            let ok = b.eq_ignore_ascii_case(&d.build_id);
                            row(ui, "Files' build", RichText::new(format!("{b}{}", if ok { "" } else { "  (not the version the app is for)" })).color(if ok { Color32::GRAY } else { RED }));
                        }
                    }
                    if let Some(r) = info.as_ref().and_then(|i| i.romfs.clone()) {
                        match r {
                            Ok(r) => row(ui, "Game data", format!("{} files, {:.1} GB", r.files, r.bytes as f64 / 1e9)),
                            Err(e) => row(ui, "Game data", RichText::new(e).color(RED)),
                        }
                    }
                    if *g.loading.lock().unwrap() { row(ui, "", "checking..."); }
                });
            });

            egui::CollapsingHeader::new("Dev build").default_open(false).show(ui, |ui| {
                ui.label(RichText::new("For an app built from source without a game-code builder: what builds its game code, and where.").color(Color32::GRAY));
                egui::Grid::new("dev").num_columns(2).show(ui, |ui| {
                    ui.label("Folder");
                    ui.add(egui::TextEdit::singleline(&mut g.edit_dir).desired_width(520.0).hint_text("the game's source checkout"));
                    ui.end_row();
                    ui.label("Command");
                    ui.add(egui::TextEdit::multiline(&mut g.edit_cmd).desired_rows(2).desired_width(520.0).font(egui::TextStyle::Monospace)
                        .hint_text("make gen && make build/<id>game.dylib"));
                    ui.end_row();
                });
                if ui.button("Save").clicked() {
                    action = Some(Action::SaveDev(sel));
                }
            });

            let log = g.run.as_ref().map(|r| (r.log.clone(), r.log_file.clone()));
            egui::CollapsingHeader::new("Log").default_open(true).show(ui, |ui| match log {
                None => {
                    ui.label(RichText::new("Shown while the game or a step runs (each run's log is kept in the logs folder).").color(Color32::GRAY));
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
                    egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show_rows(ui, 14.0, lines.len(), |ui, range| {
                        for l in lines.range(range) {
                            ui.label(RichText::new(l).monospace().size(11.0));
                        }
                    });
                }
            });
        });

        match action {
            Some(Action::Start(i, job)) => self.start(i, job, ctx),
            Some(Action::Remove(i)) => {
                self.games.remove(i);
                self.selected = self.selected.min(self.games.len().saturating_sub(1));
                self.save();
            }
            Some(Action::SaveDev(i)) => {
                let g = &mut self.games[i];
                let (d, c) = (g.edit_dir.trim().to_string(), g.edit_cmd.trim().to_string());
                g.entry.build_dir = (!d.is_empty()).then(|| PathBuf::from(d));
                g.entry.build_cmd = (!c.is_empty()).then_some(c);
                self.save();
            }
            Some(Action::ChooseApp(i)) => self.choose_app(ctx, i),
            None => {}
        }

        self.add_dialog(ctx);

        // (a package dropped on the window: added)
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for p in dropped {
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nsp") || e.eq_ignore_ascii_case("xci")) {
                self.add_package(ctx, p);
            }
        }
    }
}

impl App {
    /// The nxrt app for a game added from its package: it must run that title.
    fn choose_app(&mut self, ctx: &egui::Context, i: usize) {
        let Some(p) = rfd::FileDialog::new().set_title("The game's nxrt app").pick_file() else { return };
        let exe = resolve_app(&p);
        match runner::describe(&exe) {
            Err(e) => self.message = e,
            Ok(d) if !self.games[i].entry.title_id.is_empty() && !d.title_id.eq_ignore_ascii_case(&self.games[i].entry.title_id) => {
                self.message = format!("{} is for {}, not this game", d.title, d.game_name);
            }
            Ok(_) => {
                let mut i = i;
                // (an entry for that app alone gives way to this one, its data folder kept)
                if let Some(j) = self.games.iter().position(|g| g.entry.exe.as_deref() == Some(exe.as_path()) && g.entry.package.is_none()) {
                    let dr = self.games[j].entry.data_root.clone();
                    self.games[i].entry.data_root = dr;
                    self.games.remove(j);
                    if j < i {
                        i -= 1;
                    }
                }
                self.selected = i;
                self.games[i].entry.exe = Some(exe);
                self.games[i].refresh(ctx);
                self.save();
            }
        }
    }

    fn add_dialog(&mut self, ctx: &egui::Context) {
        if !self.add.open {
            return;
        }
        let mut open = true;
        let mut done = false;
        egui::Window::new("Add an nxrt app").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label("A recomp's app: fe3hrecomp, eowrecomp, lasrecomp ...");
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
                ui.colored_label(RED, &self.add.error);
            }
            ui.add_space(6.0);
            if ui.button("Add").clicked() {
                let exe = resolve_app(Path::new(self.add.exe.trim()));
                match runner::describe(&exe) {
                    Err(e) => self.add.error = e,
                    Ok(d) => {
                        let dr = self.add.data_root.trim();
                        let dr = (!dr.is_empty()).then(|| PathBuf::from(dr));
                        if self.games.iter().any(|g| g.entry.exe.as_deref() == Some(exe.as_path())) {
                            self.add.error = "Already in the list".into();
                        } else if let Some(i) = self.games.iter().position(|g| g.entry.exe.is_none() && g.entry.title_id.eq_ignore_ascii_case(&d.title_id)) {
                            // (a game added from its package waits for this app)
                            self.games[i].entry.exe = Some(exe);
                            self.games[i].entry.data_root = dr;
                            self.games[i].refresh(ctx);
                            self.selected = i;
                            done = true;
                        } else {
                            let g = Game::new(Entry { exe: Some(exe), data_root: dr, ..Default::default() });
                            g.refresh(ctx);
                            self.games.push(g);
                            self.selected = self.games.len() - 1;
                            done = true;
                        }
                    }
                }
            }
        });
        if done {
            self.save();
        }
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

// ------------------------------------------------------------------ the command line (scripts, tests)

/// `--list`: every game of the library and how it stands, as the window shows it
fn list() {
    for e in Library::load().games {
        let i = Info::gather(&e);
        let g = Game::new(e.clone());
        *g.info.lock().unwrap() = Some(i.clone());
        let (line, _) = summary(&g);
        println!("{} [{}] - {line}", g.title(), g.title_id());
        if let Some(p) = &e.package { println!("  package: {}", p.display()); }
        if let Some(Ok(d)) = &i.describe { println!("  app: {} v{} (code {})", d.title, d.version, d.gamelib_id); }
        if let Some(Err(err)) = &i.describe { println!("  app: {err}"); }
        if let Some(Ok(s)) = &i.status {
            let st = g.steps();
            println!("  steps: files {} generated {} compiled {} | data {} | game {}", st.files, st.generated, st.code, s.data_root, game_dir(s).display());
        }
        if let Some(Ok(r)) = &i.romfs { println!("  game data: {} files, {:.2} GB", r.files, r.bytes as f64 / 1e9); }
    }
}

/// `--step <title id or app id> <files|generate|compile|all> [src]`: a step run as the window runs it, progress on stdout
fn cli_step(who: &str, stage: &str, src: Option<&str>) -> i32 {
    let lib = Library::load();
    let Some(e) = lib.games.iter().find(|e| {
        e.title_id.eq_ignore_ascii_case(who) || e.exe.as_ref().is_some_and(|x| runner::describe(x).is_ok_and(|d| d.id == who || d.title_id.eq_ignore_ascii_case(who)))
    }) else {
        eprintln!("{who}: not in the library");
        return 2;
    };
    let Some(exe) = &e.exe else {
        eprintln!("{who}: no nxrt app for it");
        return 2;
    };
    let stage = match stage { "files" => Stage::Files, "generate" => Stage::Generate, "compile" => Stage::Compile, _ => Stage::All };
    let src = src.map(str::to_string).or_else(|| e.package.as_ref().map(|p| p.display().to_string())).unwrap_or_default();
    let code_id = runner::status(exe, e.data_root.as_deref()).map(|s| s.code_id).unwrap_or_default();
    let keys = lib.keys.clone().filter(|k| k.is_file()).or_else(|| e.keys.clone());
    let mut r = match Run::setup(exe, e.data_root.as_deref(), stage, &src, keys.as_deref(), None, || {}) {
        Ok(r) => r,
        Err(err) => { eprintln!("{err}"); return 2; }
    };
    let mut last = String::new();
    let how = loop {
        if let Some(how) = r.exited() { break how; }
        let p = r.progress.lock().unwrap().clone();
        let line = match p.fraction { Some(f) => format!("{:5.1}% {}", f * 100.0, p.step), None => p.step.clone() };
        if line != last { println!("{line}"); last = line; }
        std::thread::sleep(Duration::from_millis(500));
    };
    let res = r.progress.lock().unwrap().result.clone();
    let ok = res.as_ref().is_some_and(|r| r.ok);
    if ok && stage == Stage::Generate {  // (recorded as the window does: Compile is next)
        let mut lib = Library::load();
        if let Some(x) = lib.games.iter_mut().find(|x| x.exe == e.exe && x.package == e.package) {
            x.generated_for = runner::status(exe, e.data_root.as_deref()).map(|s| s.code_id).unwrap_or(code_id);
            let _ = lib.save();
        }
    }
    match res {
        Some(r) if r.ok => println!("done ({how})"),
        Some(r) => println!("failed: {} {} ({how})", r.step, r.hint),
        None => println!("no result ({how})"),
    }
    if ok { 0 } else { 1 }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    // --keys <prod.keys>: the keys games are read with (as the toolbar's Keys)
    if let Some(i) = args.iter().position(|a| a == "--keys") {
        let k = PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or(""));
        if let Err(e) = library::check_keys(&k) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        let mut lib = Library::load();
        lib.keys = Some(k.clone());
        if let Err(e) = lib.save() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        println!("keys: {}", k.display());
        return Ok(());
    }
    if args.iter().any(|a| a == "--list") {
        list();
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--step") {
        let a = |k: usize| args.get(i + k).map(String::as_str);
        std::process::exit(cli_step(a(1).unwrap_or(""), a(2).unwrap_or("all"), a(3)));
    }
    // --add <app or package> [--data-root <dir>]: an nxrt app, or a Switch game from its .nsp / .xci
    if let Some(i) = args.iter().position(|a| a == "--add") {
        let what = PathBuf::from(args.get(i + 1).map(String::as_str).unwrap_or(""));
        let dr = args.iter().position(|a| a == "--data-root").and_then(|j| args.get(j + 1)).map(PathBuf::from);
        let mut lib = Library::load();
        let is_pkg = what.extension().is_some_and(|e| e.eq_ignore_ascii_case("nsp") || e.eq_ignore_ascii_case("xci"));
        let msg = if is_pkg {
            let Some(keys) = keys_for(lib.keys.as_deref(), &what) else {
                eprintln!("no prod.keys beside the package or in ~/.switch");
                std::process::exit(1);
            };
            let mut games: Vec<Game> = lib.games.iter().cloned().map(Game::new).collect();
            match add_package_to(&mut games, &what, &keys) {
                Ok(i) => {
                    lib.games = games.iter().map(|g| g.entry.clone()).collect();
                    let e = &lib.games[i];
                    format!("{} [{}]: added{}", e.name, e.title_id, if e.exe.is_some() { ", with its app" } else { " (no nxrt app for it yet)" })
                }
                Err(e) => { eprintln!("{e}"); std::process::exit(1); }
            }
        } else {
            let exe = resolve_app(&what);
            match runner::describe(&exe) {
                Ok(d) => {
                    if let Some(g) = lib.games.iter_mut().find(|g| g.exe.is_none() && g.title_id.eq_ignore_ascii_case(&d.title_id)) {
                        g.exe = Some(exe);
                        g.data_root = dr;
                        format!("{}: given to {}", d.title, g.name)
                    } else {
                        format!("{}: {}", d.title, if lib.add_app(&exe, dr) { "added" } else { "already in the list" })
                    }
                }
                Err(e) => { eprintln!("{e}"); std::process::exit(1); }
            }
        };
        if let Err(e) = lib.save() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        println!("{msg}");
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_title("nxrt").with_inner_size([1060.0, 720.0]).with_min_inner_size([700.0, 460.0]).with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("nxrt-frontend", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
