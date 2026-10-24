import type { Project } from "../data/mock";
import { Icon } from "./Icon";

interface ProjectTabsProps {
  projects: Project[];
  openIds: string[];
  activeId: string;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onAdd: () => void;
}

export function ProjectTabs({
  projects,
  openIds,
  activeId,
  onSelect,
  onClose,
  onAdd,
}: ProjectTabsProps) {
  const tabs = openIds
    .map((id) => projects.find((project) => project.id === id))
    .filter((project): project is Project => Boolean(project));

  return (
    <div className="project-tabs" role="tablist" aria-label="打开的项目">
      {tabs.map((project) => (
        <div
          key={project.id}
          className={`project-tab ${project.id === activeId ? "active" : ""}`}
          role="tab"
          aria-selected={project.id === activeId}
          tabIndex={0}
          onClick={() => onSelect(project.id)}
          onKeyDown={(event) => {
            if (event.key === "Enter" || event.key === " ") {
              onSelect(project.id);
            }
          }}
        >
          <span className={`project-status ${project.status}`} />
          <span className="project-tab-name">{project.name}</span>
          <button
            type="button"
            className="project-tab-close"
            aria-label={`关闭 ${project.name}`}
            onClick={(event) => {
              event.stopPropagation();
              onClose(project.id);
            }}
          >
            <Icon name="close" size={12} />
          </button>
        </div>
      ))}

      <button type="button" className="tab-add" onClick={onAdd} aria-label="打开项目">
        <Icon name="plus" size={14} />
      </button>
    </div>
  );
}
