use std::error::Error;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::{
    DeployRunRecord, Deployment, DeploymentConfig, DeploymentSettings, Project, RunRecord, Server,
    ServerConfig, ServerInput, SyncItem, Workspace,
};

#[cfg(test)]
const SEED_JSON: &str = include_str!("../seed.json");

pub fn open(path: &Path) -> Result<Connection, Box<dyn Error>> {
    let connection = Connection::open(path)?;
    // DELETE 日志模式：数据库保持单文件；写入瞬间会出现 -journal，提交后自动删除。
    // 桌面端是单进程访问，不需要 WAL 的并发读能力。
    connection.execute_batch(
        "PRAGMA journal_mode = DELETE;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    migrate(&connection)?;
    // 不再自动灌入演示数据：工作区从空开始，由用户在 UI 中创建。
    Ok(connection)
}

pub(crate) fn migrate(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS templates (
            path        TEXT PRIMARY KEY,
            content     TEXT NOT NULL,
            kind        TEXT NOT NULL DEFAULT 'override'
                        CHECK (kind IN ('override', 'custom')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS servers (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            host        TEXT NOT NULL,
            environment TEXT NOT NULL,
            status      TEXT NOT NULL,
            latency     TEXT NOT NULL,
            kind        TEXT NOT NULL DEFAULT 'ssh',
            port        INTEGER NOT NULL DEFAULT 22,
            username    TEXT NOT NULL DEFAULT '',
            key_path    TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS projects (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            title       TEXT NOT NULL,
            repo_url    TEXT NOT NULL DEFAULT '',
            local_path  TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS deployments (
            id          TEXT PRIMARY KEY,
            server_id   TEXT NOT NULL REFERENCES servers(id) ON DELETE RESTRICT,
            project_id  TEXT NOT NULL REFERENCES projects(id) ON DELETE RESTRICT,
            status      TEXT NOT NULL DEFAULT 'unknown',
            remote_path TEXT NOT NULL,
            domain      TEXT NOT NULL DEFAULT '',
            nginx_port  INTEGER NOT NULL DEFAULT 8071,
            branch      TEXT NOT NULL DEFAULT 'main',
            ssl         INTEGER NOT NULL DEFAULT 0 CHECK (ssl IN (0, 1)),
            last_deploy TEXT NOT NULL DEFAULT '从未',
            php_version TEXT NOT NULL DEFAULT 'PHP 8.5.12',
            backups     INTEGER NOT NULL DEFAULT 0,
            sync_target TEXT NOT NULL DEFAULT 'remote'
                        CHECK (sync_target IN ('remote', 'filesystem')),
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (server_id, project_id)
        );

        CREATE INDEX IF NOT EXISTS idx_deployments_server
            ON deployments(server_id);
        CREATE INDEX IF NOT EXISTS idx_deployments_project
            ON deployments(project_id);

        CREATE TABLE IF NOT EXISTS domains (
            id            TEXT PRIMARY KEY,
            deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
            domain        TEXT NOT NULL,
            sort          INTEGER NOT NULL DEFAULT 0,
            UNIQUE (deployment_id, domain)
        );

        CREATE TABLE IF NOT EXISTS env_vars (
            id            TEXT PRIMARY KEY,
            deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
            key           TEXT NOT NULL,
            value         TEXT,
            is_secret     INTEGER NOT NULL DEFAULT 0 CHECK (is_secret IN (0, 1)),
            secret_ref    TEXT,
            UNIQUE (deployment_id, key)
        );

        CREATE TABLE IF NOT EXISTS sync_items (
            id            TEXT PRIMARY KEY,
            deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
            sort          INTEGER NOT NULL DEFAULT 0,
            method        TEXT NOT NULL,
            name          TEXT NOT NULL DEFAULT '',
            path          TEXT NOT NULL DEFAULT '',
            repo          TEXT NOT NULL DEFAULT '',
            branch        TEXT NOT NULL DEFAULT 'main',
            excludes_json TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS deployment_settings (
            deployment_id      TEXT PRIMARY KEY REFERENCES deployments(id) ON DELETE CASCADE,
            config_json        TEXT NOT NULL DEFAULT '{}',
            hooks_json         TEXT NOT NULL DEFAULT '{}',
            docker_images_json TEXT NOT NULL DEFAULT '{}',
            router_json        TEXT NOT NULL DEFAULT '{}',
            nginx_json         TEXT NOT NULL DEFAULT '{}'
        );

        CREATE TABLE IF NOT EXISTS runs (
            id            TEXT PRIMARY KEY,
            deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
            command       TEXT NOT NULL,
            args_json     TEXT NOT NULL DEFAULT '[]',
            status        TEXT NOT NULL CHECK (status IN
                          ('queued', 'running', 'succeeded', 'failed', 'canceled')),
            exit_code     INTEGER,
            started_at    TEXT,
            ended_at      TEXT,
            log_path      TEXT,
            error_summary TEXT,
            deploy_run_id TEXT,
            step_key      TEXT,
            step_title    TEXT
        );

        CREATE TABLE IF NOT EXISTS deploy_runs (
            id            TEXT PRIMARY KEY,
            deployment_id TEXT NOT NULL REFERENCES deployments(id) ON DELETE CASCADE,
            status        TEXT NOT NULL CHECK (status IN
                          ('running', 'succeeded', 'failed', 'canceled')),
            plan_json     TEXT NOT NULL DEFAULT '[]',
            started_at    TEXT NOT NULL DEFAULT (datetime('now')),
            ended_at      TEXT,
            error_summary TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_sync_items_deployment
            ON sync_items(deployment_id, sort);
        CREATE INDEX IF NOT EXISTS idx_runs_deployment_started
            ON runs(deployment_id, started_at DESC);
        CREATE INDEX IF NOT EXISTS idx_deploy_runs_deployment
            ON deploy_runs(deployment_id, started_at DESC);",
    )?;
    ensure_column(
        connection,
        "servers",
        "key_path",
        "key_path TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(connection, "runs", "deploy_run_id", "deploy_run_id TEXT")?;
    ensure_column(connection, "runs", "step_key", "step_key TEXT")?;
    ensure_column(connection, "runs", "step_title", "step_title TEXT")?;
    ensure_column(
        connection,
        "servers",
        "password",
        "password TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "servers",
        "passphrase",
        "passphrase TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "servers",
        "docker_network",
        "docker_network TEXT NOT NULL DEFAULT 'phalcon-shared'",
    )?;
    ensure_column(
        connection,
        "deployment_settings",
        "config_php",
        "config_php TEXT NOT NULL DEFAULT ''",
    )?;
    connection.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_runs_deploy_run
            ON runs(deploy_run_id, started_at);",
    )?;
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> rusqlite::Result<()> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    connection.execute(&format!("ALTER TABLE {table} ADD COLUMN {definition}"), [])?;
    Ok(())
}

fn slug(value: &str) -> String {
    let slug: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "server".to_string()
    } else {
        slug
    }
}

struct ValidatedServer {
    name: String,
    host: String,
    user: String,
    port: i64,
    password: String,
    key_file: String,
    passphrase: String,
    docker_network: String,
}

fn validate_server(input: &ServerInput) -> Result<ValidatedServer, String> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err("服务器名称不能为空".to_string());
    }
    if name.len() > 64 || name.contains('\n') || name.contains('\r') {
        return Err("服务器名称不合法（最长 64 个字符）".to_string());
    }
    let host = input.host.trim();
    if host.is_empty() {
        return Err("SSH 主机不能为空".to_string());
    }
    let user = input.user.trim();
    if user.is_empty() {
        return Err("SSH 用户不能为空".to_string());
    }
    if !(1..=65535).contains(&input.port) {
        return Err(format!("SSH 端口不合法: {}", input.port));
    }
    if input.password.trim().is_empty() && input.key_file.trim().is_empty() {
        return Err("请填写密码或私钥路径（二选一）".to_string());
    }
    if !input.password.trim().is_empty() && !input.key_file.trim().is_empty() {
        return Err("密码认证与密钥认证只能选一种".to_string());
    }

    Ok(ValidatedServer {
        name: name.to_string(),
        host: host.to_string(),
        user: user.to_string(),
        port: input.port,
        password: input.password.clone(),
        key_file: input.key_file.trim().to_string(),
        passphrase: input.key_passphrase.clone(),
        docker_network: if input.docker_network.trim().is_empty() {
            "phalcon-shared".to_string()
        } else {
            input.docker_network.trim().to_string()
        },
    })
}

pub fn create_server(connection: &Connection, input: &ServerInput) -> Result<Server, String> {
    let validated = validate_server(input)?;
    let name = validated.name.clone();

    let existing: Option<String> = connection
        .query_row("SELECT id FROM servers WHERE name = ?1", [&name], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| format!("读取服务器失败: {error}"))?;
    if existing.is_some() {
        return Err(format!("服务器已存在: {name}"));
    }

    let base = slug(&name);
    let mut id = base.clone();
    let mut index = 2;
    loop {
        let taken: Option<i64> = connection
            .query_row("SELECT 1 FROM servers WHERE id = ?1", [&id], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|error| format!("读取服务器失败: {error}"))?;
        if taken.is_none() {
            break;
        }
        id = format!("{base}-{index}");
        index += 1;
    }

    connection
        .execute(
            "INSERT INTO servers
                (id, name, host, environment, status, latency, kind, port, username, key_path,
                 password, passphrase, docker_network)
             VALUES (?1, ?2, ?3, 'production', 'checking', '—', 'ssh', ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &id,
                &name,
                &validated.host,
                validated.port,
                &validated.user,
                &validated.key_file,
                &validated.password,
                &validated.passphrase,
                &validated.docker_network
            ],
        )
        .map_err(|error| format!("创建服务器失败: {error}"))?;

    Ok(Server {
        id,
        name,
        host: validated.host,
        environment: "production".to_string(),
        status: "checking".to_string(),
        latency: "—".to_string(),
        kind: "ssh".to_string(),
        port: validated.port,
        username: validated.user,
    })
}

pub fn load_server_config(
    connection: &Connection,
    server_id: &str,
) -> Result<ServerConfig, String> {
    connection
        .query_row(
            "SELECT name, host, port, username, password, key_path, passphrase, docker_network
             FROM servers WHERE id = ?1",
            [server_id],
            |row| {
                Ok(ServerConfig {
                    name: row.get(0)?,
                    host: row.get(1)?,
                    port: row.get(2)?,
                    user: row.get(3)?,
                    password: row.get(4)?,
                    key_file: row.get(5)?,
                    key_passphrase: row.get(6)?,
                    docker_network: row.get(7)?,
                })
            },
        )
        .map_err(|error| format!("读取服务器失败: {error}"))
}

pub fn update_server(
    connection: &Connection,
    server_id: &str,
    input: &ServerInput,
) -> Result<Server, String> {
    let validated = validate_server(input)?;
    let name = validated.name.clone();

    let conflict: Option<String> = connection
        .query_row(
            "SELECT id FROM servers WHERE name = ?1 AND id <> ?2",
            params![&name, server_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取服务器失败: {error}"))?;
    if conflict.is_some() {
        return Err(format!("服务器已存在: {name}"));
    }

    let changed = connection
        .execute(
            "UPDATE servers SET
                name = ?2,
                host = ?3,
                port = ?4,
                username = ?5,
                key_path = ?6,
                password = ?7,
                passphrase = ?8,
                docker_network = ?9,
                updated_at = datetime('now')
             WHERE id = ?1",
            params![
                server_id,
                &name,
                &validated.host,
                validated.port,
                &validated.user,
                &validated.key_file,
                &validated.password,
                &validated.passphrase,
                &validated.docker_network
            ],
        )
        .map_err(|error| format!("更新服务器失败: {error}"))?;
    if changed == 0 {
        return Err("服务器不存在".to_string());
    }

    Ok(Server {
        id: server_id.to_string(),
        name,
        host: validated.host,
        environment: "production".to_string(),
        status: "checking".to_string(),
        latency: "—".to_string(),
        kind: "ssh".to_string(),
        port: validated.port,
        username: validated.user,
    })
}

pub fn count_server_references(connection: &Connection, server_id: &str) -> Result<i64, String> {
    connection
        .query_row(
            "SELECT COUNT(*) FROM deployments WHERE server_id = ?1",
            [server_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取服务器引用失败: {error}"))
}

pub fn delete_server(connection: &Connection, server_id: &str) -> Result<(), String> {
    let references = count_server_references(connection, server_id)?;
    if references > 0 {
        return Err(format!("该服务器仍被 {references} 个部署引用，不能删除"));
    }

    let changed = connection
        .execute("DELETE FROM servers WHERE id = ?1", [server_id])
        .map_err(|error| format!("删除服务器失败: {error}"))?;
    if changed == 0 {
        return Err("服务器不存在".to_string());
    }
    Ok(())
}

pub fn list_template_rows(
    connection: &Connection,
) -> Result<Vec<(String, String, String)>, String> {
    let mut statement = connection
        .prepare("SELECT path, kind, updated_at FROM templates")
        .map_err(|error| format!("读取模板失败: {error}"))?;
    let rows = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|error| format!("读取模板失败: {error}"))?;
    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|error| format!("读取模板失败: {error}"))?);
    }
    Ok(result)
}

pub fn get_template_row(
    connection: &Connection,
    path: &str,
) -> Result<Option<(String, String)>, String> {
    connection
        .query_row(
            "SELECT content, updated_at FROM templates WHERE path = ?1",
            [path],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| format!("读取模板失败: {error}"))
}

pub fn upsert_template(
    connection: &Connection,
    path: &str,
    content: &str,
    kind: &str,
) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO templates (path, content, kind, updated_at)
             VALUES (?1, ?2, ?3, datetime('now'))
             ON CONFLICT(path) DO UPDATE SET
                content = excluded.content,
                kind = excluded.kind,
                updated_at = datetime('now')",
            params![path, content, kind],
        )
        .map_err(|error| format!("保存模板失败: {error}"))?;
    Ok(())
}

pub fn delete_template(connection: &Connection, path: &str) -> Result<(), String> {
    connection
        .execute("DELETE FROM templates WHERE path = ?1", [path])
        .map_err(|error| format!("删除模板失败: {error}"))?;
    Ok(())
}

pub struct NewSyncItem {
    pub method: String,
    pub name: String,
    pub path: String,
    pub repo: String,
    pub branch: String,
    pub excludes: Vec<String>,
}

pub struct NewProject {
    pub name: String,
    pub server_id: String,
    pub remote_path: String,
    pub domains: Vec<String>,
    pub nginx_port: i64,
    pub ssl: bool,
    pub sync_target: String,
    pub config_php: String,
    pub sync_items: Vec<NewSyncItem>,
}

pub fn project_name_exists(connection: &Connection, name: &str) -> Result<bool, String> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM projects WHERE name = ?1",
            [name],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取项目失败: {error}"))?;
    Ok(count > 0)
}

