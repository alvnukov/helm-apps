import { execFile } from "node:child_process";
import { readFileSync } from "node:fs";
import { access, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import * as path from "node:path";
import { promisify } from "node:util";
import * as vscode from "vscode";
import * as YAML from "yaml";

import { buildFieldDocMarkdownLocalized, findFieldDoc, findKeyPathAtPosition } from "./hover/fieldHover";
import { extractLocalIncludeBlock, trimPreview } from "./hover/includeHover";
import { buildDependencyGraphModel } from "./language/dependencyGraph";
import { buildHelmAppsDocumentSymbols } from "./language/documentSymbols";
import { analyzeIncludes } from "./language/includeAnalysis";
import { collectSymbolOccurrences, findSymbolAtPosition } from "./language/symbols";
import { discoverEnvironments, findAppScopeAtLine, resolveEntityWithIncludes, resolveEnvMaps, type EnvironmentDiscovery } from "./preview/includeResolver";
import { expandValuesWithFileIncludes, type IncludeDefinition } from "./loader/fileIncludes";
import { extractAppChildToGlobalInclude, safeRenameAppKey } from "./refactor/appRefactor";
import { ValuesStructureProvider } from "./structure/valuesTreeProvider";

const execFileAsync = promisify(execFile);
let previewPanel: vscode.WebviewPanel | undefined;
let previewMessageSubscription: vscode.Disposable | undefined;
const includeDiagnostics = vscode.languages.createDiagnosticCollection("helm-apps.includes");
const semanticDiagnostics = vscode.languages.createDiagnosticCollection("helm-apps.semantic");
let completionSchemaCache: JsonSchema | null = null;

type JsonSchema = {
  $ref?: string;
  type?: string | string[];
  enum?: unknown[];
  description?: string;
  default?: unknown;
  properties?: Record<string, JsonSchema>;
  patternProperties?: Record<string, JsonSchema>;
  additionalProperties?: boolean | JsonSchema;
  oneOf?: JsonSchema[];
  anyOf?: JsonSchema[];
  allOf?: JsonSchema[];
  items?: JsonSchema;
};

interface PreviewOptions {
  env: string;
  applyIncludes: boolean;
  applyEnvResolution: boolean;
}

interface LibrarySettingDef {
  key: string;
  path: string[];
  title: string;
  titleRu: string;
  description: string;
  descriptionRu: string;
  enabledHelp: string;
  enabledHelpRu: string;
  disabledHelp: string;
  disabledHelpRu: string;
}

const LIBRARY_SETTINGS: LibrarySettingDef[] = [
  {
    key: "validation.strict",
    path: ["global", "validation", "strict"],
    title: "Strict validation",
    titleRu: "Строгая валидация",
    description: "Enables strict contract checks for unsupported keys.",
    descriptionRu: "Включает строгие контрактные проверки для неподдерживаемых ключей.",
    enabledHelp: "Validation fails fast on unsupported/ambiguous keys. Safer, but stricter for legacy values.",
    enabledHelpRu: "Валидация сразу падает на неподдерживаемых/неоднозначных ключах. Безопаснее, но строже для legacy values.",
    disabledHelp: "Validation remains compatible-first and allows legacy shapes where possible.",
    disabledHelpRu: "Валидация работает в режиме совместимости и по возможности пропускает legacy-формы.",
  },
  {
    key: "validation.allowNativeListsInBuiltInListFields",
    path: ["global", "validation", "allowNativeListsInBuiltInListFields"],
    title: "Allow native lists in built-in list fields",
    titleRu: "Разрешить native list в встроенных list-полях",
    description: "Allows YAML native lists in selected built-in list fields (migration mode).",
    descriptionRu: "Разрешает YAML native list в части встроенных list-полей (режим миграции).",
    enabledHelp: "Native YAML lists are accepted in selected built-in list fields for migration.",
    enabledHelpRu: "Native YAML list разрешены в выбранных встроенных list-полях для миграции.",
    disabledHelp: "Use library-preferred YAML block strings for list-like fields.",
    disabledHelpRu: "Используйте рекомендуемые библиотекой YAML block string для list-полей.",
  },
  {
    key: "validation.validateFlValueTemplates",
    path: ["global", "validation", "validateFlValueTemplates"],
    title: "Validate fl.value template delimiters",
    titleRu: "Проверять шаблоны fl.value",
    description: "Checks '{{' / '}}' balance for string templates rendered via fl.value.",
    descriptionRu: "Проверяет баланс '{{' / '}}' в строковых шаблонах, рендеримых через fl.value.",
    enabledHelp: "Template delimiter balance is checked before render for fl.value strings.",
    enabledHelpRu: "Перед рендером проверяется баланс шаблонных скобок в fl.value-строках.",
    disabledHelp: "No delimiter pre-check; malformed templates may fail later at render time.",
    disabledHelpRu: "Предпроверка не выполняется; ошибки шаблонов могут проявиться позже при рендере.",
  },
  {
    key: "labels.addEnv",
    path: ["global", "labels", "addEnv"],
    title: "Add environment label",
    titleRu: "Добавлять label окружения",
    description: "Adds app.kubernetes.io/environment=<current env> label to rendered resources.",
    descriptionRu: "Добавляет label app.kubernetes.io/environment=<текущее окружение> в ресурсы.",
    enabledHelp: "Rendered resources include app.kubernetes.io/environment label.",
    enabledHelpRu: "В отрендеренных ресурсах появляется label app.kubernetes.io/environment.",
    disabledHelp: "Environment label is not added automatically by the library.",
    disabledHelpRu: "Label окружения библиотека автоматически не добавляет.",
  },
  {
    key: "deploy.enabled",
    path: ["global", "deploy", "enabled"],
    title: "Enable release matrix auto-app activation",
    titleRu: "Включить автоактивацию release matrix",
    description: "Auto-enables apps if version is found in global.releases for selected release.",
    descriptionRu: "Автовключает app, если версия найдена в global.releases для выбранного релиза.",
    enabledHelp: "Apps can be enabled automatically from global.releases and deploy release mapping.",
    enabledHelpRu: "Приложения могут включаться автоматически по global.releases и deploy release mapping.",
    disabledHelp: "Only explicit app.enabled controls app activation.",
    disabledHelpRu: "Активация приложения определяется только явным app.enabled.",
  },
  {
    key: "deploy.annotateAllWithRelease",
    path: ["global", "deploy", "annotateAllWithRelease"],
    title: "Annotate all resources with release",
    titleRu: "Аннотировать все ресурсы релизом",
    description: "Adds helm-apps/release annotation to all resources of current deploy release.",
    descriptionRu: "Добавляет аннотацию helm-apps/release ко всем ресурсам текущего deploy-релиза.",
    enabledHelp: "All rendered resources are annotated with helm-apps/release.",
    enabledHelpRu: "Все отрендеренные ресурсы получают аннотацию helm-apps/release.",
    disabledHelp: "Release annotation is not forced globally.",
    disabledHelpRu: "Глобальное принудительное добавление release-аннотации отключено.",
  },
];

export function activate(context: vscode.ExtensionContext): void {
  const valuesStructure = new ValuesStructureProvider();

  context.subscriptions.push(
    vscode.window.registerTreeDataProvider("helmAppsValuesStructure", valuesStructure),
  );

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      valuesStructure.setDocument(editor?.document);
    }),
  );

  context.subscriptions.push(
    vscode.workspace.onDidChangeTextDocument((event) => {
      const active = vscode.window.activeTextEditor?.document;
      if (!active || event.document.uri.toString() !== active.uri.toString()) {
        return;
      }
      valuesStructure.setDocument(active);
      void refreshDiagnostics(active);
    }),
  );
  context.subscriptions.push(includeDiagnostics);
  context.subscriptions.push(semanticDiagnostics);
  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      if (!editor) {
        includeDiagnostics.clear();
        semanticDiagnostics.clear();
        return;
      }
      void refreshDiagnostics(editor.document);
    }),
  );

  context.subscriptions.push(
    vscode.languages.registerDefinitionProvider({ language: "yaml" }, {
      provideDefinition: async (document, position) => await provideDefinition(document, position),
    }),
  );
  context.subscriptions.push(
    vscode.languages.registerReferenceProvider({ language: "yaml" }, {
      provideReferences: async (document, position) => await provideReferences(document, position),
    }),
  );
  context.subscriptions.push(
    vscode.languages.registerRenameProvider({ language: "yaml" }, {
      provideRenameEdits: async (document, position, newName) => await provideRenameEdits(document, position, newName),
      prepareRename: async (document, position) => await prepareRename(document, position),
    }),
  );
  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider({ language: "yaml" }, {
      provideCompletionItems: async (document, position) => await provideCompletionItems(document, position),
    }, " ", ":", "-"),
  );
  context.subscriptions.push(
    vscode.languages.registerCodeActionsProvider({ language: "yaml" }, {
      provideCodeActions: async (document, range, codeContext) => await provideCodeActions(document, range, codeContext),
    }),
  );
  context.subscriptions.push(
    vscode.languages.registerDocumentSymbolProvider({ language: "yaml" }, {
      provideDocumentSymbols: async (document) => {
        if (!(await isHelmAppsValuesDocument(document))) {
          return [];
        }
        return buildHelmAppsDocumentSymbols(document);
      },
    }),
  );
  context.subscriptions.push(
    vscode.languages.registerHoverProvider({ language: "yaml" }, {
      provideHover: async (document, position) => await provideIncludeHover(document, position),
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.goToIncludeDefinition", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }
      const def = await provideIncludeDefinition(editor.document, editor.selection.active);
      if (!def) {
        void vscode.window.showWarningMessage("No include definition found under cursor");
        return;
      }
      const location = Array.isArray(def) ? def[0] : def;
      const targetDoc = await vscode.workspace.openTextDocument(location.uri);
      const targetEditor = await vscode.window.showTextDocument(targetDoc, { preview: false });
      targetEditor.selection = new vscode.Selection(location.range.start, location.range.start);
      targetEditor.revealRange(location.range, vscode.TextEditorRevealType.InCenter);
    }),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.findUsages", async () => {
      await vscode.commands.executeCommand("editor.action.referenceSearch.trigger");
    }),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.pasteAsHelmApps", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }
      await pasteClipboardAsHelmApps(editor);
    }),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.openDependencyGraph", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }
      if (!(await isHelmAppsValuesDocument(editor.document))) {
        void vscode.window.showWarningMessage("Open helm-apps values.yaml to view dependency graph.");
        return;
      }
      await openDependencyGraphPanel(editor.document.getText());
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.revealValuesNode", async (uri: vscode.Uri, line: number) => {
      if (!uri) {
        return;
      }
      const doc = await vscode.workspace.openTextDocument(uri);
      const editor = await vscode.window.showTextDocument(doc, { preview: false });
      const pos = new vscode.Position(line, 0);
      editor.selection = new vscode.Selection(pos, pos);
      editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenter);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.configureSchema", async () => {
      await configureSchema(context);
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.validateCurrentFile", async () => {
      await validateCurrentFile();
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.previewResolvedEntity", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }

      const text = editor.document.getText();
      const scope = findAppScopeAtLine(text, editor.selection.active.line);
      if (!scope) {
        void vscode.window.showWarningMessage("Place cursor inside <group>.<app> block");
        return;
      }

      try {
        const loaded = await loadExpandedValues(editor.document);
        const values = loaded.values;
        const envDiscovery = discoverEnvironments(values);
        const defaultEnv = detectDefaultEnv(values, envDiscovery);
        const options: PreviewOptions = {
          env: defaultEnv,
          applyIncludes: true,
          applyEnvResolution: true,
        };
        showEntityPreview(scope.group, scope.app, values, envDiscovery, options, loaded.missingFiles.map((m) => m.rawPath));
      } catch (err) {
        void vscode.window.showErrorMessage(`helm-apps preview failed: ${extractErrorMessage(err)}`);
      }
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.extractToGlobalInclude", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }

      const includeName = await vscode.window.showInputBox({
        prompt: "Include profile name (global._includes.<name>)",
        placeHolder: "apps-common",
        validateInput: (v) => (/^[a-z0-9][a-z0-9.-]*$/.test(v) ? null : "Use ^[a-z0-9][a-z0-9.-]*$"),
      });
      if (!includeName) {
        return;
      }

      await rewriteEditorText(editor, (text) =>
        extractAppChildToGlobalInclude(text, editor.selection.active.line, includeName),
      );
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.safeRenameAppKey", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }

      const newKey = await vscode.window.showInputBox({
        prompt: "New app key",
        placeHolder: "api-v2",
        validateInput: (v) => (/^[a-z0-9][a-z0-9.-]*$/.test(v) ? null : "Use ^[a-z0-9][a-z0-9.-]*$"),
      });
      if (!newKey) {
        return;
      }

      await rewriteEditorText(editor, (text) =>
        safeRenameAppKey(text, editor.selection.active.line, newKey),
      );
    }),
  );

  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.openLibrarySettings", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }
      if (!(await isHelmAppsValuesDocument(editor.document))) {
        void vscode.window.showWarningMessage("Open helm-apps values.yaml to edit library settings.");
        return;
      }
      await openLibrarySettingsPanel(editor);
    }),
  );
  context.subscriptions.push(
    vscode.commands.registerCommand("helm-apps.generateLibraryHelp", async () => {
      const editor = vscode.window.activeTextEditor;
      if (!editor) {
        return;
      }
      if (!(await isHelmAppsValuesDocument(editor.document))) {
        void vscode.window.showWarningMessage("Open helm-apps values.yaml to generate settings help.");
        return;
      }
      const ru = vscode.env.language.toLowerCase().startsWith("ru");
      const values = parseValuesObject(editor.document.getText());
      const current = new Map<string, boolean>();
      for (const setting of LIBRARY_SETTINGS) {
        current.set(setting.key, readBooleanByPath(values, setting.path));
      }
      await openLibrarySettingsHelp(current, ru, editor.document.uri.fsPath);
    }),
  );

  void configureSchema(context, true);
  valuesStructure.setDocument(vscode.window.activeTextEditor?.document);
  void refreshDiagnostics(vscode.window.activeTextEditor?.document);
}

