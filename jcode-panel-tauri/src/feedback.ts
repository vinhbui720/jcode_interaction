import { listen } from '@tauri-apps/api/event';
import { api, type FeedbackPayload } from './api';

type TokenStats = { upload: number; download: number; cache_read: number; cache_write: number };

function escapeHtml(value: string) {
  return value.replace(/[&<>"]/g, (ch) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[ch]!));
}

function inlineMarkdown(value: string) {
  return escapeHtml(value)
    .replace(/`([^`]+)`/g, '<code>$1</code>')
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/\*([^*]+)\*/g, '<em>$1</em>');
}

function renderMarkdown(value: string) {
  const limited = limitFeedback(value || '');
  const lines = limited.trim().split('\n');
  return lines.map((line) => {
    if (!line.trim()) return '<br />';
    if (line.trim().startsWith('#')) return `<strong class="toast-heading">${escapeHtml(line.replace(/^#+/, '').trim())}</strong>`;
    const bullet = line.match(/^\s*(?:[-*•]|\d+\.)\s+(.*)$/);
    if (bullet) return `<div><span class="toast-bullet">●</span> ${inlineMarkdown(bullet[1])}</div>`;
    return `<div>${inlineMarkdown(line)}</div>`;
  }).join('');
}

function limitFeedback(value: string) {
  const limit = 20_000;
  if (value.length <= limit) return value;
  return `... earlier feedback omitted ...\n${value.slice(value.length - limit)}`;
}

function compact(value: number) {
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(1).replace(/\.0$/, '')}m`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(1).replace(/\.0$/, '')}k`;
  return String(value);
}

function renderStats(stats?: TokenStats | null) {
  if (!stats) return '';
  return `<span>⬆ ${compact(stats.upload)}</span><span>⬇ ${compact(stats.download)}</span><span>◌ ${compact(stats.cache_read)}</span><span>✎ ${compact(stats.cache_write)}</span>`;
}

export function renderFeedback(root: HTMLElement) {
  root.innerHTML = `
    <main class="toast-shell">
      <section class="toast-card">
        <div class="toast-topline">
          <div class="toast-title">jcode feedback</div>
          <div id="toast-stats" class="toast-stats"></div>
        </div>
        <div class="toast-scroll"><div id="toast-text" class="toast-text" tabindex="0">Waiting for feedback...</div></div>
        <div class="toast-statusbar">
          <span id="toast-status" class="toast-status">idle</span>
          <span id="toast-notice" class="toast-notice"></span>
        </div>
        <div class="toast-actions">
          <button id="toast-jump" class="toast-jump" title="Back to latest feedback" hidden>Back to latest feedback</button>
          <button id="toast-open" title="Open conversation">Open</button>
          <button id="toast-reply" title="Reply">Reply</button>
          <button id="toast-close" title="Dismiss">Dismiss</button>
        </div>
      </section>
    </main>`;

  const textEl = root.querySelector<HTMLDivElement>('#toast-text')!;
  const scrollEl = root.querySelector<HTMLDivElement>('.toast-scroll')!;
  const noticeEl = root.querySelector<HTMLDivElement>('#toast-notice')!;
  const statusEl = root.querySelector<HTMLSpanElement>('#toast-status')!;
  const statsEl = root.querySelector<HTMLDivElement>('#toast-stats')!;
  const jumpButton = root.querySelector<HTMLButtonElement>('#toast-jump')!;
  let hideTimer: number | undefined;
  let userPinnedScroll = false;

  const isAtBottom = () => scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight < 8;
  const scrollLatest = () => {
    scrollEl.scrollTop = scrollEl.scrollHeight;
    userPinnedScroll = false;
    jumpButton.hidden = true;
  };

  const scheduleHide = () => {
    if (hideTimer) window.clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => api.hideFeedback(), 60_000);
  };

  const apply = (payload: FeedbackPayload, resetHideTimer = true) => {
    const shouldStickToLatest = !userPinnedScroll && isAtBottom();
    const oldScrollTop = scrollEl.scrollTop;
    textEl.innerHTML = renderMarkdown(payload.text || 'No feedback text.');
    noticeEl.textContent = payload.notice || '';
    noticeEl.hidden = !payload.notice;
    statusEl.textContent = payload.status || 'idle';
    statusEl.className = `toast-status toast-status-${(payload.status || 'idle').replace(/[^a-z0-9_-]/gi, '').toLowerCase()}`;
    statsEl.innerHTML = renderStats(payload.stats);
    statsEl.hidden = !payload.stats;
    if (shouldStickToLatest) {
      window.requestAnimationFrame(scrollLatest);
    } else {
      scrollEl.scrollTop = oldScrollTop;
      jumpButton.hidden = isAtBottom();
    }
    if (resetHideTimer) scheduleHide();
  };

  jumpButton.onclick = scrollLatest;
  scrollEl.addEventListener('scroll', () => {
    userPinnedScroll = !isAtBottom();
    jumpButton.hidden = !userPinnedScroll;
  });
  root.querySelector<HTMLButtonElement>('#toast-close')!.onclick = () => api.hideFeedback();
  root.querySelector<HTMLButtonElement>('#toast-reply')!.onclick = () => api.hideFeedback().finally(() => api.showPrompt());
  root.querySelector<HTMLButtonElement>('#toast-open')!.onclick = () => api.hideFeedback().finally(() => api.showDropdown());

  root.addEventListener('mouseenter', () => { if (hideTimer) window.clearTimeout(hideTimer); });
  root.addEventListener('mouseleave', scheduleHide);
  window.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') void api.hideFeedback();
  });

  void listen<FeedbackPayload>('feedback-update', (event) => apply(event.payload));
  const replay = () => {
    void api.currentFeedback()
      .then((payload) => {
        if (payload) apply(payload, false);
        return api.snapshot();
      })
      .then((snapshot) => {
        statusEl.textContent = snapshot.header_status || snapshot.state.process_status || 'idle';
        statusEl.className = `toast-status toast-status-${(snapshot.state.process_status || 'idle').replace(/[^a-z0-9_-]/gi, '').toLowerCase()}`;
        if (!statsEl.innerHTML && snapshot.state.token_stats) {
          statsEl.innerHTML = renderStats(snapshot.state.token_stats);
          statsEl.hidden = false;
        }
      })
      .catch(() => {});
  };
  replay();
  window.setInterval(replay, 1_000);
}
