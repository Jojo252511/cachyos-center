import { Check } from 'lucide-react';
import { useId, useState, type ReactNode } from 'react';

import { api } from '../../api/client';
import type { Dashboard } from '../../bindings/Dashboard';
import type { HealthReport } from '../../bindings/HealthReport';
import type { NewsStatus } from '../../bindings/NewsStatus';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import type { UpdateCheckResult } from '../../bindings/UpdateCheckResult';
import { CheckStateBadge, type CheckState } from '../../components/Badge';
import { Button } from '../../components/Button';
import { Card } from '../../components/Card';
import { ExternalLink } from '../../components/ExternalLink';
import { useI18n } from '../../i18n';
import { focusSoon } from '../../lib/focus';
import { latestUnread, type NewsGate } from '../../lib/news';
import { isOpenableUrl } from '../../lib/links';
import { useNow } from '../../state/now';
import type { Resource } from '../../state/useResource';

const GIB = 1024 * 1024 * 1024;
/** Margin on top of the plan size before free space counts as sufficient. */
const SPACE_MARGIN = 5 * GIB;

function CheckRow({ label, state, children }: { label: string; state: CheckState; children: ReactNode }) {
  return (
    <li className="checklist__row">
      <span className="checklist__label">{label}</span>
      <span className="checklist__state">
        <CheckStateBadge state={state} />
      </span>
      <div className="checklist__text">{children}</div>
    </li>
  );
}

export interface PreUpgradeChecksProps {
  updates: UpdateCheckResult;
  plan: TransactionPlan | null;
  dashboard: Dashboard | null;
  health: Resource<HealthReport>;
  news: Resource<NewsStatus>;
  gate: NewsGate;
}

/**
 * „Vor dem Upgrade“: network, disk space, lock, mirror, package signatures, news,
 * other updaters and snapshot status. Uncertain items are „unbekannt“.
 */
