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

export function canUseTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function loadWorkspace(): Promise<WorkspaceResponse> {
  return invoke<WorkspaceResponse>("load_workspace");
}

export async function importLegacyConfig(json: string): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_legacy_config", { json });
}
