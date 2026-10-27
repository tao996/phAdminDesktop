import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { Activity, Container, Project, Server } from "../data/mock";
import {
  canUseTauri,
  getContainers,
  listRuns,
  loadDeploymentConfig,
  previewDeployPlan,
  previewProjectFiles,
  saveTemplate,
  type DeployStep,
  type BackupEntry,
  type PreviewFile,
  type RunRecord,
  type SyncItem,
} from "../api/workspace";
import { Icon } from "./Icon";
import { CodeEditor } from "./CodeEditor";

export type WorkspaceSection =
  | "overview"
  | "deploy"
  | "files"
  | "containers"
  | "nginx"
  | "database"
  | "logs";

interface PanelCommonProps {
  project: Project;
  server: Server;
  onAction: (command: string, label: string) => void;
}

interface OverviewProps extends PanelCommonProps {
  activities: Activity[];
  onSectionChange: (section: WorkspaceSection) => void;
  onOpenRuns: () => void;
  refreshVersion: number;
}

function formatRunTime(value: string | null) {
  if (!value) return "—";
  const parsed = new Date(`${value.replace(" ", "T")}Z`);
  return Number.isNaN(parsed.getTime())
    ? value
    : parsed.toLocaleTimeString("zh-CN", { hour12: false });
}

