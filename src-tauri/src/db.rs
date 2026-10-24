use std::error::Error;
use std::path::Path;

use rusqlite::{Connection, params};

use crate::models::{Deployment, Project, Server, Workspace};

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

fn migrate(connection: &Connection) -> rusqlite::Result<()> {
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
            ON deployments(project_id);",
    )
}

fn seed_if_empty(connection: &mut Connection) -> Result<(), Box<dyn Error>> {
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
}