pub fn port_taken(connection: &Connection, server_id: &str, port: i64) -> Result<bool, String> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM deployments WHERE server_id = ?1 AND nginx_port = ?2",
            params![server_id, port],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取端口占用失败: {error}"))?;
    Ok(count > 0)
}

pub fn server_kind(connection: &Connection, server_id: &str) -> Result<Option<String>, String> {
    connection
        .query_row(
            "SELECT kind FROM servers WHERE id = ?1",
            [server_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取服务器失败: {error}"))
}

pub fn ensure_local_server(connection: &Connection) -> Result<String, String> {
    let existing: Option<String> = connection
        .query_row(
            "SELECT id FROM servers WHERE kind = 'filesystem' ORDER BY id LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| format!("读取本地服务器失败: {error}"))?;
    if let Some(id) = existing {
        return Ok(id);
    }

    connection
        .execute(
            "INSERT INTO servers
                (id, name, host, environment, status, latency, kind, port, username, key_path,
                 password, passphrase, docker_network)
             VALUES ('local', '本地', '', 'local', 'connected', '—', 'filesystem', 22, '', '',
                     '', '', 'phalcon-shared')",
            [],
        )
        .map_err(|error| format!("创建本地服务器失败: {error}"))?;
    Ok("local".to_string())
}

pub fn load_config_php(connection: &Connection, deployment_id: &str) -> Result<String, String> {
    connection
        .query_row(
            "SELECT COALESCE((SELECT config_php FROM deployment_settings WHERE deployment_id = ?1), '')",
            [deployment_id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|error| format!("读取 PHP 配置失败: {error}"))
}

pub fn create_project_records(
    connection: &Connection,
    project: &NewProject,
) -> Result<String, String> {
    let deployment_id = format!("{}-{}", project.server_id, project.name);
    let first_domain = project.domains.first().cloned().unwrap_or_default();

    connection
        .execute(
            "INSERT INTO projects (id, name, title, repo_url, local_path)
             VALUES (?1, ?1, ?1, '', '')",
            [&project.name],
        )
        .map_err(|error| format!("创建项目失败: {error}"))?;

    connection
        .execute(
            "INSERT INTO deployments
                (id, server_id, project_id, status, remote_path, domain, nginx_port,
                 branch, ssl, sync_target)
             VALUES (?1, ?2, ?3, 'not-deployed', ?4, ?5, ?6, 'main', ?7, ?8)",
            params![
                &deployment_id,
                &project.server_id,
                &project.name,
                &project.remote_path,
                first_domain,
                project.nginx_port,
                if project.ssl { 1 } else { 0 },
                &project.sync_target
            ],
        )
        .map_err(|error| format!("创建部署失败: {error}"))?;

    for (index, domain) in project.domains.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO domains (id, deployment_id, domain, sort)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    format!("{deployment_id}-domain-{index}"),
                    &deployment_id,
                    domain,
                    index as i64
                ],
            )
            .map_err(|error| format!("保存域名失败: {error}"))?;
    }

    for (index, item) in project.sync_items.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO sync_items
                    (id, deployment_id, sort, method, name, path, repo, branch, excludes_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    format!("{deployment_id}-sync-{index}"),
                    &deployment_id,
                    index as i64,
                    &item.method,
                    &item.name,
                    &item.path,
                    &item.repo,
                    &item.branch,
                    serde_json::to_string(&item.excludes)
                        .map_err(|error| format!("编码排除目录失败: {error}"))?
                ],
            )
            .map_err(|error| format!("保存同步项失败: {error}"))?;
    }

    connection
        .execute(
            "INSERT INTO deployment_settings (deployment_id, config_json, config_php)
             VALUES (?1, '{}', ?2)
             ON CONFLICT(deployment_id) DO UPDATE SET config_php = excluded.config_php",
            params![&deployment_id, &project.config_php],
        )
        .map_err(|error| format!("保存项目配置失败: {error}"))?;

    Ok(deployment_id)
}

