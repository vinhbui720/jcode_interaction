import { api, Snapshot } from './api';
import { renderSettingsForm } from './settings';

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]!));
}

function clientStatus(snapshot: Snapshot) {
  if (!snapshot.jcode_available) return { label: 'missing', tone: 'error', detail: 'Jcode CLI is not available on PATH.' };
  const status = snapshot.header_status || snapshot.state.process_status || 'idle';
  const active = snapshot.state.live_activity;
  const detail = active?.active ? `${active.label}: ${active.state}` : 'Ready. Use the prompt hotkey to talk to Jcode.';
  return { label: status, tone: status.replace(/[^a-z0-9_-]/gi, '').toLowerCase() || 'idle', detail };
}

function tokenLine(snapshot: Snapshot) {
  const stats = snapshot.state.token_stats;
  if (!stats) return 'No token usage yet';
  return `↑ ${stats.upload} · ↓ ${stats.download} · cache ${stats.cache_read}/${stats.cache_write}`;
}

function conversationBubbles(snapshot: Snapshot) {
  const messages = snapshot.state.recent_messages.slice(-4);
  if (!messages.length) {
    return `
      <div class="chat-empty">
        <strong>Ready when you are.</strong>
        <span>Open the prompt and talk to Jcode. Replies and status will appear here.</span>
      </div>`;
  }
  return messages.map((message) => {
    const mine = message.author.toLowerCase() === 'you';
    const author = mine ? 'You' : 'Jcode';
    return `
      <article class="chat-bubble ${mine ? 'from-user' : 'from-agent'}">
        <span>${escapeHtml(author)}</span>
        <p>${escapeHtml(message.text).slice(0, 520)}</p>
      </article>`;
  }).join('');
}

type DropdownTab = 'status' | 'settings';

export async function renderDropdown(root: HTMLElement, activeTab: DropdownTab = 'status') {
  const snapshot = await api.snapshot();
  const status = clientStatus(snapshot);
  const activeSession = snapshot.state.active_session ?? '';
  const section = snapshot.state.active_section || 'Previous section';
  const isSettings = activeTab === 'settings';
  root.innerHTML = `
    <main class="panel-shell panel-shell-modern">
      <nav class="dropdown-tabs" aria-label="Dropdown actions">
        <button id="statusTab" class="dropdown-tab ${!isSettings ? 'active' : ''}" type="button" aria-selected="${!isSettings}">Status</button>
        <button id="settingsTab" class="dropdown-tab ${isSettings ? 'active' : ''}" type="button" aria-selected="${isSettings}">Settings</button>
      </nav>

      <header class="dropdown-hero compact">
        <div class="prompt-logo dropdown-logo">JC</div>
        <div class="dropdown-title">
          <p class="eyebrow">Talk with Jcode</p>
          <h1>${escapeHtml(section)}</h1>
        </div>
        <strong class="status-pill status-${escapeHtml(status.tone)}">${escapeHtml(status.label)}</strong>
      </header>

      <div id="dropdownPanel"></div>
    </main>`;

  root.querySelector<HTMLButtonElement>('#statusTab')!.onclick = () => renderDropdown(root, 'status');
  root.querySelector<HTMLButtonElement>('#settingsTab')!.onclick = () => renderDropdown(root, 'settings');

  const panel = root.querySelector<HTMLElement>('#dropdownPanel')!;
  if (isSettings) {
    await renderSettingsForm(panel, { embedded: true });
    return;
  }

  panel.innerHTML = `
    <section class="chat-panel" aria-label="Conversation preview">
      <div class="chat-panel-topline">
        <div>
          <p class="eyebrow">Conversation</p>
          <h2>${escapeHtml(status.detail)}</h2>
        </div>
        <span class="status-pill status-${escapeHtml(status.tone)}">${escapeHtml(status.label)}</span>
      </div>
      <div class="chat-thread">
        ${conversationBubbles(snapshot)}
      </div>
    </section>

    <section class="client-card client-card-soft">
      <div class="client-card-topline">
        <span>Client status</span>
        <strong>${snapshot.jcode_available ? 'Ready' : 'Needs setup'}</strong>
      </div>
      <div class="client-meta">
        <span>${escapeHtml(tokenLine(snapshot))}</span>
        <span>Session: ${escapeHtml(activeSession || 'new')}</span>
      </div>
    </section>

    <section class="session-tools session-tools-modern chat-session-tools" aria-label="Session controls">
      <input id="sessionId" placeholder="Session id to resume" value="${escapeHtml(activeSession)}" />
      <button id="resume" class="secondary-action">Resume</button>
      <button id="newSection" class="primary-action">New</button>
    </section>

    <section class="status-actions status-actions-single" aria-label="Status actions">
      <button id="terminal" class="status-action-card" type="button"><span>Open</span><strong>Terminal</strong></button>
    </section>`;

  panel.querySelector<HTMLButtonElement>('#terminal')!.onclick = () => api.launchTerminal();
  panel.querySelector<HTMLButtonElement>('#resume')!.onclick = async () => {
    const session = panel.querySelector<HTMLInputElement>('#sessionId')!.value.trim();
    if (!session) return;
    await api.switchSession(session, session);
    renderDropdown(root);
  };
  panel.querySelector<HTMLButtonElement>('#newSection')!.onclick = async () => {
    await api.startNewSection(`jcode-panel ${new Date().toLocaleString()}`);
    renderDropdown(root);
  };
}
