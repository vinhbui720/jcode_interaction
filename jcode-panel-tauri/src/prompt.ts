import { listen } from '@tauri-apps/api/event';
import { api } from './api';

type Suggestion = { value: string; label: string; detail: string };

const TARGETS: Suggestion[] = [
  { value: '@vscode', label: '@vscode', detail: 'active VS Code file, selection, diagnostics' },
  { value: '@obsidian', label: '@obsidian', detail: 'active Obsidian note and selection' },
];

const COMMANDS: Suggestion[] = [
  { value: '/model', label: '/model', detail: 'choose or switch model' },
  { value: '/usage', label: '/usage', detail: 'show provider usage limits' },
  { value: '/screen-shot', label: '/screen-shot', detail: 'capture a picture for the prompt' },
  { value: '/screenshot', label: '/screenshot', detail: 'alias for /screen-shot' },
  { value: '/help', label: '/help', detail: 'show panel command help' },
  { value: '/resume', label: '/resume', detail: 'resume a known jcode session' },
  { value: '/clear', label: '/clear', detail: 'clear conversation context in jcode' },
  { value: '/compact', label: '/compact', detail: 'compact the current jcode context' },
  { value: '/skill', label: '/skill', detail: 'run a jcode skill' },
  { value: '/memory', label: '/memory', detail: 'query or update jcode memory' },
];

function currentToken(value: string, cursor: number) {
  const prefix = value.slice(0, cursor);
  const match = prefix.match(/(^|\s)([@/][^\s]*)$/);
  if (!match) return null;
  const token = match[2];
  const start = cursor - token.length;
  return { token, start, end: cursor };
}

function suggestionsFor(value: string, cursor: number, screenshotHotkey: string): Suggestion[] {
  const active = currentToken(value, cursor);
  if (!active) {
    return [
      { value: '@', label: '@ target', detail: 'target app context' },
      { value: '/', label: '/ command', detail: 'panel command' },
      { value: screenshotHotkey, label: screenshotHotkey, detail: 'picture screenshot hotkey' },
    ];
  }
  const raw = active.token.toLowerCase();
  if (raw.startsWith('@')) return TARGETS.filter((item) => item.value.startsWith(raw));
  if (raw.startsWith('/')) return COMMANDS.filter((item) => item.value.startsWith(raw));
  return [];
}

function applyCompletion(input: HTMLInputElement, suggestion: Suggestion) {
  const cursor = input.selectionStart ?? input.value.length;
  const active = currentToken(input.value, cursor);
  if (!suggestion.value.startsWith('@') && !suggestion.value.startsWith('/')) return false;
  if (!active) {
    input.value = `${input.value.slice(0, cursor)}${suggestion.value}${input.value.slice(cursor)}`;
    const next = cursor + suggestion.value.length;
    input.setSelectionRange(next, next);
    return true;
  }
  const suffix = suggestion.value.startsWith('@') ? ' ' : ' ';
  input.value = `${input.value.slice(0, active.start)}${suggestion.value}${suffix}${input.value.slice(active.end)}`;
  const next = active.start + suggestion.value.length + suffix.length;
  input.setSelectionRange(next, next);
  return true;
}

