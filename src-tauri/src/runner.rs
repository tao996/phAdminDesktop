use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);

const ALLOWED_PROGRAMS: &[&str] = &[
    "docker", "php", "git", "ssh", "ls", "pwd", "whoami", "composer",
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOutput {
    pub run_id: String,
    pub deployment_id: String,
    pub stream: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFinished {
    pub run_id: String,
    pub deployment_id: String,
    pub exit_code: Option<i32>,
    pub status: String,
}

#[derive(Debug)]
struct Target {
    remote_path: String,
    kind: String,
    host: String,
    port: i64,
    username: String,
    key_path: String,
}

fn next_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!(
        "run-{}-{}",
        millis,
        RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn load_target(connection: &rusqlite::Connection, deployment_id: &str) -> Result<Target, String> {
    connection
        .query_row(
            "SELECT d.remote_path, s.kind, s.host, s.port, s.username, s.key_path
             FROM deployments d
             JOIN servers s ON s.id = d.server_id
             WHERE d.id = ?1",
            params![deployment_id],
            |row| {
                Ok(Target {
                    remote_path: row.get(0)?,
                    kind: row.get(1)?,
                    host: row.get(2)?,
                    port: row.get(3)?,
                    username: row.get(4)?,
                    key_path: row.get(5)?,
                })
            },
        )
        .map_err(|error| format!("部署不存在或服务器不可用: {error}"))
}

fn build_command(target: &Target, program: &str, args: &[String]) -> Result<Command, String> {
    if target.kind == "filesystem" {
        let mut command = Command::new(program);
        command.args(args);
        if !target.remote_path.trim().is_empty() {
            command.current_dir(target.remote_path.trim());
        }
        return Ok(command);
    }

    if target.host.trim().is_empty() {
        return Err("服务器未配置 host".to_string());
    }

    let user = if target.username.trim().is_empty() {
        "root"
    } else {
        target.username.trim()
    };
    let remote = format!(
        "cd {} && {} {}",
        shell_quote(target.remote_path.trim()),
        shell_quote(program),
        args.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );

    let mut command = Command::new("ssh");
    command
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-o")
        .arg("ConnectTimeout=10");
    if !target.key_path.trim().is_empty() {
        command.arg("-i").arg(target.key_path.trim());
    }
    command
        .arg("-p")
        .arg(target.port.to_string())
        .arg(format!("{user}@{}", target.host.trim()))
        .arg("--")
        .arg("sh")
        .arg("-lc")
        .arg(remote);
    Ok(command)
}

fn stream_pipe<R: Read + Send + 'static>(
    app: AppHandle,
    run_id: String,
    deployment_id: String,
    stream: &'static str,
    reader: R,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            let _ = app.emit(
                "run-output",
                RunOutput {
                    run_id: run_id.clone(),
                    deployment_id: deployment_id.clone(),
                    stream: stream.to_string(),
                    line,
                },
            );
        }
    });
}

