import { Check, Copy } from 'lucide-react';
import { useState } from 'react';

import { api } from '../api/client';
import { useI18n } from '../i18n';
import { Button } from './Button';

/** Copies text via `write_clipboard`, only on an explicit click. */
export function CopyButton({ text, label, doneLabel }: { text: string; label?: string; doneLabel?: string }) {
  const { t } = useI18n();
  const [state, setState] = useState<'idle' | 'busy' | 'done' | 'failed'>('idle');
  const copy = async () => {
    setState('busy');
    try {
      await api.writeClipboard(text);
      setState('done');
    } catch {
      setState('failed');
    }
  };
  return (
    <span className="copy-button">
      <Button size="sm" variant="ghost" icon={state === 'done' ? <Check /> : <Copy />} busy={state === 'busy'} onClick={() => void copy()}>
        {label ?? t('common.copy')}
      </Button>
      <span className="copy-button__status" role="status">
        {state === 'done' ? (doneLabel ?? t('common.copied')) : state === 'failed' ? t('common.copyFailed') : ''}
      </span>
    </span>
  );
}
