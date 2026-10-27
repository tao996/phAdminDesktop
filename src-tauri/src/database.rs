use std::fs;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;
use crate::env_vars;
use crate::runner;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseBackup {
    pub path: String,
    pub bytes: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub name: String,
    pub path: String,
    pub bytes: u64,
}

fn backup_directory(app: &AppHandle, deployment_id: &str) -> Result<std::path::PathBuf, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("backups")
        .join(deployment_id);
    fs::create_dir_all(&directory).map_err(|error| format!("创建备份目录失败: {error}"))?;
    Ok(directory)
}

fn mysql_connection(
    state: &State<'_, AppState>,
    deployment_id: &str,
) -> Result<(runner::Target, String, String, String), String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let target = runner::load_target(&connection, deployment_id)?;
    let project_name = connection
        .query_row(
            "SELECT p.name
             FROM deployments d
             JOIN projects p ON p.id = d.project_id
             WHERE d.id = ?1",
            [deployment_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("项目不存在: {error}"))?;
    let env = |key: &str| -> Result<String, String> {
        Ok(
            env_vars::resolve(&connection, state.secrets.as_ref(), deployment_id, key)?
                .unwrap_or_default(),
        )
    };
    let user = env("MYSQL_USER")?;
    let database = env("MYSQL_DATABASE")?;
    Ok((
        target,
        if user.is_empty() {
            project_name.clone()
        } else {
            user
        },
        env("MYSQL_PASSWORD")?,
        if database.is_empty() {
            project_name
        } else {
            database
        },
    ))
}

#[tauri::command]
pub fn list_database_backups(
    app: AppHandle,
    deployment_id: String,
) -> Result<Vec<BackupEntry>, String> {
    let directory = backup_directory(&app, &deployment_id)?;
    let mut entries = Vec::new();
    for entry in fs::read_dir(&directory).map_err(|error| format!("读取备份目录失败: {error}"))?
    {
        let entry = entry.map_err(|error| format!("读取备份失败: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if !(name.ends_with(".sql") || name.ends_with(".sql.gz")) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("读取备份信息失败: {error}"))?;
        entries.push(BackupEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            bytes: metadata.len(),
        });
    }
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(entries)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreResult {
    pub snapshot_path: String,
    pub bytes: usize,
}

#[tauri::command]
pub fn restore_database_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    name: String,
) -> Result<RestoreResult, String> {
    if name.contains('/')
        || name.contains('\\')
        || !(name.ends_with(".sql") || name.ends_with(".sql.gz"))
    {
        return Err("非法备份文件名".to_string());
    }
    let directory = backup_directory(&app, &deployment_id)?;
    let path = directory.join(&name);
    let raw = fs::read(&path).map_err(|error| format!("读取备份失败: {error}"))?;
    let sql = if name.ends_with(".gz") {
        let mut decoder = GzDecoder::new(raw.as_slice());
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .map_err(|error| format!("解压备份失败: {error}"))?;
        decoded
    } else {
        raw
    };
    let snapshot = dump_database(&app, &state, &deployment_id)?;
    let (target, user, password, database) = mysql_connection(&state, &deployment_id)?;

    let mut args = vec![
        "compose".to_string(),
        "exec".to_string(),
        "-T".to_string(),
        "mysql".to_string(),
        "mysql".to_string(),
        "-u".to_string(),
        user,
    ];
    if !password.is_empty() {
        args.push(format!("-p{password}"));
    }
    args.push(database);
    let (success, stderr) = runner::run_target_with_stdin(&target, "docker", &args, &sql)?;
    if !success {
        return Err(format!("恢复失败: {}", stderr.trim()));
    }
    Ok(RestoreResult {
        snapshot_path: snapshot.path,
        bytes: sql.len(),
    })
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

fn dump_database(
    app: &AppHandle,
    state: &State<'_, AppState>,
    deployment_id: &str,
) -> Result<DatabaseBackup, String> {
    let (target, user, password, database) = mysql_connection(state, deployment_id)?;

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
        .join(deployment_id);
    fs::create_dir_all(&directory).map_err(|error| format!("创建备份目录失败: {error}"))?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&stdout)
        .map_err(|error| format!("压缩备份失败: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("压缩备份失败: {error}"))?;
    let path = directory.join(format!("{millis}.sql.gz"));
    fs::write(&path, &compressed).map_err(|error| format!("写入备份失败: {error}"))?;
    Ok(DatabaseBackup {
        path: path.to_string_lossy().to_string(),
        bytes: compressed.len(),
    })
}

#[tauri::command]
pub fn download_database_backup(
    app: AppHandle,
    deployment_id: String,
    name: String,
) -> Result<String, String> {
    if name.contains('/')
        || name.contains('\\')
        || !(name.ends_with(".sql") || name.ends_with(".sql.gz"))
    {
        return Err("非法备份文件名".to_string());
    }
    let source = backup_directory(&app, &deployment_id)?.join(&name);
    if !source.exists() {
        return Err(format!("备份不存在: {name}"));
    }
    let downloads = app
        .path()
        .download_dir()
        .map_err(|error| format!("获取下载目录失败: {error}"))?;
    fs::create_dir_all(&downloads).map_err(|error| format!("创建下载目录失败: {error}"))?;
    let destination = downloads.join(&name);
    fs::copy(&source, &destination).map_err(|error| format!("导出备份失败: {error}"))?;
    Ok(destination.to_string_lossy().to_string())
}

#[tauri::command]
pub fn backup_database(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<DatabaseBackup, String> {
    dump_database(&app, &state, &deployment_id)
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
