import { invoke } from "@tauri-apps/api/core";
import type { Server } from "../data/mock";

export interface WorkspaceServer extends Server {
  kind: string;
  port: number;
  username: string;
}

export interface WorkspaceProject {
  id: string;
  name: string;
  title: string;
  repoUrl: string;
  localPath: string;
}

export interface WorkspaceDeployment {
  id: string;
  serverId: string;
  projectId: string;
  status: string;
  remotePath: string;
  domain: string;
  nginxPort: number;
  branch: string;
  ssl: boolean;
  lastDeploy: string;
  phpVersion: string;
  backups: number;
  syncTarget: string;
}

export interface WorkspaceResponse {
  servers: WorkspaceServer[];
  projects: WorkspaceProject[];
  deployments: WorkspaceDeployment[];
}

export interface RunOutput {
  runId: string;
  deploymentId: string;
  stream: "stdout" | "stderr";
  line: string;
}

export interface RunFinished {
  runId: string;
  deploymentId: string;
  exitCode: number | null;
  status: "succeeded" | "failed" | "canceled" | "running";
}

export interface RunRecord {
  id: string;
  deploymentId: string;
  command: string;
  args: string[];
  status: "queued" | "running" | "succeeded" | "failed" | "canceled";
  exitCode: number | null;
  startedAt: string | null;
  endedAt: string | null;
  errorSummary: string | null;
  deployRunId: string | null;
  stepKey: string | null;
  stepTitle: string | null;
}

export interface DeployStep {
  key: string;
  title: string;
  program: string;
  args: string[];
}

export interface DeployRunRecord {
  id: string;
  deploymentId: string;
  status: "running" | "succeeded" | "failed" | "canceled";
  planJson: string;
  startedAt: string | null;
  endedAt: string | null;
  errorSummary: string | null;
}

export interface DeployStart {
  deployRunId: string;
  steps: DeployStep[];
}

export interface DeployStepEvent {
  deployRunId: string;
  deploymentId: string;
  runId: string;
  stepKey: string;
  stepTitle: string;
  index: number;
  total: number;
}

export interface DeployFinished {
  deployRunId: string;
  deploymentId: string;
  status: "running" | "succeeded" | "failed" | "canceled";
  error: string | null;
}

export interface SyncItem {
  id: string;
  deploymentId: string;
  sort: number;
  method: string;
  name: string;
  path: string;
  repo: string;
  branch: string;
  excludes: string[];
}

export interface DeploymentSettings {
  config: unknown;
  hooks: unknown;
  dockerImages: unknown;
  router: unknown;
  nginx: unknown;
}

export interface DeploymentConfig {
  syncItems: SyncItem[];
  settings: DeploymentSettings | null;
}

export interface ContainerInfo {
  service: string;
  name: string;
  image: string;
  status: "running" | "stopped";
  state: string;
  health: string;
  port: string;
}

export function canUseTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function loadWorkspace(): Promise<WorkspaceResponse> {
  return invoke<WorkspaceResponse>("load_workspace");
}

export interface ServerInput {
  name: string;
  host: string;
  port: number;
  user: string;
  password: string;
  keyFile: string;
  keyPassphrase: string;
  dockerNetwork: string;
}

export async function createServer(input: ServerInput): Promise<WorkspaceServer> {
  return invoke<WorkspaceServer>("create_server", { input });
}

export interface ServerConfig {
  name: string;
  host: string;
  port: number;
  user: string;
  password: string;
  keyFile: string;
  keyPassphrase: string;
  dockerNetwork: string;
}

export async function getServerConfig(serverId: string): Promise<ServerConfig> {
  return invoke<ServerConfig>("get_server_config", { serverId });
}

export async function updateServer(
  serverId: string,
  input: ServerInput,
): Promise<WorkspaceServer> {
  return invoke<WorkspaceServer>("update_server", { serverId, input });
}

export async function countServerReferences(serverId: string): Promise<number> {
  return invoke<number>("count_server_references", { serverId });
}

export async function deleteServer(serverId: string): Promise<void> {
  return invoke<void>("delete_server", { serverId });
}

export async function getLocalRepoPath(): Promise<string | null> {
  return invoke<string | null>("get_local_repo_path");
}

