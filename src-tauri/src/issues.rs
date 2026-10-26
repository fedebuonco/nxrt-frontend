//! A GitHub issue for a game, prefilled from what the manager knows: the game and its app, the build, the latest
//! manager log (a tail of it) and the environment. The player reviews it on GitHub before submitting; nothing is
//! sent by the manager itself.
use crate::api::Game;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

pub const NEW_ISSUE: &str = "https://github.com/fedebuonco/nxrt/issues/new";
const MAX_BODY: usize = 4200;
const LOG_TAIL_LINES: usize = 15;

pub struct Env {
    pub nxrt_rev: String,
    pub nxrt_dir: String,
    pub manager: String,
    pub macos: String,
    pub arch: String,
}

impl Env {
    pub fn detect(nxrt_dir: &Path) -> Env {
        let rev = Command::new("git")
            .arg("-C")
            .arg(nxrt_dir)
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unknown".into());
        let macos = Command::new("sw_vers")
            .arg("-productVersion")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| std::env::consts::OS.into());
        Env {
            nxrt_rev: rev,
            nxrt_dir: nxrt_dir.display().to_string(),
            manager: env!("CARGO_PKG_VERSION").into(),
            macos,
            arch: if std::env::consts::ARCH == "aarch64" { "arm64".into() } else { std::env::consts::ARCH.into() },
        }
    }
}

/// The prefilled URL for a game: its latest log in `logs_dir`, and the one-gather view the caller built.
pub fn for_game(g: &Game, nxrt_dir: &Path, logs_dir: &Path) -> String {
    let env = Env::detect(nxrt_dir);
    let path = latest_log(g, logs_dir);
    let tail = path.as_deref().and_then(|p| tail(p, LOG_TAIL_LINES));
    let log = path.as_deref().zip(tail.as_deref());
    url(g, &env, log)
}

pub fn url(g: &Game, env: &Env, log: Option<(&Path, &str)>) -> String {
    let title = if g.title.trim().is_empty() { "nxrt".to_string() } else { g.title.trim().to_string() };
    format!("{NEW_ISSUE}?title={}&body={}", enc(&format!("[{title}] ")), enc(&body(g, env, log)))
}

fn body(g: &Game, env: &Env, log: Option<(&Path, &str)>) -> String {
    let mut s = String::new();
    s.push_str("### Game\n\n| | |\n|---|---|\n");
    row(&mut s, "Title", &g.title);
    row(&mut s, "Title ID", &g.title_id);
    row(&mut s, "Publisher", &g.publisher);
    row(&mut s, "Game version", &g.version);
    row(&mut s, "App", &format!("{} v{}", g.app_name, g.app_version));
    if let Some(d) = &g.describe {
        row(&mut s, "Game build", &d.build_id);
        row(&mut s, "Game code", &d.gamelib_id);
    }
    row(&mut s, "State", &g.state);
    if let Some(st) = &g.status {
        row(&mut s, "The app says", &st.status);
    }
    if let Some(err) = &g.load_error {
        row(&mut s, "App error", err);
    }
    if g.build_mismatch {
        row(&mut s, "Build mismatch", "the package and the app differ");
    }
    if let Some(p) = &g.package {
        row(&mut s, "Package", p);
    }
    if let Some(p) = &g.update {
        row(&mut s, "Update", p);
    }
    if let Some(p) = &g.recomp_dir {
        row(&mut s, "Recomp", p);
    }
    if let Some(p) = &g.exe {
        row(&mut s, "App path", p);
    }
    if let Some(e) = &g.last_exit {
        row(&mut s, "Last run", &e.text);
    }

    s.push_str("\n### What happened?\n<!-- What did you do, what did you expect, and what happened instead? -->\n\n");
    s.push_str("\n### Steps to reproduce\n1. \n2. \n\n");
    s.push_str("### Logs\n\n");
    match log {
        Some((p, t)) => s.push_str(&format!(
            "The manager's log for this game: `{}`\n\n{}\n",
            p.display(),
            fence(t)
        )),
        None => s.push_str(
            "No manager log yet. Start the game (or one of its steps) from the manager once, then open this form again.\n",
        ),
    }
    s.push_str("\n### Environment\n");
    s.push_str(&format!("- nxrt: `{}` (`{}`)\n", env.nxrt_rev, env.nxrt_dir));
    s.push_str(&format!("- nxrt-frontend: `{}`\n", env.manager));
    s.push_str(&format!("- macOS: `{}` ({})\n", env.macos, env.arch));
    s.push_str("\n<!-- Opened from nxrt-frontend. Please review the paths and the log before submitting. -->\n");
    if s.chars().count() > MAX_BODY {
        s = s.chars().take(MAX_BODY).collect();
        s.push_str("\n\n<!-- truncated: paste the rest of the log in the issue -->\n");
    }
    s
}

fn row(s: &mut String, key: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    s.push_str(&format!("| **{}** | {} |\n", key, value.replace('|', "\\|").replace('\n', " ")));
}

fn fence(text: &str) -> String {
    let f = if text.contains("````") {
        "~~~~~"
    } else if text.contains("```") {
        "~~~~"
    } else {
        "```"
    };
    format!("{f}\n{text}\n{f}")
}

