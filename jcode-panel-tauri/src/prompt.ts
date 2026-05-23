import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import { api, type PopupContextChip } from './api';

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
      <div id="prompt-preview" class="prompt-preview" hidden></div>
      <div class="prompt-card">
        <span class="prompt-logo">JI</span>
        <input id="prompt-input" placeholder="Ask jcode..." autofocus />
        <span id="prompt-count" class="prompt-count">0/4000</span>
      </div>
      <div id="prompt-suggestions" class="prompt-suggestions"></div>
    </main>`;

  const input = root.querySelector<HTMLInputElement>('#prompt-input')!;
  const count = root.querySelector<HTMLSpanElement>('#prompt-count')!;
  const previewEl = root.querySelector<HTMLDivElement>('#prompt-preview')!;
  const suggestionEl = root.querySelector<HTMLDivElement>('#prompt-suggestions')!;
  let submitting = false;
  let maxChars = 4000;
  let screenshotHotkey = 'screenshot hotkey';
  let currentSuggestions: Suggestion[] = [];
  let selectedSuggestion = 0;
  let clipboardSeq = 0;
  const clipboardPastes = new Map<string, string>();
  const selectedChips = new Map<string, string>();
  let picSeq = 0;
  const picTags = new Map<string, string>();
  const picPreviews = new Map<string, string>();
  let contextSignature = '';

  const expandClipboardTags = (text: string) => {
    let expanded = text;
    for (const [tag, body] of clipboardPastes) {
      expanded = expanded.split(tag).join(body);
    }
    for (const [tag, body] of selectedChips) {
      expanded = expanded.split(tag).join(body);
    }
    for (const [tag, body] of picTags) {
      expanded = expanded.split(tag).join(body);
    }
    return expanded;
  };

  const promptLength = () => expandClipboardTags(input.value).length;

  const insertTextAtCursor = (text: string) => {
    const start = input.selectionStart ?? input.value.length;
    const end = input.selectionEnd ?? start;
    input.value = `${input.value.slice(0, start)}${text}${input.value.slice(end)}`;
    const next = start + text.length;
    input.setSelectionRange(next, next);
  };

  const screenshotPathFromTag = (tag: string) => tag.match(/^\[screenshot:(.*)\]$/)?.[1] || '';

  const allKnownChips = () => [...clipboardPastes.keys(), ...selectedChips.keys(), ...picTags.keys()];

  const chipAroundCursor = (direction: 'backward' | 'forward') => {
    const start = input.selectionStart ?? input.value.length;
    const end = input.selectionEnd ?? start;
    if (start !== end) return null;
    for (const chip of allKnownChips()) {
      let index = input.value.indexOf(chip);
      while (index !== -1) {
        const chipEnd = index + chip.length;
        const cursorInChip = start > index && start < chipEnd;
        const cursorAtEdge = direction === 'backward' ? start === chipEnd : start === index;
        const cursorAfterSpace = direction === 'backward' && start === chipEnd + 1 && input.value[chipEnd] === ' ';
        if (cursorInChip || cursorAtEdge || cursorAfterSpace) {
          return { start: index, end: cursorAfterSpace ? chipEnd + 1 : chipEnd };
        }
        index = input.value.indexOf(chip, chipEnd);
      }
    }
    return null;
  };

  const resizePromptWindow = (hasPreview: boolean) => {
    const height = hasPreview ? 238 : 112;
    void getCurrentWindow().setSize(new LogicalSize(720, height)).catch(() => {});
  };

  const updatePreview = () => {
    const activePics = [...picTags.entries()].filter(([tag]) => input.value.includes(tag));
    previewEl.hidden = activePics.length === 0;
    resizePromptWindow(activePics.length > 0);
    previewEl.innerHTML = activePics.map(([tag, fullTag]) => {
      const path = screenshotPathFromTag(fullTag);
      const src = picPreviews.get(tag) || '';
      if (!src && path) {
        void api.imageDataUrl(path).then((url) => {
          picPreviews.set(tag, url);
          updatePreview();
        }).catch(() => {});
      }
      return `
        <figure class="prompt-shot">
          <img src="${src}" alt="${tag}" />
          <figcaption>${tag}</figcaption>
        </figure>`;
    }).join('');
  };

  const focusInput = () => {
    input.disabled = false;
    const placeCursorAtEnd = () => input.setSelectionRange(input.value.length, input.value.length);
    const focusNow = () => {
      input.focus({ preventScroll: true });
      placeCursorAtEnd();
    };
    focusNow();
    requestAnimationFrame(focusNow);
    for (const delay of [20, 60, 120, 240, 500, 900]) {
      window.setTimeout(focusNow, delay);
    }
  };

  const applySelectedContextChips = (chips: PopupContextChip[]) => {
    const selected = chips.filter((chip) => chip.tag.startsWith('[selected'));
    const signature = selected.map((chip) => `${chip.tag}:${chip.body}`).join('\n---\n');
    if (!selected.length || signature === contextSignature) return;
    contextSignature = signature;
    selectedChips.clear();
    selected.forEach((chip) => selectedChips.set(chip.tag, chip.body));
    const tags = `${selected.map((chip) => chip.tag).join(' ')} `;
    const knownSelected = /^\s*(?:\[selected\d+\]\s*)+/.exec(input.value);
    if (knownSelected) {
      input.value = `${tags}${input.value.slice(knownSelected[0].length)}`;
    } else if (!input.value.trim()) {
      input.value = tags;
    } else {
      input.value = `${tags}${input.value}`;
    }
    input.setSelectionRange(input.value.length, input.value.length);
    updateUi();
  };

  (window as unknown as { __jcodeApplyPromptContextChips?: (chips: PopupContextChip[]) => void })
    .__jcodeApplyPromptContextChips = (chips) => {
      applySelectedContextChips(chips || []);
      focusInput();
    };

  const loadPromptContext = async () => {
    try {
      const chips = await api.popupContextChips();
      applySelectedContextChips(chips);
      if (chips.length) {
        const summary = chips.map((chip) => chip.kind).join(' · ');
        currentSuggestions = [{ value: '', label: 'context', detail: summary }];
      }
      updateUi();
    } catch {
      updateUi();
    }
  };

  const hide = async () => {
    try { await api.hidePrompt(); } catch { /* keep UI responsive */ }
  };

  const submit = async () => {
    if (submitting) return;
    const value = expandClipboardTags(input.value).trim();
    if (!value) return;
    if (value.length > maxChars) {
      await api.showFeedback(`Prompt is ${value.length}/${maxChars} characters after expanding chips. Shorten or remove a [selected#]/[clipboard#] chip before sending.`, 'Prompt too long');
      return;
    }
    const screenshotMatch = value.match(/^\/(?:screen-shot|screenshot)(?:\s+(.*))?$/i);
    if (screenshotMatch) {
      input.disabled = true;
      try {
        const tag = await api.captureScreenshot('area');
        const rest = screenshotMatch[1]?.trim();
        picSeq += 1;
        const picTag = `[pic${picSeq}]`;
        picTags.set(picTag, tag);
        const path = screenshotPathFromTag(tag);
        if (path) void api.imageDataUrl(path).then((url) => {
          picPreviews.set(picTag, url);
          updatePreview();
        }).catch(() => {});
        input.value = rest ? `${picTag} ${rest}` : `${picTag} `;
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
      await api.submitPromptAsync(value);
      await api.hidePrompt();
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
    const length = promptLength();
    count.textContent = `${length}/${maxChars}`;
    count.classList.toggle('over', length > maxChars);
    const cursor = input.selectionStart ?? input.value.length;
    currentSuggestions = suggestionsFor(input.value, cursor, screenshotHotkey);
    selectedSuggestion = Math.min(selectedSuggestion, Math.max(0, currentSuggestions.length - 1));
    updatePreview();
    renderSuggestions();
  };

  const handleEscape = (event: KeyboardEvent) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation();
      void hide();
      return true;
    }
    return false;
  };

  window.addEventListener('keydown', handleEscape, { capture: true });
  document.addEventListener('keydown', handleEscape, { capture: true });
  document.body.addEventListener('keydown', handleEscape, { capture: true });

  input.addEventListener('keydown', (event) => {
    if (handleEscape(event)) return;
    if (event.key === 'Backspace' || event.key === 'Delete') {
      const hit = chipAroundCursor(event.key === 'Backspace' ? 'backward' : 'forward');
      if (hit) {
        event.preventDefault();
        input.value = `${input.value.slice(0, hit.start)}${input.value.slice(hit.end)}`;
        input.setSelectionRange(hit.start, hit.start);
        updateUi();
        return;
      }
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

  window.addEventListener('blur', () => {
    window.setTimeout(() => {
      if (document.visibilityState === 'visible' && document.activeElement !== input) {
        focusInput();
      }
    }, 80);
  });
  window.addEventListener('focus', focusInput);
  window.addEventListener('pageshow', focusInput);
  document.addEventListener('visibilitychange', () => {
    if (document.visibilityState === 'visible') focusInput();
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
  input.addEventListener('paste', (event) => {
    const text = event.clipboardData?.getData('text/plain') || '';
    if (!text.trim()) return;
    event.preventDefault();
    clipboardSeq += 1;
    const tag = `[clipboard${clipboardSeq}]`;
    clipboardPastes.set(tag, text);
    insertTextAtCursor(`${tag} `);
    selectedSuggestion = 0;
    updateUi();
  });
  input.addEventListener('click', updateUi);
  input.addEventListener('keyup', updateUi);

  void listen('prompt-shown', () => {
    focusInput();
    void loadPromptContext();
  });
  void listen<PopupContextChip[]>('prompt-context-chips', (event) => {
    applySelectedContextChips(event.payload || []);
    focusInput();
  });

  void api.snapshot().then((snapshot) => {
    maxChars = snapshot.config.max_prompt_chars || maxChars;
    screenshotHotkey = snapshot.config.screenshot_hotkey || screenshotHotkey;
    updateUi();
  }).catch(updateUi);

  updateUi();
  focusInput();
}
