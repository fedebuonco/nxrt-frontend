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
mod recomp;
mod runner;

use eframe::egui::{self, Color32, RichText};
use library::{Entry, Library};
use runner::{Describe, Kind, Run, Stage, Status};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---- the look (the redesign: a graphite ground, one teal accent; states in green / blue / amber / red)
const BG: Color32 = Color32::from_rgb(0x0E, 0x10, 0x14);
const NAV: Color32 = Color32::from_rgb(0x13, 0x16, 0x1C);
const CARD: Color32 = Color32::from_rgb(0x16, 0x1A, 0x21);
const CARD_HI: Color32 = Color32::from_rgb(0x1C, 0x21, 0x29);
const LINE: Color32 = Color32::from_rgb(0x23, 0x29, 0x34);
const LINE_HI: Color32 = Color32::from_rgb(0x2C, 0x33, 0x3E);
const WELL: Color32 = Color32::from_rgb(0x0A, 0x0C, 0x0F);
const TEXT: Color32 = Color32::from_rgb(0xE6, 0xE8, 0xEC);
const SUB: Color32 = Color32::from_rgb(0xA9, 0xB0, 0xBC);
const MUTED: Color32 = Color32::from_rgb(0x9A, 0xA1, 0xAD);
const ACCENT: Color32 = Color32::from_rgb(0x5F, 0xD4, 0xC6);
const ON_ACCENT: Color32 = Color32::from_rgb(0x0E, 0x10, 0x14);
const GREEN: Color32 = Color32::from_rgb(0x5B, 0xD1, 0x8B);
const AMBER: Color32 = Color32::from_rgb(0xF5, 0xB1, 0x4C);
const RED: Color32 = Color32::from_rgb(0xEE, 0x7B, 0x7B);
const BLUE: Color32 = Color32::from_rgb(0x7F, 0xB2, 0xFF);

/// The system's own UI and mono faces where there are (macOS: SF), else egui's.
fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    for (name, path, family) in [
        ("ui", "/System/Library/Fonts/SFNS.ttf", egui::FontFamily::Proportional),
        ("mono", "/System/Library/Fonts/SFNSMono.ttf", egui::FontFamily::Monospace),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts.font_data.insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
            fonts.families.entry(family).or_default().insert(0, name.into());
        }
    }
    ctx.set_fonts(fonts);
}

fn setup_style(ctx: &egui::Context) {
    use egui::{FontId, Stroke, TextStyle};
    let mut st = (*ctx.style()).clone();
    st.visuals = egui::Visuals::dark();
    let v = &mut st.visuals;
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = WELL;
    v.faint_bg_color = CARD;
    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.selection.bg_fill = Color32::from_rgb(0x1F, 0x4D, 0x48);
    v.selection.stroke = Stroke::new(1.0, ACCENT);
    v.window_stroke = Stroke::new(1.0, LINE_HI);
    v.window_corner_radius = egui::CornerRadius::same(16);
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = egui::CornerRadius::same(10);
    }
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, LINE);
    v.widgets.inactive.bg_fill = CARD_HI;
    v.widgets.inactive.weak_bg_fill = CARD_HI;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE_HI);
    v.widgets.hovered.bg_fill = Color32::from_rgb(0x24, 0x2A, 0x34);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x24, 0x2A, 0x34);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, Color32::from_rgb(0x3A, 0x42, 0x4F));
    v.widgets.active.bg_fill = Color32::from_rgb(0x2A, 0x31, 0x3C);
    st.spacing.item_spacing = egui::vec2(10.0, 8.0);
    st.spacing.button_padding = egui::vec2(14.0, 8.0);
    st.spacing.interact_size.y = 36.0;
    st.text_styles = [
        (TextStyle::Heading, FontId::proportional(30.0)),
        (TextStyle::Body, FontId::proportional(14.5)),
        (TextStyle::Button, FontId::proportional(14.5)),
        (TextStyle::Small, FontId::proportional(12.0)),
        (TextStyle::Monospace, FontId::monospace(12.5)),
    ]
    .into();
    ctx.set_style(st);
}

/// A filled call-to-action button (the accent), or an outlined one.
fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).strong().color(ON_ACCENT)).fill(ACCENT).corner_radius(10).min_size(egui::vec2(0.0, 40.0))
}
fn secondary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).color(TEXT)).fill(CARD_HI).stroke(egui::Stroke::new(1.0, LINE_HI)).corner_radius(10).min_size(egui::vec2(0.0, 40.0))
}
fn outlined_accent(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).strong().color(ACCENT)).fill(Color32::TRANSPARENT).stroke(egui::Stroke::new(1.0, ACCENT)).corner_radius(10).min_size(egui::vec2(0.0, 40.0))
}

/// A card's frame
fn card(fill: Color32, line: Color32) -> egui::Frame {
    egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, line)).corner_radius(16).inner_margin(egui::Margin::same(16))
}

/// A status dot and its words
fn status_dot(ui: &mut egui::Ui, text: &str, col: Color32) {
    ui.horizontal(|ui| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
        ui.painter().circle_filled(r.center(), 4.0, col);
        ui.label(RichText::new(text).color(col).strong());
    });
}

/// A thin progress bar in a colour
fn thin_bar(ui: &mut egui::Ui, f: f32, col: Color32, h: f32) {
    let w = ui.available_width();
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    ui.painter().rect_filled(r, h / 2.0, Color32::from_rgb(0x23, 0x2A, 0x36));
    let mut fill = r;
    fill.set_width(r.width() * f.clamp(0.0, 1.0));
    ui.painter().rect_filled(fill, h / 2.0, col);
}

