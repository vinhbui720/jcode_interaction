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
  const lines = (value || '').trim().split('\n');
  return lines.map((line) => {
    if (!line.trim()) return '<br />';
    if (line.trim().startsWith('#')) return `<strong class="toast-heading">${escapeHtml(line.replace(/^#+/, '').trim())}</strong>`;
    const bullet = line.match(/^\s*(?:[-*•]|\d+\.)\s+(.*)$/);
    if (bullet) return `<div><span class="toast-bullet">●</span> ${inlineMarkdown(bullet[1])}</div>`;
    return `<div>${inlineMarkdown(line)}</div>`;
  }).join('');
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
        <div id="toast-text" class="toast-text">Waiting for feedback...</div>
        <div id="toast-notice" class="toast-notice"></div>
        <div class="toast-actions">
          <button id="toast-open" title="Open conversation">Open</button>
          <button id="toast-reply" title="Reply">Reply</button>
          <button id="toast-close" title="Dismiss">Dismiss</button>
        </div>
      </section>
    </main>`;

  const textEl = root.querySelector<HTMLDivElement>('#toast-text')!;
  const noticeEl = root.querySelector<HTMLDivElement>('#toast-notice')!;
  const statsEl = root.querySelector<HTMLDivElement>('#toast-stats')!;
  let hideTimer: number | undefined;

  const scheduleHide = () => {
    if (hideTimer) window.clearTimeout(hideTimer);
    hideTimer = window.setTimeout(() => api.hideFeedback(), 60_000);
  };

  const apply = (payload: FeedbackPayload) => {
    textEl.innerHTML = renderMarkdown(payload.text || 'No feedback text.');
    noticeEl.textContent = payload.notice || '';
    noticeEl.hidden = !payload.notice;
    statsEl.innerHTML = renderStats(payload.stats);
    statsEl.hidden = !payload.stats;
    scheduleHide();
  };

  root.querySelector<HTMLButtonElement>('#toast-close')!.onclick = () => api.hideFeedback();
  root.querySelector<HTMLButtonElement>('#toast-reply')!.onclick = () => api.hideFeedback().finally(() => api.showPrompt());
  root.querySelector<HTMLButtonElement>('#toast-open')!.onclick = () => api.hideFeedback().finally(() => api.showDropdown());

  root.addEventListener('mouseenter', () => { if (hideTimer) window.clearTimeout(hideTimer); });
  root.addEventListener('mouseleave', scheduleHide);

  void listen<FeedbackPayload>('feedback-update', (event) => apply(event.payload));
  void api.currentFeedback().then((payload) => { if (payload) apply(payload); }).catch(() => {});
}
