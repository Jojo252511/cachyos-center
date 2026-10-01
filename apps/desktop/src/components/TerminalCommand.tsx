import { Terminal } from 'lucide-react';

import { useI18n } from '../i18n';
import { CopyButton } from './CopyButton';

/** Shows an exact, safe terminal command with a copy button. */
export function TerminalCommand({ command, hint }: { command: string; hint?: string }) {
  const { t } = useI18n();
  return (
    <div className="terminal-command">
      {hint ? <p className="terminal-command__hint">{hint}</p> : null}
      <div className="terminal-command__row">
        <Terminal className="terminal-command__icon" aria-hidden="true" />
        <code className="terminal-command__code">{command}</code>
        <CopyButton text={command} label={t('errorAction.copyCommand')} />
      </div>
    </div>
  );
}