export function deactivate(): void {}

async function configureSchema(context: vscode.ExtensionContext, silent = false): Promise<void> {
  const manualFileMatch = vscode.workspace.getConfiguration("helm-apps").get<string[]>("schemaFileMatch", []);
  const fileMatch = manualFileMatch.length > 0 ? manualFileMatch : await discoverHelmAppsSchemaTargets();

  const yamlConfig = vscode.workspace.getConfiguration("yaml");
  const current = (yamlConfig.get<Record<string, string[]>>("schemas") ?? {}) as Record<string, string[]>;

  const schemaUri = vscode.Uri.file(path.join(context.extensionPath, "schemas", "values.schema.json")).toString();
  const next = { ...current };
  if (fileMatch.length > 0) {
    next[schemaUri] = fileMatch;
  } else {
    delete next[schemaUri];
  }

  await yamlConfig.update("schemas", next, vscode.ConfigurationTarget.Workspace);
  await configureYamlHoverBehavior();

  if (!silent) {
    void vscode.window.showInformationMessage(
      fileMatch.length > 0
        ? `helm-apps schema configured for ${fileMatch.length} file(s)`
        : "helm-apps schema mapping cleared (no helm-apps charts detected)",
    );
  }
}

async function configureYamlHoverBehavior(): Promise<void> {
  const disableSchemaHover = vscode.workspace
    .getConfiguration("helm-apps")
    .get<boolean>("disableYamlSchemaHover", true);
  if (!disableSchemaHover) {
    return;
  }
  const yamlConfig = vscode.workspace.getConfiguration("yaml");
  const current = yamlConfig.get<boolean>("hover");
  if (current !== false) {
    await yamlConfig.update("hover", false, vscode.ConfigurationTarget.Workspace);
  }
}

async function openLibrarySettingsPanel(editor: vscode.TextEditor): Promise<void> {
  const ru = vscode.env.language.toLowerCase().startsWith("ru");
  const panel = vscode.window.createWebviewPanel(
    "helmAppsLibrarySettings",
    ru ? "helm-apps: настройки библиотеки" : "helm-apps: library settings",
    vscode.ViewColumn.Beside,
    { enableScripts: true },
  );

  const values = parseValuesObject(editor.document.getText());
  const current = new Map<string, boolean>();
  for (const setting of LIBRARY_SETTINGS) {
    current.set(setting.key, readBooleanByPath(values, setting.path));
  }
  panel.webview.html = renderLibrarySettingsHtml(current, ru);

  panel.webview.onDidReceiveMessage(async (msg: unknown) => {
    if (!msg || typeof msg !== "object") {
      return;
    }
    const payload = msg as { type?: string; values?: Record<string, boolean> };
    if (payload.type === "close") {
      panel.dispose();
      return;
    }
    if (payload.type === "generateHelp" && payload.values) {
      await openLibrarySettingsHelp(new Map(Object.entries(payload.values)), ru, editor.document.uri.fsPath);
      return;
    }
    if (payload.type !== "applySettings" || !payload.values) {
      return;
    }
    try {
      const text = editor.document.getText();
      const updated = applyLibrarySettingsToValues(text, payload.values);
      const fullRange = new vscode.Range(
        editor.document.positionAt(0),
        editor.document.positionAt(text.length),
      );
      await editor.edit((builder) => builder.replace(fullRange, updated));
      void vscode.window.showInformationMessage(ru ? "Настройки библиотеки обновлены в values.yaml" : "Library settings updated in values.yaml");
      panel.dispose();
    } catch (err) {
      void vscode.window.showErrorMessage(`${ru ? "Не удалось применить настройки" : "Failed to apply settings"}: ${extractErrorMessage(err)}`);
    }
  });
}