export function PreUpgradeChecks({ updates, plan, dashboard, health, news, gate }: PreUpgradeChecksProps) {
  const i18n = useI18n();
  const { t, fmt } = i18n;
  const now = useNow();
  const [acknowledging, setAcknowledging] = useState(false);
  const newsStatusId = useId();

  // Network
  const fresh = updates.status === 'fresh' && updates.error === null && updates.checkedAt !== null;
  const offline = updates.error?.code === 'OFFLINE';
  const networkState: CheckState = offline ? 'blocked' : fresh ? 'ok' : 'unknown';
  const networkText = offline
    ? t('precheck.network.offline')
    : fresh && updates.checkedAt !== null
      ? t('precheck.network.ok', { time: fmt.relative(updates.checkedAt, now) })
      : t('precheck.network.unknown');

  // Disk space
  const available = dashboard && dashboard.system.rootTotalBytes > 0 ? dashboard.system.rootAvailableBytes : null;
  const needed = plan ? plan.downloadSize + Math.max(plan.installSizeDelta, 0) : null;
  let diskState: CheckState = 'unknown';
  let diskText = t('precheck.disk.unknown');
  if (available !== null && needed !== null) {
    diskState = available < needed ? 'blocked' : available < needed + SPACE_MARGIN ? 'warning' : 'ok';
    diskText = t(diskState === 'ok' ? 'precheck.disk.ok' : 'precheck.disk.low', { available: fmt.bytes(available), needed: fmt.bytes(needed) });
  }

  // Lock
  const locked = dashboard?.lock.state === 'locked';
  const lockState: CheckState = dashboard ? (locked ? 'blocked' : 'ok') : 'unknown';

  // Mirror (package signatures are only verified while downloading)
  const mirrorError = updates.error && updates.error.code !== 'OFFLINE' ? updates.error : null;
  const mirrorState: CheckState = mirrorError ? 'blocked' : fresh ? 'ok' : 'unknown';
  const mirrorText = mirrorError
    ? t('precheck.mirror.error', { error: t(`error.${mirrorError.code}.title`) })
    : fresh
      ? t('precheck.mirror.ok')
      : t('precheck.mirror.unknown');

  // News
  const newsData = news.data;
  const newsState: CheckState = gate.state === 'clear' ? 'ok' : gate.state === 'unread' ? 'warning' : 'unknown';
  const unreadItems = newsData?.items.filter((item) => item.unread) ?? [];
  const acknowledge = async () => {
    if (!newsData) return;
    const until = latestUnread(newsData);
    if (until === null) return;
    setAcknowledging(true);
    try {
      news.mutate(await api.acknowledgeNews(until));
    } catch {
      news.reload();
    } finally {
      setAcknowledging(false);
      // The button is gone once no news is unread: keep the focus in the news row.
      focusSoon(() => document.getElementById(newsStatusId));
    }
  };

  // Other update services
  const report = health.data;
  const activeUpdaters = report?.externalUpdaters.filter((updater) => updater.active) ?? [];
  const servicesState: CheckState = !report ? 'unknown' : activeUpdaters.length > 0 ? 'warning' : 'ok';

  // Snapshot
  const snapshot = report?.snapshot;
  const snapshotState: CheckState = !snapshot ? 'unknown' : snapshot.snapPacActive ? 'ok' : snapshot.canRequestSnapshot ? 'info' : 'unknown';
  const snapshotText = !snapshot
    ? t('common.unknown')
    : snapshot.snapPacActive
      ? t('precheck.snapshot.snapPac')
      : snapshot.canRequestSnapshot
        ? t('precheck.snapshot.requestable')
        : t('precheck.snapshot.none');

  return (
    <Card title={t('precheck.title')} icon={<Check />}>
      <p className="muted">{t('precheck.intro')}</p>
      <ul className="checklist">
        <CheckRow label={t('precheck.network')} state={networkState}>
          {networkText}
        </CheckRow>
        <CheckRow label={t('precheck.disk')} state={diskState}>
          {diskText}
        </CheckRow>
        <CheckRow label={t('precheck.lock')} state={lockState}>
          {dashboard ? (locked ? t('precheck.lock.locked') : t('precheck.lock.free')) : t('common.unknown')}
        </CheckRow>
        <CheckRow label={t('precheck.mirror')} state={mirrorState}>
          {mirrorText}
        </CheckRow>
        {/* Package signatures are verified by pacman during the download: never "ok" in advance. */}
        <CheckRow label={t('precheck.signatures')} state="unknown">
          {t('precheck.signatures.pending')}
        </CheckRow>
        <CheckRow label={t('precheck.news')} state={newsState}>
          {/* One persistent status line: it keeps the focus after „Als gelesen markieren“
              removes the button, whatever the state changes to. */}
          <p id={newsStatusId} tabIndex={-1}>
            {gate.state === 'loading'
              ? t('precheck.news.loading')
              : gate.state === 'unread'
                ? t('precheck.news.unread', { count: gate.unreadCount })
                : gate.state === 'clear' && newsData?.fetchedAt
                  ? t('precheck.news.none', { time: fmt.relative(newsData.fetchedAt, now) })
                  : newsData?.disabled
                    ? t('precheck.news.disabled')
                    : t('precheck.news.unavailable')}
          </p>
          {gate.state === 'unread' && gate.incomplete ? <p>{newsData?.disabled ? t('precheck.news.disabled') : t('precheck.news.unavailable')}</p> : null}
          {newsData && newsData.errors.length > 0 ? (
            <ul className="plain-list muted">
              {newsData.errors.map((error) => (
                <li key={error.source}>{t('precheck.news.feedError', { source: t(`news.source.${error.source}`) })}</li>
              ))}
            </ul>
          ) : null}
          {unreadItems.length > 0 ? (
            <>
              <ul className="news-list">
                {unreadItems.map((item) => (
                  <li key={item.link} className="news-list__item">
                    <span className="news-list__source">{t(`news.source.${item.source}`)}</span>
                    {isOpenableUrl(item.link) ? <ExternalLink url={item.link}>{item.title}</ExternalLink> : <span>{item.title}</span>}
                    <span className="muted">{t('news.published', { time: fmt.date(item.publishedAt) })}</span>
                  </li>
                ))}
              </ul>
              <Button size="sm" busy={acknowledging} onClick={() => void acknowledge()}>
                {t('precheck.news.markRead')}
              </Button>
            </>
          ) : null}
        </CheckRow>
        <CheckRow label={t('precheck.services')} state={servicesState}>
          {!report
            ? t('precheck.services.unknown')
            : activeUpdaters.length > 0
              ? t('precheck.services.active', { names: activeUpdaters.map((updater) => updater.name).join(', ') })
              : t('precheck.services.none')}
        </CheckRow>
        <CheckRow label={t('precheck.snapshot')} state={snapshotState}>
          <p>{snapshotText}</p>
          <p className="muted">{t('precheck.snapshot.limit')}</p>
        </CheckRow>
        {dashboard?.offlineUpdatePrepared ? (
          <CheckRow label={t('precheck.offline')} state="blocked">
            {t('precheck.offline.prepared')}
          </CheckRow>
        ) : null}
      </ul>
    </Card>
  );
}
