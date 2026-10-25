import { useEffect, useState } from "react";
import type { Project } from "../data/mock";
import {
  canUseTauri,
  loadDeploymentConfig,
  renderConfigPreview,
  renderNginxPreview,
  renderComposePreview,
  renderConfigPhpPreview,
  writeGeneratedConfig,
  openGeneratedDir,
  applyGeneratedConfig,
  remoteConfigStatus,
  listConfigBackups,
  restoreConfigBackup,
  type ConfigBackup,
  type RemoteFileStatus,
  generatedConfigStatus,
  type GeneratedFiles,
  type GeneratedStatus,
  type DeploymentConfig,
} from "../api/workspace";
import { Icon } from "./Icon";

interface ConfigModalProps {
  project: Project;
  onClose: () => void;
}

function pretty(value: unknown) {
  if (value === null || value === undefined) return "—";
  if (typeof value === "string") return value;
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}

const sections: { key: keyof NonNullable<DeploymentConfig["settings"]>; label: string }[] = [
  { key: "config", label: "config" },
  { key: "hooks", label: "hooks" },
  { key: "dockerImages", label: "docker images" },
  { key: "router", label: "router" },
  { key: "nginx", label: "nginx" },
];

export function ConfigModal({ project, onClose }: ConfigModalProps) {
  const [config, setConfig] = useState<DeploymentConfig | null>(null);
  const [active, setActive] = useState<string>("sync");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [renderedEnv, setRenderedEnv] = useState<string | null>(null);
  const [renderedNginx, setRenderedNginx] = useState<string | null>(null);
  const [renderedCompose, setRenderedCompose] = useState<string | null>(null);
  const [renderedConfigPhp, setRenderedConfigPhp] = useState<string | null>(null);
  const [generated, setGenerated] = useState<GeneratedFiles | null>(null);
  const [generatedStatus, setGeneratedStatus] = useState<GeneratedStatus | null>(null);
  const [generating, setGenerating] = useState(false);
  const [applying, setApplying] = useState(false);
  const [remoteStatus, setRemoteStatus] = useState<RemoteFileStatus[]>([]);
  const [backups, setBackups] = useState<ConfigBackup[]>([]);

  const loadBackups = async () => {
    try {
      setBackups(await listConfigBackups(project.id));
    } catch {
      setBackups([]);
    }
  };

  const restore = async (backup: ConfigBackup) => {
    if (!window.confirm(`用 ${backup.name} 恢复 ${backup.file}？当前文件会先备份。`)) return;
    try {
      await restoreConfigBackup(project.id, backup.name);
      await loadBackups();
      await checkRemote();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const checkRemote = async () => {
    try {
      setRemoteStatus(await remoteConfigStatus(project.id));
    } catch {
      setRemoteStatus([]);
    }
  };

  const refreshStatus = () => {
    if (!canUseTauri()) return;
    void generatedConfigStatus(project.id)
      .then(setGeneratedStatus)
      .catch(() => setGeneratedStatus(null));
  };

  useEffect(() => {
    refreshStatus();
  }, [project.id]);

  const generate = async () => {
    setGenerating(true);
    try {
      setGenerated(await writeGeneratedConfig(project.id));
      refreshStatus();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setGenerating(false);
    }
  };

  const renderedContent =
    active === "rendered"
      ? renderedEnv
      : active === "rendered-nginx"
        ? renderedNginx
        : active === "rendered-compose"
          ? renderedCompose
          : active === "rendered-config-php"
            ? renderedConfigPhp
            : null;

  const applyRemote = async () => {
    if (
      !window.confirm(
        "将把 .env / nginx.conf / docker-compose.yaml 上传到远程部署目录，并先备份现有文件。继续？",
      )
    ) {
      return;
    }
    setApplying(true);
    try {
      await applyGeneratedConfig(project.id, true);
      refreshStatus();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setApplying(false);
    }
  };

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;
    void renderConfigPreview(project.id)
      .then((value) => {
        if (!cancelled) setRenderedEnv(value);
      })
      .catch(() => {
        if (!cancelled) setRenderedEnv(null);
      });
    void renderNginxPreview(project.id)
      .then((value) => {
        if (!cancelled) setRenderedNginx(value);
      })
      .catch(() => {
        if (!cancelled) setRenderedNginx(null);
      });
    void renderComposePreview()
      .then((value) => {
        if (!cancelled) setRenderedCompose(value);
      })
      .catch(() => {
        if (!cancelled) setRenderedCompose(null);
      });
    void renderConfigPhpPreview(project.id)
      .then((value) => {
        if (!cancelled) setRenderedConfigPhp(value);
      })
      .catch(() => {
        if (!cancelled) setRenderedConfigPhp(null);
      });
    return () => {
      cancelled = true;
    };
  }, [project.id]);

  useEffect(() => {
    if (!canUseTauri()) {
      setError("配置预览只在 Tauri 窗口中可用。");
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);

    void loadDeploymentConfig(project.id)
      .then((data) => {
        if (!cancelled) setConfig(data);
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [project.id]);

  const settings = config?.settings;
  const activeValue =
    active === "sync"
      ? config?.syncItems ?? []
      : settings
        ? settings[active as keyof typeof settings]
        : null;

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal config-modal"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <span className="section-kicker">Config</span>
            <h2>配置预览</h2>
            <p className="runs-subtitle">{project.name} · 来自 SQLite 导入数据</p>
          </div>
          <button type="button" className="icon-button subtle" onClick={onClose} aria-label="关闭">
            <Icon name="close" size={16} />
          </button>
        </div>

        <div className="config-body">
          {loading && <div className="runs-empty">正在读取配置…</div>}
          {!loading && error && <div className="runs-empty error">{error}</div>}
          {!loading && !error && config && (
            <>
              <div className="config-tabs">
                <button
                  type="button"
                  className={active === "sync" ? "active" : ""}
                  onClick={() => setActive("sync")}
                >
                  同步项 {config.syncItems.length}
                </button>
                {settings &&
                  sections.map((section) => (
                    <button
                      type="button"
                      key={section.key}
                      className={active === section.key ? "active" : ""}
                      onClick={() => setActive(section.key)}
                    >
                      {section.label}
                    </button>
                  ))}
                <button
                  type="button"
                  className={active === "rendered" ? "active" : ""}
                  onClick={() => setActive("rendered")}
                >
                  rendered .env
                </button>
                <button
                  type="button"
                  className={active === "rendered-nginx" ? "active" : ""}
                  onClick={() => setActive("rendered-nginx")}
                >
                  rendered nginx
                </button>
                <button
                  type="button"
                  className={active === "rendered-compose" ? "active" : ""}
                  onClick={() => setActive("rendered-compose")}
                >
                  rendered compose
                </button>
                <button
                  type="button"
                  className={active === "rendered-config-php" ? "active" : ""}
                  onClick={() => setActive("rendered-config-php")}
                >
                  rendered config.php
                </button>
              </div>
              {(active === "rendered" ||
                active === "rendered-nginx" ||
                active === "rendered-compose" ||
                active === "rendered-config-php") && (
                <div className="config-generate-row">
                  <button
                    type="button"
                    className="button secondary small"
                    disabled={generating}
                    onClick={() => void generate()}
                  >
                    {generating ? "生成中…" : "写入本地生成目录"}
                  </button>
                  {generated && <code>{generated.directory}</code>}
                  {generatedStatus?.exists && (
                    <button
                      type="button"
                      className="mini-button"
                      disabled={applying}
                      onClick={() => void applyRemote()}
                    >
                      {applying ? "上传中…" : "上传到远程"}
                    </button>
                  )}
                  {generatedStatus?.exists && (
                    <button
                      type="button"
                      className="mini-button"
                      onClick={() => void checkRemote()}
                    >
                      检查远程
                    </button>
                  )}
                  {remoteStatus.map((status) => (
                    <span className="tag" key={status.file}>
                      {status.file}:
                      {!status.exists ? "缺失" : status.changed ? "不同" : "一致"}
                    </span>
                  ))}
                  {generatedStatus?.exists && (
                    <button
                      type="button"
                      className="mini-button"
                      onClick={() => void loadBackups()}
                    >
                      备份列表
                    </button>
                  )}
                  {backups.slice(0, 3).map((backup) => (
                    <button
                      type="button"
                      className="mini-button"
                      key={backup.name}
                      title={backup.name}
                      onClick={() => void restore(backup)}
                    >
                      恢复 {backup.file}
                    </button>
                  ))}
                  {generatedStatus?.exists && (
                    <button
                      type="button"
                      className="mini-button"
                      onClick={() => void openGeneratedDir(project.id)}
                    >
                      打开目录
                    </button>
                  )}
                  {renderedContent && (
                    <button
                      type="button"
                      className="mini-button"
                      onClick={() => void navigator.clipboard?.writeText(renderedContent)}
                    >
                      复制内容
                    </button>
                  )}
                  {generatedStatus && (
                    <span className="tag">
                      {!generatedStatus.exists
                        ? "未生成"
                        : generatedStatus.envChanged ||
                            generatedStatus.nginxChanged ||
                            generatedStatus.composeChanged
                          ? "有变更"
                          : "已同步"}
                    </span>
                  )}
                  <button type="button" className="mini-button" onClick={refreshStatus}>
                    刷新状态
                  </button>
                </div>
              )}
              {active === "rendered" ? (
                <pre className="config-json">
                  {renderedEnv ?? "（生成器暂不可用）"}
                </pre>
              ) : active === "rendered-nginx" ? (
                <pre className="config-json">
                  {renderedNginx ?? "（生成器暂不可用）"}
                </pre>
              ) : active === "rendered-compose" ? (
                <pre className="config-json">
                  {renderedCompose ?? "（生成器暂不可用）"}
                </pre>
              ) : active === "rendered-config-php" ? (
                <pre className="config-json">
                  {renderedConfigPhp ?? "（生成器暂不可用）"}
                </pre>
              ) : active === "sync" ? (
                config.syncItems.length === 0 ? (
                  <div className="runs-empty">没有导入同步项。</div>
                ) : (
                  <div className="config-sync-list">
                    {config.syncItems.map((item) => (
                      <div className="config-sync-row" key={item.id}>
                        <span className="tag">{item.method}</span>
                        <div>
                          <strong>{item.name || item.path || item.id}</strong>
                          <small>
                            {item.repo || "—"} · {item.branch || "main"}
                            {item.excludes.length > 0
                              ? ` · excludes ${item.excludes.length}`
                              : ""}
                          </small>
                        </div>
                      </div>
                    ))}
                  </div>
                )
              ) : (
                <pre className="config-json">{pretty(activeValue)}</pre>
              )}
            </>
          )}
        </div>

        <div className="modal-footer">
          <span className="muted-caption">只读预览，不会修改 SQLite 或服务器配置。</span>
          <button type="button" className="button primary" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
