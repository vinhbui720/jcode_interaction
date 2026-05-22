import { existsSync } from 'node:fs';
const required = [
  'src-tauri/src/core/config.rs',
  'src-tauri/src/core/state.rs',
  'src-tauri/src/core/jcode.rs',
  'src-tauri/src/integrations/vscode.rs',
  'src-tauri/src/integrations/obsidian.rs',
  'src-tauri/src/ui/windows.rs',
  'src/prompt.ts',
  'src/dropdown.ts',
  'src/settings.ts'
];
const missing = required.filter((path) => !existsSync(path));
if (missing.length) {
  console.error('Missing scaffold files:\n' + missing.join('\n'));
  process.exit(1);
}
console.log('Tauri scaffold structure OK');
