import { api, Snapshot } from './api';

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]!));
}

function clientStatus(snapshot: Snapshot) {
  if (!snapshot.jcode_available) return { label: 'missing', tone: 'error', detail: 'Jcode CLI is not available on PATH.' };
  const status = snapshot.header_status || snapshot.state.process_status || 'idle';
  const active = snapshot.state.live_activity;
  const detail = active?.active ? `${active.label}: ${active.state}` : 'Client is ready for the prompt hotkey.';
  return { label: status, tone: status.replace(/[^a-z0-9_-]/gi, '').toLowerCase() || 'idle', detail };
}

function tokenLine(snapshot: Snapshot) {
  const stats = snapshot.state.token_stats;
  if (!stats) return 'No token usage yet';
  return `↑ ${stats.upload} · ↓ ${stats.download} · cache ${stats.cache_read}/${stats.cache_write}`;
}

export async function renderDropdown(root: HTMLElement) {
  const snapshot = await api.snapshot();
  const status = clientStatus(snapshot);
  const activeSession = snapshot.state.active_session ?? '';
  const section = snapshot.state.active_section || 'Previous section';
  root.innerHTML = `
    <main class="panel-shell panel-shell-modern">
      <header class="dropdown-hero">
        <div class="prompt-logo dropdown-logo">JC</div>
        <div class="dropdown-title">
          <p class="eyebrow">Jcode client</p>
          <h1>${escapeHtml(section)}</h1>
        </div>
        <strong class="status-pill status-${escapeHtml(status.tone)}">${escapeHtml(status.label)}</strong>
      </header>

      <section class="client-card">
        <div class="client-card-topline">
          <span>Current status</span>
          <strong>${snapshot.jcode_available ? 'Ready' : 'Needs setup'}</strong>
        </div>
        <p class="client-detail">${escapeHtml(status.detail)}</p>
        <div class="client-meta">
          <span>${escapeHtml(tokenLine(snapshot))}</span>
          <span>Session: ${escapeHtml(activeSession || 'new')}</span>
        </div>
      </section>

      <section class="session-tools session-tools-modern" aria-label="Session controls">
        <input id="sessionId" placeholder="Session id to resume" value="${escapeHtml(activeSession)}" />
        <button id="resume" class="secondary-action">Resume</button>
        <button id="newSection" class="primary-action">New</button>
      </section>

      <section class="quick-actions" aria-label="Quick actions">
        <button id="settings">Settings</button>
        <button id="terminal">Open terminal</button>
        <button id="refresh">Refresh</button>
      </section>
    </main>`;

  root.querySelector<HTMLButtonElement>('#refresh')!.onclick = async () => { await api.refreshIntegrations(); renderDropdown(root); };
  root.querySelector<HTMLButtonElement>('#settings')!.onclick = () => api.showSettings();
  root.querySelector<HTMLButtonElement>('#terminal')!.onclick = () => api.launchTerminal();
  root.querySelector<HTMLButtonElement>('#resume')!.onclick = async () => {
    const session = root.querySelector<HTMLInputElement>('#sessionId')!.value.trim();
    if (!session) return;
    await api.switchSession(session, session);
    renderDropdown(root);
  };
  root.querySelector<HTMLButtonElement>('#newSection')!.onclick = async () => {
    await api.startNewSection(`jcode-panel ${new Date().toLocaleString()}`);
    renderDropdown(root);
  };
}
