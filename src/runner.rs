//! A game's own app (its runner: fe3hrecomp, eowrecomp ...): who it is (`--describe`), how its files and game code
//! stand (`--status`), and running it with its log captured. The runner stays the owner of its settings and saves; the
//! manager only starts it with the data folder it was told to use.
use serde::Deserialize;
use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

/// `<exe> --describe` (runtime/rt_ui.mm rt_print_describe)
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Describe {
    pub id: String,
    #[allow(dead_code)]  // (the runner's own name for itself: logs)
    pub exe: String,
    pub title: String,
    pub game_name: String,
    pub game_version: String,
    pub title_id: String,
    pub build_id: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    #[allow(dead_code)]  // (the app's own start-screen title)
    pub headline: String,
    #[serde(default)]
    pub gamelib_id: String,
}

/// `<exe> --status` (runtime/rt_ui.mm rt_print_status)
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Status {
    pub data_root: String,
    pub game_dir: String,
    /// "" = the game files are fine, else what is wrong with them
    pub files: String,
    /// "" = ready to play, else what is missing ("Game code not built yet" ...)
    pub status: String,
    #[serde(default)]
    pub code_id: String,
    pub files_ready: bool,
    pub code_ready: bool,
    pub ready: bool,
    /// the app can build its game code itself (a shipped build)
    pub builder: bool,
}

fn command(exe: &Path, data_root: Option<&Path>) -> Command {
    let mut c = Command::new(exe);
    if let Some(d) = data_root {
        c.env("NXRT_DATA_ROOT", d);
    }
    c.env("NXRT_NO_UPDATE_CHECK", "1");
    c
}

/// One JSON object from the runner's stdout (it logs to stderr).
fn query<T: for<'de> Deserialize<'de>>(exe: &Path, data_root: Option<&Path>, flag: &str, key: &str) -> Result<T, String> {
    let out = command(exe, data_root)
        .arg(flag)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("can't run {}: {e}", exe.display()))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text
        .lines()
        .find(|l| l.contains(key))
        .ok_or_else(|| format!("{} doesn't answer {flag} (an nxrt app from before the manager?)", exe.display()))?;
    serde_json::from_str(line).map_err(|e| format!("{flag}: {e}"))
}

pub fn describe(exe: &Path) -> Result<Describe, String> {
    query(exe, None, "--describe", "\"nxrt_describe\"")
}

pub fn status(exe: &Path, data_root: Option<&Path>) -> Result<Status, String> {
    query(exe, data_root, "--status", "\"nxrt_status\"")
}

/// The log of a run: its last lines, kept while the manager reads them.
pub type Log = Arc<Mutex<VecDeque<String>>>;
const LOG_LINES: usize = 20_000;

fn push(log: &Log, line: String) {
    let mut l = log.lock().unwrap();
    if l.len() >= LOG_LINES {
        l.pop_front();
    }
    l.push_back(line);
}

/// What a run is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    /// the game (or its own start screen)
    Game,
    /// a setup step done by the app (`--setup-files`, `--generate`, `--compile`, `--setup`)
    Setup(Stage),
    /// a dev build command (make gen + the game library): an app without a bundled builder
    DevBuild,
    /// a new recomp's app being built (its build.sh: ::step lines)
    Create,
}

/// The steps from a game package to a game that plays (runtime/rt_ui.mm rt_run_setup).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Stage {
    /// the game files from a package or folder (seconds: a package is read in place)
    Files,
    /// the game's code translated to C, kept for the compile (minutes)
    Generate,
    /// the C compiled into the game's library (minutes)
    Compile,
    /// all of it at once
    All,
}

/// `--setup`'s result line (runtime/rt_ui.mm rt_run_setup)
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SetupResult {
    pub ok: bool,
    #[serde(default)]
    pub builder: bool,
    #[serde(default)]
    pub step: String,
    #[serde(default)]
    pub hint: String,
    #[serde(default)]
    pub status: String,
}

#[derive(Deserialize)]
struct ProgressLine {
    fraction: f32,
    step: String,
}

/// How far a setup or build is (from its progress lines), and its result once it has one.
#[derive(Clone, Debug, Default)]
pub struct Progress {
    pub fraction: Option<f32>,
    pub step: String,
    pub result: Option<SetupResult>,
}

/// A running game, setup or build.
pub struct Run {
    child: Child,
    pub kind: Kind,
    pub log: Log,
    pub progress: Arc<Mutex<Progress>>,
    /// where the whole log is also written
    pub log_file: Option<PathBuf>,
}

impl Run {
    /// The game: `--play` boots straight into it (the manager is the start screen), without it the app's own start
    /// screen comes up (its settings, setup).
    pub fn game(exe: &Path, data_root: Option<&Path>, play: bool, log_file: Option<PathBuf>, repaint: impl Fn() + Send + Sync + 'static) -> Result<Run, String> {
        let mut c = command(exe, data_root);
        if play {
            c.arg("--play");
        }
        Run::spawn(c, exe, Kind::Game, log_file, repaint)
    }

