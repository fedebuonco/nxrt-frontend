//! A GitHub issue for a game, prefilled with what identifies the game, its app and the machine: ids, versions,
//! builds, macOS. Nothing of the player's own - no paths, no log (they may name the user's folders); the player
//! attaches a log themselves if they want to. They review it on GitHub before submitting; nothing is sent by the
//! manager itself.
use crate::api::Game;
use std::path::Path;
use std::process::Command;

pub const NEW_ISSUE: &str = "https://github.com/fedebuonco/nxrt/issues/new";
const MAX_BODY: usize = 4200;

pub struct Env {
    pub nxrt_rev: String,
    pub manager: String,
    pub macos: String,
    pub arch: String,
}

impl Env {
    pub fn detect(nxrt_dir: &Path) -> Env {
        let rev = std::fs::read_to_string(nxrt_dir.join(".nxrt-revision"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| git_rev(nxrt_dir))
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
            manager: env!("CARGO_PKG_VERSION").into(),
            macos,
            arch: if std::env::consts::ARCH == "aarch64" {
                "arm64".into()
            } else {
                std::env::consts::ARCH.into()
            },
        }
    }
}

/// The checkout's HEAD, but only when it is that checkout's own repository (not the app repo around a staged copy).
fn git_rev(dir: &Path) -> Option<String> {
    let top = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !top.status.success() {
        return None;
    }
    let top = std::fs::canonicalize(String::from_utf8_lossy(&top.stdout).trim()).ok()?;
    if top != std::fs::canonicalize(dir).ok()? {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let rev = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!rev.is_empty()).then_some(rev)
}

/// The prefilled URL for a game, from the one-gather view the caller built.
pub fn for_game(g: &Game, nxrt_dir: &Path) -> String {
    url(g, &Env::detect(nxrt_dir))
}

pub fn url(g: &Game, env: &Env) -> String {
    let title = if g.title.trim().is_empty() {
        "nxrt".to_string()
    } else {
        g.title.trim().to_string()
    };
    format!(
        "{NEW_ISSUE}?title={}&body={}",
        enc(&format!("[{title}] ")),
        enc(&body(g, env))
    )
}

fn body(g: &Game, env: &Env) -> String {
    let mut s = String::new();
    s.push_str("### Game\n\n| | |\n|---|---|\n");
    row(&mut s, "Title", &g.title);
    row(&mut s, "Title ID", &g.title_id);
    row(&mut s, "Game version", &g.version);
    if g.update.is_some() {
        row(&mut s, "Update", "yes");
    }
    row(&mut s, "App", &format!("{} v{}", g.app_name, g.app_version));
    if let Some(d) = &g.describe {
        row(&mut s, "Game build", &d.build_id);
        row(&mut s, "Game code", &d.gamelib_id);
    }
    row(&mut s, "State", &g.state);
    if g.build_mismatch {
        row(&mut s, "Build mismatch", "the package and the app differ");
    }
    if let Some(e) = &g.last_exit {
        row(&mut s, "Last run", &e.text);
    }

    s.push_str("\n### What happened?\n<!-- What did you do, what did you expect, and what happened instead? -->\n\n");
    s.push_str("\n### Steps to reproduce\n1. \n2. \n\n");
    s.push_str("### Logs\n<!-- Optional: drag the game's log here (the manager's Logs page opens their folder). -->\n\n");
    s.push_str("\n### Environment\n");
    s.push_str(&format!("- nxrt: `{}`\n", env.nxrt_rev));
    s.push_str(&format!("- nxrt-frontend: `{}`\n", env.manager));
    s.push_str(&format!("- macOS: `{}` ({})\n", env.macos, env.arch));
    s.push_str("\n<!-- Opened from nxrt-frontend. -->\n");
    if s.chars().count() > MAX_BODY {
        s = s.chars().take(MAX_BODY).collect();
    }
    s
}

fn row(s: &mut String, key: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    s.push_str(&format!(
        "| **{}** | {} |\n",
        key,
        value.replace('|', "\\|").replace('\n', " ")
    ));
}

/// Percent-encode a query value (RFC 3986; %20 for spaces).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 4);
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
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
    fn body_has_the_game_and_the_environment_and_no_paths() {
        let mut g = game();
        g.update = Some("/Users/someone/Games/Test Update.nsp".into());
        g.recomp_dir = Some("/Users/someone/recomps/test".into());
        g.data_root = Some("/Users/someone/data".into());
        g.load_error = Some("/Users/someone/apps/test doesn't answer".into());
        let b = body(&g, &env());
        assert!(b.starts_with("### Game"));
        assert!(b.contains("Test Game"));
        assert!(b.contains("0100faketest0000"));
        assert!(b.contains("abc123"));
        assert!(b.contains("| **Update** | yes |"));
        assert!(b.contains("### What happened?"));
        assert!(b.contains("### Steps to reproduce"));
        assert!(b.contains("nxrt: `9c58ea9`"));
        assert!(b.contains("macOS: `15.0` (arm64)"));
        assert!(b.contains("Opened from nxrt-frontend"));
        for path in ["/tmp", "/Users", "someone"] {
            assert!(!b.contains(path), "a path in the issue: {b}");
        }
    }

    #[test]
    fn url_is_a_new_issue_query() {
        let u = url(&game(), &env());
        assert!(u.starts_with("https://github.com/fedebuonco/nxrt/issues/new?title="));
        assert!(u.contains("%5BTest%20Game%5D%20"));
        assert!(u.contains("&body="));
        assert!(!u.contains('\n'));
        assert!(!u.contains("%2Ftmp"), "no paths");
        assert!(u.len() < 8000, "the URL has to fit a browser: {}", u.len());
    }
}
