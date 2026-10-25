import { useEffect, useMemo, useState } from "react";
import "./App.css";
import { TopBar } from "./components/TopBar";
import { Sidebar } from "./components/Sidebar";
import { ProjectTabs } from "./components/ProjectTabs";
import { Workspace } from "./components/Workspace";
import { TerminalPanel } from "./components/TerminalPanel";
import {
  AddProjectModal,
  CommandPalette,
  SettingsModal,
  type PaletteCommand,
} from "./components/Dialogs";
import { Icon } from "./components/Icon";
import {
  activities,
  backups,
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
import {
  canUseTauri,
  importLegacyConfig,
  loadWorkspace,
  runCommand,
  type ImportSummary,
  type RunFinished,
  type RunOutput,
  type WorkspaceResponse,
} from "./api/workspace";
import type { WorkspaceSection } from "./components/Panels";

function now() {
  return new Date().toLocaleTimeString("zh-CN", { hour12: false });
}

const sectionIds: WorkspaceSection[] = [
  "overview",
  "deploy",
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
  const [servers, setServers] = useState<Server[]>(seedServers);
  const [projectList, setProjectList] = useState<Project[]>(seedProjects);
  const [openIds, setOpenIds] = useState<string[]>(boot.openIds);
  const [activeId, setActiveId] = useState(boot.project?.id ?? "");
  const [section, setSection] = useState<WorkspaceSection>(boot.section);
  const [search, setSearch] = useState("");
  const [terminals, setTerminals] = useState<Record<string, TerminalLine[]>>(initialTerminalLines);
  const [terminalCollapsed, setTerminalCollapsed] = useState(false);
  const [showAdd, setShowAdd] = useState(boot.dialog === "add");
  const [showPalette, setShowPalette] = useState(boot.dialog === "command");
  const [showSettings, setShowSettings] = useState(boot.dialog === "settings");
  const [toast, setToast] = useState<{ id: number; message: string } | null>(null);

  const server = servers.find((item) => item.id === serverId) ?? servers[0];
  const serverProjects = useMemo(
    () => projectList.filter((project) => project.serverId === serverId),
    [projectList, serverId],
  );
  const activeProject =
    serverProjects.find((project) => project.id === activeId) ?? null;
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
        lastDeploy: deployment.lastDeploy,
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
        setShowAdd(false);
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
    appendLine(projectId, "info", `→ ${label}：UI 预览，模拟执行`);
    setToast({ id: Date.now(), message: `${label}已开始（模拟）` });
    window.setTimeout(() => {
      appendLine(projectId, "success", `✔ ${label}完成（模拟，未连接服务器）`);
    }, 850);
  };

  const handleServerChange = (id: string) => {
    const nextProjects = projectList.filter((project) => project.serverId === id);
    setServerId(id);
    setSearch("");
    setOpenIds(nextProjects.length > 0 ? [nextProjects[0].id] : []);
    setActiveId(nextProjects[0]?.id ?? "");
    setSection("overview");
    setToast({
      id: Date.now(),
      message: `已切换到 ${servers.find((item) => item.id === id)?.name ?? id}`,
    });
  };

  const handleOpen = (id: string) => {
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
    appendLine(projectId, "command", `$ ${trimmed}`);

    if (canUseTauri() && trimmed === "docker compose ps") {
      setToast({ id: Date.now(), message: "正在读取容器状态…" });
      void runCommand(projectId, "docker", ["compose", "ps"]).catch((error: unknown) => {
        appendLine(projectId, "error", `启动失败：${String(error)}`);
        setToast({ id: Date.now(), message: "容器状态读取失败" });
      });
      return;
    }

    appendLine(projectId, "info", "→ 当前终端仅接入 docker compose ps；其它命令仍为 UI 模拟");
    setToast({ id: Date.now(), message: "命令已写入终端（模拟）" });
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

  const handleImportLegacy = async (json: string): Promise<ImportSummary> => {
    if (!canUseTauri()) {
      throw new Error("浏览器预览模式不支持导入，请在 Tauri 窗口中使用。");
    }
    const summary = await importLegacyConfig(json);
    applyWorkspace(await loadWorkspace());
    setToast({
      id: Date.now(),
      message: `legacy 导入完成：${summary.servers} 服务器 / ${summary.projects} 项目 / ${summary.deployments} 部署`,
    });
    return summary;
  };

  const paletteCommands: PaletteCommand[] = activeProject
    ? [
        {
          label: `升级 ${activeProject.name}`,
          description: "同步代码，不覆盖配置",
          command: `php admin app:${activeProject.name} upgrade`,
        },
        {
          label: `部署 ${activeProject.name}`,
          description: "首次部署 / 配置预览",
          command: `php admin app:${activeProject.name} init`,
        },
        {
          label: "重启 PHP 容器",
          description: "清空 opcache",
          command: `php admin app:${activeProject.name} dc:restart:php`,
        },
        {
          label: "容器状态",
          description: "查看 docker compose ps",
          command: `php admin app:${activeProject.name} dc:status`,
        },
        {
          label: "备份数据库",
          description: "立即生成服务器备份",
          command: `php admin app:${activeProject.name} db:backup`,
        },
        {
          label: "重载 Nginx",
          description: "语法检查并 reload",
          command: "php admin nginx:reload",
        },
      ]
    : [
        {
          label: "环境检查",
          description: "检查服务器依赖与端口",
          command: "php admin server:init",
        },
      ];

  return (
    <div className="app">
      <TopBar
        servers={servers}
        serverId={serverId}
        onServerChange={handleServerChange}
        onOpenCommand={() => setShowPalette(true)}
        onOpenSettings={() => setShowSettings(true)}
      />

      <div className="app-body">
        <Sidebar
          projects={serverProjects}
          activeId={activeId}
          openIds={openIds}
          search={search}
          onSearch={setSearch}
          onOpen={handleOpen}
          onAdd={() => setShowAdd(true)}
        />

        <main className="main-column">
          <ProjectTabs
            projects={projectList}
            openIds={openIds}
            activeId={activeId}
            onSelect={handleOpen}
            onClose={handleClose}
            onAdd={() => setShowAdd(true)}
          />

          <Workspace
            project={activeProject}
            server={server}
            section={section}
            activities={activities}
            backups={backups}
            onSectionChange={setSection}
            onAction={runAction}
            onNotify={(message) => setToast({ id: Date.now(), message })}
            onTestConnection={handleTestConnection}
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
            />
          )}
        </main>
      </div>

      {showAdd && (
        <AddProjectModal
          servers={servers}
          serverId={serverId}
          existingNames={projectList.map((project) => project.name)}
          onClose={() => setShowAdd(false)}
          onCreate={(project) => {
            setProjectList((current) => [...current, project]);
            setShowAdd(false);
            setServerId(project.serverId);
            setOpenIds([project.id]);
            setActiveId(project.id);
            setSection("overview");
            setToast({ id: Date.now(), message: `已创建 ${project.name}（内存记录）` });
          }}
        />
      )}

      <CommandPalette
        open={showPalette}
        commands={paletteCommands}
        onClose={() => setShowPalette(false)}
        onRun={runAction}
      />

      {showSettings && (
        <SettingsModal
          onClose={() => setShowSettings(false)}
          onImportLegacy={handleImportLegacy}
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