pub fn delete_project_records(
    connection: &Connection,
    deployment_id: &str,
    project_name: &str,
) -> Result<(), String> {
    connection
        .execute("DELETE FROM deployments WHERE id = ?1", [deployment_id])
        .map_err(|error| format!("清理部署失败: {error}"))?;
    connection
        .execute("DELETE FROM projects WHERE id = ?1", [project_name])
        .map_err(|error| format!("清理项目失败: {error}"))?;
    Ok(())
}

pub struct ProjectConfigRow {
    pub name: String,
    pub server_id: String,
    pub server_kind: String,
    pub remote_path: String,
    pub nginx_port: i64,
    pub ssl: bool,
    pub domains: Vec<String>,
    pub config_php: String,
    /// (method, name, path, repo, branch, excludes_json)
    pub sync_items: Vec<(String, String, String, String, String, String)>,
}

pub fn load_project_config_row(
    connection: &Connection,
    deployment_id: &str,
) -> Result<ProjectConfigRow, String> {
    let (name, server_id, server_kind, remote_path, nginx_port, ssl): (
        String,
        String,
        String,
        String,
        i64,
        i64,
    ) = connection
        .query_row(
            "SELECT p.name, s.id, s.kind, d.remote_path, d.nginx_port, d.ssl
             FROM deployments d
             JOIN projects p ON p.id = d.project_id
             JOIN servers s ON s.id = d.server_id
             WHERE d.id = ?1",
            [deployment_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
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
        domains.push(row.map_err(|error| format!("读取域名失败: {error}"))?);
    }

    let mut sync_items = Vec::new();
    let mut statement = connection
        .prepare(
            "SELECT method, name, path, repo, branch, excludes_json FROM sync_items
             WHERE deployment_id = ?1 ORDER BY sort",
        )
        .map_err(|error| format!("读取同步项失败: {error}"))?;
    let rows = statement
        .query_map([deployment_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| format!("读取同步项失败: {error}"))?;
    for row in rows {
        sync_items.push(row.map_err(|error| format!("读取同步项失败: {error}"))?);
    }

    let config_php = load_config_php(connection, deployment_id)?;

    Ok(ProjectConfigRow {
        name,
        server_id,
        server_kind,
        remote_path,
        nginx_port,
        ssl: ssl != 0,
        domains,
        config_php,
        sync_items,
    })
}

pub fn port_taken_except(
    connection: &Connection,
    server_id: &str,
    port: i64,
    deployment_id: &str,
) -> Result<bool, String> {
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM deployments
             WHERE server_id = ?1 AND nginx_port = ?2 AND id <> ?3",
            params![server_id, port, deployment_id],
            |row| row.get(0),
        )
        .map_err(|error| format!("读取端口占用失败: {error}"))?;
    Ok(count > 0)
}

pub fn update_project_records(
    connection: &Connection,
    deployment_id: &str,
    project: &NewProject,
) -> Result<(), String> {
    let first_domain = project.domains.first().cloned().unwrap_or_default();
    let changed = connection
        .execute(
            "UPDATE deployments SET
                server_id = ?2,
                remote_path = ?3,
                domain = ?4,
                nginx_port = ?5,
                ssl = ?6,
                sync_target = ?7,
                updated_at = datetime('now')
             WHERE id = ?1",
            params![
                deployment_id,
                &project.server_id,
                &project.remote_path,
                first_domain,
                project.nginx_port,
                if project.ssl { 1 } else { 0 },
                &project.sync_target
            ],
        )
        .map_err(|error| format!("更新部署失败: {error}"))?;
    if changed == 0 {
        return Err("部署不存在".to_string());
    }

    connection
        .execute(
            "DELETE FROM domains WHERE deployment_id = ?1",
            [deployment_id],
        )
        .map_err(|error| format!("清理域名失败: {error}"))?;
    connection
        .execute(
            "DELETE FROM sync_items WHERE deployment_id = ?1",
            [deployment_id],
        )
        .map_err(|error| format!("清理同步项失败: {error}"))?;

    for (index, domain) in project.domains.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO domains (id, deployment_id, domain, sort)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    format!("{deployment_id}-domain-{index}"),
                    deployment_id,
                    domain,
                    index as i64
                ],
            )
            .map_err(|error| format!("保存域名失败: {error}"))?;
    }

    for (index, item) in project.sync_items.iter().enumerate() {
        connection
            .execute(
                "INSERT INTO sync_items
                    (id, deployment_id, sort, method, name, path, repo, branch, excludes_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    format!("{deployment_id}-sync-{index}"),
                    deployment_id,
                    index as i64,
                    &item.method,
                    &item.name,
                    &item.path,
                    &item.repo,
                    &item.branch,
                    serde_json::to_string(&item.excludes)
                        .map_err(|error| format!("编码排除目录失败: {error}"))?
                ],
            )
            .map_err(|error| format!("保存同步项失败: {error}"))?;
    }

    connection
        .execute(
            "INSERT INTO deployment_settings (deployment_id, config_json, config_php)
             VALUES (?1, '{}', ?2)
             ON CONFLICT(deployment_id) DO UPDATE SET config_php = excluded.config_php",
            params![deployment_id, &project.config_php],
        )
        .map_err(|error| format!("保存项目配置失败: {error}"))?;

    Ok(())
}

