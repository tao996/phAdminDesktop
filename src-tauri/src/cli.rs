use std::env;
use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Local};

use crate::db;
use crate::diagnostics::{self, DiagnosticReport};

const HELP: &str = r#"PhAdmin Desktop 诊断命令行

用法:
  phadmin-cli projects
  phadmin-cli diagnose <项目或部署ID> [--format markdown|json] [--output 文件]
  phadmin-cli error <项目或部署ID>     diagnose 的别名
  phadmin-cli logs <项目> [--source app|php|nginx|nginx-error]
  phadmin-cli containers <项目>
  phadmin-cli export <项目> [--output 文件]

说明:
  - 只读：不会修改服务器、数据库或项目文件
  - 日志在输出前会脱敏 password/token/secret 等字段
  - 规则只分析最近 120 分钟日志
  - 可用 PHADMIN_DB 环境变量指定数据库路径
"#;

fn app_data_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        let base = env::var_os("APPDATA").ok_or_else(|| "未找到 APPDATA 环境变量".to_string())?;
        return Ok(PathBuf::from(base).join("cn.gu19.phadmin.desktop"));
    }
    #[cfg(target_os = "macos")]
    {
        let home = env::var_os("HOME").ok_or_else(|| "未找到 HOME 环境变量".to_string())?;
        return Ok(PathBuf::from(home).join("Library/Application Support/cn.gu19.phadmin.desktop"));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let base = env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local/share"))
            })
            .ok_or_else(|| "未找到 XDG_DATA_HOME/HOME 环境变量".to_string())?;
        return Ok(base.join("cn.gu19.phadmin.desktop"));
    }
    #[allow(unreachable_code)]
    Err("当前平台不支持命令行诊断".to_string())
}

fn database_path() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("PHADMIN_DB") {
        return Ok(PathBuf::from(path));
    }
    Ok(app_data_dir()?.join("phadmin.db"))
}

fn timestamp(value: i64) -> String {
    DateTime::from_timestamp(value, 0)
        .map(|time| {
            time.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| value.to_string())
}

fn project_list() -> Result<String, String> {
    let path = database_path()?;
    if !path.is_file() {
        return Err(format!(
            "数据库不存在: {}\n请先启动一次 PhAdmin Desktop",
            path.display()
        ));
    }
    let connection =
        db::open_read_only(&path).map_err(|error| format!("打开数据库失败: {error}"))?;
    let rows = db::list_project_deployments(&connection)?;
    let mut output = String::from("项目\t服务器\t状态\t远程路径\t部署 ID\n");
    for row in rows {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            row.project_name, row.server_name, row.status, row.remote_path, row.deployment_id
        ));
    }
    Ok(output)
}

fn resolve_deployment(connection: &rusqlite::Connection, query: &str) -> Result<String, String> {
    if let Some(id) = db::resolve_project_deployment(connection, query)? {
        return Ok(id);
    }
    let rows = db::list_project_deployments(connection)?;
    let names = rows
        .iter()
        .map(|row| format!("{} ({})", row.project_name, row.server_name))
        .collect::<Vec<_>>()
        .join(", ");
    Err(format!("项目不存在: {query}\n可用项目: {names}"))
}

fn collect(query: &str) -> Result<DiagnosticReport, String> {
    let path = database_path()?;
    if !path.is_file() {
        return Err(format!(
            "数据库不存在: {}\n请先启动一次 PhAdmin Desktop",
            path.display()
        ));
    }
    let connection =
        db::open_read_only(&path).map_err(|error| format!("打开数据库失败: {error}"))?;
    let deployment_id = resolve_deployment(&connection, query)?;
    diagnostics::collect_for_connection(&connection, &deployment_id)
}

fn markdown(report: &DiagnosticReport) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "# PhAdmin 诊断：{} ({})\n\n",
        report.project_name, report.deployment_id
    ));
    output.push_str(&format!(
        "- 服务器: {} ({})\n- 生成时间: {}\n",
        report.server_name,
        report.server_host,
        timestamp(report.generated_at)
    ));
    output.push_str(&format!(
        "- HTTP: {}\n- 容器: {}/{} running\n- 规则窗口: 最近 {} 分钟\n- 脱敏: {} 处\n\n",
        report
            .http_status
            .map(|status| status.to_string())
            .unwrap_or_else(|| "未采集".to_string()),
        report
            .containers
            .iter()
            .filter(|container| container.state == "running")
            .count(),
        report.containers.len(),
        report.findings_window_minutes,
        report.redactions
    ));

    output.push_str("## 发现\n\n");
    if report.findings.is_empty() {
        output.push_str("未发现已知错误模式。\n\n");
    }
    for finding in &report.findings {
        output.push_str(&format!(
            "### [{}] {} ({})\n\n{}\n\n",
            finding.severity.to_uppercase(),
            finding.title,
            finding.code,
            finding.detail
        ));
        for evidence in &finding.evidence {
            output.push_str(&format!("```\n{evidence}\n```\n"));
        }
        output.push('\n');
    }

    if !report.errors.is_empty() {
        output.push_str("## 采集失败项\n\n");
        for error in &report.errors {
            output.push_str(&format!("- {error}\n"));
        }
        output.push('\n');
    }

    if !report.runs.is_empty() {
        output.push_str("## 最近任务\n\n");
        for run in &report.runs {
            output.push_str(&format!(
                "- `{}` {} {}\n",
                run.status,
                run.command,
                run.args.join(" ")
            ));
        }
        output.push('\n');
    }

    output.push_str("## 日志\n\n");
    for log in &report.logs {
        output.push_str(&format!(
            "### {}{}\n\n```\n{}\n```\n\n",
            log.source,
            if log.truncated { "（已截断）" } else { "" },
            log.content
        ));
    }
    output
}

