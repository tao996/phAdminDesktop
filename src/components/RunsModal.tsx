import { useEffect, useState } from "react";
import type { Project } from "../data/mock";
import {
  canUseTauri,
  cancelRun,
  getRunLog,
  listDeployRuns,
  listRuns,
  type DeployRunRecord,
  type RunRecord,
} from "../api/workspace";
import { Icon } from "./Icon";

interface RunsModalProps {
  project: Project;
  version: number;
  onClose: () => void;
  onNotify: (message: string) => void;
}

const statusLabel: Record<RunRecord["status"], string> = {
  queued: "排队",
  running: "运行中",
  succeeded: "成功",
  failed: "失败",
  canceled: "已取消",
};

function formatTime(value: string | null) {
  if (!value) return "—";
  return value.replace("T", " ").replace("Z", "");
}

export function RunsModal({
  project,
  version,
  onClose,
  onNotify,
}: RunsModalProps) {
  const [runs, setRuns] = useState<RunRecord[]>([]);
  const [deployRuns, setDeployRuns] = useState<DeployRunRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [cancelId, setCancelId] = useState<string | null>(null);
  const [logRun, setLogRun] = useState<RunRecord | null>(null);
  const [logText, setLogText] = useState("");
  const [logLoading, setLogLoading] = useState(false);
  const [logError, setLogError] = useState<string | null>(null);

  useEffect(() => {
    if (!canUseTauri()) {
      setError("任务历史只在 Tauri 窗口中可用。");
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);

    void listRuns(project.id)
      .then((data) => {
        if (!cancelled) setRuns(data);
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    void listDeployRuns(project.id)
      .then((data) => {
        if (!cancelled) setDeployRuns(data);
      })
      .catch(() => {
        if (!cancelled) setDeployRuns([]);
      });

    return () => {
      cancelled = true;
    };
  }, [project.id, version, reloadKey]);

  const handleCancel = async (run: RunRecord) => {
    setCancelId(run.id);
    try {
      await cancelRun(run.id);
      onNotify("已请求取消任务");
      setReloadKey((value) => value + 1);
    } catch (reason) {
      onNotify(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setCancelId(null);
    }
  };

  const openLog = async (run: RunRecord) => {
    setLogRun(run);
    setLogText("");
    setLogError(null);
    setLogLoading(true);
    try {
      setLogText(await getRunLog(run.id));
    } catch (reason) {
      setLogError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLogLoading(false);
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={onClose}>
      <div
        className="modal runs-modal"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <span className="section-kicker">Runs</span>
            <h2>任务历史</h2>
            <p className="runs-subtitle">
              {project.name} · {project.serverId} · 最近 {runs.length} 条
            </p>
          </div>
          <button type="button" className="icon-button subtle" onClick={onClose} aria-label="关闭">
            <Icon name="close" size={16} />
          </button>
        </div>

        <div className="runs-toolbar">
          <span className="muted-caption">
            stdout / stderr 实时写入终端，历史记录来自 SQLite runs 表。
          </span>
          <button
            type="button"
            className="mini-button"
            onClick={() => setReloadKey((value) => value + 1)}
            disabled={loading}
          >
            <Icon name="refresh" size={12} />
            刷新
          </button>
        </div>

        {deployRuns.length > 0 && (
          <div className="deploy-history">
            <span className="muted-caption">最近部署</span>
            {deployRuns.slice(0, 3).map((deploy) => (
              <div className="deploy-history-row" key={deploy.id}>
                <span className={`run-status ${deploy.status}`}>
                  <span className="status-dot" />
                  {deploy.status}
                </span>
                <code>{deploy.id.slice(-8)}</code>
                <small>{formatTime(deploy.startedAt)}</small>
                {deploy.errorSummary && <small className="run-error">{deploy.errorSummary}</small>}
              </div>
            ))}
          </div>
        )}

        <div className="runs-body">
          {loading && <div className="runs-empty">正在读取任务历史…</div>}
          {!loading && error && <div className="runs-empty error">{error}</div>}
          {!loading && !error && runs.length === 0 && (
            <div className="runs-empty">
              还没有任务记录。点击「测试连接」运行 `docker compose ps`。
            </div>
          )}
          {!loading &&
            !error &&
            runs.map((run) => (
              <div className="run-row" key={run.id}>
                <div className={`run-status ${run.status}`}>
                  <span className="status-dot" />
                  {statusLabel[run.status]}
                </div>
                <div className="run-main">
                  <code>
                    {run.command} {run.args.join(" ")}
                  </code>
                  {run.stepTitle && <span className="run-step">{run.stepTitle}</span>}
                  <small>
                    {formatTime(run.startedAt)} → {formatTime(run.endedAt)}
                    {run.exitCode !== null ? ` · exit ${run.exitCode}` : ""}
                  </small>
                  {run.errorSummary && <small className="run-error">{run.errorSummary}</small>}
                </div>
                <div className="run-actions">
                  <button
                    type="button"
                    className="mini-button"
                    onClick={() => void openLog(run)}
                  >
                    <Icon name="file-text" size={11} />
                    日志
                  </button>
                  {run.status === "running" && (
                    <button
                      type="button"
                      className="mini-button danger"
                      disabled={cancelId === run.id}
                      onClick={() => void handleCancel(run)}
                    >
                      <Icon name="close" size={11} />
                      {cancelId === run.id ? "取消中…" : "取消"}
                    </button>
                  )}
                  <span className="run-id">{run.id.slice(-8)}</span>
                </div>
              </div>
            ))}
        </div>

        {logRun && (
          <div className="run-log-panel">
            <div className="run-log-head">
              <span>
                日志 · {logRun.command} {logRun.args.join(" ")}
              </span>
              <button
                type="button"
                className="icon-button subtle"
                aria-label="关闭日志"
                onClick={() => setLogRun(null)}
              >
                <Icon name="close" size={14} />
              </button>
            </div>
            {logLoading && <div className="runs-empty">正在读取日志…</div>}
            {!logLoading && logError && <div className="runs-empty error">{logError}</div>}
            {!logLoading && !logError && <pre className="run-log">{logText || "（暂无输出）"}</pre>}
          </div>
        )}

        <div className="modal-footer">
          <span className="muted-caption">
            任务执行使用系统 ssh / 本地进程，密码认证暂未接入。
          </span>
          <button type="button" className="button primary" onClick={onClose}>
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}
