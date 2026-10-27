use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::AppState;
use crate::db::{self, NewProject, NewSyncItem};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncItemOut {
    pub method: String,
    pub name: String,
    pub path: String,
    pub repo: String,
    pub branch: String,
    pub excludes: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfigOut {
    pub deployment_id: String,
    pub name: String,
    pub mode: String,
    pub server_id: String,
    pub target_path: String,
    pub deploy_dir: String,
    pub app_name: String,
    pub mysql_database: String,
    pub timezone: String,
    pub mysql_user: String,
    pub mysql_password: String,
    pub redis_password: String,
    pub data_path_host: String,
    pub open_port: i64,
    pub domains: String,
    pub ssl: bool,
    pub php_config: String,
    pub sync_items: Vec<SyncItemOut>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncItemInput {
    pub method: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub excludes: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateInput {
    pub name: String,
    /// remote | local
    pub mode: String,
    #[serde(default)]
    pub server_id: String,
    #[serde(default)]
    pub target_path: String,
    #[serde(default)]
    pub deploy_dir: String,
    #[serde(default)]
    pub app_name: String,
    #[serde(default)]
    pub mysql_database: String,
    #[serde(default)]
    pub timezone: String,
    #[serde(default)]
    pub mysql_user: String,
    #[serde(default)]
    pub mysql_password: String,
    #[serde(default)]
    pub redis_password: String,
    #[serde(default)]
    pub data_path_host: String,
    pub open_port: i64,
    #[serde(default)]
    pub domains: String,
    #[serde(default)]
    pub ssl: bool,
    #[serde(default)]
    pub php_config: String,
    #[serde(default)]
    pub sync_items: Vec<SyncItemInput>,
}

fn parse_domains(raw: &str) -> Result<Vec<String>, String> {
    let mut domains = Vec::new();
    for part in raw.split(',') {
        let mut domain = part.trim().to_ascii_lowercase();
        if domain.is_empty() {
            continue;
        }
        domain = domain
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();
        if domain.contains('/') || domain.contains(' ') {
            return Err(format!("域名格式不正确: {domain}"));
        }
        if !domains.contains(&domain) {
            domains.push(domain);
        }
    }
    if domains.is_empty() {
        return Err("请至少填写一个域名".to_string());
    }
    Ok(domains)
}

fn parse_excludes(raw: &str) -> Result<Vec<String>, String> {
    let mut excludes = Vec::new();
    for part in raw.split(',') {
        let mut path = part.trim().replace('\\', "/");
        while path.starts_with('/') {
            path.remove(0);
        }
        let path = path.trim_end_matches('/').to_string();
        if path.is_empty() {
            continue;
        }
        if path.contains("..") {
            return Err("排除目录不能包含 ..".to_string());
        }
        if !excludes.contains(&path) {
            excludes.push(path);
        }
    }
    Ok(excludes)
}

fn validate_sync_items(items: &[SyncItemInput]) -> Result<Vec<NewSyncItem>, String> {
    if items.is_empty() {
        return Err("请至少添加一个代码同步项".to_string());
    }

    let mut result = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let number = index + 1;
        let method = item.method.trim();
        let name = if item.name.trim().is_empty() {
            format!("sync-{index}")
        } else {
            item.name.trim().to_string()
        };
        let mut path = item.path.trim().replace('\\', "/");
        while path.starts_with('/') {
            path.remove(0);
        }
        let path = path.trim_end_matches('/').to_string();
        if path.contains("..") {
            return Err(format!("同步项 {number} 的目录不能包含 .."));
        }
        let branch = if item.branch.trim().is_empty() {
            "main"
        } else {
            item.branch.trim()
        };

        let (repo, branch, path, excludes) = match method {
            "git" => {
                if item.repo.trim().is_empty() {
                    return Err(format!("同步项 {number}（git）需要仓库地址"));
                }
                (
                    item.repo.trim().to_string(),
                    branch.to_string(),
                    path,
                    Vec::new(),
                )
            }
            "bundle" => (String::new(), branch.to_string(), String::new(), Vec::new()),
            "ftp" => {
                if path.is_empty() {
                    return Err(format!("同步项 {number}（ftp）需要填写目录"));
                }
                let excludes = parse_excludes(&item.excludes)
                    .map_err(|error| format!("同步项 {number}: {error}"))?;
                (String::new(), String::new(), path, excludes)
            }
            other => return Err(format!("同步项 {number} 的方式不支持: {other}")),
        };

        result.push(NewSyncItem {
            method: method.to_string(),
            name,
            path,
            repo,
            branch,
            excludes,
        });
    }
    Ok(result)
}

fn create_project_inner(
    connection: &Connection,
    state: &State<'_, AppState>,
    input: &ProjectCreateInput,
) -> Result<String, String> {
    let name = input.name.trim().to_ascii_lowercase();
    if name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }
    if !name
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || character == '-' || character == '_')
    {
        return Err("项目名称只能包含字母、数字、下划线、短横线".to_string());
    }
    if db::project_name_exists(connection, &name)? {
        return Err(format!("项目名称已存在: {name}"));
    }
    if !(1..=65535).contains(&input.open_port) {
        return Err("OPEN_PORT 不合法".to_string());
    }
    if input.mysql_user.trim().is_empty() {
        return Err("MYSQL_USER 不能为空".to_string());
    }
    if input.mysql_password.is_empty() || input.redis_password.is_empty() {
        return Err("MYSQL_PASSWORD / REDIS_PASSWORD 不能为空".to_string());
    }

    let mode = input.mode.trim();
    let deploy_dir = input.deploy_dir.trim();
    let (server_id, remote_path, sync_target) = match mode {
        "remote" => {
            if input.server_id.trim().is_empty() {
                return Err("请选择服务器".to_string());
            }
            let kind = db::server_kind(connection, input.server_id.trim())?
                .ok_or_else(|| "服务器不存在".to_string())?;
            if kind != "ssh" {
                return Err("远程部署请选择 SSH 服务器".to_string());
            }
            let remote_path = if deploy_dir.is_empty() {
                format!("/data/{name}")
            } else {
                deploy_dir.to_string()
            };
            (input.server_id.trim().to_string(), remote_path, "remote")
        }
        "local" => {
            let local_path = if !deploy_dir.is_empty() {
                deploy_dir.to_string()
            } else {
                input.target_path.trim().to_string()
            };
            if local_path.is_empty() {
                return Err("请填写部署目录".to_string());
            }
            let server_id = db::ensure_local_server(connection)?;
            (server_id, local_path, "filesystem")
        }
        other => return Err(format!("未知部署模式: {other}")),
    };

    if db::port_taken(connection, &server_id, input.open_port)? {
        return Err(format!(
            "端口已被同目标上的其它项目占用: {}",
            input.open_port
        ));
    }

    let domains = parse_domains(&input.domains)?;
    let sync_items = validate_sync_items(&input.sync_items)?;
    let timezone = if input.timezone.trim().is_empty() {
        "Asia/Shanghai"
    } else {
        input.timezone.trim()
    };
    let data_path_host = if input.data_path_host.trim().is_empty() {
        "./docker/storage"
    } else {
        input.data_path_host.trim()
    };
    let app_name = if input.app_name.trim().is_empty() {
        name.clone()
    } else {
        input.app_name.trim().to_string()
    };
    let mysql_database = if input.mysql_database.trim().is_empty() {
        format!("{app_name}_db")
    } else {
        input.mysql_database.trim().to_string()
    };

    let deployment_id = db::create_project_records(
        connection,
        &NewProject {
            name: name.clone(),
            server_id: server_id.clone(),
            remote_path,
            domains,
            nginx_port: input.open_port,
            ssl: input.ssl,
            sync_target: sync_target.to_string(),
            config_php: input.php_config.trim().to_string(),
            sync_items,
        },
    )?;

    let env_pairs = [
        ("TZ", timezone.to_string()),
        ("APP_NAME", app_name.clone()),
        ("MYSQL_DATABASE", mysql_database.clone()),
        ("MYSQL_USER", input.mysql_user.trim().to_string()),
        ("MYSQL_PASSWORD", input.mysql_password.clone()),
        ("REDIS_PASSWORD", input.redis_password.clone()),
        ("DATA_PATH_HOST", data_path_host.to_string()),
    ];

    for (key, value) in env_pairs {
        if let Err(error) = crate::env_vars::set(
            connection,
            state.secrets.as_ref(),
            &deployment_id,
            key,
            &value,
        ) {
            let _ = db::delete_project_records(connection, &deployment_id, &name);
            return Err(error);
        }
    }

    Ok(deployment_id)
}

