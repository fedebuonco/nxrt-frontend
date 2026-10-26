//! The nxnative backend (github.com/seedds/nxnative, MIT): a game runs natively in nxnative's host, built for this Mac
//! from its NSP folder by nxnative's own script (`scripts/build_ipa_from_nsp.py --platform mac`). Everything the build
//! makes is in `<NSP folder>/<title ID>_build`; the script ends by printing the command that runs the game, which the
//! library keeps for Play.
use crate::library::Entry;
use std::path::{Path, PathBuf};

/// Where the nxnative checkout is: NXNATIVE_DIR, else the library's setting, else the SSD's development volume.
pub fn repo(setting: Option<&Path>) -> PathBuf {
    if let Some(d) = std::env::var_os("NXNATIVE_DIR") {
        return PathBuf::from(d);
    }
    setting
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/Volumes/FE3HDev/nxnative"))
}

/// The Python the build runs with: the checkout's own environment (lz4, capstone, fonttools), else python3.
pub fn python(repo: &Path) -> PathBuf {
    let venv = repo.join(".venv/bin/python");
    if venv.is_file() {
        venv
    } else {
        PathBuf::from("python3")
    }
}

/// The PATH the build runs with: the app's own, then where Homebrew and the .NET installer put the tools the script
/// needs (dotnet, spirv-cross, glslangValidator) - an app opened from Finder has only /usr/bin:/bin:/usr/sbin:/sbin.
pub fn tool_path() -> String {
    let mut dirs: Vec<String> = std::env::var("PATH")
        .unwrap_or_default()
        .split(':')
        .filter(|d| !d.is_empty())
        .map(String::from)
        .collect();
    for d in [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/usr/local/share/dotnet",
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
    ] {
        if !dirs.iter().any(|x| x == d) {
            dirs.push(d.to_string());
        }
    }
    dirs.join(":")
}

/// The game's NSP folder: the folder of its package (nxnative builds from a folder holding one game's NSPs).
pub fn nsp_dir(e: &Entry) -> Option<PathBuf> {
    let p = e.package.as_deref().or(e.update.as_deref())?;
    if p.is_dir() {
        Some(p.to_path_buf())
    } else {
        p.parent().map(Path::to_path_buf)
    }
}

/// The base game's title ID as nxnative names its folders and profiles (16 upper-case hex digits).
pub fn title_id(e: &Entry) -> Option<String> {
    let v = u64::from_str_radix(e.title_id.trim(), 16).ok()?;
    Some(format!("{:016X}", v & !0xFFF))
}

/// `<NSP folder>/<title ID>_build`
pub fn work_dir(e: &Entry) -> Option<PathBuf> {
    Some(nsp_dir(e)?.join(format!("{}_build", title_id(e)?)))
}

/// What is there of a game's nxnative build.
#[derive(Clone, Debug, Default)]
pub struct Probe {
    /// nxnative has a profile for the game (`dm/games/<title ID>/game.tsv`): a supported game
    pub profile: bool,
    /// the first line of the profile's note under its title (versions, SDK, engine)
    pub note: String,
    /// the game files are extracted (`extract/`)
    pub extracted: bool,
    /// the code modules are rewritten (`patched_*/`)
    pub patched: bool,
    /// the Mac host for the game is built (`tools/dmhost`)
    pub host: bool,
    pub work: Option<PathBuf>,
}

pub fn probe(e: &Entry, repo: &Path) -> Probe {
    let mut p = Probe::default();
    if let Some(tid) = title_id(e) {
        let prof = repo.join("dm/games").join(&tid);
        p.profile = prof.join("game.tsv").is_file();
        if let Ok(text) = std::fs::read_to_string(prof.join("README.md")) {
            p.note = text
                .lines()
                .skip(1)
                .find(|l| !l.trim().is_empty())
                .unwrap_or("")
                .chars()
                .take(240)
                .collect();
        }
    }
    if let Some(w) = work_dir(e) {
        p.extracted = w.join("extract").is_dir();
        p.patched = std::fs::read_dir(&w)
            .map(|d| {
                d.flatten()
                    .any(|x| x.file_name().to_string_lossy().starts_with("patched_"))
            })
            .unwrap_or(false);
        p.host = w.join("tools/dmhost").is_file();
        p.work = Some(w);
    }
    p
}

/// The message the script prints before the command that runs the game (build_ipa_from_nsp.py, a Mac-only build).
#[cfg(test)]
mod tests {
    #[test]
    fn the_tools_path_has_homebrew_and_dotnet() {
        let p = super::tool_path();
        for d in ["/opt/homebrew/bin", "/usr/local/share/dotnet", "/usr/bin"] {
            assert!(p.split(':').any(|x| x == d), "{p}");
        }
    }
}

pub const RUN_COMMAND_FOLLOWS: &str = "done: the game runs on the Mac with this command";
