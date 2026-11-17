use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::OptionalExtension;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::AppState;
use crate::db;
use crate::models::{DeployRunRecord, DeploymentConfig, RunRecord, Server, ServerInput, Workspace};
use crate::runner;

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn target_relative_path(target: &runner::Target, relative: &str) -> String {
    if target.kind == "filesystem" {
        PathBuf::from(target.remote_path.trim_end_matches(['/', '\\']))
            .join(relative)
            .to_string_lossy()
            .to_string()
    } else {
        format!(
            "{}/{}",
            target.remote_path.trim_end_matches('/'),
            relative.trim_start_matches('/')
        )
    }
}

fn read_target_text(target: &runner::Target, path: &str) -> Result<String, String> {
    if target.kind == "filesystem" {
        return fs::read_to_string(path).map_err(|error| format!("读取 {path} 失败: {error}"));
    }
    let (success, output) = runner::capture_target_command(target, "cat", &[path.to_string()])?;
    if !success {
        return Err(format!("读取 {path} 失败: {}", output.trim()));
    }
    Ok(output)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationLogFile {
    pub name: String,
    pub size: u64,
    pub modified: i64,
}

fn path_excluded(relative: &str, excludes: &[String]) -> bool {
    excludes.iter().any(|pattern| {
        let pattern = pattern.trim().trim_matches('/');
        !pattern.is_empty() && (relative == pattern || relative.starts_with(&format!("{pattern}/")))
    })
}

fn copy_tree(source: &Path, destination: &Path, excludes: &[String]) -> Result<usize, String> {
    fs::create_dir_all(destination).map_err(|error| format!("创建目录失败: {error}"))?;
    let mut copied = 0;
    for entry in fs::read_dir(source).map_err(|error| format!("读取目录失败: {error}"))? {
        let entry = entry.map_err(|error| format!("读取目录项失败: {error}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if path_excluded(&name, excludes) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(&name);
        let metadata =
            fs::symlink_metadata(&source_path).map_err(|error| format!("读取文件失败: {error}"))?;
        if metadata.is_dir() {
            copied += copy_tree(&source_path, &destination_path, excludes)?;
        } else if metadata.is_file() {
            fs::copy(&source_path, &destination_path)
                .map_err(|error| format!("复制文件失败: {error}"))?;
            copied += 1;
        }
    }
    Ok(copied)
}

fn run_git(args: &[String]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("启动 git 失败: {error}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if output.status.success() {
        Ok(text)
    } else {
        Err(if text.trim().is_empty() {
            format!("git 退出码 {:?}", output.status.code())
        } else {
            text
        })
    }
}

#[tauri::command]
pub fn load_workspace(state: State<'_, AppState>) -> Result<Workspace, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::load_workspace(&connection).map_err(|error| format!("读取工作区失败: {error}"))
}

#[tauri::command]
pub fn create_server(state: State<'_, AppState>, input: ServerInput) -> Result<Server, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::create_server(&connection, &input)
}

#[tauri::command]
pub fn get_server_config(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<crate::models::ServerConfig, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::load_server_config(&connection, &server_id)
}

#[tauri::command]
pub fn update_server(
    state: State<'_, AppState>,
    server_id: String,
    input: ServerInput,
) -> Result<Server, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::update_server(&connection, &server_id, &input)
}

#[tauri::command]
pub fn count_server_references(
    state: State<'_, AppState>,
    server_id: String,
) -> Result<i64, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::count_server_references(&connection, &server_id)
}

#[tauri::command]
pub fn delete_server(state: State<'_, AppState>, server_id: String) -> Result<(), String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::delete_server(&connection, &server_id)
}

#[tauri::command(async)]
pub fn recreate_service(
    state: State<'_, AppState>,
    deployment_id: String,
    service: String,
) -> Result<String, String> {
    if service != "all" && !matches!(service.as_str(), "nginx" | "php" | "mysql" | "redis") {
        return Err(format!("不支持重建的服务: {service}"));
    }
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };

    let mut args = vec!["compose".to_string(), "up".to_string(), "-d".to_string()];
    if service != "all" {
        args.push("--force-recreate".to_string());
        args.push("--no-deps".to_string());
        args.push(service.clone());
    }
    let (success, output) = runner::capture_target_command(&target, "docker", &args)?;
    if !success {
        return Err(format!("重建 {service} 失败: {}", output.trim()));
    }
    let mut transcript = output;

    // php（或全部服务）重建后容器 IP 会变化，Nginx 必须重新解析上游。
    if service == "php" || service == "all" {
        let (reloaded, reload_output) = runner::capture_target_command(
            &target,
            "docker",
            &[
                "compose".to_string(),
                "exec".to_string(),
                "-T".to_string(),
                "nginx".to_string(),
                "nginx".to_string(),
                "-s".to_string(),
                "reload".to_string(),
            ],
        )?;
        transcript.push_str(&reload_output);
        if !reloaded {
            return Err(format!(
                "php 已重建，但 Nginx 重载失败: {}",
                reload_output.trim()
            ));
        }
    }

    Ok(transcript)
}