    /// A setup step: Files / All from src (a folder, or an .nsp / .xci with prod.keys beside it; "" for All = only
    /// the game code), Generate and Compile on the files already set up. Hidden: no window.
    /// keys: the player's prod.keys for a package (NXRT_KEYS; None: the app looks beside it and in ~/.switch).
    /// update: the game's update for a package (NXRT_UPDATE), set up with it.
    pub fn setup(exe: &Path, data_root: Option<&Path>, stage: Stage, src: &str, keys: Option<&Path>, update: Option<&Path>, log_file: Option<PathBuf>, repaint: impl Fn() + Send + Sync + 'static) -> Result<Run, String> {
        let mut c = command(exe, data_root);
        if let Some(k) = keys {
            c.env("NXRT_KEYS", k);
        }
        if let Some(u) = update {
            c.env("NXRT_UPDATE", u);
        }
        match stage {
            Stage::Files => c.arg("--setup-files").arg(src),
            Stage::Generate => c.arg("--generate"),
            Stage::Compile => c.arg("--compile"),
            Stage::All => c.arg("--setup").arg(src),
        };
        c.env("NXRT_HIDDEN", "1");
        Run::spawn(c, exe, Kind::Setup(stage), log_file, repaint)
    }

    /// A shell command in dir (a game's source checkout: `make gen` and the game library).
    pub fn dev_build(cmd: &str, dir: &Path, log_file: Option<PathBuf>, repaint: impl Fn() + Send + Sync + 'static) -> Result<Run, String> {
        let mut c = Command::new("/bin/sh");
        c.arg("-c").arg(cmd).current_dir(dir);
        Run::spawn(c, dir, Kind::DevBuild, log_file, repaint)
    }

    /// A new recomp's build (recomp.rs: its build.sh), in its folder.
    pub fn create(dir: &Path, log_file: Option<PathBuf>, repaint: impl Fn() + Send + Sync + 'static) -> Result<Run, String> {
        let mut c = Command::new("/bin/sh");
        c.arg("build.sh").current_dir(dir);
        Run::spawn(c, dir, Kind::Create, log_file, repaint)
    }

    fn spawn(mut c: Command, what: &Path, kind: Kind, log_file: Option<PathBuf>, repaint: impl Fn() + Send + Sync + 'static) -> Result<Run, String> {
        if kind != Kind::DevBuild && kind != Kind::Create {
            if let Some(dir) = what.parent() {
                c.current_dir(dir);
            }
        }
        let mut child = c
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("can't start {}: {e}", what.display()))?;
        let log: Log = Arc::new(Mutex::new(VecDeque::new()));
        let progress: Arc<Mutex<Progress>> = Default::default();
        let file = log_file.as_ref().and_then(|p| std::fs::File::create(p).ok()).map(|f| Arc::new(Mutex::new(f)));
        let repaint = Arc::new(repaint);
        let streams: Vec<Box<dyn std::io::Read + Send>> = vec![
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        for s in streams {
            let (log, file, repaint, progress) = (log.clone(), file.clone(), repaint.clone(), progress.clone());
            std::thread::spawn(move || {
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    if let Some(f) = &file {
                        use std::io::Write;
                        let _ = writeln!(f.lock().unwrap(), "{line}");
                    }
                    if matches!(kind, Kind::Setup(_)) && line.starts_with("{\"nxrt_progress\"") {
                        if let Ok(p) = serde_json::from_str::<ProgressLine>(&line) {
                            let mut g = progress.lock().unwrap();
                            g.fraction = Some(p.fraction);
                            if !p.step.is_empty() {
                                g.step = p.step;
                            }
                        }
                    } else if matches!(kind, Kind::Setup(_)) && line.starts_with("{\"nxrt_setup\"") {
                        if let Ok(r) = serde_json::from_str::<SetupResult>(&line) {
                            progress.lock().unwrap().result = Some(r);
                        }
                    } else if kind == Kind::Create && line.starts_with("::step ") {
                        progress.lock().unwrap().step = line[7..].to_string();
                    } else {
                        if kind == Kind::DevBuild {
                            progress.lock().unwrap().step = line.chars().take(120).collect();
                        }
                        push(&log, line);
                    }
                    repaint();
                }
            });
        }
        Ok(Run { child, kind, log, progress, log_file })
    }

    /// None while it runs, else how it ended.
    pub fn exited(&mut self) -> Option<String> {
        match self.child.try_wait() {
            Ok(None) => None,
            Ok(Some(st)) => Some(exit_text(st)),
            Err(e) => Some(format!("lost track of it: {e}")),
        }
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
    }
}

fn exit_text(st: std::process::ExitStatus) -> String {
    if st.success() {
        return "closed".into();
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = st.signal() {
            let name = match sig {
                4 => "SIGILL",
                6 => "SIGABRT",
                9 => "SIGKILL",
                10 => "SIGBUS",
                11 => "SIGSEGV",
                15 => "SIGTERM",
                _ => "",
            };
            return format!("crashed (signal {sig} {name})");
        }
    }
    format!("exited with code {}", st.code().unwrap_or(-1))
}
