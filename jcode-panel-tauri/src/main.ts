import { renderDropdown } from './dropdown';
import { renderFeedback } from './feedback';
import { renderPrompt } from './prompt';
import { renderSettings } from './settings';
import './styles.css';

const params = new URLSearchParams(location.search);
const kind = params.get('window') ?? 'dropdown';
const root = document.querySelector<HTMLDivElement>('#app');
if (!root) throw new Error('missing app root');

if (kind === 'prompt') renderPrompt(root);
else if (kind === 'settings') renderSettings(root);
else if (kind === 'feedback') renderFeedback(root);
else renderDropdown(root);
