import { useEffect, useState } from "react";
import type { Project } from "../data/mock";
import {
  canUseTauri,
  loadDeploymentConfig,
  renderConfigPreview,
  renderNginxPreview,
  writeGeneratedConfig,
  type GeneratedFiles,
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
  const [generated, setGenerated] = useState<GeneratedFiles | null>(null);
  const [generating, setGenerating] = useState(false);

  const generate = async () => {
    setGenerating(true);
    try {
      setGenerated(await writeGeneratedConfig(project.id));
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setGenerating(false);
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
              </div>
              {(active === "rendered" || active === "rendered-nginx") && (
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
