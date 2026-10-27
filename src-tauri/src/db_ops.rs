use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};

use crate::AppState;
use crate::deploy_context::{self, DeploymentContext, safe_name};
use crate::runner;

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn ensure_remote(context: &DeploymentContext) -> Result<(), String> {
    if context.target.kind == "filesystem" {
        return Err("filesystem 目标没有容器，数据库运维命令不可用".to_string());
    }
    Ok(())
}

fn run_script(context: &DeploymentContext, script: &str) -> Result<(bool, String), String> {
    runner::capture_target_command(
        &context.target,
        "sh",
        &["-lc".to_string(), script.to_string()],
    )
}

fn compose_exec(context: &DeploymentContext, args: &[&str]) -> Result<(bool, String), String> {
    let args = args
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    runner::capture_target_command(&context.target, "docker", &args)
}

fn parse_time(value: &str) -> Result<(u32, u32), String> {
    let (hour, minute) = value
        .split_once(':')
        .ok_or_else(|| format!("时间格式应为 HH:MM：{value}"))?;
    let hour: u32 = hour
        .parse()
        .map_err(|_| format!("时间格式应为 HH:MM：{value}"))?;
    let minute: u32 = minute
        .parse()
        .map_err(|_| format!("时间格式应为 HH:MM：{value}"))?;
    if hour > 23 || minute > 59 || value.len() != 5 {
        return Err(format!("时间格式应为 HH:MM（00:00–23:59）：{value}"));
    }
    Ok((hour, minute))
}

/// crontab 里的备份块，`%` 必须转义成 `\%`，否则 cron 会当成换行。
pub(crate) fn build_cron_block(
    project: &str,
    backup_dir: &str,
    db_name: &str,
    time: &str,
    keep_days: u32,
) -> Result<String, String> {
    let (hour, minute) = parse_time(time)?;
    let keep_days = keep_days.clamp(1, 365);
    let prefix = format!("{}_{}_", safe_name(project), safe_name(db_name));
    let file_name = format!("{prefix}$(date +\\%Y\\%m\\%d_\\%H\\%M).sql.gz");
    let redirect = format!("{}/{}", backup_dir.trim_end_matches('/'), file_name);
    let dump = format!(
        "docker exec {}-mysql sh -c 'exec mysqldump --single-transaction --routines --triggers --events -u\"$MYSQL_USER\" -p\"$MYSQL_PASSWORD\" \"$MYSQL_DATABASE\"'",
        project
    );
    Ok(format!(
        "# BEGIN phadmin-backup {project}\n{minute:02} {hour:02} * * * {dump} | gzip > {redirect} && find {} -name '*.sql.gz' -mtime +{keep_days} -delete\n# END phadmin-backup {project}",
        backup_dir.trim_end_matches('/')
    ))
}

pub(crate) fn strip_cron_block(existing: &str, project: &str) -> String {
    let begin = format!("# BEGIN phadmin-backup {project}");
    let end = format!("# END phadmin-backup {project}");
    let mut kept = Vec::new();
    let mut skipping = false;
    for line in existing.lines() {
        let trimmed = line.trim();
        if trimmed == begin {
            skipping = true;
            continue;
        }
        if skipping {
            if trimmed == end {
                skipping = false;
            }
            continue;
        }
        kept.push(line);
    }
    kept.join("\n")
}

pub(crate) fn upsert_cron_block(existing: &str, project: &str, block: &str) -> String {
    let mut result = strip_cron_block(existing, project).trim_end().to_string();
    if !result.is_empty() {
        result.push_str("\n\n");
    }
    result.push_str(block);
    result.push('\n');
    result
}

fn read_crontab(context: &DeploymentContext) -> Result<String, String> {
    let (_, output) = run_script(context, "crontab -l 2>/dev/null || true")?;
    Ok(output)
}