function parseValuesObject(text: string): Record<string, unknown> {
  try {
    const parsed = YAML.parse(text) as unknown;
    return isMap(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function readBooleanByPath(root: Record<string, unknown>, pathParts: string[]): boolean {
  let current: unknown = root;
  for (const part of pathParts) {
    if (!isMap(current) || !Object.prototype.hasOwnProperty.call(current, part)) {
      return false;
    }
    current = current[part];
  }
  return current === true;
}

function applyLibrarySettingsToValues(text: string, selected: Record<string, boolean>): string {
  const doc = YAML.parseDocument(text);
  for (const setting of LIBRARY_SETTINGS) {
    const enabled = selected[setting.key] === true;
    doc.setIn(setting.path, enabled);
  }
  return String(doc);
}

function renderLibrarySettingsHtml(current: Map<string, boolean>, ru: boolean): string {
  const rows = LIBRARY_SETTINGS
    .map((s) => {
      const checked = current.get(s.key) ? "checked" : "";
      const title = ru ? s.titleRu : s.title;
      const description = ru ? s.descriptionRu : s.description;
      return `<label class="row">
        <input type="checkbox" data-key="${escapeHtml(s.key)}" ${checked} />
        <span class="meta">
          <span class="title">${escapeHtml(title)}</span>
          <span class="desc">${escapeHtml(description)}</span>
          <code>${escapeHtml(s.path.join("."))}</code>
        </span>
      </label>`;
    })
    .join("");

  const header = ru ? "Настройки библиотеки" : "Library Settings";
  const sub = ru
    ? "Выберите опции и примените их в values.yaml (блок global.*)."
    : "Choose options and apply them into values.yaml (global.* section).";
  const apply = ru ? "Применить в values.yaml" : "Apply to values.yaml";
  const genHelp = ru ? "Сгенерировать help" : "Generate help";
  const cancel = ru ? "Закрыть" : "Close";

  return `<!doctype html>
<html lang="${ru ? "ru" : "en"}">
<head>
  <meta charset="UTF-8"/>
  <meta name="viewport" content="width=device-width, initial-scale=1.0"/>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; padding: 14px; color: var(--vscode-foreground); }
    h2 { margin: 0 0 6px; }
    .sub { opacity: .85; margin-bottom: 14px; }
    .list { display: grid; gap: 10px; }
    .row { display: grid; grid-template-columns: auto 1fr; gap: 10px; align-items: start; border: 1px solid var(--vscode-panel-border); border-radius: 8px; padding: 10px; }
    .meta { display: grid; gap: 4px; }
    .title { font-weight: 600; }
    .desc { opacity: .9; }
    code { opacity: .8; }
    .actions { margin-top: 14px; display: flex; gap: 8px; }
    button { border: 1px solid var(--vscode-button-border, transparent); border-radius: 6px; padding: 6px 12px; cursor: pointer; }
    button.primary { background: var(--vscode-button-background); color: var(--vscode-button-foreground); }
    button.secondary { background: var(--vscode-editorWidget-background); color: var(--vscode-foreground); }
  </style>
</head>
<body>
  <h2>${escapeHtml(header)}</h2>
  <div class="sub">${escapeHtml(sub)}</div>
  <div class="list">${rows}</div>
  <div class="actions">
    <button id="apply" class="primary">${escapeHtml(apply)}</button>
    <button id="help" class="secondary">${escapeHtml(genHelp)}</button>
    <button id="cancel" class="secondary">${escapeHtml(cancel)}</button>
  </div>
  <script>
    const vscode = acquireVsCodeApi();
    const collectValues = () => {
      const values = {};
      document.querySelectorAll("input[data-key]").forEach((el) => {
        values[el.dataset.key] = el.checked;
      });
      return values;
    };
    document.getElementById("apply").addEventListener("click", () => {
      vscode.postMessage({ type: "applySettings", values: collectValues() });
    });
    document.getElementById("help").addEventListener("click", () => {
      vscode.postMessage({ type: "generateHelp", values: collectValues() });
    });
    document.getElementById("cancel").addEventListener("click", () => {
      vscode.postMessage({ type: "close" });
    });
  </script>
</body>
</html>`;
}

async function openLibrarySettingsHelp(current: Map<string, boolean>, ru: boolean, filePath: string): Promise<void> {
  const statusEnabled = ru ? "включено" : "enabled";
  const statusDisabled = ru ? "выключено" : "disabled";
  const title = ru ? "# Настройки библиотеки helm-apps" : "# helm-apps Library Settings";
  const source = ru ? `Файл: \`${filePath}\`` : `File: \`${filePath}\``;

  const lines: string[] = [title, "", source, ""];
  for (const setting of LIBRARY_SETTINGS) {
    const enabled = current.get(setting.key) === true;
    const sTitle = ru ? setting.titleRu : setting.title;
    const sDesc = ru ? setting.descriptionRu : setting.description;
    const sEnabledHelp = ru ? setting.enabledHelpRu : setting.enabledHelp;
    const sDisabledHelp = ru ? setting.disabledHelpRu : setting.disabledHelp;
    lines.push(`## ${sTitle}`);
    lines.push("");
    lines.push(`- ${ru ? "Путь" : "Path"}: \`${setting.path.join(".")}\``);
    lines.push(`- ${ru ? "Статус" : "Status"}: **${enabled ? statusEnabled : statusDisabled}**`);
    lines.push(`- ${ru ? "Что это" : "What it does"}: ${sDesc}`);
    lines.push(`- ${ru ? "Эффект сейчас" : "Current effect"}: ${enabled ? sEnabledHelp : sDisabledHelp}`);
    lines.push("");
  }

  const doc = await vscode.workspace.openTextDocument({
    language: "markdown",
    content: lines.join("\n"),
  });
  await vscode.window.showTextDocument(doc, { preview: false, viewColumn: vscode.ViewColumn.Beside });
}

async function openDependencyGraphPanel(valuesText: string): Promise<void> {
  const panel = vscode.window.createWebviewPanel(
    "helmAppsDependencyGraph",
    "helm-apps: dependency graph",
    vscode.ViewColumn.Beside,
    { enableScripts: false },
  );
  const model = buildDependencyGraphModel(valuesText);
  panel.webview.html = renderDependencyGraphHtml(model);
}

function renderDependencyGraphHtml(model: {
  apps: Array<{ group: string; app: string; includes: string[] }>;
  includes: string[];
  includeFiles: string[];
}): string {
  const includeSet = new Set(model.includes);
  const appRows = model.apps
    .map((a) => {
      const links = a.includes.length === 0
        ? `<span class="muted">no includes</span>`
        : a.includes.map((inc) => {
          const cls = includeSet.has(inc) ? "ok" : "warn";
          const suffix = includeSet.has(inc) ? "" : " (unresolved)";
          return `<span class="chip ${cls}">${escapeHtml(inc)}${suffix}</span>`;
        }).join(" ");
      return `<tr><td><code>${escapeHtml(a.group)}</code></td><td><code>${escapeHtml(a.app)}</code></td><td>${links}</td></tr>`;
    })
    .join("");
  const includeRows = model.includes.map((i) => `<li><code>${escapeHtml(i)}</code></li>`).join("");
  const fileRows = model.includeFiles.map((f) => `<li><code>${escapeHtml(f)}</code></li>`).join("");

  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; padding: 14px; color: var(--vscode-foreground); }
    h2 { margin: 0 0 10px; }
    .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; margin-bottom: 12px; }
    .card { border: 1px solid var(--vscode-panel-border); border-radius: 8px; padding: 10px; }
    .muted { opacity: .7; }
    table { width: 100%; border-collapse: collapse; }
    th, td { border-bottom: 1px solid var(--vscode-panel-border); text-align: left; padding: 6px; vertical-align: top; }
    th { opacity: .9; }
    .chip { display: inline-block; border-radius: 999px; padding: 2px 8px; margin: 2px 4px 2px 0; font-size: 12px; }
    .chip.ok { background: rgba(40,180,99,.2); border: 1px solid rgba(40,180,99,.5); }
    .chip.warn { background: rgba(230,126,34,.2); border: 1px solid rgba(230,126,34,.5); }
  </style>
</head>
<body>
  <h2>helm-apps dependency graph</h2>
  <div class="grid">
    <div class="card">
      <div><b>global._includes</b></div>
      <ul>${includeRows || "<li class='muted'>none</li>"}</ul>
    </div>
    <div class="card">
      <div><b>include files</b></div>
      <ul>${fileRows || "<li class='muted'>none</li>"}</ul>
    </div>
  </div>
  <div class="card">
    <div><b>apps -> includes</b></div>
    <table>
      <thead><tr><th>Group</th><th>App</th><th>Includes</th></tr></thead>
      <tbody>${appRows || "<tr><td colspan='3' class='muted'>no apps found</td></tr>"}</tbody>
    </table>
  </div>
</body>
</html>`;
}

async function validateCurrentFile(): Promise<void> {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    void vscode.window.showWarningMessage("No active editor");
    return;
  }

  const document = editor.document;
  if (document.isUntitled) {
    void vscode.window.showWarningMessage("Save file before validation");
    return;
  }

  const happPath = vscode.workspace.getConfiguration("helm-apps").get<string>("happPath", "happ");
  const filePath = document.uri.fsPath;

  try {
    const { stdout, stderr } = await execFileAsync(happPath, ["validate", "--values", filePath], {
      cwd: workspaceRoot(document.uri) ?? path.dirname(filePath),
      timeout: 60000,
      maxBuffer: 8 * 1024 * 1024,
    });

    const output = [stdout, stderr].filter(Boolean).join("\n").trim();
    if (output.length > 0) {
      void vscode.window.showInformationMessage(`helm-apps validation passed: ${output}`);
      return;
    }

    void vscode.window.showInformationMessage("helm-apps validation passed");
  } catch (err) {
    const message = extractErrorMessage(err);
    void vscode.window.showErrorMessage(`helm-apps validation failed: ${message}`);
  }
}

async function pasteClipboardAsHelmApps(editor: vscode.TextEditor): Promise<void> {
  const clipboard = (await vscode.env.clipboard.readText()).trim();
  if (clipboard.length === 0) {
    void vscode.window.showWarningMessage("Clipboard is empty");
    return;
  }

  const happPath = vscode.workspace.getConfiguration("helm-apps").get<string>("happPath", "happ");
  const envDiscovery = discoverEnvironments(editor.document.getText());
  const env = detectDefaultEnv(parseValuesObject(editor.document.getText()), envDiscovery);
  const cwd = workspaceRoot(editor.document.uri) ?? path.dirname(editor.document.uri.fsPath);

  let tempDir = "";
  try {
    tempDir = await mkdtemp(path.join(tmpdir(), "happ-paste-"));
    const inputPath = path.join(tempDir, "clipboard-manifest.yaml");
    await writeFile(inputPath, clipboard, "utf8");

    const { stdout, stderr } = await execFileAsync(
      happPath,
      ["manifests", "--path", inputPath, "--import-strategy", "helpers-experimental", "--env", env],
      {
        cwd,
        timeout: 120000,
        maxBuffer: 16 * 1024 * 1024,
      },
    );

    const generated = (stdout ?? "").trim();
    if (generated.length === 0) {
      const details = (stderr ?? "").trim();
      throw new Error(details.length > 0 ? details : "happ returned empty output");
    }

    await editor.edit((builder) => {
      if (!editor.selection.isEmpty) {
        builder.replace(editor.selection, generated);
      } else {
        builder.insert(editor.selection.active, generated);
      }
    });
    void vscode.window.showInformationMessage("Inserted clipboard as helm-apps values");
  } catch (err) {
    void vscode.window.showErrorMessage(`Paste as helm-apps failed: ${extractErrorMessage(err)}`);
  } finally {
    if (tempDir.length > 0) {
      try {
        await rm(tempDir, { recursive: true, force: true });
      } catch {
        // ignore temp cleanup errors
      }
    }
  }
}

function workspaceRoot(uri: vscode.Uri): string | undefined {
  const folder = vscode.workspace.getWorkspaceFolder(uri);
  return folder?.uri.fsPath;
}

function extractErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}

async function rewriteEditorText(
  editor: vscode.TextEditor,
  action: (text: string) => { updatedText: string; details: string },
): Promise<void> {
  const text = editor.document.getText();
  try {
    const result = action(text);
    const fullRange = new vscode.Range(
      editor.document.positionAt(0),
      editor.document.positionAt(text.length),
    );
    await editor.edit((builder) => {
      builder.replace(fullRange, result.updatedText);
    });
    void vscode.window.showInformationMessage(`helm-apps: ${result.details}`);
  } catch (err) {
    void vscode.window.showErrorMessage(`helm-apps refactor failed: ${extractErrorMessage(err)}`);
  }
}

function showEntityPreview(
  group: string,
  app: string,
  values: unknown,
  envDiscovery: { literals: string[]; regexes: string[] },
  options: PreviewOptions,
  missingFiles: string[],
): void {
  const title = `helm-apps preview: ${group}.${app}`;
  if (!previewPanel) {
    previewPanel = vscode.window.createWebviewPanel(
      "helmAppsResolvedEntityPreview",
      title,
      vscode.ViewColumn.Beside,
      { enableFindWidget: true, enableScripts: true },
    );
    previewPanel.onDidDispose(() => {
      previewMessageSubscription?.dispose();
      previewMessageSubscription = undefined;
      previewPanel = undefined;
    });
  } else {
    previewPanel.title = title;
    previewPanel.reveal(vscode.ViewColumn.Beside, true);
  }

  const render = () => {
    const yamlText = buildEntityPreviewYaml(values, group, app, options);
    previewPanel!.webview.html = renderPreviewHtml(title, yamlText, envDiscovery, options, missingFiles);
  };

  previewMessageSubscription?.dispose();
  previewMessageSubscription = previewPanel.webview.onDidReceiveMessage((msg: unknown) => {
    if (!isWebviewMessage(msg)) {
      return;
    }

    if (msg.type === "optionsChanged") {
      options.env = msg.env;
      options.applyIncludes = msg.applyIncludes;
      options.applyEnvResolution = msg.applyEnvResolution;
      render();
    }
  });

  render();
}

function buildEntityPreviewYaml(values: unknown, group: string, app: string, options: PreviewOptions): string {
  let entity: unknown;
  if (options.applyIncludes) {
    entity = resolveEntityWithIncludes(values, group, app);
  } else {
    const parsed = values as Record<string, unknown>;
    entity = (((parsed[group] as Record<string, unknown> | undefined) ?? {})[app] ?? {}) as unknown;
  }

  if (options.applyEnvResolution) {
    entity = resolveEnvMaps(entity, options.env);
  }

  return YAML.stringify({
    global: { env: options.env },
    [group]: { [app]: entity },
  });
}

async function loadExpandedValues(document: vscode.TextDocument): Promise<{
  values: Record<string, unknown>;
  includeDefinitions: IncludeDefinition[];
  missingFiles: Array<{ rawPath: string; tried: string[] }>;
}> {
  const parsed = YAML.parse(document.getText()) as unknown;
  if (!isMap(parsed)) {
    throw new Error("values file must be a YAML map");
  }
  return await expandValuesWithFileIncludes(parsed, document.uri.fsPath, async (filePath) => await readFile(filePath, "utf8"));
}

async function provideDefinition(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Definition | undefined> {
  const includeDefinition = await provideIncludeDefinition(document, position);
  if (includeDefinition) {
    return includeDefinition;
  }
  return await provideAppDefinition(document, position);
}

async function provideAppDefinition(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Definition | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }
  const symbol = findSymbolAtPosition(document.getText(), position.line, position.character);
  if (!symbol || symbol.kind !== "app") {
    return undefined;
  }

  const occurrences = collectSymbolOccurrences(document.getText(), symbol);
  const definition = occurrences.find((it) => it.role === "definition");
  if (!definition) {
    return undefined;
  }
  return new vscode.Location(
    document.uri,
    new vscode.Range(
      new vscode.Position(definition.line, definition.start),
      new vscode.Position(definition.line, definition.end),
    ),
  );
}

async function provideIncludeDefinition(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Definition | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }
  const includeName = getIncludeNameUnderCursor(document, position);
  if (!includeName) {
    return undefined;
  }

  const loaded = await loadExpandedValues(document);
  const map = indexIncludeDefinitions(document, loaded.values, loaded.includeDefinitions);
  const loc = map.get(includeName);
  if (loc) {
    return loc;
  }

  return await findIncludeDefinitionInReferencedFiles(document, includeName);
}

async function provideReferences(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Location[] | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }

  const symbol = findSymbolAtPosition(document.getText(), position.line, position.character);
  if (!symbol) {
    return undefined;
  }

  const occurrences = collectSymbolOccurrences(document.getText(), symbol);
  if (occurrences.length === 0) {
    return undefined;
  }

  return occurrences.map((it) =>
    new vscode.Location(
      document.uri,
      new vscode.Range(new vscode.Position(it.line, it.start), new vscode.Position(it.line, it.end)),
    ));
}

async function prepareRename(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Range | { range: vscode.Range; placeholder: string } | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }
  const symbol = findSymbolAtPosition(document.getText(), position.line, position.character);
  if (!symbol) {
    return undefined;
  }
  const occurrences = collectSymbolOccurrences(document.getText(), symbol);
  if (occurrences.length === 0) {
    return undefined;
  }
  const anchor = occurrences.find((it) =>
    it.line === position.line && position.character >= it.start && position.character <= it.end);
  if (!anchor) {
    return undefined;
  }
  return {
    range: new vscode.Range(
      new vscode.Position(anchor.line, anchor.start),
      new vscode.Position(anchor.line, anchor.end),
    ),
    placeholder: symbol.name,
  };
}

async function provideRenameEdits(
  document: vscode.TextDocument,
  position: vscode.Position,
  newName: string,
): Promise<vscode.WorkspaceEdit | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }
  const symbol = findSymbolAtPosition(document.getText(), position.line, position.character);
  if (!symbol) {
    throw new Error("No renameable helm-apps symbol under cursor");
  }

  if (!/^[A-Za-z0-9_.-]+$/.test(newName)) {
    throw new Error("Use ^[A-Za-z0-9_.-]+$ for symbol rename");
  }

  const occurrences = collectSymbolOccurrences(document.getText(), symbol);
  if (occurrences.length === 0) {
    throw new Error("No symbol occurrences found");
  }

  const edit = new vscode.WorkspaceEdit();
  for (const occ of occurrences) {
    edit.replace(
      document.uri,
      new vscode.Range(new vscode.Position(occ.line, occ.start), new vscode.Position(occ.line, occ.end)),
      newName,
    );
  }
  return edit;
}

async function provideCompletionItems(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.CompletionItem[] | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }

  const text = document.getText();
  const lines = text.split(/\r?\n/);
  const line = lines[position.line] ?? "";
  const indent = countIndent(line);
  const contextPath = completionContextPath(text, position.line, position.character, indent);
  const items: vscode.CompletionItem[] = [];

  if (contextPath.length === 0 && indent <= 2) {
    const topGroups = [
      "apps-stateless",
      "apps-stateful",
      "apps-jobs",
      "apps-cronjobs",
      "apps-services",
      "apps-ingresses",
      "apps-network-policies",
      "apps-configmaps",
      "apps-secrets",
      "apps-pvcs",
      "apps-service-accounts",
      "apps-k8s-manifests",
    ];
    for (const group of topGroups) {
      const item = new vscode.CompletionItem(group, vscode.CompletionItemKind.Module);
      item.insertText = `${group}:`;
      item.detail = "helm-apps group";
      items.push(item);
    }
  }

  const schemaItems = buildSchemaCompletionItems(text, contextPath);
  for (const it of schemaItems) {
    items.push(it);
  }

  const includeItems = buildIncludeCompletionItems(text, position.line);
  for (const it of includeItems) {
    items.push(it);
  }

  const atAppRoot = contextPath.length === 2 && contextPath[0] !== "global" && contextPath[1] !== "__GroupVars__";
  if (atAppRoot && indent >= 4) {
    const group = contextPath[0];
    const effectiveGroup = resolveEffectiveGroupType(text, group);
    const filtered = applyGroupAwareRootFiltering(items, effectiveGroup);
    items.length = 0;
    for (const it of filtered) {
      items.push(it);
    }
  }

  const last = contextPath[contextPath.length - 1] ?? "";
  if ((last === "containers" || last === "initContainers") && indent >= 6) {
    const defaultName = last === "initContainers" ? "init-container-1" : "container-1";
    pushSnippet(items, defaultName, `${defaultName}:\n  image:\n    name: \${1:nginx}\n  command: |-\n    - \${2:sleep}\n  args: |-\n    - \${3:10}\n`, "Named container entry");
  }

  const inContainer = contextPath.length >= 4
    && (contextPath[contextPath.length - 2] === "containers" || contextPath[contextPath.length - 2] === "initContainers");
  if (inContainer && indent >= 8) {
    pushSnippet(items, "image", "image:\n  name: ${1:nginx}\n  staticTag: ${2:\"latest\"}", "Container image");
    pushSnippet(items, "command", "command: |-\n  - ${1:/bin/sh}", "Container command");
    pushSnippet(items, "args", "args: |-\n  - ${1:-c}\n  - ${2:echo hi}", "Container args");
    pushSnippet(items, "envVars", "envVars:\n  ${1:LOG_LEVEL}: ${2:info}", "envVars map");
    pushSnippet(items, "envFrom", "envFrom: |-\n  - secretRef:\n      name: ${1:app-secrets}", "envFrom list");
    pushSnippet(items, "resources", "resources:\n  requests:\n    mcpu: 100\n    memoryMb: 128\n  limits:\n    mcpu: 500\n    memoryMb: 512", "Resources map");
    pushSnippet(items, "ports", "ports: |-\n  - name: ${1:http}\n    containerPort: ${2:8080}", "Container ports");
    pushSnippet(items, "volumeMounts", "volumeMounts: |-\n  - name: ${1:config}\n    mountPath: ${2:/etc/app}", "Volume mounts");
    pushSnippet(items, "readinessProbe", "readinessProbe:\n  enabled: true\n  httpGet:\n    path: ${1:/healthz}\n    port: ${2:8080}", "Readiness probe");
    pushSnippet(items, "livenessProbe", "livenessProbe:\n  enabled: true\n  httpGet:\n    path: ${1:/healthz}\n    port: ${2:8080}", "Liveness probe");
  }

  if (last === "image" && indent >= 10) {
    pushSnippet(items, "name", "name: ${1:nginx}", "Image name");
    pushSnippet(items, "staticTag", "staticTag: ${1:\"latest\"}", "Fixed image tag");
  }

  if (last === "service" && indent >= 6) {
    pushSnippet(items, "enabled", "enabled: true", "Enable service");
    pushSnippet(items, "name", "name: ${1:\"{{ $.CurrentApp.name }}\"}", "Service name");
    pushSnippet(items, "ports", "ports: |-\n  - name: ${1:http}\n    port: ${2:80}\n    targetPort: ${3:http}", "Service ports");
    pushSnippet(items, "type", "type: ${1:ClusterIP}", "Service type");
  }

  if (items.length === 0) {
    return undefined;
  }
  return dedupeCompletionItems(items);
}

function pushSnippet(items: vscode.CompletionItem[], label: string, insert: string, detail: string): void {
  const item = new vscode.CompletionItem(label, vscode.CompletionItemKind.Snippet);
  item.insertText = new vscode.SnippetString(insert);
  item.detail = detail;
  items.push(item);
}

function dedupeCompletionItems(items: vscode.CompletionItem[]): vscode.CompletionItem[] {
  const out: vscode.CompletionItem[] = [];
  const seen = new Set<string>();
  for (const item of items) {
    const key = `${item.label}:${String(item.kind)}:${item.detail ?? ""}`;
    if (seen.has(key)) {
      continue;
    }
    seen.add(key);
    out.push(item);
  }
  return out;
}

function buildSchemaCompletionItems(text: string, contextPath: string[]): vscode.CompletionItem[] {
  const root = loadCompletionSchemaRoot();
  if (!root) {
    return [];
  }
  const path = schemaPathForContext(text, contextPath);
  const schema = resolveSchemaAtPathLocal(root, path);
  if (!schema) {
    return [];
  }
  const keys = collectSchemaPropertyKeysLocal(schema, root, path[path.length - 1] ?? "");
  const items: vscode.CompletionItem[] = [];
  for (const key of keys) {
    const item = new vscode.CompletionItem(key, vscode.CompletionItemKind.Field);
    item.insertText = `${key}: `;
    item.detail = "Schema key";
    items.push(item);
  }
  return items;
}

function buildIncludeCompletionItems(text: string, line: number): vscode.CompletionItem[] {
  if (parentKeyForLine(text, line) !== "_include") {
    return [];
  }
  const includeNames = extractGlobalIncludeNames(text);
  return includeNames.map((name) => {
    const item = new vscode.CompletionItem(name, vscode.CompletionItemKind.Reference);
    item.insertText = name;
    item.detail = "global._includes";
    item.sortText = `00_${name}`;
    return item;
  });
}

function applyGroupAwareRootFiltering(items: vscode.CompletionItem[], effectiveGroup: string): vscode.CompletionItem[] {
  const allowed = allowedRootKeysByGroup(effectiveGroup);
  if (allowed.size > 0) {
    const filtered: vscode.CompletionItem[] = [];
    for (const item of items) {
      const label = typeof item.label === "string" ? item.label : item.label.label;
      const isRootKeyCandidate = item.kind === vscode.CompletionItemKind.Field || item.kind === vscode.CompletionItemKind.Snippet;
      if (isRootKeyCandidate && !allowed.has(label)) {
        continue;
      }
      item.sortText = `10_${label}`;
      filtered.push(item);
    }
    return filtered;
  }
  for (const item of items) {
    const label = typeof item.label === "string" ? item.label : item.label.label;
    if (!item.sortText) {
      item.sortText = `20_${label}`;
    }
  }
  return items;
}

function allowedRootKeysByGroup(group: string): Set<string> {
  const common = [
    "enabled",
    "_include",
    "name",
    "annotations",
    "labels",
    "selector",
    "type",
  ];
  const workload = [
    "containers",
    "initContainers",
    "resources",
    "envVars",
    "service",
    "serviceAccount",
    "affinity",
    "tolerations",
    "nodeSelector",
    "volumes",
    "imagePullSecrets",
    "podDisruptionBudget",
    "horizontalPodAutoscaler",
    "verticalPodAutoscaler",
  ];
  const ingress = [
    "class",
    "host",
    "hosts",
    "paths",
    "tls",
    "ingressClassName",
    "service",
    "servicePort",
    "dexAuth",
    "sendAuthorizationHeader",
  ];
  const service = [
    "ports",
    "selector",
    "type",
  ];
  const configmap = ["data", "binaryData", "immutable"];
  const secret = ["data", "binaryData", "immutable", "stringData", "kind"];

  if (isWorkloadGroup(group)) {
    return new Set([...common, ...workload]);
  }
  if (group === "apps-ingresses") {
    return new Set([...common, ...ingress]);
  }
  if (group === "apps-services") {
    return new Set([...common, ...service]);
  }
  if (group === "apps-configmaps") {
    return new Set([...common, ...configmap]);
  }
  if (group === "apps-secrets") {
    return new Set([...common, ...secret]);
  }
  return new Set();
}

function isWorkloadGroup(group: string): boolean {
  return group === "apps-stateless"
    || group === "apps-stateful"
    || group === "apps-jobs"
    || group === "apps-cronjobs";
}

function resolveEffectiveGroupType(text: string, groupName: string): string {
  if (groupName.startsWith("apps-")) {
    return groupName;
  }
  try {
    const parsed = YAML.parse(text) as unknown;
    const root = toMap(parsed);
    const group = root ? toMap(root[groupName]) : null;
    const groupVars = group ? toMap(group.__GroupVars__) : null;
    const rawType = groupVars ? groupVars.type : undefined;
    if (typeof rawType === "string" && rawType.trim().length > 0) {
      return rawType.trim();
    }
    if (isMap(rawType)) {
      const env = (() => {
        const global = root ? toMap(root.global) : null;
        const e = global ? global.env : undefined;
        return typeof e === "string" && e.trim().length > 0 ? e.trim() : "dev";
      })();
      const typed = resolveEnvMaps(rawType, env);
      if (typeof typed === "string" && typed.trim().length > 0) {
        return typed.trim();
      }
    }
  } catch {
    // ignore parse errors, fallback to raw group
  }
  return groupName;
}

function schemaPathForContext(text: string, contextPath: string[]): string[] {
  if (contextPath.length === 0) {
    return [];
  }
  const first = contextPath[0];
  if (first === "global" || first.startsWith("apps-")) {
    return contextPath;
  }
  const effective = resolveEffectiveGroupType(text, first);
  return [effective, ...contextPath.slice(1)];
}

function extractGlobalIncludeNames(text: string): string[] {
  const lines = text.split(/\r?\n/);
  const names: string[] = [];
  let inGlobal = false;
  let inIncludes = false;
  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }
    const indent = m[1].length;
    const key = m[2];
    if (indent === 0) {
      inGlobal = key === "global";
      inIncludes = false;
      continue;
    }
    if (inGlobal && indent === 2) {
      inIncludes = key === "_includes";
      continue;
    }
    if (inGlobal && inIncludes && indent === 4) {
      names.push(key);
    }
  }
  return names;
}

function parentKeyForLine(text: string, line: number): string | null {
  const lines = text.split(/\r?\n/);
  const current = lines[line] ?? "";
  const indent = countIndent(current);
  for (let i = line - 1; i >= 0; i -= 1) {
    const m = lines[i].match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }
    const keyIndent = m[1].length;
    if (keyIndent < indent) {
      return m[2];
    }
  }
  return null;
}

function completionContextPath(text: string, line: number, character: number, cursorIndent: number): string[] {
  const lines = text.split(/\r?\n/);
  const stack: Array<{ indent: number; key: string; line: number; start: number; end: number }> = [];

  for (let i = 0; i <= line; i += 1) {
    const raw = lines[i] ?? "";
    const trimmed = raw.trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }
    const m = raw.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }
    const indent = m[1].length;
    while (stack.length > 0 && stack[stack.length - 1].indent >= indent) {
      stack.pop();
    }
    const key = m[2];
    const start = raw.indexOf(key);
    stack.push({ indent, key, line: i, start, end: start + key.length });
  }

  const currentLine = lines[line] ?? "";
  const currentKey = currentLine.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
  let effectiveIndent = cursorIndent;
  if (currentKey) {
    const keyIndent = currentKey[1].length;
    const keyStart = keyIndent;
    const keyEnd = keyStart + currentKey[2].length;
    if (character <= keyEnd + 1) {
      effectiveIndent = keyIndent;
    } else {
      effectiveIndent = keyIndent + 2;
    }
  }

  while (stack.length > 0 && stack[stack.length - 1].indent >= effectiveIndent) {
    stack.pop();
  }
  return stack.map((s) => s.key);
}

function loadCompletionSchemaRoot(): JsonSchema | null {
  if (completionSchemaCache) {
    return completionSchemaCache;
  }
  const candidates = [
    path.resolve(__dirname, "../../schemas/values.schema.json"),
    path.resolve(__dirname, "../../../schemas/values.schema.json"),
  ];
  for (const p of candidates) {
    try {
      const raw = readFileSync(p, "utf8");
      completionSchemaCache = JSON.parse(raw) as JsonSchema;
      return completionSchemaCache;
    } catch {
      // try next
    }
  }
  return null;
}

function resolveSchemaAtPathLocal(root: JsonSchema, pathParts: string[]): JsonSchema | null {
  return walkSchemaLocal(root, pathParts, 0, root);
}

function walkSchemaLocal(current: JsonSchema, pathParts: string[], index: number, root: JsonSchema): JsonSchema | null {
  const schema = resolveRefsLocal(current, root);
  if (!schema) {
    return null;
  }
  if (index >= pathParts.length) {
    return schema;
  }
  const segment = pathParts[index];
  const candidates = nextSchemasForSegmentLocal(schema, segment, root);
  for (const c of candidates) {
    const resolved = walkSchemaLocal(c, pathParts, index + 1, root);
    if (resolved) {
      return resolved;
    }
  }
  return null;
}

function nextSchemasForSegmentLocal(schema: JsonSchema, segment: string, root: JsonSchema): JsonSchema[] {
  const out: JsonSchema[] = [];
  for (const variant of schemaVariantsLocal(schema, root)) {
    if (variant.properties && variant.properties[segment]) {
      out.push(variant.properties[segment]);
    }
    if (variant.patternProperties) {
      for (const [pattern, child] of Object.entries(variant.patternProperties)) {
        try {
          const re = new RegExp(pattern);
          if (re.test(segment)) {
            out.push(child);
          }
        } catch {
          // bad regex in schema, ignore
        }
      }
    }
    if (typeof variant.additionalProperties === "object") {
      out.push(variant.additionalProperties);
    }
  }
  return out;
}

function schemaVariantsLocal(schema: JsonSchema, root: JsonSchema): JsonSchema[] {
  const base = resolveRefsLocal(schema, root);
  if (!base) {
    return [];
  }
  const out: JsonSchema[] = [base];
  for (const arr of [base.allOf, base.anyOf, base.oneOf]) {
    if (!arr) {
      continue;
    }
    for (const item of arr) {
      const resolved = resolveRefsLocal(item, root);
      if (resolved) {
        out.push(resolved);
      }
    }
  }
  return out;
}

function resolveRefsLocal(schema: JsonSchema | undefined, root: JsonSchema): JsonSchema | null {
  if (!schema) {
    return null;
  }
  let current: JsonSchema | undefined = schema;
  const seen = new Set<string>();
  while (current && current.$ref) {
    const ref = current.$ref;
    if (!ref.startsWith("#/") || seen.has(ref)) {
      break;
    }
    seen.add(ref);
    current = getByPointerLocal(root, ref);
  }
  return current ?? null;
}

function getByPointerLocal(root: JsonSchema, pointer: string): JsonSchema | undefined {
  const chunks = pointer
    .slice(2)
    .split("/")
    .map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~"));
  let cur: unknown = root;
  for (const c of chunks) {
    if (!cur || typeof cur !== "object" || !(c in (cur as Record<string, unknown>))) {
      return undefined;
    }
    cur = (cur as Record<string, unknown>)[c];
  }
  return cur as JsonSchema;
}

function collectSchemaPropertyKeysLocal(schema: JsonSchema, root: JsonSchema, parentKey: string): string[] {
  const out = new Set<string>();
  for (const variant of schemaVariantsLocal(schema, root)) {
    if (variant.properties) {
      for (const key of Object.keys(variant.properties)) {
        out.add(key);
      }
    }
    if (variant.patternProperties) {
      for (const key of Object.keys(variant.patternProperties)) {
        const sample = sampleKeyFromPattern(key, parentKey);
        if (sample) {
          out.add(sample);
        }
      }
    }
  }
  if (parentKey === "containers" && !out.has("container-1")) {
    out.add("container-1");
  }
  if (parentKey === "initContainers" && !out.has("init-container-1")) {
    out.add("init-container-1");
  }
  if (parentKey === "_includes" && !out.has("apps-default")) {
    out.add("apps-default");
  }
  return [...out];
}

function sampleKeyFromPattern(pattern: string, parentKey: string): string | null {
  if (pattern === "^[A-Za-z0-9][A-Za-z0-9_.-]*$") {
    if (parentKey === "containers") {
      return "container-1";
    }
    if (parentKey === "initContainers") {
      return "init-container-1";
    }
    return "app-1";
  }
  if (pattern.includes("container")) {
    return parentKey === "initContainers" ? "init-container-1" : "container-1";
  }
  return null;
}

async function provideCodeActions(
  document: vscode.TextDocument,
  range: vscode.Range | vscode.Selection,
  context?: vscode.CodeActionContext,
): Promise<vscode.CodeAction[] | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }
  const lineNumber = range.start.line;
  const line = document.lineAt(lineNumber).text;
  const actions: vscode.CodeAction[] = [];

  const inlineInclude = line.match(/^(\s*)_include:\s*\[(.+)\]\s*$/);
  if (inlineInclude) {
    const indent = inlineInclude[1];
    const values = inlineInclude[2]
      .split(",")
      .map((v) => unquote(v.trim()))
      .filter((v) => v.length > 0);
    if (values.length > 0) {
      const replacement = `${indent}_include:\n${values.map((v) => `${indent}  - ${v}`).join("\n")}`;
      const action = new vscode.CodeAction("Convert inline _include list to multiline", vscode.CodeActionKind.QuickFix);
      action.edit = new vscode.WorkspaceEdit();
      action.edit.replace(
        document.uri,
        new vscode.Range(new vscode.Position(lineNumber, 0), new vscode.Position(lineNumber, line.length)),
        replacement,
      );
      actions.push(action);
    }
  }

  const appScope = findAppScopeAtLine(document.getText(), lineNumber);
  if (appScope && /^(\s*)([A-Za-z0-9_.-]+):\s*$/.test(line) && countIndent(line) === 2) {
    const nextLine = lineNumber + 1;
    const action = new vscode.CodeAction("Add enabled: true", vscode.CodeActionKind.QuickFix);
    action.edit = new vscode.WorkspaceEdit();
    action.edit.insert(document.uri, new vscode.Position(nextLine, 0), "    enabled: true\n");
    actions.push(action);
  }

  const unresolved = (context?.diagnostics ?? []).find((d) =>
    d.message.startsWith("Unresolved include profile:"));
  if (unresolved) {
    const includeName = unresolved.message.replace("Unresolved include profile:", "").trim();
    if (/^[A-Za-z0-9_.-]+$/.test(includeName)) {
      const action = new vscode.CodeAction(`Create include profile '${includeName}'`, vscode.CodeActionKind.QuickFix);
      action.edit = new vscode.WorkspaceEdit();
      const insertion = buildIncludeProfileInsertion(document, includeName);
      action.edit.insert(document.uri, insertion.position, insertion.text);
      actions.push(action);
    }
  }

  if (actions.length === 0) {
    return undefined;
  }
  return actions;
}

function buildIncludeProfileInsertion(
  document: vscode.TextDocument,
  includeName: string,
): { position: vscode.Position; text: string } {
  const lines = document.getText().split(/\r?\n/);
  let globalLine = -1;
  let includesLine = -1;
  let includesIndent = 2;

  for (let i = 0; i < lines.length; i += 1) {
    const m = lines[i].match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }
    const indent = m[1].length;
    const key = m[2];
    if (indent === 0 && key === "global") {
      globalLine = i;
      continue;
    }
    if (globalLine >= 0 && indent === 2 && key === "_includes") {
      includesLine = i;
      includesIndent = indent;
      break;
    }
    if (globalLine >= 0 && indent === 0 && key !== "global") {
      break;
    }
  }

  if (includesLine >= 0) {
    let end = lines.length;
    for (let i = includesLine + 1; i < lines.length; i += 1) {
      const m = lines[i].match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
      if (!m) {
        continue;
      }
      const indent = m[1].length;
      if (indent <= includesIndent) {
        end = i;
        break;
      }
    }
    return {
      position: new vscode.Position(end, 0),
      text: `    ${includeName}:\n      enabled: true\n`,
    };
  }

  if (globalLine >= 0) {
    return {
      position: new vscode.Position(globalLine + 1, 0),
      text: `  _includes:\n    ${includeName}:\n      enabled: true\n`,
    };
  }

  const prefix = document.getText().trim().length > 0 ? "\n" : "";
  return {
    position: new vscode.Position(lines.length, 0),
    text: `${prefix}global:\n  _includes:\n    ${includeName}:\n      enabled: true\n`,
  };
}

async function provideIncludeHover(
  document: vscode.TextDocument,
  position: vscode.Position,
): Promise<vscode.Hover | undefined> {
  if (!(await isHelmAppsValuesDocument(document))) {
    return undefined;
  }
  const ru = vscode.env.language.toLowerCase().startsWith("ru");
  const includeLabel = ru ? "инклуд" : "include";
  const sourceLabel = ru ? "источник" : "source";

  const includeName = getIncludeNameUnderCursor(document, position);
  if (includeName) {
    const localBlock = extractLocalIncludeBlock(document.getText(), includeName);
    if (localBlock) {
      const md = new vscode.MarkdownString();
      md.appendMarkdown(`**${includeLabel}** \`${includeName}\`  \n`);
      md.appendMarkdown(`${sourceLabel}: \`${document.uri.fsPath}\`\n\n`);
      md.appendCodeblock(trimPreview(localBlock), "yaml");
      return new vscode.Hover(md);
    }

    const loaded = await loadExpandedValues(document);
    const fileDef = loaded.includeDefinitions.find((d) => d.name === includeName);
    if (fileDef) {
      const raw = await readFile(fileDef.filePath, "utf8");
      const md = new vscode.MarkdownString();
      md.appendMarkdown(`**${includeLabel}** \`${includeName}\`  \n`);
      md.appendMarkdown(`${sourceLabel}: \`${fileDef.filePath}\`\n\n`);
      md.appendCodeblock(trimPreview(raw), "yaml");
      return new vscode.Hover(md);
    }

    const discovered = await findIncludeDefinitionInReferencedFiles(document, includeName);
    if (discovered) {
      const raw = await readFile(discovered.uri.fsPath, "utf8");
      const md = new vscode.MarkdownString();
      md.appendMarkdown(`**${includeLabel}** \`${includeName}\`  \n`);
      md.appendMarkdown(`${sourceLabel}: \`${discovered.uri.fsPath}\`\n\n`);
      md.appendCodeblock(trimPreview(raw), "yaml");
      return new vscode.Hover(md);
    }

    const resolved = toMap(toMap(loaded.values.global)?._includes)?.[includeName];
    if (resolved !== undefined) {
      const md = new vscode.MarkdownString();
      md.appendMarkdown(`**${includeLabel}** \`${includeName}\`  \n`);
      md.appendMarkdown(`${sourceLabel}: ${ru ? "разрешённый global._includes" : "resolved global._includes"}\n\n`);
      md.appendCodeblock(trimPreview(YAML.stringify({ [includeName]: resolved })), "yaml");
      return new vscode.Hover(md);
    }
  }

  const fieldHover = provideFieldHover(document, position);
  if (fieldHover) {
    return fieldHover;
  }

  return undefined;
}

