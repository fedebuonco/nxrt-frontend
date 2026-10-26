//! The manager's own log: tracing to a daily file (and stderr), a run id shared with nxrt, and the tail the Logs view
//! reads. `NXRT_FRONTEND_LOG` (or `RUST_LOG`) sets the filter; `NXRT_GAME_LOG` is passed to a game as nxrt's
//! `NXRT_LOG`. See docs/LOGGING.md.
use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

pub const DEFAULT_FILTER: &str = "info";
static FILTER: OnceLock<String> = OnceLock::new();
static RUN_SEQ: AtomicU64 = AtomicU64::new(1);
const KEEP_DAYS: u64 = 7;

pub fn log_dir() -> PathBuf {
    crate::library::dir().join("logs")
}

/// Once, first thing in main: a file layer (no ANSI) and stderr (ANSI), both under the env filter.
pub fn init() {
    let dir = log_dir();
    let _ = std::fs::create_dir_all(&dir);
    let spec = std::env::var("NXRT_FRONTEND_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .unwrap_or_else(|_| DEFAULT_FILTER.to_string());
    let filter = EnvFilter::try_new(&spec).unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let _ = FILTER.set(spec);
    let appender = tracing_appender::rolling::daily(&dir, "manager.log");
    let file_layer = fmt::layer()
        .with_writer(Mutex::new(appender))
        .with_ansi(false)
        .with_target(true)
        .with_file(true)
        .with_line_number(true);
    let stderr_layer = fmt::layer().with_writer(std::io::stderr).with_ansi(true).with_target(true);
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();
    prune(&dir, KEEP_DAYS);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(target: "panic", "panic: {info}");
        previous(info);
    }));
}

pub fn filter() -> &'static str {
    FILTER.get().map(String::as_str).unwrap_or(DEFAULT_FILTER)
}

/// A short id for one run; nxrt gets it as NXRT_RUN_ID and shows it as [run=…].
pub fn new_run_id() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("r{secs}-{:03}", RUN_SEQ.fetch_add(1, Ordering::Relaxed) % 1000)
}

pub fn info() -> crate::api::LogInfo {
    crate::api::LogInfo {
        dir: log_dir().display().to_string(),
        path: manager_log_path().map(|p| p.display().to_string()),
        filter: filter().into(),
    }
}

/// The newest daily manager log (manager.log.YYYY-MM-DD), if the file layer has written one.
pub fn manager_log_path() -> Option<PathBuf> {
    let dir = log_dir();
    let mut best: Option<(SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("manager.log") {
            continue;
        }
        let modified = entry.metadata().ok().and_then(|m| m.modified().ok()).unwrap_or(UNIX_EPOCH);
        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
            best = Some((modified, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}

#[derive(Serialize, Clone)]
pub struct ManagerLog {
    pub path: String,
    pub lines: Vec<String>,
    pub filter: String,
    pub dir: String,
}

pub fn manager_log(lines: usize) -> ManagerLog {
    let path = manager_log_path();
    let lines = path.as_deref().map(|p| tail(p, lines)).unwrap_or_default();
    ManagerLog {
        path: path.map(|p| p.display().to_string()).unwrap_or_default(),
        lines,
        filter: filter().into(),
        dir: log_dir().display().to_string(),
    }
}

/// The last `max` lines of a log, reading at most its last 128 KiB; lines are capped so one bad line cannot fill the UI.
pub fn tail(path: &Path, max: usize) -> Vec<String> {
    let Ok(mut f) = std::fs::File::open(path) else { return Vec::new() };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(128 * 1024);
    if f.seek(SeekFrom::Start(start)).is_err() {
        return Vec::new();
    }
    let mut buf = Vec::new();
    if f.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);  // (the first line may be cut mid-way)
    }
    lines
        .iter()
        .rev()
        .take(max)
        .rev()
        .map(|l| l.chars().take(2000).collect())
        .collect()
}

/// A webview script error (the window's error / unhandledrejection listeners call this).
pub fn ui(level: &str, message: &str, source: Option<&str>, stack: Option<&str>) {
    let message: String = message.chars().take(2000).collect();
    let source = source.map(|s| s.chars().take(500).collect::<String>());
    let stack = stack.map(|s| s.chars().take(4000).collect::<String>());
    match level {
        "error" => tracing::error!(target: "ui", source = ?source, stack = ?stack, "{message}"),
        "warn" => tracing::warn!(target: "ui", source = ?source, stack = ?stack, "{message}"),
        "debug" => tracing::debug!(target: "ui", source = ?source, stack = ?stack, "{message}"),
        "trace" => tracing::trace!(target: "ui", source = ?source, stack = ?stack, "{message}"),
        _ => tracing::info!(target: "ui", source = ?source, stack = ?stack, "{message}"),
    }
}

fn prune(dir: &Path, days: u64) {
    let keep = Duration::from_secs(days.saturating_mul(24 * 3600));
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("manager.log") {
            continue;
        }
        let old = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|m| m.elapsed().ok())
            .is_some_and(|age| age > keep);
        if old {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_ids_are_short_and_unique() {
        let a = new_run_id();
        let b = new_run_id();
        assert!(a.starts_with('r'));
        assert!(a.len() <= 24);
        assert_ne!(a, b);
    }

    #[test]
    fn tail_reads_the_last_lines() {
        let path = std::env::temp_dir().join(format!("nxrt-manager-tail-{}.log", std::process::id()));
        std::fs::write(&path, "one\ntwo\nthree\nfour\n").unwrap();
        assert_eq!(tail(&path, 2), vec!["three".to_string(), "four".to_string()]);
        assert_eq!(tail(&path, 10).len(), 4);
        assert!(tail(&path.with_extension("missing"), 2).is_empty());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn filter_falls_back_without_initialization() {
        assert!(!filter().is_empty());
    }
}
