# Testing

Three layers. None needs a real game, a controller, or the real library.

| Layer        | Command            | What it covers                                                                                                                                                                                                                                    |
| ------------ | ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust backend | `cargo test`       | runs (fake runner scripts: --describe/--status, kept answers, timeouts, progress, bad UTF-8, Stop of the whole group, exit texts), adding packages (base/update pairing), the step pipeline, the library file, controls, issues, logging, updates |
| Window units | `npm test`         | `esc`/paths/`ago`/`bytes`, the icon set                                                                                                                                                                                                           |
| Window UI    | `npm run test:e2e` | Playwright: every workflow above, plus an axe accessibility pass on library/game/settings and ARIA structure snapshots of those screens                                                                                                           |

`npm run test:all` runs the two window layers. The Rust tests use the `src-tauri/` manifest.

## The UI tests

`tests/e2e/` starts Vite, opens the window in the system Chrome, and installs a fake `window.__TAURI__` before the app
loads (`tests/e2e/mock.js`). The fake answers `snapshot_command`, `pad_status` and `check_update` from
`tests/e2e/fixtures.js` (a test gives its own answers, or errors, through `installMock`'s `answers`) and records every
`invoke`. Backend events are pushed with `window.__emit(name, payload)`: run states and ends, library changes, and pad
presses (`'pad-button'`), which is how the controller map is checked without hardware. Nothing touches the real library
or starts a game.

The runner tests in `src-tauri/src/runner.rs` start small shell scripts in the temporary folder as fake nxrt apps; on
macOS the first start of a new script can take a moment (it is scanned), which is why the query timeout is 3 s under
test.

If Chrome is not installed, either change `channel` in `playwright.config.js` to `chromium` and run
`npx playwright install chromium`, or install Chrome. The tests use Playwright's web server, so `npm run test:e2e` is
self-contained.

### Accessibility

`tests/e2e/a11y.spec.js` runs axe against the library, a game page and Settings and fails on `serious`/`critical`
WCAG 2.1 A/AA findings. Animations are disabled for the scan so a fading page is not measured mid-transition.

### Semantic snapshots

`tests/e2e/semantics.spec.js` uses `toMatchAriaSnapshot()`: the accessible structure of the three screens is committed
under `tests/e2e/semantics.spec.js-snapshots/`. A UI change that alters roles, labels or control order shows up as a
reviewable diff instead of a silent regression. Update intentionally with:

```sh
npx playwright test tests/e2e/semantics.spec.js --update-snapshots
```

### Failed UI runs

On CI, Playwright keeps a trace and screenshot of a failure (`test-results/`), and the workflow uploads them as
the `ui-failure-artifacts` artifact. Locally, open a trace with `npx playwright show-trace test-results/.../trace.zip`.

## CI

`.github/workflows/ci.yml` has two jobs. **Window UI** runs `npm run test:e2e` on Ubuntu (system Chrome) and uploads
failure traces; it runs for `main`, `nxnative` and pull requests. **macOS ·
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
