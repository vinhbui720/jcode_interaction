import { api } from './api';

function escapeAttr(value: string) {
  return value.replace(/[&<>"]/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]!));
}

function keyName(event: KeyboardEvent) {
  const key = event.key;
  if (key === ' ') return 'Space';
  if (key.startsWith('Arrow')) return key.replace('Arrow', '');
  if (key.length === 1) return key.toUpperCase();
  return key;
}

function hotkeyFromEvent(event: KeyboardEvent) {
  const key = keyName(event);
  const modifierKeys = new Set(['Control', 'Shift', 'Alt', 'Meta', 'Super']);
  const parts: string[] = [];
  if (event.ctrlKey && key !== 'Control') parts.push('Ctrl');
  if (event.altKey && key !== 'Alt') parts.push('Alt');
  if (event.shiftKey && key !== 'Shift') parts.push('Shift');
  if (event.metaKey && key !== 'Meta' && key !== 'Super') parts.push('Super');
  if (!modifierKeys.has(key)) parts.push(key);
  return parts.join('+');
}

function mouseButtonName(button: number) {
  // DOM button 3/4 are the common browser Back/Forward side buttons,
  // matching X11 button map 8/9 on typical mice.
  if (button === 3) return 'Mouse8';
  if (button === 4) return 'Mouse9';
  return `Mouse${button + 1}`;
}

function hotkeyFromMouseEvent(event: MouseEvent) {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push('Ctrl');
  if (event.altKey) parts.push('Alt');
  if (event.shiftKey) parts.push('Shift');
  if (event.metaKey) parts.push('Super');
  parts.push(mouseButtonName(event.button));
  return parts.join('+');
}

function installHotkeyCapture(root: HTMLElement, inputId: string, buttonId: string) {
  const input = root.querySelector<HTMLInputElement>(`#${inputId}`)!;
  const button = root.querySelector<HTMLButtonElement>(`#${buttonId}`)!;
  let capturing = false;

  const stopCapture = () => {
    capturing = false;
    button.textContent = 'Record';
    button.classList.remove('recording');
  };

  button.onclick = () => {
    capturing = true;
    button.textContent = 'Press keys...';
    button.classList.add('recording');
    input.focus();
  };

  input.addEventListener('keydown', (event) => {
    if (!capturing) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === 'Escape') {
      stopCapture();
      return;
    }
    const hotkey = hotkeyFromEvent(event);
    if (!hotkey) return;
    input.value = hotkey;
    if (!['Control', 'Shift', 'Alt', 'Meta', 'Super'].includes(event.key)) stopCapture();
  });

  const captureMouse = (event: MouseEvent) => {
    if (!capturing) return;
    event.preventDefault();
    event.stopPropagation();
    input.value = hotkeyFromMouseEvent(event);
    stopCapture();
  };
  input.addEventListener('mousedown', captureMouse);
  button.addEventListener('mousedown', captureMouse);
  input.addEventListener('auxclick', captureMouse);
  button.addEventListener('auxclick', captureMouse);
  input.addEventListener('contextmenu', (event) => { if (capturing) event.preventDefault(); });
  button.addEventListener('contextmenu', (event) => { if (capturing) event.preventDefault(); });

  input.addEventListener('blur', () => {
    if (capturing) window.setTimeout(stopCapture, 150);
  });
}

export async function renderSettings(root: HTMLElement) {
  const snapshot = await api.snapshot();
  const cfg = snapshot.config;
  const terminals = await api.availableTerminals().catch(() => []);
  const terminalChoices = Array.from(new Set([cfg.terminal, ...terminals].filter(Boolean)));
  root.innerHTML = `
    <main class="settings-shell">
      <header><h1>Settings</h1><p>Saved to disk and restored on next start.</p></header>
      <label>Prompt hotkey
        <div class="settings-inline"><input id="promptHotkey" readonly value="${escapeAttr(cfg.prompt_hotkey)}" /><button id="recordPrompt" type="button">Record</button></div>
      </label>
      <label>Screenshot hotkey
        <div class="settings-inline"><input id="screenshotHotkey" readonly value="${escapeAttr(cfg.screenshot_hotkey)}" /><button id="recordScreenshot" type="button">Record</button></div>
      </label>
      <label>Terminal
        <div class="settings-inline">
          <select id="terminalSelect">
            ${terminalChoices.map((terminal) => `<option value="${escapeAttr(terminal)}" ${terminal === cfg.terminal ? 'selected' : ''}>${escapeAttr(terminal)}</option>`).join('')}
            <option value="__custom__">Custom command...</option>
          </select>
          <button id="refreshTerminals" type="button">Refresh</button>
        </div>
        <input id="terminal" value="${escapeAttr(cfg.terminal)}" placeholder="terminal command or template with {quoted_cmd}" />
      </label>
      <label>Max prompt chars <input id="maxPromptChars" type="number" min="100" value="${cfg.max_prompt_chars}" /></label>
      <label class="row"><input id="sendContext" type="checkbox" ${cfg.send_context_default ? 'checked' : ''} /> Send context by default</label>
      <button id="save">Save settings</button>
      <pre id="status"></pre>
    </main>`;

  installHotkeyCapture(root, 'promptHotkey', 'recordPrompt');
  installHotkeyCapture(root, 'screenshotHotkey', 'recordScreenshot');

  const terminalSelect = root.querySelector<HTMLSelectElement>('#terminalSelect')!;
  const terminalInput = root.querySelector<HTMLInputElement>('#terminal')!;
  terminalSelect.onchange = () => {
    if (terminalSelect.value !== '__custom__') terminalInput.value = terminalSelect.value;
    terminalInput.hidden = terminalSelect.value !== '__custom__';
  };
  terminalInput.hidden = terminalChoices.includes(cfg.terminal);

  root.querySelector<HTMLButtonElement>('#refreshTerminals')!.onclick = async () => {
    const latest = await api.availableTerminals();
    const current = terminalInput.value;
    const choices = Array.from(new Set([current, ...latest].filter(Boolean)));
    terminalSelect.innerHTML = choices.map((terminal) => `<option value="${escapeAttr(terminal)}" ${terminal === current ? 'selected' : ''}>${escapeAttr(terminal)}</option>`).join('') + '<option value="__custom__">Custom command...</option>';
  };

  root.querySelector<HTMLButtonElement>('#save')!.onclick = async () => {
    const terminal = terminalSelect.value === '__custom__' ? terminalInput.value : terminalSelect.value;
    const newConfig = {
      prompt_hotkey: root.querySelector<HTMLInputElement>('#promptHotkey')!.value,
      screenshot_hotkey: root.querySelector<HTMLInputElement>('#screenshotHotkey')!.value,
      terminal,
      max_prompt_chars: Number(root.querySelector<HTMLInputElement>('#maxPromptChars')!.value || 4000),
      send_context_default: root.querySelector<HTMLInputElement>('#sendContext')!.checked,
    };
    await api.saveSettings(newConfig);
    root.querySelector<HTMLPreElement>('#status')!.textContent = `Saved. Prompt hotkey is now ${newConfig.prompt_hotkey}.`;
  };
}
