import { useEffect, useState } from "react";
import type { Project, ProjectStatus } from "../data/mock";
import { Icon } from "./Icon";

interface SidebarProps {
  projects: Project[];
  serverNames: Record<string, string>;
  activeId: string;
  openIds: string[];
  search: string;
  onSearch: (value: string) => void;
  onOpen: (id: string) => void;
  onAdd: () => void;
  onEditProject: (id: string) => void;
}

const statusLabel: Record<ProjectStatus, string> = {
  running: "运行中",
  stopped: "已停止",
  warning: "异常",
  "not-deployed": "未部署",
};

export function Sidebar({
  projects,
  serverNames,
  activeId,
  openIds,
  search,
  onSearch,
  onOpen,
  onAdd,
  onEditProject,
}: SidebarProps) {
  const [menu, setMenu] = useState<{ x: number; y: number; id: string } | null>(null);
  const keyword = search.trim().toLowerCase();
  const filtered = projects.filter(
    (project) =>
      keyword === "" ||
      project.name.toLowerCase().includes(keyword) ||
      project.title.toLowerCase().includes(keyword) ||
      project.domain.toLowerCase().includes(keyword),
  );

  useEffect(() => {
    if (!menu) return;
    const close = () => setMenu(null);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenu(null);
    };
    window.addEventListener("click", close);
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [menu]);

  return (
    <aside className="sidebar">
      <div className="sidebar-heading">
        <div>
          <span className="section-kicker">Workspace</span>
          <h2>项目</h2>
        </div>
        <span className="count-badge">{projects.length}</span>
      </div>

      <label className="search-box">
        <Icon name="search" size={14} />
        <input
          value={search}
          onChange={(event) => onSearch(event.target.value)}
          placeholder="搜索项目 / 域名"
          aria-label="搜索项目"
        />
      </label>

      <nav className="project-list" aria-label="项目列表">
        {filtered.map((project) => {
          const isActive = project.id === activeId;
          const isOpen = openIds.includes(project.id);
          return (
            <button
              key={project.id}
              type="button"
              className={`project-row ${isActive ? "active" : ""}`}
              onClick={() => onOpen(project.id)}
              onContextMenu={(event) => {
                event.preventDefault();
                event.stopPropagation();
                setMenu({ x: event.clientX, y: event.clientY, id: project.id });
              }}
            >
              <span className={`project-status ${project.status}`} />
              <span className="project-row-copy">
                <span className="project-row-name">
                  {project.name}
                  {isOpen && <span className="open-indicator" />}
                </span>
                <span className="project-row-meta">
                  {serverNames[project.serverId] ?? "未分配服务器"} ·{" "}
                  {statusLabel[project.status]}
                </span>
              </span>
              <Icon name="chevron-right" size={13} className="project-row-arrow" />
            </button>
          );
        })}

        {projects.length === 0 && (
          <div className="sidebar-empty">
            <Icon name="folder" size={18} />
            <span>还没有项目，点击下方「添加项目」</span>
          </div>
        )}
        {projects.length > 0 && filtered.length === 0 && (
          <div className="sidebar-empty">
            <Icon name="search" size={18} />
            <span>没有匹配的项目</span>
          </div>
        )}
      </nav>

      <button className="add-project-button" type="button" onClick={onAdd}>
        <Icon name="plus" size={15} />
        添加项目
      </button>

      <div className="sidebar-footer">
        <span className="status-dot online" />
        <span>本地数据目录已连接</span>
        <span className="sidebar-version">v0.1.0</span>
      </div>

      {menu && (
        <div
          className="context-menu"
          style={{ left: menu.x, top: menu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <button
            type="button"
            className="context-menu-item"
            onClick={() => {
              onEditProject(menu.id);
              setMenu(null);
            }}
          >
            <Icon name="settings" size={13} />
            修改项目
          </button>
        </div>
      )}
    </aside>
  );
}
