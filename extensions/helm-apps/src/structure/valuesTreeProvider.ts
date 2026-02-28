import * as vscode from "vscode";

import type { ValuesNode } from "./treeParser";
import { parseYamlKeyTree } from "./treeParser";

export class ValuesStructureProvider implements vscode.TreeDataProvider<ValuesNode> {
  private readonly onDidChangeTreeDataEmitter = new vscode.EventEmitter<ValuesNode | undefined | null | void>();
  readonly onDidChangeTreeData = this.onDidChangeTreeDataEmitter.event;

  private uri: vscode.Uri | null = null;
  private roots: ValuesNode[] = [];

  setDocument(document: vscode.TextDocument | undefined): void {
    if (!document || document.languageId !== "yaml") {
      this.uri = null;
      this.roots = [];
      this.refresh();
      return;
    }

    this.uri = document.uri;
    this.roots = parseYamlKeyTree(document.getText());
    this.refresh();
  }

  refresh(): void {
    this.onDidChangeTreeDataEmitter.fire();
  }

  getTreeItem(element: ValuesNode): vscode.TreeItem {
    const item = new vscode.TreeItem(
      element.label,
      element.children.length > 0
        ? vscode.TreeItemCollapsibleState.Collapsed
        : vscode.TreeItemCollapsibleState.None,
    );

    item.description = element.path;
    item.command = {
      command: "helm-apps.revealValuesNode",
      title: "Reveal values key",
      arguments: [this.uri, element.line],
    };

    return item;
  }

  getChildren(element?: ValuesNode): Thenable<ValuesNode[]> {
    if (!element) {
      return Promise.resolve(this.roots);
    }
    return Promise.resolve(element.children);
  }
}
