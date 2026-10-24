import { useRef, useState } from "react";
import type { Project, Server } from "../data/mock";
import type { ImportSummary } from "../api/workspace";
import { Icon } from "./Icon";

interface AddProjectModalProps {
  servers: Server[];
  serverId: string;
  existingNames: string[];
  onClose: () => void;
  onCreate: (project: Project) => void;
}

export function AddProjectModal({
  servers,
  serverId,
  existingNames,
  onClose,
  onCreate,
}: AddProjectModalProps) {
  const [name, setName] = useState("");
  const [title, setTitle] = useState("");
  const [path, setPath] = useState("/data/");
  const [domain, setDomain] = useState("");
  const [port, setPort] = useState("8076");
  const [targetServer, setTargetServer] = useState(serverId);
  const [error, setError] = useState("");

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    const safeName = name.trim().toLowerCase().replace(/[^a-z0-9_-]/g, "");
    if (!safeName) {
      setError("项目名不能为空，且仅允许字母、数字、下划线、短横线。");
      return;
    }
    if (existingNames.includes(safeName)) {
      setError("项目名已存在，请换一个。");
      return;
    }
    onCreate({
      id: `${targetServer}-${safeName}`,
      serverId: targetServer,
      name: safeName,
      title: title.trim() || safeName,
      path: path.trim() || `/data/${safeName}`,
      domain: domain.trim() || `${safeName}.local`,
      port: Number(port) || 8076,
      branch: "main",
      status: "not-deployed",
      ssl: false,
      lastDeploy: "从未",
      backups: 0,
      phpVersion: "PHP 8.5.12",
      containers: [],
    });
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div className="modal" role="dialog" aria-modal="true" onMouseDown={(event) => event.stopPropagation()}>
        <div className="modal-header">
          <div>
            <span className="section-kicker">New Deployment</span>
            <h2>添加项目</h2>
          </div>
          <button type="button" className="icon-button subtle" onClick={onClose} aria-label="关闭">
            <Icon name="close" size={16} />
          </button>
        </div>

        <form onSubmit={submit} autoComplete="off">
          <div className="form-grid">
            <label className="field">
              <span>项目名</span>
              <input
                className="input"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="boyu"
                autoComplete="off"
                autoFocus
              />
            </label>
            <label className="field">
              <span>显示名称</span>
              <input
                className="input"
                value={title}
                onChange={(event) => setTitle(event.target.value)}
                placeholder="博与科技"
              />
            </label>
            <label className="field">
              <span>目标服务器</span>
              <select
                className="form-select"
                value={targetServer}
                onChange={(event) => setTargetServer(event.target.value)}
              >
                {servers.map((server) => (
                  <option key={server.id} value={server.id}>
                    {server.name} · {server.host}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>远程路径</span>
              <input
                className="input mono"
                value={path}
                onChange={(event) => setPath(event.target.value)}
                placeholder="/data/phalcon-admin"
              />
            </label>
            <label className="field">
              <span>域名</span>
              <input
                className="input mono"
                value={domain}
                onChange={(event) => setDomain(event.target.value)}
                placeholder="gu19.cn"
              />
            </label>
            <label className="field">
              <span>Nginx 端口</span>
              <input
                className="input mono"
                value={port}
                onChange={(event) => setPort(event.target.value)}
                placeholder="8071"
              />
            </label>
          </div>

          {error && <div className="form-error">{error}</div>}

          <div className="modal-footer">
            <span className="muted-caption">当前为 UI 预览，只创建内存记录。</span>
            <div className="heading-actions">
              <button type="button" className="button secondary" onClick={onClose}>
                取消
              </button>
              <button type="submit" className="button primary">
                <Icon name="plus" size={14} />
                创建项目
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}

export interface PaletteCommand {
  label: string;
  description: string;
  command: string;
}

interface CommandPaletteProps {
  open: boolean;
  commands: PaletteCommand[];
  onClose: () => void;
  onRun: (command: string, label: string) => void;
}

export function CommandPalette({ open, commands, onClose, onRun }: CommandPaletteProps) {
  const [query, setQuery] = useState("");
  if (!open) return null;

  const filtered = commands.filter((item) => {
    const keyword = query.trim().toLowerCase();
    return (
      keyword === "" ||
      item.label.toLowerCase().includes(keyword) ||
      item.description.toLowerCase().includes(keyword) ||
      item.command.toLowerCase().includes(keyword)
    );
  });

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal palette"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="palette-search">
          <Icon name="search" size={16} />
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="搜索命令、项目或操作…"
            autoFocus
          />
          <kbd>ESC</kbd>
        </div>
        <div className="palette-list">
          {filtered.map((item) => (
            <button
              key={item.command}
              type="button"
              className="palette-item"
              onClick={() => {
                onRun(item.command, item.label);
                onClose();
              }}
            >
              <span className="palette-icon">
                <Icon name="terminal" size={14} />
              </span>
              <span className="palette-copy">
                <strong>{item.label}</strong>
                <small>{item.description}</small>
              </span>
              <code>{item.command}</code>
            </button>
          ))}
          {filtered.length === 0 && <div className="empty-inline">没有匹配的命令</div>}
        </div>
        <div className="palette-footer">
          <span>UI 预览：命令只写入终端，不会执行</span>
          <span>↑↓ 选择 · Enter 运行</span>
        </div>
      </div>
    </div>
  );
}

interface SettingsModalProps {
  onClose: () => void;
  onImportLegacy: (json: string) => Promise<ImportSummary>;
}

export function SettingsModal({ onClose, onImportLegacy }: SettingsModalProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [importing, setImporting] = useState(false);
  const [importMessage, setImportMessage] = useState<{
    ok: boolean;
    text: string;
  } | null>(null);

  const importFile = async (file: File | undefined) => {
    if (!file) return;
    setImporting(true);
    setImportMessage(null);
    try {
      const summary = await onImportLegacy(await file.text());
      setImportMessage({
        ok: true,
        text: `已导入 ${summary.servers} 个服务器、${summary.projects} 个项目、${summary.deployments} 个部署、${summary.syncItems} 个同步项。`,
      });
    } catch (error) {
      setImportMessage({
        ok: false,
        text: error instanceof Error ? error.message : "导入失败",
      });
    } finally {
      setImporting(false);
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal settings-modal"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <span className="section-kicker">Preferences</span>
            <h2>设置</h2>
          </div>
          <button type="button" className="icon-button subtle" onClick={onClose} aria-label="关闭">
            <Icon name="close" size={16} />
          </button>
        </div>
        <div className="settings-list">
          <div className="settings-row">
            <div>
              <strong>主题</strong>
              <small>跟随系统 / 深色 / 浅色</small>
            </div>
            <select className="form-select" defaultValue="dark">
              <option value="dark">深色</option>
              <option value="light">浅色</option>
              <option value="system">跟随系统</option>
            </select>
          </div>
          <div className="settings-row">
            <div>
              <strong>数据目录</strong>
              <small>SQLite 与运行日志的本地位置</small>
            </div>
            <code>%APPDATA%/cn.gu19.phadmin.desktop</code>
          </div>
          <div className="settings-row settings-row-import">
            <div>
              <strong>导入 legacy 配置</strong>
              <small>选择 php admin config:export 生成的 JSON</small>
            </div>
            <div>
              <input
                ref={fileInputRef}
                type="file"
                accept=".json,application/json"
                className="hidden-file-input"
                onChange={(event) => void importFile(event.currentTarget.files?.[0])}
              />
              <button
                type="button"
                className="button secondary small"
                disabled={importing}
                onClick={() => fileInputRef.current?.click()}
              >
                <Icon name="download" size={13} />
                {importing ? "导入中…" : "选择 JSON"}
              </button>
            </div>
          </div>
          {importMessage && (
            <div className={`settings-import-message ${importMessage.ok ? "ok" : "error"}`}>
              <Icon name={importMessage.ok ? "check" : "alert"} size={13} />
              {importMessage.text}
            </div>
          )}
          <div className="settings-row">
            <div>
              <strong>危险操作确认</strong>
              <small>reset / migrate / SSL / cron 前二次确认</small>
            </div>
            <input type="checkbox" defaultChecked />
          </div>
          <div className="settings-row">
            <div>
              <strong>终端滚动行数</strong>
              <small>超过后自动截断</small>
            </div>
            <select className="form-select" defaultValue="5000">
              <option value="2000">2,000</option>
              <option value="5000">5,000</option>
              <option value="10000">10,000</option>
            </select>
          </div>
        </div>
        <div className="modal-footer">
          <span className="muted-caption">导入只写入本地 SQLite，不会连接服务器。</span>
          <button type="button" className="button primary" onClick={onClose}>
            完成
          </button>
        </div>
      </div>
    </div>
  );
}
