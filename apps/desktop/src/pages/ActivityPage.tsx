import { RefreshCw } from 'lucide-react';

import { api } from '../api/client';
import type { HistoryEntry } from '../bindings/HistoryEntry';
import type { HistorySource } from '../bindings/HistorySource';
import { Badge, type BadgeTone } from '../components/Badge';
import { Button } from '../components/Button';
import { PageHeader } from '../components/PageHeader';
import { EmptyState, ErrorState, LoadingState } from '../components/StateViews';
import { useI18n } from '../i18n';
import { activityOutcome } from '../lib/activity';
import { useStatus } from '../state/Status';
import { useResource } from '../state/useResource';

const LIMIT = 100;

const SOURCE_TONE: Record<HistorySource, BadgeTone> = { app: 'accent', timer: 'info', externalPacman: 'neutral' };

function ActivityItem({ entry }: { entry: HistoryEntry }) {
  const i18n = useI18n();
  const { t, fmt } = i18n;
  const outcome = activityOutcome(entry, i18n);
  const hasCounts = entry.installed + entry.upgraded + entry.removed + entry.downgraded > 0;
  return (
    <li className="activity-item">
      <div className="activity-item__head">
        <Badge tone={SOURCE_TONE[entry.source]}>{t(`activity.source.${entry.source}`)}</Badge>
        <h2 className="activity-item__title">{entry.kind ? t(`operation.kind.${entry.kind}`) : t('activity.kind.external')}</h2>
        <Badge tone={outcome.tone}>{outcome.text}</Badge>
      </div>
      <dl className="activity-item__meta">
        <div>
          <dt>{t('activity.started')}</dt>
          <dd>
            <time dateTime={fmt.iso(entry.startedAt)}>{fmt.dateTime(entry.startedAt)}</time>
          </dd>
        </div>
        <div>
          <dt>{t('activity.ended')}</dt>
          <dd>{entry.endedAt !== null ? <time dateTime={fmt.iso(entry.endedAt)}>{fmt.dateTime(entry.endedAt)}</time> : outcome.tone === 'info' ? t('activity.running') : t('common.unknown')}</dd>
        </div>
        {hasCounts ? (
          <div>
            <dt>{t('operation.changes.title')}</dt>
            <dd>
              {(
                [
                  ['installed', entry.installed],
                  ['upgraded', entry.upgraded],
                  ['removed', entry.removed],
                  ['downgraded', entry.downgraded],
                ] as const
              )
                .filter(([, count]) => count > 0)
                .map(([kind, count]) => t(`activity.count.${kind}`, { count }))
                .join(', ')}
            </dd>
          </div>
        ) : null}
      </dl>
      {entry.errorCode ? <p className="status-line status-line--danger">{t('activity.error', { error: t(`error.${entry.errorCode}.title`) })}</p> : null}
      {entry.packages.length > 0 ? (
        <details className="details">
          <summary>{t('activity.packages', { count: entry.packages.length })}</summary>
          <p className="mono">{entry.packages.join(', ')}</p>
        </details>
      ) : null}
    </li>
  );
}

export function ActivityPage() {
  const { t } = useI18n();
  const status = useStatus();
  const activity = useResource('activity', () => api.getActivity(LIMIT), status.activityVersion);

  return (
    <div className="page">
      <PageHeader
        title={t('activity.title')}
        subtitle={t('activity.subtitle')}
        actions={
          <Button variant="primary" icon={<RefreshCw />} busy={activity.loading && activity.data !== undefined} onClick={activity.reload}>
            {t('common.refresh')}
          </Button>
        }
      />
      {activity.error ? <ErrorState error={activity.error} onRetry={activity.reload} /> : null}
      {!activity.data && !activity.error ? <LoadingState /> : null}
      {activity.data ? (
        activity.data.length === 0 ? (
          <EmptyState title={t('activity.empty')} />
        ) : (
          <>
            <ol className="activity-list">
              {activity.data.map((entry) => (
                <ActivityItem key={entry.id} entry={entry} />
              ))}
            </ol>
            {activity.data.length >= LIMIT ? <p className="muted">{t('activity.limit', { count: LIMIT })}</p> : null}
          </>
        )
      ) : null}
    </div>
  );
}
