use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::Serialize;
use serde_json::Value;
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

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ContainerInfo {
    pub service: String,
    pub name: String,
    pub image: String,
    pub status: String,
    pub state: String,
    pub health: String,
    pub port: String,
}

#[derive(Debug)]
pub(crate) struct Target {
    pub(crate) remote_path: String,
    pub(crate) kind: String,
    pub(crate) host: String,
    pub(crate) port: i64,
    pub(crate) username: String,
    pub(crate) key_path: String,
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

fn create_run_log(app: &AppHandle, run_id: &str) -> Result<(String, Arc<Mutex<File>>), String> {
    let log_dir: PathBuf = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("获取数据目录失败: {error}"))?
        .join("logs");
    fs::create_dir_all(&log_dir).map_err(|error| format!("创建日志目录失败: {error}"))?;
    let path = log_dir.join(format!("{run_id}.log"));
    let file = File::create(&path).map_err(|error| format!("创建日志文件失败: {error}"))?;
    Ok((
        path.to_string_lossy().to_string(),
        Arc::new(Mutex::new(file)),
    ))
}

pub(crate) fn load_target(
    connection: &rusqlite::Connection,
    deployment_id: &str,
) -> Result<Target, String> {
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

fn build_ssh_command(target: &Target, remote: &str) -> Result<Command, String> {
    if target.host.trim().is_empty() {
        return Err("服务器未配置 host".to_string());
    }
    let user = if target.username.trim().is_empty() {
        "root"
    } else {
        target.username.trim()
    };
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
        .arg(remote);
    Ok(command)
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

    let remote = format!(
        "cd {} && {} {}",
        shell_quote(target.remote_path.trim()),
        shell_quote(program),
        args.iter()
            .map(|arg| shell_quote(arg))
            .collect::<Vec<_>>()
            .join(" ")
    );
    build_ssh_command(target, &remote)
}

pub(crate) fn target_path_exists(target: &Target, path: &str) -> Result<bool, String> {
    if target.kind == "filesystem" {
        return Ok(Path::new(path).exists());
    }

    let script = format!("test -e {} && echo yes || echo no", shell_quote(path));
    let mut command = build_command(target, "sh", &["-lc".to_string(), script])?;
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("检查远程路径失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("检查远程路径失败: {}", stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim() == "yes")
}

pub(crate) fn ensure_target_dir(target: &Target, path: &str) -> Result<(), String> {
    if target.kind == "filesystem" {
        return std::fs::create_dir_all(path).map_err(|error| format!("创建目录失败: {error}"));
    }

    let remote = format!("mkdir -p {}", shell_quote(path));
    let mut command = build_ssh_command(target, &remote)?;
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("创建远程目录失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("创建远程目录失败: {}", stderr.trim()));
    }
    Ok(())
}

pub(crate) fn capture_target_command(
    target: &Target,
    program: &str,
    args: &[String],
) -> Result<(bool, String), String> {
    let mut command = build_command(target, program, args)?;
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("执行 {program} 失败: {error}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok((output.status.success(), text))
}

pub(crate) fn upload_to_target(
    target: &Target,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    if target.kind == "filesystem" {
        if let Some(parent) = Path::new(remote_path).parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("创建目标目录失败: {error}"))?;
        }
        return std::fs::copy(local_path, remote_path)
            .map(|_| ())
            .map_err(|error| format!("复制 bundle 失败: {error}"));
    }

    scp_target(target, local_path, remote_path, false)
}

pub(crate) fn upload_dir_to_target(
    target: &Target,
    local_path: &Path,
    remote_path: &str,
) -> Result<(), String> {
    if target.kind == "filesystem" {
        return Err("本地目录同步应使用本地复制".to_string());
    }
    scp_target(target, local_path, remote_path, true)
}

fn scp_target(
    target: &Target,
    local_path: &Path,
    remote_path: &str,
    recursive: bool,
) -> Result<(), String> {
    let user = if target.username.trim().is_empty() {
        "root"
    } else {
        target.username.trim()
    };
    let mut command = Command::new("scp");
    if recursive {
        command.arg("-r");
    }
    command
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg("-P")
        .arg(target.port.to_string());
    if !target.key_path.trim().is_empty() {
        command.arg("-i").arg(target.key_path.trim());
    }
    command
        .arg(local_path)
        .arg(format!("{user}@{}:{remote_path}", target.host.trim()));
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("上传失败: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("上传失败: {}", stderr.trim()));
    }
    Ok(())
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn display_port(value: &Value) -> String {
    if let Some(publishers) = value.get("Publishers").and_then(Value::as_array) {
        for publisher in publishers {
            if let Some(port) = publisher.get("PublishedPort").and_then(Value::as_u64)
                && port > 0
            {
                return port.to_string();
            }
        }
    }

    let ports = string_field(value, "Ports");
    if ports.trim().is_empty() {
        return "-".to_string();
    }
    let first = ports.split(',').next().unwrap_or("").trim();
    if !first.contains("->") {
        return "-".to_string();
    }
    let host = first.split("->").next().unwrap_or(first);
    host.rsplit(':')
        .next()
        .filter(|port| !port.is_empty())
        .unwrap_or("-")
        .to_string()
}

fn container_from_value(value: &Value) -> Option<ContainerInfo> {
    let service = string_field(value, "Service");
    let name = {
        let name = string_field(value, "Name");
        if name.is_empty() {
            string_field(value, "Names")
        } else {
            name
        }
    };
    let image = string_field(value, "Image");
    if service.is_empty() && name.is_empty() && image.is_empty() {
        return None;
    }

    let state = {
        let state = string_field(value, "State");
        if state.is_empty() {
            string_field(value, "Status")
        } else {
            state
        }
    };
    let status_text = string_field(value, "Status");
    let status = if state.eq_ignore_ascii_case("running")
        || status_text.to_ascii_lowercase().starts_with("up")
    {
        "running"
    } else {
        "stopped"
    };

    Some(ContainerInfo {
        service: if service.is_empty() {
            name.clone()
        } else {
            service
        },
        name,
        image,
        status: status.to_string(),
        state,
        health: string_field(value, "Health"),
        port: display_port(value),
    })
}

fn parse_containers(raw: &str) -> Result<Vec<ContainerInfo>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let values: Vec<Value> = if trimmed.starts_with('[') {
        serde_json::from_str(trimmed).map_err(|error| format!("解析容器 JSON 失败: {error}"))?
    } else {
        trimmed
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<Value>, _>>()
            .map_err(|error| format!("解析容器 JSON 行失败: {error}"))?
    };

    Ok(values.iter().filter_map(container_from_value).collect())
}

fn collect_containers(target: &Target) -> Result<Vec<ContainerInfo>, String> {
    let args = vec![
        "compose".to_string(),
        "ps".to_string(),
        "--format".to_string(),
        "json".to_string(),
    ];
    let mut command = build_command(target, "docker", &args)?;
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("执行 docker compose ps 失败: {error}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.trim();
        return Err(if detail.is_empty() {
            "docker compose ps 执行失败".to_string()
        } else {
            format!("docker compose ps 执行失败: {detail}")
        });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_containers(&stdout)
}

#[tauri::command]
pub fn get_containers(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<Vec<ContainerInfo>, String> {
    let target = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        load_target(&connection, &deployment_id)?
    };
    collect_containers(&target)
}

fn stream_pipe<R: Read + Send + 'static>(
    app: AppHandle,
    run_id: String,
    deployment_id: String,
    stream: &'static str,
    reader: R,
    log: Arc<Mutex<File>>,
) {
    std::thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(mut file) = log.lock() {
                let _ = writeln!(file, "[{stream}] {line}");
            }
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

    let (target, run_id, log) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let target = load_target(&connection, &deployment_id)?;
        let run_id = next_run_id();
        let (log_path, log) = create_run_log(&app, &run_id)?;
        let args_json =
            serde_json::to_string(&args).map_err(|error| format!("参数编码失败: {error}"))?;
        connection
            .execute(
                "INSERT INTO runs
                    (id, deployment_id, command, args_json, status, started_at, log_path)
                 VALUES (?1, ?2, ?3, ?4, 'running', datetime('now'), ?5)",
                params![&run_id, &deployment_id, &program, args_json, log_path],
            )
            .map_err(|error| format!("创建运行记录失败: {error}"))?;
        (target, run_id, log)
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
            log.clone(),
        );
    }
    if let Some(stderr) = stderr {
        stream_pipe(
            app.clone(),
            run_id.clone(),
            deployment_id.clone(),
            "stderr",
            stderr,
            log.clone(),
        );
    }

    if let Ok(mut runs) = state.runs.lock() {
        runs.insert(run_id.clone(), child.clone());
    }

    let app_for_wait = app.clone();
    let run_id_for_wait = run_id.clone();
    let deployment_id_for_wait = deployment_id.clone();
    let log_for_wait = log.clone();
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

        if let Ok(mut file) = log_for_wait.lock() {
            let _ = writeln!(
                file,
                "[finished] status={status} exit={}",
                exit_code.map_or("unknown".to_string(), |code| code.to_string())
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
    fn checks_local_target_paths() {
        let target = Target {
            remote_path: ".".to_string(),
            kind: "filesystem".to_string(),
            host: String::new(),
            port: 22,
            username: String::new(),
            key_path: String::new(),
        };
        assert!(target_path_exists(&target, "Cargo.toml").expect("existing path"));
        assert!(!target_path_exists(&target, "missing-path-for-test").expect("missing path"));
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
        assert!(
            args.last()
                .is_some_and(|arg| arg.contains("cd '/data/demo'"))
        );
        assert!(!args.contains(&"-lc".to_string()));
    }

    #[test]
    fn parses_compose_json_array() {
        let raw = r#"[
          {"Service":"nginx","Name":"demo-nginx-1","Image":"nginx:alpine","State":"running","Status":"Up 2 minutes","Health":"healthy","Publishers":[{"PublishedPort":8071}]},
          {"Service":"php","Name":"demo-php-1","Image":"authus/phalcon:5.23.0","State":"exited","Status":"Exited (0) 1 minute ago","Ports":"0.0.0.0:9000->9000/tcp"},
          {"Service":"redis","Name":"demo-redis-1","Image":"redis:7","State":"running","Status":"Up 2 minutes","Ports":"6379/tcp"}
        ]"#;
        let containers = parse_containers(raw).expect("parse");
        assert_eq!(containers.len(), 3);
        assert_eq!(containers[0].service, "nginx");
        assert_eq!(containers[0].status, "running");
        assert_eq!(containers[0].health, "healthy");
        assert_eq!(containers[0].port, "8071");
        assert_eq!(containers[1].status, "stopped");
        assert_eq!(containers[1].port, "9000");
        assert_eq!(containers[2].port, "-");
    }

    #[test]
    fn parses_compose_json_lines() {
        let raw = concat!(
            "{\"Service\":\"nginx\",\"Name\":\"demo-nginx-1\",\"Image\":\"nginx:alpine\",\"Status\":\"Up 1 minute\"}\n",
            "{\"Service\":\"redis\",\"Name\":\"demo-redis-1\",\"Image\":\"redis:7\",\"Status\":\"Exited (0)\"}\n"
        );
        let containers = parse_containers(raw).expect("parse");
        assert_eq!(containers.len(), 2);
        assert_eq!(containers[0].status, "running");
        assert_eq!(containers[1].status, "stopped");
    }

    #[test]
    #[ignore = "needs a local docker compose project via PHADMIN_COMPOSE_DIR"]
    fn collects_real_compose_containers() {
        let path = std::env::var("PHADMIN_COMPOSE_DIR")
            .expect("set PHADMIN_COMPOSE_DIR to a running compose project");
        let target = Target {
            remote_path: path,
            kind: "filesystem".to_string(),
            host: String::new(),
            port: 22,
            username: String::new(),
            key_path: String::new(),
        };
        let containers = collect_containers(&target).expect("collect containers");
        assert!(!containers.is_empty(), "expected at least one container");
        assert!(
            containers
                .iter()
                .any(|container| container.status == "running")
        );
    }
}
