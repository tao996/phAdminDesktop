use serde::{Deserialize, Serialize};
use serde_json::Value;

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
pub struct ServerInput {
    #[serde(default)]
    pub name: String,
    pub host: String,
    #[serde(default = "default_ssh_port")]
    pub port: i64,
    pub user: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub key_file: String,
    #[serde(default)]
    pub key_passphrase: String,
    #[serde(default)]
    pub docker_network: String,
}

fn default_ssh_port() -> i64 {
    22
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub name: String,
    pub host: String,
    pub port: i64,
    pub user: String,
    pub password: String,
    pub key_file: String,
    pub key_passphrase: String,
    pub docker_network: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub id: String,
    pub deployment_id: String,
    pub command: String,
    pub args: Vec<String>,
    pub status: String,
    pub exit_code: Option<i32>,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error_summary: Option<String>,
    pub deploy_run_id: Option<String>,
    pub step_key: Option<String>,
    pub step_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncItem {
    pub id: String,
    pub deployment_id: String,
    pub sort: i64,
    pub method: String,
    pub name: String,
    pub path: String,
    pub repo: String,
    pub branch: String,
    pub excludes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentSettings {
    pub config: Value,
    pub hooks: Value,
    pub docker_images: Value,
    pub router: Value,
    pub nginx: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeploymentConfig {
    pub sync_items: Vec<SyncItem>,
    pub settings: Option<DeploymentSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeployRunRecord {
    pub id: String,
    pub deployment_id: String,
    pub status: String,
    pub plan_json: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub error_summary: Option<String>,
}
