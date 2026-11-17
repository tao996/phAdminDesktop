use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, NaiveDateTime, Utc};
use regex::{Captures, Regex};
use serde::Serialize;
use tauri::State;

use crate::AppState;
use crate::db;
use crate::runner;

const APP_LOG_LINES: usize = 300;
const SERVICE_LOG_LINES: usize = 150;
const NGINX_ERROR_LINES: usize = 100;
const MAX_LOG_CHARS: usize = 24_000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticContainer {
    pub service: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticFinding {
    pub code: String,
    pub severity: String,
    pub title: String,
    pub detail: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticLog {
    pub source: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRun {
    pub command: String,
    pub args: Vec<String>,
    pub status: String,
    pub error: Option<String>,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticReport {
    pub deployment_id: String,
    pub project_name: String,
    pub server_name: String,
    pub server_host: String,
    pub generated_at: i64,
    pub http_status: Option<u16>,
    pub http_error: Option<String>,
    pub containers: Vec<DiagnosticContainer>,
    pub findings: Vec<DiagnosticFinding>,
    pub logs: Vec<DiagnosticLog>,
    pub runs: Vec<DiagnosticRun>,
    pub errors: Vec<String>,
    pub redactions: usize,
    pub findings_window_minutes: i64,
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn target_path(target: &runner::Target, relative: &str) -> String {
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

fn run_read_only(
    target: &runner::Target,
    program: &str,
    args: &[String],
) -> Result<String, String> {
    let (success, output) = runner::capture_target_command(target, program, args)?;
    if !success {
        return Err(output.trim().to_string());
    }
    Ok(output)
}

fn tail_lines(content: &str, limit: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(limit);
    lines[start..].join("\n")
}

fn truncate_chars(content: &str, limit: usize) -> (String, bool) {
    let total = content.chars().count();
    if total <= limit {
        return (content.to_string(), false);
    }
    (content.chars().skip(total - limit).collect(), true)
}

fn redaction_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        vec![
            Regex::new(
                r#"(?i)((?:password|passwd|pwd|secret|token|api[_-]?key|authorization|cookie)\s*["']?\s*[:=]\s*["']?)([^"'\s,;}\]]+)"#,
            )
            .expect("valid secret redaction regex"),
            Regex::new(r"(?i)(Bearer\s+)([A-Za-z0-9._~+/=-]+)")
                .expect("valid bearer redaction regex"),
        ]
    })
}

fn redact_text(text: &str) -> (String, usize) {
    let mut output = text.to_string();
    let mut count = 0;
    for pattern in redaction_patterns() {
        output = pattern
            .replace_all(&output, |captures: &Captures| {
                if captures.get(2).map(|value| value.as_str()) != Some("***") {
                    count += 1;
                }
                format!("{}***", &captures[1])
            })
            .into_owned();
    }
    (output, count)
}

fn add_log(logs: &mut Vec<DiagnosticLog>, redactions: &mut usize, source: &str, content: String) {
    if content.trim().is_empty() {
        return;
    }
    let (redacted, count) = redact_text(&content);
    *redactions += count;
    let (content, truncated) = truncate_chars(&redacted, MAX_LOG_CHARS);
    logs.push(DiagnosticLog {
        source: source.to_string(),
        content,
        truncated,
    });
}

fn collect_http(target: &runner::Target, port: i64) -> (Option<u16>, Option<String>) {
    if target.kind != "ssh" {
        return (None, Some("本地文件系统目标暂不采集 HTTP 状态".to_string()));
    }
    let args = vec![
        "-k".to_string(),
        "-sS".to_string(),
        "--max-time".to_string(),
        "8".to_string(),
        "-o".to_string(),
        "/dev/null".to_string(),
        "-w".to_string(),
        "%{http_code}".to_string(),
        format!("http://127.0.0.1:{port}/"),
    ];
    match run_read_only(target, "curl", &args) {
        Ok(output) => {
            let code = output
                .split_whitespace()
                .find(|part| part.len() == 3 && part.chars().all(|c| c.is_ascii_digit()))
                .and_then(|part| part.parse::<u16>().ok());
            match code {
                Some(code) => (Some(code), None),
                None => (
                    None,
                    Some(format!("HTTP 检查未返回状态码: {}", output.trim())),
                ),
            }
        }
        Err(error) => (None, Some(format!("HTTP 检查失败: {error}"))),
    }
}

fn collect_containers(
    target: &runner::Target,
    errors: &mut Vec<String>,
) -> Vec<DiagnosticContainer> {
    let args = vec![
        "compose".to_string(),
        "ps".to_string(),
        "-a".to_string(),
        "--format".to_string(),
        "{{.Name}}\t{{.Service}}\t{{.Image}}\t{{.State}}\t{{.Status}}".to_string(),
    ];
    let output = match run_read_only(target, "docker", &args) {
        Ok(output) => output,
        Err(error) => {
            errors.push(format!("读取容器状态失败: {error}"));
            return Vec::new();
        }
    };
    let mut containers = Vec::new();
    for line in output.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 5 || fields[0] == "NAME" {
            continue;
        }
        containers.push(DiagnosticContainer {
            name: fields[0].to_string(),
            service: fields[1].to_string(),
            image: fields[2].to_string(),
            state: fields[3].to_string(),
            status: fields[4].to_string(),
        });
    }
    containers
}

fn newest_local_log(directory: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(directory).ok()?;
    entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "log"))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            Some((metadata.modified().ok()?, entry.path()))
        })
        .max_by_key(|(modified, _)| *modified)
        .map(|(_, path)| path)
}

