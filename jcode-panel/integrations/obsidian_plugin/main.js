module.exports = class JcodePanelPlugin extends require('obsidian').Plugin {
  async onload() {
    this.addCommand({
      id: 'send-active-note-context-to-jcode-panel',
      name: 'Send active note context to jcode-panel bridge',
      callback: async () => {
        const file = this.app.workspace.getActiveFile();
        const view = this.app.workspace.getActiveViewOfType(require('obsidian').MarkdownView);
        const selectedText = view && view.editor ? view.editor.getSelection() : '';
        try {
          await fetch('http://127.0.0.1:8765/', {
            method: 'POST',
            headers: {'content-type': 'application/json'},
            body: JSON.stringify({
              title: file ? file.basename : 'Obsidian',
              url: file ? 'obsidian://' + file.path : 'obsidian://',
              selectedText
            })
          });
        } catch (_) {}
      }
    });
  }
};