pub fn get_setting(connection: &Connection, key: &str) -> Result<Option<String>, String> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|error| format!("读取设置失败: {error}"))
}

pub fn set_setting(connection: &Connection, key: &str, value: &str) -> Result<(), String> {
    connection
        .execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )
        .map_err(|error| format!("保存设置失败: {error}"))?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn seed_if_empty(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
    let count: i64 = connection.query_row("SELECT COUNT(*) FROM servers", [], |row| row.get(0))?;
    if count > 0 {
        return Ok(());
    }

    let seed: Workspace = serde_json::from_str(SEED_JSON)?;
    let transaction = connection.transaction()?;

    for server in &seed.servers {
        transaction.execute(
            "INSERT INTO servers
                (id, name, host, environment, status, latency, kind, port, username)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &server.id,
                &server.name,
                &server.host,
                &server.environment,
                &server.status,
                &server.latency,
                &server.kind,
                server.port,
                &server.username
            ],
        )?;
    }

    for project in &seed.projects {
        transaction.execute(
            "INSERT INTO projects (id, name, title, repo_url, local_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                &project.id,
                &project.name,
                &project.title,
                &project.repo_url,
                &project.local_path
            ],
        )?;
    }

    for deployment in &seed.deployments {
        transaction.execute(
            "INSERT INTO deployments
                (id, server_id, project_id, status, remote_path, domain, nginx_port,
                 branch, ssl, last_deploy, php_version, backups, sync_target)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                &deployment.id,
                &deployment.server_id,
                &deployment.project_id,
                &deployment.status,
                &deployment.remote_path,
                &deployment.domain,
                deployment.nginx_port,
                &deployment.branch,
                i64::from(deployment.ssl),
                &deployment.last_deploy,
                &deployment.php_version,
                deployment.backups,
                &deployment.sync_target
            ],
        )?;
    }

    transaction.commit()?;
    Ok(())
}

