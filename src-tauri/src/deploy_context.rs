use rusqlite::{Connection, params};
use tauri::State;

use crate::AppState;
use crate::env_vars;
use crate::runner::{self, Target};

/// 原生运维命令共用的部署上下文：项目名、目录、域名、端口、目标机。
pub(crate) struct DeploymentContext {
    pub project: String,
    pub remote_path: String,
    pub domains: Vec<String>,
    pub nginx_port: i64,
    pub ssl: bool,
    pub target: Target,
}

pub(crate) fn load(
    connection: &Connection,
    deployment_id: &str,
) -> Result<DeploymentContext, String> {
    let (project, remote_path, nginx_port, ssl, domain): (String, String, i64, i64, String) =
        connection
            .query_row(
                "SELECT p.name, d.remote_path, d.nginx_port, d.ssl, d.domain
                 FROM deployments d
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1",
                [deployment_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .map_err(|error| format!("部署不存在: {error}"))?;

    let mut domains = Vec::new();
    let mut statement = connection
        .prepare("SELECT domain FROM domains WHERE deployment_id = ?1 ORDER BY sort")
        .map_err(|error| format!("读取域名失败: {error}"))?;
    let rows = statement
        .query_map([deployment_id], |row| row.get::<_, String>(0))
        .map_err(|error| format!("读取域名失败: {error}"))?;
    for row in rows {
        let value = row.map_err(|error| format!("读取域名失败: {error}"))?;
        if !value.trim().is_empty() {
            domains.push(value.trim().to_string());
        }
    }
    if domains.is_empty() && !domain.trim().is_empty() {
        domains.push(domain.trim().to_string());
    }

    let target = runner::load_target(connection, deployment_id)?;

    Ok(DeploymentContext {
        project,
        remote_path: remote_path.trim_end_matches(['/', '\\']).to_string(),
        domains,
        nginx_port,
        ssl: ssl != 0,
        target,
    })
}

pub(crate) fn load_state(
    state: &State<'_, AppState>,
    deployment_id: &str,
) -> Result<DeploymentContext, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    load(&connection, deployment_id)
}

/// 容器使用的数据库名；和 compose 模板、备份命令保持同一来源。
pub(crate) fn database_name(
    connection: &Connection,
    state: &State<'_, AppState>,
    deployment_id: &str,
    project: &str,
) -> Result<String, String> {
    let value = env_vars::resolve(
        connection,
        state.secrets.as_ref(),
        deployment_id,
        "MYSQL_DATABASE",
    )?
    .unwrap_or_default();
    if value.trim().is_empty() {
        Ok(project.to_string())
    } else {
        Ok(value.trim().to_string())
    }
}

pub(crate) fn project_name(connection: &Connection, deployment_id: &str) -> Result<String, String> {
    connection
        .query_row(
            "SELECT p.name FROM deployments d JOIN projects p ON p.id = d.project_id WHERE d.id = ?1",
            params![deployment_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("项目不存在: {error}"))
}

/// 只保留文件名安全字符，避免 cron / 备份文件名里出现路径分隔符。
pub(crate) fn safe_name(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        "project".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_name_strips_path_characters() {
        assert_eq!(safe_name("boyu/demo db"), "boyu_demo_db");
        assert_eq!(safe_name(""), "project");
        assert_eq!(safe_name("a-b_c9"), "a-b_c9");
    }
}
