import { useEffect, useRef, useState } from "react";
import type { Project, TerminalLine } from "../data/mock";
import { Icon } from "./Icon";

interface TerminalPanelProps {
  project: Project;
  lines: TerminalLine[];
  collapsed: boolean;
  onToggle: () => void;
  onClear: () => void;
  onCommand: (command: string) => void;
  onOpenRuns: () => void;
}

const typeLabel: Record<TerminalLine["type"], string> = {
  command: "CMD",
  info: "INFO",
  success: "OK",
  error: "ERR",
  muted: "LOG",
};

export function TerminalPanel({
  project,
  lines,
  collapsed,
  onToggle,
  onClear,
  onCommand,
  onOpenRuns,
}: TerminalPanelProps) {
  const [value, setValue] = useState("");
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!collapsed) {
      bottomRef.current?.scrollIntoView({ block: "end" });
    }
  }, [lines, collapsed]);

  const copyLog = () => {
    const text = lines.map((line) => `[${line.time}] ${line.text}`).join("\n");
    void navigator.clipboard?.writeText(text);
  };

  return (
    <section className={`terminal-panel ${collapsed ? "collapsed" : ""}`}>
      <div className="terminal-header">
        <div className="terminal-tabs">
          <span className="terminal-tab active">
            <Icon name="terminal" size={13} />
            终端 · {project.name}
            <span className="terminal-live" />
          </span>
          <span className="terminal-tab muted">服务器</span>
        </div>
        <div className="terminal-actions">
          <span className="terminal-status">
            <span className="status-dot online" />
            已连接
          </span>
          <button type="button" className="mini-button" onClick={onOpenRuns}>
            <Icon name="clock" size={12} />
            任务
          </button>
          <button type="button" className="mini-button" onClick={copyLog}>
            <Icon name="copy" size={12} />
            复制
          </button>
          <button type="button" className="mini-button" onClick={onClear}>
            <Icon name="trash" size={12} />
            清空
          </button>
          <button
            type="button"
            className="icon-button subtle"
            title={collapsed ? "展开终端" : "收起终端"}
            aria-label={collapsed ? "展开终端" : "收起终端"}
            onClick={onToggle}
          >
            <Icon name="panel-bottom" size={15} />
          </button>
        </div>
      </div>

      {!collapsed && (
        <>
          <div className="terminal-body">
            {lines.length === 0 && (
              <div className="terminal-empty">终端已清空。输入命令开始。</div>
            )}
            {lines.map((line) => (
              <div className={`terminal-line ${line.type}`} key={line.id}>
                <span className="terminal-time">{line.time}</span>
                <span className="terminal-level">{typeLabel[line.type]}</span>
                <span className="terminal-text">{line.text}</span>
              </div>
            ))}
            <div ref={bottomRef} />
          </div>

          <form
            className="terminal-input"
            onSubmit={(event) => {
              event.preventDefault();
              const command = value.trim();
              if (!command) return;
              onCommand(command);
              setValue("");
            }}
          >
            <span className="terminal-prompt">$</span>
            <input
              value={value}
              onChange={(event) => setValue(event.target.value)}
              placeholder="输入命令，例如 docker compose ps"
              aria-label="终端命令"
            />
            <span className="terminal-hint">Enter 执行 · 白名单</span>
          </form>
        </>
      )}
    </section>
  );
}
