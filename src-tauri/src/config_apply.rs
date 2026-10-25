use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::runner::{self, Target};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

const FILES: &[&str] = &[".env", "nginx.conf", "docker-compose.yaml"];

fn backup_existing(target: &Target, destination: &str, millis: u128) -> Result<(), String> {
    if target.kind == "filesystem" {
        let path = Path::new(destination);
        if path.exists() {
            fs::copy(path, format!("{destination}.bak-{millis}"))
                .map_err(|error| format!("备份 {destination} 失败: {error}"))?;
        }
        return Ok(());
    }

    let script = format!(
        "if [ -f {} ]; then cp -a {} {}.bak-{millis}; fi",
        shell_quote(destination),
        shell_quote(destination),
        shell_quote(destination),
    );
    let (success, output) =
        runner::capture_target_command(target, "sh", &["-lc".to_string(), script])?;
    if !success {
        return Err(format!("备份 {destination} 失败: {}", output.trim()));
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[tauri::command]
pub fn apply_generated_config(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    backup: bool,
) -> Result<String, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let source_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("generated")
        .join(&deployment_id);
    for file in FILES {
        let path = source_dir.join(file);
        if !path.is_file() {
            return Err(format!("缺少生成文件 {}，请先生成本地配置", path.display()));
        }
    }

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    runner::ensure_target_dir(&target, &target.remote_path)?;

    let mut transcript = String::new();
    for file in FILES {
        let destination = if target.kind == "filesystem" {
            PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
                .join(file)
                .to_string_lossy()
                .to_string()
        } else {
            format!("{}/{}", target.remote_path.trim_end_matches('/'), file)
        };
        if backup {
            backup_existing(&target, &destination, millis)?;
        }
        runner::upload_to_target(&target, &source_dir.join(file), &destination)?;
        transcript.push_str(&format!(
            "$ upload {} -> {}:{}\n",
            file, target.host, destination
        ));
    }
    Ok(transcript)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileStatus {
    pub file: String,
    pub exists: bool,
    pub changed: bool,
}

#[tauri::command]
pub fn remote_config_status(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<Vec<RemoteFileStatus>, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let source_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("generated")
        .join(&deployment_id);

    let mut statuses = Vec::new();
    for file in FILES {
        let local_path = source_dir.join(file);
        let local = fs::read_to_string(&local_path).unwrap_or_default();
        let destination = if target.kind == "filesystem" {
            PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
                .join(file)
                .to_string_lossy()
                .to_string()
        } else {
            format!("{}/{}", target.remote_path.trim_end_matches('/'), file)
        };

        let remote = if target.kind == "filesystem" {
            fs::read_to_string(&destination).ok()
        } else {
            let (success, output) = runner::capture_target_command(&target, "cat", &[destination])?;
            success.then_some(output)
        };
        statuses.push(RemoteFileStatus {
            file: file.to_string(),
            exists: remote.is_some(),
            changed: remote.as_deref() != Some(local.as_str()),
        });
    }
    Ok(statuses)
}