/// The game's tile: its icon, else its initials on a tint of its own
fn tile(ui: &mut egui::Ui, icon: &Option<egui::TextureHandle>, name: &str, size: egui::Vec2, radius: u8) {
    match icon {
        Some(t) => {
            // (cover: the square icon fills a wide banner, cropped top and bottom)
            let (ar, tex) = (size.x / size.y, t.size_vec2());
            let ta = tex.x / tex.y.max(1.0);
            let uv = if ar > ta {
                let h = ta / ar;
                egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - h) / 2.0), egui::pos2(1.0, (1.0 + h) / 2.0))
            } else {
                let w = ar / ta;
                egui::Rect::from_min_max(egui::pos2((1.0 - w) / 2.0, 0.0), egui::pos2((1.0 + w) / 2.0, 1.0))
            };
            ui.add(egui::Image::new(t).uv(uv).fit_to_exact_size(size).maintain_aspect_ratio(false).corner_radius(radius));
        }
        None => {
            let (r, _) = ui.allocate_exact_size(size, egui::Sense::hover());
            let h = name.bytes().fold(7u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
            let tints = [(0x2A, 0x23, 0x33), (0x1E, 0x2E, 0x2A), (0x1D, 0x27, 0x38), (0x33, 0x28, 0x1E), (0x26, 0x22, 0x30)];
            let (cr, cg, cb) = tints[(h as usize) % tints.len()];
            ui.painter().rect_filled(r, radius as f32, Color32::from_rgb(cr, cg, cb));
            let initials: String = name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty() && w.len() > 2 || w.chars().all(|c| c.is_ascii_uppercase()))
                .filter_map(|w| w.chars().next()).take(3).collect();
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, if initials.is_empty() { "?".into() } else { initials.to_uppercase() },
                egui::FontId::proportional(size.y * 0.3), Color32::from_rgb(0xE9, 0xD9, 0xC5));
        }
    }
}

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
    /// a recomp being made for it: the app its build makes, and its data folder
    creating: Option<(PathBuf, PathBuf)>,
}

impl Game {
    fn new(entry: Entry) -> Game {
        let edit_dir = entry.build_dir.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let edit_cmd = entry.build_cmd.clone().unwrap_or_default();
        Game { entry, info: Default::default(), loading: Default::default(), run: None, run_code_id: String::new(), last_exit: None, edit_dir, edit_cmd, creating: None }
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
    CreateRecomp(usize),
    Open(usize),
    AddPackage,
    AddApp,
}

#[derive(Default)]
struct CreateDialog {
    open: bool,
    game: usize,
    id: String,
    dir: String,
    nxrt: String,
    error: String,
}

/// A new recomp for a game added from its package (recomp.rs): its game files set up from the package into
/// <dir>/data/extracted, its folder written. Its app is built next (Run::create).
fn create_recomp(e: &Entry, id: &str, dir: &Path, nxrt: &Path, keys: &Path) -> Result<(), String> {
    if !recomp::valid_id(id) {
        return Err("The id is a short name in lower-case letters and digits (it names the app: <id>recomp)".into());
    }
    if !nxrt.join("nxrt.mk").is_file() {
        return Err(format!("{}: not an nxrt checkout (no nxrt.mk)", nxrt.display()));
    }
    let Some(pkg) = &e.package else { return Err("No package for this game".into()) };
    if dir.join("Makefile").exists() {
        return Err(format!("{} already has a recomp", dir.display()));
    }
    let files = dir.join("data").join("extracted");
    std::fs::create_dir_all(&files).map_err(|err| format!("{}: {err}", files.display()))?;
    manage::package_setup(pkg, keys, &files)?;
    let modules = recomp::layout(&files.join("exefs"))?;
    recomp::scaffold(dir, id, e, &modules, nxrt)
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

#[derive(Clone, Copy, PartialEq)]
enum View {
    Library,
    Game,
    Logs,
    Settings,
}

struct App {
    library: Library,
    games: Vec<Game>,
    selected: usize,
    view: View,
    /// NXRT_FRONTEND_SHOT=<png>: the window saved as it looks after a few frames, then the app exits (checks, docs)
    shot: Option<(PathBuf, u32)>,
    add: AddDialog,
    message: String,
    icons: HashMap<String, Option<egui::TextureHandle>>,
    create: CreateDialog,
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
        setup_fonts(&cc.egui_ctx);
        setup_style(&cc.egui_ctx);
        let mut view = View::Library;
        let mut selected = 0;
        // NXRT_FRONTEND_VIEW=library|logs|settings|game:<n>: the view it opens on (with the shot)
        if let Ok(v) = std::env::var("NXRT_FRONTEND_VIEW") {
            match v.as_str() {
                "logs" => view = View::Logs,
                "settings" => view = View::Settings,
                g if g.starts_with("game:") => { view = View::Game; selected = g[5..].parse().unwrap_or(0); }
                _ => {}
            }
        }
        let shot = std::env::var_os("NXRT_FRONTEND_SHOT").map(|p| (PathBuf::from(p), 0));
        App { library, games, selected, view, shot, add: AddDialog::default(), message, icons: HashMap::new(), create: CreateDialog::default() }
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
    /// Its page opens.
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
                self.view = View::Game;
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
                (Kind::Create, _) => match g.creating.take() {
                    Some((app, data)) if how == "closed" && app.is_file() => {
                        g.entry.exe = Some(app);
                        g.entry.data_root = Some(data);
                        changed = true;
                        ("Recomp created: next, generate its game code".into(), true)
                    }
                    _ => (format!("Creating the recomp failed ({how}): see the log in the logs folder"), false),
                },
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

/// Shows a file in Finder (selected), or opens a folder.
fn reveal_file(p: &Path) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg("-R").arg(p).spawn();
    #[cfg(not(target_os = "macos"))]
    if let Some(d) = p.parent() {
        reveal(d);
    }
}

/// What each place a game uses is for (Where things are; the buttons' tooltips say the same)
const ABOUT_PACKAGE: &str = "Your copy of the game (.nsp / .xci). Never changed: the game's data is read from it in place, \
    so keep it where it is.";
const ABOUT_KEYS: &str = "The keys from your own Switch (prod.keys). Needed to read the package; nothing is decrypted to disk.";
const ABOUT_APP: &str = "The recomp's program: it runs this game (and does its setup steps). The manager starts it.";
const ABOUT_RECOMP: &str = "The recomp the manager created for this game: its source (Makefile, runtime/<id>_game.c, \
    tools/patches.py). Edit it to work on the port; build.sh rebuilds the app.";
const ABOUT_DATA: &str = "Yours: the game's settings, saves and their backups, mods, and the shader cache. Back this folder up; \
    deleting it loses your saves.";
const ABOUT_GAME: &str = "Made from the package: the game's code files (exefs), a pointer to its data in the package \
    (romfs.src), and the built game code (.gamebuild, the game's library). Safe to delete: the steps make it again.";
const ABOUT_LOGS: &str = "A log of every run and step the manager started: what to send when something goes wrong.";

/// One place of Where things are, as a card: its name and what to do with it (keep / back up / rebuildable), what
/// it is, its path, and a button to show it.
fn place(ui: &mut egui::Ui, name: &str, tag: (&str, Color32, Color32), about: &str, path: &Path, is_file: bool) {
    card(CARD, LINE).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(name).strong());
            egui::Frame::new().fill(tag.1).corner_radius(6).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                ui.label(RichText::new(tag.0).small().color(tag.2));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(path.exists(), secondary(if is_file { "Show" } else { "Open" }).min_size(egui::vec2(0.0, 30.0))).clicked() {
                    if is_file { reveal_file(path) } else { reveal(path) }
                }
            });
        });
        ui.label(RichText::new(about).color(SUB).small());
        let exists = path.exists();
        ui.label(RichText::new(format!("{}{}", path.display(), if exists { "" } else { "  (not there yet)" })).monospace().size(11.0)
            .color(if exists { MUTED } else { AMBER }));
    });
}

