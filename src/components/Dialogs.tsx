import { useEffect, useState } from "react";
import {
  countServerReferences,
  getLocalRepoPath,
  getServerConfig,
  setLocalRepoPath,
  type ServerInput,
} from "../api/workspace";
import { Icon } from "./Icon";

interface ServerModalProps {
  /** 传入 id 表示修改已有服务器，否则为新增。 */
  serverId?: string;
  serverName?: string;
  onClose: () => void;
  onSave: (input: ServerInput) => Promise<void>;
  onDelete?: () => Promise<void>;
}

export function ServerModal({
  serverId,
  serverName,
  onClose,
  onSave,
  onDelete,
}: ServerModalProps) {
  const editing = Boolean(serverId);
  const [name, setName] = useState(serverName ?? "");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("22");
  const [user, setUser] = useState("root");
  const [auth, setAuth] = useState<"password" | "key">("password");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [keyFile, setKeyFile] = useState("");
  const [keyPassphrase, setKeyPassphrase] = useState("");
  const [dockerNetwork, setDockerNetwork] = useState("phalcon-shared");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [referenceCount, setReferenceCount] = useState<number | null>(null);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [loading, setLoading] = useState(editing);

  const requestDelete = async () => {
    if (!serverId) return;
    setConfirming(true);
    setReferenceCount(null);
    setDeleteError(null);
    try {
      setReferenceCount(await countServerReferences(serverId));
    } catch (reason) {
      setDeleteError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const confirmDelete = async () => {
    if (!onDelete) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await onDelete();
    } catch (reason) {
      setDeleteError(reason instanceof Error ? reason.message : String(reason));
      setDeleting(false);
    }
  };

  useEffect(() => {
    if (!serverId) return;
    let cancelled = false;
    void getServerConfig(serverId)
      .then((config) => {
        if (cancelled) return;
        setName(config.name);
        setHost(config.host);
        setPort(String(config.port));
        setUser(config.user);
        setPassword(config.password);
        setKeyFile(config.keyFile);
        setKeyPassphrase(config.keyPassphrase);
        setDockerNetwork(config.dockerNetwork || "phalcon-shared");
        setAuth(config.keyFile ? "key" : "password");
        setLoading(false);
      })
      .catch((reason: unknown) => {
        if (cancelled) return;
        setError(reason instanceof Error ? reason.message : String(reason));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [serverId]);

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!name.trim()) {
      setError("请填写服务器名称（用于项目列表显示）");
      return;
    }
    if (!host.trim()) {
      setError("请填写主机（IP 或域名）");
      return;
    }
    if (!user.trim()) {
      setError("请填写 SSH 用户");
      return;
    }
    if (auth === "password" && !password) {
      setError("请填写 SSH 密码");
      return;
    }
    if (auth === "key" && !keyFile.trim()) {
      setError("请填写私钥路径");
      return;
    }

    setSaving(true);
    setError(null);
    try {
      await onSave({
        name: name.trim(),
        host: host.trim(),
        port: Number(port) || 22,
        user: user.trim(),
        password: auth === "password" ? password : "",
        keyFile: auth === "key" ? keyFile.trim() : "",
        keyPassphrase: auth === "key" ? keyPassphrase : "",
        dockerNetwork: dockerNetwork.trim() || "phalcon-shared",
      });
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <span className="section-kicker">{editing ? "Edit Server" : "New Server"}</span>
            <h2>{editing ? "修改服务器" : "添加服务器"}</h2>
          </div>
          <button type="button" className="icon-button subtle" onClick={onClose} aria-label="关闭">
            <Icon name="close" size={16} />
          </button>
        </div>

        <form onSubmit={(event) => void submit(event)} autoComplete="off">
          <div className="form-grid server-form-grid">
            <label className="field">
              <span>名称</span>
              <input
                className="input mono"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="如 vbox / production"
                autoFocus
              />
            </label>
            <label className="field">
              <span>主机</span>
              <input
                className="input mono"
                value={host}
                onChange={(event) => setHost(event.target.value)}
                placeholder="10.0.0.5 或 domain"
              />
            </label>
            <label className="field">
              <span>端口</span>
              <input
                className="input mono"
                value={port}
                onChange={(event) => setPort(event.target.value)}
                placeholder="22"
              />
            </label>
            <label className="field">
              <span>用户</span>
              <input
                className="input mono"
                value={user}
                onChange={(event) => setUser(event.target.value)}
                placeholder="root"
              />
            </label>
            <label className="field">
              <span>认证方式</span>
              <select
                className="form-select"
                value={auth}
                onChange={(event) => setAuth(event.target.value as "password" | "key")}
              >
                <option value="password">密码</option>
                <option value="key">私钥</option>
              </select>
            </label>

            {auth === "password" ? (
              <label className="field">
                <span>密码</span>
                <div className="input-with-suffix">
                  <input
                    className="input mono"
                    type={showPassword ? "text" : "password"}
                    autoComplete="new-password"
                    value={password}
                    onChange={(event) => setPassword(event.target.value)}
                    placeholder="SSH 密码"
                  />
                  <button
                    type="button"
                    className="input-suffix-button"
                    aria-label={showPassword ? "隐藏密码" : "显示密码"}
                    title={showPassword ? "隐藏密码" : "显示密码"}
                    onClick={() => setShowPassword((value) => !value)}
                  >
                    <Icon name={showPassword ? "eye-off" : "eye"} size={14} />
                  </button>
                </div>
              </label>
            ) : (
              <>
                <label className="field">
                  <span>私钥路径</span>
                  <input
                    className="input mono"
                    value={keyFile}
                    onChange={(event) => setKeyFile(event.target.value)}
                    placeholder="C:\\Users\\me\\.ssh\\id_ed25519"
                  />
                </label>
                <label className="field">
                  <span>私钥口令（可选）</span>
                  <input
                    className="input mono"
                    type="password"
                    autoComplete="new-password"
                    value={keyPassphrase}
                    onChange={(event) => setKeyPassphrase(event.target.value)}
                    placeholder="加密私钥的 passphrase"
                  />
                </label>
              </>
            )}
            <label className="field">
              <span>共享网络名（docker.network）</span>
              <input
                className="input mono"
                value={dockerNetwork}
                onChange={(event) => setDockerNetwork(event.target.value)}
                placeholder="phalcon-shared"
              />
            </label>
          </div>

          {loading && <div className="muted-caption">正在读取服务器配置…</div>}
          {error && <div className="form-error">{error}</div>}

          <div className="modal-footer">
            <span className="muted-caption">
              名称用于项目列表展示；密码 / 私钥口令保存在本地 SQLite。
            </span>
            <div className="heading-actions">
              {editing && onDelete && (
                <button
                  type="button"
                  className="button danger"
                  onClick={() => void requestDelete()}
                  disabled={deleting || saving || loading}
                >
                  删除
                </button>
              )}
              <button type="button" className="button secondary" onClick={onClose}>
                取消
              </button>
              <button
                type="submit"
                className="button primary"
                disabled={saving || loading || deleting}
              >
                {saving ? "保存中…" : editing ? "保存" : "添加"}
              </button>
            </div>
          </div>
        </form>
      </div>

      {confirming && (
        <div
          className="confirm-layer"
          role="dialog"
          aria-modal="true"
          aria-label="删除服务器确认"
          onMouseDown={(event) => event.stopPropagation()}
        >
          <div className="confirm-card">
            <div className="confirm-header">
              <Icon name="trash" size={16} />
              <h3>删除服务器</h3>
            </div>
            <p className="confirm-text">
              确定删除 <strong>{serverName ?? serverId}</strong>？删除后不可恢复。
            </p>

            {referenceCount === null && !deleteError && (
              <p className="muted-caption">正在检查引用…</p>
            )}
            {referenceCount !== null && referenceCount > 0 && (
              <div className="form-error">
                该服务器仍被 {referenceCount} 个部署引用，不能删除。
              </div>
            )}
            {deleteError && <div className="form-error">{deleteError}</div>}

            <div className="confirm-actions">
              <button
                type="button"
                className="button secondary"
                onClick={() => setConfirming(false)}
                disabled={deleting}
              >
                取消
              </button>
              <button
                type="button"
                className="button danger"
                onClick={() => void confirmDelete()}
                disabled={
                  deleting ||
                  referenceCount === null ||
                  referenceCount > 0 ||
                  Boolean(deleteError)
                }
              >
                {deleting ? "删除中…" : "确定"}
              </button>
            </div>
          </div>
        </div>
      )}
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
  onOpenDataDir: () => void;
}

export function SettingsModal({ onClose, onOpenDataDir }: SettingsModalProps) {
  const [localRepo, setLocalRepo] = useState("");
  const [repoStatus, setRepoStatus] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    void getLocalRepoPath()
      .then((path) => {
        if (cancelled) return;
        if (path === null || path.trim() === "") {
          const fallback = "D:\\demo\\code\\flutter\\library\\projects\\phalcon-admin";
          setLocalRepo(fallback);
          void setLocalRepoPath(fallback);
        } else {
          setLocalRepo(path);
        }
      })
      .catch(() => {
        if (!cancelled) setRepoStatus("读取失败");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const saveRepo = async () => {
    try {
      await setLocalRepoPath(localRepo.trim());
      setRepoStatus("已保存");
    } catch (reason) {
      setRepoStatus(reason instanceof Error ? reason.message : String(reason));
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
            <div className="heading-actions">
              <code>%APPDATA%/cn.gu19.phadmin.desktop</code>
              <button type="button" className="button secondary small" onClick={onOpenDataDir}>
                <Icon name="folder" size={13} />
                打开目录
              </button>
            </div>
          </div>
          <div className="settings-row">
            <div>
              <strong>本地仓库</strong>
              <small>代码同步（bundle / ftp）使用的本地源目录</small>
            </div>
            <div className="heading-actions">
              <input
                className="input mono settings-repo-input"
                value={localRepo}
                onChange={(event) => {
                  setLocalRepo(event.target.value);
                  setRepoStatus(null);
                }}
                placeholder="D:\\path\\to\\repo"
              />
              <button type="button" className="button secondary small" onClick={() => void saveRepo()}>
                保存
              </button>
              {repoStatus && <span className="muted-caption">{repoStatus}</span>}
            </div>
          </div>
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


