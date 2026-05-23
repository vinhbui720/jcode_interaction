import { invoke } from '@tauri-apps/api/core';

export type Snapshot = {
  config: { prompt_hotkey: string; screenshot_hotkey: string; terminal: string; send_context_default: boolean; max_prompt_chars: number; tts_enabled: boolean; tts_api_url: string; tts_command: string };
  state: { active_session?: string | null; active_section: string; last_prompt: string; token_stats?: { upload: number; download: number; cache_read: number; cache_write: number } | null; recent_messages: { author: string; text: string }[]; prompt_history: string[]; last_context_summary: string; browser_bridge_seen: boolean; process_status: string; live_activity?: { label: string; state: string; started_at_ms: number; active: boolean } | null };
  jcode_available: boolean;
  conversation_preview: string;
  header_status: string;
};

export type DiagnosticsReport = { checks: { name: string; ok: boolean; message: string; fix: string }[] };
export type PopupContextChip = { tag: string; body: string; kind: string };
export type ActiveContext = { app: string; window_title: string; selected_text: string; clipboard_text: string; browser?: { title: string; url: string; selected_text: string } | null };
export type FeedbackPayload = { text: string; notice?: string; status?: string; stats?: { upload: number; download: number; cache_read: number; cache_write: number } | null };

export const api = {
  snapshot: () => invoke<Snapshot>('snapshot'),
  submitPrompt: (prompt: string) => invoke<{ ok: boolean; output: string; token_stats?: { upload: number; download: number; cache_read: number; cache_write: number } | null }>('submit_prompt', { prompt }),
  submitPromptAsync: (prompt: string) => invoke<void>('submit_prompt_async', { prompt }),
  normalizePromptText: (text: string) => invoke<{ text: string; hints: string[] }>('normalize_prompt_text', { text }),
  captureScreenshot: (mode: string) => invoke<string>('capture_screenshot', { mode }),
  imageDataUrl: (path: string) => invoke<string>('image_data_url', { path }),
  activeContextSnapshot: () => invoke<ActiveContext>('active_context_snapshot'),
  popupContextChips: () => invoke<PopupContextChip[]>('popup_context_chips'),
  switchSession: (session: string, name?: string) => invoke('switch_session', { session, name }),
  startNewSection: (name?: string) => invoke('start_new_section', { name }),
  saveSettings: (newConfig: Snapshot['config']) => invoke('save_settings', { newConfig }),
  setTtsEnabled: (enabled: boolean) => invoke<Snapshot['config']>('set_tts_enabled', { enabled }),
  speakFeedbackText: (text: string) => invoke('speak_feedback_text', { text }),
  suspendPromptHotkey: () => invoke('suspend_prompt_hotkey'),
  resumePromptHotkey: () => invoke('resume_prompt_hotkey'),
  quitApp: () => invoke('quit_app'),
  availableTerminals: () => invoke<string[]>('available_terminals'),
  refreshIntegrations: () => invoke('refresh_integrations'),
  integrationStatus: () => invoke('integration_status'),
  diagnosticsReport: () => invoke<DiagnosticsReport>('diagnostics_report'),
  launchTerminal: (command?: string) => invoke('launch_terminal', { command }),
  showPrompt: () => invoke('show_prompt'),
  showDropdown: () => invoke('show_dropdown'),
  showSettings: () => invoke('show_settings'),
  promptFollowMouseTick: () => invoke<boolean>('prompt_follow_mouse_tick'),
  showFeedback: (text: string, notice?: string, stats?: { upload: number; download: number; cache_read: number; cache_write: number } | null) => invoke('show_feedback', { text, notice, stats }),
  currentFeedback: () => invoke<FeedbackPayload | null>('current_feedback'),
  hideFeedback: () => invoke('hide_feedback'),
  hidePrompt: () => invoke('hide_prompt'),
};