fn collect_app_log(target: &runner::Target, errors: &mut Vec<String>) -> String {
    if target.kind == "filesystem" {
        let directory = target_path(target, "src/storage/logs");
        let directory = Path::new(&directory);
        return match newest_local_log(directory) {
            Some(path) => fs::read_to_string(path)
                .map(|content| tail_lines(&content, APP_LOG_LINES))
                .unwrap_or_default(),
            None => String::new(),
        };
    }

    let directory = target_path(target, "src/storage/logs");
    let script = format!(
        "latest=$(find {} -maxdepth 1 -type f -name '*.log' -printf '%T@\\t%p\\n' 2>/dev/null | sort -rn | head -n1 | cut -f2-); if [ -n \"$latest\" ]; then tail -n {} -- \"$latest\"; fi",
        shell_quote(&directory),
        APP_LOG_LINES
    );
    match run_read_only(target, "sh", &["-lc".to_string(), script]) {
        Ok(output) => output,
        Err(error) => {
            errors.push(format!("读取应用日志失败: {error}"));
            String::new()
        }
    }
}

fn collect_service_log(target: &runner::Target, service: &str, errors: &mut Vec<String>) -> String {
    let args = vec![
        "compose".to_string(),
        "logs".to_string(),
        "--tail".to_string(),
        SERVICE_LOG_LINES.to_string(),
        service.to_string(),
    ];
    match run_read_only(target, "docker", &args) {
        Ok(output) => output,
        Err(error) => {
            errors.push(format!("读取 {service} 日志失败: {error}"));
            String::new()
        }
    }
}

fn collect_nginx_error_log(target: &runner::Target, errors: &mut Vec<String>) -> String {
    let path = target_path(target, "docker/log/nginx/error.log");
    if target.kind == "filesystem" {
        return fs::read_to_string(&path)
            .map(|content| tail_lines(&content, NGINX_ERROR_LINES))
            .unwrap_or_default();
    }
    let args = vec!["-n".to_string(), NGINX_ERROR_LINES.to_string(), path];
    match run_read_only(target, "tail", &args) {
        Ok(output) => output,
        Err(error) => {
            if !error.contains("No such file") {
                errors.push(format!("读取 Nginx 错误日志失败: {error}"));
            }
            String::new()
        }
    }
}

