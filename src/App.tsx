import { useEffect, useState } from "react";
import "./App.css";
import { TopBar } from "./components/TopBar";
import { Sidebar } from "./components/Sidebar";
import { ProjectTabs } from "./components/ProjectTabs";
import { Workspace } from "./components/Workspace";
import { TerminalPanel } from "./components/TerminalPanel";
import { RunsModal } from "./components/RunsModal";
import { TemplateManager } from "./components/TemplateManager";
import { AddProjectModal } from "./components/AddProjectModal";
import {
  CommandPalette,
  ServerModal,
  SettingsModal,
  type PaletteCommand,
} from "./components/Dialogs";
import { Icon } from "./components/Icon";
import {
  activities,
  initialTerminalLines,
  projects as seedProjects,
  servers as seedServers,
  type Project,
  type ProjectStatus,
  type Server,
  type TerminalLine,
  type TerminalLineType,
} from "./data/mock";
import { listen } from "@tauri-apps/api/event";
import { parseCommandLine } from "./lib/command";
import {
  canUseTauri,
  loadWorkspace,
  loadDeploymentConfig,
  listRuns,
  runCommand,
  startDeploy,
  applyGeneratedConfig,
  writeGeneratedConfig,
  prepareRuntimeAssets,
  backupDatabase,
  listDatabaseBackups,
  downloadDatabaseBackup,
  restoreDatabaseBackup,
  listConfigBackups,
  restoreConfigBackup,
  syncBundleItem,
  syncFtpItem,
  syncGitItem,
  migrateDatabase,
  seedDatabase,
  openSeedDirectory,
  installBackupCron,
  removeBackupCron,
  listRemoteBackups,
  publishNginxConfig,
  removeNginxConfig,
  reloadHostNginx,
  readNginxLog,
  checkServer,
  enableSsl,
  resetProject,
  generateGitDeployKey,
  startSshTunnel,
  stopSshTunnel,
  startPhpMyAdmin,
  stopPhpMyAdmin,
  createServer,
  updateServer,
  deleteServer,
  createProject,
  updateProject,
  openAppDataDir,
  type ProjectCreateInput,
  type ServerInput,
  type DeployFinished,
  type DeployStepEvent,
  type RunFinished,
  type RunOutput,
  type WorkspaceResponse,
} from "./api/workspace";
import type { WorkspaceSection } from "./components/Panels";

function now() {
  return new Date().toLocaleTimeString("zh-CN", { hour12: false });
}

function formatDbTime(raw: string): string {
  if (!raw || raw === "从未") return raw;
  const parsed = new Date(`${raw.replace(" ", "T")}Z`);
  return Number.isNaN(parsed.getTime())
    ? raw
    : parsed.toLocaleString("zh-CN", { hour12: false });
}

const sectionIds: WorkspaceSection[] = [
  "overview",
  "deploy",
  "files",
  "containers",
  "nginx",
  "database",
  "logs",
];

function readBootState() {
  const params = new URLSearchParams(window.location.search);
  const requestedServer = params.get("server");
  const serverId = seedServers.some((item) => item.id === requestedServer)
    ? (requestedServer as string)
    : "prod";
  const serverProjects = seedProjects.filter((project) => project.serverId === serverId);
  const requestedProject = params.get("project");
  const project =
    serverProjects.find((item) => item.id === requestedProject) ?? serverProjects[0];
  const requestedSection = params.get("section");
  const section = sectionIds.includes(requestedSection as WorkspaceSection)
    ? (requestedSection as WorkspaceSection)
    : "overview";
  const openIds = requestedProject && project
    ? [project.id]
    : serverProjects.slice(0, 2).map((item) => item.id);
  return { serverId, project, section, openIds, dialog: params.get("dialog") };
}

const boot = readBootState();