function provideFieldHover(document: vscode.TextDocument, position: vscode.Position): vscode.Hover | undefined {
  const path = findKeyPathAtPosition(document.getText(), position.line, position.character);
  if (!path) {
    return undefined;
  }
  const doc = findFieldDoc(path);
  if (!doc) {
    return undefined;
  }

  const md = new vscode.MarkdownString();
  md.isTrusted = false;
  md.appendMarkdown(buildFieldDocMarkdownLocalized(path, doc, vscode.env.language));
  return new vscode.Hover(md);
}

async function discoverHelmAppsSchemaTargets(): Promise<string[]> {
  const charts = await vscode.workspace.findFiles("**/Chart.yaml", "**/{.git,node_modules,vendor,tmp,.werf}/**");
  const out = new Set<string>();

  for (const chart of charts) {
    // eslint-disable-next-line no-await-in-loop
    const enabled = await isHelmAppsChart(chart);
    if (!enabled) {
      continue;
    }
    const chartDir = path.dirname(chart.fsPath);
    const patterns = ["values*.yaml", "values*.yml"];
    for (const p of patterns) {
      // eslint-disable-next-line no-await-in-loop
      const valuesFiles = await vscode.workspace.findFiles(
        new vscode.RelativePattern(chartDir, p),
        "**/{.git,node_modules,vendor,tmp,.werf}/**",
      );
      for (const v of valuesFiles) {
        out.add(vscode.workspace.asRelativePath(v, false));
      }
    }
  }

  return [...out].sort();
}

