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
  const send = root.querySelector<HTMLButtonElement>('#send')!;
  let submitting = false;
  let followTimer: number | undefined;

  const stopFollowing = () => {
    if (followTimer) window.clearInterval(followTimer);
    followTimer = undefined;
  };
  const startFollowing = () => {
    stopFollowing();
    followTimer = window.setInterval(async () => {
      try {
        const keepGoing = await api.promptFollowMouseTick();
        if (!keepGoing) stopFollowing();
      } catch {
        stopFollowing();
      }
    }, 16);
  };
  startFollowing();

  try {
    const [ctx, chips] = await Promise.all([api.activeContextSnapshot(), api.popupContextChips()]);
    if (!input.value && chips.length) {
      input.value = `${chips.map((chip) => chip.tag).join(' ')} `;
    }
    let browserHost = '';
    try { browserHost = ctx.browser?.url ? new URL(ctx.browser.url).host : ''; } catch { browserHost = ctx.browser?.title ?? ''; }
    const summary = [ctx.app || (browserHost ? 'Browser' : ''), browserHost || ctx.window_title, (ctx.browser?.selected_text || ctx.selected_text) ? 'selected text' : ''].filter(Boolean).join(' · ');
    if (summary) hint.textContent = `Context: ${summary}`;
  } catch {
    // Context capture is best-effort, keep prompt usable without X11 helpers.
  }

  const hide = async () => {
    stopFollowing();
    await api.hidePrompt();
  };

  const submit = async () => {
    if (submitting) return;
    const value = input.value.trim();
    if (!value) return;
    submitting = true;
    stopFollowing();
    input.disabled = true;
    send.disabled = true;
    try {
      await api.showFeedback('Sending prompt to jcode...', 'Working');
      const result = await api.submitPrompt(value);
      await api.showFeedback(result.output || 'Done.', result.ok ? 'jcode response complete' : 'jcode returned an error', result.token_stats ?? null);
      input.value = '';
      await api.hidePrompt();
    } catch (error) {
      await api.showFeedback(String(error || 'Prompt failed'), 'Error');
      input.disabled = false;
      send.disabled = false;
      input.focus();
      submitting = false;
    }
  };

  root.querySelector<HTMLButtonElement>('#close')!.onclick = hide;
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
  send.onclick = submit;
  input.addEventListener('keydown', async (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      await hide();
      return;
    }
    if (event.key === 'Tab') {
      event.preventDefault();
      const result = await api.normalizePromptText(input.value);
      input.value = result.text;
      input.setSelectionRange(input.value.length, input.value.length);
      hint.textContent = result.hints.length ? `Context: ${result.hints.join(', ')}` : 'Context chip ready';
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      await submit();
    }
  });
  input.addEventListener('input', async () => {
    const result = await api.normalizePromptText(input.value);
    if (result.text !== input.value) {
      input.value = result.text;
      input.setSelectionRange(input.value.length, input.value.length);
    }
    hint.textContent = result.hints.length ? `Context: ${result.hints.join(', ')}` : 'Use @vscode/@obsidian for live app context, 📷 for a screenshot tag.';
  });
  window.addEventListener('beforeunload', stopFollowing);
  setTimeout(() => input.focus(), 50);
}