const TAG_KEEP: (&str, Color32, Color32) = ("keep", Color32::from_rgb(0x2A, 0x2F, 0x38), Color32::from_rgb(0xC9, 0xCE, 0xD6));
const TAG_BACKUP: (&str, Color32, Color32) = ("back up", Color32::from_rgb(0x3A, 0x2A, 0x1B), Color32::from_rgb(0xF2, 0xC4, 0x8D));
const TAG_REBUILD: (&str, Color32, Color32) = ("rebuildable", Color32::from_rgb(0x1E, 0x2E, 0x2A), Color32::from_rgb(0x9F, 0xDC, 0xC3));
const TAG_SOURCE: (&str, Color32, Color32) = ("source", Color32::from_rgb(0x1D, 0x27, 0x38), Color32::from_rgb(0xA9, 0xC6, 0xF5));

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

/// What the game's card and page offer as the one next thing
fn next_action(g: &Game, i: usize) -> Option<(String, Action, bool)> {
    if g.running() {
        return None;
    }
    if g.entry.exe.is_none() {
        return g.entry.package.is_some().then(|| ("Create a recomp".to_string(), Action::CreateRecomp(i), false));
    }
    let s = g.status()?;
    if s.ready {
        return Some(("Play".into(), Action::Start(i, Job::Play), true));
    }
    let st = g.steps();
    if !st.files {
        let pkg = g.entry.package.as_ref()?.display().to_string();
        return Some(("Set up the game files".into(), Action::Start(i, Job::Step(Stage::Files, pkg)), true));
    }
    if !st.builder && st.dev {
        return Some(("Build the game code".into(), Action::Start(i, Job::DevBuild), true));
    }
    if !st.generated {
        return Some(("Generate".into(), Action::Start(i, Job::Step(Stage::Generate, String::new())), true));
    }
    Some(("Compile".into(), Action::Start(i, Job::Step(Stage::Compile, String::new())), true))
}

/// A running step's progress: (fraction if known, its words)
fn run_progress(g: &Game) -> Option<(Option<f32>, String)> {
    let r = g.run.as_ref()?;
    if r.kind == Kind::Game {
        return None;
    }
    let p = r.progress.lock().unwrap().clone();
    Some((p.fraction, if p.step.is_empty() { "Starting...".into() } else { p.step }))
}

fn nav_item(ui: &mut egui::Ui, label: &str, on: bool) -> bool {
    let b = egui::Button::new(RichText::new(label).color(if on { TEXT } else { SUB }).strong())
        .fill(if on { CARD_HI } else { Color32::TRANSPARENT })
        .stroke(egui::Stroke::NONE)
        .corner_radius(10)
        .min_size(egui::vec2(ui.available_width(), 44.0));
    ui.add(b).clicked()
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_runs(ctx);
        let mut action: Option<Action> = None;
        if let Some((path, n)) = &mut self.shot {
            *n += 1;
            ctx.request_repaint();
            if *n == 90 {  // (the games' status gathered, the icons loaded)
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
            let img = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
            if let Some(img) = img {
                let [w, h] = img.size;
                let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, rgba) {
                    let _ = buf.save(&*path);
                }
                std::process::exit(0);
            }
        }

        // ---- the navigation, with the keys at its foot
        egui::SidePanel::left("nav").resizable(false).exact_width(232.0).frame(egui::Frame::new().fill(NAV).inner_margin(egui::Margin::symmetric(16, 20))
            .stroke(egui::Stroke::new(1.0, LINE))).show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::Frame::new().fill(ACCENT).corner_radius(8).inner_margin(egui::Margin::symmetric(7, 3)).show(ui, |ui| {
                    ui.label(RichText::new("nx").strong().color(ON_ACCENT).size(15.0));
                });
                ui.label(RichText::new("nxrt").strong().size(19.0));
            });
            ui.add_space(18.0);
            let busy = self.games.iter().filter(|g| g.running()).count();
            if nav_item(ui, &format!("Library   {}", self.games.len()), self.view == View::Library || self.view == View::Game) {
                self.view = View::Library;
            }
            if nav_item(ui, "Logs", self.view == View::Logs) {
                self.view = View::Logs;
            }
            if nav_item(ui, "Settings", self.view == View::Settings) {
                self.view = View::Settings;
            }
            if busy > 0 {
                ui.add_space(8.0);
                ui.horizontal(|ui| { ui.spinner(); ui.label(RichText::new(format!("{busy} running")).color(BLUE)); });
            }
            ui.add_space((ui.available_height() - 132.0).max(12.0));  // (the keys at the foot)
            {
                card(Color32::from_rgb(0x17, 0x1B, 0x22), LINE).inner_margin(egui::Margin::same(14)).show(ui, |ui| ui.vertical(|ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("KEYS").small().color(MUTED));
                    let home = dirs::home_dir().map(|h| h.join(".switch/prod.keys")).filter(|p| p.is_file());
                    let (name, col, path) = match (&self.library.keys, &home) {
                        (Some(k), _) if k.is_file() => (k.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), GREEN, k.parent().map(|p| p.display().to_string()).unwrap_or_default()),
                        (Some(k), _) => ("missing".into(), RED, k.display().to_string()),
                        (None, Some(h)) => ("prod.keys".into(), SUB, h.parent().map(|p| p.display().to_string()).unwrap_or_default()),
                        (None, None) => ("none chosen".into(), AMBER, "prod.keys beside each package".into()),
                    };
                    status_dot(ui, &name, col);
                    ui.label(RichText::new(path).monospace().size(10.5).color(MUTED));
                    ui.horizontal(|ui| {
                        if ui.add(secondary("Change...").min_size(egui::vec2(0.0, 32.0))).on_hover_text(ABOUT_KEYS).clicked() {
                            self.pick_keys();
                        }
                        if self.library.keys.is_some() && ui.add(secondary("Forget").min_size(egui::vec2(0.0, 32.0))).clicked() {
                            self.library.keys = None;
                            self.save();
                        }
                    });
                }));
            }
        });

        // the icons the views draw (loaded once)
        let tids: Vec<String> = self.games.iter().map(|g| g.title_id()).collect();
        let icons: Vec<Option<egui::TextureHandle>> = tids.iter().map(|t| self.icon(ctx, t)).collect();

        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(egui::Margin::symmetric(36, 28))).show(ctx, |ui| {
            if !self.message.is_empty() {
                card(Color32::from_rgb(0x2A, 0x22, 0x16), Color32::from_rgb(0x5A, 0x44, 0x22)).inner_margin(egui::Margin::symmetric(14, 10)).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&self.message).color(AMBER));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(secondary("Dismiss").min_size(egui::vec2(0.0, 28.0))).clicked() { self.message.clear(); }
                        });
                    });
                });
                ui.add_space(12.0);
            }
            let view_id = match self.view { View::Game => 1000 + self.selected, v => v as usize };
            egui::ScrollArea::vertical().id_salt(("view", view_id)).auto_shrink([false, false]).show(ui, |ui| match self.view {
                View::Library => self.library_view(ui, ctx, &icons, &mut action),
                View::Game => self.game_view(ui, &icons, &mut action),
                View::Logs => logs_view(ui),
                View::Settings => self.settings_view(ui),
            });
        });

        match action {
            Some(Action::Start(i, job)) => self.start(i, job, ctx),
            Some(Action::Remove(i)) => {
                self.games.remove(i);
                self.selected = self.selected.min(self.games.len().saturating_sub(1));
                self.view = View::Library;
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
            Some(Action::CreateRecomp(i)) => {
                let id = recomp::default_id(&self.games[i].entry.name);
                self.create = CreateDialog {
                    open: true,
                    game: i,
                    dir: recomp::default_dir(&id).display().to_string(),
                    id,
                    nxrt: recomp::default_nxrt().display().to_string(),
                    error: String::new(),
                };
            }
            Some(Action::Open(i)) => {
                self.selected = i;
                self.view = View::Game;
            }
            Some(Action::AddPackage) => {
                if let Some(p) = rfd::FileDialog::new().set_title("A Switch game (.nsp / .xci)").add_filter("Switch game", &["nsp", "xci"]).pick_file() {
                    self.add_package(ctx, p);
                }
            }
            Some(Action::AddApp) => self.add = AddDialog { open: true, ..Default::default() },
            None => {}
        }

        self.add_dialog(ctx);
        self.create_dialog(ctx);

        // (a package dropped on the window: added)
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        for p in dropped {
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("nsp") || e.eq_ignore_ascii_case("xci")) {
                self.add_package(ctx, p);
            }
        }
    }
}

