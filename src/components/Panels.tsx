import { useEffect, useState } from "react";
import type { Activity, Backup, Container, Project, Server } from "../data/mock";
import {
  canUseTauri,
  getContainers,
  listRuns,
  type RunRecord,
} from "../api/workspace";
import { Icon } from "./Icon";

export type WorkspaceSection =
  | "overview"
  | "deploy"
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
  return value.length >= 16 ? value.slice(11, 16) : value;
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
    action: [run.command, ...run.args].join(" "),
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
}: {
  icon: "server" | "box" | "shield" | "clock";
  label: string;
  value: string;
  detail: string;
  tone?: "default" | "success" | "warning";
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
      <span className="stat-detail">{detail}</span>
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
  const [liveContainers, setLiveContainers] = useState<Container[] | null>(null);
  const [liveActivities, setLiveActivities] = useState<Activity[] | null>(null);
  const [liveError, setLiveError] = useState<string | null>(null);

  useEffect(() => {
    if (!canUseTauri()) return;
    let cancelled = false;
    setLiveError(null);
    setLiveContainers(null);
    setLiveActivities(null);

    void getContainers(project.id)
      .then((items) => {
        if (!cancelled) {
          setLiveContainers(
            items.map((container) => ({
              service: container.service,
              image: container.image,
              status: container.status,
              port: container.port,
            })),
          );
        }
      })
      .catch((error: unknown) => {
        if (!cancelled) {
          setLiveError(error instanceof Error ? error.message : String(error));
        }
      });

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

  const containers = liveContainers ?? project.containers;
  const running = containers.filter((container) => container.status === "running").length;
  const total = containers.length;
  const projectActivities =
    liveActivities ?? activities.filter((activity) => activity.projectId === project.id);

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
          detail={`${server.host} · ${server.latency}`}
        />
        <StatCard
          icon="box"
          label="容器"
          value={total === 0 ? "—" : `${running}/${total}`}
          detail={total === 0 ? "尚未部署" : "运行中的服务"}
          tone={total > 0 && running < total ? "warning" : "success"}
        />
        <StatCard
          icon="shield"
          label="Nginx"
          value={project.ssl ? "SSL 已启用" : "HTTP"}
          detail={`${project.domain} · :${project.port}`}
          tone={project.ssl ? "success" : "warning"}
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
              onClick={() => onAction(`php admin app:${project.name} dc:restart:php`, "重启 PHP")}
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
              onClick={() => onAction(`php admin app:${project.name} db:backup`, "备份数据库")}
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

export function DeployPanel({ project, server, onAction }: PanelCommonProps) {
  const [filter, setFilter] = useState<"all" | "method" | "name">("all");
  const [method, setMethod] = useState("git");
  const [entry, setEntry] = useState("主仓库");
  const [full, setFull] = useState(false);
  const [updateConfig, setUpdateConfig] = useState(false);
  const [restart, setRestart] = useState(true);

  const filterPart =
    filter === "method" ? ` method=${method}` : filter === "name" ? ` name=${entry}` : "";
  const command = `php admin app:${project.name} upgrade${filterPart}${
    full && filter === "all" ? " full=1" : ""
  }${updateConfig ? " -y" : ""}`;

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
                {server.name} · {server.host}
              </div>
            </label>
            <label className="field">
              <span>远程路径</span>
              <div className="readonly-field mono">{project.path}</div>
            </label>
          </div>

          <div className="field">
            <span>同步过滤</span>
            <div className="segmented">
              <button
                type="button"
                className={filter === "all" ? "active" : ""}
                onClick={() => setFilter("all")}
              >
                全部
              </button>
              <button
                type="button"
                className={filter === "method" ? "active" : ""}
                onClick={() => setFilter("method")}
              >
                按方式
              </button>
              <button
                type="button"
                className={filter === "name" ? "active" : ""}
                onClick={() => setFilter("name")}
              >
                按条目
              </button>
            </div>
          </div>

          {filter === "method" && (
            <label className="field">
              <span>同步方式</span>
              <select className="form-select" value={method} onChange={(event) => setMethod(event.target.value)}>
                <option value="git">git — 远程 clone/pull</option>
                <option value="bundle">bundle — 本地打包直传</option>
                <option value="ftp">ftp — SFTP 增量上传</option>
              </select>
            </label>
          )}

          {filter === "name" && (
            <label className="field">
              <span>条目名称</span>
              <select className="form-select" value={entry} onChange={(event) => setEntry(event.target.value)}>
                <option>主仓库</option>
                <option>src/App/Projects/boyu</option>
                <option>src/App/Modules/demo</option>
              </select>
            </label>
          )}

          <label className="check-row">
            <input
              type="checkbox"
              checked={full}
              disabled={filter !== "all"}
              onChange={(event) => setFull(event.target.checked)}
            />
            <span>
              FTP 全量上传
              <small>忽略增量清单，强制上传全部文件</small>
            </span>
          </label>
          <label className="check-row">
            <input
              type="checkbox"
              checked={updateConfig}
              onChange={(event) => setUpdateConfig(event.target.checked)}
            />
            <span>
              同时更新配置（-y）
              <small>会覆盖远程 compose / nginx / php.ini</small>
            </span>
          </label>
          <label className="check-row">
            <input
              type="checkbox"
              checked={restart}
              onChange={(event) => setRestart(event.target.checked)}
            />
            <span>
              完成后重启 php 容器
              <small>追加执行 dc:restart:php</small>
            </span>
          </label>

          <div className="form-actions">
            <button
              type="button"
              className="button secondary"
              onClick={() => onAction(`php admin app:${project.name} init`, "预览配置")}
            >
              <Icon name="file-text" size={14} />
              预览配置
            </button>
            <button
              type="button"
              className="button primary"
              onClick={() => {
                onAction(command, "升级代码");
                if (restart) {
                  window.setTimeout(
                    () => onAction(`php admin app:${project.name} dc:restart:php`, "重启 PHP"),
                    900,
                  );
                }
              }}
            >
              <Icon name="rocket" size={14} />
              开始升级
            </button>
          </div>
        </section>

        <aside className="card command-card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Command</span>
              <h3>命令预览</h3>
            </div>
            <span className="tag">legacy 对照</span>
          </div>
          <pre className="command-preview">
            <span className="prompt">$</span> {command}
            {restart && (
              <>
                {"\n"}
                <span className="prompt">$</span> php admin app:{project.name} dc:restart:php
              </>
            )}
          </pre>
          <div className="command-hint">
            <Icon name="alert" size={13} />
            当前为 UI 预览，按钮只模拟执行并把命令写入终端，不会连接服务器。
          </div>
          <div className="step-list">
            {[
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

export function ContainersPanel({ project, onAction }: PanelCommonProps) {
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
    if (canUseTauri()) {
      void refreshContainers();
    }
  }, [project.id]);

  const refreshContainers = async () => {
    if (!canUseTauri()) {
      onAction(`php admin app:${project.name} dc:status`, "刷新容器状态");
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
                    onAction("docker compose start", "全部启动");
                  }}
                >
                  <Icon name="play" size={13} />
                  全部启动
                </button>
                <button
                  type="button"
                  onClick={(event) => {
                    event.currentTarget.closest("details")?.removeAttribute("open");
                    onAction("docker compose up -d", "全部重启");
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
                    onAction("docker compose stop", "全部停止");
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
              onClick={() => onAction(`php admin app:${project.name} dc:restart`, "启动容器")}
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
                            onAction(
                              `php admin app:${project.name} dc:log:${container.service}`,
                              `${container.service} 日志`,
                            )
                          }
                        >
                          日志
                        </button>
                        <button
                          type="button"
                          className="mini-button"
                          onClick={() =>
                            onAction(
                              `php admin app:${project.name} dc:restart:${container.service}`,
                              `重启 ${container.service}`,
                            )
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

export function NginxPanel({ project, onAction }: PanelCommonProps) {
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
              onClick={() => onAction("php admin nginx:reload", "重载 Nginx")}
            >
              <Icon name="refresh" size={14} />
              语法检查并重载
            </button>
            <button
              type="button"
              className="button primary"
              onClick={() => onAction(`php admin app:${project.name} nginx:add`, "上传 Nginx 配置")}
            >
              <Icon name="arrow-up-right" size={14} />
              上传配置
            </button>
          </div>
        </section>

        <aside className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Logs</span>
              <h3>最近访问</h3>
            </div>
          </div>
          <div className="mini-log">
            <div><span>200</span> GET / 12ms</div>
            <div><span>200</span> GET /login 8ms</div>
            <div><span>302</span> POST /m/tao/admin.login 14ms</div>
            <div><span>200</span> GET /m/tao/admin.index 21ms</div>
          </div>
          <button
            type="button"
            className="button ghost full"
            onClick={() => onAction("php admin nginx:log:access", "查看访问日志")}
          >
            查看完整访问日志
            <Icon name="chevron-right" size={13} />
          </button>
        </aside>
      </div>
    </div>
  );
}

interface DatabaseProps extends PanelCommonProps {
  backups: Backup[];
}

export function DatabasePanel({ project, backups, onAction }: DatabaseProps) {
  const [pmaDeployed, setPmaDeployed] = useState(false);

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
            onClick={() => onAction(`php admin app:${project.name} db:migrate`, "结构迁移")}
          >
            结构迁移
          </button>
          <button
            type="button"
            className="button secondary small"
            onClick={() => onAction(`php admin app:${project.name} db:seed`, "Seed 基础数据")}
          >
            Seed
          </button>
          <button
            type="button"
            className="button primary small"
            onClick={() => onAction(`php admin app:${project.name} db:backup download=1`, "备份并下载")}
          >
            <Icon name="database" size={13} />
            立即备份
          </button>
        </div>
      </div>

      <div className="two-column wide-left">
        <section className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Backups</span>
              <h3>备份列表</h3>
            </div>
            <span className="muted-caption">{backups.length} 个文件</span>
          </div>
          <div className="table-wrap">
            <table className="data-table">
              <thead>
                <tr>
                  <th>文件</th>
                  <th>大小</th>
                  <th>时间</th>
                  <th className="align-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {backups.map((backup) => (
                  <tr key={backup.name}>
                    <td className="mono backup-name">{backup.name}</td>
                    <td>{backup.size}</td>
                    <td className="muted-cell">{backup.createdAt}</td>
                    <td>
                      <div className="row-actions">
                        <button
                          type="button"
                          className="mini-button"
                          onClick={() =>
                            onAction(
                              `php admin app:${project.name} db:backup:get ${backup.name}`,
                              "下载备份",
                            )
                          }
                        >
                          <Icon name="download" size={12} />
                          下载
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </section>

        <aside className="card">
          <div className="card-heading">
            <div>
              <span className="section-kicker">Schedule</span>
              <h3>定时备份</h3>
            </div>
            <span className="state-badge running">
              <span className="status-dot online" />
              已启用
            </span>
          </div>
          <div className="schedule-card">
            <div>
              <span>每天</span>
              <strong>03:00</strong>
            </div>
            <div>
              <span>保留</span>
              <strong>7 天</strong>
            </div>
            <div>
              <span>位置</span>
              <strong className="mono">/data/backups</strong>
            </div>
          </div>
          <div className="form-actions stacked">
            <button
              type="button"
              className="button secondary full"
              onClick={() => onAction(`php admin app:${project.name} db:backup:list`, "刷新备份列表")}
            >
              <Icon name="refresh" size={13} />
              刷新备份列表
            </button>
            <button
              type="button"
              className="button ghost full"
              onClick={() => onAction(`php admin app:${project.name} db:proxy`, "启动 SSH 隧道")}
            >
              <Icon name="terminal" size={13} />
              开启 SSH 隧道
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
              端口 <code>13307</code> · 临时暴露数据库，使用后请删除
            </p>
            <div className="temp-access-actions">
              <button
                type="button"
                className="button primary small"
                onClick={() => {
                  setPmaDeployed(true);
                  onAction(`php admin app:${project.name} db:pma host=13307`, "部署 phpMyAdmin");
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
                  onAction(`php admin app:${project.name} db:pma-rm`, "删除 phpMyAdmin");
                }}
              >
                <Icon name="trash" size={13} />
                删除
              </button>
            </div>
          </div>
        </aside>
      </div>
    </div>
  );
}

const logSample = [
  { time: "12:30:01", level: "INFO", text: "→ [1/2] [bundle] 主仓库 main" },
  { time: "12:30:02", level: "INFO", text: "→ 生成 deploy/projects/boyu/.env" },
  { time: "12:30:31", level: "INFO", text: "→ docker compose up -d" },
  { time: "12:31:18", level: "OK", text: "✔ 代码同步完成（2m 18s）" },
  { time: "12:31:19", level: "WARN", text: "提示：更新代码后请重启 php 容器" },
  { time: "12:31:21", level: "INFO", text: "→ php artisan migration r" },
  { time: "12:31:40", level: "OK", text: "✔ 迁移完成，共 3 个 migration" },
  { time: "12:31:41", level: "INFO", text: "→ 执行 afterUpgrade hooks" },
];

export function LogsPanel({ project }: Pick<PanelCommonProps, "project">) {
  const [level, setLevel] = useState("ALL");
  const filtered = logSample.filter((line) => level === "ALL" || line.level === level);

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
            <button type="button" className="mini-button">
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
          <div className="log-line muted">
            <span className="log-time">12:31:42</span>
            <span className="log-level info">INFO</span>
            <span className="log-text">— {project.name} · 日志已归档 —</span>
          </div>
        </div>
      </section>
    </div>
  );
}

export function ProjectStatusBadge({ status }: { status: Project["status"] }) {
  return <span className={`state-badge ${status}`}>{statusText[status]}</span>;
}
