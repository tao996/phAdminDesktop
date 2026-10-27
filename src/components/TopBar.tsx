import { useEffect, useState } from "react";
import type { Server } from "../data/mock";
import { Icon } from "./Icon";

interface TopBarProps {
  servers: Server[];
  activeServerId: string;
  onEditServer: (serverId: string) => void;
  onAddServer: () => void;
  onOpenTemplates: () => void;
  onOpenSettings: () => void;
}

export function TopBar({
  servers,
  activeServerId,
  onEditServer,
  onAddServer,
  onOpenTemplates,
  onOpenSettings,
}: TopBarProps) {
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open) return;
    const close = () => setOpen(false);
    window.addEventListener("click", close);
    return () => window.removeEventListener("click", close);
  }, [open]);

  return (
    <header className="topbar">
      <div className="topbar-leading">
        <div className="server-menu-wrap" onClick={(event) => event.stopPropagation()}>
          <button
            className="button secondary small"
            type="button"
            aria-haspopup="menu"
            aria-expanded={open}
            onClick={() => setOpen((value) => !value)}
          >
            <Icon name="server" size={14} />
            服务器管理
            <Icon name="chevron-down" size={12} />
          </button>

          {open && (
            <div className="server-menu" role="menu">
              <div className="server-menu-heading">
                已添加服务器
                <span className="server-menu-count">{servers.length}</span>
              </div>

              {servers.length === 0 && (
                <div className="server-menu-empty">还没有服务器</div>
              )}

              {servers.map((server) => (
                <button
                  key={server.id}
                  type="button"
                  role="menuitem"
                  className={`server-menu-item ${server.id === activeServerId ? "active" : ""}`}
                  onClick={() => {
                    onEditServer(server.id);
                    setOpen(false);
                  }}
                >
                  <span className={`server-dot ${server.status}`} />
                  <span className="server-menu-copy">
                    <strong>{server.name}</strong>
                    <small>{server.host}</small>
                  </span>
                </button>
              ))}

              <div className="server-menu-divider" />

              <button
                type="button"
                role="menuitem"
                className="server-menu-item add"
                onClick={() => {
                  setOpen(false);
                  onAddServer();
                }}
              >
                <Icon name="plus" size={14} />
                <span className="server-menu-copy">
                  <strong>添加服务器</strong>
                </span>
              </button>
            </div>
          )}
        </div>

        <button
          className="button secondary small"
          type="button"
          onClick={onOpenTemplates}
        >
          <Icon name="layers" size={14} />
          模板管理
        </button>
      </div>

      <div className="topbar-actions">
        <button
          className="icon-button"
          type="button"
          title="设置"
          aria-label="设置"
          onClick={onOpenSettings}
        >
          <Icon name="settings" size={17} />
        </button>
      </div>
    </header>
  );
}
