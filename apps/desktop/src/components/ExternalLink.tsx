import { ExternalLink as ExternalIcon } from 'lucide-react';
import { useState, type ReactNode } from 'react';

import { api } from '../api/client';
import { useI18n } from '../i18n';

/**
 * Opens an allow-listed official URL in the browser via `open_external`,
 * only after an explicit user action.
 */
export function ExternalLink({ url, children, className }: { url: string; children: ReactNode; className?: string }) {
  const { t } = useI18n();
  const [failed, setFailed] = useState(false);
  const open = async () => {
    setFailed(false);
    try {
      await api.openExternal(url);
    } catch {
      setFailed(true);
    }
  };
  return (
    <span className={`external-link ${className ?? ''}`}>
      <button type="button" className="link-button" onClick={() => void open()}>
        {children}
        <ExternalIcon className="link-button__icon" aria-hidden="true" />
        <span className="sr-only"> {t('common.opensInBrowser')}</span>
      </button>
      {failed ? (
        <span className="external-link__error" role="alert">
          {url}
        </span>
      ) : null}
    </span>
  );
}
