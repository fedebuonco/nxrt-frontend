# Manager logging

The manager (nxrt-frontend) has its own log, separate from each run's log. It is a `tracing` subscriber with a file
layer (a daily file, no ANSI), a stderr layer (ANSI), and an env filter. A run gets an id (`NXRT_RUN_ID`) shared with
the game, so the manager's `run started` / `run finished` lines and nxrt's `[run=…]` lines line up.

## Plan and status

- One manager log, daily, seven kept; the run logs keep their own files.
- Levels and targets via `NXRT_FRONTEND_LOG` (or `RUST_LOG`); default `info`.
- A run id per game/step, passed to nxrt; `NXRT_GAME_LOG` selects the game's nxrt level.
- A Manager card in Logs (levels, search, follow) and a Diagnostics card in Settings.
- Webview errors (window `error` / `unhandledrejection`) go through `log_ui` into the same log.
- The existing run logs stay the place for a run's own output; the manager log is the place for the manager's story.

Status: implemented in this patch. Next: a runtime level switch (subscriber reload), exporting a diagnostics bundle,
and structured event fields in the UI.

## Where

`~/Library/Application Support/nxrt-frontend/logs/`:

| File                         | What                                                                     |
| ---------------------------- | ------------------------------------------------------------------------ |
| `manager.log.YYYY-MM-DD`     | the manager's own log (daily; older than seven days are pruned on start) |
| `<app-id>-<time>.log`        | a run's log: the game/step's stdout and stderr                           |
| `<app-id>-<step>-<time>.log` | a setup step's log                                                       |

The file names use the app id from `--describe`. The issue form doesn't include a log (logs can name your folders):
attach the newest one for that app yourself.

## Levels and targets

`error` < `warn` < `info` < `debug` < `trace`; default `info`.

Targets used by explicit events: `app`, `run`, `api`, `pads`, `ui`, `recomp`, `settings`, `library`, `issues`. Library
crates (tauri) log under their own targets; a global `NXRT_FRONTEND_LOG=debug` shows them too.

```sh
NXRT_FRONTEND_LOG=debug npm run dev
NXRT_FRONTEND_LOG=run=trace,api=debug npm run dev
NXRT_FRONTEND_LOG=warn npm run dev            # only problems
```

`RUST_LOG` is used when `NXRT_FRONTEND_LOG` is absent. The filter is read once at start-up; Settings → Diagnostics
shows it.

## A run, end to end

1. The manager creates `NXRT_RUN_ID=r<seconds>-<seq>` and passes it with the run (any kind: Play, Start screen, a step,
   a dev build, a recomp build).
2. `run started` is written with the key, kind, log file and run id; `run finished` (or `run failed`) follows.
3. nxrt's logger gets the same id (its `[run=…]`) and the level from `NXRT_GAME_LOG` when set:

```sh
NXRT_GAME_LOG=debug npm run dev      # the game's nxrt logging at debug
```

4. The run's own log file is in the same folder; the manager log says which file it is.

## In the window

- **Logs → Manager**: the newest manager log, level chips (All/Error/Warn/Info/Debug/Trace), a text filter, and
  **Follow** (a 3 s poll while the page is open). The path is shown above the lines.
- **Settings → Diagnostics**: the log level, the current file, and a shortcut to the Manager card.
- **Logs → Runs**: the run logs; the manager's daily files are kept out of that list and live in the Manager card.
- Script errors are logged with their source and stack (`target: ui`) without a modal.

## Run logs versus the manager log

The run log is the game's own output: its stdout, stderr, progress lines and crash text. The manager log is the
manager's: commands, gather failures, run lifecycle, pad start-up, saved settings/controls, issue actions, UI errors.
Use both when triaging: the manager log says which run, the run log says what the game was doing.

## Tests

`cargo test` covers the run-id shape, the tail reader and the filter default. The UI has no test harness; the Manager
card and the chips were checked against a live log (All/INFO/Debug) and the follow poll.

## See also

- nxrt's logger: [nxrt/docs/LOGGING.md](../nxrt/docs/LOGGING.md) (`NXRT_LOG`, `NXRT_LOG_FILE`, `NXRT_RUN_ID`).
