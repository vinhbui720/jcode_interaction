import { api, Snapshot } from './api';

function tokenBadge(snapshot: Snapshot) {
  const stats = snapshot.state.token_stats;
  if (!stats) return '<span class="badge muted">no token data</span>';
  return `<span class="badge">⬆ ${stats.upload} · ⬇ ${stats.download} · cache ${stats.cache_read}/${stats.cache_write}</span>`;
}

export async function renderDropdown(root: HTMLElement) {
  const snapshot = await api.snapshot();
  const messages = snapshot.state.recent_messages.slice(-8).map((m) => `
    <article class="message"><strong>${m.author}</strong><p>${m.text}</p></article>`).join('') || '<p class="muted">No messages yet.</p>';
  root.innerHTML = `
    <main class="panel-shell">
      <header class="panel-header">
        <div><h1>Jcode Interaction</h1><p>${snapshot.state.active_section}</p></div>
        ${tokenBadge(snapshot)}
      </header>
      <section class="status-grid">
        <div><span>jcode</span><strong>${snapshot.jcode_available ? 'ready' : 'missing'}</strong></div>
        <div><span>session</span><strong>${snapshot.state.active_session ?? 'new'}</strong></div>
      </section>
      <section class="conversation">${messages}</section>
      <footer>
        <button id="refresh">Refresh integrations</button>
        <button id="settings">Settings</button>
      </footer>
    </main>`;
  root.querySelector<HTMLButtonElement>('#refresh')!.onclick = async () => { await api.refreshIntegrations(); renderDropdown(root); };
  root.querySelector<HTMLButtonElement>('#settings')!.onclick = () => api.showSettings();
}
