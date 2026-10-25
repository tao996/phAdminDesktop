use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;
use crate::runner;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBackup {
    pub path: String,
    pub bytes: usize,
}

fn mysqldump_args(user: &str, password: &str, database: &str) -> Vec<String> {
    let mut args = vec![
        "compose".to_string(),
        "exec".to_string(),
        "-T".to_string(),
        "mysql".to_string(),
        "mysqldump".to_string(),
        "--single-transaction".to_string(),
        "--quick".to_string(),
        "--skip-lock-tables".to_string(),
        "-u".to_string(),
        user.to_string(),
    ];
    if !password.is_empty() {
        args.push(format!("-p{password}"));
    }
    args.push(database.to_string());
    args
}

#[tauri::command]
pub fn backup_database(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<DatabaseBackup, String> {
    let (target, project_name) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let target = runner::load_target(&connection, &deployment_id)?;
        let project_name = connection
            .query_row(
                "SELECT p.name
                 FROM deployments d
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1",
                [&deployment_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("项目不存在: {error}"))?;
        (target, project_name)
    };

    let (user, password, database) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let env = |key: &str| -> String {
            connection
                .query_row(
                    "SELECT value FROM env_vars WHERE deployment_id = ?1 AND key = ?2",
                    rusqlite::params![&deployment_id, key],
                    |row| row.get::<_, String>(0),
                )
                .ok()
                .unwrap_or_default()
        };
        let user = env("MYSQL_USER");
        let database = env("MYSQL_DATABASE");
        (
            if user.is_empty() {
                project_name.clone()
            } else {
                user
            },
            env("MYSQL_PASSWORD"),
            if database.is_empty() {
                project_name
            } else {
                database
            },
        )
    };

    let args = mysqldump_args(&user, &password, &database);
    let (success, stdout, stderr) = runner::capture_target_stdout(&target, "docker", &args)?;
    if !success {
        return Err(format!("mysqldump 失败: {}", stderr.trim()));
    }
    if stdout.is_empty() {
        return Err("mysqldump 输出为空".to_string());
    }

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("backups")
        .join(&deployment_id);
    fs::create_dir_all(&directory).map_err(|error| format!("创建备份目录失败: {error}"))?;
    let path = directory.join(format!("{millis}.sql"));
    fs::write(&path, &stdout).map_err(|error| format!("写入备份失败: {error}"))?;
    Ok(DatabaseBackup {
        path: path.to_string_lossy().to_string(),
        bytes: stdout.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_mysqldump_args_with_optional_password() {
        let args = mysqldump_args("demo", "secret", "demo");
        assert!(args.contains(&"-psecret".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("demo"));
        assert!(
            !mysqldump_args("demo", "", "demo")
                .iter()
                .any(|arg| arg.starts_with("-p"))
        );
    }
}
