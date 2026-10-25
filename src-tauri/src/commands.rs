use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use rusqlite::OptionalExtension;
use tauri::{AppHandle, Manager, State};

use crate::AppState;
use crate::db;
use crate::legacy;
use crate::models::{DeploymentConfig, ImportSummary, RunRecord, Workspace};
use crate::runner;

#[tauri::command]
pub fn load_workspace(state: State<'_, AppState>) -> Result<Workspace, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::load_workspace(&connection).map_err(|error| format!("读取工作区失败: {error}"))
}

#[tauri::command]
pub fn import_legacy_config(
    state: State<'_, AppState>,
    json: String,
) -> Result<ImportSummary, String> {
    let mut connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    legacy::import(&mut connection, &json).map_err(|error| format!("导入 legacy 配置失败: {error}"))
}

#[tauri::command]
pub fn list_runs(
    state: State<'_, AppState>,
    deployment_id: String,
    limit: Option<u32>,
) -> Result<Vec<RunRecord>, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let limit = i64::from(limit.unwrap_or(50).clamp(1, 200));
    db::list_runs(&connection, &deployment_id, limit)
        .map_err(|error| format!("读取任务历史失败: {error}"))
}

#[tauri::command]
pub fn read_run_log(state: State<'_, AppState>, run_id: String) -> Result<String, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let log_path: Option<String> = connection
        .query_row(
            "SELECT log_path FROM runs WHERE id = ?1",
            [&run_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取任务日志路径失败: {error}"))?
        .flatten();

    let path = log_path.ok_or_else(|| "该任务没有日志文件".to_string())?;
    std::fs::read_to_string(&path).map_err(|error| format!("读取日志文件失败: {error}"))
}

#[tauri::command]
pub fn load_deployment_config(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<DeploymentConfig, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::load_deployment_config(&connection, &deployment_id)
        .map_err(|error| format!("读取部署配置失败: {error}"))
}

#[tauri::command]
pub fn sync_git_item(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    item_id: String,
) -> Result<String, String> {
    let lock_key = format!("{deployment_id}:{item_id}");
    {
        let mut syncs = state
            .syncs
            .lock()
            .map_err(|error| format!("同步锁被占用: {error}"))?;
        if !syncs.insert(lock_key.clone()) {
            return Err("该同步项正在执行".to_string());
        }
    }

    let result = (|| -> Result<String, String> {
        let (method, repo, branch, path) = {
            let connection = state
                .db
                .lock()
                .map_err(|error| format!("数据库连接被占用: {error}"))?;
            connection
                .query_row(
                    "SELECT method, repo, branch, path
                     FROM sync_items
                     WHERE id = ?1 AND deployment_id = ?2",
                    rusqlite::params![&item_id, &deployment_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .map_err(|error| format!("同步项不存在: {error}"))?
        };

        if method != "git" {
            return Err(format!("暂只支持 git 同步项，当前 method={method}"));
        }

        let target = {
            let connection = state
                .db
                .lock()
                .map_err(|error| format!("数据库连接被占用: {error}"))?;
            runner::load_target(&connection, &deployment_id)?
        };

        let base = target.remote_path.trim_end_matches(['/', '\\']);
        let target_text = if path.trim().is_empty() {
            base.to_string()
        } else if target.kind == "filesystem" {
            PathBuf::from(base)
                .join(path.trim())
                .to_string_lossy()
                .to_string()
        } else {
            format!(
                "{}/{}",
                base.trim_end_matches('/'),
                path.trim().trim_start_matches('/')
            )
        };
        let git_dir = format!("{}/.git", target_text.trim_end_matches('/'));
        let repo_exists = runner::target_path_exists(&target, &git_dir)?;

        let args = if repo_exists {
            vec![
                "-C".to_string(),
                target_text,
                "pull".to_string(),
                "--ff-only".to_string(),
            ]
        } else {
            if repo.trim().is_empty() {
                return Err("同步项没有配置 repo，无法 clone".to_string());
            }
            let parent = Path::new(&target_text)
                .parent()
                .map(|value| value.to_string_lossy().to_string())
                .ok_or_else(|| "无法确定同步项目标父目录".to_string())?;
            runner::ensure_target_dir(&target, &parent)?;
            vec![
                "clone".to_string(),
                "--depth=1".to_string(),
                "-b".to_string(),
                if branch.trim().is_empty() {
                    "main".to_string()
                } else {
                    branch.trim().to_string()
                },
                repo.trim().to_string(),
                target_text,
            ]
        };

        runner::run_command(app.clone(), state, deployment_id, "git".to_string(), args)
    })();

    match result {
        Ok(run_id) => {
            let cleanup_app = app.clone();
            let cleanup_run_id = run_id.clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(200));
                    let state = cleanup_app.state::<AppState>();
                    let status = state.db.lock().ok().and_then(|connection| {
                        connection
                            .query_row(
                                "SELECT status FROM runs WHERE id = ?1",
                                [&cleanup_run_id],
                                |row| row.get::<_, String>(0),
                            )
                            .ok()
                    });
                    if status.as_deref() != Some("running") {
                        if let Ok(mut syncs) = state.syncs.lock() {
                            syncs.remove(&lock_key);
                        }
                        break;
                    }
                }
            });
            Ok(run_id)
        }
        Err(error) => {
            if let Ok(mut syncs) = app.state::<AppState>().syncs.lock() {
                syncs.remove(&lock_key);
            }
            Err(error)
        }
    }
}