async function isHelmAppsChart(chartYamlUri: vscode.Uri): Promise<boolean> {
  const chartDir = path.dirname(chartYamlUri.fsPath);
  const templates = await vscode.workspace.findFiles(
    new vscode.RelativePattern(chartDir, "templates/**/*.{yaml,yml,tpl}"),
    "**/{.git,node_modules,vendor,tmp,.werf}/**",
  );

  for (const t of templates.slice(0, 80)) {
    try {
      // eslint-disable-next-line no-await-in-loop
      const raw = await readFile(t.fsPath, "utf8");
      if (raw.includes(`include "apps-utils.init-library"`)) {
        return true;
      }
    } catch {
      // skip unreadable template
    }
  }
  return false;
}

async function isHelmAppsValuesDocument(document: vscode.TextDocument): Promise<boolean> {
  if (document.languageId !== "yaml") {
    return false;
  }
  const text = document.getText();
  if (!looksLikeHelmAppsValuesText(text)) {
    return false;
  }

  const chart = await findNearestChartYaml(document.uri.fsPath);
  if (!chart) {
    return false;
  }
  return await isHelmAppsChart(chart);
}

function looksLikeHelmAppsValuesText(text: string): boolean {
  if (/\n?global:\s*/m.test(text) && /(?:^|\n)\s*_includes:\s*/m.test(text)) {
    return true;
  }
  if (/(?:^|\n)apps-[a-z0-9-]+:\s*/m.test(text)) {
    return true;
  }
  if (/(?:^|\n)\s*__GroupVars__:\s*/m.test(text)) {
    return true;
  }
  return false;
}

