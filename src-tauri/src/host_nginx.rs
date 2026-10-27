use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::deploy_context::{self, DeploymentContext};
use crate::render;
use crate::runner;

const CONFIG_DIR: &str = "/etc/nginx/conf.d";
const SSL_DIR: &str = "/etc/nginx/ssl";

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn conf_path(project: &str) -> String {
    format!("{CONFIG_DIR}/{project}.conf")
}

fn ensure_host_target(context: &DeploymentContext) -> Result<(), String> {
    if context.target.kind == "filesystem" {
        return Err("filesystem 目标没有宿主机 Nginx，此操作不可用".to_string());
    }
    if context.target.host.trim().is_empty() {
        return Err("服务器未配置 host".to_string());
    }
    Ok(())
}

fn run_script(context: &DeploymentContext, script: &str) -> Result<String, String> {
    let (success, output) = runner::capture_target_command(
        &context.target,
        "sh",
        &["-lc".to_string(), script.to_string()],
    )?;
    if !success {
        return Err(format!("远程命令失败: {}", output.trim()));
    }
    Ok(output)
}

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

/// 生成并上传宿主机 Nginx 站点配置；`nginx -t` 失败时回滚到上传前的版本。
pub(crate) fn publish_config(
    state: &State<'_, AppState>,
    deployment_id: &str,
    ssl_override: Option<bool>,
) -> Result<String, String> {
    let context = deploy_context::load_state(state, deployment_id)?;
    ensure_host_target(&context)?;
    if context.domains.is_empty() {
        return Err("项目未配置域名，无法生成 Nginx 配置".to_string());
    }

    let ssl = ssl_override.unwrap_or(context.ssl);
    let content = render::render_nginx(&context.domains, context.nginx_port, ssl);
    let remote = conf_path(&context.project);
    let stamp = millis();
    let backup = format!("{remote}.bak-{stamp}");

    let prepare = format!(
        "mkdir -p {}\nif [ -f {remote} ]; then cp -a {remote} {backup}; fi",
        shell_quote(CONFIG_DIR),
        remote = shell_quote(&remote),
        backup = shell_quote(&backup),
    );
    run_script(&context, &prepare)?;
    runner::upload_text_to_target(&context.target, &remote, &content).map_err(|error| {
        format!("上传 {remote} 失败（SSH 用户需要能写 /etc/nginx/conf.d，通常使用 root）: {error}")
    })?;

    let verify = format!(
        "if nginx -t; then nginx -s reload; else cp -a {backup} {remote} 2>/dev/null || rm -f {remote}; echo '配置校验失败，已回滚'; exit 1; fi",
        backup = shell_quote(&backup),
        remote = shell_quote(&remote),
    );
    let output = run_script(&context, &verify)
        .map_err(|error| format!("Nginx 配置校验失败，已回滚到原配置: {error}"))?;

    Ok(format!(
        "已发布 Nginx 配置: {}:{remote}\n{output}",
        context.target.host
    ))
}

#[tauri::command]
pub fn publish_nginx_config(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    publish_config(&state, &deployment_id, None)
}

#[tauri::command]
pub fn remove_nginx_config(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_host_target(&context)?;
    let remote = conf_path(&context.project);
    let script = format!(
        "if [ -f {remote} ]; then rm -f {remote}; nginx -t && nginx -s reload; echo '已移除 {remote}'; else echo '配置不存在: {remote}'; fi",
        remote = shell_quote(&remote),
    );
    run_script(&context, &script)
}

#[tauri::command]
pub fn reload_host_nginx(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_host_target(&context)?;
    run_script(&context, "nginx -t && nginx -s reload")
}

#[tauri::command]
pub fn read_nginx_log(
    state: State<'_, AppState>,
    deployment_id: String,
    kind: String,
) -> Result<String, String> {
    let kind = match kind.as_str() {
        "access" | "error" => kind,
        _ => return Err("日志类型只能是 access 或 error".to_string()),
    };
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_host_target(&context)?;
    let script = format!(
        "tail -n 200 {} 2>&1 || echo '日志文件不存在'",
        shell_quote(&format!("/var/log/nginx/{kind}.log"))
    );
    run_script(&context, &script)
}

