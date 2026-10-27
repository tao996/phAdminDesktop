use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::AppState;
use crate::runner::{self, Target};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

const FILES: &[&str] = &[".env", "nginx.conf", "docker-compose.yaml", "config.php"];

/// 生成目录里的文件名 → 项目内目标路径。
fn remote_relative(file: &str) -> &str {
    if file == "config.php" {
        "src/config/config.php"
    } else {
        file
    }
}

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
    for relative in [
        "docker/mysql/my.cnf",
        "docker/nginx/sites/default.conf",
        "docker/php/php.ini",
    ] {
        let path = source_dir.join(relative);
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
        let relative = remote_relative(file);
        let destination = if target.kind == "filesystem" {
            PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
                .join(relative)
                .to_string_lossy()
                .to_string()
        } else {
            format!("{}/{}", target.remote_path.trim_end_matches('/'), relative)
        };
        if let Some(parent) = Path::new(&destination).parent() {
            runner::ensure_target_dir(&target, &parent.to_string_lossy())?;
        }
        if backup {
            backup_existing(&target, &destination, millis)?;
        }
        runner::upload_to_target(&target, &source_dir.join(file), &destination)?;
        let local = fs::read_to_string(source_dir.join(file))
            .map_err(|error| format!("读取本地 {file} 失败: {error}"))?;
        let remote = if target.kind == "filesystem" {
            fs::read_to_string(&destination).ok()
        } else {
            let (success, output) =
                runner::capture_target_command(&target, "cat", std::slice::from_ref(&destination))?;
            success.then_some(output)
        };
        if remote.as_deref() != Some(local.as_str()) {
            return Err(format!("{file} 上传后校验失败"));
        }
        transcript.push_str(&format!(
            "$ upload {} -> {}:{}\n$ verified {}\n",
            file, target.host, destination, file
        ));
    }

    // 容器运行配置：模板渲染后按项目内路径上传。
    for relative in [
        "docker/mysql/my.cnf",
        "docker/nginx/sites/default.conf",
        "docker/php/php.ini",
    ] {
        let source = source_dir.join(relative);
        if !source.is_file() {
            return Err(format!(
                "缺少生成文件 {}，请先生成本地配置",
                source.display()
            ));
        }
        let destination = if target.kind == "filesystem" {
            PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
                .join(relative)
                .to_string_lossy()
                .to_string()
        } else {
            format!("{}/{}", target.remote_path.trim_end_matches('/'), relative)
        };
        if let Some(parent) = Path::new(&destination).parent() {
            runner::ensure_target_dir(&target, &parent.to_string_lossy())?;
        }
        if backup {
            backup_existing(&target, &destination, millis)?;
        }
        runner::upload_to_target(&target, &source, &destination)?;
        transcript.push_str(&format!(
            "$ upload {} -> {}:{}\n",
            relative, target.host, destination
        ));
    }

    // 自定义模板：write_generated_config 写入清单，按路径原样上传。
    if let Ok(manifest) = fs::read_to_string(source_dir.join("template-manifest.json")) {
        let paths: Vec<String> = serde_json::from_str(&manifest).unwrap_or_default();
        for relative in &paths {
            let source = source_dir.join(relative);
            if !source.is_file() {
                return Err(format!(
                    "缺少生成文件 {}，请先生成本地配置",
                    source.display()
                ));
            }
            let destination = if target.kind == "filesystem" {
                PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
                    .join(relative)
                    .to_string_lossy()
                    .to_string()
            } else {
                format!("{}/{}", target.remote_path.trim_end_matches('/'), relative)
            };
            if let Some(parent) = Path::new(&destination).parent() {
                runner::ensure_target_dir(&target, &parent.to_string_lossy())?;
            }
            runner::upload_to_target(&target, &source, &destination)?;
            transcript.push_str(&format!(
                "$ upload {} -> {}:{}\n",
                relative, target.host, destination
            ));
        }
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
        let relative = remote_relative(file);
        let destination = if target.kind == "filesystem" {
            PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
                .join(relative)
                .to_string_lossy()
                .to_string()
        } else {
            format!("{}/{}", target.remote_path.trim_end_matches('/'), relative)
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigBackup {
    pub name: String,
    pub file: String,
}

fn destination_for(target: &Target, file: &str) -> String {
    if target.kind == "filesystem" {
        PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
            .join(file)
            .to_string_lossy()
            .to_string()
    } else {
        format!("{}/{}", target.remote_path.trim_end_matches('/'), file)
    }
}

#[tauri::command]
pub fn list_config_backups(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<Vec<ConfigBackup>, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let mut names = Vec::new();
    if target.kind == "filesystem" {
        if let Ok(entries) = fs::read_dir(target.remote_path.trim_end_matches(['/', '\\'])) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(".bak-") {
                    names.push(name);
                }
            }
        }
    } else {
        let args = vec![
            target.remote_path.trim_end_matches('/').to_string(),
            "-maxdepth".to_string(),
            "1".to_string(),
            "-type".to_string(),
            "f".to_string(),
            "-name".to_string(),
            "*.bak-*".to_string(),
            "-printf".to_string(),
            "%f\\n".to_string(),
        ];
        let (success, output) = runner::capture_target_command(&target, "find", &args)?;
        if success {
            names.extend(output.lines().map(|line| line.trim().to_string()));
        }
    }
    names.sort();
    names.reverse();
    Ok(names
        .into_iter()
        .filter_map(|name| {
            let file = name.split(".bak-").next()?.to_string();
            FILES
                .contains(&file.as_str())
                .then_some(ConfigBackup { name, file })
        })
        .collect())
}