async function findNearestChartYaml(fromFile: string): Promise<vscode.Uri | undefined> {
  let dir = path.dirname(fromFile);
  const root = path.parse(dir).root;
  while (dir !== root) {
    const candidate = path.join(dir, "Chart.yaml");
    try {
      // eslint-disable-next-line no-await-in-loop
      await access(candidate);
      return vscode.Uri.file(candidate);
    } catch {
      // continue
    }
    dir = path.dirname(dir);
  }
  return undefined;
}

function getIncludeNameUnderCursor(document: vscode.TextDocument, position: vscode.Position): string | null {
  const lines = document.getText().split(/\r?\n/);
  const line = lines[position.line] ?? "";
  const token = tokenUnderCursor(line, position.character);
  if (!token) {
    return null;
  }

  const inlineInclude = line.match(/^(\s*)_include:\s*(.+)$/);
  if (inlineInclude) {
    const valuePart = inlineInclude[2];
    if (valuePart.includes(token)) {
      return token;
    }
  }

  const m = line.match(/^(\s*)-\s+(.+?)\s*$/);
  if (!m) {
    return null;
  }

  const listIndent = m[1].length;
  let parentKey = "";
  for (let i = position.line - 1; i >= 0; i -= 1) {
    const t = lines[i].trim();
    if (t.length === 0 || t.startsWith("#")) {
      continue;
    }
    const k = lines[i].match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (k) {
      const ind = k[1].length;
      if (ind < listIndent) {
        parentKey = k[2];
        break;
      }
    }
  }
  if (parentKey !== "_include") {
    return null;
  }
  return token;
}