fn parse_log_time(source: &str, line: &str) -> Option<DateTime<Utc>> {
    let naive = match source {
        "app" => serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|value| {
                value
                    .get("time")
                    .and_then(|time| time.as_str())
                    .map(str::to_string)
            })
            .and_then(|time| NaiveDateTime::parse_from_str(&time, "%Y-%m-%d %H:%M:%S").ok()),
        "php" => {
            let start = line.find('[')?;
            let rest = &line[start + 1..];
            let end = rest.find(']')?;
            NaiveDateTime::parse_from_str(&rest[..end], "%d-%b-%Y %H:%M:%S").ok()
        }
        "nginx" | "nginx-error" => {
            let stamp = line.get(..19)?;
            NaiveDateTime::parse_from_str(stamp, "%Y/%m/%d %H:%M:%S").ok()
        }
        _ => None,
    }?;
    Some(DateTime::from_naive_utc_and_offset(naive, Utc))
}

/// 规则只看最近窗口内的日志，避免把已经修复的历史错误当成当前故障。
fn recent_rule_logs(logs: &[DiagnosticLog], cutoff: DateTime<Utc>) -> Vec<DiagnosticLog> {
    logs.iter()
        .map(|log| {
            let content = log
                .content
                .lines()
                .filter(|line| parse_log_time(&log.source, line).is_none_or(|time| time >= cutoff))
                .collect::<Vec<_>>()
                .join("\n");
            DiagnosticLog {
                source: log.source.clone(),
                content,
                truncated: log.truncated,
            }
        })
        .collect()
}

fn evidence_for(logs: &[DiagnosticLog], needles: &[&str]) -> Vec<String> {
    let mut evidence = Vec::new();
    for log in logs {
        for line in log.content.lines() {
            let lower = line.to_lowercase();
            if needles.iter().any(|needle| lower.contains(needle)) {
                let line = line.trim();
                if !line.is_empty() && !evidence.iter().any(|item| item == line) {
                    evidence.push(line.chars().take(300).collect());
                    if evidence.len() >= 3 {
                        return evidence;
                    }
                }
            }
        }
    }
    evidence
}

fn push_finding(
    findings: &mut Vec<DiagnosticFinding>,
    code: &str,
    severity: &str,
    title: &str,
    detail: &str,
    evidence: Vec<String>,
) {
    if findings.iter().any(|finding| finding.code == code) {
        return;
    }
    findings.push(DiagnosticFinding {
        code: code.to_string(),
        severity: severity.to_string(),
        title: title.to_string(),
        detail: detail.to_string(),
        evidence,
    });
}

