import { BookOpen, FileText, RefreshCw } from 'lucide-react';

import { describeError } from '../api/errors';
import type { AppError } from '../bindings/AppError';
import { useI18n } from '../i18n';
import { Button } from './Button';
import { ExternalLink } from './ExternalLink';
import { Notice, type NoticeTone } from './Notice';
import { TerminalCommand } from './TerminalCommand';

export interface ErrorPanelProps {
  error: AppError;
  /** „Erneut prüfen“; hidden when not provided. */
  onRetry?: () => void;
  retryLabel?: string;
  retryBusy?: boolean;
  /** „Log ansehen“; hidden when not provided. */
  onViewLog?: () => void;
  /** Overrides the default terminal command for the code. */
  command?: string;
  title?: string;
  /** Errors after a user action are announced (`role="alert"`). */
  announce?: boolean;
  tone?: NoticeTone;
  /** Id of the element that takes the focus when the panel disappears, e.g. after „Erneut prüfen“. */
  focusFallback?: string;
}

const WARNING_CODES = new Set(['BUSY', 'OFFLINE', 'STALE', 'NOT_AUTHORIZED', 'PREREQUISITE_MISSING', 'BLOCKED', 'CONFLICT']);

/** Localized error with its concrete next steps and the technical detail on demand. */
export function ErrorPanel({ error, onRetry, retryLabel, retryBusy, onViewLog, command, title, announce = false, tone, focusFallback }: ErrorPanelProps) {
  const { t } = useI18n();
  const info = describeError(error, t, command ? { command } : undefined);
  const showRetry = info.actions.includes('retry') && onRetry;
  const showLog = info.actions.includes('viewLog') && onViewLog;
  const showTerminal = info.actions.includes('terminal') && info.command;
  const showDocs = info.actions.includes('docs') && info.docUrl;
  return (
    <Notice
      tone={tone ?? (WARNING_CODES.has(error.code) ? 'warning' : 'danger')}
      title={title ?? info.title}
      role={announce ? 'alert' : undefined}
      className="error-panel"
      focusFallback={focusFallback}
      actions={
        showRetry || showLog || showDocs ? (
          <>
            {showRetry ? (
              <Button size="sm" icon={<RefreshCw />} onClick={onRetry} busy={retryBusy}>
                {retryLabel ?? t('errorAction.retry')}
              </Button>
            ) : null}
            {showLog ? (
              <Button size="sm" icon={<FileText />} onClick={onViewLog}>
                {t('errorAction.viewLog')}
              </Button>
            ) : null}
            {showDocs && info.docUrl ? (
              <span className="error-panel__docs">
                <BookOpen aria-hidden="true" className="error-panel__docs-icon" />
                <ExternalLink url={info.docUrl}>{t('errorAction.docs')}</ExternalLink>
              </span>
            ) : null}
          </>
        ) : undefined
      }
    >
      <p>{info.explanation}</p>
      {showTerminal && info.command ? (
        <div className="error-panel__terminal">
          <p className="error-panel__terminal-title">{t('errorAction.terminal')}</p>
          <TerminalCommand command={info.command} hint={t('errorAction.terminalHint')} />
        </div>
      ) : null}
      <details className="details">
        <summary>{t('common.technicalDetails')}</summary>
        <dl className="kv kv--compact">
          <div className="kv__row">
            <dt className="kv__key">{t('error.technicalMessage')}</dt>
            <dd className="kv__value">
              <code>{error.code}</code>: {error.message}
            </dd>
          </div>
          {error.detail ? (
            <div className="kv__row">
              <dt className="kv__key">{t('common.technicalDetails')}</dt>
              <dd className="kv__value">
                <pre className="mono-block">{error.detail}</pre>
              </dd>
            </div>
          ) : null}
        </dl>
      </details>
    </Notice>
  );
}
