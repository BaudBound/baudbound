import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import {
  StreamLanguage,
  HighlightStyle,
  bracketMatching,
  foldGutter,
  syntaxHighlighting,
} from "@codemirror/language";
import { highlightSelectionMatches, searchKeymap } from "@codemirror/search";
import { EditorState } from "@codemirror/state";
import {
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view";
import { toml } from "@codemirror/legacy-modes/mode/toml";
import CodeMirror from "@uiw/react-codemirror";
import { tags } from "@lezer/highlight";
import { useEffect, useRef } from "react";

import { cn } from "@/lib/utils";

const tomlExtensions = [
  lineNumbers(),
  highlightActiveLineGutter(),
  history(),
  foldGutter(),
  bracketMatching(),
  highlightActiveLine(),
  highlightSelectionMatches(),
  StreamLanguage.define(toml),
  EditorState.tabSize.of(2),
  keymap.of([indentWithTab, ...defaultKeymap, ...historyKeymap, ...searchKeymap]),
  syntaxHighlighting(
    HighlightStyle.define([
      { tag: tags.comment, color: "#727d95", fontStyle: "italic" },
      { tag: [tags.bool, tags.number], color: "#f5a623" },
      { tag: tags.string, color: "#3ecf8e" },
      { tag: [tags.keyword, tags.atom], color: "#5b8af5" },
      { tag: [tags.propertyName, tags.definitionKeyword], color: "#d8deec" },
      { tag: tags.operator, color: "#e62d3e" },
    ]),
  ),
];

const baudboundEditorTheme = EditorView.theme(
  {
    "&": {
      backgroundColor: "#090b10",
      color: "#d8deec",
      fontSize: "13px",
      height: "100%",
    },
    "&.cm-editor": {
      backgroundColor: "#090b10",
      height: "100%",
      minHeight: "0",
      userSelect: "text",
    },
    "&.cm-focused": {
      outline: "none",
    },
    ".cm-activeLine": {
      backgroundColor: "rgba(23, 27, 39, 0.72)",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "rgba(23, 27, 39, 0.72)",
      color: "#d8deec",
    },
    ".cm-content": {
      backgroundColor: "#090b10",
      caretColor: "#d8deec",
      color: "#d8deec",
      minHeight: "100%",
      padding: "14px 0",
      userSelect: "text",
    },
    ".cm-cursor": {
      borderLeftColor: "#e62d3e",
      borderLeftWidth: "2px",
    },
    ".cm-foldGutter .cm-gutterElement": {
      color: "#5b6478",
    },
    ".cm-gutters": {
      backgroundColor: "#0d1017",
      borderRight: "1px solid #202635",
      color: "#727d95",
      minWidth: "48px",
    },
    ".cm-gutterElement": {
      padding: "0 10px 0 8px",
    },
    ".cm-line": {
      color: "#d8deec",
      padding: "0 14px",
      userSelect: "text",
    },
    ".cm-matchingBracket, .cm-nonmatchingBracket": {
      backgroundColor: "rgba(91, 138, 245, 0.18)",
      outline: "1px solid rgba(91, 138, 245, 0.35)",
    },
    ".cm-panels": {
      backgroundColor: "#0d1017",
      borderColor: "#202635",
      color: "#d8deec",
    },
    ".cm-panels button": {
      backgroundColor: "#171b27",
      border: "1px solid #202635",
      borderRadius: "4px",
      color: "#d8deec",
      padding: "2px 8px",
    },
    ".cm-panels input": {
      backgroundColor: "#080b12",
      border: "1px solid #202635",
      borderRadius: "4px",
      color: "#d8deec",
      outline: "none",
      padding: "2px 6px",
    },
    ".cm-scroller": {
      backgroundColor: "#090b10",
      fontFamily:
        "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', monospace",
      height: "100%",
      lineHeight: "21px",
      minHeight: "0",
      overflow: "auto",
      scrollbarColor: "#3a4255 transparent",
      userSelect: "text",
    },
    ".cm-scroller::-webkit-scrollbar": {
      height: "8px",
      width: "8px",
    },
    ".cm-scroller::-webkit-scrollbar-thumb": {
      backgroundColor: "#3a4255",
      borderRadius: "999px",
    },
    "& ::selection": {
      backgroundColor: "rgba(91, 138, 245, 0.42)",
    },
    ".cm-content ::selection": {
      backgroundColor: "rgba(91, 138, 245, 0.42)",
    },
    ".cm-selectionBackground": {
      backgroundColor: "rgba(91, 138, 245, 0.42) !important",
    },
    "&.cm-focused .cm-selectionBackground": {
      backgroundColor: "rgba(91, 138, 245, 0.52) !important",
    },
    "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground": {
      backgroundColor: "rgba(91, 138, 245, 0.52) !important",
    },
    ".cm-tooltip": {
      backgroundColor: "#0d1017",
      border: "1px solid #202635",
      borderRadius: "6px",
      color: "#d8deec",
    },
    ".cm-tooltip-autocomplete ul li[aria-selected]": {
      backgroundColor: "#171b27",
      color: "#d8deec",
    },
  },
  { dark: true },
);

export function TomlCodeEditor({
  disabled = false,
  onChange,
  value,
}: {
  disabled?: boolean;
  onChange: (value: string) => void;
  value: string;
}) {
  const editorViewRef = useRef<EditorView | null>(null);
  const shellRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    const shell = shellRef.current;
    if (!shell) return;

    const resizeObserver = new ResizeObserver(() => {
      editorViewRef.current?.requestMeasure();
    });
    resizeObserver.observe(shell);
    return () => resizeObserver.disconnect();
  }, []);

  return (
    <div className="grid gap-2">
      <div
        className={cn(
          "h-[560px] min-h-[180px] resize-y overflow-hidden rounded-md border border-border bg-background",
          disabled && "opacity-60",
        )}
        ref={shellRef}
      >
        <CodeMirror
          basicSetup={false}
          className="h-full"
          editable={!disabled}
          extensions={tomlExtensions}
          height="100%"
          onChange={onChange}
          onCreateEditor={(view) => {
            editorViewRef.current = view;
          }}
          theme={baudboundEditorTheme}
          value={value}
        />
      </div>
      <div className="text-xs text-muted-foreground">
        Tab indents, Shift+Tab outdents. Search, selection, line numbers, and scrolling are handled by CodeMirror.
      </div>
    </div>
  );
}
