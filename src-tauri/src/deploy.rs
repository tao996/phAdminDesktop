use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::params;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::AppState;
use crate::runner;

static DEPLOY_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployStep {
    pub key: String,
    pub title: String,
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployStart {
    pub deploy_run_id: String,
    pub steps: Vec<DeployStep>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployStepEvent {
    pub deploy_run_id: String,
    pub deployment_id: String,
    pub run_id: String,
    pub step_key: String,
    pub step_title: String,
    pub index: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployFinished {
    pub deploy_run_id: String,
    pub deployment_id: String,
    pub status: String,
    pub error: Option<String>,
}

fn next_deploy_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!(
        "deploy-{}-{}",
        millis,
        DEPLOY_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn build_plan(restart_php: bool, nginx_port: i64) -> Vec<DeployStep> {
    let mut steps = Vec::new();
    steps.push(DeployStep {
        key: "compose-up".to_string(),
        title: "启动 / 更新容器".to_string(),
        program: "docker".to_string(),
        args: vec!["compose".to_string(), "up".to_string(), "-d".to_string()],
    });
    steps.push(DeployStep {
        key: "nginx-reload".to_string(),
        title: "重载 Nginx".to_string(),
        program: "docker".to_string(),
        args: vec![
            "compose".to_string(),
            "exec".to_string(),
            "nginx".to_string(),
            "nginx".to_string(),
            "-s".to_string(),
            "reload".to_string(),
        ],
    });

    if restart_php {
        steps.push(DeployStep {
            key: "restart-php".to_string(),
            title: "重启 PHP".to_string(),
            program: "docker".to_string(),
            args: vec![
                "compose".to_string(),
                "restart".to_string(),
                "php".to_string(),
            ],
        });
    }

    if nginx_port > 0 {
        steps.push(DeployStep {
            key: "health-check".to_string(),
            title: "健康检查".to_string(),
            program: "curl".to_string(),
            args: vec![
                "-fsS".to_string(),
                "--max-time".to_string(),
                "10".to_string(),
                "--retry".to_string(),
                "5".to_string(),
                "--retry-delay".to_string(),
                "2".to_string(),
                "--retry-connrefused".to_string(),
                format!("http://127.0.0.1:{nginx_port}/"),
            ],
        });
    }

    steps
}

fn finish_deploy(
    app: &AppHandle,
    deploy_run_id: &str,
    deployment_id: &str,
    status: &str,
    error: Option<String>,
) {
    let state = app.state::<AppState>();
    if let Ok(connection) = state.db.lock() {
        let _ = connection.execute(
            "UPDATE deploy_runs
             SET status = ?1, ended_at = datetime('now'), error_summary = ?2
             WHERE id = ?3",
            params![status, error.as_deref(), deploy_run_id],
        );
        let deployment_status = match status {
            "succeeded" => "running",
            "canceled" => "stopped",
            _ => "warning",
        };
        let _ = connection.execute(
            "UPDATE deployments
             SET status = ?1, last_deploy = datetime('now'), updated_at = datetime('now')
             WHERE id = ?2",
            params![deployment_status, deployment_id],
        );
    }
    let _ = app.emit(
        "deploy-finished",
        DeployFinished {
            deploy_run_id: deploy_run_id.to_string(),
            deployment_id: deployment_id.to_string(),
            status: status.to_string(),
            error,
        },
    );
}

fn wait_for_run(app: &AppHandle, run_id: &str, timeout: Duration) -> Result<String, String> {
    let started = std::time::Instant::now();
    loop {
        if started.elapsed() > timeout {
            return Err(format!("步骤超时: {run_id}"));
        }

        let state = app.state::<AppState>();
        let status = {
            let connection = state
                .db
                .lock()
                .map_err(|error| format!("数据库连接被占用: {error}"))?;
            connection
                .query_row("SELECT status FROM runs WHERE id = ?1", [run_id], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(|error| format!("读取运行状态失败: {error}"))?
        };

        if status != "running" {
            return Ok(status);
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn execute_plan(
    app: AppHandle,
    deployment_id: String,
    deploy_run_id: String,
    steps: Vec<DeployStep>,
) {
    let total = steps.len();

    for (index, step) in steps.iter().enumerate() {
        let state = app.state::<AppState>();
        let run_id = match runner::run_command(
            app.clone(),
            state,
            deployment_id.clone(),
            step.program.clone(),
            step.args.clone(),
        ) {
            Ok(run_id) => run_id,
            Err(error) => {
                finish_deploy(&app, &deploy_run_id, &deployment_id, "failed", Some(error));
                return;
            }
        };

        if let Ok(connection) = app.state::<AppState>().db.lock() {
            let _ = connection.execute(
                "UPDATE runs
                 SET deploy_run_id = ?1, step_key = ?2, step_title = ?3
                 WHERE id = ?4",
                params![&deploy_run_id, &step.key, &step.title, &run_id],
            );
        }

        let _ = app.emit(
            "deploy-step",
            DeployStepEvent {
                deploy_run_id: deploy_run_id.clone(),
                deployment_id: deployment_id.clone(),
                run_id: run_id.clone(),
                step_key: step.key.clone(),
                step_title: step.title.clone(),
                index,
                total,
            },
        );

        let status = match wait_for_run(&app, &run_id, Duration::from_secs(30 * 60)) {
            Ok(status) => status,
            Err(error) => {
                finish_deploy(&app, &deploy_run_id, &deployment_id, "failed", Some(error));
                return;
            }
        };

        if status != "succeeded" {
            let deploy_status = if status == "canceled" {
                "canceled"
            } else {
                "failed"
            };
            finish_deploy(
                &app,
                &deploy_run_id,
                &deployment_id,
                deploy_status,
                Some(format!("步骤失败: {} ({status})", step.title)),
            );
            return;
        }
    }

    finish_deploy(&app, &deploy_run_id, &deployment_id, "succeeded", None);
}

#[tauri::command]
pub fn preview_deploy_plan(
    state: State<'_, AppState>,
    deployment_id: String,
    restart_php: bool,
) -> Result<Vec<DeployStep>, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let nginx_port: i64 = connection
        .query_row(
            "SELECT d.nginx_port
             FROM deployments d
             WHERE d.id = ?1",
            [&deployment_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("部署不存在: {error}"))?;
    Ok(build_plan(restart_php, nginx_port))
}

/// 共享网络由 compose 以 external 引用，部署前确保它存在。
fn ensure_shared_network(app: &AppHandle, deployment_id: &str) -> Result<(), String> {
    let target = {
        let state = app.state::<AppState>();
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        runner::load_target(&connection, deployment_id)?
    };
    let network = target.docker_network.trim();
    if network.is_empty() {
        return Ok(());
    }

    let (exists, _) = runner::capture_target_command(
        &target,
        "docker",
        &[
            "network".to_string(),
            "inspect".to_string(),
            network.to_string(),
        ],
    )?;
    if exists {
        return Ok(());
    }

    let (created, output) = runner::capture_target_command(
        &target,
        "docker",
        &[
            "network".to_string(),
            "create".to_string(),
            network.to_string(),
        ],
    )?;
    if !created {
        return Err(format!("创建共享网络 {network} 失败: {}", output.trim()));
    }
    Ok(())
}

#[tauri::command]
pub fn start_deploy(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    restart_php: bool,
) -> Result<DeployStart, String> {
    let nginx_port: i64 = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        connection
            .query_row(
                "SELECT d.nginx_port FROM deployments d WHERE d.id = ?1",
                [&deployment_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("部署不存在: {error}"))?
    };

    let steps = build_plan(restart_php, nginx_port);
    let deploy_run_id = next_deploy_id();
    let plan_json =
        serde_json::to_string(&steps).map_err(|error| format!("部署计划编码失败: {error}"))?;

    {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        let running: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM deploy_runs
                 WHERE deployment_id = ?1 AND status = 'running'",
                [&deployment_id],
                |row| row.get(0),
            )
            .map_err(|error| format!("检查部署状态失败: {error}"))?;
        if running > 0 {
            return Err("该项目已有部署正在进行".to_string());
        }
        connection
            .execute(
                "INSERT INTO deploy_runs (id, deployment_id, status, plan_json)
                 VALUES (?1, ?2, 'running', ?3)",
                params![&deploy_run_id, &deployment_id, plan_json],
            )
            .map_err(|error| format!("创建部署记录失败: {error}"))?;
    }

    let app_for_thread = app.clone();
    let deploy_id_for_thread = deploy_run_id.clone();
    let deployment_id_for_thread = deployment_id.clone();
    let steps_for_thread = steps.clone();
    thread::spawn(move || {
        if let Err(error) = ensure_shared_network(&app_for_thread, &deployment_id_for_thread) {
            finish_deploy(
                &app_for_thread,
                &deploy_id_for_thread,
                &deployment_id_for_thread,
                "failed",
                Some(error),
            );
            return;
        }
        execute_plan(
            app_for_thread,
            deployment_id_for_thread,
            deploy_id_for_thread,
            steps_for_thread,
        );
    });

    Ok(DeployStart {
        deploy_run_id,
        steps,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_includes_restart_only_when_requested() {
        let base = build_plan(false, 8071);
        assert_eq!(
            base.iter()
                .map(|step| step.key.as_str())
                .collect::<Vec<_>>(),
            vec!["compose-up", "nginx-reload", "health-check"]
        );
        assert_eq!(
            base[0].args,
            vec!["compose", "up", "-d"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );

        let with_restart = build_plan(true, 8071);
        assert_eq!(with_restart.len(), 4);
        assert_eq!(with_restart[2].key, "restart-php");
        assert_eq!(
            with_restart[2].args,
            vec!["compose", "restart", "php"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );
        assert_eq!(with_restart[3].key, "health-check");
    }

    #[test]
    fn plan_never_calls_the_legacy_cli() {
        let plan = build_plan(true, 8071);
        assert!(plan.iter().all(|step| step.program != "php"));
    }
}