fn install_crontab(context: &DeploymentContext, content: &str) -> Result<String, String> {
    let remote = format!("/tmp/phadmin-crontab-{}", millis());
    runner::upload_text_to_target(&context.target, &remote, content)?;
    let script = format!(
        "crontab {} && rm -f {} && command -v crontab >/dev/null 2>&1",
        shell_quote(&remote),
        shell_quote(&remote),
    );
    let (success, output) = run_script(context, &script)?;
    if !success {
        return Err(format!("写入 crontab 失败: {}", output.trim()));
    }
    Ok(output)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn backup_dir(context: &DeploymentContext) -> String {
    format!("{}/docker/storage/backup/mysql", context.remote_path)
}

/// 在服务器执行结构迁移（`php artisan migration r`），替代 legacy `db:migrate`。
#[tauri::command]
pub fn migrate_database(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_remote(&context)?;
    let (success, output) = compose_exec(
        &context,
        &[
            "compose",
            "exec",
            "-T",
            "php",
            "php",
            "artisan",
            "migration",
            "r",
        ],
    )?;
    if !success {
        return Err(format!("迁移失败: {}", output.trim()));
    }
    Ok(output)
}

fn seed_directory(app: &AppHandle, deployment_id: &str) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("db")
        .join(deployment_id))
}

/// 上传 `app_data/db/<deployment>/` 下的 SQL，再执行 `artisan db:seed --dir=/var/www/db`。
#[tauri::command]
pub fn seed_database(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_remote(&context)?;

    let local_dir = seed_directory(&app, &deployment_id)?;
    let mut transcript = String::new();
    let remote_dir = format!("{}/src/db", context.remote_path);
    if local_dir.is_dir() {
        let mut files = fs::read_dir(&local_dir)
            .map_err(|error| format!("读取 seed 目录失败: {error}"))?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().map(|ext| ext == "sql").unwrap_or(false))
            .collect::<Vec<_>>();
        files.sort();
        if !files.is_empty() {
            runner::ensure_target_dir(&context.target, &remote_dir)?;
        }
        for file in files {
            let name = file
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_default();
            let destination = format!("{remote_dir}/{name}");
            runner::upload_to_target(&context.target, &file, &destination)?;
            transcript.push_str(&format!("上传 {name}\n"));
        }
    }

    let (success, output) = compose_exec(
        &context,
        &[
            "compose",
            "exec",
            "-T",
            "php",
            "php",
            "artisan",
            "db:seed",
            "--dir=/var/www/db",
        ],
    )?;
    if !success {
        return Err(format!("Seed 失败: {}", output.trim()));
    }
    transcript.push_str(&output);
    Ok(transcript)
}

#[tauri::command]
pub fn open_seed_directory(app: AppHandle, deployment_id: String) -> Result<String, String> {
    let directory = seed_directory(&app, &deployment_id)?;
    fs::create_dir_all(&directory).map_err(|error| format!("创建目录失败: {error}"))?;
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

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupCronResult {
    pub project: String,
    pub time: String,
    pub keep_days: u32,
    pub backup_dir: String,
    pub transcript: String,
}

/// 安装/更新服务器 crontab 每日备份；标记块幂等，`find -mtime` 负责保留策略。
#[tauri::command]
pub fn install_backup_cron(
    state: State<'_, AppState>,
    deployment_id: String,
    time: Option<String>,
    keep_days: Option<u32>,
) -> Result<BackupCronResult, String> {
    let time = time.unwrap_or_else(|| "03:00".to_string());
    let keep_days = keep_days.unwrap_or(7).clamp(1, 365);

    let (context, db_name) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let context = deploy_context::load(&connection, &deployment_id)?;
        let db_name =
            deploy_context::database_name(&connection, &state, &deployment_id, &context.project)?;
        (context, db_name)
    };
    ensure_remote(&context)?;

    let dir = backup_dir(&context);
    let block = build_cron_block(&context.project, &dir, &db_name, &time, keep_days)?;
    let existing = read_crontab(&context)?;
    let updated = upsert_cron_block(&existing, &context.project, &block);

    let (success, output) = run_script(&context, &format!("mkdir -p {}", shell_quote(&dir)))?;
    if !success {
        return Err(format!("创建备份目录失败: {}", output.trim()));
    }
    let output = install_crontab(&context, &updated)?;

    let verify = format!(
        "crontab -l 2>/dev/null | grep -F {} >/dev/null && echo '已安装' || echo '未找到标记块'",
        shell_quote(&format!("# BEGIN phadmin-backup {}", context.project))
    );
    let (verified, verify_output) = run_script(&context, &verify)?;
    if !verified || !verify_output.contains("已安装") {
        return Err(format!("定时备份安装校验失败: {}", verify_output.trim()));
    }

    Ok(BackupCronResult {
        project: context.project,
        time,
        keep_days,
        backup_dir: dir,
        transcript: format!("{output}\n{verify_output}"),
    })
}