fn logs_view(ui: &mut egui::Ui) {
    ui.heading(RichText::new("Logs").strong());
    ui.label(RichText::new(ABOUT_LOGS).color(SUB));
    ui.add_space(12.0);
    let dir = library::dir().join("logs");
    let mut logs: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&dir).map(|rd| rd.flatten()
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path()))).collect()).unwrap_or_default();
    logs.sort_by(|a, b| b.0.cmp(&a.0));
    if ui.add(secondary("Open the logs folder")).clicked() {
        let _ = std::fs::create_dir_all(&dir);
        reveal(&dir);
    }
    ui.add_space(8.0);
    for (_, p) in logs.iter().take(40) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).monospace());
            if ui.add(secondary("Show").min_size(egui::vec2(0.0, 28.0))).clicked() { reveal_file(p); }
        });
    }
    if logs.is_empty() {
        ui.label(RichText::new("No runs yet.").color(MUTED));
    }
}

impl App {
    fn library_view(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context, icons: &[Option<egui::TextureHandle>], action: &mut Option<Action>) {
        // ---- the header: title, what stands, add
        ui.horizontal_wrapped(|ui| {
            ui.vertical(|ui| {
                ui.heading(RichText::new("Library").strong());
                let ready = self.games.iter().filter(|g| g.status().is_some_and(|s| s.ready)).count();
                let working = self.games.iter().filter(|g| g.running()).count();
                let mut line = format!("{} game{} · {ready} ready to play", self.games.len(), if self.games.len() == 1 { "" } else { "s" });
                if working > 0 { line += &format!(" · {working} working"); }
                ui.label(RichText::new(line).color(MUTED));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(primary("+  Add a Switch game")).on_hover_text("Its .nsp or .xci: what it is is read from the package").clicked() {
                    *action = Some(Action::AddPackage);
                }
                if ui.add(secondary("Add an nxrt app")).on_hover_text("A recomp's app: fe3hrecomp, eowrecomp ...").clicked() {
                    *action = Some(Action::AddApp);
                }
                if ui.add(secondary("Refresh")).clicked() {
                    for g in &self.games { g.refresh(ui.ctx()); }
                }
            });
        });
        ui.add_space(20.0);

        // ---- the grid of games
        let gap = 20.0;
        let min_w = 230.0;
        let avail = ui.available_width();
        let cols = ((avail + gap) / (min_w + gap)).floor().max(1.0) as usize;
        let w = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
        for row_start in (0..self.games.len()).step_by(cols) {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                for i in row_start..(row_start + cols).min(self.games.len()) {
                    let g = &self.games[i];
                    let (line, col) = summary(g);
                    let prog = run_progress(g);
                    let next = next_action(g, i);
                    let line_col = if g.entry.exe.is_none() { LINE_HI } else if prog.is_some() { Color32::from_rgb(0x3A, 0x4A, 0x63) } else { LINE };
                    let r = ui.allocate_ui_with_layout(egui::vec2(w, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::Frame::new().fill(CARD).stroke(egui::Stroke::new(1.0, line_col)).corner_radius(16).show(ui, |ui| {
                            ui.set_width(w);
                            ui.spacing_mut().item_spacing.y = 6.0;
                            tile(ui, &icons[i], &g.title(), egui::vec2(w, (w * 0.62).min(170.0)), 16);
                            egui::Frame::new().inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 16 }).show(ui, |ui| {
                                ui.set_width(w - 32.0);
                                ui.label(RichText::new(g.title()).strong().size(15.5));
                                let mut sub = Vec::new();
                                if !g.entry.publisher.is_empty() { sub.push(g.entry.publisher.clone()); }
                                let ver = if !g.entry.version.is_empty() { g.entry.version.clone() } else { g.describe().map(|d| d.game_version).unwrap_or_default() };
                                if !ver.is_empty() { sub.push(format!("v{ver}")); }
                                if let Some(d) = g.describe() { sub.push(d.title); }
                                ui.label(RichText::new(sub.join(" · ")).color(MUTED).small());
                                match &prog {
                                    Some((f, step)) => {
                                        status_dot(ui, step, BLUE);
                                        thin_bar(ui, f.unwrap_or(0.0), BLUE, 6.0);
                                    }
                                    None => status_dot(ui, &line, col),
                                }
                                ui.add_space(4.0);
                                match &next {
                                    Some((label, _, true)) => {
                                        if ui.add(primary(label).min_size(egui::vec2(w - 32.0, 42.0))).clicked() {
                                            *action = next_action(g, i).map(|n| n.1);
                                        }
                                    }
                                    Some((label, _, false)) => {
                                        if ui.add(outlined_accent(label).min_size(egui::vec2(w - 32.0, 42.0))).clicked() {
                                            *action = next_action(g, i).map(|n| n.1);
                                        }
                                    }
                                    None => {
                                        if ui.add(secondary("Open").min_size(egui::vec2(w - 32.0, 42.0))).clicked() {
                                            *action = Some(Action::Open(i));
                                        }
                                    }
                                }
                            });
                        });
                    });
                    // the rest of the card opens the game
                    if action.is_none() && r.response.interact(egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        *action = Some(Action::Open(i));
                    }
                }
            });
            ui.add_space(gap);
        }

        // ---- the drop zone
        egui::Frame::new().stroke(egui::Stroke::new(1.5, LINE_HI)).corner_radius(16).inner_margin(egui::Margin::same(26)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Drop an .nsp or .xci here").strong().color(ACCENT));
                ui.label(RichText::new("The game is read from its own package: nothing is copied").color(MUTED).small());
            });
        });
    }

    fn game_view(&mut self, ui: &mut egui::Ui, icons: &[Option<egui::TextureHandle>], action: &mut Option<Action>) {
        let sel = self.selected;
        let keys = self.library.keys.clone().filter(|k| k.is_file());
        let Some(g) = self.games.get_mut(sel) else {
            self.view = View::Library;
            return;
        };
        if ui.add(egui::Button::new(RichText::new("‹  Library").color(SUB)).frame(false).min_size(egui::vec2(0.0, 36.0))).clicked() {
            self.view = View::Library;
            return;
        }
        let info = g.info();
        let d = g.describe();
        let s = g.status();
        let st = g.steps();
        let busy = g.running();

        // ---- the header: tile, name, what it is, state; Start screen and Play
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            tile(ui, &icons[sel], &g.title(), egui::vec2(132.0, 132.0), 24);
            ui.add_space(12.0);
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(g.title()).strong().size(32.0));
                let mut sub = Vec::new();
                if !g.entry.publisher.is_empty() { sub.push(g.entry.publisher.clone()); }
                let ver = if !g.entry.version.is_empty() { g.entry.version.clone() } else { d.as_ref().map(|d| d.game_version.clone()).unwrap_or_default() };
                if !ver.is_empty() { sub.push(format!("v{ver}")); }
                let tid = g.title_id();
                if !tid.is_empty() { sub.push(tid.to_uppercase()); }
                if let Some(d) = &d { sub.push(format!("runs with {} ({})", d.title, d.version)); }
                ui.label(RichText::new(sub.join(" · ")).color(MUTED));
                ui.add_space(4.0);
                match run_progress(g) {
                    Some((_, step)) => status_dot(ui, &step, BLUE),
                    None => { let (l, c) = summary(g); status_dot(ui, &l, c); }
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| if g.entry.exe.is_some() {
                    let ready = s.as_ref().is_some_and(|s| s.ready);
                    if g.run.as_ref().is_some_and(|r| r.kind == Kind::Game) {
                        if ui.add(secondary("Stop").min_size(egui::vec2(110.0, 48.0))).clicked() {
                            if let Some(r) = &mut g.run { r.stop(); }
                        }
                    } else if ui.add_enabled(ready && !busy, primary("▶  Play").min_size(egui::vec2(140.0, 48.0))).clicked() {
                        *action = Some(Action::Start(sel, Job::Play));
                    }
                    if !busy && ui.add(secondary("Start screen").min_size(egui::vec2(0.0, 48.0))).on_hover_text("The game's own start screen: its settings, controls, setup").clicked() {
                        *action = Some(Action::Start(sel, Job::StartScreen));
                    }
                });
            });
        });
        ui.add_space(22.0);

        // ---- no app yet: make one, or choose it
        if g.entry.exe.is_none() {
            card(CARD, LINE_HI).inner_margin(egui::Margin::same(24)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if let Some((_, step)) = run_progress(g) {
                    ui.horizontal(|ui| { ui.spinner(); ui.label(RichText::new(step).strong().color(BLUE)); });
                    if ui.add(secondary("Cancel")).clicked() {
                        if let Some(r) = &mut g.run { r.stop(); }
                    }
                } else {
                    ui.label(RichText::new("No recomp runs this game yet").strong().size(18.0));
                    ui.label(RichText::new("If its recomp exists, add its app. Otherwise make one from the package: a plain recompilation to start a port from.").color(SUB));
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if g.entry.package.is_some() && ui.add(primary("Create a recomp")).clicked() { *action = Some(Action::CreateRecomp(sel)); }
                        if ui.add(secondary("Choose its app...")).clicked() { *action = Some(Action::ChooseApp(sel)); }
                        if ui.add(secondary("Remove")).on_hover_text("From this list only: nothing is deleted").clicked() { *action = Some(Action::Remove(sel)); }
                    });
                }
                if let Some((text, ok)) = &g.last_exit { ui.colored_label(if *ok { GREEN } else { RED }, text); }
            });
            ui.add_space(18.0);
            log_box(ui, g);
            return;
        }
        if let Some(d) = &d {
            if !g.entry.build_id.is_empty() && !d.build_id.is_empty() && !g.entry.build_id.eq_ignore_ascii_case(&d.build_id) {
                ui.colored_label(RED, format!("This package is another version of the game than {} is for (v{}).", d.title, d.game_version));
            }
        }

        // ---- the step pipeline: four cards, then the running step's progress
        let prog = run_progress(g);
        let running_stage = g.run.as_ref().and_then(|r| match r.kind { Kind::Setup(s) => Some(s), Kind::DevBuild => Some(Stage::All), _ => None });
        let pkg = g.entry.package.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
        let romfs = info.as_ref().and_then(|i| i.romfs.clone()).and_then(|r| r.ok());
        let files_detail = if st.files {
            match (info.as_ref().and_then(|i| i.files.clone()), &romfs) {
                (Some(f), Some(r)) if f.romfs_package => format!("Read from the package in place · {} files, {:.1} GB", r.files, r.bytes as f64 / 1e9),
                (Some(f), Some(r)) if f.romfs_dir => format!("An extracted copy · {} files, {:.1} GB", r.files, r.bytes as f64 / 1e9),
                _ => "Ready".into(),
            }
        } else if pkg.is_empty() { "From the game's package or an extracted folder".into() } else { "From its package, in seconds: nothing is copied".into() };
        struct StepCard { title: &'static str, detail: String, done: bool, running: bool, job: Option<Job>, label: &'static str }
        let gen_split = st.builder || !st.dev;
        let mut steps = vec![StepCard { title: "Game files", detail: files_detail, done: st.files, running: running_stage == Some(Stage::Files),
            job: (!pkg.is_empty()).then(|| Job::Step(Stage::Files, pkg.clone())), label: "Set up" }];
        if gen_split {
            steps.push(StepCard { title: "Generate the game code", detail: "The game's ARM code, translated to C".into(), done: st.generated,
                running: running_stage == Some(Stage::Generate), job: Some(Job::Step(Stage::Generate, String::new())), label: "Generate" });
            steps.push(StepCard { title: "Compile the game code", detail: "That C, built into the game's library".into(), done: st.code,
                running: matches!(running_stage, Some(Stage::Compile) | Some(Stage::All)), job: Some(Job::Step(Stage::Compile, String::new())), label: "Compile" });
        } else {
            steps.push(StepCard { title: "Build the game code", detail: "This app's Dev build command".into(), done: st.code,
                running: running_stage == Some(Stage::All), job: Some(Job::DevBuild), label: "Build" });
        }
        let ready = s.as_ref().is_some_and(|s| s.ready);
        steps.push(StepCard { title: "Play", detail: "The game in its own window".into(), done: false, running: g.run.as_ref().is_some_and(|r| r.kind == Kind::Game),
            job: None, label: "" });
        let next_idx = steps.iter().position(|c| !c.done && c.job.is_some());

        card(CARD, LINE).inner_margin(egui::Margin::same(22)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let n = steps.len();
            let gap = 12.0;
            let avail = ui.available_width();
            let cols = if avail >= 4.0 * 190.0 + 3.0 * gap { n } else { 2 };
            let cw = (avail - gap * (cols as f32 - 1.0)) / cols as f32;
            for chunk in steps.chunks(cols).enumerate() {
                let (ci, chunk) = chunk;
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for (k, c) in chunk.iter().enumerate() {
                        let idx = ci * cols + k;
                        let is_next = Some(idx) == next_idx && !busy;
                        let is_play_ready = c.title == "Play" && ready;
                        let (fill, line, head_col, head) = if c.done || is_play_ready {
                            (Color32::from_rgb(0x12, 0x23, 0x1E), Color32::from_rgb(0x22, 0x46, 0x3A), GREEN, if is_play_ready { format!("Step {} · ready", idx + 1) } else { format!("✔  Step {} · done", idx + 1) })
                        } else if c.running {
                            (Color32::from_rgb(0x15, 0x20, 0x36), Color32::from_rgb(0x3A, 0x5A, 0x8C), BLUE, format!("Step {} · running", idx + 1))
                        } else if is_next {
                            (Color32::from_rgb(0x2A, 0x22, 0x16), Color32::from_rgb(0x6A, 0x50, 0x26), AMBER, format!("➜  Step {} · next", idx + 1))
                        } else {
                            (Color32::from_rgb(0x14, 0x17, 0x1D), LINE, MUTED, format!("Step {}", idx + 1))
                        };
                        ui.allocate_ui_with_layout(egui::vec2(cw, 0.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            egui::Frame::new().fill(fill).stroke(egui::Stroke::new(1.0, line)).corner_radius(14).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
                                ui.set_width(cw - 32.0);
                                ui.set_min_height(96.0);
                                ui.label(RichText::new(head.to_uppercase()).small().strong().color(head_col));
                                ui.label(RichText::new(c.title).strong().size(15.5));
                                ui.label(RichText::new(&c.detail).small().color(SUB));
                                if is_next {
                                    if let Some(job) = &c.job {
                                        ui.add_space(4.0);
                                        if ui.add(primary(c.label)).clicked() {
                                            *action = Some(Action::Start(sel, match job {
                                                Job::Step(s, src) => Job::Step(*s, src.clone()),
                                                Job::DevBuild => Job::DevBuild,
                                                Job::Play => Job::Play,
                                                Job::StartScreen => Job::StartScreen,
                                            }));
                                        }
                                    }
                                }
                            });
                        });
                    }
                });
                ui.add_space(gap);
            }
            // the running step
            if let Some((f, step)) = &prog {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(step).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if let Some(f) = f { ui.label(RichText::new(format!("{:.0}%", f * 100.0)).monospace().color(SUB)); }
                    });
                });
                match f {
                    Some(f) => thin_bar(ui, *f, BLUE, 10.0),
                    None => { ui.spinner(); }
                }
                ui.horizontal(|ui| {
                    ui.label(RichText::new("The next step asks to start when this one is done").small().color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(secondary("Cancel")).clicked() {
                            if let Some(r) = &mut g.run { r.stop(); }
                        }
                    });
                });
            } else if let Some((text, ok)) = &g.last_exit {
                ui.colored_label(if *ok { GREEN } else { RED }, text);
            } else if !busy {
                ui.horizontal(|ui| {
                    egui::menu::menu_custom_button(ui, secondary("Set up again  ▾"), |ui| {
                        if ui.button("From a game package (.nsp / .xci)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new().set_title("The game package").add_filter("Game package", &["nsp", "xci", "nca"]).pick_file() {
                                *action = Some(Action::Start(sel, Job::Step(Stage::Files, p.display().to_string())));
                            }
                        }
                        if ui.button("From a folder (exefs + romfs)...").clicked() {
                            ui.close_menu();
                            if let Some(p) = rfd::FileDialog::new().set_title("The game files' folder").pick_folder() {
                                *action = Some(Action::Start(sel, Job::Step(Stage::Files, p.display().to_string())));
                            }
                        }
                        if st.files && ui.button("Generate and compile again").clicked() {
                            ui.close_menu();
                            *action = Some(Action::Start(sel, Job::Step(Stage::All, String::new())));
                        }
                    });
                });
            }
        });
        ui.add_space(24.0);

        // ---- where things are, and the log
        let wide = ui.available_width() > 900.0;
        let places = |ui: &mut egui::Ui, g: &Game| {
            ui.label(RichText::new("Where things are").strong().size(19.0));
            ui.add_space(4.0);
            if let Some(p) = &g.entry.package { place(ui, "Game package", TAG_KEEP, ABOUT_PACKAGE, p, true); }
            if let Some(s) = &s {
                place(ui, "Data folder", TAG_BACKUP, ABOUT_DATA, Path::new(&s.data_root), false);
                place(ui, "Game folder", TAG_REBUILD, ABOUT_GAME, &game_dir(s), false);
            }
            if let Some(e) = &g.entry.exe { place(ui, "App", TAG_KEEP, ABOUT_APP, e, true); }
            if let Some(r) = &g.entry.recomp_dir { place(ui, "Recomp folder", TAG_SOURCE, ABOUT_RECOMP, r, false); }
            let k = keys.clone().or_else(|| g.entry.keys.clone());
            if let Some(k) = &k { place(ui, "Keys", TAG_KEEP, ABOUT_KEYS, k, true); }
        };
        if wide {
            ui.columns(2, |c| {
                places(&mut c[0], g);
                log_box(&mut c[1], g);
            });
        } else {
            places(ui, g);
            ui.add_space(12.0);
            log_box(ui, g);
        }

        ui.add_space(16.0);
        egui::CollapsingHeader::new(RichText::new("Details").color(SUB)).default_open(false).show(ui, |ui| {
            egui::Grid::new("details").num_columns(2).spacing([16.0, 4.0]).show(ui, |ui| {
                if let Some(d) = &d {
                    row(ui, "App version", d.version.clone());
                    row(ui, "Game build", d.build_id.clone());
                    row(ui, "Game code id", d.gamelib_id.clone());
                }
                if let Some(Err(e)) = info.as_ref().and_then(|i| i.describe.clone()) { row(ui, "App", RichText::new(e).color(RED)); }
                if let Some(s) = &s {
                    if !s.files_ready { row(ui, "Game files", s.files.clone()); }
                    if !s.ready && !s.status.is_empty() { row(ui, "The app says", s.status.clone()); }
                    row(ui, "Game-code builder", if s.builder { "in the app" } else { "none (a dev build: Dev build below)" });
                }
                if let Some(f) = info.as_ref().and_then(|i| i.files.clone()) {
                    if let (Some(b), Some(d)) = (&f.build_id, &d) {
                        let ok = b.eq_ignore_ascii_case(&d.build_id);
                        row(ui, "Files' build", RichText::new(format!("{b}{}", if ok { "" } else { "  (not the version the app is for)" })).color(if ok { MUTED } else { RED }));
                    }
                }
                if *g.loading.lock().unwrap() { row(ui, "", "checking..."); }
            });
        });
        egui::CollapsingHeader::new(RichText::new("Dev build").color(SUB)).default_open(false).show(ui, |ui| {
            ui.label(RichText::new("For an app built from source without a game-code builder: what builds its game code, and where.").color(MUTED));
            egui::Grid::new("dev").num_columns(2).show(ui, |ui| {
                ui.label("Folder");
                ui.add(egui::TextEdit::singleline(&mut g.edit_dir).desired_width(520.0).hint_text("the game's source checkout"));
                ui.end_row();
                ui.label("Command");
                ui.add(egui::TextEdit::multiline(&mut g.edit_cmd).desired_rows(2).desired_width(520.0).font(egui::TextStyle::Monospace).hint_text("make gen && make build/<id>game.dylib"));
                ui.end_row();
            });
            if ui.add(secondary("Save")).clicked() { *action = Some(Action::SaveDev(sel)); }
        });
        ui.add_space(8.0);
        if !busy && ui.add(secondary("Remove from the library")).on_hover_text("From this list only: nothing is deleted").clicked() {
            *action = Some(Action::Remove(sel));
        }
    }

    fn settings_view(&mut self, ui: &mut egui::Ui) {
        ui.heading(RichText::new("Settings").strong());
        ui.add_space(12.0);
        card(CARD, LINE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("Keys").strong());
            ui.label(RichText::new(ABOUT_KEYS).color(SUB));
            ui.label(RichText::new(self.library.keys.as_ref().map(|k| k.display().to_string()).unwrap_or_else(|| "none chosen: prod.keys beside each package, or ~/.switch".into())).monospace().color(MUTED));
            if ui.add(secondary("Choose prod.keys...")).clicked() { self.pick_keys(); }
        });
        ui.add_space(12.0);
        card(CARD, LINE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("New recomps").strong());
            ui.label(RichText::new("Where Create a recomp puts them, and the nxrt checkout they build against (each can be changed when creating).").color(SUB));
            ui.label(RichText::new(format!("{}  ·  nxrt {}", library::dir().join("recomps").display(), recomp::default_nxrt().display())).monospace().color(MUTED));
        });
        ui.add_space(12.0);
        card(CARD, LINE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new("The manager's own files").strong());
            ui.label(RichText::new("The library (library.toml), the games' icons and the logs.").color(SUB));
            let d = library::dir();
            ui.label(RichText::new(d.display().to_string()).monospace().color(MUTED));
            if ui.add(secondary("Open")).clicked() { let _ = std::fs::create_dir_all(&d); reveal(&d); }
        });
    }
}