#[tauri::command]
pub fn restore_config_backup(
    state: State<'_, AppState>,
    deployment_id: String,
    backup_name: String,
) -> Result<String, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let file = backup_name
        .split(".bak-")
        .next()
        .unwrap_or_default()
        .to_string();
    if !FILES.contains(&file.as_str()) || !backup_name.contains(".bak-") {
        return Err("非法的备份文件名".to_string());
    }
    let backup = destination_for(&target, &backup_name);
    let destination = destination_for(&target, &file);
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    backup_existing(&target, &destination, millis)?;

    if target.kind == "filesystem" {
        fs::copy(&backup, &destination).map_err(|error| format!("恢复失败: {error}"))?;
    } else {
        let script = format!(
            "cp -a {} {}",
            shell_quote(&backup),
            shell_quote(&destination)
        );
        let (success, output) =
            runner::capture_target_command(&target, "sh", &["-lc".to_string(), script])?;
        if !success {
            return Err(format!("恢复失败: {}", output.trim()));
        }
    }
    Ok(format!("restored {file} from {backup_name}"))
}

#[tauri::command]
pub fn prepare_runtime_assets(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    runner::ensure_target_dir(&target, &target.remote_path)?;
    let mut transcript = String::new();

    if target.kind == "filesystem" {
        let root = Path::new(target.remote_path.trim_end_matches(['/', '\\']));
        let pairs = [
            (
                "docker/nginx/sites/default.example.conf",
                "docker/nginx/sites/default.conf",
            ),
            ("docker/php/php.prod.ini", "docker/php/php.ini"),
            ("docker/mysql/my.example.cnf", "docker/mysql/my.cnf"),
        ];
        for (source, destination) in pairs {
            let source = root.join(source);
            let destination = root.join(destination);
            if !destination.exists() && source.is_file() {
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent).map_err(|error| format!("创建目录失败: {error}"))?;
                }
                fs::copy(&source, &destination)
                    .map_err(|error| format!("复制 {} 失败: {error}", source.display()))?;
                transcript.push_str(&format!(
                    "copy {} -> {}\n",
                    source.display(),
                    destination.display()
                ));
            }
        }
        for directory in [
            "docker/log/nginx",
            "docker/log/php",
            "docker/log/mysql",
            "src/storage",
        ] {
            fs::create_dir_all(root.join(directory))
                .map_err(|error| format!("创建 {directory} 失败: {error}"))?;
        }
    } else {
        let root = shell_quote(target.remote_path.trim());
        let script = format!(
            "set -e\ncd {root}\n[ -f docker/nginx/sites/default.conf ] || cp docker/nginx/sites/default.example.conf docker/nginx/sites/default.conf 2>/dev/null || true\n[ -f docker/php/php.ini ] || cp docker/php/php.prod.ini docker/php/php.ini 2>/dev/null || true\n[ -f docker/mysql/my.cnf ] || cp docker/mysql/my.example.cnf docker/mysql/my.cnf 2>/dev/null || true\nmkdir -p docker/log/nginx docker/log/php docker/log/mysql src/storage\n"
        );
        let (success, output) =
            runner::capture_target_command(&target, "sh", &["-lc".to_string(), script])?;
        if !success {
            return Err(format!("准备运行目录失败: {}", output.trim()));
        }
        transcript.push_str(&output);
    }
    Ok(transcript)
}

#[tauri::command]
pub fn prune_config_backups(
    state: State<'_, AppState>,
    deployment_id: String,
    keep: u32,
) -> Result<usize, String> {
    let keep = keep.clamp(1, 100) as usize;
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let mut names = Vec::new();
    if target.kind == "filesystem" {
        if let Ok(entries) = fs::read_dir(target.remote_path.trim_end_matches(['/', '\\'])) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.contains(".bak-") {
                    names.push(name);
                }
            }
        }
    } else {
        let args = vec![
            target.remote_path.trim_end_matches('/').to_string(),
            "-maxdepth".to_string(),
            "1".to_string(),
            "-type".to_string(),
            "f".to_string(),
            "-name".to_string(),
            "*.bak-*".to_string(),
            "-printf".to_string(),
            "%f\\n".to_string(),
        ];
        let (success, output) = runner::capture_target_command(&target, "find", &args)?;
        if success {
            names.extend(output.lines().map(|line| line.trim().to_string()));
        }
    }

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for name in names {
        if let Some(file) = name.split(".bak-").next()
            && FILES.contains(&file)
        {
            groups.entry(file.to_string()).or_default().push(name);
        }
    }
    let mut removed = 0;
    for (_, mut backups) in groups {
        backups.sort();
        backups.reverse();
        for name in backups.into_iter().skip(keep) {
            if target.kind == "filesystem" {
                fs::remove_file(destination_for(&target, &name))
                    .map_err(|error| format!("删除 {name} 失败: {error}"))?;
            } else {
                let (success, output) = runner::capture_target_command(
                    &target,
                    "rm",
                    &["-f".to_string(), destination_for(&target, &name)],
                )?;
                if !success {
                    return Err(format!("删除 {name} 失败: {}", output.trim()));
                }
            }
            removed += 1;
        }
    }
    Ok(removed)
}
