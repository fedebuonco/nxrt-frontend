# Architecture

nxrt-frontend is a Tauri 2 desktop app: a Rust backend (`src-tauri/`) owns the library and runs each game's own app;
the window (`src/`) is plain HTML/CSS/JavaScript with no framework, built by Vite.

## The shape now

```
src/                     the window
  index.html             the single page (sidebar, main, sheet, toasts)
  app.css                the design system
  app.js                 state, render, actions, focus engine, pad map
  js/format.js           pure text/path/time helpers
  js/icons.js            the line icons
src-tauri/src/
  main.rs                thin desktop entry (the Tauri mobile pattern)
  lib.rs                 Tauri builder, CLI, setup, command list
  state.rs               Shared: library, runs, last exits, icons, pad
  commands.rs            the window's commands + the run monitor
  api.rs                 serialisable models gathered from the game's app
  library.rs runner.rs controls.rs pads.rs issues.rs logging.rs update.rs ...
  nxnative.rs            the second backend: its submodule/checkout, its build folder, its build and run commands
  build.rs               builds libnxrt_manage from the nxrt submodule
vite.config.js           root src/, output web-dist/, dev port 1420
```

The release build: `scripts/bundle-nxrt.sh` stages the submodule into `Contents/Resources/nxrt`, Vite builds
`src/` into `web-dist/`, then Tauri embeds it in the app.

## What a canonical Tauri 2 app looks like

The `create-tauri-app` templates and the Tauri docs converge on:

- a **thin `main.rs`** and a **`lib.rs` with `run()`**, so the same app can be a mobile entry point;
- a frontend under **`src/`** built by **Vite** (React, Svelte, Vue or vanilla), with **`src-tauri/`** beside it;
- **state** managed by Tauri (`app.manage`) and **commands** in focused modules;
- **capabilities** and a **CSP** in `src-tauri`;
- **rustfmt + clippy**, ESLint + Prettier, and CI running them;
- the official **updater plugin** for signed auto-updates.

## What this pass changed

- Rust: `main.rs` is now a five-line entry; the builder, CLI and command list live in `lib.rs`; the shared state and its
  types live in `state.rs`. `cargo fmt` and `cargo clippy --all-targets -- -D warnings` are clean.
- Window: moved from `ui/` to **`src/`**, served by **Vite** in development (HMR) and built into **`web-dist/`** for the
  app. The pure helpers and icons are ES modules instead of one 2,000-line script; ESLint and Prettier run clean.
- `npm run lint`, `npm run fmt`, `npm run dev:web`, `npm run build:web` are the window's tools; `npm run dev` and
  `npm run build` keep driving Tauri.
- `tauri.conf.json` now uses `devUrl` + `beforeDevCommand` and `web-dist` + `beforeBuildCommand`, the usual Vite setup.

## Deliberate choices

- **No UI framework.** The window is a handful of screens over one state object; the focus engine and controller map are
  DOM-first. Vite + modules gives the build and structure without adding a second runtime. A framework (React/Svelte,
  TypeScript) is the next move if the screens grow, not a prerequisite.
- **String command errors.** Tauri serialises them as-is and the window shows them directly. A `thiserror` error enum
  would be nicer once commands are split.
- **A custom update check** (`update.rs` + a public manifest in the nxrt repo) instead of `tauri-plugin-updater`: the
  frontend repo is private and releases are not signed for auto-update, so the app only notifies and opens the release
  page. Moving to the official plugin needs signed artifacts and a public update endpoint.
- **One `commands.rs`.** It is cohesive (the snapshot plus the run lifecycle) at this size; the domain modules around it
  (library, runner, controls, issues, update) already carry the real logic.
- **A backend per game.** The backend is a field of the game's library entry: `api::view` dispatches to `view_nxrt` or
  `view_nxnative` (and says which backends the game offers), `commands.rs` to the matching start, and the window renders
  one switch. Everything else in the app sees a game like any other.

## Next steps, in order

1. Split `commands.rs` into `commands/library.rs`, `commands/runs.rs`, `commands/settings.rs` and `commands/updates.rs`
   behind a small `commands/mod.rs` facade; keep the handler list stable.
2. A typed `AppError` (thiserror) with `Into<String>` for Tauri, plus a few integration tests around the snapshot.
3. TypeScript (and possibly a small framework) once the window has another screen; the state object is ready for it.
4. CI: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, `npm run fmt:check`, `npm run lint`, `npm run build:web`.
5. Signed releases + `tauri-plugin-updater` if auto-update becomes a requirement.