export async function setLocalRepoPath(path: string): Promise<void> {
  return invoke<void>("set_local_repo_path", { path });
}

export interface PreviewFile {
  path: string;
  content: string;
}

export async function previewProjectFiles(deploymentId: string): Promise<PreviewFile[]> {
  return invoke<PreviewFile[]>("preview_project_files", { deploymentId });
}

export interface ProjectSyncItemInput {
  method: "git" | "bundle" | "ftp";
  name: string;
  path: string;
  repo: string;
  branch: string;
  excludes: string;
}

export interface ProjectCreateInput {
  name: string;
  mode: "remote" | "local";
  serverId: string;
  targetPath: string;
  deployDir: string;
  appName: string;
  mysqlDatabase: string;
  timezone: string;
  mysqlUser: string;
  mysqlPassword: string;
  redisPassword: string;
  dataPathHost: string;
  openPort: number;
  domains: string;
  ssl: boolean;
  phpConfig: string;
  syncItems: ProjectSyncItemInput[];
}

export async function createProject(input: ProjectCreateInput): Promise<string> {
  return invoke<string>("create_project", { input });
}

export interface ProjectConfig extends ProjectCreateInput {
  deploymentId: string;
}

export async function getProjectConfig(deploymentId: string): Promise<ProjectConfig> {
  return invoke<ProjectConfig>("get_project_config", { deploymentId });
}

export async function updateProject(
  deploymentId: string,
  input: ProjectCreateInput,
): Promise<void> {
  return invoke<void>("update_project", { deploymentId, input });
}
export interface TemplateInfo {
  path: string;
  kind: "builtin" | "override" | "custom" | string;
  updatedAt?: string | null;
}

export interface TemplateDetail {
  path: string;
  content: string;
  kind: "builtin" | "override" | "custom" | string;
  builtin: boolean;
  updatedAt?: string | null;
}

export interface TemplatePreview {
  content: string;
  unresolved: string[];
}

export async function listTemplates(): Promise<TemplateInfo[]> {
  return invoke<TemplateInfo[]>("list_templates");
}

export async function getTemplate(path: string): Promise<TemplateDetail> {
  return invoke<TemplateDetail>("get_template", { path });
}

export async function saveTemplate(
  path: string,
  content: string,
): Promise<TemplateDetail> {
  return invoke<TemplateDetail>("save_template", { path, content });
}

export async function resetTemplate(path: string): Promise<void> {
  return invoke<void>("reset_template", { path });
}

export async function previewTemplate(
  path: string,
  deploymentId?: string,
): Promise<TemplatePreview> {
  return invoke<TemplatePreview>("preview_template", {
    path,
    deploymentId: deploymentId || null,
  });
}

export async function openAppDataDir(): Promise<string> {
  return invoke<string>("open_app_data_dir");
}

export async function runCommand(
  deploymentId: string,
  program: string,
  args: string[],
): Promise<string> {
  return invoke<string>("run_command", { deploymentId, program, args });
}

export async function cancelRun(runId: string): Promise<void> {
  return invoke<void>("cancel_run", { runId });
}

export async function listRuns(
  deploymentId: string,
  limit = 50,
): Promise<RunRecord[]> {
  return invoke<RunRecord[]>("list_runs", { deploymentId, limit });
}

export async function listDeployRuns(
  deploymentId: string,
  limit = 20,
): Promise<DeployRunRecord[]> {
  return invoke<DeployRunRecord[]>("list_deploy_runs", { deploymentId, limit });
}

export async function getContainers(
  deploymentId: string,
): Promise<ContainerInfo[]> {
  return invoke<ContainerInfo[]>("get_containers", { deploymentId });
}

export async function getRunLog(runId: string): Promise<string> {
  return invoke<string>("read_run_log", { runId });
}

export async function startDeploy(
  deploymentId: string,
  restartPhp: boolean,
): Promise<DeployStart> {
  return invoke<DeployStart>("start_deploy", { deploymentId, restartPhp });
}

export async function previewDeployPlan(
  deploymentId: string,
  restartPhp: boolean,
): Promise<DeployStep[]> {
  return invoke<DeployStep[]>("preview_deploy_plan", {
    deploymentId,
    restartPhp,
  });
}