#[tauri::command]
pub fn create_project(
    state: State<'_, AppState>,
    input: ProjectCreateInput,
) -> Result<String, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    create_project_inner(&connection, &state, &input)
}

fn env_or(
    connection: &Connection,
    state: &State<'_, AppState>,
    deployment_id: &str,
    key: &str,
    fallback: &str,
) -> String {
    crate::env_vars::resolve(connection, state.secrets.as_ref(), deployment_id, key)
        .ok()
        .flatten()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

#[tauri::command]
pub fn get_project_config(
    state: State<'_, AppState>,
    deployment_id: String,
) -> Result<ProjectConfigOut, String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let row = db::load_project_config_row(&connection, &deployment_id)?;
    let mode = if row.server_kind == "filesystem" {
        "local"
    } else {
        "remote"
    };

    Ok(ProjectConfigOut {
        name: row.name.clone(),
        mode: mode.to_string(),
        server_id: row.server_id.clone(),
        target_path: if mode == "local" {
            row.remote_path.clone()
        } else {
            String::new()
        },
        deploy_dir: row.remote_path.clone(),
        app_name: env_or(&connection, &state, &deployment_id, "APP_NAME", &row.name),
        mysql_database: env_or(
            &connection,
            &state,
            &deployment_id,
            "MYSQL_DATABASE",
            &format!("{}_db", row.name),
        ),
        timezone: env_or(&connection, &state, &deployment_id, "TZ", "Asia/Shanghai"),
        mysql_user: env_or(&connection, &state, &deployment_id, "MYSQL_USER", "admin"),
        mysql_password: env_or(&connection, &state, &deployment_id, "MYSQL_PASSWORD", ""),
        redis_password: env_or(&connection, &state, &deployment_id, "REDIS_PASSWORD", ""),
        data_path_host: env_or(
            &connection,
            &state,
            &deployment_id,
            "DATA_PATH_HOST",
            "./docker/storage",
        ),
        open_port: row.nginx_port,
        domains: row.domains.join(","),
        ssl: row.ssl,
        php_config: row.config_php.clone(),
        sync_items: row
            .sync_items
            .iter()
            .map(
                |(method, name, path, repo, branch, excludes_json)| SyncItemOut {
                    method: method.clone(),
                    name: name.clone(),
                    path: path.clone(),
                    repo: repo.clone(),
                    branch: branch.clone(),
                    excludes: serde_json::from_str::<Vec<String>>(excludes_json)
                        .unwrap_or_default()
                        .join(","),
                },
            )
            .collect(),
        deployment_id: deployment_id.clone(),
    })
}

