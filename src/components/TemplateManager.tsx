import { useEffect, useState } from "react";
import {
  getTemplate,
  listTemplates,
  resetTemplate,
  saveTemplate,
  type TemplateInfo,
} from "../api/workspace";
import { Icon } from "./Icon";

interface TemplateManagerProps {
  onClose: () => void;
}

export function TemplateManager({ onClose }: TemplateManagerProps) {
  const [templates, setTemplates] = useState<TemplateInfo[]>([]);
  const [selected, setSelected] = useState<string | null>(null);
  const [path, setPath] = useState("");
  const [content, setContent] = useState("");
  const [original, setOriginal] = useState("");
  const [builtin, setBuiltin] = useState(false);
  const [adding, setAdding] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [status, setStatus] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const openPath = async (target: string, force = false) => {
    if (!force && content !== original) {
      if (!window.confirm("当前模板有未保存的修改，切换后会丢失，继续？")) return;
    }
    setAdding(false);
    setSelected(target);
    setPath(target);
    setLoading(true);
    setError(null);
    setStatus(null);
    try {
      const detail = await getTemplate(target);
      setContent(detail.content);
      setOriginal(detail.content);
      setBuiltin(detail.builtin);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    let cancelled = false;
    void listTemplates()
      .then((list) => {
        if (cancelled) return;
        setTemplates(list);
        const first = list[0]?.path;
        if (first) void openPath(first, true);
      })
      .catch((reason: unknown) => {
        if (!cancelled) {
          setError(reason instanceof Error ? reason.message : String(reason));
        }
      });
    return () => {
      cancelled = true;
    };
    // 只在打开面板时加载一次
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const refreshList = async (prefer?: string) => {
    const list = await listTemplates();
    setTemplates(list);
    const next = prefer ?? selected ?? list[0]?.path;
    if (next) {
      await openPath(next, true);
    } else {
      setSelected(null);
      setPath("");
      setContent("");
      setOriginal("");
      setBuiltin(false);
    }
  };

  const startAdd = () => {
    if (content !== original && !window.confirm("当前模板有未保存的修改，继续？")) {
      return;
    }
    setAdding(true);
    setSelected(null);
    setPath("");
    setContent("");
    setOriginal("");
    setBuiltin(false);
    setError(null);
    setStatus(null);
  };

  const save = async () => {
    if (!path.trim()) {
      setError("请填写模板路径");
      return;
    }
    if (!content.trim()) {
      setError("模板内容不能为空");
      return;
    }
    setSaving(true);
    setError(null);
    setStatus(null);
    try {
      const detail = await saveTemplate(path.trim(), content);
      setBuiltin(detail.builtin);
      setSelected(detail.path);
      setPath(detail.path);
      setOriginal(content);
      setAdding(false);
      await refreshList(detail.path);
      setStatus("已保存");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  };

  const restore = async () => {
    const target = selected ?? path.trim();
    if (!target) return;
    const label = builtin ? "恢复内置默认模板" : "删除该自定义模板";
    if (!window.confirm(`确定${label}？`)) return;
    try {
      await resetTemplate(target);
      setSelected(null);
      setContent("");
      setOriginal("");
      const list = await listTemplates();
      setTemplates(list);
      const first = list[0]?.path;
      if (first) {
        await openPath(first, true);
      } else {
        setPath("");
        setBuiltin(false);
      }
      setStatus("已恢复");
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  };

  const close = () => {
    if (content !== original && !window.confirm("模板还有未保存的修改，确定关闭？")) {
      return;
    }
    onClose();
  };

  return (
    <div className="modal-backdrop" role="presentation" onMouseDown={close}>
      <div
        className="modal template-manager"
        role="dialog"
        aria-modal="true"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className="modal-header">
          <div>
            <span className="section-kicker">Templates</span>
            <h2>模板管理</h2>
          </div>
          <button
            type="button"
            className="icon-button subtle"
            onClick={close}
            aria-label="关闭"
          >
            <Icon name="close" size={16} />
          </button>
        </div>

        <div className="tm-body">
          <aside className="tm-list">
            <div className="tm-list-head">
              <span>模板文件</span>
              <span className="server-menu-count">{templates.length}</span>
            </div>

            <div className="tm-items">
              {templates.map((template) => (
                <button
                  key={template.path}
                  type="button"
                  className={`tm-item ${selected === template.path ? "active" : ""}`}
                  onClick={() => void openPath(template.path)}
                >
                  <Icon name="layers" size={13} />
                  <span className="tm-item-copy">
                    <strong>{template.path}</strong>
                    <small>
                      {template.kind === "override"
                        ? "已覆盖"
                        : template.kind === "custom"
                          ? "自定义"
                          : "内置"}
                    </small>
                  </span>
                </button>
              ))}
            </div>

            <button type="button" className="button secondary tm-add" onClick={startAdd}>
              <Icon name="plus" size={14} />
              添加模板文件
            </button>
          </aside>

          <section className="tm-editor">
            <label className="field">
              <span>项目内路径</span>
              <input
                className="input mono"
                value={path}
                onChange={(event) => setPath(event.target.value)}
                placeholder="docker/mysql/my.cnf"
                disabled={!adding}
              />
            </label>

            <p className="tm-hint">
              模板内容支持使用 <code>{"{{xxx}}"}</code> 这种语法，以后可以在项目中通过设置值来生成项目自己的配置文件。
            </p>

            <textarea
              className="tm-textarea mono"
              value={content}
              onChange={(event) => setContent(event.target.value)}
              spellCheck={false}
              disabled={loading}
              placeholder={
                adding
                  ? "填写模板内容，可使用 {{xxx}} 变量"
                  : loading
                    ? "正在读取模板…"
                    : "从左侧选择模板"
              }
            />

            {error && <div className="form-error">{error}</div>}
          </section>
        </div>

        <div className="modal-footer tm-footer">
          <span className="muted-caption">
            {adding
              ? "新模板保存后立即参与部署渲染。"
              : selected
                ? builtin
                  ? "内置模板：保存后作为覆盖，可恢复默认。"
                  : "自定义模板：删除后不再上传该文件。"
                : "从左侧选择模板，或添加新模板。"}
            {status && <span className="tm-status"> · {status}</span>}
          </span>
          <div className="heading-actions">
            {selected && (
              <button
                type="button"
                className="button danger"
                onClick={() => void restore()}
                disabled={loading || saving}
              >
                {builtin ? "恢复默认" : "删除"}
              </button>
            )}
            <button type="button" className="button secondary" onClick={close}>
              关闭
            </button>
            <button
              type="button"
              className="button primary"
              onClick={() => void save()}
              disabled={saving || loading}
            >
              <Icon name="check" size={14} />
              {saving ? "保存中…" : "保存"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