#[tauri::command]
pub fn run_command(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    program: String,
    args: Vec<String>,
) -> Result<String, String> {
    if !ALLOWED_PROGRAMS.contains(&program.as_str()) {
        return Err(format!(
            "命令不在允许列表：{program}（允许：{}）",
            ALLOWED_PROGRAMS.join(", ")
        ));
    }

    let (target, run_id) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let target = load_target(&connection, &deployment_id)?;
        let run_id = next_run_id();
        let args_json =
            serde_json::to_string(&args).map_err(|error| format!("参数编码失败: {error}"))?;
        connection
            .execute(
                "INSERT INTO runs
                    (id, deployment_id, command, args_json, status, started_at)
                 VALUES (?1, ?2, ?3, ?4, 'running', datetime('now'))",
                params![&run_id, &deployment_id, &program, args_json],
            )
            .map_err(|error| format!("创建运行记录失败: {error}"))?;
        (target, run_id)
    };

    let mut command = match build_command(&target, &program, &args) {
        Ok(command) => command,
        Err(error) => {
            if let Ok(connection) = state.db.lock() {
                let _ = connection.execute(
                    "UPDATE runs SET status = 'failed', ended_at = datetime('now'), error_summary = ?1
                     WHERE id = ?2",
                    params![&error, &run_id],
                );
            }
            return Err(error);
        }
    };

    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            if let Ok(connection) = state.db.lock() {
                let _ = connection.execute(
                    "UPDATE runs SET status = 'failed', ended_at = datetime('now'), error_summary = ?1
                     WHERE id = ?2",
                    params![format!("启动进程失败: {error}"), &run_id],
                );
            }
            return Err(format!("启动进程失败: {error}"));
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(child));

    if let Some(stdout) = stdout {
        stream_pipe(
            app.clone(),
            run_id.clone(),
            deployment_id.clone(),
            "stdout",
            stdout,
        );
    }
    if let Some(stderr) = stderr {
        stream_pipe(
            app.clone(),
            run_id.clone(),
            deployment_id.clone(),
            "stderr",
            stderr,
        );
    }

    if let Ok(mut runs) = state.runs.lock() {
        runs.insert(run_id.clone(), child.clone());
    }

    let app_for_wait = app.clone();
    let run_id_for_wait = run_id.clone();
    let deployment_id_for_wait = deployment_id.clone();
    std::thread::spawn(move || {
        let exit_code = child
            .lock()
            .ok()
            .and_then(|mut child| child.wait().ok())
            .and_then(|status| status.code());

        let state = app_for_wait.state::<AppState>();
        let canceled = state
            .canceled
            .lock()
            .map(|mut canceled| canceled.remove(&run_id_for_wait))
            .unwrap_or(false);
        if let Ok(mut runs) = state.runs.lock() {
            runs.remove(&run_id_for_wait);
        }

        let status = if canceled {
            "canceled"
        } else if exit_code == Some(0) {
            "succeeded"
        } else {
            "failed"
        };

        if let Ok(connection) = state.db.lock() {
            let _ = connection.execute(
                "UPDATE runs
                 SET status = ?1, exit_code = ?2, ended_at = datetime('now')
                 WHERE id = ?3",
                params![status, exit_code, &run_id_for_wait],
            );
        }

        let _ = app_for_wait.emit(
            "run-finished",
            RunFinished {
                run_id: run_id_for_wait,
                deployment_id: deployment_id_for_wait,
                exit_code,
                status: status.to_string(),
            },
        );
    });

    Ok(run_id)
}

#[tauri::command]
pub fn cancel_run(state: State<'_, AppState>, run_id: String) -> Result<(), String> {
    let child = state
        .runs
        .lock()
        .map_err(|error| format!("任务列表被占用: {error}"))?
        .get(&run_id)
        .cloned();
    let Some(child) = child else {
        return Ok(());
    };

    state
        .canceled
        .lock()
        .map_err(|error| format!("任务状态被占用: {error}"))?
        .insert(run_id.clone());
    child
        .lock()
        .map_err(|error| format!("任务进程被占用: {error}"))?
        .kill()
        .map_err(|error| format!("终止进程失败: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_shell_arguments() {
        assert_eq!(shell_quote("plain"), "'plain'");
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
        assert_eq!(shell_quote("a b"), "'a b'");
    }

    #[test]
    fn builds_local_command_with_workdir() {
        let target = Target {
            remote_path: "D:/demo/project".to_string(),
            kind: "filesystem".to_string(),
            host: String::new(),
            port: 22,
            username: String::new(),
            key_path: String::new(),
        };
        let command = build_command(&target, "docker", &["compose".to_string()]).expect("command");
        assert_eq!(command.get_program(), "docker");
        assert_eq!(
            command.get_current_dir().and_then(|path| path.to_str()),
            Some("D:/demo/project")
        );
    }

    #[test]
    fn builds_ssh_command_with_key() {
        let target = Target {
            remote_path: "/data/demo".to_string(),
            kind: "ssh".to_string(),
            host: "example.com".to_string(),
            port: 2222,
            username: "deploy".to_string(),
            key_path: "C:/keys/id_ed25519".to_string(),
        };
        let command = build_command(&target, "docker", &["compose".to_string()]).expect("command");
        let args: Vec<_> = command
            .get_args()
            .map(|arg| arg.to_string_lossy().to_string())
            .collect();
        assert_eq!(command.get_program(), "ssh");
        assert!(args.contains(&"-i".to_string()));
        assert!(args.contains(&"C:/keys/id_ed25519".to_string()));
        assert!(args.contains(&"deploy@example.com".to_string()));
        assert!(args.contains(&"2222".to_string()));
    }
}
