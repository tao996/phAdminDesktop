import { useEffect, useState } from "react";
import type { Server } from "../data/mock";
import {
  getProjectConfig,
  type ProjectCreateInput,
  type ProjectSyncItemInput,
} from "../api/workspace";
import { Icon } from "./Icon";

const DEFAULT_PHP_CONFIG = `[
    'app' => [
        'title' => '站点名称',
        'origin' => 'https://myapp.example.com',
        'assets' => [
            'cdn' => '', 'hosts' => [], 'min' => false,
        ],
        'demo' => [
            'open' => false, 'admin' => ['account' => 'admin', 'password' => '123456'],
        ],
        'test' => [
            'open' => false,
        ],
        'superAdmin' => [1],
        'default' => '',
        'defaultApp' => [
            'namespace' => '',
        ],
        'welcome' => '', // 后台首页
    ],
]`;

interface AddProjectModalProps {
  servers: Server[];
  serverId: string;
  existingNames: string[];
  deploymentId?: string;
  onClose: () => void;
  onSubmit: (input: ProjectCreateInput, deploymentId?: string) => Promise<void>;
}

export function AddProjectModal({
  servers,
  serverId,
  existingNames,
  deploymentId,
  onClose,
  onSubmit,
}: AddProjectModalProps) {
  const editing = Boolean(deploymentId);
  const sshServers = servers.filter((server) => server.kind !== "filesystem");
  const [name, setName] = useState("");
  const [mode, setMode] = useState<"remote" | "local">(
    sshServers.length > 0 ? "remote" : "local",
  );
  const [targetServer, setTargetServer] = useState(serverId || sshServers[0]?.id || "");
  const [deployDir, setDeployDir] = useState("");
  const [appName, setAppName] = useState("");
  const [mysqlDatabase, setMysqlDatabase] = useState("");
  const [timezone, setTimezone] = useState("Asia/Shanghai");
  const [mysqlUser, setMysqlUser] = useState("admin");
  const [mysqlPassword, setMysqlPassword] = useState("123456");
  const [redisPassword, setRedisPassword] = useState("123456");
  const [dataPathHost, setDataPathHost] = useState("./docker/storage");
  const [openPort, setOpenPort] = useState("8071");
  const [domains, setDomains] = useState("");
  const [ssl, setSsl] = useState(false);
  const [phpConfig, setPhpConfig] = useState(DEFAULT_PHP_CONFIG);
  const [syncItems, setSyncItems] = useState<ProjectSyncItemInput[]>([
    { method: "git", name: "", path: "", repo: "", branch: "main", excludes: "" },
  ]);
  const [error, setError] = useState("");
  const [saving, setSaving] = useState(false);
  const [loading, setLoading] = useState(editing);

  useEffect(() => {
    if (!deploymentId) return;
    let cancelled = false;
    void getProjectConfig(deploymentId)
      .then((config) => {
        if (cancelled) return;
        setName(config.name);
        setMode(config.mode === "local" ? "local" : "remote");
        setTargetServer(config.serverId);
        setDeployDir(config.deployDir || config.targetPath);
        setAppName(config.appName);
        setMysqlDatabase(config.mysqlDatabase);
        setTimezone(config.timezone);
        setMysqlUser(config.mysqlUser);
        setMysqlPassword(config.mysqlPassword);
        setRedisPassword(config.redisPassword);
        setDataPathHost(config.dataPathHost);
        setOpenPort(String(config.openPort));
        setDomains(config.domains);
        setSsl(config.ssl);
        setPhpConfig(config.phpConfig || DEFAULT_PHP_CONFIG);
        setSyncItems(
          config.syncItems.length > 0
            ? config.syncItems
            : [{ method: "git", name: "", path: "", repo: "", branch: "main", excludes: "" }],
        );
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
  }, [deploymentId]);

  const updateSync = (index: number, patch: Partial<ProjectSyncItemInput>) => {
    setSyncItems((items) =>
      items.map((item, current) => (current === index ? { ...item, ...patch } : item)),
    );
  };

  const safeName = name.trim().toLowerCase().replace(/[^a-z0-9_-]/g, "");
  const effectiveAppName = appName.trim() || safeName;
  const database = mysqlDatabase.trim() || `${effectiveAppName || "xxx"}_db`;

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!safeName) {
      setError("项目名称不能为空，且仅允许字母、数字、下划线、短横线。");
      return;
    }
    if (!editing && existingNames.includes(safeName)) {
      setError("项目名称已存在，请换一个。");
      return;
    }
    if (mode === "remote" && !targetServer) {
      setError("请选择服务器。");
      return;
    }
    if (mode === "local" && !deployDir.trim()) {
      setError("请填写部署目录。");
      return;
    }
    const port = Number(openPort);
    if (!Number.isInteger(port) || port < 1 || port > 65535) {
      setError("OPEN_PORT 必须是 1-65535 的整数。");
      return;
    }
    if (!domains.trim()) {
      setError("请填写至少一个域名。");
      return;
    }
    if (!mysqlUser.trim() || !mysqlPassword || !redisPassword) {
      setError("MYSQL_USER / MYSQL_PASSWORD / REDIS_PASSWORD 不能为空。");
      return;
    }
    if (syncItems.length === 0) {
      setError("请至少添加一个代码同步项。");
      return;
    }
    for (const [index, item] of syncItems.entries()) {
      if (item.method === "git" && !item.repo.trim()) {
        setError(`同步项 ${index + 1}（git）需要填写仓库地址。`);
        return;
      }
      if (item.method === "ftp" && !item.path.trim()) {
        setError(`同步项 ${index + 1}（ftp）需要填写目录。`);
        return;
      }
    }

    setSaving(true);
    setError("");
    try {
      await onSubmit(
        {
          name: safeName,
          mode,
          serverId: mode === "remote" ? targetServer : "",
          targetPath: "",
          deployDir: deployDir.trim(),
          appName: effectiveAppName,
          mysqlDatabase: database,
          timezone: timezone.trim() || "Asia/Shanghai",
          mysqlUser: mysqlUser.trim(),
          mysqlPassword,
          redisPassword,
          dataPathHost: dataPathHost.trim() || "./docker/storage",
          openPort: port,
          domains: domains.trim(),
          ssl,
          phpConfig,
          syncItems,
        },
        deploymentId,
      );
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
      setSaving(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal project-modal"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <span className="section-kicker">{editing ? "Edit Project" : "New Project"}</span>
            <h2>{editing ? "修改项目配置" : "添加项目配置"}</h2>
          </div>
          <button type="button" className="icon-button subtle" onClick={onClose} aria-label="关闭">
            <Icon name="close" size={16} />
          </button>
        </div>

        <form onSubmit={(event) => void submit(event)} autoComplete="off">
          <div className="form-grid">
            <label className="field">
              <span>项目名称</span>
              <input
                className="input mono"
                value={name}
                onChange={(event) => setName(event.target.value)}
                placeholder="myapp"
                autoComplete="off"
                autoFocus={!editing}
                disabled={editing}
              />
            </label>
            <label className="field">
              <span>部署模式</span>
              <select
                className="form-select"
                value={mode}
                onChange={(event) => setMode(event.target.value as "remote" | "local")}
              >
                <option value="remote">Remote（SSH 服务器）</option>
                <option value="local">Local（本机目录）</option>
              </select>
            </label>

            {mode === "remote" && (
              <label className="field">
                <span>服务器</span>
                <select
                  className="form-select"
                  value={targetServer}
                  onChange={(event) => setTargetServer(event.target.value)}
                >
                  {sshServers.length === 0 && <option value="">请先添加服务器</option>}
                  {sshServers.map((server) => (
                    <option key={server.id} value={server.id}>
                      {server.name}
                    </option>
                  ))}
                </select>
              </label>
            )}
          </div>

          <h3 className="form-section-title">项目配置</h3>
          <div className="form-grid">
            <label className="field">
              <span>部署目录</span>
              <input
                className="input mono"
                value={deployDir}
                onChange={(event) => setDeployDir(event.target.value)}
                placeholder={
                  mode === "remote"
                    ? `/data/${safeName || "myapp"}`
                    : `D:\\www\\${safeName || "myapp"}`
                }
              />
            </label>
            <label className="field">
              <span>OPEN_PORT</span>
              <input
                className="input mono"
                value={openPort}
                onChange={(event) => setOpenPort(event.target.value)}
                placeholder="8071"
              />
            </label>
            <label className="field">
              <span>域名（英文逗号分隔）</span>
              <input
                className="input mono"
                value={domains}
                onChange={(event) => setDomains(event.target.value)}
                placeholder="a.com,b.com"
              />
            </label>
            <label className="field switch-field">
              <span>nginx.ssl（证书自动管理）</span>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={ssl}
                  onChange={(event) => setSsl(event.target.checked)}
                />
                <span className="switch-track" />
              </label>
            </label>
          </div>

          <h3 className="form-section-title">环境配置</h3>
          <div className="form-grid">
            <label className="field">
              <span>TZ</span>
              <input
                className="input mono"
                value={timezone}
                onChange={(event) => setTimezone(event.target.value)}
              />
            </label>
            <label className="field">
              <span>APP_NAME</span>
              <input
                className="input mono"
                value={appName}
                onChange={(event) => setAppName(event.target.value)}
                placeholder={safeName || "myapp"}
              />
            </label>
            <label className="field">
              <span>MYSQL_DATABASE</span>
              <input
                className="input mono"
                value={mysqlDatabase}
                onChange={(event) => setMysqlDatabase(event.target.value)}
                placeholder={`${effectiveAppName || "myapp"}_db`}
              />
            </label>
            <label className="field">
              <span>MYSQL_USER</span>
              <input
                className="input mono"
                value={mysqlUser}
                onChange={(event) => setMysqlUser(event.target.value)}
              />
            </label>
            <label className="field">
              <span>MYSQL_PASSWORD</span>
              <input
                className="input mono"
                value={mysqlPassword}
                onChange={(event) => setMysqlPassword(event.target.value)}
              />
            </label>
            <label className="field">
              <span>REDIS_PASSWORD</span>
              <input
                className="input mono"
                value={redisPassword}
                onChange={(event) => setRedisPassword(event.target.value)}
              />
            </label>
            <label className="field">
              <span>DATA_PATH_HOST</span>
              <input
                className="input mono"
                value={dataPathHost}
                onChange={(event) => setDataPathHost(event.target.value)}
              />
            </label>
          </div>

          <h3 className="form-section-title">代码同步</h3>
          <div className="sync-list">
            {syncItems.map((item, index) => (
              <div className="sync-card" key={index}>
                <div className="sync-card-head">
                  <label className="field sync-method-field">
                    <span>方式</span>
                    <select
                      className="form-select"
                      value={item.method}
                      onChange={(event) =>
                        updateSync(index, {
                          method: event.target.value as ProjectSyncItemInput["method"],
                        })
                      }
                    >
                      <option value="git">git</option>
                      <option value="bundle">bundle</option>
                      <option value="ftp">ftp</option>
                    </select>
                  </label>
                  <label className="field sync-name-field">
                    <span>名称</span>
                    <input
                      className="input mono"
                      value={item.name}
                      onChange={(event) => updateSync(index, { name: event.target.value })}
                      placeholder="sync-1"
                    />
                  </label>
                  {(item.method === "git" || item.method === "bundle") && (
                    <label className="field sync-branch-field">
                      <span>分支</span>
                      <input
                        className="input mono"
                        value={item.branch}
                        onChange={(event) => updateSync(index, { branch: event.target.value })}
                        placeholder="main"
                      />
                    </label>
                  )}
                  <button
                    type="button"
                    className="icon-button subtle sync-remove"
                    aria-label="移除同步项"
                    onClick={() =>
                      setSyncItems((items) => items.filter((_, current) => current !== index))
                    }
                  >
                    <Icon name="trash" size={14} />
                  </button>
                </div>

                {item.method !== "bundle" && (
                  <div className="sync-card-fields">
                    {item.method === "git" && (
                    <>
                      <label className="field sync-field-full">
                        <span>仓库地址（必填）</span>
                        <input
                          className="input mono"
                          value={item.repo}
                          onChange={(event) => updateSync(index, { repo: event.target.value })}
                          placeholder="git@github.com:user/repo.git"
                        />
                      </label>
                      <label className="field sync-field-grow">
                        <span>目录（可空）</span>
                        <input
                          className="input mono"
                          value={item.path}
                          onChange={(event) => updateSync(index, { path: event.target.value })}
                          placeholder="留空 = 主仓库"
                        />
                      </label>
                    </>
                  )}

                  {item.method === "ftp" && (
                    <>
                      <label className="field sync-field-grow">
                        <span>目录（必填）</span>
                        <input
                          className="input mono"
                          value={item.path}
                          onChange={(event) => updateSync(index, { path: event.target.value })}
                          placeholder="src/App/Projects/demo"
                        />
                      </label>
                      <label className="field sync-field-full">
                        <span>排除目录（英文逗号分隔，按顺序）</span>
                        <input
                          className="input mono"
                          value={item.excludes}
                          onChange={(event) =>
                            updateSync(index, { excludes: event.target.value })
                          }
                          placeholder="vendor,cache,runtime"
                        />
                      </label>
                    </>
                  )}
                  </div>
                )}
              </div>
            ))}

            <div className="sync-actions">
              <button
                type="button"
                className="button secondary small"
                onClick={() =>
                  setSyncItems((items) => [
                    ...items,
                    { method: "git", name: "", path: "", repo: "", branch: "main", excludes: "" },
                  ])
                }
              >
                <Icon name="plus" size={13} />
                添加同步项
              </button>
              <span className="muted-caption">
                按顺序执行；git/bundle 留空目录表示主仓库，ftp 只增改不删除。
              </span>
            </div>
          </div>

          <label className="field php-config-field">
            <span>PHP 配置（应用配置覆盖）</span>
            <textarea
              className="php-config mono"
              value={phpConfig}
              onChange={(event) => setPhpConfig(event.target.value)}
              spellCheck={false}
            />
          </label>

          {error && <div className="form-error">{error}</div>}

          <div className="modal-footer">
            <span className="muted-caption">
              保存后写入 SQLite；环境变量中的密码存入系统凭据管理器。
            </span>
            <div className="heading-actions">
              <button type="button" className="button secondary" onClick={onClose}>
                取消
              </button>
              <button
                type="submit"
                className="button primary"
                disabled={saving || loading}
              >
                <Icon name={editing ? "check" : "plus"} size={14} />
                {saving ? "保存中…" : editing ? "保存修改" : "创建项目"}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
}