export function renderPrompt(root: HTMLElement) {
  root.innerHTML = `
    <main class="prompt-shell">
      <div class="prompt-card">
        <span class="prompt-logo">JI</span>
        <input id="prompt-input" placeholder="Ask jcode..." autofocus />
        <span id="prompt-count" class="prompt-count">0/4000</span>
      </div>
      <div id="prompt-suggestions" class="prompt-suggestions"></div>
    </main>`;

  const input = root.querySelector<HTMLInputElement>('#prompt-input')!;
  const count = root.querySelector<HTMLSpanElement>('#prompt-count')!;
  const suggestionEl = root.querySelector<HTMLDivElement>('#prompt-suggestions')!;
  let submitting = false;
  let maxChars = 4000;
  let screenshotHotkey = 'screenshot hotkey';
  let currentSuggestions: Suggestion[] = [];
  let selectedSuggestion = 0;

  const focusInput = () => {
    input.disabled = false;
    requestAnimationFrame(() => {
      input.focus();
      input.select();
    });
    setTimeout(() => input.focus(), 40);
    setTimeout(() => input.focus(), 120);
  };

  const hide = async () => {
    try { await api.hidePrompt(); } catch { /* keep UI responsive */ }
  };

  const submit = async () => {
    if (submitting) return;
    const value = input.value.trim();
    if (!value) return;
    if (value === '/screen-shot' || value === '/screenshot') {
      input.disabled = true;
      try {
        const tag = await api.captureScreenshot('area');
        input.value = `${tag} `;
        updateUi();
      } catch (error) {
        await api.showFeedback(String(error || 'Screenshot failed'), 'Error');
      } finally {
        input.disabled = false;
        focusInput();
      }
      return;
    }
    submitting = true;
    input.disabled = true;
    try {
      await api.hidePrompt();
      await api.submitPromptAsync(value);
      input.value = '';
      updateUi();
    } catch (error) {
      await api.showFeedback(String(error || 'Prompt failed'), 'Error');
      input.disabled = false;
      focusInput();
    } finally {
      submitting = false;
    }
  };

  const renderSuggestions = () => {
    suggestionEl.innerHTML = currentSuggestions.map((item, index) => `
      <button class="prompt-suggestion ${index === selectedSuggestion ? 'active' : ''}" data-index="${index}">
        <strong>${item.label}</strong><span>${item.detail}</span>
      </button>`).join('');
    suggestionEl.querySelectorAll<HTMLButtonElement>('.prompt-suggestion').forEach((button) => {
      button.onclick = () => {
        const index = Number(button.dataset.index || '0');
        const item = currentSuggestions[index];
        if (item && applyCompletion(input, item)) updateUi();
        focusInput();
      };
    });
  };

  const updateUi = () => {
    count.textContent = `${input.value.length}/${maxChars}`;
    count.classList.toggle('over', input.value.length > maxChars);
    const cursor = input.selectionStart ?? input.value.length;
    currentSuggestions = suggestionsFor(input.value, cursor, screenshotHotkey);
    selectedSuggestion = Math.min(selectedSuggestion, Math.max(0, currentSuggestions.length - 1));
    renderSuggestions();
  };

  input.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      void hide();
      return;
    }
    if (event.key === 'ArrowRight' && event.altKey && currentSuggestions.length) {
      event.preventDefault();
      selectedSuggestion = (selectedSuggestion + 1) % currentSuggestions.length;
      renderSuggestions();
      return;
    }
    if (event.key === 'ArrowLeft' && event.altKey && currentSuggestions.length) {
      event.preventDefault();
      selectedSuggestion = (selectedSuggestion - 1 + currentSuggestions.length) % currentSuggestions.length;
      renderSuggestions();
      return;
    }
    if (event.key === 'Tab') {
      event.preventDefault();
      const selected = currentSuggestions[selectedSuggestion];
      if (selected && applyCompletion(input, selected)) {
        updateUi();
        return;
      }
      void api.normalizePromptText(input.value).then((result) => {
        input.value = result.text;
        input.setSelectionRange(input.value.length, input.value.length);
        updateUi();
      });
      return;
    }
    if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      void submit();
    }
  });

  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      void hide();
    }
  });

  window.addEventListener('blur', () => {
    window.setTimeout(() => {
      if (document.visibilityState === 'visible' && document.activeElement !== input) {
        void hide();
      }
    }, 80);
  });

  input.addEventListener('input', () => {
    selectedSuggestion = 0;
    updateUi();
    void api.normalizePromptText(input.value).then((result) => {
      if (result.text !== input.value) {
        input.value = result.text;
        input.setSelectionRange(input.value.length, input.value.length);
        updateUi();
      }
    }).catch(() => {});
  });
  input.addEventListener('click', updateUi);
  input.addEventListener('keyup', updateUi);

  void listen('prompt-shown', () => focusInput());

  void api.snapshot().then((snapshot) => {
    maxChars = snapshot.config.max_prompt_chars || maxChars;
    screenshotHotkey = snapshot.config.screenshot_hotkey || screenshotHotkey;
    updateUi();
  }).catch(updateUi);

  void Promise.all([api.activeContextSnapshot(), api.popupContextChips()]).then(([ctx, chips]) => {
    if (!input.value && chips.length) {
      input.value = `${chips.map((chip) => chip.tag).join(' ')} `;
    }
    let browserHost = '';
    try { browserHost = ctx.browser?.url ? new URL(ctx.browser.url).host : ''; } catch { browserHost = ctx.browser?.title ?? ''; }
    const summary = [ctx.app || (browserHost ? 'Browser' : ''), browserHost || ctx.window_title, (ctx.browser?.selected_text || ctx.selected_text) ? 'selected text' : ''].filter(Boolean).join(' · ');
    if (summary) currentSuggestions = [{ value: '', label: 'context', detail: summary }];
    updateUi();
  }).catch(() => {});

  updateUi();
  focusInput();
}
