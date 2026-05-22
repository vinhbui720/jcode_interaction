import { api, Snapshot } from './api';

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[ch]!));
}

function tokenBadge(snapshot: Snapshot) {
  const stats = snapshot.state.token_stats;
  if (!stats) return '<span class="badge muted">no token data</span>';
  return `<span class="badge">⬆ ${stats.upload} · ⬇ ${stats.download} · cache ${stats.cache_read}/${stats.cache_write}</span>`;
}

export async function renderDropdown(root: HTMLElement) {
  const snapshot = await api.snapshot();
  const diagnostics = await api.diagnosticsReport();
  const messages = snapshot.state.recent_messages.slice(-8).map((m) => `
    <article class="message"><strong>${m.author}</strong><p>${m.text}</p></article>`).join('') || '<p class="muted">No messages yet.</p>';
  const history = snapshot.state.prompt_history.slice(-6).reverse().map((item) => `<li>${item}</li>`).join('') || '<li class="muted">No prompt history yet.</li>';
  const checks = diagnostics.checks.map((check) => `<li class="${check.ok ? 'ok' : 'fail'}"><strong>${check.name}</strong>: ${check.message}</li>`).join('');
  const status = snapshot.state.process_status || 'idle';
  const statusClass = status.replace(/[^a-z0-9_-]/gi, '').toLowerCase() || 'idle';
  root.innerHTML = `
    <main class="panel-shell">
      <header class="panel-header">
        <div><h1>Jcode Interaction</h1><p>${snapshot.state.active_section}</p></div>
        ${tokenBadge(snapshot)}
      </header>
      <section class="status-grid">
        <div><span>status</span><strong class="status-pill status-${statusClass}">${escapeHtml(status)}</strong></div>
        <div><span>jcode</span><strong>${snapshot.jcode_available ? 'ready' : 'missing'}</strong></div>
        <div><span>session</span><strong>${escapeHtml(snapshot.state.active_session ?? 'new')}</strong></div>
        <div><span>latest</span><strong>${escapeHtml(snapshot.conversation_preview)}</strong></div>
      </section>
      <section class="session-tools">
        <input id="sessionId" placeholder="Resume session id" value="${snapshot.state.active_session ?? ''}" />
        <button id="resume">Resume</button>
        <button id="newSection">New section</button>
      </section>
      <section class="conversation">${messages}</section>
      <section class="diagnostics"><h2>Diagnostics</h2><ul>${checks}</ul></section>
      <section class="history"><h2>Prompt history</h2><ul>${history}</ul></section>
      <footer>
        <button id="refresh">Refresh integrations</button>
        <button id="terminal">Open terminal</button>
        <button id="settings">Settings</button>
      </footer>
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
