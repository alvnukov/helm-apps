import * as vscode from "vscode";

interface ActionItemDef {
  readonly label: string;
  readonly description: string;
  readonly command: string;
  readonly icon: vscode.ThemeIcon;
}

class ActionItem extends vscode.TreeItem {
  constructor(def: ActionItemDef) {
    super(def.label, vscode.TreeItemCollapsibleState.None);
    this.description = def.description;
    this.tooltip = `${def.label}\n${def.description}`;
    this.iconPath = def.icon;
    this.command = {
      command: def.command,
      title: def.label,
    };
  }
}

export class HelmAppsWorkbenchActionsProvider implements vscode.TreeDataProvider<ActionItem> {
  private readonly ru: boolean;
  private readonly onDidChangeTreeDataEmitter = new vscode.EventEmitter<ActionItem | undefined | void>();
  readonly onDidChangeTreeData = this.onDidChangeTreeDataEmitter.event;

  constructor(locale: string) {
    this.ru = locale.toLowerCase().startsWith("ru");
  }

  getTreeItem(element: ActionItem): vscode.TreeItem {
    return element;
  }

  getChildren(): Thenable<ActionItem[]> {
    return Promise.resolve(this.items().map((d) => new ActionItem(d)));
  }

  private items(): ActionItemDef[] {
    if (this.ru) {
      return [
        {
          label: "Создать стартовый чарт",
          description: "Каркас чарта + оффлайн библиотека в charts/helm-apps",
          command: "helm-apps.createStarterChart",
          icon: new vscode.ThemeIcon("new-folder"),
        },
        {
          label: "Превью сущности",
          description: "Резолв _include/env и предпросмотр выбранного apps-* блока",
          command: "helm-apps.previewResolvedEntity",
          icon: new vscode.ThemeIcon("preview"),
        },
        {
          label: "Проверить текущий файл",
          description: "Запустить диагностику include/policy/semantic",
          command: "helm-apps.validateCurrentFile",
          icon: new vscode.ThemeIcon("check"),
        },
        {
          label: "Настройки библиотеки",
          description: "Визуальная настройка global.validation/deploy/labels",
          command: "helm-apps.openLibrarySettings",
          icon: new vscode.ThemeIcon("settings-gear"),
        },
        {
          label: "Вставить как helm-apps",
          description: "Преобразовать манифесты из буфера в values-формат",
          command: "helm-apps.pasteAsHelmApps",
          icon: new vscode.ThemeIcon("clippy"),
        },
        {
          label: "Источник библиотеки",
          description: "Переключить GitHub/локальный источник и кэш чарта",
          command: "helm-apps.manageLibrarySource",
          icon: new vscode.ThemeIcon("repo"),
        },
        {
          label: "Обновить dependency",
          description: "Обновить helm-apps dependency в ближайшем Chart.yaml",
          command: "helm-apps.updateLibraryDependency",
          icon: new vscode.ThemeIcon("package"),
        },
        {
          label: "Обновить Chart.lock",
          description: "Запустить helm dependency update для чарта",
          command: "helm-apps.updateLibraryLockfile",
          icon: new vscode.ThemeIcon("sync"),
        },
      ];
    }

    return [
      {
        label: "Create Starter Chart",
        description: "Scaffold chart + offline library in charts/helm-apps",
        command: "helm-apps.createStarterChart",
        icon: new vscode.ThemeIcon("new-folder"),
      },
      {
        label: "Preview Entity",
        description: "Resolve include/env and preview selected apps-* block",
        command: "helm-apps.previewResolvedEntity",
        icon: new vscode.ThemeIcon("preview"),
      },
      {
        label: "Validate Current File",
        description: "Run include/policy/semantic diagnostics",
        command: "helm-apps.validateCurrentFile",
        icon: new vscode.ThemeIcon("check"),
      },
      {
        label: "Library Settings",
        description: "Visual editor for global.validation/deploy/labels",
        command: "helm-apps.openLibrarySettings",
        icon: new vscode.ThemeIcon("settings-gear"),
      },
      {
        label: "Paste as Helm Apps",
        description: "Convert manifests from clipboard into values format",
        command: "helm-apps.pasteAsHelmApps",
        icon: new vscode.ThemeIcon("clippy"),
      },
      {
        label: "Library Source",
        description: "Switch GitHub/local source and manage chart cache",
        command: "helm-apps.manageLibrarySource",
        icon: new vscode.ThemeIcon("repo"),
      },
      {
        label: "Update Dependency",
        description: "Update helm-apps dependency in nearest Chart.yaml",
        command: "helm-apps.updateLibraryDependency",
        icon: new vscode.ThemeIcon("package"),
      },
      {
        label: "Update Chart.lock",
        description: "Run helm dependency update for target chart",
        command: "helm-apps.updateLibraryLockfile",
        icon: new vscode.ThemeIcon("sync"),
      },
    ];
  }
}
