import { Copy } from 'lucide-react';
import { useState } from 'react';

import { api } from '../../api/client';
import { useI18n } from '../../i18n';
import { useResource } from '../../state/useResource';
import { Button } from '../Button';
import { AppDialog } from '../Dialog';
import { ErrorPanel } from '../ErrorPanel';
import { LoadingState } from '../StateViews';

/**
 * Preview of the sanitized diagnostic report. The text is copied only after
 * the user reviewed it and clicked „Kopieren“.
 */
export function DiagnosticDialog({ onClose }: { onClose(): void }) {
  const { t } = useI18n();
  const report = useResource('diagnostic-report', () => api.getDiagnosticReport());
  const [copyState, setCopyState] = useState<'idle' | 'busy' | 'done' | 'failed'>('idle');

  const copy = async () => {
    if (report.data === undefined) return;
    setCopyState('busy');
    try {
      await api.writeClipboard(report.data);
      setCopyState('done');
    } catch {
      setCopyState('failed');
    }
  };

  return (
    <AppDialog
      open
      onOpenChange={(open) => (open ? undefined : onClose())}
      title={t('diagnostic.title')}
      description={t('diagnostic.description')}
      footer={
        <>
          <span className="dialog__status" role="status">
            {copyState === 'done' ? t('diagnostic.copied') : copyState === 'failed' ? t('common.copyFailed') : ''}
          </span>
          <Button onClick={onClose}>{t('common.close')}</Button>
          <Button variant="primary" icon={<Copy />} busy={copyState === 'busy'} disabled={report.data === undefined} onClick={() => void copy()}>
            {t('diagnostic.copy')}
          </Button>
        </>
      }
    >
      {report.error ? <ErrorPanel error={report.error} onRetry={report.reload} /> : null}
      {report.data === undefined && !report.error ? <LoadingState label={t('diagnostic.loading')} /> : null}
      {report.data !== undefined ? (
        <pre className="mono-block diagnostic-preview" tabIndex={0} aria-label={t('diagnostic.previewLabel')}>
          {report.data}
        </pre>
      ) : null}
    </AppDialog>
  );
}