#[tauri::command]
pub fn update_project(
    state: State<'_, AppState>,
    deployment_id: String,
    input: ProjectCreateInput,
) -> Result<(), String> {
    let connection = state
        .db
        .lock()
        .map_err(|error| format!("数据库连接被占用: {error}"))?;
    let row = db::load_project_config_row(&connection, &deployment_id)?;

    if !(1..=65535).contains(&input.open_port) {
        return Err("OPEN_PORT 不合法".to_string());
    }
    if input.mysql_user.trim().is_empty() {
        return Err("MYSQL_USER 不能为空".to_string());
    }
    if input.mysql_password.is_empty() || input.redis_password.is_empty() {
        return Err("MYSQL_PASSWORD / REDIS_PASSWORD 不能为空".to_string());
    }

    let deploy_dir = input.deploy_dir.trim();
    let app_name = if input.app_name.trim().is_empty() {
        row.name.clone()
    } else {
        input.app_name.trim().to_string()
    };
    let mysql_database = if input.mysql_database.trim().is_empty() {
        format!("{app_name}_db")
    } else {
        input.mysql_database.trim().to_string()
    };

    let (server_id, remote_path, sync_target) = match input.mode.trim() {
        "remote" => {
            if input.server_id.trim().is_empty() {
                return Err("请选择服务器".to_string());
            }
            let kind = db::server_kind(&connection, input.server_id.trim())?
                .ok_or_else(|| "服务器不存在".to_string())?;
            if kind != "ssh" {
                return Err("远程部署请选择 SSH 服务器".to_string());
            }
            let remote_path = if deploy_dir.is_empty() {
                format!("/data/{}", row.name)
            } else {
                deploy_dir.to_string()
            };
            (input.server_id.trim().to_string(), remote_path, "remote")
        }
        "local" => {
            let local_path = if !deploy_dir.is_empty() {
                deploy_dir.to_string()
            } else {
                input.target_path.trim().to_string()
            };
            if local_path.is_empty() {
                return Err("请填写部署目录".to_string());
            }
            let server_id = db::ensure_local_server(&connection)?;
            (server_id, local_path, "filesystem")
        }
        other => return Err(format!("未知部署模式: {other}")),
    };

    if db::port_taken_except(&connection, &server_id, input.open_port, &deployment_id)? {
        return Err(format!(
            "端口已被同目标上的其它项目占用: {}",
            input.open_port
        ));
    }

    let domains = parse_domains(&input.domains)?;
    let sync_items = validate_sync_items(&input.sync_items)?;
    let timezone = if input.timezone.trim().is_empty() {
        "Asia/Shanghai"
    } else {
        input.timezone.trim()
    };
    let data_path_host = if input.data_path_host.trim().is_empty() {
        "./docker/storage"
    } else {
        input.data_path_host.trim()
    };

    db::update_project_records(
        &connection,
        &deployment_id,
        &NewProject {
            name: row.name.clone(),
            server_id,
            remote_path,
            domains,
            nginx_port: input.open_port,
            ssl: input.ssl,
            sync_target: sync_target.to_string(),
            config_php: input.php_config.trim().to_string(),
            sync_items,
        },
    )?;

    let env_pairs = [
        ("TZ", timezone.to_string()),
        ("APP_NAME", app_name.clone()),
        ("MYSQL_DATABASE", mysql_database.clone()),
        ("MYSQL_USER", input.mysql_user.trim().to_string()),
        ("MYSQL_PASSWORD", input.mysql_password.clone()),
        ("REDIS_PASSWORD", input.redis_password.clone()),
        ("DATA_PATH_HOST", data_path_host.to_string()),
    ];
    for (key, value) in env_pairs {
        crate::env_vars::set(
            &connection,
            state.secrets.as_ref(),
            &deployment_id,
            key,
            &value,
        )?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_normalizes_domains() {
        assert_eq!(
            parse_domains("https://A.com/, b.com ,a.com").expect("domains"),
            vec!["a.com".to_string(), "b.com".to_string()]
        );
        assert!(parse_domains(" ").is_err());
        assert!(parse_domains("a.com/path").is_err());
    }

    #[test]
    fn validates_sync_items() {
        let items = vec![
            SyncItemInput {
                method: "git".to_string(),
                name: "main".to_string(),
                path: String::new(),
                repo: "https://example.com/app.git".to_string(),
                branch: String::new(),
                excludes: String::new(),
            },
            SyncItemInput {
                method: "ftp".to_string(),
                name: "assets".to_string(),
                path: "/src/App/Projects/demo/".to_string(),
                repo: String::new(),
                branch: String::new(),
                excludes: "vendor, cache".to_string(),
            },
        ];
        let validated = validate_sync_items(&items).expect("sync items");
        assert_eq!(validated[0].branch, "main");
        assert_eq!(validated[1].path, "src/App/Projects/demo");
        assert_eq!(validated[1].excludes, vec!["vendor", "cache"]);
        assert_eq!(validated[1].name, "assets");

        assert!(
            validate_sync_items(&[SyncItemInput {
                method: "git".to_string(),
                name: String::new(),
                path: String::new(),
                repo: String::new(),
                branch: "main".to_string(),
                excludes: String::new(),
            }])
            .is_err()
        );
        assert!(
            validate_sync_items(&[SyncItemInput {
                method: "ftp".to_string(),
                name: String::new(),
                path: String::new(),
                repo: String::new(),
                branch: String::new(),
                excludes: String::new(),
            }])
            .is_err()
        );
        assert!(
            validate_sync_items(&[SyncItemInput {
                method: "ftp".to_string(),
                name: String::new(),
                path: "../etc".to_string(),
                repo: String::new(),
                branch: String::new(),
                excludes: String::new(),
            }])
            .is_err()
        );
        assert!(validate_sync_items(&[]).is_err());
    }
}
