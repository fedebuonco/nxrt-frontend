# Testing

Three layers. None needs a real game, a controller, or the real library.

| Layer        | Command            | What it covers                                                                               |
| ------------ | ------------------ | -------------------------------------------------------------------------------------------- |
| Rust backend | `cargo test`       | controls merge/validation, issue bodies, library files, logging, updates, API keys/paths     |
| Window units | `npm test`         | `esc`/paths/`ago`/`bytes`, the icon set                                                      |
| Window UI    | `npm run test:e2e` | Playwright: library, a game + issue, the controller map's Test mode, the update chip, Donate |

`npm run test:all` runs the two window layers. The Rust tests use the `src-tauri/` manifest.

## The UI tests

`tests/e2e/` starts Vite, opens the window in the system Chrome, and installs a fake `window.__TAURI__` before the app
loads (`tests/e2e/mock.js`). The fake answers `snapshot_command`, `pad_status` and `check_update` from
`tests/e2e/fixtures.js` and records every `invoke`; pad presses are pushed with `window.__emit('pad-button', …)`, which
is how the controller map is checked without hardware. Nothing touches the real library or starts a game.

If Chrome is not installed, either change `channel` in `playwright.config.js` to `chromium` and run
`npx playwright install chromium`, or install Chrome. The tests use Playwright's web server, so `npm run test:e2e` is
self-contained.

## CI

`.github/workflows/ci.yml` has two jobs. **Window UI** runs `npm run test:e2e` on Ubuntu (system Chrome). **macOS ·
checks + DMG** runs on an Apple-silicon runner: the Rust fmt/clippy/test checks, the window lint/format/unit checks,
`npm run build`, `npm run dmg`, and uploads the DMG plus `SHA256SUMS` as the `nxrt-dmg` artifact. A `v*` tag also
attaches those files to the GitHub release.

## Checks

```sh
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run lint
npm run fmt:check
npm run test:all
```

The C side of nxrt has its own host tests in `nxrt/tests/run.sh` (settings, saves, parsers, texturing; no game or GPU).
