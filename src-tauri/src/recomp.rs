//! A new recomp for a game that has none: an nxrt game folder written from the game's own package (who it is, its
//! code modules and where each loads), its game files set up from the package, and its app built - after which the
//! game's usual steps (Generate, Compile, Play) apply. The same start as any port on nxrt (nxrt/examples/game-template),
//! with nothing game-specific yet: a plain recompilation.
use crate::library::Entry;
use std::path::{Path, PathBuf};

/// Where the modules may go: GUEST_LO up to HLE_BASE (nxrt/runtime/rt.h)
const GUEST_LO: u64 = 0x70_0000_0000;
const HLE_BASE: u64 = 0x70_8000_0000;
const MODULE_ALIGN: u64 = 0x200_0000;  // (32 MiB, as the ports lay them out)

/// The nxrt checkout new recomps build against: NXRT_DIR at build time, else this app's own nxrt submodule.
pub fn default_nxrt() -> PathBuf {
    let p = PathBuf::from(option_env!("NXRT_DIR").unwrap_or(concat!(env!("CARGO_MANIFEST_DIR"), "/../nxrt")));
    p.canonicalize().unwrap_or(p)
}

pub fn default_dir(id: &str) -> PathBuf {
    crate::library::dir().join("recomps").join(id)
}

/// A short id from the game's name: the initials of its words ("Fire Emblem: Three Houses" -> "feth"), lower case.
pub fn default_id(name: &str) -> String {
    let words: Vec<&str> = name.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let id: String = if words.len() >= 2 {
        words.iter().filter_map(|w| w.chars().next()).collect()
    } else {
        words.concat()
    };
    let id = id.to_ascii_lowercase();
    if id.is_empty() { "game".into() } else { id }
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 24 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()) && id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
}

/// An NSO's size in memory: its data segment's end plus bss (header: data {file, mem, size} at 0x30, bss at 0x3C).
fn nso_mem_size(path: &Path) -> Option<u64> {
    let h = std::fs::read(path).ok()?;
    if h.len() < 0x40 || &h[0..4] != b"NSO0" {
        return None;
    }
    let u = |o: usize| u32::from_le_bytes(h[o..o + 4].try_into().unwrap()) as u64;
    Some(u(0x34) + u(0x38) + u(0x3C))
}

/// The game's code modules in rtld's order (main, subsdk0-9, sdk) and where each loads: one after another from
/// GUEST_LO, each at a 32 MiB boundary.
pub fn layout(exefs: &Path) -> Result<Vec<(String, u64)>, String> {
    let mut names = vec!["main".to_string()];
    names.extend((0..10).map(|i| format!("subsdk{i}")));
    names.push("sdk".into());
    let mut out = Vec::new();
    let mut at = GUEST_LO;
    for n in names {
        let p = exefs.join(&n);
        if !p.is_file() {
            continue;
        }
        let size = nso_mem_size(&p).ok_or_else(|| format!("{}: not an NSO", p.display()))?;
        out.push((n, at));
        at = (at + size + MODULE_ALIGN - 1) / MODULE_ALIGN * MODULE_ALIGN;
    }
    if out.first().is_none_or(|m| m.0 != "main") {
        return Err(format!("{}: no main", exefs.display()));
    }
    if at > HLE_BASE {
        return Err(format!("the game's modules need {} MB: more than nxrt's guest space", (at - GUEST_LO) >> 20));
    }
    Ok(out)
}

fn c_str(s: &str) -> String {
    s.chars().filter(|c| *c != '"' && *c != '\\' && !c.is_control()).collect()
}

