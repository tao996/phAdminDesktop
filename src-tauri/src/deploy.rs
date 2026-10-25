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

fn build_plan(project_name: &str, restart_php: bool) -> Vec<DeployStep> {
    let mut steps = vec![
        DeployStep {
            key: "upgrade".to_string(),
            title: "同步代码 / 生成配置".to_string(),
            program: "php".to_string(),
            args: vec![
                "admin".to_string(),
                format!("app:{project_name}"),
                "upgrade".to_string(),
            ],
        },
        DeployStep {
            key: "compose-up".to_string(),
            title: "启动 / 更新容器".to_string(),
            program: "docker".to_string(),
            args: vec!["compose".to_string(), "up".to_string(), "-d".to_string()],
        },
        DeployStep {
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
        },
    ];

    if restart_php {
        steps.push(DeployStep {
            key: "restart-php".to_string(),
            title: "重启 PHP".to_string(),
            program: "php".to_string(),
            args: vec![
                "admin".to_string(),
                format!("app:{project_name}"),
                "dc:restart:php".to_string(),
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
    let project_name = connection
        .query_row(
            "SELECT p.name
             FROM deployments d
             JOIN projects p ON p.id = d.project_id
             WHERE d.id = ?1",
            [&deployment_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("部署不存在: {error}"))?;
    Ok(build_plan(&project_name, restart_php))
}

#[tauri::command]
pub fn start_deploy(
    app: AppHandle,
    state: State<'_, AppState>,
    deployment_id: String,
    restart_php: bool,
) -> Result<DeployStart, String> {
    let project_name = {
        let connection = state
            .db
            .lock()
            .map_err(|error| format!("数据库连接被占用: {error}"))?;
        connection
            .query_row(
                "SELECT p.name
                 FROM deployments d
                 JOIN projects p ON p.id = d.project_id
                 WHERE d.id = ?1",
                [&deployment_id],
                |row| row.get::<_, String>(0),
            )
            .map_err(|error| format!("部署不存在: {error}"))?
    };

    let steps = build_plan(&project_name, restart_php);
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
        let base = build_plan("demo", false);
        assert_eq!(
            base.iter()
                .map(|step| step.key.as_str())
                .collect::<Vec<_>>(),
            vec!["upgrade", "compose-up", "nginx-reload"]
        );
        assert_eq!(
            base[0].args,
            vec!["admin", "app:demo", "upgrade"]
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>()
        );

        let with_restart = build_plan("demo", true);
        assert_eq!(with_restart.len(), 4);
        assert_eq!(with_restart[3].key, "restart-php");
    }
}
