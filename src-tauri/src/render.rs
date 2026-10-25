use tauri::State;

use crate::AppState;

const ENV_TEMPLATE: &str = include_str!("../templates/env.example");

fn render_env(project: &str, remote_path: &str, nginx_port: i64) -> String {
    ENV_TEMPLATE
        .replace("{{TZ}}", "Asia/Shanghai")
        .replace("{{APP_NAME}}", project)
        .replace("{{DATA_PATH_HOST}}", remote_path)
        .replace("{{NGINX_PORT}}", &nginx_port.to_string())
        .replace("{{MYSQL_PASSWORD}}", "")
        .replace("{{MYSQL_USER}}", project)
        .replace("{{MYSQL_DATABASE}}", project)
        .replace("{{REDIS_PASSWORD}}", "")
        .replace("{{NGINX_IMAGE}}", "nginx:stable-alpine")
        .replace("{{PHP_IMAGE}}", "authus/phalcon:5.23.0")
        .replace("{{MYSQL_IMAGE}}", "mysql:8.1.0")
        .replace("{{REDIS_IMAGE}}", "redis:7.2-alpine")
}

#[tauri::command]
pub fn render_config_preview(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let (project, remote_path, nginx_port) = connection
        .query_row(
            "SELECT p.name, d.remote_path, d.nginx_port
             FROM deployments d
             JOIN projects p ON p.id = d.project_id
             WHERE d.id = ?1",
            [&deployment_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .map_err(|error| format!("部署不存在: {error}"))?;
    Ok(render_env(&project, &remote_path, nginx_port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_env_without_placeholders() {
        let env = render_env("demo", "/data/demo", 8071);
        assert!(env.contains("APP_NAME=demo"));
        assert!(env.contains("NGINX_PORT=8071"));
        assert!(env.contains("DATA_PATH_HOST=/data/demo"));
        assert!(env.contains("PHP_IMAGE=authus/phalcon:5.23.0"));
        assert!(!env.contains("{{"));
    }
}
