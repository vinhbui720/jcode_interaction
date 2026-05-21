const vscode = require('vscode');
const fs = require('fs');
const os = require('os');
const path = require('path');

const contextPath = path.join(os.homedir(), '.local', 'state', 'jcode-panel', 'contexts', 'vscode.json');

function ensureDir(filePath) {
  fs.mkdirSync(path.dirname(filePath), { recursive: true });
}

function selectionText(editor) {
  if (!editor || editor.selection.isEmpty) return '';
  return editor.document.getText(editor.selection).slice(0, 12000);
}

function writeContext(editor) {
  if (!editor || editor.document.isUntitled) return;
  const doc = editor.document;
  const pos = editor.selection.active;
  const workspaceFolder = vscode.workspace.getWorkspaceFolder(doc.uri);
  const payload = {
    app: 'vscode',
    file: doc.fileName,
    line: pos.line + 1,
    column: pos.character + 1,
    selection: selectionText(editor),
    languageId: doc.languageId,
    workspaceRoot: workspaceFolder ? workspaceFolder.uri.fsPath : '',
    timestamp: new Date().toISOString()
  };
  ensureDir(contextPath);
  fs.writeFileSync(contextPath, JSON.stringify(payload, null, 2));
}

function activate(context) {
  const writeActive = () => writeContext(vscode.window.activeTextEditor);
  context.subscriptions.push(vscode.commands.registerCommand('jcodePanel.writeContext', writeActive));
  context.subscriptions.push(vscode.window.onDidChangeActiveTextEditor(writeContext));
  context.subscriptions.push(vscode.window.onDidChangeTextEditorSelection(e => writeContext(e.textEditor)));
  context.subscriptions.push(vscode.workspace.onDidSaveTextDocument(() => writeActive()));
  writeActive();
}

function deactivate() {}

module.exports = { activate, deactivate };
