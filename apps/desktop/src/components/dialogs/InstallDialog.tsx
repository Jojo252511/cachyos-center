import { Download, RefreshCw } from 'lucide-react';
import { useState } from 'react';

import { api } from '../../api/client';
import { normalizeError } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import { useI18n } from '../../i18n';
import { includedUpgrades } from '../../lib/plan';
import { usePackageActionGuard } from '../../state/guards';
import { useOperations } from '../../state/Operations';
import { useStatus } from '../../state/Status';
import { useResource } from '../../state/useResource';
import { Button } from '../Button';
import { AppDialog } from '../Dialog';
import { ErrorPanel } from '../ErrorPanel';
import { Notice } from '../Notice';
import { PlanSummary } from '../plan/PlanSummary';
import { GuardNotice } from '../StatusNotices';
import { LoadingState } from '../StateViews';

export interface InstallTarget {
  repository: string;
  name: string;
}

/**
 * Installation of a repository package as a consistent `pacman -Syu repo/pkg`
 * transaction. Outdated repository data (`STALE`) is refreshed on request.
 */
export function InstallDialog({ target, onClose }: { target: InstallTarget; onClose(): void }) {
  const { t } = useI18n();
  const status = useStatus();
  const operations = useOperations();
  const guard = usePackageActionGuard();
  const plan = useResource(`install:${target.repository}/${target.name}`, () => api.planInstall(target.repository, target.name));
  const [refreshError, setRefreshError] = useState<AppError | null>(null);
  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState<AppError | null>(null);

  const refreshNow = async () => {
    setRefreshError(null);
    const { result, error } = await status.checkUpdates();
    if (result?.status === 'fresh') {
      plan.reload();
    } else {
      setRefreshError(error ?? result?.error ?? { code: 'INTERNAL', message: `update check ended with status ${result?.status ?? 'unknown'}`, detail: null });
    }
  };

  const confirm = async () => {
    if (!plan.data) return;
    setStarting(true);
    setStartError(null);
    try {
      await operations.start({ kind: 'install', repository: target.repository, name: target.name, plan: plan.data });
      onClose();
    } catch (reason) {
      setStartError(normalizeError(reason));
    } finally {
      setStarting(false);
    }
  };

  const stale = plan.error?.code === 'STALE';
  const upgrades = plan.data ? includedUpgrades(plan.data) : 0;
  const ready = plan.data !== undefined && !plan.loading && !stale;

  return (
    <AppDialog
      open
      onOpenChange={(open) => (open ? undefined : onClose())}
      variant="transaction"
      locked={starting}
      title={t('install.dialog.title', { name: target.name })}
      description={t('install.dialog.description', { repository: target.repository })}
      footer={
        <>
          <Button onClick={onClose} disabled={starting}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="primary"
            icon={<Download />}
            busy={starting}
            disabled={!ready || guard.reason !== null || status.checking}
            onClick={() => void confirm()}
          >
            {t('install.dialog.confirm')}
          </Button>
        </>
      }
    >
      <Notice tone="info">
        <p>
          {t('install.dialog.syuExplanation', { target: `${target.repository}/${target.name}` })}{' '}
          {ready ? (upgrades > 0 ? t('warning.includesSystemUpgrade', { count: upgrades }) : t('install.dialog.noOtherUpdates')) : null}
        </p>
      </Notice>

      {stale ? (
        <Notice
          tone="warning"
          title={t('install.dialog.staleTitle')}
          actions={
            <Button size="sm" icon={<RefreshCw />} busy={status.checking} onClick={() => void refreshNow()}>
              {status.checking ? t('install.dialog.refreshing') : t('install.dialog.refreshNow')}
            </Button>
          }
        >
          <p>{t('install.dialog.staleText')}</p>
        </Notice>
      ) : null}
      {refreshError ? (
        <ErrorPanel error={refreshError} announce title={t('install.dialog.refreshFailed')} onRetry={() => void refreshNow()} retryBusy={status.checking} />
      ) : null}
      {plan.error && !stale ? <ErrorPanel error={plan.error} title={t('install.dialog.blocked')} onRetry={plan.reload} /> : null}

      {plan.loading && !plan.error ? <LoadingState label={t('install.dialog.planning')} /> : null}
      {ready && plan.data ? (
        <>
          <PlanSummary plan={plan.data} hideWarnings={['includesSystemUpgrade']} />
          <Notice tone="info">
            <p>{t('plan.systemDbNote')}</p>
          </Notice>
          <p className="muted">{t('plan.polkitNote')}</p>
        </>
      ) : null}
      <GuardNotice message={guard.message} />
      {startError ? (
        <ErrorPanel error={startError} announce title={`${t('operation.startFailed')}: ${t(`error.${startError.code}.title`)}`} />
      ) : null}
    </AppDialog>
  );
}
