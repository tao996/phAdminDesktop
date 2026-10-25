mod commands;
mod db;
mod legacy;
mod models;
mod runner;

use std::collections::{HashMap, HashSet};
use std::process::Child;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;
use tauri::Manager;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub runs: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    pub canceled: Mutex<HashSet<String>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let connection = db::open(&data_dir.join("phadmin.db"))?;
            app.manage(AppState {
                db: Mutex::new(connection),
                runs: Mutex::new(HashMap::new()),
                canceled: Mutex::new(HashSet::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_workspace,
            commands::import_legacy_config,
            commands::list_runs,
            commands::read_run_log,
            runner::run_command,
            runner::cancel_run,
            runner::get_containers
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