fn build_findings(
    http_status: Option<u16>,
    containers: &[DiagnosticContainer],
    logs: &[DiagnosticLog],
) -> Vec<DiagnosticFinding> {
    let mut findings = Vec::new();

    if matches!(http_status, Some(500)) {
        push_finding(
            &mut findings,
            "HTTP_500",
            "critical",
            "HTTP 500",
            "应用返回 500，优先查看应用日志和 PHP 错误日志中的异常堆栈。",
            Vec::new(),
        );
    }
    if matches!(http_status, Some(502)) {
        push_finding(
            &mut findings,
            "HTTP_502",
            "critical",
            "HTTP 502",
            "Nginx 无法连接上游 PHP，检查 php 容器状态并在重建后重载 Nginx。",
            Vec::new(),
        );
    }

    let mysql_evidence = evidence_for(logs, &["sqlstate[hy000] [1045]", "access denied for user"]);
    if !mysql_evidence.is_empty() {
        push_finding(
            &mut findings,
            "MYSQL_AUTH",
            "critical",
            "MySQL 认证失败",
            "数据库账号或密码不一致。核对 .env 中 MYSQL_DATABASE/MYSQL_PASSWORD 与数据卷内实际账号。",
            mysql_evidence,
        );
    }

    let parse_evidence = evidence_for(logs, &["parse error", "syntax error"]);
    if !parse_evidence.is_empty() {
        push_finding(
            &mut findings,
            "PHP_PARSE",
            "critical",
            "PHP 配置语法错误",
            "根据日志中的文件和行号修正 PHP 配置，再重建/重启 php。",
            parse_evidence,
        );
    }

    let redis_evidence = evidence_for(
        logs,
        &["getaddrinfo for redis failed", "name does not resolve"],
    );
    if !redis_evidence.is_empty() {
        push_finding(
            &mut findings,
            "REDIS_DNS",
            "warning",
            "Redis 域名解析失败",
            "检查容器网络、redis 服务名和 Docker DNS；重建容器后确认 getent hosts redis。",
            redis_evidence,
        );
    }

    let upstream_evidence = evidence_for(logs, &["connect() failed", "upstream"]);
    if !upstream_evidence.is_empty() {
        push_finding(
            &mut findings,
            "UPSTREAM",
            "critical",
            "Nginx 上游不可达",
            "php 容器可能已重建但 Nginx 仍缓存旧 IP；执行 nginx -s reload。",
            upstream_evidence,
        );
    }

    let db_conn_evidence = evidence_for(logs, &["sqlstate[hy000] [2002]", "connection refused"]);
    if !db_conn_evidence.is_empty() {
        push_finding(
            &mut findings,
            "DB_CONNECT",
            "critical",
            "数据库连接失败",
            "检查 mysql 容器是否运行、端口/主机名是否正确。",
            db_conn_evidence,
        );
    }

    let disk_evidence = evidence_for(logs, &["no space left on device"]);
    if !disk_evidence.is_empty() {
        push_finding(
            &mut findings,
            "DISK_FULL",
            "critical",
            "磁盘空间不足",
            "清理服务器磁盘或日志后重试。",
            disk_evidence,
        );
    }

    let permission_evidence = evidence_for(logs, &["permission denied"]);
    if !permission_evidence.is_empty() {
        push_finding(
            &mut findings,
            "PERMISSION",
            "warning",
            "文件/目录权限问题",
            "检查容器内运行用户对项目目录和 storage 的读写权限。",
            permission_evidence,
        );
    }

    let scanner_evidence = evidence_for(logs, &["handler class cannot be loaded"]);
    if !scanner_evidence.is_empty() {
        push_finding(
            &mut findings,
            "ROUTE_SCAN",
            "warning",
            "路由/控制器异常",
            "日志中出现 handler class cannot be loaded；可能是扫描请求，也可能是真实路由或控制器缺失，请结合 URI 与模块配置判断。",
            scanner_evidence,
        );
    }

    for container in containers {
        if !container.state.is_empty() && container.state != "running" {
            push_finding(
                &mut findings,
                "CONTAINER_DOWN",
                "warning",
                "容器未运行",
                "检查容器退出原因和最近日志，再决定重启或重建。",
                vec![format!(
                    "{} service={} state={} status={}",
                    container.name, container.service, container.state, container.status
                )],
            );
        }
    }

    findings.sort_by_key(|finding| match finding.severity.as_str() {
        "critical" => 0,
        "warning" => 1,
        _ => 2,
    });
    findings
}

struct DiagnosticContext {
    meta: db::DiagnosticDeployment,
    target: runner::Target,
    run_rows: Vec<db::DiagnosticRunRow>,
}

fn load_context(
    connection: &rusqlite::Connection,
    deployment_id: &str,
) -> Result<DiagnosticContext, String> {
    Ok(DiagnosticContext {
        meta: db::load_diagnostic_deployment(connection, deployment_id)?,
        target: runner::load_target(connection, deployment_id)?,
        run_rows: db::load_diagnostic_runs(connection, deployment_id, 5)?,
    })
}