#[tauri::command(async)]
pub fn upload_project_file(
    state: State<'_, AppState>,
    deployment_id: String,
    path: String,
    content: String,
) -> Result<String, String> {
    let relative = crate::templates::normalize_path(&path)?;
    if content.len() > 1024 * 1024 {
        return Err("文件内容超过 1 MB 限制".to_string());
    }
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let destination = target_relative_path(&target, &relative);
    if let Some(parent) = Path::new(&destination).parent() {
        runner::ensure_target_dir(&target, &parent.to_string_lossy())?;
    }
    runner::upload_text_to_target(&target, &destination, &content)?;
    let remote = read_target_text(&target, &destination)?;
    if remote != content {
        return Err(format!("{relative} 上传后校验失败"));
    }
    Ok(format!(
        "$ upload {relative} -> {destination}\n$ verify {relative}: content matches"
    ))
}

#[tauri::command(async)]
pub fn read_remote_project_file(
    state: State<'_, AppState>,
    deployment_id: String,
    path: String,
) -> Result<String, String> {
    let relative = crate::templates::normalize_path(&path)?;
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let destination = target_relative_path(&target, &relative);
    let content = read_target_text(&target, &destination)?;
    if content.len() > 2 * 1024 * 1024 {
        return Err("文件超过 2 MB，无法在预览中打开".to_string());
    }
    Ok(content)
}

fn tail_text(content: &str, limit: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(limit);
    lines[start..].join("\n")
}

#[tauri::command(async)]
pub fn list_application_logs(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<Vec<ApplicationLogFile>, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let directory = target_relative_path(&target, "src/storage/logs");
    let mut files = Vec::new();
    if target.kind == "filesystem" {
        match fs::read_dir(&directory) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let metadata = match entry.metadata() {
                        Ok(value) if value.is_file() => value,
                        _ => continue,
                    };
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.ends_with(".log") {
                        continue;
                    }
                    let modified = metadata
                        .modified()
                        .ok()
                        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
                        .map(|value| value.as_secs() as i64)
                        .unwrap_or(0);
                    files.push(ApplicationLogFile {
                        name,
                        size: metadata.len(),
                        modified,
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("读取日志目录失败: {error}")),
        }
    } else {
        let script = format!(
            "find {} -maxdepth 1 -type f -name '*.log' -printf '%f\\t%s\\t%T@\\n'",
            shell_quote(&directory)
        );
        let (success, output) =
            runner::capture_target_command(&target, "sh", &["-lc".to_string(), script])?;
        if !success {
            return Err(format!("读取日志目录失败: {}", output.trim()));
        }
        for line in output.lines() {
            let mut parts = line.split('\t');
            let (Some(name), Some(size), Some(modified)) =
                (parts.next(), parts.next(), parts.next())
            else {
                continue;
            };
            let Ok(size) = size.parse::<u64>() else {
                continue;
            };
            let modified = modified
                .parse::<f64>()
                .map(|value| value as i64)
                .unwrap_or(0);
            files.push(ApplicationLogFile {
                name: name.to_string(),
                size,
                modified,
            });
        }
    }
    files.sort_by_key(|file| std::cmp::Reverse(file.modified));
    Ok(files)
}