/// The recomp's files. Its game files go to <dir>/data/extracted (set up from the package by the caller), with
/// <dir>/extracted pointing there (what `make gen` reads).
pub fn scaffold(dir: &Path, id: &str, e: &Entry, modules: &[(String, u64)], nxrt: &Path) -> Result<(), String> {
    let w = |rel: &str, text: String| -> Result<(), String> {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).map_err(|e| format!("{}: {e}", p.display()))?;
        std::fs::write(&p, text).map_err(|e| format!("{}: {e}", p.display()))
    };
    let mods: Vec<String> = modules.iter().map(|(n, b)| format!("{n}:{b:#x}")).collect();
    let name = c_str(&e.name);
    let bid: Vec<String> = (0..8).map(|i| format!("0x{}", e.build_id.get(i * 2..i * 2 + 2).unwrap_or("00"))).collect();
    w("Makefile", format!(
        "# {name} on nxrt: who the game is (runtime/{id}_game.c) + the shared build rules. Made by nxrt-frontend from the\n\
         # game's own package: a plain recompilation, nothing game-specific yet. NXRT=<path> builds against another checkout.\n\
         NXRT     ?= {nxrt}\n\
         GAME_ID  := {id}\n\
         GAME_EXE := {id}recomp\n\
         GAME_MODULES := {mods}\n\
         GAME_SRC := $(wildcard runtime/*.c) $(wildcard runtime/hle/*.c) $(wildcard runtime/ui/*.mm)\n\
         include $(NXRT)/nxrt.mk\n",
        nxrt = nxrt.display(), mods = mods.join(" ")))?;
    let modlist: Vec<String> = modules.iter().map(|(n, b)| format!("{{\"{n}\", {b:#x}ull}}")).collect();
    let upper = id.to_ascii_uppercase();
    w(&format!("runtime/{id}_game.c"), format!(
        "// {name} on nxrt: who the game is. Made by nxrt-frontend from the game's package; no hooks yet (a plain\n\
         // recompilation). Hooks (runtime/nxgame.h) go here as the port needs them.\n\
         #include \"nxgame.h\"\n\n\
         const NxGame nx_game = {{\n\
         \x20   .info = {{\n\
         \x20       .id = \"{id}\",\n\
         \x20       .exe = \"{id}recomp\",\n\
         \x20       .title = \"{upper} Recomp\",\n\
         \x20       .game_name = \"{name}\",\n\
         \x20       .game_version = \"{ver}\",\n\
         \x20       .title_id = 0x{tid}ull,\n\
         \x20       .build_id = {{{bid}}},  // exefs/main +0x40 (v{ver})\n\
         \x20       .bundle_id = \"org.nxrt.{id}recomp\",\n\
         \x20       .data_dir = \"{id}recomp\",\n\
         \x20       .ini = \"{id}.ini\",\n\
         \x20       .version = \"dev\",\n\
         \x20       .headline = \"{name}\",\n\
         \x20       .romfs_probe = \"\",\n\
         \x20       .modules = {{{modlist}}},  // (as the Makefile's GAME_MODULES)\n\
         \x20   }},\n\
         }};\n",
        ver = c_str(&e.version), tid = e.title_id, bid = bid.join(", "), modlist = modlist.join(", ")))?;
    w("tools/patches.py", "\"\"\"The game's own code changes, applied by tools/gen.py (--game tools).\n\n\
        PATCHES: guest address -> function(gen, addr) returning the C that replaces that one instruction.\n\
        VARIANTS (optional): main-module offset -> instruction words the runtime may pick instead at run time.\n\"\"\"\n\
        PATCHES = {}\nVARIANTS = {}\n".into())?;
    w(".gitignore", "# the player's game files and what is made from them\ndata/\nextracted\ngenerated/\nbuild/\ntools/venv/\n__pycache__/\n*.log\n.DS_Store\n".into())?;
    w("README.md", format!(
        "# {name} on nxrt\n\nA recomp of *{name}* (v{ver}, {tid}) made by nxrt-frontend from the game's own package: a plain \
         recompilation on [nxrt]({nxrt}), nothing game-specific yet. No game files or generated code belong in a repository.\n\n\
         - `runtime/{id}_game.c`: who the game is (and its hooks, as the port needs them)\n\
         - `Makefile`: its code modules ({mods})\n\
         - `data/`: its data folder (`data/extracted`: the game files, the RomFS read from the package in place)\n\n\
         ```sh\nmake venv && make            # the app\nbuild/{id}recomp --generate && build/{id}recomp --compile   # its game code\n```\n",
        ver = c_str(&e.version), tid = e.title_id.to_uppercase(), nxrt = nxrt.display(), mods = mods.join(" ")))?;
    // the build, as nxrt-frontend runs it (::step lines: its progress)
    w("build.sh", format!(
        "#!/bin/sh\n# Builds this recomp's app (nxrt-frontend runs it; the game code is the app's own --generate / --compile).\n\
         set -e\ncd \"$(dirname \"$0\")\"\n\
         [ -x /usr/bin/clang ] && export CLANG=/usr/bin/clang\n\
         command -v xcrun >/dev/null 2>&1 && export SDKROOT=\"$(xcrun --show-sdk-path)\"\n\
         echo '::step 1/2 The generator'\"'\"'s Python environment (capstone, lz4, unicorn)'\n\
         make venv\n\
         echo '::step 2/2 Building the app ({id}recomp)'\n\
         make -j\"$(sysctl -n hw.ncpu 2>/dev/null || nproc)\" build/{id}recomp\n\
         echo '::step done'\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir.join("build.sh"), std::fs::Permissions::from_mode(0o755));
    }
    let link = dir.join("extracted");
    if std::fs::symlink_metadata(&link).is_err() {
        #[cfg(unix)]
        std::os::unix::fs::symlink("data/extracted", &link).map_err(|e| format!("{}: {e}", link.display()))?;
    }
    Ok(())
}

/// The app the recomp's build makes.
pub fn app_path(dir: &Path, id: &str) -> PathBuf {
    dir.join("build").join(format!("{id}recomp"))
}
