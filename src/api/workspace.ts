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