/// 服务器环境体检（替代 legacy `server:init` 的检测部分）。
#[tauri::command]
pub fn check_server(state: State<'_, AppState>, deployment_id: String) -> Result<String, String> {
    let context = deploy_context::load_state(&state, &deployment_id)?;
    ensure_host_target(&context)?;
    let script = r#"echo "== docker"
(docker --version || true) 2>&1
(docker compose version || true) 2>&1
echo "== nginx"
(nginx -v || true) 2>&1
echo "== certbot"
(certbot --version || echo '未安装') 2>&1
echo "== 80/443"
(ss -lntp 2>/dev/null || netstat -lntp 2>/dev/null || true) | grep -E ':(80|443)[[:space:]]' || echo '未监听'
echo "== conf.d"
ls -1 /etc/nginx/conf.d 2>/dev/null || echo '目录不存在'
echo "== ssl"
ls -1 /etc/nginx/ssl 2>/dev/null || echo '无证书'"#;
    let (_, output) = runner::capture_target_command(
        &context.target,
        "sh",
        &["-lc".to_string(), script.to_string()],
    )?;
    Ok(output)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SslResult {
    pub primary_domain: String,
    pub certificate: String,
    pub key: String,
    pub transcript: String,
}

/// 用宿主机 certbot（webroot）申请证书、链接到 /etc/nginx/ssl，并切到 HTTPS 配置。
#[tauri::command]
pub fn enable_ssl(
    state: State<'_, AppState>,
    deployment_id: String,
    email: String,
) -> Result<SslResult, String> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') || email.contains(char::is_whitespace) {
        return Err("请填写有效的证书通知邮箱".to_string());
    }

    let context = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        deploy_context::load(&connection, &deployment_id)?
    };
    ensure_host_target(&context)?;
    if context.domains.is_empty() {
        return Err("项目未配置域名，已跳过 SSL".to_string());
    }

    let primary = context.domains[0].clone();
    let webroot = format!("{}/src/public", context.remote_path);
    let domain_args = context
        .domains
        .iter()
        .map(|domain| format!("-d {}", shell_quote(domain)))
        .collect::<Vec<_>>()
        .join(" ");
    let script = format!(
        r#"set -e
command -v certbot >/dev/null 2>&1 || {{ echo 'certbot 未安装（apt install certbot）'; exit 2; }}
test -d {webroot} || {{ echo 'webroot 不存在: {webroot}'; exit 2; }}
mkdir -p {ssl_dir}
certbot certonly --webroot -w {webroot} {domains} --non-interactive --agree-tos -m {email} 2>&1
ln -sf {letsencrypt}/{primary}/fullchain.pem {ssl_dir}/{primary}.pem
ln -sf {letsencrypt}/{primary}/privkey.pem {ssl_dir}/{primary}.key
echo '证书已就绪'"#,
        webroot = shell_quote(&webroot),
        ssl_dir = shell_quote(SSL_DIR),
        domains = domain_args,
        email = shell_quote(email),
        letsencrypt = "/etc/letsencrypt/live",
        primary = primary,
    );

    let transcript =
        run_script(&context, &script).map_err(|error| format!("SSL 申请失败: {error}"))?;

    // 先把 HTTPS 配置发布成功，再落库 ssl=1；发布失败时数据库仍保持 HTTP 状态。
    let publish = publish_config(&state, &deployment_id, Some(true))
        .map_err(|error| format!("证书已签发，但 HTTPS 配置发布失败: {error}"))?;

    {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        connection
            .execute(
                "UPDATE deployments SET ssl = 1, updated_at = datetime('now') WHERE id = ?1",
                params![&deployment_id],
            )
            .map_err(|error| format!("更新 SSL 状态失败: {error}"))?;
    }

    Ok(SslResult {
        primary_domain: primary.clone(),
        certificate: format!("{SSL_DIR}/{primary}.pem"),
        key: format!("{SSL_DIR}/{primary}.key"),
        transcript: format!("{transcript}\n{publish}"),
    })
}