function indexIncludeDefinitions(
  document: vscode.TextDocument,
  values: Record<string, unknown>,
  fileDefs: IncludeDefinition[],
): Map<string, vscode.Location> {
  const index = new Map<string, vscode.Location>();
  const localDefs = findLocalGlobalIncludeLines(document);
  for (const [name, line] of localDefs.entries()) {
    index.set(name, new vscode.Location(document.uri, new vscode.Position(line, 0)));
  }

  for (const def of fileDefs) {
    if (!index.has(def.name)) {
      index.set(def.name, new vscode.Location(vscode.Uri.file(def.filePath), new vscode.Position(def.line, 0)));
    }
  }
  return index;
}

async function findIncludeDefinitionInReferencedFiles(
  document: vscode.TextDocument,
  includeName: string,
): Promise<vscode.Location | undefined> {
  const refs = collectIncludeFileRefs(document.getText());
  const seen = new Set<string>();
  const baseDir = path.dirname(document.uri.fsPath);

  for (const ref of refs) {
    if (isTemplatedIncludePath(ref.path)) {
      continue;
    }
    const candidates = buildIncludeCandidates(ref.path, baseDir);
    for (const candidate of candidates) {
      if (seen.has(candidate)) {
        continue;
      }
      seen.add(candidate);
      try {
        // eslint-disable-next-line no-await-in-loop
        const raw = await readFile(candidate, "utf8");
        const line = findTopLevelKeyLine(raw, includeName);
        if (line >= 0) {
          return new vscode.Location(vscode.Uri.file(candidate), new vscode.Position(line, 0));
        }
      } catch {
        // skip unreadable/missing include refs
      }
    }
  }
  return undefined;
}

function findTopLevelKeyLine(yamlText: string, key: string): number {
  const lines = yamlText.split(/\r?\n/);
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (line.trim().length === 0 || line.trim().startsWith("#")) {
      continue;
    }
    const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }
    if (m[1].length !== 0) {
      continue;
    }
    if (m[2] === key) {
      return i;
    }
  }
  return -1;
}

function findLocalGlobalIncludeLines(document: vscode.TextDocument): Map<string, number> {
  const lines = document.getText().split(/\r?\n/);
  const out = new Map<string, number>();
  let inGlobal = false;
  let inIncludes = false;

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const trimmed = line.trim();
    if (trimmed.length === 0 || trimmed.startsWith("#")) {
      continue;
    }
    const m = line.match(/^(\s*)([A-Za-z0-9_.-]+):\s*(.*)$/);
    if (!m) {
      continue;
    }
    const indent = m[1].length;
    const key = m[2];

    if (indent === 0) {
      inGlobal = key === "global";
      inIncludes = false;
      continue;
    }
    if (inGlobal && indent === 2) {
      inIncludes = key === "_includes";
      continue;
    }
    if (inGlobal && inIncludes && indent === 4) {
      out.set(key, i);
    }
  }
  return out;
}

function renderPreviewHtml(
  title: string,
  yamlText: string,
  envDiscovery: { literals: string[]; regexes: string[] },
  options: PreviewOptions,
  missingFiles: string[],
): string {
  const safeTitle = escapeHtml(title);
  const optionsJson = escapeHtml(JSON.stringify(options));
  const literalEnvs = [...new Set([options.env, ...envDiscovery.literals].filter((v) => v.trim().length > 0))];
  const regexEnvOptions = envDiscovery.regexes
    .map((re) => {
      const sample = sampleEnvFromRegex(re);
      return sample ? { regex: re, sample } : null;
    })
    .filter((v): v is { regex: string; sample: string } => v !== null);
  const knownEnvs = [...new Set([
    ...literalEnvs,
    ...regexEnvOptions.map((r) => r.sample),
  ])];
  const quickEnvButtons = knownEnvs.length > 0
    ? `<div class="quick-envs">${
      knownEnvs
        .map((env) => `<button type="button" class="quick-env" data-env="${escapeHtml(env)}">${escapeHtml(env)}</button>`)
        .join("")
    }</div>`
    : "";
  const details = (envDiscovery.regexes.length > 0 || missingFiles.length > 0)
    ? `<details>
        <summary>Details</summary>
        ${
          envDiscovery.regexes.length > 0
            ? `<div class="hint">regex env keys: ${escapeHtml(envDiscovery.regexes.join(", "))}</div>`
            : ""
        }
        ${
          missingFiles.length > 0
            ? `<div class="warn">missing include files (skipped): ${escapeHtml(missingFiles.join(", "))}</div>`
            : ""
        }
      </details>`
    : "";

  return `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>${safeTitle}</title>
    <style>
      body { font-family: Menlo, Monaco, Consolas, "Courier New", monospace; padding: 12px; }
      h2 { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; margin: 0 0 12px; }
      .bar { display: flex; gap: 12px; align-items: center; flex-wrap: wrap; margin-bottom: 10px; }
      label { font-size: 12px; }
      input[type="text"] { min-width: 240px; }
      .quick-envs { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 10px; }
      .quick-env { border: 1px solid #425276; background: #102142; color: #d8e6ff; border-radius: 999px; font-size: 11px; padding: 2px 8px; cursor: pointer; }
      .quick-env:hover { background: #17305f; }
      .hint { font-size: 11px; opacity: 0.8; margin-top: 8px; }
      .warn { font-size: 11px; color: #f2ad4e; margin-top: 8px; }
      .render { margin-top: 10px; border: 1px solid #2a3654; border-radius: 8px; background: #081126; overflow: auto; max-height: calc(100vh - 260px); }
      pre { margin: 0; padding: 14px; color: #d8e6ff; font-size: 12px; line-height: 1.45; white-space: pre; }
      .y-key { color: #8dc3ff; font-weight: 600; }
      .y-bool { color: #71f0b4; font-weight: 600; }
      .y-num { color: #9bd2ff; }
      .y-comment { color: #6782ac; font-style: italic; }
      .y-string { color: #f7c27f; }
      .y-block { color: #b2c6e6; font-weight: 600; }
      details { margin-bottom: 10px; }
      summary { cursor: pointer; font-size: 12px; user-select: none; }
    </style>
  </head>
  <body>
    <h2>${safeTitle}</h2>
    <div class="bar">
      <label>env:
        <input id="envInput" type="text" value="${escapeHtml(options.env)}" />
      </label>
      <label><input id="applyIncludes" type="checkbox" ${options.applyIncludes ? "checked" : ""}/> apply includes</label>
      <label><input id="applyEnvResolution" type="checkbox" ${options.applyEnvResolution ? "checked" : ""}/> resolve env maps</label>
    </div>
    ${quickEnvButtons}
    ${details}
    <div class="render"><pre id="yamlPreview">${renderYamlHighlightedHtml(yamlText)}</pre></div>
    <script>
      const vscode = acquireVsCodeApi();
      const options = JSON.parse("${optionsJson}");
      const envInput = document.getElementById("envInput");
      const quickEnvButtons = document.querySelectorAll(".quick-env");
      const applyIncludes = document.getElementById("applyIncludes");
      const applyEnvResolution = document.getElementById("applyEnvResolution");

      envInput.addEventListener("change", emit);
      envInput.addEventListener("keyup", (e) => {
        if (e.key === "Enter") {
          emit();
        }
      });
      quickEnvButtons.forEach((btn) => {
        btn.addEventListener("click", () => {
          const next = btn.getAttribute("data-env");
          if (!next) return;
          envInput.value = next;
          envInput.dispatchEvent(new Event("change"));
        });
      });
      applyIncludes.addEventListener("change", emit);
      applyEnvResolution.addEventListener("change", emit);

      function emit() {
        vscode.postMessage({
          type: "optionsChanged",
          env: envInput.value || options.env,
          applyIncludes: applyIncludes.checked,
          applyEnvResolution: applyEnvResolution.checked
        });
      }
    </script>
  </body>
</html>`;
}

