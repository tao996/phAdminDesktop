use tauri::State;

use crate::AppState;
use crate::db;
use crate::legacy;
use crate::models::{ImportSummary, Workspace};

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
