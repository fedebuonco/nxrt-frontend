//! nxrt-frontend's desktop entry point; the app itself lives in the library (the Tauri mobile pattern).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nxrt_frontend_lib::run();
}
