import type { Server } from "../data/mock";
import { Icon } from "./Icon";

interface TopBarProps {
  servers: Server[];
  serverId: string;
  onServerChange: (id: string) => void;
  onOpenCommand: () => void;
  onOpenSettings: () => void;
}

export function TopBar({
  servers,
  serverId,
  onServerChange,
  onOpenCommand,
  onOpenSettings,
}: TopBarProps) {
  const server = servers.find((item) => item.id === serverId) ?? servers[0];

  return (
    <header className="topbar">
      <div className="brand">
        <span className="brand-mark">P</span>
        <span className="brand-name">PhAdmin</span>
        <span className="brand-sub">Desktop</span>
        <span className="preview-pill">UI Preview</span>
      </div>

      <div className="server-picker">
        <span className={`server-dot ${server.status}`} />
        <div className="server-picker-copy">
          <span className="server-picker-label">服务器</span>
          <select
            className="server-select"
            value={serverId}
            onChange={(event) => onServerChange(event.target.value)}
            aria-label="选择服务器"
          >
            {servers.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name} · {item.host}
              </option>
            ))}
          </select>
        </div>
        <span className="server-latency">{server.latency}</span>
        <Icon name="chevron-down" size={13} className="server-chevron" />
      </div>

      <div className="topbar-actions">
        <button className="command-button" type="button" onClick={onOpenCommand}>
          <Icon name="command" size={14} />
          <span>命令</span>
          <kbd>⌘K</kbd>
        </button>
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