/// The last `lines` lines of a log, each capped, or None when it is empty.
pub fn tail(path: &Path, lines: usize) -> Option<String> {
    let data = std::fs::read(path).ok()?;
    let start = data.len().saturating_sub(128 * 1024);
    let text = String::from_utf8_lossy(&data[start..]);
    let all: Vec<&str> = text.lines().collect();
    let picked: Vec<String> = all
        .iter()
        .rev()
        .take(lines)
        .rev()
        .map(|l| l.chars().take(300).collect::<String>())
        .collect();
    let out = picked.join("\n").trim_end().to_string();
    (!out.is_empty()).then_some(out)
}

/// The newest manager log whose name starts with the game's app id (`<id>-<time>.log`, `<id>-generate-...`).
pub fn latest_log(g: &Game, dir: &Path) -> Option<PathBuf> {
    let id = g.describe.as_ref().map(|d| d.id.as_str()).unwrap_or("");
    if id.is_empty() {
        return None;
    }
    let prefix = format!("{id}-");
    let mut best: Option<(u64, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix) || !name.ends_with(".log") {
            continue;
        }
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if best.as_ref().map_or(true, |(m, _)| mtime > *m) {
            best = Some((mtime, entry.path()));
        }
    }
    best.map(|(_, p)| p)
}

/// Percent-encode a query value (RFC 3986; %20 for spaces).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::DescribeInfo;

    fn game() -> Game {
        Game {
            title: "Test Game".into(),
            title_id: "0100faketest0000".into(),
            version: "1.0.0".into(),
            app_name: "Test Recomp".into(),
            app_version: "dev".into(),
            exe: Some("/tmp/test-app".into()),
            package: Some("/tmp/Test Game.nsp".into()),
            state: "Ready to play".into(),
            describe: Some(DescribeInfo {
                id: "fake".into(),
                title: "Test Recomp".into(),
                build_id: "abc123".into(),
                gamelib_id: "code456".into(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn env() -> Env {
        Env {
            nxrt_rev: "9c58ea9".into(),
            nxrt_dir: "/tmp/nxrt".into(),
            manager: "0.1.0".into(),
            macos: "15.0".into(),
            arch: "arm64".into(),
        }
    }

    #[test]
    fn encodes_the_query() {
        assert_eq!(enc("a b"), "a%20b");
        assert_eq!(enc("a\nb"), "a%0Ab");
        assert_eq!(enc("/tmp/x"), "%2Ftmp%2Fx");
        assert_eq!(enc("é"), "%C3%A9");
        assert!(!enc("a b\nc").contains('\n'));
    }

    #[test]
    fn body_has_the_game_the_log_and_the_environment() {
        let b = body(&game(), &env(), Some((Path::new("/tmp/fake-1.log"), "frame 1\nframe 2")));
        assert!(b.starts_with("### Game"));
        assert!(b.contains("Test Game"));
        assert!(b.contains("0100faketest0000"));
        assert!(b.contains("abc123"));
        assert!(b.contains("### What happened?"));
        assert!(b.contains("### Steps to reproduce"));
        assert!(b.contains("/tmp/fake-1.log"));
        assert!(b.contains("frame 2"));
        assert!(b.contains("nxrt: `9c58ea9`"));
        assert!(b.contains("macOS: `15.0` (arm64)"));
        assert!(b.contains("Opened from nxrt-frontend"));
        let none = body(&game(), &env(), None);
        assert!(none.contains("No manager log yet"));
    }

    #[test]
    fn url_is_a_new_issue_query() {
        let u = url(&game(), &env(), Some((Path::new("/tmp/fake-1.log"), "done")));
        assert!(u.starts_with("https://github.com/fedebuonco/nxrt/issues/new?title="));
        assert!(u.contains("%5BTest%20Game%5D%20"));
        assert!(u.contains("&body="));
        assert!(!u.contains('\n'));
        assert!(u.len() < 8000, "the URL has to fit a browser: {}", u.len());
    }

    #[test]
    fn tail_is_the_last_lines() {
        let dir = std::env::temp_dir().join(format!("nxrt-issue-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("fake-1.log");
        std::fs::write(&p, "one\ntwo\nthree\nfour\n").unwrap();
        assert_eq!(tail(&p, 2).as_deref(), Some("three\nfour"));
        assert_eq!(tail(&p, 10).as_deref(), Some("one\ntwo\nthree\nfour"));
        assert!(tail(&dir.join("missing.log"), 2).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn newest_log_for_the_app_id() {
        let dir = std::env::temp_dir().join(format!("nxrt-issue-log-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fake-1.log"), "one").unwrap();
        std::fs::write(dir.join("fake-generate-2.log"), "two").unwrap();
        std::fs::write(dir.join("other-3.log"), "three").unwrap();
        std::fs::write(dir.join("not-a-log.txt"), "x").unwrap();
        let found = latest_log(&game(), &dir).unwrap();
        assert!(found.file_name().unwrap().to_string_lossy().starts_with("fake-"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
