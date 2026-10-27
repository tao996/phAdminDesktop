use std::process::Child;
use std::thread;
use std::time::Duration;

use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::deploy_context::{self, DeploymentContext};
use crate::runner;

pub struct Tunnel {
    pub child: Child,
    pub local_port: u16,
    pub remote: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInfo {
    pub deployment_id: String,
    pub local_port: u16,
    pub remote: String,
    pub host: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PmaInfo {
    pub container: String,
    pub url: String,
    pub network: String,
    pub username: String,
    pub password: String,
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn ensure_remote(context: &DeploymentContext) -> Result<(), String> {
    if context.target.kind == "filesystem" {
        return Err("filesystem 目标没有远程服务器，此操作不可用".to_string());
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

/// 重置主仓库和每个 git/bundle 同步目录到最新提交（只丢弃已跟踪文件的修改）。
#[tauri::command]
pub fn reset_project(state: State<'_, AppState>, deployment_id: String) -> Result<String, String> {
    let (context, sync_paths) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let context = deploy_context::load(&connection, &deployment_id)?;
        let mut statement = connection
            .prepare(
                "SELECT path FROM sync_items
                 WHERE deployment_id = ?1 AND method IN ('git', 'bundle') AND path <> ''
                 ORDER BY sort",
            )
            .map_err(|error| format!("读取同步项失败: {error}"))?;
        let rows = statement
            .query_map([&deployment_id], |row| row.get::<_, String>(0))
            .map_err(|error| format!("读取同步项失败: {error}"))?;
        let mut paths = Vec::new();
        for row in rows {
            let path = row.map_err(|error| format!("读取同步项失败: {error}"))?;
            if !path.trim().is_empty() {
                paths.push(path.trim().to_string());
            }
        }
        (context, paths)
    };

    let mut directories = vec![context.remote_path.clone()];
    for path in sync_paths {
        let joined = if context.target.kind == "filesystem" {
            std::path::Path::new(&context.remote_path)
                .join(&path)
                .to_string_lossy()
                .to_string()
        } else {
            format!("{}/{}", context.remote_path.trim_end_matches('/'), path)
        };
        if !directories.contains(&joined) {
            directories.push(joined);
        }
    }

    let mut transcript = String::new();
    let mut reset_any = false;

    if context.target.kind == "filesystem" {
        for directory in &directories {
            let git_dir = std::path::Path::new(directory).join(".git");
            if !git_dir.exists() {
                transcript.push_str(&format!("跳过（非 git 仓库）: {directory}\n"));
                continue;
            }
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(directory)
                .arg("reset")
                .arg("--hard")
                .output()
                .map_err(|error| format!("执行 git reset 失败: {error}"))?;
            if !output.status.success() {
                return Err(format!(
                    "重置 {directory} 失败: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
            reset_any = true;
            transcript.push_str(&format!(
                "重置 {directory}\n{}",
                String::from_utf8_lossy(&output.stdout)
            ));
        }
    } else {
        let script = directories
            .iter()
            .map(|directory| {
                format!(
                    "if [ -d {dir}/.git ]; then echo '== {dir}'; cd {dir} && git reset --hard || exit 1; else echo '跳过（非 git 仓库）: {dir}'; fi",
                    dir = shell_quote(directory)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let (success, output) = run_script(&context, &script)?;
        if !success {
            return Err(format!("重置失败: {}", output.trim()));
        }
        reset_any = output.contains("HEAD is now at") || output.contains("重置");
        transcript.push_str(&output);
    }

    if !reset_any && !transcript.is_empty() {
        return Err(format!("没有可重置的 git 仓库：\n{transcript}"));
    }
    Ok(transcript)
}

/// 为服务器生成（或读取）部署用 ed25519 公钥，替代 legacy `git:ssh`。
#[tauri::command]
pub fn generate_git_deploy_key(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_remote(&context)?;
    let script = "mkdir -p ~/.ssh && chmod 700 ~/.ssh && \
                  [ -f ~/.ssh/id_ed25519 ] || ssh-keygen -t ed25519 -N '' -C phadmin -f ~/.ssh/id_ed25519 >/dev/null 2>&1; \
                  cat ~/.ssh/id_ed25519.pub";
    let (success, output) = run_script(&context, script)?;
    if !success || output.trim().is_empty() {
        return Err(format!("生成部署密钥失败: {}", output.trim()));
    }
    Ok(output.trim().to_string())
}

#[tauri::command]
pub fn start_ssh_tunnel(
    state: State<'_, AppState>,
    deployment_id: String,
    local_port: Option<u16>,
) -> Result<TunnelInfo, String> {
    let (context, project) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        (
            deploy_context::load(&connection, &deployment_id)?,
            deploy_context::project_name(&connection, &deployment_id)?,
        )
    };
    ensure_remote(&context)?;

    let local_port = local_port.unwrap_or(13306);
    if local_port < 1024 {
        return Err("本地端口需要 >= 1024".to_string());
    }
    if state
        .tunnels
        .lock()
        .map_err(|error| format!("隧道锁被占用: {error}"))?
        .contains_key(&deployment_id)
    {
        return Err("该项目的 SSH 隧道已在运行".to_string());
    }

    // compose 模板不发布 MySQL 端口，宿主机也不解析容器名；优先用容器网络 IP 建隧道。
    let container = format!("{project}-mysql");
    let (success, output) = runner::capture_target_command(
        &context.target,
        "docker",
        &[
            "inspect".to_string(),
            "-f".to_string(),
            "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}".to_string(),
            container.clone(),
        ],
    )?;
    let container_ip = if success {
        output.trim().to_string()
    } else {
        String::new()
    };
    let remote = if container_ip.is_empty() {
        container.clone()
    } else {
        container_ip
    };

    let mut child = runner::spawn_ssh_tunnel(&context.target, local_port, &remote, 3306)?;
    thread::sleep(Duration::from_millis(400));
    if let Some(status) = child
        .try_wait()
        .map_err(|error| format!("检查隧道状态失败: {error}"))?
    {
        return Err(format!(
            "SSH 隧道启动失败（ssh 立即退出，代码 {:?}）",
            status.code()
        ));
    }

    state
        .tunnels
        .lock()
        .map_err(|error| format!("隧道锁被占用: {error}"))?
        .insert(
            deployment_id.clone(),
            Tunnel {
                child,
                local_port,
                remote: format!("{remote}:3306"),
            },
        );

    Ok(TunnelInfo {
        deployment_id,
        local_port,
        remote: format!("{remote}:3306"),
        host: context.target.host,
    })
}

#[tauri::command]
pub fn stop_ssh_tunnel(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let tunnel = state
        .tunnels
        .lock()
        .map_err(|error| format!("隧道锁被占用: {error}"))?
        .remove(&deployment_id);
    match tunnel {
        Some(mut tunnel) => {
            let _ = tunnel.child.kill();
            let _ = tunnel.child.wait();
            Ok(format!("SSH 隧道已关闭（本地端口 {}）", tunnel.local_port))
        }
        None => Ok("没有运行中的 SSH 隧道".to_string()),
    }
}

#[tauri::command]
pub fn list_ssh_tunnels(state: State<'_, AppState>) -> Result<Vec<TunnelInfo>, String> {
    let mut tunnels = state
        .tunnels
        .lock()
        .map_err(|error| format!("隧道锁被占用: {error}"))?;
    tunnels.retain(|_, tunnel| matches!(tunnel.child.try_wait(), Ok(None)));
    Ok(tunnels
        .iter()
        .map(|(deployment_id, tunnel)| TunnelInfo {
            deployment_id: deployment_id.clone(),
            local_port: tunnel.local_port,
            remote: tunnel.remote.clone(),
            host: String::new(),
        })
        .collect())
}

#[tauri::command]
pub fn start_phpmyadmin(
    state: State<'_, AppState>,
    deployment_id: String,
    host_port: Option<u16>,
) -> Result<PmaInfo, String> {
    let (context, project) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        (
            deploy_context::load(&connection, &deployment_id)?,
            deploy_context::project_name(&connection, &deployment_id)?,
        )
    };
    ensure_remote(&context)?;

    let (username, password) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        (
            crate::env_vars::resolve(
                &connection,
                state.secrets.as_ref(),
                &deployment_id,
                "MYSQL_USER",
            )?
            .unwrap_or_else(|| "admin".to_string()),
            crate::env_vars::resolve(
                &connection,
                state.secrets.as_ref(),
                &deployment_id,
                "MYSQL_PASSWORD",
            )?
            .unwrap_or_default(),
        )
    };

    let host_port = host_port.unwrap_or(13307);
    if host_port < 1024 {
        return Err("宿主机端口需要 >= 1024".to_string());
    }
    let mysql_container = format!("{project}-mysql");
    let container = format!("{project}-pma");

    let (success, output) = runner::capture_target_command(
        &context.target,
        "docker",
        &[
            "inspect".to_string(),
            "-f".to_string(),
            "{{range $k,$_ := .NetworkSettings.Networks}}{{$k}}{{end}}".to_string(),
            mysql_container.clone(),
        ],
    )?;
    let network = output.trim().to_string();
    if !success || network.is_empty() {
        return Err(format!(
            "无法反查 {mysql_container} 的 Docker 网络，请确认容器已启动"
        ));
    }

    let _ = runner::capture_target_command(
        &context.target,
        "docker",
        &["rm".to_string(), "-f".to_string(), container.clone()],
    );

    let (success, output) = runner::capture_target_command(
        &context.target,
        "docker",
        &[
            "run".to_string(),
            "-d".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            container.clone(),
            "--network".to_string(),
            network.clone(),
            "-p".to_string(),
            format!("{host_port}:80"),
            "-e".to_string(),
            "PMA_HOST=mysql".to_string(),
            "-e".to_string(),
            "PMA_PORT=3306".to_string(),
            "phpmyadmin/phpmyadmin".to_string(),
        ],
    )?;
    if !success {
        return Err(format!("phpMyAdmin 启动失败: {}", output.trim()));
    }

    let (running, state_output) = runner::capture_target_command(
        &context.target,
        "docker",
        &[
            "inspect".to_string(),
            "-f".to_string(),
            "{{.State.Running}}".to_string(),
            container.clone(),
        ],
    )?;
    if !running || state_output.trim() != "true" {
        return Err(format!("phpMyAdmin 容器未运行: {}", state_output.trim()));
    }

    Ok(PmaInfo {
        url: format!("http://{}:{}", context.target.host, host_port),
        container,
        network,
        username,
        password,
    })
}

#[tauri::command]
pub fn stop_phpmyadmin(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let (context, project) = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        (
            deploy_context::load(&connection, &deployment_id)?,
            deploy_context::project_name(&connection, &deployment_id)?,
        )
    };
    ensure_remote(&context)?;
    let container = format!("{project}-pma");
    let (success, output) = runner::capture_target_command(
        &context.target,
        "docker",
        &["rm".to_string(), "-f".to_string(), container.clone()],
    )?;
    if !success && !output.contains("No such container") {
        return Err(format!("删除 phpMyAdmin 失败: {}", output.trim()));
    }
    Ok(format!("已清理 {container}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_quote_escapes_single_quotes() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
    }
}
