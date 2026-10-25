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

- **Add game...**: pick a game's app, and optionally its data folder (empty: the app's own default).
- **Play** boots the game (`--play`); **Start screen** opens the game's own start screen (its settings, game-files
  setup).
- Each run's log is shown live and kept in `~/Library/Application Support/nxrt-frontend/logs/`.

From the command line:

```sh
nxrt-frontend --add <runner> [--data-root <dir>]   # add a game
nxrt-frontend --list                               # every game and how it stands
```

The library is `~/Library/Application Support/nxrt-frontend/library.toml` (`NXRT_FRONTEND_LIBRARY` overrides it).

## Roadmap

See the plan: phase 0-1 (this: libnxrt_manage, `--describe` / `--status`, the manager launching the per-game apps),
2 (game-files setup and game-code builds from the manager), 3 (one generic runner), 4 (saves, mods, settings, updates),
5 (packaging).