fn write_output(content: &str, output: Option<&str>) -> Result<(), String> {
    match output {
        Some(path) => {
            fs::write(path, content).map_err(|error| format!("写入 {path} 失败: {error}"))
        }
        None => {
            print!("{content}");
            Ok(())
        }
    }
}

struct Options {
    target: String,
    format: String,
    output: Option<String>,
    source: Option<String>,
}

fn parse_options(args: &[String], allow_source: bool) -> Result<Options, String> {
    let mut target = None;
    let mut format = "markdown".to_string();
    let mut output = None;
    let mut source = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--format" => {
                index += 1;
                format = args
                    .get(index)
                    .ok_or_else(|| "--format 需要 markdown 或 json".to_string())?
                    .to_lowercase();
                if format != "markdown" && format != "json" {
                    return Err("--format 只能是 markdown 或 json".to_string());
                }
            }
            "--output" | "-o" => {
                index += 1;
                output = Some(
                    args.get(index)
                        .ok_or_else(|| "--output 需要文件路径".to_string())?
                        .to_string(),
                );
            }
            "--source" if allow_source => {
                index += 1;
                source = Some(
                    args.get(index)
                        .ok_or_else(|| "--source 需要 app/php/nginx/nginx-error".to_string())?
                        .to_string(),
                );
            }
            "--help" | "-h" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            value if value.starts_with('-') => {
                return Err(format!("未知参数: {value}\n\n{HELP}"));
            }
            value => {
                if target.is_some() {
                    return Err(format!("只能指定一个项目: {value}"));
                }
                target = Some(value.to_string());
            }
        }
        index += 1;
    }
    Ok(Options {
        target: target.ok_or_else(|| format!("缺少项目参数\n\n{HELP}"))?,
        format,
        output,
        source,
    })
}

fn run_diagnose(args: &[String]) -> Result<(), String> {
    let options = parse_options(args, false)?;
    let report = collect(&options.target)?;
    let content = if options.format == "json" {
        serde_json::to_string_pretty(&report).map_err(|error| format!("生成 JSON 失败: {error}"))?
    } else {
        markdown(&report)
    };
    write_output(&content, options.output.as_deref())
}

fn run_logs(args: &[String]) -> Result<(), String> {
    let options = parse_options(args, true)?;
    let report = collect(&options.target)?;
    let source = options.source.unwrap_or_else(|| "app".to_string());
    let log = report
        .logs
        .iter()
        .find(|log| log.source == source)
        .ok_or_else(|| format!("没有采集到 {source} 日志"))?;
    write_output(&log.content, options.output.as_deref())
}

fn run_containers(args: &[String]) -> Result<(), String> {
    let options = parse_options(args, false)?;
    let report = collect(&options.target)?;
    let mut content = String::from("服务\t状态\t镜像\t详情\n");
    for container in &report.containers {
        content.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            container.service, container.state, container.image, container.status
        ));
    }
    write_output(&content, options.output.as_deref())
}

fn run_export(args: &[String]) -> Result<(), String> {
    let mut args = args.to_vec();
    if !args.iter().any(|arg| arg == "--output" || arg == "-o") {
        let target = args
            .first()
            .cloned()
            .unwrap_or_else(|| "diagnose".to_string());
        args.push("--output".to_string());
        args.push(format!("{target}-diagnose.md"));
    }
    run_diagnose(&args)
}

/// CLI 入口；由 console binary 调用，失败时返回可打印的错误。
pub fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(command) = args.first() else {
        print!("{HELP}");
        return Ok(());
    };
    let rest = &args[1..];
    match command.as_str() {
        "projects" => print!("{}", project_list()?),
        "diagnose" | "error" => run_diagnose(rest)?,
        "logs" => run_logs(rest)?,
        "containers" => run_containers(rest)?,
        "export" => run_export(rest)?,
        "help" | "--help" | "-h" => print!("{HELP}"),
        "--version" | "-V" => println!("phadmin-cli {}", env!("CARGO_PKG_VERSION")),
        _ => return Err(format!("未知命令: {command}\n\n{HELP}")),
    }
    Ok(())
}