fn collect_context(
    context: DiagnosticContext,
    deployment_id: String,
) -> Result<DiagnosticReport, String> {
    let DiagnosticContext {
        meta,
        target,
        run_rows,
    } = context;

    let mut errors = Vec::new();
    let mut redactions = 0usize;
    let mut logs = Vec::new();
    let (http_status, http_error) = collect_http(&target, meta.nginx_port);
    if let Some(error) = &http_error {
        errors.push(error.clone());
    }
    let containers = collect_containers(&target, &mut errors);

    let app_log = collect_app_log(&target, &mut errors);
    add_log(&mut logs, &mut redactions, "app", app_log);
    let php_log = collect_service_log(&target, "php", &mut errors);
    add_log(&mut logs, &mut redactions, "php", php_log);
    let nginx_log = collect_service_log(&target, "nginx", &mut errors);
    add_log(&mut logs, &mut redactions, "nginx", nginx_log);
    let nginx_error = collect_nginx_error_log(&target, &mut errors);
    add_log(&mut logs, &mut redactions, "nginx-error", nginx_error);

    let runs = run_rows
        .into_iter()
        .map(|row| DiagnosticRun {
            command: row.command,
            args: serde_json::from_str(&row.args_json).unwrap_or_default(),
            status: row.status,
            error: row.error_summary,
            started_at: row.started_at,
        })
        .collect();

    let findings = {
        let rule_logs = recent_rule_logs(&logs, Utc::now() - chrono::Duration::minutes(120));
        build_findings(http_status, &containers, &rule_logs)
    };

    Ok(DiagnosticReport {
        deployment_id,
        project_name: meta.project_name,
        server_name: meta.server_name,
        server_host: meta.host,
        generated_at: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs() as i64)
            .unwrap_or(0),
        http_status,
        http_error,
        containers,
        findings,
        logs,
        runs,
        errors,
        redactions,
        findings_window_minutes: 120,
    })
}

/// CLI 使用的同步诊断入口：调用方负责只读数据库连接。
pub(crate) fn collect_for_connection(
    connection: &rusqlite::Connection,
    deployment_id: &str,
) -> Result<DiagnosticReport, String> {
    let context = load_context(connection, deployment_id)?;
    collect_context(context, deployment_id.to_string())
}

#[tauri::command(async)]
pub fn collect_diagnostics(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<DiagnosticReport, String> {
    let context = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        load_context(&connection, &deployment_id)?
    };
    collect_context(context, deployment_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_passwords_and_tokens() {
        let (text, count) = redact_text(
            r#"{"password":"secret1","token":"abc"} MYSQL_PASSWORD=hunter2 Authorization: Bearer xyz"#,
        );
        assert!(count >= 4);
        assert!(!text.contains("secret1"));
        assert!(!text.contains("hunter2"));
        assert!(!text.contains("Bearer xyz"));
    }

    #[test]
    fn detects_mysql_auth_error() {
        let logs = vec![DiagnosticLog {
            source: "php".to_string(),
            content: "SQLSTATE[HY000] [1045] Access denied for user 'admin'@'1.2.3.4'".to_string(),
            truncated: false,
        }];
        let findings = build_findings(Some(500), &[], &logs);
        assert!(findings.iter().any(|finding| finding.code == "MYSQL_AUTH"));
    }

    #[test]
    fn detects_parse_error_and_502() {
        let logs = vec![DiagnosticLog {
            source: "php".to_string(),
            content: "PHP Parse error: syntax error, unexpected end of file".to_string(),
            truncated: false,
        }];
        let findings = build_findings(Some(502), &[], &logs);
        assert!(findings.iter().any(|finding| finding.code == "PHP_PARSE"));
        assert!(findings.iter().any(|finding| finding.code == "HTTP_502"));
    }

    #[test]
    fn flags_stopped_container() {
        let containers = vec![DiagnosticContainer {
            service: "php".to_string(),
            name: "demo-php".to_string(),
            image: "php:8".to_string(),
            state: "exited".to_string(),
            status: "Exited (1)".to_string(),
        }];
        let findings = build_findings(None, &containers, &[]);
        assert!(
            findings
                .iter()
                .any(|finding| finding.code == "CONTAINER_DOWN")
        );
    }

    #[test]
    fn ignores_old_log_lines_but_keeps_recent_ones() {
        let recent = (Utc::now() - chrono::Duration::minutes(5))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let old = (Utc::now() - chrono::Duration::days(1))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let logs = vec![DiagnosticLog {
            source: "app".to_string(),
            content: format!(
                "{{\"time\":\"{old}\",\"level\":\"error\",\"message\":\"old parse error\"}}\n{{\"time\":\"{recent}\",\"level\":\"error\",\"message\":\"current 1045 access denied for user\"}}"
            ),
            truncated: false,
        }];
        let filtered = recent_rule_logs(&logs, Utc::now() - chrono::Duration::minutes(120));
        assert!(!filtered[0].content.contains("old parse error"));
        assert!(filtered[0].content.contains("current 1045"));
    }
}
