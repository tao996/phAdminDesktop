use std::error::Error;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::models::{
    DeployRunRecord, Deployment, DeploymentConfig, DeploymentSettings, Project, RunRecord, Server,
    SyncItem, Workspace,
};

const SEED_JSON: &str = include_str!("../seed.json");

pub fn open(path: &Path) -> Result<Connection, Box<dyn Error>> {
    let mut connection = Connection::open(path)?;
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )?;
    migrate(&connection)?;
    seed_if_empty(&mut connection)?;
    Ok(connection)
}

pub(crate) fn migrate(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS servers (
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
