use rusqlite::OptionalExtension;
use tauri::State;

use crate::AppState;
use crate::db;
use crate::legacy;
use crate::models::{ImportSummary, RunRecord, Workspace};

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