#[tauri::command(async)]
pub fn read_application_log(
    state: State<'_, AppState>,
    deployment_id: String,
    name: String,
) -> Result<String, String> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err("日志文件名不正确".to_string());
    }
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let relative = format!("src/storage/logs/{name}");
    let path = target_relative_path(&target, &relative);
    if target.kind == "filesystem" {
        let content =
            fs::read_to_string(&path).map_err(|error| format!("读取日志 {name} 失败: {error}"))?;
        return Ok(tail_text(&content, 500));
    }
    let (success, output) = runner::capture_target_command(
        &target,
        "tail",
        &["-n".to_string(), "500".to_string(), path],
    )?;
    if !success {
        return Err(format!("读取日志 {name} 失败: {}", output.trim()));
    }
    Ok(output)
}

#[tauri::command(async)]
pub fn prune_application_logs(
    state: State<'_, AppState>,
    deployment_id: String,
    keep_days: u32,
) -> Result<Vec<String>, String> {
    let keep_days = keep_days.clamp(1, 365);
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };
    let directory = target_relative_path(&target, "src/storage/logs");
    let mut removed = Vec::new();

    if target.kind == "filesystem" {
        let cutoff = SystemTime::now()
            .checked_sub(Duration::from_secs(u64::from(keep_days) * 86400))
            .unwrap_or(UNIX_EPOCH);
        match fs::read_dir(&directory) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let metadata = match entry.metadata() {
                        Ok(value) if value.is_file() => value,
                        _ => continue,
                    };
                    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
                    if modified >= cutoff {
                        continue;
                    }
                    let name = entry.file_name().to_string_lossy().to_string();
                    if !name.ends_with(".log") {
                        continue;
                    }
                    fs::remove_file(entry.path())
                        .map_err(|error| format!("删除日志 {name} 失败: {error}"))?;
                    removed.push(name);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("读取日志目录失败: {error}")),
        }
    } else {
        let script = format!(
            "find {} -maxdepth 1 -type f -name '*.log' -mtime +{} -printf '%f\\n' -delete",
            shell_quote(&directory),
            keep_days
        );
        let (success, output) =
            runner::capture_target_command(&target, "sh", &["-lc".to_string(), script])?;
        if !success {
            return Err(format!("清理日志失败: {}", output.trim()));
        }
        removed.extend(
            output
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_string),
        );
    }

    removed.sort();
    Ok(removed)
}

#[tauri::command]
pub fn set_deployment_status(
    state: State<'_, AppState>,
    deployment_id: String,
    status: String,
) -> Result<(), String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::set_deployment_status(&connection, &deployment_id, &status)
}

#[tauri::command]
pub fn get_local_repo_path(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::get_setting(&connection, "local_repo_path")
}

#[tauri::command]
pub fn set_local_repo_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    db::set_setting(&connection, "local_repo_path", path.trim())
}

