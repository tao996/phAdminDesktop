use std::collections::{HashMap, HashSet};
use std::error::Error;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::models::ImportSummary;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProjectConfig {
    #[serde(default)]
    ssh: LegacySsh,
    #[serde(default)]
    project: LegacyProject,
    #[serde(default)]
    domains: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
    #[serde(default)]
    sync: LegacySync,
    #[serde(default)]
    nginx: LegacyNginx,
    #[serde(default)]
    config: Value,
    #[serde(default)]
    hooks: Value,
    #[serde(default)]
    docker: Value,
    #[serde(default)]
    router: Value,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySsh {
    #[serde(default)]
    host: String,
    #[serde(default = "default_port")]
    port: i64,
    #[serde(default)]
    user: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacyProject {
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    nginx_port: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySync {
    #[serde(default)]
    items: Vec<LegacySyncItem>,
    #[serde(default)]
    target: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySyncItem {
    #[serde(default)]
    method: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    repo: String,
    #[serde(default = "default_branch")]
    branch: String,
    #[serde(default)]
    excludes: Vec<String>,
}

#[derive(Debug, Default, Deserialize, serde::Serialize)]
struct LegacyNginx {
    #[serde(default)]
    ssl: bool,
}

fn default_port() -> i64 {
    22
}

fn default_branch() -> String {
    "main".to_string()
}

fn slug(value: &str) -> String {
    let result: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let result = result.trim_matches('-').to_string();
    if result.is_empty() {
        "item".to_string()
    } else {
        result
    }
}

fn is_secret_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    ["PASSWORD", "SECRET", "TOKEN", "PRIVATE_KEY", "APP_KEY"]
        .iter()
        .any(|needle| upper.contains(needle))
}

pub fn import(
    connection: &mut Connection,
    json_text: &str,
) -> Result<ImportSummary, Box<dyn Error>> {
    let configs: HashMap<String, LegacyProjectConfig> = serde_json::from_str(json_text)?;
    let transaction = connection.transaction()?;

    let mut server_ids = HashSet::new();
    let mut project_ids = HashSet::new();
    let mut deployment_ids = HashSet::new();
    let mut sync_item_count = 0;

    for (project_name, config) in configs {
        let project_name = if config.project.name.trim().is_empty() {
            project_name
        } else {
            config.project.name.trim().to_string()
        };

        let server_id = ensure_server(&transaction, &config, &project_name)?;
        let project_id = ensure_project(&transaction, &project_name)?;
        let deployment_id = ensure_deployment(
            &transaction,
            &server_id,
            &project_id,
            &project_name,
            &config,
        )?;

        server_ids.insert(server_id);
        project_ids.insert(project_id);
        deployment_ids.insert(deployment_id.clone());

        sync_item_count += replace_sync_items(&transaction, &deployment_id, &config.sync)?;
        replace_domains(&transaction, &deployment_id, &config.domains)?;
        replace_env(&transaction, &deployment_id, &config.env)?;
        write_settings(&transaction, &deployment_id, &config)?;
    }

    transaction.commit()?;

    Ok(ImportSummary {
        servers: server_ids.len(),
        projects: project_ids.len(),
        deployments: deployment_ids.len(),
        sync_items: sync_item_count,
    })
}

fn ensure_server(
    transaction: &Transaction,
    config: &LegacyProjectConfig,
    project_name: &str,
) -> Result<String, Box<dyn Error>> {
    let host = config.ssh.host.trim();

    if host.is_empty() {
        if let Some(id) = transaction
            .query_row(
                "SELECT id FROM servers WHERE kind = 'filesystem' ORDER BY id LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        let id = "local".to_string();
        transaction.execute(
            "INSERT OR IGNORE INTO servers
                (id, name, host, environment, status, latency, kind, port, username)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                &id,
                "本地 filesystem",
                "filesystem",
                "local",
                "checking",
                "—",
                "filesystem",
                22,
                ""
            ],
        )?;
        return Ok(id);
    }

    let port = if config.ssh.port > 0 {
        config.ssh.port
    } else {
        22
    };
    let user = if config.ssh.user.trim().is_empty() {
        "root"
    } else {
        config.ssh.user.trim()
    };

    if let Some(id) = transaction
        .query_row(
            "SELECT id FROM servers WHERE host = ?1 AND port = ?2 AND username = ?3 LIMIT 1",
            params![host, port, user],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(id);
    }

    let environment = if project_name.to_ascii_lowercase().contains("staging") {
        "staging"
    } else {
        "production"
    };
    let id = format!("server-{}-{}-{}", slug(host), port, slug(user));
    let name = format!("{user}@{host}:{port}");
    transaction.execute(
        "INSERT OR IGNORE INTO servers
            (id, name, host, environment, status, latency, kind, port, username)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            &id,
            &name,
            host,
            environment,
            "checking",
            "—",
            "ssh",
            port,
            user
        ],
    )?;

    let actual_id = transaction.query_row(
        "SELECT id FROM servers WHERE name = ?1",
        params![&name],
        |row| row.get::<_, String>(0),
    )?;
    Ok(actual_id)
}

fn ensure_project(transaction: &Transaction, name: &str) -> Result<String, Box<dyn Error>> {
    if let Some(id) = transaction
        .query_row(
            "SELECT id FROM projects WHERE name = ?1 LIMIT 1",
            params![name],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(id);
    }

    let id = format!("project-{name}");
    transaction.execute(
        "INSERT OR IGNORE INTO projects (id, name, title, repo_url, local_path)
         VALUES (?1, ?2, ?3, '', '')",
        params![&id, name, name],
    )?;

    let actual_id = transaction.query_row(
        "SELECT id FROM projects WHERE name = ?1",
        params![name],
        |row| row.get::<_, String>(0),
    )?;
    Ok(actual_id)
}

fn ensure_deployment(
    transaction: &Transaction,
    server_id: &str,
    project_id: &str,
    project_name: &str,
    config: &LegacyProjectConfig,
) -> Result<String, Box<dyn Error>> {
    let existing_id = transaction
        .query_row(
            "SELECT id FROM deployments WHERE server_id = ?1 AND project_id = ?2",
            params![server_id, project_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let deployment_id =
        existing_id.unwrap_or_else(|| format!("deployment-{server_id}-{project_id}"));
    let sync_target = match config.sync.target.as_deref() {
        Some("filesystem") => "filesystem",
        _ => "remote",
    };
    let branch = config
        .sync
        .items
        .first()
        .map(|item| item.branch.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("main");
    let remote_path = if config.project.path.trim().is_empty() {
        format!("/data/{project_name}")
    } else {
        config.project.path.trim().to_string()
    };
    let domain = config.domains.first().cloned().unwrap_or_default();
    let nginx_port = if config.project.nginx_port > 0 {
        config.project.nginx_port
    } else {
        8071
    };

    transaction.execute(
        "INSERT INTO deployments
            (id, server_id, project_id, status, remote_path, domain, nginx_port,
             branch, ssl, last_deploy, php_version, backups, sync_target)
         VALUES (?1, ?2, ?3, 'unknown', ?4, ?5, ?6, ?7, ?8, '未知', 'PHP 8.5.12', 0, ?9)
         ON CONFLICT(id) DO UPDATE SET
            server_id = excluded.server_id,
            project_id = excluded.project_id,
            remote_path = excluded.remote_path,
            domain = excluded.domain,
            nginx_port = excluded.nginx_port,
            branch = excluded.branch,
            ssl = excluded.ssl,
            sync_target = excluded.sync_target,
            updated_at = datetime('now')",
        params![
            &deployment_id,
            server_id,
            project_id,
            remote_path,
            domain,
            nginx_port,
            branch,
            i64::from(config.nginx.ssl),
            sync_target
        ],
    )?;

    Ok(deployment_id)
}

fn replace_sync_items(
    transaction: &Transaction,
    deployment_id: &str,
    sync: &LegacySync,
) -> Result<usize, Box<dyn Error>> {
    transaction.execute(
        "DELETE FROM sync_items WHERE deployment_id = ?1",
        params![deployment_id],
    )?;

    for (index, item) in sync.items.iter().enumerate() {
        if item.method.trim().is_empty() {
            continue;
        }
        let id = format!("{deployment_id}-sync-{index}");
        let excludes = serde_json::to_string(&item.excludes)?;
        transaction.execute(
            "INSERT INTO sync_items
                (id, deployment_id, sort, method, name, path, repo, branch, excludes_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                deployment_id,
                index as i64,
                item.method.trim(),
                item.name.trim(),
                item.path.trim(),
                item.repo.trim(),
                item.branch.trim(),
                excludes
            ],
        )?;
    }

    Ok(sync
        .items
        .iter()
        .filter(|item| !item.method.trim().is_empty())
        .count())
}

fn replace_domains(
    transaction: &Transaction,
    deployment_id: &str,
    domains: &[String],
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "DELETE FROM domains WHERE deployment_id = ?1",
        params![deployment_id],
    )?;

    for (index, domain) in domains.iter().enumerate() {
        if domain.trim().is_empty() {
            continue;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO domains (id, deployment_id, domain, sort)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                format!("{deployment_id}-domain-{index}"),
                deployment_id,
                domain.trim(),
                index as i64
            ],
        )?;
    }

    Ok(())
}

fn replace_env(
    transaction: &Transaction,
    deployment_id: &str,
    env: &HashMap<String, String>,
) -> Result<(), rusqlite::Error> {
    transaction.execute(
        "DELETE FROM env_vars WHERE deployment_id = ?1",
        params![deployment_id],
    )?;

    for (index, (key, value)) in env.iter().enumerate() {
        let secret = is_secret_key(key);
        transaction.execute(
            "INSERT INTO env_vars
                (id, deployment_id, key, value, is_secret, secret_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
            params![
                format!("{deployment_id}-env-{index}"),
                deployment_id,
                key,
                if secret { None } else { Some(value) },
                i64::from(secret)
            ],
        )?;
    }

    Ok(())
}

fn write_settings(
    transaction: &Transaction,
    deployment_id: &str,
    config: &LegacyProjectConfig,
) -> Result<(), Box<dyn Error>> {
    let config_json = if config.config.is_null() {
        "{}".to_string()
    } else {
        serde_json::to_string(&config.config)?
    };
    let hooks_json = if config.hooks.is_null() {
        "{}".to_string()
    } else {
        serde_json::to_string(&config.hooks)?
    };
    let docker_images = config
        .docker
        .get("images")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let docker_images_json = serde_json::to_string(&docker_images)?;
    let router_json = if config.router.is_null() {
        "{}".to_string()
    } else {
        serde_json::to_string(&config.router)?
    };
    let nginx_json = serde_json::to_string(&config.nginx)?;

    transaction.execute(
        "INSERT INTO deployment_settings
            (deployment_id, config_json, hooks_json, docker_images_json, router_json, nginx_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(deployment_id) DO UPDATE SET
            config_json = excluded.config_json,
            hooks_json = excluded.hooks_json,
            docker_images_json = excluded.docker_images_json,
            router_json = excluded.router_json,
            nginx_json = excluded.nginx_json",
        params![
            deployment_id,
            config_json,
            hooks_json,
            docker_images_json,
            router_json,
            nginx_json
        ],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    const LEGACY_JSON: &str = r#"{
      "demo-import": {
        "ssh": {"host": "10.0.0.9", "port": 22, "user": "deploy"},
        "project": {"name": "demo-import", "path": "/data/demo", "nginxPort": 8090},
        "domains": ["demo.example.com"],
        "env": {"MYSQL_DATABASE": "demo_db", "MYSQL_PASSWORD": "secret"},
        "sync": {
          "target": "remote",
          "items": [{"method": "bundle", "branch": "main"}]
        },
        "nginx": {"ssl": true},
        "config": {"app": {"title": "Demo"}},
        "hooks": {}
      }
    }"#;

    #[test]
    fn imports_legacy_config_and_is_idempotent() {
        let mut connection = Connection::open_in_memory().expect("open in-memory db");
        db::migrate(&connection).expect("migrate");
        db::seed_if_empty(&mut connection).expect("seed");

        let summary = import(&mut connection, LEGACY_JSON).expect("import");
        assert_eq!(summary.servers, 1);
        assert_eq!(summary.projects, 1);
        assert_eq!(summary.deployments, 1);
        assert_eq!(summary.sync_items, 1);

        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM deployments", [], |row| row.get(0))
            .expect("count deployments");
        assert_eq!(count, 6);

        let secret: (Option<String>, i64) = connection
            .query_row(
                "SELECT value, is_secret FROM env_vars WHERE key = 'MYSQL_PASSWORD'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("secret env");
        assert_eq!(secret.0, None);
        assert_eq!(secret.1, 1);

        let again = import(&mut connection, LEGACY_JSON).expect("import again");
        assert_eq!(again.deployments, 1);
        let count_again: i64 = connection
            .query_row("SELECT COUNT(*) FROM deployments", [], |row| row.get(0))
            .expect("count deployments again");
        assert_eq!(count_again, 6);
    }
}
