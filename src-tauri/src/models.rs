use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: String,
    pub name: String,
    pub host: String,
    pub environment: String,
    pub status: String,
    pub latency: String,
    pub kind: String,
    pub port: i64,
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub title: String,
    pub repo_url: String,
    pub local_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deployment {
    pub id: String,
    pub server_id: String,
    pub project_id: String,
    pub status: String,
    pub remote_path: String,
    pub domain: String,
    pub nginx_port: i64,
    pub branch: String,
    pub ssl: bool,
    pub last_deploy: String,
    pub php_version: String,
    pub backups: i64,
    pub sync_target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub servers: Vec<Server>,
    pub projects: Vec<Project>,
    pub deployments: Vec<Deployment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub servers: usize,
    pub projects: usize,
    pub deployments: usize,
    pub sync_items: usize,
}
