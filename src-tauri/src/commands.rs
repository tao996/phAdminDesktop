use tauri::State;

use crate::AppState;
use crate::db;
use crate::models::Workspace;

#[tauri::command]
pub fn load_workspace(state: State<'_, AppState>) -> Result<Workspace, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::load_workspace(&connection).map_err(|error| format!("读取工作区失败: {error}"))
}
