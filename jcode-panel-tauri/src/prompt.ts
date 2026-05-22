import { api } from './api';

export function renderPrompt(root: HTMLElement) {
  root.innerHTML = `
    <main class="prompt-shell">
      <div class="prompt-card">
        <span class="chip">Jcode</span>
        <input id="prompt-input" placeholder="Ask jcode, use @vscode or @obsidian..." autofocus />
        <button id="send">Send</button>
        <button id="close" title="Hide">×</button>
      </div>
      <div class="prompt-hint">F8 popup parity target · chips and screenshot flow will map to Rust modules</div>
    </main>`;
  const input = root.querySelector<HTMLInputElement>('#prompt-input')!;
  root.querySelector<HTMLButtonElement>('#close')!.onclick = () => api.hidePrompt();
  root.querySelector<HTMLButtonElement>('#send')!.onclick = async () => {
    const value = input.value.trim();
    if (!value) return;
    input.disabled = true;
    try {
      await api.submitPrompt(value);
      input.value = '';
      await api.hidePrompt();
    } finally {
      input.disabled = false;
      input.focus();
    }
  };
  input.addEventListener('keydown', async (event) => {
    if (event.key === 'Escape') await api.hidePrompt();
    if (event.key === 'Enter') root.querySelector<HTMLButtonElement>('#send')!.click();
  });
  setTimeout(() => input.focus(), 50);
}
