import { useEffect, useState } from "react";
import type { Activity, Backup, Project, Server } from "../data/mock";
import {
  ContainersPanel,
  DatabasePanel,
  DeployPanel,
  LogsPanel,
  NginxPanel,
  OverviewPanel,
  ProjectStatusBadge,
  type WorkspaceSection,
} from "./Panels";
import { Icon } from "./Icon";

interface WorkspaceProps {
  project: Project | null;
  server: Server;
  section: WorkspaceSection;
  activities: Activity[];
  backups: Backup[];
  onSectionChange: (section: WorkspaceSection) => void;
  onAction: (command: string, label: string) => void;
  onNotify: (message: string) => void;
  onTestConnection: () => void;
  onOpenRuns: () => void;
  onExecuteCommand: (command: string) => void;
  onStartDeploy: (restartPhp: boolean) => void;
  onSyncItem: (itemId: string) => Promise<void>;
  onSyncBundleItem: (itemId: string) => Promise<void>;
  refreshVersion: number;
}

const sections: { id: WorkspaceSection; label: string; icon: "activity" | "rocket" | "box" | "globe" | "database" | "file-text" }[] = [
  { id: "overview", label: "概览", icon: "activity" },
  { id: "deploy", label: "部署与升级", icon: "rocket" },
  { id: "containers", label: "容器", icon: "box" },
  { id: "nginx", label: "Nginx", icon: "globe" },
  { id: "database", label: "数据库", icon: "database" },
  { id: "logs", label: "日志", icon: "file-text" },
];

export function Workspace({
  project,
  server,
  section,
  activities,
  backups,
  onSectionChange,
  onAction,
  onNotify,
  onTestConnection,
  onOpenRuns,
  onExecuteCommand,
  onStartDeploy,
  onSyncItem,
  onSyncBundleItem,
  refreshVersion,
}: WorkspaceProps) {
  const [moreOpen, setMoreOpen] = useState(
    () => new URLSearchParams(window.location.search).get("menu") === "more",
  );

  useEffect(() => {
    if (!moreOpen) return;
    const close = () => setMoreOpen(false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [moreOpen]);

  if (!project) {
    return (
      <section className="workspace">
        <div className="workspace-empty">
          <span className="empty-mark">
            <Icon name="layers" size={26} />
          </span>
          <h2>选择一个项目</h2>
          <p>从左侧项目列表打开一个项目，或添加新的部署。</p>
        </div>
      </section>
    );
  }

  return (
    <section className="workspace">
      <div className="workspace-header">
        <div className="workspace-title">
          <div className="breadcrumb">
            <span>项目</span>
            <Icon name="chevron-right" size={11} />
            <span>{project.name}</span>
          </div>
          <div className="title-row">
            <h1>{project.title}</h1>
            <ProjectStatusBadge status={project.status} />
            <span className="branch-chip">
              <Icon name="git-branch" size={12} />
              {project.branch}
            </span>
          </div>
          <div className="project-meta">
            <span>
              <Icon name="globe" size={12} />
              {project.domain}
            </span>
            <span className="mono">
              <Icon name="folder" size={12} />
              {project.path}
            </span>
            <span className="mono">:{project.port}</span>
          </div>
        </div>

        <div className="workspace-actions">
          <button
            type="button"
            className="button secondary"
            title="通过 SSH / 本地执行 docker compose ps"
            onClick={onTestConnection}
          >
            <Icon name="box" size={14} />
            测试连接
          </button>
          <button
            type="button"
            className="button primary"
            onClick={() => onSectionChange("deploy")}
          >
            <Icon name="rocket" size={14} />
            部署 / 升级
          </button>
          <div className="more-wrap">
            <button
              type="button"
              className="icon-button subtle"
              title="更多操作"
              aria-label="更多操作"
              aria-expanded={moreOpen}
              onClick={(event) => {
                event.stopPropagation();
                setMoreOpen((open) => !open);
              }}
            >
              <Icon name="more" size={16} />
            </button>
            {moreOpen && (
              <div className="more-menu" onClick={(event) => event.stopPropagation()}>
                <button
                  type="button"
                  onClick={() => {
                    onSectionChange("nginx");
                    setMoreOpen(false);
                  }}
                >
                  <Icon name="globe" size={13} />
                  Nginx 配置
                </button>
                <button
                  type="button"
                  onClick={() => {
                    onAction(`php admin app:${project.name} dc:log`, "查看容器日志");
                    setMoreOpen(false);
                  }}
                >
                  <Icon name="terminal" size={13} />
                  容器日志
                </button>
                <button
                  type="button"
                  onClick={() => {
                    void navigator.clipboard?.writeText(project.name);
                    onNotify(`已复制项目名：${project.name}`);
                    setMoreOpen(false);
                  }}
                >
                  <Icon name="copy" size={13} />
                  复制项目名
                </button>
                <div className="more-divider" />
                <button
                  type="button"
                  className="danger"
                  onClick={() => {
                    onAction(`php admin app:${project.name} reset`, "重置代码");
                    setMoreOpen(false);
                  }}
                >
                  <Icon name="refresh" size={13} />
                  重置代码
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      <nav className="workspace-nav" aria-label="项目功能">
        {sections.map((item) => (
          <button
            key={item.id}
            type="button"
            className={section === item.id ? "active" : ""}
            onClick={() => onSectionChange(item.id)}
          >
            <Icon name={item.icon} size={14} />
            {item.label}
          </button>
        ))}
      </nav>

      <div className="workspace-body">
        {section === "overview" && (
          <OverviewPanel
            project={project}
            server={server}
            activities={activities}
            onAction={onAction}
            onSectionChange={onSectionChange}
            onOpenRuns={onOpenRuns}
            refreshVersion={refreshVersion}
          />
        )}
        {section === "deploy" && (
          <DeployPanel
            project={project}
            server={server}
            onAction={onAction}
            onStartDeploy={onStartDeploy}
            onSyncItem={onSyncItem}
            onSyncBundleItem={onSyncBundleItem}
          />
        )}
        {section === "containers" && (
          <ContainersPanel
            project={project}
            server={server}
            onAction={onAction}
            onExecuteCommand={onExecuteCommand}
            refreshVersion={refreshVersion}
          />
        )}
        {section === "nginx" && (
          <NginxPanel project={project} server={server} onAction={onAction} />
        )}
        {section === "database" && (
          <DatabasePanel
            project={project}
            server={server}
            backups={backups}
            onAction={onAction}
          />
        )}
        {section === "logs" && <LogsPanel project={project} />}
      </div>
    </section>
  );
}
