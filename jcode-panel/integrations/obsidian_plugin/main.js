const { Plugin, MarkdownView } = require('obsidian');
const fs = require('fs');
const os = require('os');
const path = require('path');

const contextPath = path.join(os.homedir(), '.local', 'state', 'jcode-panel', 'contexts', 'obsidian.json');

module.exports = class JcodePanelPlugin extends Plugin {
  async onload() {
    const writeActiveContext = () => this.writeActiveContext();
    this.registerEvent(this.app.workspace.on('active-leaf-change', writeActiveContext));
    this.registerEvent(this.app.workspace.on('editor-change', writeActiveContext));
    this.registerEvent(this.app.workspace.on('file-open', writeActiveContext));
    this.addCommand({
      id: 'write-active-note-context-for-jcode-panel',
      name: 'Write active note context for jcode-panel',
      callback: writeActiveContext
    });
    writeActiveContext();
  }

  writeActiveContext() {
    const file = this.app.workspace.getActiveFile();
    const view = this.app.workspace.getActiveViewOfType(MarkdownView);
    if (!file || !view || !view.editor) return;
    const editor = view.editor;
    const cursor = editor.getCursor();
    const selection = editor.getSelection() || '';
    const fullText = editor.getValue() || '';
    const lines = fullText.split(/\r?\n/);
    const start = Math.max(0, cursor.line - 80);
    const end = Math.min(lines.length, cursor.line + 81);
    const excerpt = lines.slice(start, end).map((line, idx) => `${start + idx + 1}: ${line}`).join('\n');
    const payload = {
      app: 'obsidian',
      title: file.basename,
      path: file.path,
      line: cursor.line + 1,
      column: cursor.ch + 1,
      selection: selection.slice(0, 12000),
      text: excerpt.slice(0, 12000),
      timestamp: new Date().toISOString()
    };
    fs.mkdirSync(path.dirname(contextPath), { recursive: true });
    fs.writeFileSync(contextPath, JSON.stringify(payload, null, 2));
  }
};