#[tauri::command]
pub fn remove_backup_cron(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_remote(&context)?;

    let existing = read_crontab(&context)?;
    if !existing.contains(&format!("# BEGIN phadmin-backup {}", context.project)) {
        return Ok("未发现该项目的定时备份任务".to_string());
    }
    let stripped = strip_cron_block(&existing, &context.project);
    if stripped.trim().is_empty() {
        let (success, output) = run_script(&context, "crontab -r 2>/dev/null || true")?;
        if !success {
            return Err(format!("移除 crontab 失败: {}", output.trim()));
        }
    } else {
        install_crontab(&context, &(stripped.trim_end().to_string() + "\n"))?;
    }
    Ok("定时备份已移除（已生成的备份文件保留在服务器）".to_string())
}

#[tauri::command]
pub fn list_remote_backups(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_remote(&context)?;
    let dir = backup_dir(&context);
    let (success, output) = run_script(
        &context,
        &format!(
            "ls -lh --time-style=long-iso {} 2>/dev/null | grep -v '^total' || echo '暂无备份'",
            shell_quote(&dir)
        ),
    )?;
    if !success {
        return Err(format!("读取服务器备份失败: {}", output.trim()));
    }
    Ok(format!("服务器备份目录: {dir}\n{output}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_time_and_rejects_bad_values() {
        assert_eq!(parse_time("03:00").expect("time"), (3, 0));
        assert_eq!(parse_time("23:59").expect("time"), (23, 59));
        assert!(parse_time("24:00").is_err());
        assert!(parse_time("3:0").is_err());
        assert!(parse_time("03-00").is_err());
    }

    #[test]
    fn cron_block_escapes_percent_and_keeps_dump_inside_container() {
        let block = build_cron_block(
            "boyu",
            "/data/boyu/docker/storage/backup/mysql",
            "boyu_db",
            "03:05",
            7,
        )
        .expect("block");
        assert!(block.contains("05 03 * * *"));
        assert!(block.contains("docker exec boyu-mysql"));
        assert!(block.contains("-mtime +7 -delete"));
        assert!(block.contains("\\%Y\\%m\\%d_\\%H\\%M"));
        assert!(block.contains("# BEGIN phadmin-backup boyu"));
        assert!(block.contains("# END phadmin-backup boyu"));
    }

    #[test]
    fn upsert_replaces_existing_block_and_preserves_other_lines() {
        let existing = "MAILTO=ops@example.com\n\n# BEGIN phadmin-backup boyu\nold line\n# END phadmin-backup boyu\n";
        let block =
            build_cron_block("boyu", "/data/boyu/backup", "boyu_db", "03:00", 7).expect("block");
        let updated = upsert_cron_block(existing, "boyu", &block);
        assert!(updated.contains("MAILTO=ops@example.com"));
        assert!(!updated.contains("old line"));
        assert_eq!(updated.matches("# BEGIN phadmin-backup boyu").count(), 1);
    }

    #[test]
    fn strip_removes_only_this_project_block() {
        let existing = "# BEGIN phadmin-backup a\nx\n# END phadmin-backup a\n# BEGIN phadmin-backup b\ny\n# END phadmin-backup b\n";
        let stripped = strip_cron_block(existing, "a");
        assert!(!stripped.contains("phadmin-backup a"));
        assert!(stripped.contains("phadmin-backup b"));
        assert!(strip_cron_block(stripped.as_str(), "b").trim().is_empty());
    }
}
