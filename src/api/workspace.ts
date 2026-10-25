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

export interface ImportSummary {
  servers: number;
  projects: number;
  deployments: number;
  syncItems: number;
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

export async function importLegacyConfig(json: string): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_legacy_config", { json });
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
}

export async function writeGeneratedConfig(
  deploymentId: string,
): Promise<GeneratedFiles> {
  return invoke<GeneratedFiles>("write_generated_config", { deploymentId });
}
