import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
import {
  HighlightStyle,
  foldAll,
  unfoldAll,
  foldEffect,
  foldGutter,
  foldKeymap,
  indentUnit,
  syntaxHighlighting,
} from "@codemirror/language";
import { tags as t } from "@lezer/highlight";
import { yaml } from "@codemirror/lang-yaml";

if (typeof globalThis !== "undefined") {
  globalThis.__happCmEntryReached = true;
}

function countIndent(text) {
  let i = 0;
  while (i < text.length && text.charCodeAt(i) === 32) i += 1;
  return i;
}

function foldByIndent(view, level) {
  if (!view || !view.state) return;
  const doc = view.state.doc;
  const threshold = Math.max(0, Number(level || 1) - 1);
  const effects = [];

  for (let i = 1; i <= doc.lines; i += 1) {
    const line = doc.line(i);
    const raw = line.text;
    if (!raw.trim() || raw.trimStart().startsWith("#")) continue;

    const curIndent = countIndent(raw);
    let nextLine = i + 1;
    while (nextLine <= doc.lines) {
      const n = doc.line(nextLine);
      if (n.text.trim()) break;
      nextLine += 1;
    }
    if (nextLine > doc.lines) continue;
    const nline = doc.line(nextLine);
    const nextIndent = countIndent(nline.text);
    if (nextIndent <= curIndent) continue;

    const depth = Math.floor(curIndent / 2);
    if (depth < threshold) continue;

    let end = nextLine;
    for (let j = nextLine + 1; j <= doc.lines; j += 1) {
      const test = doc.line(j);
      if (!test.text.trim()) {
        end = j;
        continue;
      }
      if (countIndent(test.text) <= curIndent) break;
      end = j;
    }

    const endLine = doc.line(end);
    if (endLine.to > line.to) {
      effects.push(foldEffect.of({ from: line.to, to: endLine.to }));
    }
  }

  if (effects.length) view.dispatch({ effects });
}

function makeTheme(fontSizePx) {
  const px = Number.isFinite(Number(fontSizePx)) ? Number(fontSizePx) : 14;
  return EditorView.theme({
    "&": { fontSize: `${px}px`, height: "100%", color: "#c8c8c8", backgroundColor: "#1f2128" },
    ".cm-content": { caretColor: "#d0d0d0" },
    ".cm-scroller": { fontFamily: "JetBrains Mono, Menlo, Monaco, Consolas, monospace" },
    ".cm-gutters": {
      backgroundColor: "#1c1e24",
      color: "#5f6774",
      borderRight: "1px solid #2b2f38",
    },
    ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "#252932" },
    ".cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection": {
      backgroundColor: "#2f3f68",
    },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "#c8c8c8" },
    ".cm-foldGutter .cm-gutterElement": { color: "#606978" },
    ".cm-tooltip": {
      backgroundColor: "#2b2f38",
      color: "#c8c8c8",
      border: "1px solid #3b4252",
    },
  });
}

const jetBrainsLikeHighlight = HighlightStyle.define([
  { tag: [t.propertyName, t.attributeName], color: "#d19a66" },
  { tag: [t.keyword, t.operatorKeyword], color: "#cc7832" },
  { tag: [t.string, t.special(t.string)], color: "#98c379" },
  { tag: [t.number], color: "#d19a66" },
  { tag: [t.bool, t.null], color: "#c678dd" },
  { tag: [t.comment], color: "#6a8f74", fontStyle: "italic" },
  { tag: [t.variableName, t.typeName, t.className], color: "#c8c8c8" },
  { tag: [t.atom, t.labelName], color: "#e5c07b" },
  { tag: [t.punctuation, t.bracket], color: "#9aa5b1" },
]);

function createYamlEditor(el, opts = {}) {
  const value = typeof opts.value === "string" ? opts.value : "";
  const readOnly = !!opts.readOnly;
  const onChange = typeof opts.onChange === "function" ? opts.onChange : null;
  const wrapLines = !!opts.wrapLines;
  const fontSize = Number(opts.fontSize || 14);

  const editableCompartment = new Compartment();
  const wrapCompartment = new Compartment();
  const fontCompartment = new Compartment();

  const updateListener = EditorView.updateListener.of((update) => {
    if (!update.docChanged || !onChange) return;
    onChange(update.state.doc.toString());
  });

  const state = EditorState.create({
    doc: value,
    extensions: [
      lineNumbers(),
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap, ...foldKeymap]),
      yaml(),
      syntaxHighlighting(jetBrainsLikeHighlight),
      foldGutter(),
      indentUnit.of("  "),
      updateListener,
      editableCompartment.of(EditorView.editable.of(!readOnly)),
      wrapCompartment.of(wrapLines ? EditorView.lineWrapping : []),
      fontCompartment.of(makeTheme(fontSize)),
    ],
  });

  const view = new EditorView({ state, parent: el });

  return {
    view,
    getValue() {
      return view.state.doc.toString();
    },
    setValue(next) {
      const text = typeof next === "string" ? next : "";
      const cur = view.state.doc.toString();
      if (cur === text) return;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
      });
    },
    setReadOnly(next) {
      view.dispatch({
        effects: editableCompartment.reconfigure(EditorView.editable.of(!next)),
      });
    },
    setWrapLines(next) {
      view.dispatch({
        effects: wrapCompartment.reconfigure(next ? EditorView.lineWrapping : []),
      });
    },
    setFontSize(next) {
      view.dispatch({ effects: fontCompartment.reconfigure(makeTheme(next)) });
    },
    focus() {
      view.focus();
    },
    foldAll() {
      foldAll(view);
    },
    unfoldAll() {
      unfoldAll(view);
    },
    foldLevel(level) {
      unfoldAll(view);
      foldByIndent(view, level);
    },
    destroy() {
      view.destroy();
    },
  };
}

const happCodeMirrorApi = {
  createYamlEditor,
};

if (typeof globalThis !== "undefined") {
  globalThis.__happCmBeforeAssign = true;
}

let happRoot = null;
try {
  happRoot = Function("return this")();
} catch (_) {}
if (happRoot) {
  happRoot.HappCodeMirror = happCodeMirrorApi;
}
if (typeof self !== "undefined") {
  self.HappCodeMirror = happCodeMirrorApi;
}
if (typeof globalThis !== "undefined") {
  globalThis.HappCodeMirror = happCodeMirrorApi;
}
if (typeof window !== "undefined") {
  window.HappCodeMirror = happCodeMirrorApi;
}
if (typeof globalThis !== "undefined") {
  globalThis.__happCmAfterAssign = !!globalThis.HappCodeMirror;
}
