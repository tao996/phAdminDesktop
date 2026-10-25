mod commands;
mod config_apply;
mod db;
mod deploy;
mod legacy;
mod models;
mod render;
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
    pub syncs: Mutex<HashSet<String>>,
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
                syncs: Mutex::new(HashSet::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_workspace,
            commands::load_deployment_config,
            commands::sync_git_item,
            commands::sync_bundle_item,
            commands::sync_ftp_item,
            render::render_config_preview,
            render::render_nginx_preview,
            render::render_compose_preview,
            render::render_config_php_preview,
            render::write_generated_config,
            render::generated_config_status,
            render::open_generated_dir,
            config_apply::apply_generated_config,
            config_apply::remote_config_status,
            config_apply::list_config_backups,
            config_apply::restore_config_backup,
            commands::import_legacy_config,
            commands::list_runs,
            commands::list_deploy_runs,
            commands::read_run_log,
            deploy::start_deploy,
            deploy::preview_deploy_plan,
            runner::run_command,
            runner::cancel_run,
            runner::get_containers
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
