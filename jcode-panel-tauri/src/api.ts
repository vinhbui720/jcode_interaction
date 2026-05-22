import { invoke } from '@tauri-apps/api/core';

export type Snapshot = {
  config: { prompt_hotkey: string; screenshot_hotkey: string; terminal: string; send_context_default: boolean; max_prompt_chars: number };
  state: { active_session?: string | null; active_section: string; last_prompt: string; token_stats?: { upload: number; download: number; cache_read: number; cache_write: number } | null; recent_messages: { author: string; text: string }[]; prompt_history: string[]; last_context_summary: string; browser_bridge_seen: boolean };
  jcode_available: boolean;
};

export const api = {
  snapshot: () => invoke<Snapshot>('snapshot'),
  submitPrompt: (prompt: string) => invoke('submit_prompt', { prompt }),
  switchSession: (session: string, name?: string) => invoke('switch_session', { session, name }),
  startNewSection: (name?: string) => invoke('start_new_section', { name }),
  saveSettings: (newConfig: Snapshot['config']) => invoke('save_settings', { newConfig }),
  refreshIntegrations: () => invoke('refresh_integrations'),
  integrationStatus: () => invoke('integration_status'),
  showSettings: () => invoke('show_settings'),
  hidePrompt: () => invoke('hide_prompt'),
};