function App() {
  const [serverId, setServerId] = useState(boot.serverId);
  const [servers, setServers] = useState<Server[]>(canUseTauri() ? [] : seedServers);
  const [projectList, setProjectList] = useState<Project[]>(
    canUseTauri() ? [] : seedProjects,
  );
  const [openIds, setOpenIds] = useState<string[]>(boot.openIds);
  const [activeId, setActiveId] = useState(boot.project?.id ?? "");
  const [section, setSection] = useState<WorkspaceSection>(boot.section);
  const [search, setSearch] = useState("");
  const [terminals, setTerminals] = useState<Record<string, TerminalLine[]>>(initialTerminalLines);
  const [terminalCollapsed, setTerminalCollapsed] = useState(false);
  const [projectDialog, setProjectDialog] = useState<{ deploymentId?: string } | null>(
    boot.dialog === "add" ? {} : null,
  );
  const [serverDialog, setServerDialog] = useState<{ serverId?: string } | null>(null);
  const [showTemplates, setShowTemplates] = useState(false);
  const [showPalette, setShowPalette] = useState(boot.dialog === "command");
  const [showSettings, setShowSettings] = useState(boot.dialog === "settings");
  const [showRuns, setShowRuns] = useState(false);
  const [runsVersion, setRunsVersion] = useState(0);
  const [toast, setToast] = useState<{ id: number; message: string } | null>(null);

  const activeProject = projectList.find((project) => project.id === activeId) ?? null;
  const server =
    servers.find((item) => item.id === (activeProject?.serverId ?? serverId)) ??
    servers[0] ??
    null;
  const activeLines = activeProject ? terminals[activeProject.id] ?? [] : [];

  const applyWorkspace = (data: WorkspaceResponse) => {
    const projectById = new Map(data.projects.map((project) => [project.id, project]));
    const nextProjects: Project[] = data.deployments.map((deployment) => {
      const project = projectById.get(deployment.projectId);
      const mock = seedProjects.find(
        (item) => item.id === deployment.id || item.name === project?.name,
      );
      return {
        id: deployment.id,
        serverId: deployment.serverId,
        name: project?.name ?? deployment.projectId,
        title: project?.title ?? deployment.projectId,
        path: deployment.remotePath,
        domain: deployment.domain,
        port: deployment.nginxPort,
        branch: deployment.branch,
        status: deployment.status as ProjectStatus,
        ssl: deployment.ssl,
        lastDeploy: formatDbTime(deployment.lastDeploy),
        backups: deployment.backups,
        phpVersion: deployment.phpVersion,
        containers: mock?.containers ?? [],
      };
    });

    setServers(data.servers);
    setProjectList(nextProjects);

    const preferredServer =
      data.servers.find((item) => item.id === serverId) ?? data.servers[0];
    if (!preferredServer) return;
    const preferredProjects = nextProjects.filter(
      (project) => project.serverId === preferredServer.id,
    );
    const preferredProject =
      preferredProjects.find((project) => project.id === activeId) ?? preferredProjects[0];

    setServerId(preferredServer.id);
    setOpenIds((current) => {
      const stillOpen = current.filter((id) =>
        nextProjects.some((project) => project.id === id),
      );
      return stillOpen.length > 0
        ? stillOpen
        : preferredProjects.slice(0, 2).map((project) => project.id);
    });
    setActiveId(preferredProject?.id ?? "");
  };

  useEffect(() => {
    const block = (event: MouseEvent) => event.preventDefault();
    document.addEventListener("contextmenu", block);
    return () => document.removeEventListener("contextmenu", block);
  }, []);

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 3200);
    return () => window.clearTimeout(timer);
  }, [toast]);

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;

    void loadWorkspace()
      .then((data) => {
        if (!cancelled) applyWorkspace(data);
      })
      .catch((error: unknown) => {
        console.error("加载 SQLite 工作区失败，继续使用 mock 数据", error);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setShowPalette((open) => !open);
      }
      if (event.key === "Escape") {
        setShowPalette(false);
        setProjectDialog(null);
        setShowSettings(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const appendLine = (projectId: string, type: TerminalLineType, text: string) => {
    setTerminals((current) => ({
      ...current,
      [projectId]: [
        ...(current[projectId] ?? []),
        {
          id: `${Date.now()}-${Math.random().toString(16).slice(2)}`,
          time: now(),
          type,
          text,
        },
      ],
    }));
  };

  const executeCommandLine = async (
    command: string,
    projectId: string,
  ): Promise<string | null> => {
    const trimmed = command.trim();
    if (!trimmed) return null;

    let tokens: string[];
    try {
      tokens = parseCommandLine(trimmed);
    } catch (error) {
      appendLine(projectId, "command", `$ ${trimmed}`);
      appendLine(projectId, "error", error instanceof Error ? error.message : String(error));
      return null;
    }
    if (tokens.length === 0) return null;

    appendLine(projectId, "command", `$ ${trimmed}`);
    setToast({ id: Date.now(), message: `执行 ${tokens[0]} …` });
    try {
      return await runCommand(projectId, tokens[0], tokens.slice(1));
    } catch (error) {
      appendLine(projectId, "error", `启动失败：${String(error)}`);
      setToast({ id: Date.now(), message: "命令启动失败" });
      return null;
    }
  };

  useEffect(() => {
    if (!canUseTauri()) return;
    let disposed = false;
    let unlisten: Array<() => void> = [];

    void Promise.all([
      listen<RunOutput>("run-output", (event) => {
        const payload = event.payload;
        appendLine(
          payload.deploymentId,
          payload.stream === "stderr" ? "error" : "info",
          payload.line,
        );
      }),
      listen<RunFinished>("run-finished", (event) => {
        const payload = event.payload;
        setRunsVersion((value) => value + 1);
        if (payload.status === "succeeded") {
          appendLine(payload.deploymentId, "success", "✔ 任务完成（exit 0）");
        } else if (payload.status === "canceled") {
          appendLine(payload.deploymentId, "error", "■ 任务已取消");
        } else {
          appendLine(
            payload.deploymentId,
            "error",
            `✘ 任务失败（exit ${payload.exitCode ?? "unknown"}）`,
          );
        }
      }),
      listen<DeployStepEvent>("deploy-step", (event) => {
        const payload = event.payload;
        appendLine(
          payload.deploymentId,
          "command",
          `▶ [${payload.index + 1}/${payload.total}] ${payload.stepTitle}`,
        );
      }),
      listen<DeployFinished>("deploy-finished", (event) => {
        const payload = event.payload;
        if (payload.status === "succeeded") {
          appendLine(payload.deploymentId, "success", "✔ 部署完成");
          setToast({ id: Date.now(), message: "部署完成" });
        } else {
          appendLine(
            payload.deploymentId,
            "error",
            `✘ 部署${payload.status === "canceled" ? "已取消" : "失败"}${
              payload.error ? `：${payload.error}` : ""
            }`,
          );
          setToast({ id: Date.now(), message: "部署未成功" });
        }
      }),
    ]).then((listeners) => {
      if (disposed) {
        listeners.forEach((stop) => stop());
      } else {
        unlisten = listeners;
      }
    });

    return () => {
      disposed = true;
      unlisten.forEach((stop) => stop());
    };
  }, []);

  const runAction = (command: string, label: string) => {
    if (!activeProject) return;
    const projectId = activeProject.id;
    appendLine(projectId, "command", `$ ${command}`);
    if (command.startsWith("phadmin:")) {
      appendLine(projectId, "info", `→ ${label}：原生动作，浏览器预览模式不执行`);
      setToast({ id: Date.now(), message: `${label}：浏览器预览不执行` });
      return;
    }
    appendLine(projectId, "info", `→ ${label}：UI 预览，模拟执行`);
    setToast({ id: Date.now(), message: `${label}已开始（模拟）` });
    window.setTimeout(() => {
      appendLine(projectId, "success", `✔ ${label}完成（模拟，未连接服务器）`);
    }, 850);
  };

  const handleSaveServer = async (input: ServerInput) => {
    if (!canUseTauri()) {
      setToast({ id: Date.now(), message: "浏览器预览不支持保存服务器" });
      throw new Error("浏览器预览模式不写 SQLite");
    }
    const editingId = serverDialog?.serverId;
    const saved = editingId
      ? await updateServer(editingId, input)
      : await createServer(input);
    applyWorkspace(await loadWorkspace());
    setServerId(saved.id);
    setServerDialog(null);
    setToast({
      id: Date.now(),
      message: editingId ? `已更新服务器 ${saved.name}` : `已添加服务器 ${saved.name}`,
    });
  };

  const handleDeleteServer = async () => {
    const editingId = serverDialog?.serverId;
    if (!editingId) return;
    if (!canUseTauri()) {
      throw new Error("浏览器预览模式不写 SQLite");
    }
    await deleteServer(editingId);
    applyWorkspace(await loadWorkspace());
    setServerDialog(null);
    setToast({ id: Date.now(), message: "服务器已删除" });
  };

  const handleSaveProject = async (
    input: ProjectCreateInput,
    deploymentId?: string,
  ) => {
    if (!canUseTauri()) {
      throw new Error("浏览器预览模式不写 SQLite");
    }
    const targetId = deploymentId ?? (await createProject(input));
    if (deploymentId) {
      await updateProject(deploymentId, input);
    }
    applyWorkspace(await loadWorkspace());
    setProjectDialog(null);
    setOpenIds((current) =>
      current.includes(targetId) ? current : [...current, targetId],
    );
    setActiveId(targetId);
    setSection("overview");
    setToast({
      id: Date.now(),
      message: deploymentId ? `已更新项目 ${input.name}` : `已创建项目 ${input.name}`,
    });
  };

  const handleOpenDataDir = async () => {
    if (!canUseTauri()) {
      setToast({ id: Date.now(), message: "浏览器预览无法打开本地目录" });
      return;
    }
    try {
      const directory = await openAppDataDir();
      setToast({ id: Date.now(), message: `已打开 ${directory}` });
    } catch (error) {
      setToast({ id: Date.now(), message: `打开目录失败：${String(error)}` });
    }
  };

  const handleOpen = (id: string) => {
    const project = projectList.find((item) => item.id === id);
    if (project) setServerId(project.serverId);
    setOpenIds((current) => (current.includes(id) ? current : [...current, id]));
    setActiveId(id);
    setSection("overview");
  };

  const handleClose = (id: string) => {
    const index = openIds.indexOf(id);
    const next = openIds.filter((item) => item !== id);
    if (id === activeId) {
      setActiveId(next[Math.min(index, next.length - 1)] ?? "");
    }
    setOpenIds(next);
  };

  const handleCommand = (command: string) => {
    if (!activeProject) return;
    const projectId = activeProject.id;
    const trimmed = command.trim();
    if (!trimmed) return;

    if (!canUseTauri()) {
      appendLine(projectId, "command", `$ ${trimmed}`);
      appendLine(projectId, "info", "→ 浏览器预览模式，不执行真实命令");
      setToast({ id: Date.now(), message: "浏览器预览：命令未执行" });
      return;
    }

    void executeCommandLine(trimmed, projectId);
  };

  const handleExecuteCommand = (command: string) => {
    if (!activeProject) return;
    if (command.startsWith("phadmin:")) {
      if (!canUseTauri()) {
        runAction(command, "原生操作");
        return;
      }
      void handleNativeAction(command);
      return;
    }
    if (!canUseTauri()) {
      runAction(command, "命令");
      return;
    }
    handleCommand(command);
  };

  /// 原生动作统一入口：组件只需传 `phadmin:<action>?k=v`，这里映射到 Rust command。
  const handleNativeAction = async (command: string) => {
    if (!activeProject) return;
    const projectId = activeProject.id;
    const [action, query = ""] = command.slice("phadmin:".length).split("?");
    const params = new URLSearchParams(query);

    const appendOutput = (text: unknown) => {
      if (typeof text !== "string" || text.trim() === "") return;
      text
        .split(/\r?\n/)
        .filter((line) => line.trim() !== "")
        .forEach((line) => appendLine(projectId, "info", line));
    };

    try {
      switch (action) {
        case "db-backup": {
          appendLine(projectId, "command", "$ mysqldump");
          const result = await backupDatabase(projectId);
          appendLine(
            projectId,
            "success",
            `数据库备份完成：${result.path}（${result.bytes} bytes）`,
          );
          setToast({ id: Date.now(), message: "数据库备份完成" });
          return;
        }
        case "db-migrate":
          appendLine(
            projectId,
            "command",
            "$ docker compose exec php php artisan migration r",
          );
          appendOutput(await migrateDatabase(projectId));
          break;
        case "db-seed":
          appendLine(
            projectId,
            "command",
            "$ docker compose exec php php artisan db:seed --dir=/var/www/db",
          );
          appendOutput(await seedDatabase(projectId));
          break;
        case "db-seed-dir": {
          const directory = await openSeedDirectory(projectId);
          setToast({ id: Date.now(), message: `已打开 seed 目录：${directory}` });
          return;
        }
        case "db-cron-install": {
          const time = params.get("time") ?? "03:00";
          const keep = Number(params.get("keep") ?? 7);
          const result = await installBackupCron(projectId, time, keep);
          appendLine(
            projectId,
            "success",
            `定时备份已安装：每天 ${result.time}，保留 ${result.keepDays} 天 → ${result.backupDir}`,
          );
          appendOutput(result.transcript);
          break;
        }
        case "db-cron-remove":
          appendOutput(await removeBackupCron(projectId));
          break;
        case "db-remote-backups":
          appendOutput(await listRemoteBackups(projectId));
          break;
        case "nginx-publish":
          appendLine(projectId, "command", "$ nginx -t && nginx -s reload");
          appendOutput(await publishNginxConfig(projectId));
          break;
        case "nginx-remove":
          appendOutput(await removeNginxConfig(projectId));
          break;
        case "nginx-reload":
          appendOutput(await reloadHostNginx(projectId));
          break;
        case "nginx-log-access":
          appendOutput(await readNginxLog(projectId, "access"));
          break;
        case "nginx-log-error":
          appendOutput(await readNginxLog(projectId, "error"));
          break;
        case "server-check":
          appendOutput(await checkServer(projectId));
          break;
        case "git-key": {
          const key = await generateGitDeployKey(projectId);
          appendLine(projectId, "success", `部署公钥：\n${key}`);
          break;
        }
        case "ssl-enable": {
          const email = params.get("email") ?? "";
          const result = await enableSsl(projectId, email);
          appendLine(
            projectId,
            "success",
            `SSL 已启用：${result.primaryDomain}（${result.certificate}）`,
          );
          appendOutput(result.transcript);
          break;
        }
        case "reset": {
          if (!window.confirm("重置会把主仓库和 git/bundle 同步目录的已跟踪文件恢复到最新提交，继续？")) {
            return;
          }
          appendOutput(await resetProject(projectId));
          break;
        }
        case "tunnel-start": {
          const tunnel = await startSshTunnel(projectId, Number(params.get("port") ?? 13306));
          appendLine(
            projectId,
            "success",
            `SSH 隧道已开启：127.0.0.1:${tunnel.localPort} → ${tunnel.host} ${tunnel.remote}`,
          );
          setToast({ id: Date.now(), message: `SSH 隧道已开启（本地 ${tunnel.localPort}）` });
          return;
        }
        case "tunnel-stop":
          appendOutput(await stopSshTunnel(projectId));
          break;
        case "pma-start": {
          const pma = await startPhpMyAdmin(projectId, Number(params.get("port") ?? 13307));
          appendLine(projectId, "info", "━━━━━ phpMyAdmin 访问地址 ━━━━━");
          appendLine(projectId, "info", `  ℹ   ${pma.url} —— 可点击`);
          appendLine(projectId, "info", `  ℹ   用户名: ${pma.username}`);
          appendLine(projectId, "info", `  ℹ   密码:   ${pma.password || "（空）"}`);
          setToast({ id: Date.now(), message: "phpMyAdmin 已启动" });
          return;
        }
        case "pma-stop":
          appendOutput(await stopPhpMyAdmin(projectId));
          break;
        case "deploy":
          await handleStartDeploy(true);
          return;
        default:
          throw new Error(`未知的原生操作：${action}`);
      }
      setToast({ id: Date.now(), message: "操作完成" });
    } catch (error) {
      appendLine(projectId, "error", `操作失败：${String(error)}`);
      setToast({ id: Date.now(), message: "操作失败，详见终端" });
    }
  };

  const handleStartDeploy = async (restartPhp: boolean) => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("phadmin:deploy", "部署 / 升级");
      return;
    }

    const projectId = activeProject.id;
    const previousBackups = new Map<string, string>();
    const restorePreviousConfig = async () => {
      if (previousBackups.size === 0) return;
      appendLine(projectId, "info", "部署失败，开始恢复部署前配置");
      for (const [file, backup] of previousBackups) {
        try {
          await restoreConfigBackup(projectId, backup);
          appendLine(projectId, "success", `已恢复 ${file}（${backup}）`);
        } catch (error) {
          appendLine(projectId, "error", `恢复 ${file} 失败：${String(error)}`);
        }
      }
    };
    try {
      const backups = await listConfigBackups(projectId);
      for (const backup of backups) {
        if (!previousBackups.has(backup.file)) {
          previousBackups.set(backup.file, backup.name);
        }
      }
      appendLine(projectId, "info", "部署：先执行同步全部与配置上传");
      await handleSyncAllItems();
      await prepareRuntimeAssets(projectId);
      await writeGeneratedConfig(projectId);
      await applyGeneratedConfig(projectId, true);
      appendLine(projectId, "info", "部署：配置已上传，启动容器编排");

      let expectedRunId = "";
      let unlisten: (() => void) | undefined;
      const finished = new Promise<DeployFinished>((resolve) => {
        void listen<DeployFinished>("deploy-finished", (event) => {
          if (expectedRunId && event.payload.deployRunId === expectedRunId) {
            unlisten?.();
            resolve(event.payload);
          }
        }).then((dispose) => {
          unlisten = dispose;
        });
      });

      let result;
      try {
        result = await startDeploy(projectId, restartPhp);
        expectedRunId = result.deployRunId;
      } catch (error) {
        unlisten?.();
        throw error;
      }
      appendLine(
        projectId,
        "info",
        `部署 ${result.deployRunId} 已启动（${result.steps.length} 步）`,
      );
      setToast({ id: Date.now(), message: "部署已启动" });

      const outcome = await finished;
      if (outcome.status !== "succeeded") {
        await restorePreviousConfig();
      }
    } catch (error) {
      await restorePreviousConfig();
      appendLine(projectId, "error", `部署启动失败：${String(error)}`);
      setToast({ id: Date.now(), message: "部署启动失败" });
    }

    try {
      applyWorkspace(await loadWorkspace());
    } catch (error) {
      console.error("部署后刷新工作区失败", error);
    }
  };

  const handleSyncItem = async (itemId: string): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("git pull", "同步代码");
      return;
    }

    const projectId = activeProject.id;
    try {
      const runId = await syncGitItem(projectId, itemId);
      appendLine(projectId, "info", `同步任务 ${runId} 已启动`);
      setToast({ id: Date.now(), message: "代码同步已启动" });
    } catch (error) {
      appendLine(projectId, "error", `同步启动失败：${String(error)}`);
      setToast({ id: Date.now(), message: "同步启动失败" });
    }
  };

  const handleSyncBundleItem = async (itemId: string): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("git bundle", "同步代码");
      return;
    }

    const projectId = activeProject.id;
    try {
      const output = await syncBundleItem(projectId, itemId);
      output
        .split(/\r?\n/)
        .filter((line) => line.trim() !== "")
        .forEach((line) => appendLine(projectId, "info", line));
      setRunsVersion((value) => value + 1);
      setToast({ id: Date.now(), message: "bundle 同步完成" });
    } catch (error) {
      appendLine(projectId, "error", `bundle 同步失败：${String(error)}`);
      setRunsVersion((value) => value + 1);
      setToast({ id: Date.now(), message: "bundle 同步失败" });
    }
  };

  const handleSyncFtpItem = async (itemId: string): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("scp -r", "同步代码");
      return;
    }

    const projectId = activeProject.id;
    try {
      const output = await syncFtpItem(projectId, itemId);
      output
        .split(/\r?\n/)
        .filter((line) => line.trim() !== "")
        .forEach((line) => appendLine(projectId, "info", line));
      setRunsVersion((value) => value + 1);
      setToast({ id: Date.now(), message: "ftp 同步完成" });
    } catch (error) {
      appendLine(projectId, "error", `ftp 同步失败：${String(error)}`);
      setRunsVersion((value) => value + 1);
      setToast({ id: Date.now(), message: "ftp 同步失败" });
    }
  };

  const handleSyncAllItems = async (): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("sync all", "同步全部");
      return;
    }
    const projectId = activeProject.id;
    try {
      const config = await loadDeploymentConfig(projectId);
      for (const item of config.syncItems) {
        if (item.method === "git") {
          const runId = await syncGitItem(projectId, item.id);
          appendLine(projectId, "info", `启动同步 ${item.name || item.id}: ${runId}`);
          let finished = false;
          for (let attempt = 0; attempt < 240; attempt += 1) {
            const runs = await listRuns(projectId, 20);
            const run = runs.find((entry) => entry.id === runId);
            if (run?.status === "failed" || run?.status === "canceled") {
              throw new Error(`同步项 ${item.name || item.id} 执行失败`);
            }
            if (run?.status === "succeeded") {
              finished = true;
              break;
            }
            await new Promise((resolve) => window.setTimeout(resolve, 500));
          }
          if (!finished) {
            throw new Error(`同步项 ${item.name || item.id} 等待超时`);
          }
        } else if (item.method === "bundle" || item.method === "ftp") {
          const output =
            item.method === "bundle"
              ? await syncBundleItem(projectId, item.id)
              : await syncFtpItem(projectId, item.id);
          output
            .split(/\r?\n/)
            .filter((line) => line.trim() !== "")
            .forEach((line) => appendLine(projectId, "info", line));
        }
        setRunsVersion((value) => value + 1);
      }
      setToast({ id: Date.now(), message: "全部同步完成" });
    } catch (error) {
      appendLine(projectId, "error", `全部同步失败：${String(error)}`);
      setRunsVersion((value) => value + 1);
      setToast({ id: Date.now(), message: "全部同步失败" });
      throw error;
    }
  };

  const handleNginxReload = async (): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("phadmin:nginx-reload", "重载 Nginx");
      return;
    }
    try {
      const output = await reloadHostNginx(activeProject.id);
      output
        .split(/\r?\n/)
        .filter((line) => line.trim() !== "")
        .forEach((line) => appendLine(activeProject.id, "info", line));
      setToast({ id: Date.now(), message: "宿主机 Nginx 已重载" });
    } catch (error) {
      appendLine(activeProject.id, "error", `Nginx reload 失败：${String(error)}`);
      setToast({ id: Date.now(), message: "Nginx reload 失败" });
    }
  };

  const handleNginxAccessLog = async (): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("phadmin:nginx-log-access", "查看访问日志");
      return;
    }
    try {
      const output = await readNginxLog(activeProject.id, "access");
      output
        .split(/\r?\n/)
        .filter((line) => line.trim() !== "")
        .forEach((line) => appendLine(activeProject.id, "info", line));
      setToast({ id: Date.now(), message: "已读取 Nginx 访问日志" });
    } catch (error) {
      appendLine(activeProject.id, "error", `访问日志失败：${String(error)}`);
    }
  };

  const handleNativeDbBackup = async (): Promise<void> => {
    if (!activeProject) return;
    if (!canUseTauri()) {
      runAction("mysqldump", "备份数据库");
      return;
    }
    try {
      const result = await backupDatabase(activeProject.id);
      appendLine(
        activeProject.id,
        "success",
        `数据库备份完成：${result.path}（${result.bytes} bytes）`,
      );
      setToast({ id: Date.now(), message: "数据库备份完成" });
    } catch (error) {
      appendLine(activeProject.id, "error", `数据库备份失败：${String(error)}`);
      setToast({ id: Date.now(), message: "数据库备份失败" });
    }
  };

  const handleListNativeBackups = async () => {
    if (!activeProject) return [];
    if (!canUseTauri()) return [];
    try {
      return await listDatabaseBackups(activeProject.id);
    } catch (error) {
      appendLine(activeProject.id, "error", `读取备份列表失败：${String(error)}`);
      return [];
    }
  };

  const handleRestoreNativeBackup = async (name: string): Promise<void> => {
    if (!activeProject) return;
    if (!window.confirm(`用 ${name} 覆盖当前数据库？`)) return;
    try {
      const result = await restoreDatabaseBackup(activeProject.id, name);
      appendLine(
        activeProject.id,
        "success",
        `数据库已从 ${name} 恢复（${result.bytes} bytes）；恢复前快照：${result.snapshotPath}`,
      );
      setToast({ id: Date.now(), message: "数据库恢复完成" });
    } catch (error) {
      appendLine(activeProject.id, "error", `数据库恢复失败：${String(error)}`);
      setToast({ id: Date.now(), message: "数据库恢复失败" });
    }
  };

  const handleDownloadNativeBackup = async (name: string): Promise<void> => {
    if (!activeProject) return;
    try {
      const path = await downloadDatabaseBackup(activeProject.id, name);
      appendLine(activeProject.id, "success", `备份已导出：${path}`);
      setToast({ id: Date.now(), message: "备份已下载到下载目录" });
    } catch (error) {
      appendLine(activeProject.id, "error", `导出备份失败：${String(error)}`);
      setToast({ id: Date.now(), message: "导出备份失败" });
    }
  };

  const handleTestConnection = () => {
    if (!activeProject) return;
    const projectId = activeProject.id;
    appendLine(projectId, "command", "$ docker compose ps");

    if (!canUseTauri()) {
      appendLine(projectId, "info", "→ 浏览器预览模式，不执行真实命令");
      setToast({ id: Date.now(), message: "浏览器预览：模拟执行" });
      return;
    }

    setToast({ id: Date.now(), message: "正在连接服务器并读取容器状态…" });
    void runCommand(projectId, "docker", ["compose", "ps"]).catch((error: unknown) => {
      appendLine(projectId, "error", `连接失败：${String(error)}`);
      setToast({ id: Date.now(), message: "连接失败，请检查 SSH / Docker" });
    });
  };

  const handlePaletteRun = (command: string, label: string) => {
    if (!canUseTauri()) {
      runAction(command, label);
      return;
    }
    if (command.startsWith("phadmin:")) {
      void handleNativeAction(command);
      return;
    }
    handleCommand(command);
  };

  const paletteCommands: PaletteCommand[] = activeProject
    ? [
        {
          label: `部署 ${activeProject.name}`,
          description: "同步全部 → 上传配置 → compose up → 健康检查",
          command: "phadmin:deploy",
        },
        {
          label: "重启 PHP 容器",
          description: "docker compose restart php",
          command: "docker compose restart php",
        },
        {
          label: "容器状态",
          description: "查看 docker compose ps",
          command: "docker compose ps",
        },
        {
          label: "备份数据库",
          description: "mysqldump 到本地备份目录",
          command: "phadmin:db-backup",
        },
        {
          label: "发布 Nginx 配置",
          description: "生成并上传到宿主机 /etc/nginx/conf.d",
          command: "phadmin:nginx-publish",
        },
        {
          label: "重载 Nginx",
          description: "语法检查并 reload",
          command: "phadmin:nginx-reload",
        },
        {
          label: "服务器环境检查",
          description: "docker / nginx / certbot / 端口",
          command: "phadmin:server-check",
        },
      ]
    : [
        {
          label: "服务器环境检查",
          description: "检查服务器依赖与端口",
          command: "phadmin:server-check",
        },
      ];

  return (
    <div className="app">
      <TopBar
        servers={servers.filter((item) => item.kind !== "filesystem")}
        activeServerId={activeProject?.serverId ?? serverId}
        onEditServer={(serverId) => setServerDialog({ serverId })}
        onAddServer={() => setServerDialog({})}
        onOpenTemplates={() => setShowTemplates(true)}
        onOpenSettings={() => setShowSettings(true)}
      />

      <div className="app-body">
        <Sidebar
          projects={projectList}
          serverNames={Object.fromEntries(servers.map((server) => [server.id, server.name]))}
          activeId={activeId}
          openIds={openIds}
          search={search}
          onSearch={setSearch}
          onOpen={handleOpen}
          onAdd={() => setProjectDialog({})}
          onEditProject={(id) => setProjectDialog({ deploymentId: id })}
        />

        <main className="main-column">
          <ProjectTabs
            projects={projectList}
            openIds={openIds}
            activeId={activeId}
            onSelect={handleOpen}
            onClose={handleClose}
            onAdd={() => setProjectDialog({})}
          />

          <Workspace
            project={activeProject}
            server={server}
            section={section}
            activities={activities}
            onSectionChange={setSection}
            onAction={handleExecuteCommand}
            onNotify={(message) => setToast({ id: Date.now(), message })}
            onTestConnection={handleTestConnection}
            onOpenRuns={() => setShowRuns(true)}
            onExecuteCommand={handleExecuteCommand}
            onStartDeploy={handleStartDeploy}
            onSyncItem={handleSyncItem}
            onSyncBundleItem={handleSyncBundleItem}
            onSyncFtpItem={handleSyncFtpItem}
            onSyncAll={handleSyncAllItems}
            onAccessLog={handleNginxAccessLog}
            onReloadNginx={handleNginxReload}
            onNativeDbBackup={handleNativeDbBackup}
            onListNativeBackups={handleListNativeBackups}
            onRestoreNativeBackup={handleRestoreNativeBackup}
            onDownloadNativeBackup={handleDownloadNativeBackup}
            refreshVersion={runsVersion}
          />

          {activeProject && (
            <TerminalPanel
              project={activeProject}
              lines={activeLines}
              collapsed={terminalCollapsed}
              onToggle={() => setTerminalCollapsed((collapsed) => !collapsed)}
              onClear={() =>
                setTerminals((current) => ({ ...current, [activeProject.id]: [] }))
              }
              onCommand={handleCommand}
              onOpenRuns={() => setShowRuns(true)}
            />
          )}
        </main>
      </div>

      {projectDialog && (
        <AddProjectModal
          servers={servers}
          serverId={serverId}
          existingNames={projectList.map((project) => project.name)}
          deploymentId={projectDialog.deploymentId}
          onClose={() => setProjectDialog(null)}
          onSubmit={handleSaveProject}
        />
      )}

      {serverDialog && (
        <ServerModal
          serverId={serverDialog.serverId}
          serverName={servers.find((item) => item.id === serverDialog.serverId)?.name}
          onClose={() => setServerDialog(null)}
          onSave={handleSaveServer}
          onDelete={serverDialog.serverId ? handleDeleteServer : undefined}
        />
      )}

      {showTemplates && (
        <TemplateManager onClose={() => setShowTemplates(false)} />
      )}

      <CommandPalette
        open={showPalette}
        commands={paletteCommands}
        onClose={() => setShowPalette(false)}
        onRun={handlePaletteRun}
      />

      {showRuns && activeProject && (
        <RunsModal
          project={activeProject}
          version={runsVersion}
          onClose={() => setShowRuns(false)}
          onNotify={(message) => setToast({ id: Date.now(), message })}
        />
      )}

      {showSettings && (
        <SettingsModal
          onClose={() => setShowSettings(false)}
          onOpenDataDir={() => void handleOpenDataDir()}
        />
      )}

      {toast && (
        <div className="toast" key={toast.id}>
          <span className="toast-icon">
            <Icon name="check" size={13} />
          </span>
          {toast.message}
        </div>
      )}
    </div>
  );
}

export default App;