function renderYamlHighlightedHtml(text: string): string {
  const lines = text.split(/\r?\n/);
  return lines
    .map((line) => highlightYamlLine(line))
    .join("\n");
}

function highlightYamlLine(line: string): string {
  const commentIdx = line.indexOf("#");
  let code = line;
  let comment = "";
  if (commentIdx >= 0) {
    code = line.slice(0, commentIdx);
    comment = line.slice(commentIdx);
  }

  const keyMatch = code.match(/^(\s*)([^:#\n][^:\n]*):(\s*)(.*)$/);
  if (keyMatch) {
    const indent = escapeHtml(keyMatch[1]);
    const key = `<span class="y-key">${escapeHtml(keyMatch[2])}</span>:`;
    const ws = escapeHtml(keyMatch[3]);
    const rawVal = keyMatch[4] ?? "";
    let val = escapeHtml(rawVal);
    const trimmed = rawVal.trim();

    if (trimmed === "|" || trimmed === "|-" || trimmed === ">") {
      val = `<span class="y-block">${escapeHtml(rawVal)}</span>`;
    } else if (/^(true|false|null)$/.test(trimmed)) {
      val = `<span class="y-bool">${escapeHtml(rawVal)}</span>`;
    } else if (/^-?\d+(\.\d+)?$/.test(trimmed)) {
      val = `<span class="y-num">${escapeHtml(rawVal)}</span>`;
    } else if (/^['"].*['"]$/.test(trimmed)) {
      val = `<span class="y-string">${escapeHtml(rawVal)}</span>`;
    }

    const commentHtml = comment ? `<span class="y-comment">${escapeHtml(comment)}</span>` : "";
    return `${indent}${key}${ws}${val}${commentHtml}`;
  }

  const commentOnly = comment && code.trim().length === 0
    ? `<span class="y-comment">${escapeHtml(comment)}</span>`
    : "";
  const codeHtml = escapeHtml(code);
  return `${codeHtml}${commentOnly}`;
}

function sampleEnvFromRegex(regex: string): string | null {
  if (regex.length === 0) {
    return null;
  }
  let s = regex.trim();
  if (s.startsWith("^")) {
    s = s.slice(1);
  }
  if (s.endsWith("$")) {
    s = s.slice(0, -1);
  }
  s = s
    .replace(/\.\*/g, "")
    .replace(/\.\+/g, "")
    .replace(/\.\?/g, "")
    .replace(/\[[^\]]*]/g, "")
    .replace(/\([^)]*\)/g, "")
    .replace(/[\\|?+*()[\]{}]/g, "")
    .trim();
  return s.length > 0 ? s : null;
}

function escapeHtml(input: string): string {
  return input
    .split("&").join("&amp;")
    .split("<").join("&lt;")
    .split(">").join("&gt;");
}

function isWebviewMessage(
  value: unknown,
): value is { type: "optionsChanged"; env: string; applyIncludes: boolean; applyEnvResolution: boolean } {
  if (!value || typeof value !== "object") {
    return false;
  }
  const v = value as Record<string, unknown>;
  return v.type === "optionsChanged" && typeof v.env === "string"
    && typeof v.applyIncludes === "boolean"
    && typeof v.applyEnvResolution === "boolean";
}

function isMap(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function toMap(value: unknown): Record<string, unknown> | null {
  return isMap(value) ? value : null;
}

async function refreshDiagnostics(document: vscode.TextDocument | undefined): Promise<void> {
  await refreshIncludeDiagnostics(document);
  await refreshSemanticDiagnostics(document);
}

async function refreshIncludeDiagnostics(document: vscode.TextDocument | undefined): Promise<void> {
  if (!document || document.languageId !== "yaml") {
    includeDiagnostics.clear();
    return;
  }

  const refs = collectIncludeFileRefs(document.getText());
  const diagnostics: vscode.Diagnostic[] = [];

  for (const ref of refs) {
    if (isTemplatedIncludePath(ref.path)) {
      continue;
    }
    const candidates = buildIncludeCandidates(ref.path, path.dirname(document.uri.fsPath));
    let found = false;
    for (const candidate of candidates) {
      try {
        // eslint-disable-next-line no-await-in-loop
        await access(candidate);
        found = true;
        break;
      } catch {
        // try next candidate
      }
    }
    if (found) {
      continue;
    }

    const range = new vscode.Range(new vscode.Position(ref.line, 0), new vscode.Position(ref.line, 200));
    const message = `Include file not found: ${ref.path}`;
    const d = new vscode.Diagnostic(range, message, vscode.DiagnosticSeverity.Warning);
    d.source = "helm-apps";
    diagnostics.push(d);
  }

  includeDiagnostics.set(document.uri, diagnostics);
}

async function refreshSemanticDiagnostics(document: vscode.TextDocument | undefined): Promise<void> {
  if (!document || document.languageId !== "yaml") {
    semanticDiagnostics.clear();
    return;
  }
  if (!(await isHelmAppsValuesDocument(document))) {
    semanticDiagnostics.clear();
    return;
  }

  const text = document.getText();
  const loaded = await loadExpandedValues(document);
  const analysis = analyzeIncludes(text, loaded.includeDefinitions);
  const diagnostics: vscode.Diagnostic[] = [];

  for (const unresolved of analysis.unresolvedUsages) {
    const lineText = document.lineAt(unresolved.line).text;
    const idx = lineText.indexOf(unresolved.name);
    const start = idx >= 0 ? idx : 0;
    const end = idx >= 0 ? idx + unresolved.name.length : lineText.length;
    const range = new vscode.Range(
      new vscode.Position(unresolved.line, start),
      new vscode.Position(unresolved.line, end),
    );
    const d = new vscode.Diagnostic(
      range,
      `Unresolved include profile: ${unresolved.name}`,
      vscode.DiagnosticSeverity.Warning,
    );
    d.source = "helm-apps";
    diagnostics.push(d);
  }

  for (const unused of analysis.unusedDefinitions) {
    const lineText = document.lineAt(unused.line).text;
    const idx = lineText.indexOf(unused.name);
    const start = idx >= 0 ? idx : 0;
    const end = idx >= 0 ? idx + unused.name.length : lineText.length;
    const range = new vscode.Range(
      new vscode.Position(unused.line, start),
      new vscode.Position(unused.line, end),
    );
    const d = new vscode.Diagnostic(
      range,
      `Unused include profile: ${unused.name}`,
      vscode.DiagnosticSeverity.Information,
    );
    d.source = "helm-apps";
    diagnostics.push(d);
  }

  semanticDiagnostics.set(document.uri, diagnostics);
}

function collectIncludeFileRefs(text: string): Array<{ path: string; line: number }> {
  const lines = text.split(/\r?\n/);
  const refs: Array<{ path: string; line: number }> = [];

  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const keyMatch = line.match(/^(\s*)(_include_from_file|_include_files):\s*(.*)$/);
    if (!keyMatch) {
      continue;
    }
    const indent = keyMatch[1].length;
    const key = keyMatch[2];
    const tail = keyMatch[3].trim();

    if (key === "_include_from_file") {
      const v = unquote(tail);
      if (v && !isTemplatedIncludePath(v)) {
        refs.push({ path: v, line: i });
      }
      continue;
    }

    if (tail.startsWith("[") && tail.endsWith("]")) {
      const inside = tail.slice(1, -1);
      for (const part of inside.split(",")) {
        const v = unquote(part.trim());
        if (v && !isTemplatedIncludePath(v)) {
          refs.push({ path: v, line: i });
        }
      }
      continue;
    }

    for (let j = i + 1; j < lines.length; j += 1) {
      const sub = lines[j];
      const t = sub.trim();
      if (t.length === 0 || t.startsWith("#")) {
        continue;
      }
      const subIndent = countIndent(sub);
      if (subIndent <= indent) {
        break;
      }
      const item = sub.match(/^\s*-\s+(.+)\s*$/);
      if (item) {
        const v = unquote(item[1].trim());
        if (v && !isTemplatedIncludePath(v)) {
          refs.push({ path: v, line: j });
        }
      }
    }
  }

  return refs;
}

function buildIncludeCandidates(rawPath: string, baseDir: string): string[] {
  if (path.isAbsolute(rawPath)) {
    return [rawPath];
  }
  return [path.resolve(baseDir, rawPath)];
}

function unquote(value: string): string {
  const v = value.trim();
  if (v.length < 2) {
    return v;
  }
  if ((v.startsWith("\"") && v.endsWith("\"")) || (v.startsWith("'") && v.endsWith("'"))) {
    return v.slice(1, -1);
  }
  return v;
}

function isTemplatedIncludePath(value: string): boolean {
  return value.includes("{{") || value.includes("}}");
}

function detectDefaultEnv(values: unknown, envDiscovery: EnvironmentDiscovery): string {
  const root = toMap(values);
  const globalEnv = root && typeof toMap(root.global)?.env === "string"
    ? String(toMap(root.global)?.env).trim()
    : "";
  if (globalEnv.length > 0) {
    return globalEnv;
  }
  return envDiscovery.literals[0] ?? "dev";
}

function tokenUnderCursor(line: string, char: number): string | null {
  const re = /[A-Za-z0-9_.-]+/g;
  for (const m of line.matchAll(re)) {
    const start = m.index ?? 0;
    const end = start + m[0].length;
    if (char >= start && char <= end) {
      return m[0];
    }
  }
  return null;
}

function countIndent(line: string): number {
  let n = 0;
  while (n < line.length && line[n] === " ") {
    n += 1;
  }
  return n;
}