export async function loadDeploymentConfig(
  deploymentId: string,
): Promise<DeploymentConfig> {
  return invoke<DeploymentConfig>("load_deployment_config", { deploymentId });
}

export async function syncGitItem(
  deploymentId: string,
  itemId: string,
): Promise<string> {
  return invoke<string>("sync_git_item", { deploymentId, itemId });
}

export async function syncBundleItem(
  deploymentId: string,
  itemId: string,
): Promise<string> {
  return invoke<string>("sync_bundle_item", { deploymentId, itemId });
}

export async function syncFtpItem(
  deploymentId: string,
  itemId: string,
): Promise<string> {
  return invoke<string>("sync_ftp_item", { deploymentId, itemId });
}

export async function renderConfigPreview(deploymentId: string): Promise<string> {
  return invoke<string>("render_config_preview", { deploymentId });
}

export async function renderNginxPreview(deploymentId: string): Promise<string> {
  return invoke<string>("render_nginx_preview", { deploymentId });
}

export interface GeneratedFiles {
  directory: string;
  envPath: string;
  nginxPath: string;
  composePath: string;
  configPath: string;
}

export interface GeneratedStatus {
  directory: string;
  exists: boolean;
  envChanged: boolean;
  nginxChanged: boolean;
  composeChanged: boolean;
  configChanged: boolean;
}

export async function generatedConfigStatus(
  deploymentId: string,
): Promise<GeneratedStatus> {
  return invoke<GeneratedStatus>("generated_config_status", { deploymentId });
}

export async function renderComposePreview(): Promise<string> {
  return invoke<string>("render_compose_preview");
}

export async function renderConfigPhpPreview(deploymentId: string): Promise<string> {
  return invoke<string>("render_config_php_preview", { deploymentId });
}

export async function writeGeneratedConfig(
  deploymentId: string,
): Promise<GeneratedFiles> {
  return invoke<GeneratedFiles>("write_generated_config", { deploymentId });
}

export async function openGeneratedDir(deploymentId: string): Promise<void> {
  return invoke<void>("open_generated_dir", { deploymentId });
}

export async function applyGeneratedConfig(
  deploymentId: string,
  backup: boolean,
): Promise<string> {
  return invoke<string>("apply_generated_config", { deploymentId, backup });
}

export interface RemoteFileStatus {
  file: string;
  exists: boolean;
  changed: boolean;
}

export async function remoteConfigStatus(
  deploymentId: string,
): Promise<RemoteFileStatus[]> {
  return invoke<RemoteFileStatus[]>("remote_config_status", { deploymentId });
}

export interface ConfigBackup {
  name: string;
  file: string;
}

export async function listConfigBackups(deploymentId: string): Promise<ConfigBackup[]> {
  return invoke<ConfigBackup[]>("list_config_backups", { deploymentId });
}

export async function restoreConfigBackup(
  deploymentId: string,
  backupName: string,
): Promise<string> {
  return invoke<string>("restore_config_backup", { deploymentId, backupName });
}

export async function pruneConfigBackups(
  deploymentId: string,
  keep: number,
): Promise<number> {
  return invoke<number>("prune_config_backups", { deploymentId, keep });
}

export async function prepareRuntimeAssets(deploymentId: string): Promise<string> {
  return invoke<string>("prepare_runtime_assets", { deploymentId });
}

export interface EnvVarInfo {
  key: string;
  isSecret: boolean;
  hasValue: boolean;
  /** 密文变量的值不会从后端返回，这里恒为 null。 */
  value: string | null;
  required: boolean;
}

export async function listEnvVars(deploymentId: string): Promise<EnvVarInfo[]> {
  return invoke<EnvVarInfo[]>("list_env_vars", { deploymentId });
}

export async function setEnvVar(
  deploymentId: string,
  key: string,
  value: string,
): Promise<EnvVarInfo> {
  return invoke<EnvVarInfo>("set_env_var", { deploymentId, key, value });
}

export async function deleteEnvVar(deploymentId: string, key: string): Promise<void> {
  return invoke<void>("delete_env_var", { deploymentId, key });
}

export interface DatabaseBackupFile {
  path: string;
  bytes: number;
}

export async function backupDatabase(
  deploymentId: string,
): Promise<DatabaseBackupFile> {
  return invoke<DatabaseBackupFile>("backup_database", { deploymentId });
}