/// The run's log, live (or where the logs are kept)
fn log_box(ui: &mut egui::Ui, g: &Game) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Log").strong().size(19.0));
        if let Some(f) = g.run.as_ref().and_then(|r| r.log_file.clone()) {
            ui.label(RichText::new(f.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()).monospace().size(11.0).color(MUTED));
        }
    });
    ui.add_space(4.0);
    egui::Frame::new().fill(WELL).stroke(egui::Stroke::new(1.0, LINE)).corner_radius(14).inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        match g.run.as_ref().map(|r| r.log.clone()) {
            None => {
                ui.set_min_height(120.0);
                ui.label(RichText::new("Shown while the game or a step runs. Every run's log is kept: Logs, in the menu.").color(MUTED));
            }
            Some(log) => {
                let lines = log.lock().unwrap();
                egui::ScrollArea::vertical().id_salt("log").stick_to_bottom(true).max_height(300.0).auto_shrink([false, false]).show_rows(ui, 16.0, lines.len(), |ui, range| {
                    for l in lines.range(range) {
                        ui.label(RichText::new(l).monospace().size(11.5).color(Color32::from_rgb(0xB8, 0xBE, 0xC8)));
                    }
                });
            }
        }
    });
}

impl App {
    fn create_dialog(&mut self, ctx: &egui::Context) {
        if !self.create.open {
            return;
        }
        let i = self.create.game;
        let Some(name) = self.games.get(i).map(|g| g.title()) else { self.create.open = false; return };
        let mut open = true;
        let mut go = false;
        egui::Window::new(format!("Create a recomp of {name}")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(RichText::new("A new nxrt game folder from the game's package: who it is, its code modules, its game files \
                (read from the package in place), and its app built. Then generate and compile its game code as for any \
                game. A plain recompilation: a port starts from here.").color(Color32::GRAY));
            ui.add_space(6.0);
            egui::Grid::new("create").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label("Id");
                ui.horizontal(|ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut self.create.id).desired_width(140.0));
                    ui.label(RichText::new(format!("the app: {}recomp", self.create.id)).color(Color32::GRAY));
                    if r.changed() {
                        self.create.dir = recomp::default_dir(&self.create.id).display().to_string();
                    }
                });
                ui.end_row();
                ui.label("Folder").on_hover_text("The recomp's own folder: its source, its app (build/), and its data/ - the game files \
                    set up from the package, the settings and saves");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.create.dir).desired_width(420.0));
                    if ui.button("Browse...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().set_title("Where the recomp goes (a folder for it is made there)").pick_folder() {
                            self.create.dir = p.join(&self.create.id).display().to_string();
                        }
                    }
                });
                ui.end_row();
                ui.label("nxrt").on_hover_text("The nxrt checkout the recomp builds against: the shared runtime and the recompiler");
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut self.create.nxrt).desired_width(420.0));
                    if ui.button("Browse...").clicked() {
                        if let Some(p) = rfd::FileDialog::new().set_title("The nxrt checkout").pick_folder() {
                            self.create.nxrt = p.display().to_string();
                        }
                    }
                });
                ui.end_row();
            });
            ui.label(RichText::new("Needs: the Xcode command line tools, Python 3, Homebrew's SDL3 and FFmpeg. The app builds in a \
                minute or two; its game code is the next steps.").small().color(Color32::GRAY));
            if !self.create.error.is_empty() {
                ui.colored_label(RED, &self.create.error);
            }
            ui.add_space(6.0);
            go = ui.button(RichText::new("Create").strong()).clicked();
        });
        if !open {
            self.create.open = false;
        }
        if !go {
            return;
        }
        let (id, dir, nxrt) = (self.create.id.trim().to_string(), PathBuf::from(self.create.dir.trim()), PathBuf::from(self.create.nxrt.trim()));
        let e = self.games[i].entry.clone();
        let Some(keys) = self.library.keys.clone().filter(|k| k.is_file()).or_else(|| e.keys.clone()) else {
            self.create.error = "No keys: choose your prod.keys in the toolbar".into();
            return;
        };
        if let Err(err) = create_recomp(&e, &id, &dir, &nxrt, &keys) {
            self.create.error = err;
            return;
        }
        let logs = library::dir().join("logs");
        let _ = std::fs::create_dir_all(&logs);
        let c = ctx.clone();
        match Run::create(&dir, Some(logs.join(format!("{id}-create.log"))), move || c.request_repaint()) {
            Ok(r) => {
                let g = &mut self.games[i];
                g.entry.recomp_dir = Some(dir.clone());
                g.creating = Some((recomp::app_path(&dir, &id), dir.join("data")));
                g.run = Some(r);
                g.last_exit = None;
                self.create.open = false;
                self.save();
            }
            Err(err) => self.create.error = err,
        }
    }

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
            ui.label("Its data folder: the game's settings, saves, mods (and by default its game files). Leave empty for the \
                app's own, as when you start the app by itself.");
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
    // --create <title id> [--id <id>] [--dir <folder>]: a new recomp of a game added from its package, its app built
    if let Some(i) = args.iter().position(|a| a == "--create") {
        let who = args.get(i + 1).cloned().unwrap_or_default();
        let opt = |k: &str| args.iter().position(|a| a == k).and_then(|j| args.get(j + 1)).cloned();
        let mut lib = Library::load();
        let Some(gi) = lib.games.iter().position(|e| e.title_id.eq_ignore_ascii_case(&who)) else {
            eprintln!("{who}: not in the library");
            std::process::exit(2);
        };
        let e = lib.games[gi].clone();
        let id = opt("--id").unwrap_or_else(|| recomp::default_id(&e.name));
        let dir = opt("--dir").map(PathBuf::from).unwrap_or_else(|| recomp::default_dir(&id));
        let keys = lib.keys.clone().filter(|k| k.is_file()).or(e.keys.clone()).unwrap_or_default();
        if let Err(err) = create_recomp(&e, &id, &dir, &recomp::default_nxrt(), &keys) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        println!("{}: recomp at {} (modules in its Makefile); building its app...", e.name, dir.display());
        let mut r = Run::create(&dir, None, || {}).unwrap_or_else(|err| { eprintln!("{err}"); std::process::exit(1) });
        let mut last = String::new();
        let how = loop {
            if let Some(how) = r.exited() { break how; }
            let st = r.progress.lock().unwrap().step.clone();
            if st != last { println!("{st}"); last = st; }
            std::thread::sleep(Duration::from_millis(500));
        };
        let app = recomp::app_path(&dir, &id);
        if how != "closed" || !app.is_file() {
            for l in r.log.lock().unwrap().iter().rev().take(15).collect::<Vec<_>>().into_iter().rev() { eprintln!("  {l}"); }
            eprintln!("the build {how}");
            std::process::exit(1);
        }
        let g = &mut lib.games[gi];
        g.exe = Some(app.clone());
        g.data_root = Some(dir.join("data"));
        g.recomp_dir = Some(dir);
        if let Err(err) = lib.save() { eprintln!("{err}"); std::process::exit(1); }
        println!("app: {} - next: --step {who} generate, then compile", app.display());
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
        viewport: egui::ViewportBuilder::default().with_title("nxrt")
            .with_inner_size(if std::env::var_os("NXRT_FRONTEND_SHOT").is_some() { [1280.0, 860.0] } else { [1180.0, 780.0] })
            .with_min_inner_size([760.0, 500.0]).with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native("nxrt-frontend", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
