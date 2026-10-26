//! The frontend update check: a small public JSON manifest in the nxrt repo, fetched with curl (no token, no
//! dependency). The window asks at start and when the user clicks Check now; a newer version shows a sidebar chip.
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const DEFAULT_URL: &str = "https://raw.githubusercontent.com/fedebuonco/nxrt/macos-core/frontend/latest.json";
const TTL: Duration = Duration::from_secs(6 * 3600);

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub available: bool,
    pub current: String,
    pub latest: String,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    notes: Option<String>,
}

static CACHE: Mutex<Option<(Instant, UpdateInfo)>> = Mutex::new(None);

pub fn current() -> String {
    env!("CARGO_PKG_VERSION").into()
}

pub fn failed(message: String) -> UpdateInfo {
    UpdateInfo {
        available: false,
        current: current(),
        latest: String::new(),
        url: None,
        notes: None,
        error: Some(message),
    }
}

pub fn check(force: bool) -> UpdateInfo {
    let now = Instant::now();
    if !force {
        if let Some((at, info)) = CACHE.lock().unwrap().as_ref() {
            if now.duration_since(*at) < TTL {
                return info.clone();
            }
        }
    }
    let info = fetch();
    *CACHE.lock().unwrap() = Some((now, info.clone()));
    info
}

fn fetch() -> UpdateInfo {
    let url = std::env::var("NXRT_FRONTEND_UPDATE_URL").unwrap_or_else(|_| DEFAULT_URL.into());
    if !(url.starts_with("https://") || url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost")) {
        return failed(format!("update URL is not https (or a local test URL): {url}"));
    }
    let out = std::process::Command::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--location",
            "--max-time",
            "8",
            "--user-agent",
            concat!("nxrt-frontend/", env!("CARGO_PKG_VERSION")),
            &url,
        ])
        .output();
    let out = match out {
        Ok(o) => o,
        Err(e) => return failed(format!("can't run curl: {e}")),
    };
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return failed(if msg.is_empty() { format!("update check failed ({})", out.status) } else { msg });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let manifest: Manifest = match serde_json::from_str(&text) {
        Ok(m) => m,
        Err(e) => return failed(format!("update manifest: {e}")),
    };
    let latest = manifest.version.trim().trim_start_matches('v').to_string();
    let available = newer(&latest, current().as_str());
    UpdateInfo {
        available,
        current: current(),
        latest,
        url: manifest.url,
        notes: manifest.notes,
        error: None,
    }
}

fn parts(version: &str) -> Option<(u64, u64, u64)> {
    let version = version.trim().trim_start_matches('v');
    let core = version.split(['-', '+']).next()?;
    let mut it = core.split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it.next().unwrap_or("0").parse().ok()?;
    Some((major, minor, patch))
}

fn newer(latest: &str, current: &str) -> bool {
    match (parts(latest), parts(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_versions_compare() {
        assert!(newer("0.1.1", "0.1.0"));
        assert!(newer("v0.2.0", "0.1.9"));
        assert!(!newer("0.1.0", "0.1.0"));
        assert!(!newer("0.0.9", "0.1.0"));
        assert!(!newer("garbage", "0.1.0"));
        assert!(newer("1.0", "0.9.9"));
    }

    #[test]
    fn failures_are_not_updates() {
        let f = failed("offline".into());
        assert!(!f.available);
        assert_eq!(f.error.as_deref(), Some("offline"));
        assert_eq!(f.current, env!("CARGO_PKG_VERSION"));
    }
}
