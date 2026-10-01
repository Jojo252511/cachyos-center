import { Inbox, LoaderCircle } from 'lucide-react';
import type { ReactNode } from 'react';

import type { AppError } from '../bindings/AppError';
import { useI18n } from '../i18n';
import { ErrorPanel } from './ErrorPanel';

export function Spinner({ label }: { label?: string }) {
  return (
    <span className="spinner" role="status">
      <LoaderCircle className="spin" aria-hidden="true" />
      {label ? <span>{label}</span> : null}
    </span>
  );
}

export function LoadingState({ label }: { label?: string }) {
  const { t } = useI18n();
  return (
    <div className="state-view state-view--loading">
      <Spinner label={label ?? t('common.loading')} />
    </div>
  );
}

export function EmptyState({ title, children, action, icon }: { title: ReactNode; children?: ReactNode; action?: ReactNode; icon?: ReactNode }) {
  return (
    <div className="state-view state-view--empty">
      <span className="state-view__icon" aria-hidden="true">
        {icon ?? <Inbox />}
      </span>
      <p className="state-view__title">{title}</p>
      {children ? <div className="state-view__text">{children}</div> : null}
      {action ? <div className="state-view__action">{action}</div> : null}
    </div>
  );
}

/** Loading error of a view with „Erneut prüfen“. */
export function ErrorState({ error, onRetry, retryBusy }: { error: AppError; onRetry?: () => void; retryBusy?: boolean }) {
  const { t } = useI18n();
  return <ErrorPanel error={error} onRetry={onRetry} retryBusy={retryBusy} title={`${t('state.errorTitle')}: ${t(`error.${error.code}.title`)}`} />;
}
