import { api } from './api';

export async function renderPrompt(root: HTMLElement) {
  root.innerHTML = `
    <main class="prompt-shell">
      <div class="prompt-card">
        <span class="chip">Jcode</span>
        <input id="prompt-input" placeholder="Ask jcode, use @vscode or @obsidian..." autofocus />
        <button id="shot" title="Capture area screenshot">📷</button>
        <button id="send">Send</button>
        <button id="close" title="Hide">×</button>
      </div>
      <div id="prompt-hint" class="prompt-hint">Use @vscode/@obsidian for live app context, 📷 for a screenshot tag.</div>
    </main>`;
  const input = root.querySelector<HTMLInputElement>('#prompt-input')!;
  const hint = root.querySelector<HTMLDivElement>('#prompt-hint')!;
  try {
    const [ctx, chips] = await Promise.all([api.activeContextSnapshot(), api.popupContextChips()]);
    if (!input.value && chips.length) {
      input.value = `${chips.map((chip) => chip.tag).join(' ')} `;
    }
    const summary = [ctx.app, ctx.window_title, ctx.selected_text ? 'selected text' : ''].filter(Boolean).join(' · ');
    if (summary) hint.textContent = `Context: ${summary}`;
  } catch {
    // Context capture is best-effort, keep prompt usable without X11 helpers.
  }
  root.querySelector<HTMLButtonElement>('#close')!.onclick = () => api.hidePrompt();
  root.querySelector<HTMLButtonElement>('#shot')!.onclick = async () => {
    hint.textContent = 'Capturing screenshot...';
    try {
      const tag = await api.captureScreenshot('area');
      input.value = `${input.value.trim()} ${tag}`.trim();
      hint.textContent = `Attached ${tag}`;
      input.focus();
    } catch (error) {
      hint.textContent = String(error || 'Screenshot failed');
    }
  };
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
    if (event.key === 'Tab') {
      event.preventDefault();
      const result = await api.normalizePromptText(input.value);
      input.value = result.text;
      input.setSelectionRange(input.value.length, input.value.length);
      hint.textContent = result.hints.length ? `Context: ${result.hints.join(', ')}` : 'Context chip ready';
    }
    if (event.key === 'Enter') root.querySelector<HTMLButtonElement>('#send')!.click();
  });
  input.addEventListener('input', async () => {
    const result = await api.normalizePromptText(input.value);
    if (result.text !== input.value) {
      input.value = result.text;
      input.setSelectionRange(input.value.length, input.value.length);
    }
    hint.textContent = result.hints.length ? `Context: ${result.hints.join(', ')}` : 'Use @vscode/@obsidian for live app context, 📷 for a screenshot tag.';
  });
  setTimeout(() => input.focus(), 50);
}
