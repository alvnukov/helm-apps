import { Compartment, EditorSelection, EditorState, StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, WidgetType, keymap, lineNumbers } from "@codemirror/view";
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
    "&": { fontSize: `${px}px`, height: "100%", color: "#bcbec4", backgroundColor: "#2b2d30" },
    ".cm-content": { caretColor: "#ced0d6" },
    ".cm-scroller": { fontFamily: "JetBrains Mono, Menlo, Monaco, Consolas, monospace" },
    ".cm-gutters": {
      backgroundColor: "#2b2d30",
      color: "#606366",
      borderRight: "1px solid #3c3f41",
    },
    ".cm-activeLine, .cm-activeLineGutter": { backgroundColor: "#313335" },
    ".cm-selectionLayer .cm-selectionBackground": {
      backgroundColor: "#365880 !important",
    },
    "&.cm-focused .cm-selectionLayer .cm-selectionBackground": {
      backgroundColor: "#365880 !important",
    },
    ".cm-content ::selection": {
      backgroundColor: "#365880 !important",
    },
    ".cm-cursor, .cm-dropCursor": { borderLeftColor: "#ced0d6" },
    ".cm-content .happ-virtual-cursor": {
      display: "inline-block",
      width: "0",
      height: "1.25em",
      marginLeft: "-1px",
      borderLeft: "2px solid rgba(126, 174, 255, 0.96)",
      boxShadow: "0 0 0 1px rgba(126, 174, 255, 0.2)",
      verticalAlign: "text-bottom",
      pointerEvents: "none",
      animation: "happVirtualCursorBlink 1.1s steps(1, end) infinite",
    },
    "@keyframes happVirtualCursorBlink": {
      "0%,48%": { opacity: "1" },
      "49%,100%": { opacity: "0.2" },
    },
    ".cm-foldGutter .cm-gutterElement": { color: "#7b7f86" },
    ".cm-tooltip": {
      backgroundColor: "#3c3f41",
      color: "#d0d2d8",
      border: "1px solid #4e5254",
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

const setVirtualCursorEffect = StateEffect.define();

class VirtualCursorWidget extends WidgetType {
  toDOM() {
    const span = document.createElement("span");
    span.className = "happ-virtual-cursor";
    span.setAttribute("aria-hidden", "true");
    return span;
  }
}

const virtualCursorField = StateField.define({
  create() {
    return Decoration.none;
  },
  update(deco, tr) {
    deco = deco.map(tr.changes);
    for (const effect of tr.effects) {
      if (!effect.is(setVirtualCursorEffect)) continue;
      const value = effect.value;
      if (value == null || Number.isNaN(Number(value))) {
        return Decoration.none;
      }
      const pos = Math.max(0, Math.min(tr.state.doc.length, Number(value)));
      return Decoration.set([
        Decoration.widget({ widget: new VirtualCursorWidget(), side: 1 }).range(pos),
      ]);
    }
    return deco;
  },
  provide: (field) => EditorView.decorations.from(field),
});

function createYamlEditor(el, opts = {}) {
  const value = typeof opts.value === "string" ? opts.value : "";
  const readOnly = !!opts.readOnly;
  const onChange = typeof opts.onChange === "function" ? opts.onChange : null;
  const onSelectionChange =
    typeof opts.onSelectionChange === "function" ? opts.onSelectionChange : null;
  const wrapLines = !!opts.wrapLines;
  const fontSize = Number(opts.fontSize || 14);

  const editableCompartment = new Compartment();
  const wrapCompartment = new Compartment();
  const fontCompartment = new Compartment();

  const updateListener = EditorView.updateListener.of((update) => {
    if (update.docChanged && onChange) {
      onChange(update.state.doc.toString());
    }
    if (update.selectionSet && onSelectionChange) {
      const main = update.state.selection.main;
      onSelectionChange({
        from: main.from,
        to: main.to,
        text: update.state.sliceDoc(main.from, main.to),
      });
    }
  });

  const state = EditorState.create({
    doc: value,
    extensions: [
      lineNumbers(),
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap, ...foldKeymap]),
      yaml(),
      syntaxHighlighting(jetBrainsLikeHighlight),
      virtualCursorField,
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
    setSelection(from, to) {
      const a = Math.max(0, Math.min(view.state.doc.length, Number(from) || 0));
      const b = Math.max(0, Math.min(view.state.doc.length, Number(to) || a));
      view.dispatch({
        selection: EditorSelection.single(a, b),
        scrollIntoView: true,
      });
    },
    setSelections(ranges) {
      const docLen = view.state.doc.length;
      const safeRanges = (Array.isArray(ranges) ? ranges : [])
        .map((r) => {
          const from = Math.max(0, Math.min(docLen, Number(r && r.from)));
          const to = Math.max(0, Math.min(docLen, Number(r && r.to)));
          if (!Number.isFinite(from) || !Number.isFinite(to)) return null;
          return EditorSelection.range(Math.min(from, to), Math.max(from, to));
        })
        .filter(Boolean);
      if (!safeRanges.length) {
        const pos = view.state.selection.main.head;
        view.dispatch({ selection: EditorSelection.single(pos, pos) });
        return;
      }
      view.dispatch({
        selection: EditorSelection.create(safeRanges, 0),
        scrollIntoView: true,
      });
    },
    clearSelections() {
      const pos = view.state.selection.main.head;
      view.dispatch({ selection: EditorSelection.single(pos, pos) });
    },
    setVirtualCursor(pos) {
      const value = Number.isFinite(Number(pos))
        ? Math.max(0, Math.min(view.state.doc.length, Number(pos)))
        : null;
      view.dispatch({ effects: setVirtualCursorEffect.of(value) });
    },
    clearVirtualCursor() {
      view.dispatch({ effects: setVirtualCursorEffect.of(null) });
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