export interface BackupEntry {
  name: string;
  path: string;
  bytes: number;
}

export async function listDatabaseBackups(
  deploymentId: string,
): Promise<BackupEntry[]> {
  return invoke<BackupEntry[]>("list_database_backups", { deploymentId });
}

export async function downloadDatabaseBackup(
  deploymentId: string,
  name: string,
): Promise<string> {
  return invoke<string>("download_database_backup", { deploymentId, name });
}

export async function restoreDatabaseBackup(
  deploymentId: string,
  name: string,
): Promise<{ snapshotPath: string; bytes: number }> {
  return invoke<{ snapshotPath: string; bytes: number }>("restore_database_backup", {
    deploymentId,
    name,
  });
}

/* ---------------- 原生运维（替代 php admin） ---------------- */

export async function migrateDatabase(deploymentId: string): Promise<string> {
  return invoke<string>("migrate_database", { deploymentId });
}

export async function seedDatabase(deploymentId: string): Promise<string> {
  return invoke<string>("seed_database", { deploymentId });
}

export async function openSeedDirectory(deploymentId: string): Promise<string> {
  return invoke<string>("open_seed_directory", { deploymentId });
}

export interface BackupCronResult {
  project: string;
  time: string;
  keepDays: number;
  backupDir: string;
  transcript: string;
}

export async function installBackupCron(
  deploymentId: string,
  time: string,
  keepDays: number,
): Promise<BackupCronResult> {
  return invoke<BackupCronResult>("install_backup_cron", {
    deploymentId,
    time,
    keepDays,
  });
}

export async function removeBackupCron(deploymentId: string): Promise<string> {
  return invoke<string>("remove_backup_cron", { deploymentId });
}

export async function listRemoteBackups(deploymentId: string): Promise<string> {
  return invoke<string>("list_remote_backups", { deploymentId });
}

export async function publishNginxConfig(deploymentId: string): Promise<string> {
  return invoke<string>("publish_nginx_config", { deploymentId });
}

export async function removeNginxConfig(deploymentId: string): Promise<string> {
  return invoke<string>("remove_nginx_config", { deploymentId });
}

export async function reloadHostNginx(deploymentId: string): Promise<string> {
  return invoke<string>("reload_host_nginx", { deploymentId });
}

export async function readNginxLog(
  deploymentId: string,
  kind: "access" | "error",
): Promise<string> {
  return invoke<string>("read_nginx_log", { deploymentId, kind });
}

export async function checkServer(deploymentId: string): Promise<string> {
  return invoke<string>("check_server", { deploymentId });
}

export interface SslResult {
  primaryDomain: string;
  certificate: string;
  key: string;
  transcript: string;
}

export async function enableSsl(
  deploymentId: string,
  email: string,
): Promise<SslResult> {
  return invoke<SslResult>("enable_ssl", { deploymentId, email });
}

export async function resetProject(deploymentId: string): Promise<string> {
  return invoke<string>("reset_project", { deploymentId });
}

export async function generateGitDeployKey(deploymentId: string): Promise<string> {
  return invoke<string>("generate_git_deploy_key", { deploymentId });
}

export interface TunnelInfo {
  deploymentId: string;
  localPort: number;
  remote: string;
  host: string;
}

export async function startSshTunnel(
  deploymentId: string,
  localPort: number,
): Promise<TunnelInfo> {
  return invoke<TunnelInfo>("start_ssh_tunnel", { deploymentId, localPort });
}

export async function stopSshTunnel(deploymentId: string): Promise<string> {
  return invoke<string>("stop_ssh_tunnel", { deploymentId });
}

export async function listSshTunnels(): Promise<TunnelInfo[]> {
  return invoke<TunnelInfo[]>("list_ssh_tunnels");
}

export interface PmaInfo {
  container: string;
  url: string;
  network: string;
  username: string;
  password: string;
}

export async function startPhpMyAdmin(
  deploymentId: string,
  hostPort: number,
): Promise<PmaInfo> {
  return invoke<PmaInfo>("start_phpmyadmin", { deploymentId, hostPort });
}

export async function stopPhpMyAdmin(deploymentId: string): Promise<string> {
  return invoke<string>("stop_phpmyadmin", { deploymentId });
}