function formatRunDuration(run: RunRecord) {
  if (!run.startedAt || !run.endedAt) {
    return run.status === "running" ? "进行中" : "—";
  }
  const start = Date.parse(run.startedAt.replace(" ", "T") + "Z");
  const end = Date.parse(run.endedAt.replace(" ", "T") + "Z");
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return "—";
  const seconds = Math.round((end - start) / 1000);
  return seconds < 60 ? `${seconds}s` : `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

function runToActivity(run: RunRecord, projectId: string): Activity {
  const result =
    run.status === "succeeded"
      ? "success"
      : run.status === "running" || run.status === "queued"
        ? "running"
        : "failed";
  return {
    id: run.id,
    projectId,
    time: formatRunTime(run.startedAt),
    action: run.stepTitle ?? [run.command, ...run.args].join(" "),
    result,
    duration: formatRunDuration(run),
  };
}

const statusText: Record<Project["status"], string> = {
  running: "运行中",
  stopped: "已停止",
  warning: "异常",
  "not-deployed": "未部署",
};

function StatCard({
  icon,
  label,
  value,
  detail,
  tone = "default",
  onDetailClick,
}: {
  icon: "server" | "box" | "shield" | "clock";
  label: string;
  value: string;
  detail: string;
  tone?: "default" | "success" | "warning";
  onDetailClick?: () => void;
}) {
  return (
    <div className={`stat-card ${tone}`}>
      <div className="stat-card-top">
        <span className="stat-icon">
          <Icon name={icon} size={15} />
        </span>
        <span className="stat-label">{label}</span>
        <span className={`status-dot ${tone === "warning" ? "warning" : "online"}`} />
      </div>
      <strong>{value}</strong>
      {onDetailClick ? (
        <button type="button" className="stat-detail stat-detail-link" onClick={onDetailClick}>
          {detail}
        </button>
      ) : (
        <span className="stat-detail">{detail}</span>
      )}
    </div>
  );
}

export function OverviewPanel({
  project,
  server,
  activities,
  onAction,
  onSectionChange,
  onOpenRuns,
  refreshVersion,
}: OverviewProps) {
  const [liveActivities, setLiveActivities] = useState<Activity[] | null>(null);
  const [liveContainers, setLiveContainers] = useState<Container[] | null>(null);
  const [liveError, setLiveError] = useState<string | null>(null);
  const refreshSeen = useRef(false);

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;
    setLiveError(null);
    setLiveActivities(null);
    setLiveContainers(null);

    // 项目切换时只读取本地运行记录，不主动连接服务器（避免新项目触发 docker compose ps）。
    void listRuns(project.id, 5)
      .then((runs) => {
        if (!cancelled) {
          setLiveActivities(runs.map((run) => runToActivity(run, project.id)));
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLiveError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [project.id, refreshVersion]);

  // 测试连接 / 部署完成后刷新版本变化时才拉取容器，项目切换不自动连接。
  useEffect(() => {
    if (!canUseTauri()) return;
    if (!refreshSeen.current) {
      refreshSeen.current = true;
      return;
    }
    let cancelled = false;
    void getContainers(project.id)
      .then((items) => {
        if (cancelled) return;
        setLiveContainers(
          items.map((container) => ({
            service: container.service,
            image: container.image,
            status: container.status,
            port: container.port,
          })),
        );
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLiveError(error instanceof Error ? error.message : String(error));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [refreshVersion]);

  const containers = liveContainers ?? project.containers;
  const running = containers.filter((container) => container.status === "running").length;
  const total = containers.length;
  const projectActivities =
    liveActivities ?? activities.filter((activity) => activity.projectId === project.id);
  const siteUrl = project.domain
    ? (project.ssl ? "https://" : "http://") + project.domain
    : "";
  const openSite = () => {
    if (!siteUrl) return;
    if (canUseTauri()) {
      void openUrl(siteUrl).catch((error: unknown) => {
        setLiveError(error instanceof Error ? error.message : String(error));
      });
    } else {
      window.open(siteUrl, "_blank", "noreferrer");
    }
  };

  return (
    <div className="panel-stack">
      {liveError && (
        <div className="container-error">
          <Icon name="alert" size={13} />
          <span>{liveError}</span>
        </div>
      )}
      <div className="stat-grid">
        <StatCard
          icon="server"
          label="服务器"
          value={server.name}
          detail={`${server.username} · ${server.latency}`}
        />
        <StatCard
          icon="box"
          label="容器"
          value={total === 0 ? "—" : `${running}/${total}`}
          detail={
            total === 0
              ? project.status === "not-deployed"
                ? "尚未部署"
                : "点击测试连接刷新"
              : "运行中的服务"
          }
          tone={total > 0 && running < total ? "warning" : "success"}
        />
        <StatCard
          icon="shield"
          label="Nginx"
          value={project.ssl ? "SSL 已启用" : "HTTP"}
          detail={siteUrl || "未配置域名"}
          tone={project.ssl ? "success" : "warning"}
          onDetailClick={siteUrl ? openSite : undefined}
        />
        <StatCard
          icon="clock"
          label="最近部署"
          value={project.lastDeploy}
          detail={`${project.branch} · ${project.phpVersion}`}
        />
      </div>

      <div className="overview-grid">
        <section className="card activity-card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Recent</span>
              <h3>最近活动</h3>
            </div>
            <button type="button" className="text-button" onClick={onOpenRuns}>
              查看全部 <Icon name="chevron-right" size={12} />
            </button>
          </div>
          <div className="activity-list">
            {projectActivities.length === 0 && (
              <div className="empty-inline">还没有任务记录，先执行一次测试连接或部署。</div>
            )}
            {projectActivities.map((activity) => (
              <div className="activity-row" key={activity.id}>
                <span className={`activity-icon ${activity.result}`}>
                  <Icon
                    name={activity.result === "failed" ? "alert" : activity.result === "running" ? "refresh" : "check"}
                    size={12}
                  />
                </span>
                <div className="activity-copy">
                  <span className="activity-action">{activity.action}</span>
                  <span className="activity-meta">
                    {activity.time} · {activity.duration}
                  </span>
                </div>
                <span className={`result-pill ${activity.result}`}>
                  {activity.result === "success" ? "成功" : activity.result === "failed" ? "失败" : "运行中"}
                </span>
              </div>
            ))}
          </div>
        </section>

        <aside className="card quick-card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Actions</span>
              <h3>快捷操作</h3>
            </div>
          </div>
          <div className="quick-actions">
            <button
              type="button"
              className="quick-action primary"
              onClick={() => onSectionChange("deploy")}
            >
              <span className="quick-icon">
                <Icon name="rocket" size={15} />
              </span>
              <span>
                <strong>部署 / 升级</strong>
                <small>同步代码与配置</small>
              </span>
              <Icon name="chevron-right" size={13} />
            </button>
            <button
              type="button"
              className="quick-action"
              onClick={() => onAction("docker compose restart php", "重启 PHP")}
            >
              <span className="quick-icon">
                <Icon name="refresh" size={15} />
              </span>
              <span>
                <strong>重启 PHP</strong>
                <small>清空 opcache 与进程</small>
              </span>
              <Icon name="chevron-right" size={13} />
            </button>
            <button
              type="button"
              className="quick-action"
              onClick={() => onAction("phadmin:db-backup", "备份数据库")}
            >
              <span className="quick-icon">
                <Icon name="database" size={15} />
              </span>
              <span>
                <strong>备份数据库</strong>
                <small>立即生成一份备份</small>
              </span>
              <Icon name="chevron-right" size={13} />
            </button>
            <button
              type="button"
              className="quick-action"
              onClick={() => onSectionChange("containers")}
            >
              <span className="quick-icon">
                <Icon name="box" size={15} />
              </span>
              <span>
                <strong>容器状态</strong>
                <small>查看服务与日志</small>
              </span>
              <Icon name="chevron-right" size={13} />
            </button>
          </div>
        </aside>
      </div>
    </div>
  );
}

interface DeployPanelProps extends PanelCommonProps {
  onStartDeploy: (restartPhp: boolean) => void;
  onSyncItem: (itemId: string) => Promise<void>;
  onSyncBundleItem: (itemId: string) => Promise<void>;
  onSyncFtpItem: (itemId: string) => Promise<void>;
  onSyncAll: () => Promise<void>;
}

export function DeployPanel({
  project,
  server,
  onAction,
  onStartDeploy,
  onSyncItem,
  onSyncBundleItem,
  onSyncFtpItem,
  onSyncAll,
}: DeployPanelProps) {
  const [syncingAll, setSyncingAll] = useState(false);
  const [restart, setRestart] = useState(true);
  const [syncItems, setSyncItems] = useState<SyncItem[]>([]);
  const [configError, setConfigError] = useState<string | null>(null);
  const [syncingId, setSyncingId] = useState<string | null>(null);
  const [plan, setPlan] = useState<DeployStep[]>([]);

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;
    setConfigError(null);

    void loadDeploymentConfig(project.id)
      .then((config) => {
        if (cancelled) return;
        setSyncItems(config.syncItems);
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setConfigError(error instanceof Error ? error.message : String(error));
        }
      });

    return () => {
      cancelled = true;
    };
  }, [project.id]);

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;
    void previewDeployPlan(project.id, restart)
      .then((value) => {
        if (!cancelled) setPlan(value);
      })
      .catch(() => {
        if (!cancelled) setPlan([]);
      });
    return () => {
      cancelled = true;
    };
  }, [project.id, restart]);

  return (
    <div className="panel-stack">
      <div className="two-column">
        <section className="card form-card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Deploy</span>
              <h3>升级代码</h3>
            </div>
            <span className="tag">{project.branch}</span>
          </div>

          <div className="form-grid">
            <label className="field">
              <span>目标服务器</span>
              <div className="readonly-field">
                <Icon name="server" size={13} />
                {server.name}
              </div>
            </label>
            <label className="field">
              <span>远程路径</span>
              <div className="readonly-field mono">{project.path}</div>
            </label>
          </div>

          <label className="check-row">
            <input
              type="checkbox"
              checked={restart}
              onChange={(event) => setRestart(event.target.checked)}
            />
            <span>
              完成后重启 php 容器
              <small>追加执行 docker compose restart php</small>
            </span>
          </label>

          <div className="form-actions">
            <button
              type="button"
              className="button secondary"
              disabled={syncingAll || syncItems.length === 0}
              onClick={() => {
                setSyncingAll(true);
                void onSyncAll().finally(() => setSyncingAll(false));
              }}
            >
              <Icon name="refresh" size={14} />
              {syncingAll ? "同步全部中…" : "同步全部"}
            </button>
            <button
              type="button"
              className="button secondary"
              onClick={() => onAction("phadmin:git-key", "生成部署密钥")}
            >
              <Icon name="shield" size={14} />
              部署密钥
            </button>
            <button
              type="button"
              className="button primary"
              onClick={() => {
                onStartDeploy(restart);
              }}
            >
              <Icon name="rocket" size={14} />
              开始部署
            </button>
          </div>

          <p className="muted-caption deploy-key-hint">
            部署密钥会在目标服务器生成 SSH 公钥（<code>~/.ssh/id_ed25519.pub</code>），
            用于拉取私有 Git 仓库；生成后请把公钥添加到 Git 服务商的部署密钥中。
          </p>

          {configError && (
            <div className="container-error">
              <Icon name="alert" size={13} />
              <span>{configError}</span>
            </div>
          )}
          {syncItems.length > 0 && (
            <div className="sync-summary">
              <span className="muted-caption">同步项 {syncItems.length}</span>
              {syncItems.map((item) => (
                <span className="sync-item" key={item.id}>
                  <span className="tag">{item.method}</span>
                  {item.name || item.path || item.id}
                  {item.method === "git" && (
                    <button
                      type="button"
                      className="mini-button"
                      title="本地 / SSH：clone 或 pull 此同步项"
                      disabled={syncingId === item.id}
                      onClick={() => {
                        setSyncingId(item.id);
                        void onSyncItem(item.id).finally(() => setSyncingId(null));
                      }}
                    >
                      {syncingId === item.id ? "启动中…" : "同步"}
                    </button>
                  )}
                  {item.method === "bundle" && (
                    <button
                      type="button"
                      className="mini-button"
                      title="本地 / SSH：git bundle 直传"
                      disabled={syncingId === item.id}
                      onClick={() => {
                        setSyncingId(item.id);
                        void onSyncBundleItem(item.id).finally(() => setSyncingId(null));
                      }}
                    >
                      {syncingId === item.id ? "同步中…" : "同步"}
                    </button>
                  )}
                  {item.method === "ftp" && (
                    <button
                      type="button"
                      className="mini-button"
                      title="本地目录复制 / SSH scp -r"
                      disabled={syncingId === item.id}
                      onClick={() => {
                        setSyncingId(item.id);
                        void onSyncFtpItem(item.id).finally(() => setSyncingId(null));
                      }}
                    >
                      {syncingId === item.id ? "同步中…" : "同步"}
                    </button>
                  )}
                </span>
              ))}
            </div>
          )}
        </section>

        <aside className="card command-card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Command</span>
              <h3>命令预览</h3>
            </div>
            <span className="tag">原生步骤</span>
          </div>
          <pre className="command-preview">
            <span className="prompt">$</span> 同步全部（git / bundle / ftp）
            {"\n"}
            <span className="prompt">$</span> 生成 .env / nginx.conf / docker-compose.yaml / config.php
            {"\n"}
            <span className="prompt">$</span> 上传到 {project.path}
            {"\n"}
            <span className="prompt">$</span> docker compose up -d
            {"\n"}
            <span className="prompt">$</span> nginx -s reload
            {restart && (
              <>
                {"\n"}
                <span className="prompt">$</span> docker compose restart php
              </>
            )}
            {"\n"}
            <span className="prompt">$</span> curl http://127.0.0.1:{project.port}/
          </pre>
          <div className="command-hint">
            <Icon name="alert" size={13} />
            {canUseTauri()
              ? "全部由桌面端执行：同步 → 生成并上传配置 → compose up → Nginx reload → 健康检查；任一步失败会恢复部署前配置。"
              : "当前为 UI 预览，按钮只模拟执行并把命令写入终端，不会连接服务器。"}
          </div>
          <div className="step-list">
            {plan.length > 0
              ? plan.map((step, index) => (
                  <div className="step-row" key={step.key}>
                    <span className="step-index">{index + 1}</span>
                    <div>
                      <strong>{step.title}</strong>
                      <small className="mono">
                        {step.program} {step.args.join(" ")}
                      </small>
                    </div>
                  </div>
                ))
              : [
                  ["同步代码", "bundle / ftp 按 sync.items 顺序"],
                  ["生成配置", ".env / compose / nginx / php.ini"],
                  ["启动容器", "docker compose up -d"],
                  ["更新 Nginx", "生成 server block 并 reload"],
                ].map(([title, desc], index) => (
                  <div className="step-row" key={title}>
                    <span className="step-index">{index + 1}</span>
                    <div>
                      <strong>{title}</strong>
                      <small>{desc}</small>
                    </div>
                  </div>
                ))}
          </div>
        </aside>
      </div>
    </div>
  );
}

interface ContainersPanelProps extends PanelCommonProps {
  onExecuteCommand: (command: string) => void;
  refreshVersion: number;
}

export function ContainersPanel({
  project,
  onAction,
  onExecuteCommand,
  refreshVersion,
}: ContainersPanelProps) {
  const [containers, setContainers] = useState<Container[]>(project.containers);
  const [refreshing, setRefreshing] = useState(false);
  const [containerError, setContainerError] = useState<string | null>(null);
  const running = containers.filter((container) => container.status === "running").length;
  const allMenuOpen = new URLSearchParams(window.location.search).get("all") === "open";

  useEffect(() => {
    setContainers(project.containers);
    setContainerError(null);
  }, [project.id, project.containers]);

  useEffect(() => {
    // 只在部署/命令完成后的刷新版本变化时拉取容器状态；
    // 切换项目不自动连接服务器。
    if (!canUseTauri() || refreshVersion === 0) return;
    void refreshContainers();
  }, [refreshVersion]);

  const refreshContainers = async () => {
    if (!canUseTauri()) {
      onAction("docker compose ps", "刷新容器状态");
      return;
    }

    setRefreshing(true);
    setContainerError(null);
    try {
      const data = await getContainers(project.id);
      setContainers(
        data.map((container) => ({
          service: container.service,
          image: container.image,
          status: container.status,
          port: container.port,
        })),
      );
    } catch (error) {
      setContainerError(error instanceof Error ? error.message : String(error));
    } finally {
      setRefreshing(false);
    }
  };

  return (
    <div className="panel-stack">
      <section className="card">
        <div className="card-heading">
          <div>
            <span className="section-kicker">Docker</span>
            <h3>容器状态</h3>
          </div>
          <div className="heading-actions">
            <span className="muted-caption">
              {running}/{containers.length || 0} running
            </span>
            <details className="all-actions" open={allMenuOpen || undefined}>
              <summary className="button secondary small" title="全部启动 / 重启 / 停止">
                <Icon name="sliders" size={13} />
                全部操作
                <Icon name="chevron-down" size={12} />
              </summary>
              <div className="all-menu">
                <button
                  type="button"
                  onClick={(event) => {
                    event.currentTarget.closest("details")?.removeAttribute("open");
                    onExecuteCommand("docker compose start");
                  }}
                >
                  <Icon name="play" size={13} />
                  全部启动
                </button>
                <button
                  type="button"
                  onClick={(event) => {
                    event.currentTarget.closest("details")?.removeAttribute("open");
                    onExecuteCommand("docker compose up -d");
                  }}
                >
                  <Icon name="refresh" size={13} />
                  全部重启
                </button>
                <button
                  type="button"
                  className="danger"
                  onClick={(event) => {
                    event.currentTarget.closest("details")?.removeAttribute("open");
                    onExecuteCommand("docker compose stop");
                  }}
                >
                  <Icon name="close" size={13} />
                  全部停止
                </button>
              </div>
            </details>
            <button
              type="button"
              className="button secondary small"
              disabled={refreshing}
              onClick={() => void refreshContainers()}
            >
              <Icon name="refresh" size={13} />
              {refreshing ? "刷新中…" : "刷新"}
            </button>
          </div>
        </div>

        <div className="image-notice">
          <span className="image-notice-icon">
            <Icon name="shield" size={14} />
          </span>
          <div>
            <strong>PHP 镜像已更新</strong>
            <span>authus/phalcon:5.23.0 · 旧 5.13.0 存在 PHP 版本漏洞；全部重启会按新镜像重建容器。</span>
          </div>
          <span className="tag latest">最新</span>
        </div>

        {containerError && (
          <div className="container-error">
            <Icon name="alert" size={13} />
            <span>{containerError}</span>
          </div>
        )}

        {containers.length === 0 ? (
          <div className="empty-state">
            <span className="empty-icon">
              <Icon name="box" size={20} />
            </span>
            <h4>还没有容器</h4>
            <p>先完成一次部署，容器会在这里显示。</p>
            <button
              type="button"
              className="button primary small"
              onClick={() => onAction("docker compose up -d", "启动容器")}
            >
              <Icon name="play" size={13} />
              启动容器
            </button>
          </div>
        ) : (
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>服务</th>
                  <th>状态</th>
                  <th>镜像</th>
                  <th>端口</th>
                  <th className="align-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {containers.map((container) => (
                  <tr key={container.service}>
                    <td>
                      <span className="service-name">
                        <Icon name="box" size={13} />
                        {container.service}
                      </span>
                    </td>
                    <td>
                      <span className={`state-badge ${container.status}`}>
                        <span className={`status-dot ${container.status === "running" ? "online" : "offline"}`} />
                        {container.status === "running" ? "running" : "stopped"}
                      </span>
                    </td>
                    <td className="mono muted-cell">
                      {container.image}
                      {container.service === "php" && (
                        <span className="tag latest image-tag">最新</span>
                      )}
                    </td>
                    <td className="mono">{container.port}</td>
                    <td>
                      <div className="row-actions">
                        <button
                          type="button"
                          className="mini-button"
                          onClick={() =>
                            onExecuteCommand(
                              `docker compose logs --tail=200 ${container.service}`,
                            )
                          }
                        >
                          日志
                        </button>
                        <button
                          type="button"
                          className="mini-button"
                          onClick={() =>
                            onExecuteCommand(`docker compose restart ${container.service}`)
                          }
                        >
                          重启
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}

export function NginxPanel({
  project,
  onAction,
  onReloadNginx,
  onAccessLog,
}: PanelCommonProps & {
  onReloadNginx: () => void;
  onAccessLog: () => void;
}) {
  const [email, setEmail] = useState("");
  return (
    <div className="panel-stack">
      <div className="two-column wide-left">
        <section className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Reverse Proxy</span>
              <h3>Nginx</h3>
            </div>
            <span className={`state-badge ${project.ssl ? "running" : "stopped"}`}>
              <span className={`status-dot ${project.ssl ? "online" : "offline"}`} />
              {project.ssl ? "SSL 已启用" : "仅 HTTP"}
            </span>
          </div>

          <div className="info-grid">
            <div className="info-cell">
              <span>域名</span>
              <strong className="mono">{project.domain}</strong>
            </div>
            <div className="info-cell">
              <span>上游</span>
              <strong className="mono">127.0.0.1:{project.port}</strong>
            </div>
            <div className="info-cell">
              <span>配置文件</span>
              <strong className="mono">/etc/nginx/conf.d/{project.name}.conf</strong>
            </div>
            <div className="info-cell">
              <span>证书</span>
              <strong>{project.ssl ? "Let’s Encrypt · 自动续期" : "未申请"}</strong>
            </div>
          </div>

          <div className="domain-list">
            <span className="domain-chip">
              <Icon name="globe" size={12} />
              {project.domain}
            </span>
            {project.ssl && (
              <span className="domain-chip secure">
                <Icon name="shield" size={12} />
                HTTPS
              </span>
            )}
          </div>

          <div className="form-actions">
            <button
              type="button"
              className="button secondary"
              onClick={onReloadNginx}
            >
              <Icon name="refresh" size={14} />
              语法检查并重载
            </button>
            <button
              type="button"
              className="button primary"
              onClick={() => onAction("phadmin:nginx-publish", "发布 Nginx 配置")}
            >
              <Icon name="arrow-up-right" size={14} />
              发布配置
            </button>
            <button
              type="button"
              className="button secondary"
              onClick={() => onAction("phadmin:nginx-remove", "移除 Nginx 配置")}
            >
              <Icon name="trash" size={14} />
              移除配置
            </button>
            <button
              type="button"
              className="button secondary"
              onClick={() => onAction("phadmin:server-check", "服务器环境检查")}
            >
              <Icon name="server" size={14} />
              环境检查
            </button>
          </div>

          <div className="form-actions">
            <input
              className="input"
              type="email"
              placeholder="证书通知邮箱"
              value={email}
              onChange={(event) => setEmail(event.target.value)}
            />
            <button
              type="button"
              className="button primary"
              disabled={!email.includes("@")}
              onClick={() =>
                onAction(
                  `phadmin:ssl-enable?email=${encodeURIComponent(email)}`,
                  "申请 SSL 证书",
                )
              }
            >
              <Icon name="shield" size={14} />
              申请 / 更新证书
            </button>
          </div>
        </section>

        <aside className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Logs</span>
              <h3>Nginx 日志</h3>
            </div>
          </div>
          <p className="muted-caption">
            读取宿主机 /var/log/nginx 最近 200 行，输出到下方终端。
          </p>
          <div className="form-actions stacked">
            <button type="button" className="button ghost full" onClick={onAccessLog}>
              查看完整访问日志
              <Icon name="chevron-right" size={13} />
            </button>
            <button
              type="button"
              className="button ghost full"
              onClick={() => onAction("phadmin:nginx-log-error", "查看错误日志")}
            >
              查看错误日志
              <Icon name="chevron-right" size={13} />
            </button>
          </div>
        </aside>
      </div>
    </div>
  );
}

interface DatabaseProps extends PanelCommonProps {
  onNativeBackup: () => void;
  onListNativeBackups: () => Promise<BackupEntry[]>;
  onRestoreNativeBackup: (name: string) => Promise<void>;
  onDownloadNativeBackup: (name: string) => Promise<void>;
}

export function DatabasePanel({
  project,
  onAction,
  onNativeBackup,
  onListNativeBackups,
  onRestoreNativeBackup,
  onDownloadNativeBackup,
}: DatabaseProps) {
  const [pmaDeployed, setPmaDeployed] = useState(false);
  const [tunnelOpen, setTunnelOpen] = useState(false);
  const [cronTime, setCronTime] = useState("03:00");
  const [cronKeep, setCronKeep] = useState(7);
  const [nativeBackups, setNativeBackups] = useState<BackupEntry[]>([]);
  const [backupNotice, setBackupNotice] = useState("");

  useEffect(() => {
    void onListNativeBackups()
      .then(setNativeBackups)
      .catch(() => setNativeBackups([]));
  }, [project.id]);

  const refreshNativeBackups = async () => {
    try {
      const list = await onListNativeBackups();
      setNativeBackups(list);
      setBackupNotice(list.length > 0 ? `已加载 ${list.length} 个本地备份` : "暂无本地备份");
      window.setTimeout(() => setBackupNotice(""), 3000);
      window.setTimeout(() => {
        document
          .querySelector(".database-native-card")
          ?.scrollIntoView({ behavior: "smooth", block: "nearest" });
      }, 50);
    } catch {
      setBackupNotice("读取本地备份失败");
    }
  };

  return (
    <div className="panel-stack">
      <div className="action-bar">
        <div>
          <span className="section-kicker">Database</span>
          <h3>备份与迁移</h3>
        </div>
        <div className="heading-actions">
          <button
            type="button"
            className="button secondary small"
            title="刷新本机备份列表，并定位到页面底部的「原生备份」"
            onClick={() => void refreshNativeBackups()}
          >
            本地备份
          </button>
          <button
            type="button"
            className="button secondary small"
            title="在服务器上执行 mysqldump 生成备份，并列出远程备份文件"
            onClick={() => onAction("phadmin:db-remote-backups", "服务器备份列表")}
          >
            服务器备份
          </button>
          <button
            type="button"
            className="button secondary small"
            title="打开本地 seed 数据目录；把 SQL 文件放进去后点「Seed」导入"
            onClick={() => onAction("phadmin:db-seed-dir", "打开 seed 目录")}
          >
            Seed 目录
          </button>
          <button
            type="button"
            className="button secondary small"
            title="在服务器容器内执行 Phalcon 数据库结构迁移（migration）"
            onClick={() => onAction("phadmin:db-migrate", "结构迁移")}
          >
            结构迁移
          </button>
          <button
            type="button"
            className="button secondary small"
            title="执行 seed 目录中的基础数据导入"
            onClick={() => onAction("phadmin:db-seed", "Seed 基础数据")}
          >
            Seed
          </button>
          <button
            type="button"
            className="button primary small"
            title="立即生成一份本机压缩备份（.sql.gz）"
            onClick={onNativeBackup}
          >
            <Icon name="database" size={13} />
            立即备份
          </button>
        </div>
      </div>

      {backupNotice && <div className="database-notice">{backupNotice}</div>}

      <div className="two-column wide-left">
        <section className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Schedule</span>
              <h3>服务器定时备份</h3>
            </div>
            <span className="muted-caption">crontab + mysqldump</span>
          </div>
          <p className="muted-caption">
            在服务器安装每日 crontab：mysqldump 压缩到
            <code> &lt;remote&gt;/docker/storage/backup/mysql</code>，超过保留天数自动删除。
          </p>
          <div className="form-grid">
            <label className="field">
              <span>执行时间</span>
              <input
                className="input mono"
                value={cronTime}
                onChange={(event) => setCronTime(event.target.value)}
                placeholder="03:00"
              />
            </label>
            <label className="field">
              <span>保留天数</span>
              <input
                className="input mono"
                type="number"
                min={1}
                max={365}
                value={cronKeep}
                onChange={(event) =>
                  setCronKeep(Math.min(365, Math.max(1, Number(event.target.value) || 1)))
                }
              />
            </label>
          </div>
          <div className="form-actions">
            <button
              type="button"
              className="button primary"
              onClick={() =>
                onAction(
                  `phadmin:db-cron-install?time=${encodeURIComponent(cronTime)}&keep=${cronKeep}`,
                  "安装定时备份",
                )
              }
            >
              <Icon name="clock" size={14} />
              安装 / 更新
            </button>
            <button
              type="button"
              className="button secondary"
              onClick={() => onAction("phadmin:db-cron-remove", "移除定时备份")}
            >
              <Icon name="trash" size={14} />
              移除
            </button>
          </div>
        </section>

        <aside className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Temporary</span>
              <h3>临时访问</h3>
            </div>
          </div>
          <div className="form-actions stacked">
            <button
              type="button"
              className="button ghost full"
              onClick={() => {
                onAction(
                  tunnelOpen ? "phadmin:tunnel-stop" : "phadmin:tunnel-start?port=13306",
                  tunnelOpen ? "关闭 SSH 隧道" : "开启 SSH 隧道",
                );
                setTunnelOpen((open) => !open);
              }}
            >
              <Icon name="terminal" size={13} />
              {tunnelOpen ? "关闭 SSH 隧道（13306）" : "开启 SSH 隧道（13306）"}
            </button>
          </div>
          <div className="temp-access">
            <div className="temp-access-heading">
              <div>
                <span className="section-kicker">Temporary</span>
                <strong>临时 phpMyAdmin</strong>
              </div>
              <span className={`state-badge ${pmaDeployed ? "running" : "not-deployed"}`}>
                <span className={`status-dot ${pmaDeployed ? "online" : "offline"}`} />
                {pmaDeployed ? "已部署" : "未部署"}
              </span>
            </div>
            <p>
              端口 <code>13307</code> · 容器 <code>--rm</code>，使用后请删除
            </p>
            <div className="temp-access-actions">
              <button
                type="button"
                className="button primary small"
                onClick={() => {
                  setPmaDeployed(true);
                  onAction("phadmin:pma-start?port=13307", "部署 phpMyAdmin");
                }}
              >
                <Icon name="database" size={13} />
                {pmaDeployed ? "重新部署" : "部署 phpMyAdmin"}
              </button>
              <button
                type="button"
                className="button ghost small"
                disabled={!pmaDeployed}
                onClick={() => {
                  setPmaDeployed(false);
                  onAction("phadmin:pma-stop", "删除 phpMyAdmin");
                }}
              >
                <Icon name="trash" size={13} />
                删除
              </button>
            </div>
          </div>
        </aside>
      </div>

      <section className="card database-native-card">
        <div className="card-heading">
          <div>
            <span className="section-kicker">Local MySQL</span>
            <h3>原生备份</h3>
          </div>
          <span className="muted-caption">{nativeBackups.length} 个文件</span>
        </div>
        <div className="native-backup-list">
          {nativeBackups.slice(0, 5).map((backup) => (
            <div className="native-backup-row" key={backup.name}>
              <code>{backup.name}</code>
              <small>{backup.bytes} bytes</small>
              <button
                type="button"
                className="mini-button"
                onClick={() => void onDownloadNativeBackup(backup.name)}
              >
                <Icon name="download" size={12} />
                下载
              </button>
              <button
                type="button"
                className="mini-button"
                onClick={() => void onRestoreNativeBackup(backup.name)}
              >
                恢复
              </button>
            </div>
          ))}
          {nativeBackups.length === 0 && (
            <div className="empty-inline">暂无本地备份，点击「立即备份」生成。</div>
          )}
        </div>
      </section>
    </div>
  );
}

export function LogsPanel({ project }: Pick<PanelCommonProps, "project">) {
  const [level, setLevel] = useState("ALL");
  const [lines, setLines] = useState<{ time: string; level: string; text: string }[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;
    setLoading(true);
    void listRuns(project.id, 100)
      .then((runs) => {
        if (cancelled) return;
        setLines(
          runs.map((run) => ({
            time: formatRunTime(run.startedAt),
            level:
              run.status === "succeeded"
                ? "OK"
                : run.status === "failed" || run.status === "canceled"
                  ? "WARN"
                  : "INFO",
            text: `${[run.command, ...run.args].join(" ")}${
              run.stepTitle ? ` · ${run.stepTitle}` : ""
            }${run.errorSummary ? ` · ${run.errorSummary}` : ""}`,
          })),
        );
      })
      .catch(() => {
        if (!cancelled) setLines([]);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [project.id]);

  const filtered = lines.filter((line) => level === "ALL" || line.level === level);

  const exportLogs = () => {
    const text = filtered
      .map((line) => `${line.time} ${line.level} ${line.text}`)
      .join("\n");
    const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }));
    const link = document.createElement("a");
    link.href = url;
    link.download = `${project.name}-runs.log`;
    link.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="panel-stack">
      <section className="card logs-card">
        <div className="card-heading">
          <div>
            <span className="section-kicker">History</span>
            <h3>运行日志</h3>
          </div>
          <div className="heading-actions">
            <div className="segmented compact">
              {["ALL", "INFO", "OK", "WARN"].map((item) => (
                <button
                  key={item}
                  type="button"
                  className={level === item ? "active" : ""}
                  onClick={() => setLevel(item)}
                >
                  {item}
                </button>
              ))}
            </div>
            <button type="button" className="mini-button" onClick={exportLogs}>
              <Icon name="download" size={12} />
              导出
            </button>
          </div>
        </div>
        <div className="log-viewer">
          {filtered.map((line) => (
            <div className="log-line" key={`${line.time}-${line.text}`}>
              <span className="log-time">{line.time}</span>
              <span className={`log-level ${line.level.toLowerCase()}`}>{line.level}</span>
              <span className="log-text">{line.text}</span>
            </div>
          ))}
          {!loading && filtered.length === 0 && (
            <div className="empty-inline">暂无运行记录</div>
          )}
          {loading && <div className="empty-inline">读取运行记录…</div>}
        </div>
      </section>
    </div>
  );
}

export function FilesPanel({ project }: Pick<PanelCommonProps, "project">) {
  const [files, setFiles] = useState<PreviewFile[]>([]);
  const [active, setActive] = useState("");
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [editorVersion, setEditorVersion] = useState(0);
  const [addOpen, setAddOpen] = useState(false);
  const [newPath, setNewPath] = useState("");
  const [newContent, setNewContent] = useState("");
  const [addError, setAddError] = useState("");
  const [savingFile, setSavingFile] = useState(false);

  const load = async () => {
    if (!canUseTauri()) {
      setError("浏览器预览模式不生成文件。");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const list = await previewProjectFiles(project.id);
      setFiles(list);
      setDrafts({});
      setActive((current) =>
        list.some((file) => file.path === current) ? current : (list[0]?.path ?? ""),
      );
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    void load();
  }, [project.id]);

  const current = files.find((file) => file.path === active);
  const content = current ? (drafts[current.path] ?? current.content) : "";
  const dirty = current ? drafts[current.path] !== undefined && drafts[current.path] !== current.content : false;

  const copyContent = async () => {
    await navigator.clipboard.writeText(content);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1500);
  };

  const saveNewFile = async () => {
    const path = newPath.trim();
    if (!path) {
      setAddError("请填写文件路径");
      return;
    }
    if (!newContent.trim()) {
      setAddError("请填写文件内容");
      return;
    }
    setSavingFile(true);
    setAddError("");
    try {
      const saved = await saveTemplate(path, newContent);
      setAddOpen(false);
      setNewPath("");
      setNewContent("");
      await load();
      setActive(saved.path);
    } catch (reason) {
      setAddError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSavingFile(false);
    }
  };

  return (
    <div className="panel-stack">
      <div className="files-head">
        <div>
          <span className="section-kicker">Generated</span>
          <h3>文件预览</h3>
          <p className="muted-caption">
            项目配置 + 模板管理合并后的最终内容，即同步到服务器上的文件；右侧可直接编辑查看（不会写回模板）。
          </p>
        </div>
        <div className="heading-actions">
          <button
            type="button"
            className="button secondary small"
            onClick={() => {
              setAddOpen(true);
              setAddError("");
            }}
          >
            <Icon name="plus" size={13} />
            添加文件
          </button>
          <button
            type="button"
            className="button secondary small"
            onClick={() => void load()}
            disabled={loading}
          >
            <Icon name="refresh" size={13} />
            刷新
          </button>
        </div>
      </div>

      {error && (
        <div className="container-error">
          <Icon name="alert" size={13} />
          <span>{error}</span>
        </div>
      )}

      <div className="files-grid">
        <aside className="files-list">
          {files.map((file) => (
            <button
              key={file.path}
              type="button"
              className={file.path === active ? "files-item active" : "files-item"}
              onClick={() => setActive(file.path)}
            >
              <Icon name="file-text" size={12} />
              <span>{file.path}</span>
            </button>
          ))}
          {files.length === 0 && <div className="empty-inline">暂无文件</div>}
        </aside>
        <section className="files-view">
          {current ? (
            <>
              <div className="files-path">
                <span>{current.path}</span>
                <div className="heading-actions">
                  {dirty && (
                    <button
                      type="button"
                      className="mini-button"
                      onClick={() => {
                        setDrafts((currentDrafts) => {
                          const next = { ...currentDrafts };
                          delete next[current.path];
                          return next;
                        });
                        setEditorVersion((value) => value + 1);
                      }}
                    >
                      重置修改
                    </button>
                  )}
                  <button type="button" className="mini-button" onClick={() => void copyContent()}>
                    <Icon name="copy" size={12} />
                    {copied ? "已复制" : "复制"}
                  </button>
                </div>
              </div>
              <CodeEditor
                key={`${current.path}:${editorVersion}`}
                path={current.path}
                value={content}
                onChange={(value) =>
                  setDrafts((currentDrafts) => ({
                    ...currentDrafts,
                    [current.path]: value,
                  }))
                }
              />
            </>
          ) : (
            <div className="empty-inline">选择左侧文件查看内容</div>
          )}
        </section>
      </div>

      {addOpen && (
        <div
          className="modal-backdrop"
          role="presentation"
          onMouseDown={() => setAddOpen(false)}
        >
          <div
            className="modal add-file-modal"
            role="dialog"
            aria-modal="true"
            onMouseDown={(event) => event.stopPropagation()}
          >
            <div className="modal-header">
              <div>
                <span className="section-kicker">Custom File</span>
                <h2>添加文件</h2>
              </div>
              <button
                type="button"
                className="icon-button subtle"
                onClick={() => setAddOpen(false)}
                aria-label="关闭"
              >
                <Icon name="close" size={16} />
              </button>
            </div>
            <div className="add-file-body">
              <label className="field">
                <span>文件路径（相对项目根目录）</span>
                <input
                  className="input mono"
                  value={newPath}
                  onChange={(event) => setNewPath(event.target.value)}
                  placeholder="docker/nginx/conf.d/extra.conf"
                />
              </label>
              <label className="field">
                <span>文件内容（支持 {'{{VAR}}'} 模板变量）</span>
                <textarea
                  className="input mono add-file-content"
                  value={newContent}
                  onChange={(event) => setNewContent(event.target.value)}
                  placeholder="server {&#10;  listen 80;&#10;}"
                  spellCheck={false}
                />
              </label>
              {addError && <div className="form-error">{addError}</div>}
            </div>
            <div className="modal-footer">
              <span className="muted-caption">保存后写入模板表，参与生成、预览与上传。</span>
              <div className="heading-actions">
                <button
                  type="button"
                  className="button secondary"
                  onClick={() => setAddOpen(false)}
                >
                  取消
                </button>
                <button
                  type="button"
                  className="button primary"
                  onClick={() => void saveNewFile()}
                  disabled={savingFile}
                >
                  {savingFile ? "保存中…" : "保存文件"}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export function ProjectStatusBadge({ status }: { status: Project["status"] }) {
  return <span className={`state-badge ${status}`}>{statusText[status]}</span>;
}
