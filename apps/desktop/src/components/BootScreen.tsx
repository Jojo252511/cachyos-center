import { RefreshCw } from 'lucide-react';

import type { AppError } from '../bindings/AppError';
import { useI18n } from '../i18n';
import { Button } from './Button';
import { Spinner } from './StateViews';

export function BootScreen() {
  const { t } = useI18n();
  return (
    <div className="boot-screen">
      <Spinner label={t('shell.loading')} />
    </div>
  );
}

export function StartupError({ error, onRetry }: { error: AppError; onRetry(): void }) {
  const { t } = useI18n();
  return (
    <div className="boot-screen">
      <div className="boot-screen__panel" role="alert">
        <h1 className="boot-screen__title">{t('shell.startupFailed')}</h1>
        <p>{t('shell.startupFailedText')}</p>
        <p className="muted">
          <code>{error.code}</code>: {error.message}
        </p>
        <Button variant="primary" icon={<RefreshCw />} onClick={onRetry}>
          {t('errorAction.retry')}
        </Button>
      </div>
    </div>
  );
}
