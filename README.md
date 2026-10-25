# nxrt-frontend

One manager for the Switch recomps built on [nxrt](https://github.com/fedebuonco/nxrt) (FE3H, EoW, LAS, Sky ...):
a library of games, how each game's files and game code stand, Play with the log alongside.

Each game stays its own app (its nxrt *runner*: `fe3hrecomp`, `eowrecomp` ...). The manager starts it; the game's app
keeps owning its settings, saves and mods.

- **Who a game is**: `<runner> --describe` (one JSON object: names, title id, game build, game-code id).
- **How it stands**: `<runner> --status` (data folder, game folder, whether the files and the game code are ready). The
  runner is the one judge of whether its game code is current: the id it wants is built into it.
- **Its files**: libnxrt_manage (`nxrt/manage/nxrt_manage.h`, compiled by `build.rs` from the nxrt submodule): exefs and
  its build id, the RomFS - an extracted `romfs/` or the game package read in place (`romfs.src`).

## Build and run (macOS)

```sh
git clone --recursive git@github.com:fedebuonco/nxrt-frontend.git
cd nxrt-frontend
cargo run --release
```

Rust 1.85 works: generate the lockfile with
`CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback cargo generate-lockfile` if a newer dependency asks for more.
`NXRT_DIR=<path>` builds against another nxrt checkout.

## Use

1. **Add a Switch game...** (or drop its `.nsp` / `.xci` on the window): the manager reads what it is from the package
   itself - name, publisher, version, icon, title id, game build - with your `prod.keys`: the one chosen in the
   toolbar (**Keys:**), else one beside the package or in `~/.switch/`. It shows up in the library as that game.
2. The game's **nxrt app** (its recomp: `fe3hrecomp` ...) is matched by title id. If none is in the library yet, the game
   waits for it: **Add an nxrt app...** or **Choose its app...**.
   No recomp for it at all? **Create a recomp...** makes one from the package: an nxrt game folder (its Makefile
   with the code modules laid out from the NSOs' sizes, `runtime/<id>_game.c` with who the game is, `build.sh`), its
   game files set up in its `data/` folder, and its app built (the generator's Python environment, then `make`). A
   plain recompilation, nothing game-specific yet: a port starts from there. Needs the Xcode command line tools,
   Python 3, Homebrew's SDL3 and FFmpeg, and an nxrt checkout (default: this app's own submodule).
3. The steps to play it, each asked for when it is next:
   - **Game files**: set up from the package (seconds: the package is read in place, nothing is copied);
   - **Generate the game code**: the game's ARM code translated to C (`<app> --generate`);
   - **Compile the game code**: that C compiled into the game's library (`<app> --compile`, reusing what was
     generated).
4. **Play** (`--play`); **Start screen** opens the game's own start screen (its settings and controls).

Each run's log is shown live and kept in `~/Library/Application Support/nxrt-frontend/logs/`.

From the command line:

```sh
nxrt-frontend --add <package.nsp | app> [--data-root <dir>]   # add a Switch game, or an nxrt app
nxrt-frontend --keys <prod.keys>                               # the keys games are read with (the toolbar's Keys)
nxrt-frontend --list                                           # every game and its steps
nxrt-frontend --step <title id | app id> files|generate|compile|all [src]
nxrt-frontend --create <title id> [--id <id>] [--dir <folder>]  # a new recomp of a game added from its package
```

The library is `~/Library/Application Support/nxrt-frontend/library.toml` (`NXRT_FRONTEND_LIBRARY` overrides it), the
games' icons beside it in `icons/`.

### What the apps answer

| Flag | What |
|---|---|
| `--describe` | the game the app runs (JSON) |
| `--status` | its data folder, game folder, whether files and game code are ready (JSON) |
| `--setup-files <pkg or folder>` | the game files only |
| `--generate` / `--compile` | the game code: C from the game's code / the library from that C |
| `--setup <pkg or folder>` | all of it |

The steps print `{"nxrt_progress":1,...}` lines and one `{"nxrt_setup":1,...}` result on stdout.

## Roadmap

See the plan: phase 0-1 (this: libnxrt_manage, `--describe` / `--status`, the manager launching the per-game apps),
2 (game-files setup and game-code builds from the manager), 3 (one generic runner), 4 (saves, mods, settings, updates),
5 (packaging).