pub fn load_workspace(connection: &Connection) -> rusqlite::Result<Workspace> {
    let servers = {
        let mut statement = connection.prepare(
            "SELECT id, name, host, environment, status, latency, kind, port, username
             FROM servers ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Server {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                environment: row.get(3)?,
                status: row.get(4)?,
                latency: row.get(5)?,
                kind: row.get(6)?,
                port: row.get(7)?,
                username: row.get(8)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let projects = {
        let mut statement = connection.prepare(
            "SELECT id, name, title, repo_url, local_path
             FROM projects ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                title: row.get(2)?,
                repo_url: row.get(3)?,
                local_path: row.get(4)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let deployments = {
        let mut statement = connection.prepare(
            "SELECT id, server_id, project_id, status, remote_path, domain, nginx_port,
                    branch, ssl, last_deploy, php_version, backups, sync_target
             FROM deployments ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            let ssl: i64 = row.get(8)?;
            Ok(Deployment {
                id: row.get(0)?,
                server_id: row.get(1)?,
                project_id: row.get(2)?,
                status: row.get(3)?,
                remote_path: row.get(4)?,
                domain: row.get(5)?,
                nginx_port: row.get(6)?,
                branch: row.get(7)?,
                ssl: ssl != 0,
                last_deploy: row.get(9)?,
                php_version: row.get(10)?,
                backups: row.get(11)?,
                sync_target: row.get(12)?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    Ok(Workspace {
        servers,
        projects,
        deployments,
    })
}

pub fn list_runs(
    connection: &Connection,
    deployment_id: &str,
    limit: i64,
) -> rusqlite::Result<Vec<RunRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, deployment_id, command, args_json, status, exit_code,
                started_at, ended_at, error_summary,
                deploy_run_id, step_key, step_title
         FROM runs
         WHERE deployment_id = ?1
         ORDER BY started_at DESC, id DESC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(params![deployment_id, limit], |row| {
        let args_json: String = row.get(3)?;
        let args = serde_json::from_str(&args_json).unwrap_or_default();
        Ok(RunRecord {
            id: row.get(0)?,
            deployment_id: row.get(1)?,
            command: row.get(2)?,
            args,
            status: row.get(4)?,
            exit_code: row.get(5)?,
            started_at: row.get(6)?,
            ended_at: row.get(7)?,
            error_summary: row.get(8)?,
            deploy_run_id: row.get(9)?,
            step_key: row.get(10)?,
            step_title: row.get(11)?,
        })
    })?;
    rows.collect()
}

pub fn list_deploy_runs(
    connection: &Connection,
    deployment_id: &str,
    limit: i64,
) -> rusqlite::Result<Vec<DeployRunRecord>> {
    let mut statement = connection.prepare(
        "SELECT id, deployment_id, status, plan_json, started_at, ended_at, error_summary
         FROM deploy_runs
         WHERE deployment_id = ?1
         ORDER BY started_at DESC, id DESC
         LIMIT ?2",
    )?;
    let rows = statement.query_map(params![deployment_id, limit], |row| {
        Ok(DeployRunRecord {
            id: row.get(0)?,
            deployment_id: row.get(1)?,
            status: row.get(2)?,
            plan_json: row.get(3)?,
            started_at: row.get(4)?,
            ended_at: row.get(5)?,
            error_summary: row.get(6)?,
        })
    })?;
    rows.collect()
}

pub fn load_deployment_config(
    connection: &Connection,
    deployment_id: &str,
) -> rusqlite::Result<DeploymentConfig> {
    let sync_items = {
        let mut statement = connection.prepare(
            "SELECT id, deployment_id, sort, method, name, path, repo, branch, excludes_json
             FROM sync_items
             WHERE deployment_id = ?1
             ORDER BY sort, id",
        )?;
        let rows = statement.query_map([deployment_id], |row| {
            let excludes_json: String = row.get(8)?;
            let excludes = serde_json::from_str(&excludes_json).unwrap_or_default();
            Ok(SyncItem {
                id: row.get(0)?,
                deployment_id: row.get(1)?,
                sort: row.get(2)?,
                method: row.get(3)?,
                name: row.get(4)?,
                path: row.get(5)?,
                repo: row.get(6)?,
                branch: row.get(7)?,
                excludes,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let settings = connection
        .query_row(
            "SELECT config_json, hooks_json, docker_images_json, router_json, nginx_json
             FROM deployment_settings
             WHERE deployment_id = ?1",
            [deployment_id],
            |row| {
                Ok(DeploymentSettings {
                    config: serde_json::from_str(&row.get::<_, String>(0)?).unwrap_or_default(),
                    hooks: serde_json::from_str(&row.get::<_, String>(1)?).unwrap_or_default(),
                    docker_images: serde_json::from_str(&row.get::<_, String>(2)?)
                        .unwrap_or_default(),
                    router: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
                    nginx: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
                })
            },
        )
        .optional()?;

    Ok(DeploymentConfig {
        sync_items,
        settings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_starts_with_empty_workspace() {
        let mut path = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        path.push(format!("phadmin-empty-{}-{stamp}.db", std::process::id()));

        let connection = open(&path).expect("open empty db");
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .expect("journal mode");
        assert_eq!(mode.to_ascii_lowercase(), "delete");
        let workspace = load_workspace(&connection).expect("load workspace");
        assert!(workspace.servers.is_empty());
        assert!(workspace.projects.is_empty());
        assert!(workspace.deployments.is_empty());
        drop(connection);

        for suffix in ["", "-wal", "-shm", "-journal"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    #[test]
    fn creates_server_with_unique_slug_and_defaults() {
        let connection = Connection::open_in_memory().expect("open memory db");
        migrate(&connection).expect("migrate");

        let input = |host: &str, user: &str, password: &str, key_file: &str| ServerInput {
            name: format!("srv-{host}-{user}"),
            host: host.to_string(),
            port: 2222,
            user: user.to_string(),
            password: password.to_string(),
            key_file: key_file.to_string(),
            key_passphrase: String::new(),
            docker_network: "shared-net".to_string(),
        };

        let server = create_server(&connection, &input("10.0.0.5", "tester", "secret", ""))
            .expect("create server");
        assert_eq!(server.name, "srv-10.0.0.5-tester");
        assert_eq!(server.username, "tester");
        assert_eq!(server.port, 2222);
        assert_eq!(server.environment, "production");
        assert_eq!(server.kind, "ssh");
        assert_eq!(server.status, "checking");
        assert_eq!(server.id, "srv-10-0-0-5-tester");
        assert_eq!(
            load_server_config(&connection, &server.id)
                .expect("config")
                .docker_network,
            "shared-net"
        );

        let second = create_server(&connection, &input("10.0.0.6", "tester", "", "/keys/id"))
            .expect("create key server");
        assert_eq!(second.name, "srv-10.0.0.6-tester");
        assert_ne!(second.id, server.id);

        assert!(create_server(&connection, &input("10.0.0.5", "tester", "x", "")).is_err());
        assert!(create_server(&connection, &input("10.0.0.7", "tester", "", "")).is_err());
        assert!(create_server(&connection, &input("10.0.0.8", "tester", "x", "/k")).is_err());
        assert!(create_server(&connection, &input("", "tester", "x", "")).is_err());
        assert!(create_server(&connection, &input("10.0.0.9", "", "x", "")).is_err());
    }

    #[test]
    fn updates_server_and_rejects_name_conflict() {
        let connection = Connection::open_in_memory().expect("open memory db");
        migrate(&connection).expect("migrate");

        let input = |host: &str, password: &str, key_file: &str| ServerInput {
            name: format!("srv-{host}"),
            host: host.to_string(),
            port: 2222,
            user: "tester".to_string(),
            password: password.to_string(),
            key_file: key_file.to_string(),
            key_passphrase: String::new(),
            docker_network: String::new(),
        };

        let first =
            create_server(&connection, &input("10.0.0.5", "old-pass", "")).expect("create first");
        create_server(&connection, &input("10.0.0.6", "", "/keys/id")).expect("create second");

        let updated = update_server(&connection, &first.id, &input("10.0.0.7", "new-pass", ""))
            .expect("update");
        assert_eq!(updated.name, "srv-10.0.0.7");

        let config = load_server_config(&connection, &first.id).expect("config");
        assert_eq!(config.host, "10.0.0.7");
        assert_eq!(config.password, "new-pass");
        assert!(config.key_file.is_empty());
        assert_eq!(config.docker_network, "phalcon-shared");

        assert!(update_server(&connection, &first.id, &input("10.0.0.6", "x", "")).is_err());
        assert!(update_server(&connection, "missing", &input("10.0.0.9", "x", "")).is_err());
    }

    #[test]
    fn deletes_only_unreferenced_servers() {
        let connection = Connection::open_in_memory().expect("open memory db");
        migrate(&connection).expect("migrate");

        let input = |host: &str| ServerInput {
            name: format!("srv-{host}"),
            host: host.to_string(),
            port: 22,
            user: "tester".to_string(),
            password: "secret".to_string(),
            key_file: String::new(),
            key_passphrase: String::new(),
            docker_network: String::new(),
        };

        let referenced = create_server(&connection, &input("10.0.0.5")).expect("referenced");
        let free = create_server(&connection, &input("10.0.0.6")).expect("free");
        connection
            .execute(
                "INSERT INTO projects (id, name, title) VALUES ('p1', 'p', 'P')",
                [],
            )
            .expect("project");
        connection
            .execute(
                "INSERT INTO deployments (id, server_id, project_id, remote_path)
                 VALUES ('d1', ?1, 'p1', '/data/p')",
                [&referenced.id],
            )
            .expect("deployment");

        assert_eq!(
            count_server_references(&connection, &referenced.id).expect("count"),
            1
        );
        assert!(delete_server(&connection, &referenced.id).is_err());
        delete_server(&connection, &free.id).expect("delete unreferenced");
        assert!(load_server_config(&connection, &free.id).is_err());
        assert!(delete_server(&connection, "missing").is_err());
    }

    #[test]
    fn creates_project_records_with_domains_and_settings() {
        let connection = Connection::open_in_memory().expect("open memory db");
        migrate(&connection).expect("migrate");
        let server_id = ensure_local_server(&connection).expect("local server");
        assert_eq!(ensure_local_server(&connection).expect("reuse"), server_id);

        let deployment_id = create_project_records(
            &connection,
            &NewProject {
                name: "demo".to_string(),
                server_id: server_id.clone(),
                remote_path: "D:/demo".to_string(),
                domains: vec!["a.com".to_string(), "b.com".to_string()],
                nginx_port: 8071,
                ssl: true,
                sync_target: "filesystem".to_string(),
                config_php: "['app' => []]".to_string(),
                sync_items: vec![
                    NewSyncItem {
                        method: "git".to_string(),
                        name: "main".to_string(),
                        path: String::new(),
                        repo: "https://example.com/app.git".to_string(),
                        branch: "main".to_string(),
                        excludes: Vec::new(),
                    },
                    NewSyncItem {
                        method: "ftp".to_string(),
                        name: "assets".to_string(),
                        path: "src/App/Projects/demo".to_string(),
                        repo: String::new(),
                        branch: String::new(),
                        excludes: vec!["vendor".to_string()],
                    },
                ],
            },
        )
        .expect("create project");

        assert_eq!(deployment_id, format!("{server_id}-demo"));
        assert!(project_name_exists(&connection, "demo").expect("exists"));
        assert!(port_taken(&connection, &server_id, 8071).expect("port"));
        assert!(!port_taken(&connection, &server_id, 8072).expect("port"));

        let domains: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM domains WHERE deployment_id = ?1",
                [&deployment_id],
                |row| row.get(0),
            )
            .expect("domains");
        assert_eq!(domains, 2);
        let sync_items: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sync_items WHERE deployment_id = ?1",
                [&deployment_id],
                |row| row.get(0),
            )
            .expect("sync items");
        assert_eq!(sync_items, 2);
        assert_eq!(
            load_config_php(&connection, &deployment_id).expect("config"),
            "['app' => []]"
        );

        delete_project_records(&connection, &deployment_id, "demo").expect("cleanup");
        assert!(!project_name_exists(&connection, "demo").expect("exists"));
    }

    #[test]
    fn updates_project_records() {
        let connection = Connection::open_in_memory().expect("open memory db");
        migrate(&connection).expect("migrate");
        let server_id = ensure_local_server(&connection).expect("local server");
        let deployment_id = create_project_records(
            &connection,
            &NewProject {
                name: "edit-demo".to_string(),
                server_id: server_id.clone(),
                remote_path: "D:/old".to_string(),
                domains: vec!["old.com".to_string()],
                nginx_port: 8071,
                ssl: false,
                sync_target: "filesystem".to_string(),
                config_php: "[]".to_string(),
                sync_items: vec![NewSyncItem {
                    method: "git".to_string(),
                    name: "main".to_string(),
                    path: String::new(),
                    repo: "https://example.com/old.git".to_string(),
                    branch: "main".to_string(),
                    excludes: Vec::new(),
                }],
            },
        )
        .expect("create");

        update_project_records(
            &connection,
            &deployment_id,
            &NewProject {
                name: "edit-demo".to_string(),
                server_id: server_id.clone(),
                remote_path: "D:/new".to_string(),
                domains: vec!["a.com".to_string(), "b.com".to_string()],
                nginx_port: 8072,
                ssl: true,
                sync_target: "filesystem".to_string(),
                config_php: "['app' => []]".to_string(),
                sync_items: vec![NewSyncItem {
                    method: "ftp".to_string(),
                    name: "assets".to_string(),
                    path: "src/App/Projects/edit-demo".to_string(),
                    repo: String::new(),
                    branch: String::new(),
                    excludes: vec!["cache".to_string()],
                }],
            },
        )
        .expect("update");

        let row = load_project_config_row(&connection, &deployment_id).expect("row");
        assert_eq!(row.remote_path, "D:/new");
        assert_eq!(row.nginx_port, 8072);
        assert!(row.ssl);
        assert_eq!(row.domains, vec!["a.com", "b.com"]);
        assert_eq!(row.sync_items.len(), 1);
        assert_eq!(row.sync_items[0].0, "ftp");
        assert_eq!(row.sync_items[0].1, "assets");
        assert_eq!(row.config_php, "['app' => []]");
        assert!(!port_taken_except(&connection, &server_id, 8072, &deployment_id).expect("port"));
        assert!(!port_taken_except(&connection, &server_id, 8071, &deployment_id).expect("port"));
    }

    #[test]
    fn seeds_and_loads_workspace() {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        migrate(&connection).expect("migrate");
        seed_if_empty(&mut connection).expect("seed");

        let workspace = load_workspace(&connection).expect("load workspace");
        assert_eq!(workspace.servers.len(), 3);
        assert_eq!(workspace.projects.len(), 5);
        assert_eq!(workspace.deployments.len(), 5);

        let boyu = workspace
            .deployments
            .iter()
            .find(|deployment| deployment.id == "boyu")
            .expect("boyu deployment");
        assert_eq!(boyu.server_id, "prod");
        assert_eq!(boyu.nginx_port, 8071);
        assert!(boyu.ssl);

        // 幂等：再次执行不会重复插入
        seed_if_empty(&mut connection).expect("seed again");
        let again = load_workspace(&connection).expect("load again");
        assert_eq!(again.deployments.len(), 5);
    }

    #[test]
    fn lists_runs_newest_first() {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        migrate(&connection).expect("migrate");
        seed_if_empty(&mut connection).expect("seed");

        connection
            .execute(
                "INSERT INTO runs (id, deployment_id, command, args_json, status, exit_code, started_at)
                 VALUES ('run-1', 'boyu', 'docker', '[\"compose\",\"ps\"]', 'succeeded', 0, '2026-01-01 00:00:00')",
                [],
            )
            .expect("insert run 1");
        connection
            .execute(
                "INSERT INTO runs (id, deployment_id, command, args_json, status, started_at)
                 VALUES ('run-2', 'boyu', 'whoami', '[]', 'running', '2026-01-02 00:00:00')",
                [],
            )
            .expect("insert run 2");

        let runs = list_runs(&connection, "boyu", 50).expect("list runs");
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].id, "run-2");
        assert_eq!(runs[0].status, "running");
        assert_eq!(runs[1].id, "run-1");
        assert_eq!(runs[1].args, vec!["compose", "ps"]);
        assert_eq!(runs[1].exit_code, Some(0));
    }

    #[test]
    fn loads_sync_items_and_settings() {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        migrate(&connection).expect("migrate");
        seed_if_empty(&mut connection).expect("seed");

        connection
            .execute(
                "INSERT INTO sync_items
                    (id, deployment_id, sort, method, name, path, repo, branch, excludes_json)
                 VALUES ('sync-1', 'boyu', 0, 'git', '主仓库', 'src/App',
                         'https://example.com/app.git', 'main', '[\"var/cache\"]')",
                [],
            )
            .expect("insert sync item");
        connection
            .execute(
                "INSERT INTO deployment_settings
                    (deployment_id, config_json, hooks_json, docker_images_json, router_json, nginx_json)
                 VALUES ('boyu', '{\"app\":{\"debug\":false}}', '{}', '{}', '{}', '{\"ssl\":true}')",
                [],
            )
            .expect("insert settings");

        let config = load_deployment_config(&connection, "boyu").expect("load config");
        assert_eq!(config.sync_items.len(), 1);
        assert_eq!(config.sync_items[0].method, "git");
        assert_eq!(config.sync_items[0].excludes, vec!["var/cache"]);
        let settings = config.settings.expect("settings");
        assert_eq!(settings.config["app"]["debug"], false);
        assert_eq!(settings.nginx["ssl"], true);
    }
}
