mod commands;
mod config_apply;
mod database;
mod db;
mod db_ops;
mod deploy;
mod deploy_context;
mod env_vars;
mod host_nginx;
mod models;
mod project_create;
mod project_ops;
mod render;
mod runner;
mod secrets;
mod templates;

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
    pub secrets: Arc<dyn secrets::SecretStore>,
    pub tunnels: Mutex<HashMap<String, project_ops::Tunnel>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.with_webview(|webview| unsafe {
                    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings;
                    if let Ok(core) = webview.controller().CoreWebView2()
                        && let Ok(settings) = core.Settings()
                    {
                        let settings: ICoreWebView2Settings = settings;
                        let _ = settings.SetAreDefaultContextMenusEnabled(false);
                    }
                });
            }

            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let connection = db::open(&data_dir.join("phadmin.db"))?;
            app.manage(AppState {
                db: Mutex::new(connection),
                runs: Mutex::new(HashMap::new()),
                canceled: Mutex::new(HashSet::new()),
                syncs: Mutex::new(HashSet::new()),
                secrets: secrets::system_store(),
                tunnels: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_workspace,
            commands::load_deployment_config,
            commands::create_server,
            project_create::create_project,
            project_create::get_project_config,
            project_create::update_project,
            commands::get_server_config,
            commands::update_server,
            commands::count_server_references,
            commands::delete_server,
            templates::list_templates,
            templates::get_template,
            templates::save_template,
            templates::reset_template,
            render::preview_template,
            render::preview_project_files,
            commands::open_app_data_dir,
            commands::get_local_repo_path,
            commands::set_local_repo_path,
            env_vars::list_env_vars,
            env_vars::set_env_var,
            env_vars::delete_env_var,
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
            config_apply::prepare_runtime_assets,
            config_apply::prune_config_backups,
            database::backup_database,
            database::download_database_backup,
            database::list_database_backups,
            database::restore_database_backup,
            db_ops::migrate_database,
            db_ops::seed_database,
            db_ops::open_seed_directory,
            db_ops::install_backup_cron,
            db_ops::remove_backup_cron,
            db_ops::list_remote_backups,
            host_nginx::publish_nginx_config,
            host_nginx::remove_nginx_config,
            host_nginx::reload_host_nginx,
            host_nginx::read_nginx_log,
            host_nginx::check_server,
            host_nginx::enable_ssl,
            project_ops::reset_project,
            project_ops::generate_git_deploy_key,
            project_ops::start_ssh_tunnel,
            project_ops::stop_ssh_tunnel,
            project_ops::list_ssh_tunnels,
            project_ops::start_phpmyadmin,
            project_ops::stop_phpmyadmin,
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
