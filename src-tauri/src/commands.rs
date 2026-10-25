use std::path::PathBuf;

use rusqlite::OptionalExtension;
use tauri::{AppHandle, State};

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
    let (kind, remote_path, method, repo, branch, path) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        connection
            .query_row(
                "SELECT s.kind, d.remote_path, i.method, i.repo, i.branch, i.path
                 FROM sync_items i
                 JOIN deployments d ON d.id = i.deployment_id
                 JOIN servers s ON s.id = d.server_id
                 WHERE i.id = ?1 AND i.deployment_id = ?2",
                rusqlite::params![&item_id, &deployment_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                    ))
                },
            )
            .map_err(|error| format!("同步项不存在: {error}"))?
    };

    if method != "git" {
        return Err(format!("暂只支持 git 同步项，当前 method={method}"));
    }
    if kind != "filesystem" {
        return Err("原生 git 同步暂只支持本地 filesystem 服务器".to_string());
    }

    let base = PathBuf::from(remote_path);
    let target = if path.trim().is_empty() {
        base.clone()
    } else {
        base.join(path.trim())
    };
    let target_text = target.to_string_lossy().to_string();

    let args = if target.join(".git").exists() {
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
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建同步目录失败: {error}"))?;
        }
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

    runner::run_command(app, state, deployment_id, "git".to_string(), args)
}
