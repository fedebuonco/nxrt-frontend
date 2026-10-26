//! nxrt-frontend: one manager for the nxrt Switch recomps. The window is a Tauri webview; this binary is the whole
//! app behind it - the library, the games' own apps (`--describe` / `--status`), the steps, the runs and their logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod api;
mod cli;
mod commands;
mod controls;
mod issues;
mod library;
mod logging;
mod manage;
mod ops;
mod pads;
mod recomp;
mod runner;
mod update;

use tauri::Manager;

fn main() {
    logging::init();
    tracing::info!(target: "app", version = env!("CARGO_PKG_VERSION"), config = %library::dir().display(),
                   filter = logging::filter(), "nxrt-frontend starting");
    // (the command line first: --list, --step, --add, --create, --keys)
    if let Some(code) = cli::run() {
        std::process::exit(code);
    }
    let shared = commands::Shared::new();
    tauri::Builder::default()
        .manage(shared)
        .setup(|app| {
            {
                use tauri::Manager;
                let shared = app.state::<commands::Shared>().inner().clone();
                pads::start(app.handle().clone(), shared);
            }
            // NXRT_FRONTEND_VIEW=library|logs|settings|game:<n>|sheet:add|sheet:create:<n>:
            // the section the window opens on (for docs and screenshots)
            if let Ok(v) = std::env::var("NXRT_FRONTEND_VIEW") {
                if let Some(w) = app.get_webview_window("main") {
                    let js = format!(
                        "setTimeout(function(){{ if (window.__nxrtView) window.__nxrtView({}); }}, 500)",
                        serde_json::to_string(&v).unwrap_or_else(|_| "\"\"".into())
                    );
                    let _ = w.eval(js);
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot_command,
            commands::icon,
            commands::pick_package,
            commands::pick_app,
            commands::pick_keys,
            commands::pick_folder,
            commands::add_package,
            commands::add_app,
            commands::choose_app,
            commands::remove,
            commands::set_keys,
            commands::forget_keys,
            commands::start_game,
            commands::start_step,
            commands::start_dev,
            commands::stop,
            commands::create_recomp,
            commands::save_dev,
            commands::reveal,
            commands::open_logs,
            commands::logs,
            commands::config_dir,
            commands::pad_status,
            commands::save_controls,
            commands::open_issue,
            commands::manager_log,
            commands::open_donate,
            commands::log_ui,
            commands::check_update,
            commands::open_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running nxrt-frontend");
}
