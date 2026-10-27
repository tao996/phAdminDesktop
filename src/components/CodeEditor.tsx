import { useEffect, useRef } from "react";
import { EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import {
  HighlightStyle,
  StreamLanguage,
  syntaxHighlighting,
} from "@codemirror/language";
import { yaml } from "@codemirror/lang-yaml";
import { php } from "@codemirror/lang-php";
import { nginx } from "@codemirror/legacy-modes/mode/nginx";
import { properties } from "@codemirror/legacy-modes/mode/properties";
import { tags } from "@lezer/highlight";
import type { Extension } from "@codemirror/state";

interface CodeEditorProps {
  path: string;
  value: string;
  onChange: (value: string) => void;
}

const highlightStyle = HighlightStyle.define([
  { tag: tags.comment, color: "#6b7a90", fontStyle: "italic" },
  { tag: [tags.keyword, tags.modifier], color: "#c792ea" },
  { tag: [tags.string, tags.special(tags.string)], color: "#a5d6a7" },
  { tag: [tags.number, tags.bool, tags.null], color: "#f78c6c" },
  { tag: [tags.propertyName, tags.attributeName], color: "#82aaff" },
  { tag: [tags.variableName, tags.definition(tags.variableName)], color: "#ffcb6b" },
  { tag: tags.tagName, color: "#f07178" },
  { tag: [tags.operator, tags.punctuation], color: "#89ddff" },
  { tag: [tags.meta, tags.processingInstruction], color: "#c792ea" },
  { tag: tags.invalid, color: "#ff6b7a" },
]);

const theme = EditorView.theme(
  {
    "&": {
      height: "100%",
      backgroundColor: "#0d121b",
      color: "var(--text)",
      fontSize: "11px",
    },
    ".cm-scroller": {
      overflow: "auto",
      fontFamily: "var(--mono)",
      lineHeight: "1.6",
    },
    ".cm-content": {
      caretColor: "var(--accent)",
      padding: "8px 0",
    },
    ".cm-line": {
      padding: "0 10px",
    },
    ".cm-gutters": {
      backgroundColor: "#0d121b",
      color: "#4d5a6e",
      border: "none",
      paddingLeft: "4px",
    },
    ".cm-lineNumbers .cm-gutterElement": {
      minWidth: "28px",
    },
    "&.cm-focused": {
      outline: "none",
    },
  },
  { dark: true },
);

function languageFor(path: string): Extension {
  const name = path.toLowerCase().split("/").pop() ?? "";
  if (name.endsWith(".php")) return php();
  if (name.endsWith(".yaml") || name.endsWith(".yml")) return yaml();
  if (name.endsWith(".conf")) return StreamLanguage.define(nginx);
  if (
    name.endsWith(".ini") ||
    name.endsWith(".cnf") ||
    name === ".env" ||
    name.startsWith(".env.")
  ) {
    return StreamLanguage.define(properties);
  }
  return [];
}

export function CodeEditor({ path, value, onChange }: CodeEditorProps) {
  const host = useRef<HTMLDivElement>(null);
  const changeRef = useRef(onChange);
  changeRef.current = onChange;

  useEffect(() => {
    if (!host.current) return;
    const state = EditorState.create({
      doc: value,
      extensions: [
        lineNumbers(),
        history(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        EditorState.tabSize.of(2),
        languageFor(path),
        syntaxHighlighting(highlightStyle),
        theme,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            changeRef.current(update.state.doc.toString());
          }
        }),
      ],
    });
    const view = new EditorView({ state, parent: host.current });
    return () => view.destroy();
    // 文件切换/重置由父组件 key 重新挂载，这里只初始化一次。
  }, [path]);

  return <div className="code-editor" ref={host} />;
}
