# nxrt-frontend

One manager for the Switch recomps built on [nxrt](https://github.com/fedebuonco/nxrt) (FE3H, EoW, LAS, Sky ...):
a library of games, how each game's files and game code stand, Play with the log alongside.

It is a **Tauri 2** app: a Rust backend (`src-tauri/`) that owns the library and the games' own apps, and a static web
window (`ui/`) made of plain HTML, CSS and JavaScript — system SF type, one accent, a focus ring a controller can drive.

Each game stays its own app (its nxrt *runner*: `fe3hrecomp`, `eowrecomp` ...). The manager starts it; the game's app
keeps owning its settings, saves and mods.

- **Who a game is**: `<runner> --describe` (one JSON object: names, title id, game build, game-code id).
- **How it stands**: `<runner> --status` (data folder, game folder, whether the files and the game code are ready). The
  runner is the one judge of whether its game code is current: the id it wants is built into it.
- **Its files**: libnxrt_manage (`nxrt/manage/nxrt_manage.h`, compiled by `src-tauri/build.rs` from the nxrt submodule):
  exefs and its build id, the RomFS - an extracted `romfs/` or the game package read in place (`romfs.src`).

## The window

- **Library**: a grid of the games, each with its cover, version, state, and the one next thing (Play, Set up, Create).
- **A game**: the cover and title, Play / Start screen, the four steps from a package to a game that plays, where its
  files are, its live log, and (folded away) details and a Dev build command.
- **Logs**: every run the manager started, with when and how big.
- **Settings**: the keys file, controller status and map, where new recomps go, and the manager's own folder.

One mint accent, three levels of text, hairline cards on a graphite ground. Focus is a mint ring; the controller hints
are at the foot of the window.

## Build and run (macOS)

Requirements: macOS 11 or newer, **Rust stable via rustup (1.90 or newer)**, Node 20+, the Xcode command line tools,
and the `nxrt` submodule.

```sh
git clone --recursive git@github.com:fedebuonco/nxrt-frontend.git
cd nxrt-frontend
npm install
npm run dev       # tauri dev: builds the Rust app and opens the window
```

`npm run build` makes an app bundle in `src-tauri/target/release/bundle/macos/` (unsigned: right-click → Open the first
time). A local arm64 build is kept in [`dist/`](dist) — `nxrt-0.1.0-arm64.dmg`, ad-hoc signed and not notarized.

The release build stages the nxrt submodule into the app (`Contents/Resources/nxrt`), so creating a recomp needs no
checkout. `NXRT_DIR=<path>` at run time overrides that copy with another checkout (development). `npm install` fetches
the Tauri CLI and nothing else: the window has no bundler.

The window's files are embedded when the Rust app is built, so **restart `npm run dev` after editing `ui/`** (a plain
window reload is not enough).

## Use

1. **Add a Switch game...** (or drop its `.nsp` / `.xci` on the window): the manager reads what it is from the package
   itself - name, publisher, version, icon, title id, game build - with your `prod.keys`: the one chosen in
   **Settings → Keys**, else one beside the package or in `~/.switch/`. It shows up in the library as that game.
