import { CircleArrowUp, Download, Pause, RefreshCw, RotateCw } from 'lucide-react';
import { useEffect, useState } from 'react';

import { api } from '../api/client';
import { prerequisiteCommand } from '../api/errors';
import { Badge } from '../components/Badge';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { UpgradeDialog } from '../components/dialogs/UpgradeDialog';
import { ErrorPanel } from '../components/ErrorPanel';
import { Notice } from '../components/Notice';
import { PageHeader } from '../components/PageHeader';
import { GuardNotice, LockNotice, PlatformNotices } from '../components/StatusNotices';
import { EmptyState, LoadingState, Spinner } from '../components/StateViews';
import { TerminalCommand } from '../components/TerminalCommand';
import { useI18n } from '../i18n';
import { newsGate } from '../lib/news';
import { usePackageActionGuard } from '../state/guards';
import { useNow } from '../state/now';
import { useOperations } from '../state/Operations';
import { useStatus } from '../state/Status';
import { useResource } from '../state/useResource';
import { PreUpgradeChecks } from './updates/PreUpgradeChecks';
import { UpdateTable } from './updates/UpdateTable';

/** The check button in the header; it takes the focus when another check button or an error panel disappears. */
const CHECK_BUTTON_ID = 'updates-check';

export function UpdatesPage() {
  const { t, fmt } = useI18n();
  const status = useStatus();
  const operations = useOperations();
  const guard = usePackageActionGuard();
  const now = useNow();
  const [dialogOpen, setDialogOpen] = useState(false);
  const news = useResource('news', () => api.getNews(true, false), status.activityVersion);
  const health = useResource('health', () => api.getHealth(), status.activityVersion);
  const { refreshUpdates } = status;

  useEffect(() => {
    void refreshUpdates();
  }, [refreshUpdates]);

  const updates = status.updates;
  const dashboard = status.dashboard;
  const gate = newsGate(news.data, news.error, news.loading);

  if (!updates) {
    return (
      <div className="page">
        <PageHeader title={t('updates.title')} subtitle={t('updates.subtitle')} />
        <PlatformNotices />
        <LoadingState />
      </div>
    );
  }

  const plan = updates.plan;
  // With packages held back by the pacman configuration an online upgrade is not complete.
  const partial = updates.heldBack.length > 0;
  const hasData = updates.checkedAt !== null && (updates.status === 'fresh' || updates.status === 'stale' || updates.status === 'failed');
  const installReason =
    guard.message ??
    (status.checking
      ? t('guard.checking')
      : updates.status !== 'fresh'
        ? t('updates.needsFreshCheck')
        : !plan || plan.entries.length === 0
          ? t('updates.nothingToInstall')
          : null);
  const canInstall = installReason === null && plan !== null;
  const showReason = installReason !== null && (guard.reason !== null || (updates.status === 'fresh' && updates.updates.length > 0) || updates.status === 'stale');

  const header = (
    <PageHeader
      title={t('updates.title')}
      subtitle={partial ? t('updates.subtitleHeldBack') : t('updates.subtitle')}
      actions={
        <>
          <Button id={CHECK_BUTTON_ID} icon={<RefreshCw />} busy={status.checking} disabled={updates.status === 'unsupported'} onClick={() => void status.checkUpdates()}>
            {status.checking ? t('updates.checking') : t('updates.check')}
          </Button>
          <Button variant="primary" icon={<Download />} disabled={!canInstall} aria-describedby={showReason ? 'install-reason' : undefined} onClick={() => setDialogOpen(true)}>
            {t('updates.install')}
          </Button>
        </>
      }
    />
  );

  return (
    <div className="page">
      {header}
      <PlatformNotices />
      {showReason ? (
        <div id="install-reason">
          <GuardNotice message={t('updates.blockedReason', { reason: installReason ?? '' })} />
        </div>
      ) : null}
      {dashboard ? <LockNotice lock={dashboard.lock} /> : null}
      {dashboard?.offlineUpdatePrepared ? (
        <Notice tone="info" title={t('dashboard.offlinePrepared')}>
          <p>{t('state.offlinePreparedText')}</p>
        </Notice>
      ) : null}
      {status.checking ? (
        <Notice tone="info" role="status">
          <Spinner label={t('updates.checkingHint')} />
        </Notice>
      ) : null}
      {status.checkError ? (
        <ErrorPanel error={status.checkError} announce onRetry={() => void status.checkUpdates()} retryBusy={status.checking} focusFallback={CHECK_BUTTON_ID} />
      ) : null}

      {updates.status === 'unsupported' ? (
        <Notice tone="danger" title={t('updates.unsupported.title')}>
          <p>{t('updates.unsupported.text')}</p>
        </Notice>
      ) : null}
      {updates.status === 'prerequisiteMissing' ? (
        <Notice tone="warning" title={t('updates.prerequisite.title')}>
          <p>{t('updates.prerequisite.text', { packages: fmt.list(updates.missingPrerequisites.length > 0 ? updates.missingPrerequisites : ['pacman-contrib']) })}</p>
          <TerminalCommand command={prerequisiteCommand(updates.missingPrerequisites)} hint={t('errorAction.terminalHint')} />
        </Notice>
      ) : null}
      {updates.status === 'neverChecked' && !status.checking ? (
        <EmptyState
          icon={<CircleArrowUp />}
          title={t('updates.neverChecked.title')}
          action={
            <Button variant="primary" icon={<RefreshCw />} data-focus-fallback={CHECK_BUTTON_ID} onClick={() => void status.checkUpdates()}>
              {t('updates.check')}
            </Button>
          }
        >
          <p>{t('updates.neverChecked.text')}</p>
        </EmptyState>
      ) : null}
      {updates.status === 'failed' ? (
        <>
          {updates.error ? (
            <ErrorPanel error={updates.error} onRetry={() => void status.checkUpdates()} retryBusy={status.checking} focusFallback={CHECK_BUTTON_ID} />
          ) : null}
          <p className="muted">
            {updates.attemptedAt !== null ? t('updates.lastAttempt', { time: fmt.relative(updates.attemptedAt, now) }) : null}{' '}
            {updates.checkedAt !== null ? t('updates.lastSuccess', { time: fmt.dateTime(updates.checkedAt) }) : null}
          </p>
          {updates.checkedAt !== null ? <Notice tone="warning">{t('updates.failedOld', { time: fmt.dateTime(updates.checkedAt) })}</Notice> : null}
        </>
      ) : null}
      {updates.status === 'stale' && updates.checkedAt !== null ? (
        <Notice tone="warning" title={t('updates.stale.title')}>
          <p>{t('updates.stale.text', { time: fmt.dateTime(updates.checkedAt) })}</p>
        </Notice>
      ) : null}

      {hasData ? (
        <>
          {updates.status === 'fresh' && updates.checkedAt !== null ? (
            <p className="muted">{t('updates.stateFresh', { time: fmt.relative(updates.checkedAt, now) })}</p>
          ) : null}
          {updates.updates.length === 0 && updates.status === 'fresh' ? (
            updates.heldBack.length > 0 ? (
              <Notice tone="warning">{t('updates.noneHeldBack')}</Notice>
            ) : (
              <Notice tone="success">{t('updates.none')}</Notice>
            )
          ) : null}
          {updates.updates.length > 0 ? (
            <Card
              title={
                <>
                  {t('updates.list.title', { count: updates.updates.length })}
                  {updates.status !== 'fresh' ? <Badge tone="warning">{t('updates.list.outdated')}</Badge> : null}
                </>
              }
              icon={<CircleArrowUp />}
            >
              <p className="muted">{partial ? t('updates.list.explanationHeldBack') : t('updates.list.explanation')}</p>
              <UpdateTable updates={updates.updates} caption={t('updates.list.title', { count: updates.updates.length })} />
              <p className="table-footer">
                {updates.totalDownloadSize !== null ? t('updates.list.totalDownload', { size: fmt.bytes(updates.totalDownloadSize) }) : null}
              </p>
              {updates.rebootRecommended ? (
                <p className="status-line status-line--info">
                  <RotateCw aria-hidden="true" />
                  {t('updates.rebootHint')}
                </p>
              ) : null}
            </Card>
          ) : null}
          {updates.heldBack.length > 0 ? (
            <Card title={t('updates.heldBack.title', { count: updates.heldBack.length })} icon={<Pause />}>
              <Notice tone="warning">
                <p>{t('updates.heldBack.text')}</p>
              </Notice>
              <UpdateTable updates={updates.heldBack} caption={t('updates.heldBack.title', { count: updates.heldBack.length })} />
            </Card>
          ) : null}
          {updates.updates.length > 0 ? (
            <PreUpgradeChecks updates={updates} plan={plan} dashboard={dashboard} health={health} news={news} gate={gate} />
          ) : null}
        </>
      ) : null}

      {dialogOpen && plan ? (
        <UpgradeDialog
          open
          onOpenChange={setDialogOpen}
          plan={plan}
          heldBack={updates.heldBack}
          news={gate}
          snapshot={health.data?.snapshot ?? null}
          blockedReason={guard.message}
          onConfirm={(createSnapshot) => operations.start({ kind: 'upgrade', plan, createSnapshot })}
        />
      ) : null}
    </div>
  );
}
