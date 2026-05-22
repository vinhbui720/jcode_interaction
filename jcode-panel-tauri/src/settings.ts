import { api } from './api';

export async function renderSettings(root: HTMLElement) {
  const snapshot = await api.snapshot();
  const cfg = snapshot.config;
  root.innerHTML = `
    <main class="settings-shell">
      <header><h1>Settings</h1><p>Modern settings UI, backed by persistent config.</p></header>
      <label>Prompt hotkey <input id="promptHotkey" value="${cfg.prompt_hotkey}" /></label>
      <label>Screenshot hotkey <input id="screenshotHotkey" value="${cfg.screenshot_hotkey}" /></label>
      <label>Terminal <input id="terminal" value="${cfg.terminal}" /></label>
      <label>Max prompt chars <input id="maxPromptChars" type="number" value="${cfg.max_prompt_chars}" /></label>
      <label class="row"><input id="sendContext" type="checkbox" ${cfg.send_context_default ? 'checked' : ''} /> Send context by default</label>
      <button id="save">Save settings</button>
      <pre id="status"></pre>
    </main>`;
  root.querySelector<HTMLButtonElement>('#save')!.onclick = async () => {
    const newConfig = {
      prompt_hotkey: root.querySelector<HTMLInputElement>('#promptHotkey')!.value,
      screenshot_hotkey: root.querySelector<HTMLInputElement>('#screenshotHotkey')!.value,
      terminal: root.querySelector<HTMLInputElement>('#terminal')!.value,
      max_prompt_chars: Number(root.querySelector<HTMLInputElement>('#maxPromptChars')!.value || 4000),
      send_context_default: root.querySelector<HTMLInputElement>('#sendContext')!.checked,
    };
    await api.saveSettings(newConfig);
    root.querySelector<HTMLPreElement>('#status')!.textContent = 'Saved';
  };
}
