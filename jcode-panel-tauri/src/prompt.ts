import { listen } from '@tauri-apps/api/event';
import { api } from './api';

export function renderPrompt(root: HTMLElement) {
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
  const close = root.querySelector<HTMLButtonElement>('#close')!;
  const shot = root.querySelector<HTMLButtonElement>('#shot')!;
  let submitting = false;
  let followTimer: number | undefined;
  let followBusy = false;
  let hidden = false;

  const stopFollowing = () => {
    if (followTimer) window.clearInterval(followTimer);
    followTimer = undefined;
    followBusy = false;
  };

  const startFollowing = () => {
    if (submitting || hidden) return;
    stopFollowing();
    input.disabled = false;
    send.disabled = false;
    setTimeout(() => input.focus(), 10);
    // 40ms is smooth enough for a popup and avoids saturating WebKit/Tauri with xdotool calls.
    followTimer = window.setInterval(async () => {
      if (followBusy || submitting || hidden) return;
      followBusy = true;
      try {
        const keepGoing = await api.promptFollowMouseTick();
        if (!keepGoing) stopFollowing();
      } catch {
        stopFollowing();
      } finally {
        followBusy = false;
      }
    }, 40);
  };

  const hide = async () => {
    hidden = true;
    stopFollowing();
    try { await api.hidePrompt(); } catch { /* keep UI responsive even if window already hidden */ }
  };

  const submit = async () => {
    if (submitting) return;
    const value = input.value.trim();
    if (!value) return;
    submitting = true;
    hidden = true;
    stopFollowing();
    input.disabled = true;
    send.disabled = true;
    try {
      await api.hidePrompt();
      await api.showFeedback('Sending prompt to jcode...', 'Working');
      const result = await api.submitPrompt(value);
      await api.showFeedback(result.output || 'Done.', result.ok ? 'jcode response complete' : 'jcode returned an error', result.token_stats ?? null);
      input.value = '';
    } catch (error) {
      hidden = false;
      await api.showFeedback(String(error || 'Prompt failed'), 'Error');
      input.disabled = false;
      send.disabled = false;
      input.focus();
      startFollowing();
    } finally {
      submitting = false;
    }
  };

  close.onclick = () => { void hide(); };
  send.onclick = () => { void submit(); };
  shot.onclick = async () => {
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

  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      void hide();
      return;
    }
    if (event.key === 'Tab') {
      event.preventDefault();
      void api.normalizePromptText(input.value).then((result) => {
        input.value = result.text;
        input.setSelectionRange(input.value.length, input.value.length);
        hint.textContent = result.hints.length ? `Context: ${result.hints.join(', ')}` : 'Context chip ready';
      });
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      void submit();
    }
  });

  input.addEventListener('input', () => {
    void api.normalizePromptText(input.value).then((result) => {
      if (result.text !== input.value) {
        input.value = result.text;
        input.setSelectionRange(input.value.length, input.value.length);
      }
      hint.textContent = result.hints.length ? `Context: ${result.hints.join(', ')}` : 'Use @vscode/@obsidian for live app context, 📷 for a screenshot tag.';
    }).catch(() => {});
  });

  // Non-blocking: if event registration stalls, buttons and Escape still work.
  void listen('prompt-shown', () => {
    hidden = false;
    startFollowing();
  });

  void Promise.all([api.activeContextSnapshot(), api.popupContextChips()]).then(([ctx, chips]) => {
    if (!input.value && chips.length) {
      input.value = `${chips.map((chip) => chip.tag).join(' ')} `;
    }
    let browserHost = '';
    try { browserHost = ctx.browser?.url ? new URL(ctx.browser.url).host : ''; } catch { browserHost = ctx.browser?.title ?? ''; }
    const summary = [ctx.app || (browserHost ? 'Browser' : ''), browserHost || ctx.window_title, (ctx.browser?.selected_text || ctx.selected_text) ? 'selected text' : ''].filter(Boolean).join(' · ');
    if (summary) hint.textContent = `Context: ${summary}`;
  }).catch(() => {});

  window.addEventListener('beforeunload', stopFollowing);
  setTimeout(() => input.focus(), 50);
  startFollowing();
}