#[tauri::command]
pub fn open_app_data_dir(app: AppHandle) -> Result<String, String> {
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?;
    std::fs::create_dir_all(&directory).map_err(|error| format!("创建数据目录失败: {error}"))?;

    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .arg(&directory)
            .spawn()
            .map_err(|error| format!("打开目录失败: {error}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&directory)
            .spawn()
            .map_err(|error| format!("打开目录失败: {error}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&directory)
            .spawn()
            .map_err(|error| format!("打开目录失败: {error}"))?;
    }

    Ok(directory.to_string_lossy().to_string())
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
pub fn list_deploy_runs(
    state: State<'_, AppState>,
    deployment_id: String,
    limit: Option<u32>,
) -> Result<Vec<DeployRunRecord>, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let limit = i64::from(limit.unwrap_or(20).clamp(1, 100));
    db::list_deploy_runs(&connection, &deployment_id, limit)
        .map_err(|error| format!("读取部署历史失败: {error}"))
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
            // 兼容旧版 git：不使用 `git -C`（部分服务器 git 不支持），
            // 通过 --git-dir / --work-tree 指定仓库目录。
            let repo_root = target_text.trim_end_matches('/');
            vec![
                format!("--git-dir={repo_root}/.git"),
                format!("--work-tree={repo_root}"),
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

#[tauri::command]
pub fn sync_bundle_item(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    item_id: String,
) -> Result<String, String> {
    let lock_key = format!("{deployment_id}:{item_id}:bundle");
    {
        let mut syncs = state
            .syncs
            .lock()
            .map_err(|error| format!("同步锁被占用: {error}"))?;
        if !syncs.insert(lock_key.clone()) {
            return Err("该同步项正在执行".to_string());
        }
    }

    let (kind, local_path, remote_path, method, branch, path) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        connection
            .query_row(
                "SELECT s.kind,
                        COALESCE(NULLIF(p.local_path, ''),
                                 (SELECT value FROM settings WHERE key = 'local_repo_path'), ''),
                        d.remote_path, i.method, i.branch, i.path
                 FROM sync_items i
                 JOIN deployments d ON d.id = i.deployment_id
                 JOIN projects p ON p.id = d.project_id
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

    let bundle_path = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("tmp")
        .join(format!(
            "bundle-{}.bundle",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis())
                .unwrap_or(0)
        ));
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("创建临时目录失败: {error}"))?;
    }

    let result = (|| -> Result<String, String> {
        if method != "bundle" {
            return Err(format!("当前同步项不是 bundle，method={method}"));
        }

        let target = {
            let connection = state
                .db
                .lock()
                .map_err(|error| format!("数据库连接被占用: {error}"))?;
            runner::load_target(&connection, &deployment_id)?
        };

        if local_path.trim().is_empty() {
            return Err("未配置本地仓库路径：请在设置中填写「本地仓库」".to_string());
        }
        let source = if path.trim().is_empty() {
            PathBuf::from(local_path)
        } else {
            PathBuf::from(local_path).join(path.trim())
        };
        let dest = if path.trim().is_empty() {
            PathBuf::from(&remote_path)
        } else {
            PathBuf::from(&remote_path).join(path.trim())
        };
        if !source.join(".git").exists() {
            return Err(format!("本地源不是 git 仓库: {}", source.display()));
        }
        if source.join(".git/shallow").exists() {
            return Err(format!(
                "本地仓库是浅克隆，bundle 会缺少历史对象；请先执行 git -C {} fetch --unshallow",
                source.display()
            ));
        }
        if kind == "filesystem"
            && let Some(parent) = dest.parent()
        {
            fs::create_dir_all(parent).map_err(|error| format!("创建目标父目录失败: {error}"))?;
        }

        let branch = if branch.trim().is_empty() {
            "main".to_string()
        } else {
            branch.trim().to_string()
        };
        let mut transcript = String::new();
        let mut run = |args: Vec<String>| -> Result<(), String> {
            transcript.push_str(&format!("$ git {}\n", args.join(" ")));
            let output = run_git(&args)?;
            transcript.push_str(&output);
            if !output.ends_with('\n') {
                transcript.push('\n');
            }
            Ok(())
        };

        let bundle_text = bundle_path.to_string_lossy().to_string();
        run(vec![
            "-C".to_string(),
            source.to_string_lossy().to_string(),
            "bundle".to_string(),
            "create".to_string(),
            bundle_text.clone(),
            branch.clone(),
        ])?;

        if kind == "filesystem" {
            if dest.join(".git").exists() {
                run(vec![
                    "-C".to_string(),
                    dest.to_string_lossy().to_string(),
                    "fetch".to_string(),
                    bundle_text,
                    branch.clone(),
                ])?;
                run(vec![
                    "-C".to_string(),
                    dest.to_string_lossy().to_string(),
                    "merge".to_string(),
                    "--ff-only".to_string(),
                    "FETCH_HEAD".to_string(),
                ])?;
            } else {
                run(vec![
                    "clone".to_string(),
                    "-b".to_string(),
                    branch.clone(),
                    bundle_text,
                    dest.to_string_lossy().to_string(),
                ])?;
            }
        } else {
            let remote_bundle = format!(
                "/tmp/phadmin-bundle-{}.bundle",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_millis())
                    .unwrap_or(0)
            );
            runner::upload_to_target(&target, &bundle_path, &remote_bundle)?;
            transcript.push_str(&format!(
                "$ scp {} -> {}\n",
                bundle_path.display(),
                remote_bundle
            ));

            let dest_text = if path.trim().is_empty() {
                target.remote_path.trim_end_matches('/').to_string()
            } else {
                format!(
                    "{}/{}",
                    target.remote_path.trim_end_matches('/'),
                    path.trim().trim_start_matches('/')
                )
            };
            let parent = Path::new(&dest_text)
                .parent()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "/tmp".to_string());
            runner::ensure_target_dir(&target, &dest_text)?;
            let script = format!(
                "set -e\ntrap 'rm -f {bundle}' EXIT\nif [ -d {git_dir} ]; then\n  git -C {dest} fetch {bundle} {branch}\n  if ! git -C {dest} merge --ff-only FETCH_HEAD; then\n    echo '远程仓库历史与本地不一致，强制对齐到本地提交'\n    git -C {dest} reset --hard FETCH_HEAD\n  fi\nelse\n  mkdir -p {parent}\n  git clone -b {branch} {bundle} {dest}\nfi\n",
                bundle = shell_quote(&remote_bundle),
                git_dir = shell_quote(&format!("{dest_text}/.git")),
                dest = shell_quote(&dest_text),
                branch = shell_quote(&branch),
                parent = shell_quote(&parent),
            );
            transcript.push_str(&format!("$ ssh {} 'sh -lc ...'\n", target.host));
            let (success, output) =
                runner::capture_target_command(&target, "sh", &["-lc".to_string(), script])?;
            transcript.push_str(&output);
            if !success {
                return Err(format!("远程 bundle 同步失败: {}", output.trim()));
            }
        }

        let _ = fs::remove_file(&bundle_path);
        Ok(transcript)
    })();

    let _ = fs::remove_file(&bundle_path);

    let (status, output, error) = match result {
        Ok(output) => ("succeeded".to_string(), output, None),
        Err(error) => ("failed".to_string(), String::new(), Some(error)),
    };
    let run_id = format!(
        "bundle-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    );
    let mut log_path: Option<String> = None;
    if let Ok(log_dir) = app.path().app_data_dir().map(|dir| dir.join("logs")) {
        let _ = fs::create_dir_all(&log_dir);
        let path = log_dir.join(format!("{run_id}.log"));
        if let Ok(mut file) = File::create(&path) {
            let _ = file.write_all(output.as_bytes());
            if let Some(error) = &error {
                let _ = writeln!(file, "[error] {error}");
            }
            log_path = Some(path.to_string_lossy().to_string());
        }
    }

    if let Ok(connection) = app.state::<AppState>().db.lock() {
        let _ = connection.execute(
            "INSERT INTO runs
                (id, deployment_id, command, args_json, status, exit_code,
                 started_at, ended_at, log_path, error_summary)
             VALUES (?1, ?2, 'git bundle', ?3, ?4, ?5, datetime('now'), datetime('now'), ?6, ?7)",
            rusqlite::params![
                &run_id,
                &deployment_id,
                serde_json::json!(["bundle", item_id]).to_string(),
                &status,
                if status == "succeeded" { 0 } else { 1 },
                log_path.as_deref(),
                error.as_deref()
            ],
        );
    }

    if let Ok(mut syncs) = app.state::<AppState>().syncs.lock() {
        syncs.remove(&lock_key);
    }

    match error {
        Some(error) => Err(format!("{error}\n{output}")),
        None => Ok(output),
    }
}

#[tauri::command]
pub fn sync_ftp_item(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    item_id: String,
) -> Result<String, String> {
    let lock_key = format!("{deployment_id}:{item_id}:ftp");

    let (local_path, remote_path, method, path, excludes_json) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        connection
            .query_row(
                "SELECT COALESCE(NULLIF(p.local_path, ''),
                                 (SELECT value FROM settings WHERE key = 'local_repo_path'), ''),
                        d.remote_path, i.method, i.path, i.excludes_json
                 FROM sync_items i
                 JOIN deployments d ON d.id = i.deployment_id
                 JOIN projects p ON p.id = d.project_id
                 WHERE i.id = ?1 AND i.deployment_id = ?2",
                rusqlite::params![&item_id, &deployment_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .map_err(|error| format!("同步项不存在: {error}"))?
    };

    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, &deployment_id)?
    };

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
        if method != "ftp" {
            return Err(format!("当前同步项不是 ftp，method={method}"));
        }
        if local_path.trim().is_empty() {
            return Err("未配置本地仓库路径：请在设置中填写「本地仓库」".to_string());
        }
        let source = if path.trim().is_empty() {
            PathBuf::from(local_path)
        } else {
            PathBuf::from(local_path).join(path.trim())
        };
        if !source.is_dir() {
            return Err(format!("本地源目录不存在: {}", source.display()));
        }
        let excludes: Vec<String> = serde_json::from_str(&excludes_json).unwrap_or_default();
        let mut transcript = String::new();

        if target.kind == "filesystem" {
            let dest = if path.trim().is_empty() {
                PathBuf::from(&remote_path)
            } else {
                PathBuf::from(&remote_path).join(path.trim())
            };
            transcript.push_str(&format!(
                "$ copy {} -> {}\n",
                source.display(),
                dest.display()
            ));
            let copied = copy_tree(&source, &dest, &excludes)?;
            transcript.push_str(&format!("copied {copied} files\n"));
        } else {
            let dest = if path.trim().is_empty() {
                target.remote_path.trim_end_matches('/').to_string()
            } else {
                format!(
                    "{}/{}",
                    target.remote_path.trim_end_matches('/'),
                    path.trim().trim_start_matches('/')
                )
            };
            let parent = Path::new(&dest)
                .parent()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| "/tmp".to_string());
            runner::ensure_target_dir(&target, &parent)?;
            runner::upload_dir_to_target(&target, &source, &dest)?;
            transcript.push_str(&format!("$ scp -r {} -> {}\n", source.display(), dest));
        }

        Ok(transcript)
    })();

    let (status, output, error) = match result {
        Ok(output) => ("succeeded".to_string(), output, None),
        Err(error) => ("failed".to_string(), String::new(), Some(error)),
    };
    let run_id = format!(
        "ftp-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0)
    );
    let mut log_path: Option<String> = None;
    if let Ok(log_dir) = app.path().app_data_dir().map(|dir| dir.join("logs")) {
        let _ = fs::create_dir_all(&log_dir);
        let path = log_dir.join(format!("{run_id}.log"));
        if let Ok(mut file) = File::create(&path) {
            let _ = file.write_all(output.as_bytes());
            if let Some(error) = &error {
                let _ = writeln!(file, "[error] {error}");
            }
            log_path = Some(path.to_string_lossy().to_string());
        }
    }

    if let Ok(connection) = app.state::<AppState>().db.lock() {
        let _ = connection.execute(
            "INSERT INTO runs
                (id, deployment_id, command, args_json, status, exit_code,
                 started_at, ended_at, log_path, error_summary)
             VALUES (?1, ?2, 'ftp sync', ?3, ?4, ?5, datetime('now'), datetime('now'), ?6, ?7)",
            rusqlite::params![
                &run_id,
                &deployment_id,
                serde_json::json!(["ftp", item_id]).to_string(),
                &status,
                if status == "succeeded" { 0 } else { 1 },
                log_path.as_deref(),
                error.as_deref()
            ],
        );
    }

    if let Ok(mut syncs) = app.state::<AppState>().syncs.lock() {
        syncs.remove(&lock_key);
    }

    match error {
        Some(error) => Err(format!("{error}\n{output}")),
        None => Ok(output),
    }
}
