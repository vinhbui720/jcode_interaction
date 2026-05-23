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
    <main class="toast-shell toast-shell-notification">
      <section class="toast-card toast-card-notification">
        <div class="toast-grabber"></div>
        <div class="toast-topline">
          <div class="toast-title-wrap">
            <span class="toast-agent-dot">JC</span>
            <div>
              <div class="toast-title">Jcode says</div>
              <div id="toast-notice" class="toast-notice"></div>
            </div>
          </div>
          <div id="toast-stats" class="toast-stats"></div>
        </div>
        <div class="toast-scroll">
          <div id="toast-text" class="toast-text" tabindex="0">Waiting for feedback...</div>
          <button id="toast-jump" class="toast-jump" title="Back to latest feedback" hidden>↓</button>
        </div>
        <div class="toast-statusbar">
          <span id="toast-status" class="toast-status">idle</span>
          <span class="toast-live-hint">Esc dismisses · Reply opens prompt</span>
        </div>
        <div class="toast-actions">
          <button id="toast-sound" title="Speak feedback" aria-pressed="false">Sound off</button>
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
  const soundButton = root.querySelector<HTMLButtonElement>('#toast-sound')!;
  let hideTimer: number | undefined;
  let speakTimer: number | undefined;
  let hovering = false;
  let focused = false;
  let ttsFinished = false;
  let soundEnabled = false;
  let soundLoaded = false;
  let lastSpokenText = '';
  let pendingSpokenText = '';
  let userPinnedScroll = false;

  const isAtBottom = () => scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight < 8;
  const scrollLatest = () => {
    scrollEl.scrollTop = scrollEl.scrollHeight;
    userPinnedScroll = false;
    jumpButton.hidden = true;
  };

  const interacting = () => hovering || focused;

  const scheduleHide = (delayMs = ttsFinished ? 2_000 : 5_000) => {
    if (hideTimer) window.clearTimeout(hideTimer);
    if (interacting()) return;
    hideTimer = window.setTimeout(() => api.hideFeedback(), delayMs);
  };

  const updateSoundButton = () => {
    soundButton.textContent = soundEnabled ? 'Sound on' : 'Sound off';
    soundButton.classList.toggle('active', soundEnabled);
    soundButton.setAttribute('aria-pressed', String(soundEnabled));
  };

  const scheduleSpeak = (text: string, status: string) => {
    if (!soundLoaded) {
      void api.snapshot().then((snapshot) => {
        soundLoaded = true;
        soundEnabled = !!snapshot.config.tts_enabled;
        updateSoundButton();
        scheduleSpeak(text, status);
      }).catch(() => { soundLoaded = true; });
      return;
    }
    if (!soundEnabled) return;
    const trimmed = text.trim();
    const normalizedStatus = status.toLowerCase();
    const streamingStatus = ['sending', 'answering', 'working'].some((part) => normalizedStatus.includes(part));
    const stableStatus = !streamingStatus;
    if (!stableStatus) return;
    if (!trimmed || trimmed === lastSpokenText || trimmed === pendingSpokenText) return;
    pendingSpokenText = trimmed;
    if (speakTimer) window.clearTimeout(speakTimer);
    speakTimer = window.setTimeout(() => {
      lastSpokenText = trimmed;
      pendingSpokenText = '';
      void api.speakFeedbackText(trimmed).catch(() => {});
    }, 650);
  };

  const apply = (payload: FeedbackPayload, resetHideTimer = true, allowSpeak = false) => {
    if (resetHideTimer) ttsFinished = false;
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
    if (allowSpeak) scheduleSpeak(payload.text || '', payload.status || '');
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
  soundButton.onclick = async () => {
    soundEnabled = !soundEnabled;
    updateSoundButton();
    try {
      const cfg = await api.setTtsEnabled(soundEnabled);
      soundEnabled = cfg.tts_enabled;
      updateSoundButton();
      if (soundEnabled) void api.currentFeedback().then((payload) => payload && scheduleSpeak(payload.text, payload.status || ''));
    } catch {
      soundEnabled = !soundEnabled;
      updateSoundButton();
    }
  };

  root.addEventListener('mouseenter', () => {
    hovering = true;
    if (hideTimer) window.clearTimeout(hideTimer);
  });
  root.addEventListener('focusin', () => {
    focused = true;
    if (hideTimer) window.clearTimeout(hideTimer);
  });
  root.addEventListener('focusout', () => {
    window.setTimeout(() => {
      focused = root.contains(document.activeElement);
      scheduleHide();
    }, 0);
  });
  root.addEventListener('mouseleave', () => {
    hovering = false;
    scheduleHide();
  });
  window.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') void api.hideFeedback();
  });

  void listen<FeedbackPayload>('feedback-update', (event) => apply(event.payload, true, false));
  void listen('feedback-tts-finished', () => {
    ttsFinished = true;
    scheduleHide(2_000);
  });
  const replay = () => {
    void api.currentFeedback()
      .then((payload) => {
        if (payload) apply(payload, false, false);
        return api.snapshot();
      })
      .then((snapshot) => {
        statusEl.textContent = snapshot.header_status || snapshot.state.process_status || 'idle';
        statusEl.className = `toast-status toast-status-${(snapshot.state.process_status || 'idle').replace(/[^a-z0-9_-]/gi, '').toLowerCase()}`;
        if (!statsEl.innerHTML && snapshot.state.token_stats) {
          statsEl.innerHTML = renderStats(snapshot.state.token_stats);
          statsEl.hidden = false;
        }
        soundEnabled = !!snapshot.config.tts_enabled;
        soundLoaded = true;
        updateSoundButton();
      })
      .catch(() => {});
  };
  replay();
  const replayTimer = window.setInterval(replay, 1_000);
  window.addEventListener('unload', () => window.clearInterval(replayTimer));
}