2. The game's **nxrt app** (its recomp: `fe3hrecomp` ...) is matched by title id. If none is in the library yet, the game
   waits for it: **Add an nxrt app...** or **Choose its app...**.
   No recomp for it at all? **Create a recomp...** makes one from the package: an nxrt game folder (its Makefile with the
   code modules laid out from the NSOs' sizes, `runtime/<id>_game.c` with who the game is, `build.sh`), its game files set
   up in its `data/` folder, and its app built (the generator's Python environment, then `make`). A plain recompilation,
   nothing game-specific yet: a port starts from there. Needs Python 3, Homebrew's SDL3 and FFmpeg; the nxrt source is
   built into the app.
3. The steps to play it, each asked for when it is next:
   - **Game files**: set up from the package (seconds: the package is read in place, nothing is copied);
   - **Generate the game code**: the game's ARM code translated to C (`<app> --generate`);
   - **Compile the game code**: that C compiled into the game's library (`<app> --compile`, reusing what was generated).
4. **Play** (`--play`); **Start screen** opens the game's own start screen (its settings and controls).
5. **Report an issue…** on a game's page opens the nxrt GitHub new-issue form with the game, its app and build, the
   latest manager log (a tail of it) and the environment laid out. Review it, add what happened, submit.

Each run's log is shown live and kept in `~/Library/Application Support/nxrt-frontend/logs/`.

## Controller and keyboard

Any standard controller works over USB or Bluetooth — Xbox, DualSense, Switch Pro, 8BitDo — read with `gilrs` in the
Rust backend while the manager's window has focus. The labels in the window follow the pad you have: Nintendo's bottom
button is B, Xbox's is A, PlayStation's is ✕.

| | |
|---|---|
| D-pad / left stick | move the focus ring |
| bottom button (A, ✕, B) | select |
| right button (B, ○, A) | back |
| left button (X, □, Y) | Play / Stop the focused game |
| top button (Y, △, X) | the game's page, or its Details |
| LB / RB | Library, Logs, Settings |
| LT / RT | scroll the page |
| Start | Play |
| Select / Back | the controls sheet |

The hints bar at the foot of the window always shows the current map; **Select** opens the full one, and Settings has the
map too. D-pad navigation is spatial: in the grid it moves between cards, and on a game page the first focus is the main
action (Play, Stop, or the next step).

If a connected pad isn't seen, allow **Input Monitoring** for nxrt in System Settings › Privacy & Security (macOS gates
HID access). Controller input is ignored whenever the manager's window is not the one in front, so a pad used by a
running game does not also drive the manager.

The same moves work from the keyboard: **arrows** to move, **Enter** / **Space** to select, **Esc** to go back, **[** and
**]** to change section. With no controller the hints bar stays hidden; Settings still shows the map.

## Controller settings: for every game, or one

**Settings → Controller defaults** holds the bindings and input preferences passed to every game when it starts. A game's
own page has a **Controls** disclosure; rows there override the defaults for that game only. Both are sparse: a row left
on **Game's own** (defaults) or **Use defaults** (a game) leaves the game's setting alone, and a row set in the manager
wins over the game's own for that key. With a controller, focus a row and use **← →** (or **A**) to change it; with a
mouse or keyboard, use its selector.

The manager does not edit the game's `nx_app.ini`. Before a launch it writes the merged settings — the defaults, then the
game's overrides — to `~/Library/Application Support/nxrt-frontend/controls/<game>.conf`, and starts the game with
`NXRT_CONTROLLER=<that file>`. The game's input layer reads it after its own settings, so the manager's rows are the
effective ones (the game's start screen shows them). A row the manager does not set stays the game's.

The overlay is read by the `nxrt` submodule (`input/input.c`, `input_loaded`). A game app built before that revision
ignores `NXRT_CONTROLLER`; rebuild the app to pick it up.

## Logging

The manager writes its own log to `~/Library/Application Support/nxrt-frontend/logs/manager.log.YYYY-MM-DD` (daily,
seven kept) and every run keeps its own log in the same folder. Set `NXRT_FRONTEND_LOG=debug` (or
`run=trace,api=debug`) for more; `NXRT_GAME_LOG=debug` passes a level to the game's nxrt logging. A run id is put in
both: the manager's `run started/finished` lines and nxrt's `[run=…]`. **Logs → Manager** shows the manager log with
level filters and Follow; **Settings → Diagnostics** shows the level and path. See [docs/LOGGING.md](docs/LOGGING.md)
and nxrt's [docs/LOGGING.md](nxrt/docs/LOGGING.md).

```sh
NXRT_FRONTEND_LOG=debug npm run dev
NXRT_GAME_LOG=debug npm run dev
```

## Where things are

The library is `~/Library/Application Support/nxrt-frontend/library.toml` (`NXRT_FRONTEND_LIBRARY` overrides it), the
games' icons beside it in `icons/`, and the runs' logs in `logs/`. New recomps go in `recomps/` by default.

The manager never edits a game's settings or saves: the game's own app owns them. Controller settings are passed as an
overlay at launch (see above), not written into the game's file. Removing a game from the library removes it from this
list only; nothing on disk is deleted.

## Command line

Build the app once (`npm run dev` or `npm run build`), then use the binary directly — the flags print and exit without
opening a window:

```sh
nxrt-frontend --add <package.nsp | app> [--data-root <dir>]   # add a Switch game, or an nxrt app
nxrt-frontend --keys <prod.keys>                               # the keys games are read with
nxrt-frontend --list                                           # every game and its steps
nxrt-frontend --step <title id | app id> files|generate|compile|all [src]
nxrt-frontend --issue <title id | app id>                      # print the prefilled nxrt GitHub issue URL
nxrt-frontend --create <title id> [--id <id>] [--dir <folder>]  # a new recomp of a game added from its package
```

`cargo run --manifest-path src-tauri/Cargo.toml -- --list` works too. `--issue` prints the URL instead of opening it; a
game's page has the **Report an issue…** button that opens it.

### Screenshots and checks

`NXRT_FRONTEND_VIEW=library|logs|settings|game:<n>|sheet:add|sheet:create:<n>` opens that section (or sheet) at launch.
`NXRT_FRONTEND_VIEW=pad:library|pad:game:<n>|pad:settings|pad:help` previews the controller focus ring and hints without
a controller (a simulated Xbox pad). macOS screenshots of the window: `screencapture -l <window id> shot.png`.

### What the apps answer

| Flag | What |
|---|---|
| `--describe` | the game the app runs (JSON) |
| `--status` | its data folder, game folder, whether files and game code are ready (JSON) |
| `--setup-files <pkg or folder>` | the game files only |
| `--generate` / `--compile` | the game code: C from the game's code / the library from that C |
| `--setup <pkg or folder>` | all of it |

The steps print `{"nxrt_progress":1,...}` lines and one `{"nxrt_setup":1,...}` result on stdout.

## Layout of the source

| | |
|---|---|
| `ui/` | the window: `index.html`, `app.css`, `app.js` (views, focus engine, pad map, sheets) |
| `dist/` | a local, unsigned arm64 build and its notes |
| `src-tauri/src/main.rs` | the builder, the command-line switches, the Tauri command list |
| `src-tauri/src/commands.rs` | library/run/step commands, the thread that follows a run and emits its log |
| `src-tauri/src/api.rs` | the serialisable view of a game, gathered from its app in parallel |
| `src-tauri/src/pads.rs` | controllers: gilrs → `pad-button` / `pad-dir` / `pad-trigger` / `pad-status` events |
| `src-tauri/src/controls.rs` | controller defaults + per-game settings, and the `NXRT_CONTROLLER` overlay |
| `src-tauri/src/issues.rs` | a game's prefilled nxrt GitHub issue (details, log tail, environment) |
| `scripts/bundle-nxrt.sh` | stages the nxrt submodule into the app bundle for a release build |
| `src-tauri/src/library.rs` | the library file and its `Entry`s |
| `src-tauri/src/runner.rs` | running a game or a step, its log and progress |
| `src-tauri/src/manage.rs` | packages, exefs, RomFS through libnxrt_manage |
| `src-tauri/src/recomp.rs` | a new nxrt game folder for a game that has none |
| `src-tauri/src/ops.rs` | the operations the window and the command line share |
| `src-tauri/icons/` | the app icon (generated from `assets/icon.png`) |
| `nxrt/` | the nxrt submodule |

## Roadmap

See the plan: phase 0-1 (libnxrt_manage, `--describe` / `--status`, the manager launching the per-game apps), 2
(game-files setup and game-code builds from the manager), 3 (one generic runner), 4 (saves, mods, settings, updates),
5 (packaging). The window and controller pass came with phase 2.
