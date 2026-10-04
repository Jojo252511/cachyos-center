import {
  ArrowRight,
  CircleArrowUp,
  CircleCheck,
  CircleQuestionMark,
  Cpu,
  HeartPulse,
  Info,
  LoaderCircle,
  Lock,
  OctagonAlert,
  Package,
  RefreshCw,
  RotateCw,
  TriangleAlert,
} from 'lucide-react';
import type { ReactNode } from 'react';

import type { Dashboard } from '../bindings/Dashboard';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';
import { Badge, SeverityIcon } from '../components/Badge';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { KeyValue, KeyValueList } from '../components/KeyValue';
import { Notice } from '../components/Notice';
import { PageHeader } from '../components/PageHeader';
import { ErrorState, LoadingState } from '../components/StateViews';
import { TimeText } from '../components/TimeText';
import { useI18n, type I18n } from '../i18n';
import { activityOutcome } from '../lib/activity';
import { dashboardHeadline, type Headline } from '../lib/headline';
import { useAppState } from '../state/AppState';
import { useNow } from '../state/now';
import { isTerminalState, useOperations } from '../state/Operations';
import { hrefFor } from '../state/router';
import { useStatus } from '../state/Status';

const HEADLINE_ICON: Record<Headline['kind'], ReactNode> = {
  operation: <LoaderCircle className="spin" />,
  unsupported: <OctagonAlert />,
  locked: <Lock />,
  neverChecked: <CircleQuestionMark />,
  failed: <OctagonAlert />,
  prerequisiteMissing: <TriangleAlert />,
  stale: <TriangleAlert />,
  updates: <CircleArrowUp />,
  currentHeldBack: <TriangleAlert />,
  current: <CircleCheck />,
};

function headlineTitle(headline: Headline, updates: UpdateCheckResult, operationKind: string, { t }: I18n): string {
  switch (headline.kind) {
    case 'operation':
      return t('dashboard.status.operation', { kind: operationKind });
    case 'unsupported':
      return t('dashboard.status.unsupported');
    case 'locked':
      return t('dashboard.status.locked');
    case 'neverChecked':
      return t('dashboard.status.neverChecked');
    case 'failed':
      return t('dashboard.status.checkFailed');
    case 'prerequisiteMissing':
      return t('dashboard.status.prerequisiteMissing');
    case 'stale':
      return t('dashboard.status.stale');
    case 'updates':
      return t('dashboard.status.updates', { count: headline.count ?? 0 });
    case 'currentHeldBack':
      return t('dashboard.status.currentHeldBack', { count: updates.heldBack.length });
    case 'current':
      return t('dashboard.status.current');
  }
}

function headlineDetail(headline: Headline, updates: UpdateCheckResult, now: number, { t, fmt }: I18n): string {
  const checked = updates.checkedAt !== null ? fmt.relative(updates.checkedAt, now) : t('common.unknown');
  switch (headline.kind) {
    case 'operation':
      return t('dashboard.detail.operation');
    case 'unsupported':
      return t('dashboard.detail.unsupported');
    case 'locked':
      return t('dashboard.detail.locked');
    case 'neverChecked':
      return t('dashboard.detail.neverChecked');
    case 'failed':
      return t('dashboard.detail.failed');
    case 'prerequisiteMissing':
      return t('dashboard.detail.prerequisiteMissing', { packages: fmt.list(updates.missingPrerequisites.length > 0 ? updates.missingPrerequisites : ['pacman-contrib']) });
    case 'stale':
      return t('dashboard.detail.stale', { time: updates.checkedAt !== null ? fmt.dateTime(updates.checkedAt) : t('common.unknown') });
    case 'updates':
      return t('dashboard.detail.updates', { time: checked, size: fmt.bytes(updates.totalDownloadSize ?? 0) });
    case 'currentHeldBack':
    case 'current':
      return t('dashboard.detail.current', { time: checked });
  }
}

function lastUpdateLine(dashboard: Dashboard, i18n: I18n): string {
  const { t, fmt } = i18n;
  const last = dashboard.lastActivity;
  const attemptFailed =
    last !== null &&
    last.kind === 'systemUpgrade' &&
    last.state !== null &&
    isTerminalState(last.state) &&
    last.state !== 'succeeded' &&
    last.startedAt > (dashboard.lastFullUpgrade ?? 0);
  if (attemptFailed && last.state) {
    return t('dashboard.lastUpgradeAttempt', { date: fmt.dateTime(last.startedAt), result: t(`operation.state.${last.state}`) });
  }
  return dashboard.lastFullUpgrade !== null ? t('dashboard.lastUpdate', { date: fmt.dateTime(dashboard.lastFullUpgrade) }) : t('dashboard.lastUpdateUnknown');
}

function UpdatesCard({ updates, headline }: { updates: UpdateCheckResult; headline: Headline }) {
  const i18n = useI18n();
  const { t, fmt } = i18n;
  const reliable = headline.count !== null && updates.status === 'fresh';
  return (
    <Card
      title={t('dashboard.card.updates')}
      icon={<CircleArrowUp />}
      footer={
        <a className="card-link" href={hrefFor('updates')}>
          {t('dashboard.card.updatesLink')} <ArrowRight aria-hidden="true" />
        </a>
      }
    >
      {reliable ? (
        <p className="big-number">
          <span className="big-number__value">{fmt.number(updates.updates.length)}</span>
          <span className="big-number__label">{t('dashboard.card.updatesAvailable', { count: updates.updates.length })}</span>
        </p>
      ) : (
        <p className="big-number big-number--unknown">
          <span className="big-number__label">{t('dashboard.card.updatesUnknown')}</span>
        </p>
      )}
      <ul className="plain-list">
        {reliable && updates.updates.length > 0 && updates.totalDownloadSize !== null ? (
          <li>{t('dashboard.card.download', { size: fmt.bytes(updates.totalDownloadSize) })}</li>
        ) : null}
        {updates.heldBack.length > 0 ? <li>{t('dashboard.card.heldBack', { count: updates.heldBack.length })}</li> : null}
        {updates.checkedAt !== null ? (
          <li>
            {t('dashboard.card.checked')} <TimeText seconds={updates.checkedAt} />
          </li>
        ) : null}
      </ul>
    </Card>
  );
}

function SystemCard({ dashboard }: { dashboard: Dashboard }) {
  const { t, fmt } = useI18n();
  const s = dashboard.system;
  const lowSpace = dashboard.healthItems.some((item) => item.kind === 'lowDiskSpace');
  return (
    <Card
      title={t('dashboard.card.system')}
      icon={<Cpu />}
      className="card--wide"
      footer={
        <a className="card-link" href={hrefFor('system')}>
          {t('dashboard.card.systemLink')} <ArrowRight aria-hidden="true" />
        </a>
      }
    >
      <KeyValueList className="kv--compact kv--columns">
        <KeyValue label={t('system.os.name')}>
          {s.os}
          {!s.isCachyos ? <span className="muted block">{t('dashboard.os.notCachyos')}</span> : null}
        </KeyValue>
        <KeyValue label={t('dashboard.card.kernel')}>
          <span className="mono">{s.kernel}</span>
        </KeyValue>
        <KeyValue label={t('dashboard.card.uptime')}>{fmt.duration(s.uptimeSeconds)}</KeyValue>
        <KeyValue label={t('dashboard.card.cpu')}>{s.cpu}</KeyValue>
        {s.gpus.length > 0 ? <KeyValue label={t('dashboard.card.gpu')}>{s.gpus.join(', ')}</KeyValue> : null}
        <KeyValue label={t('dashboard.card.memory')}>
          {t('dashboard.card.freeOf', { available: fmt.bytes(s.memoryAvailableBytes), total: fmt.bytes(s.memoryTotalBytes) })}
        </KeyValue>
        <KeyValue label={t('dashboard.card.rootSpace')}>
          {s.rootTotalBytes > 0
            ? `${t('dashboard.card.freeOf', { available: fmt.bytes(s.rootAvailableBytes), total: fmt.bytes(s.rootTotalBytes) })} · ${s.rootFilesystem}`
            : t('common.unknown')}
          {lowSpace ? (
            <span className="block">
              <Badge tone="warning" icon={<TriangleAlert />}>
                {t('dashboard.lowSpace')}
              </Badge>
            </span>
          ) : null}
        </KeyValue>
        <KeyValue label={t('dashboard.card.session')}>
          {t(`session.${s.session}`)}
          {s.desktop && s.session !== 'hyprland' ? ` (${s.desktop})` : ''}
        </KeyValue>
      </KeyValueList>
    </Card>
  );
}

function SoftwareCard({ dashboard }: { dashboard: Dashboard }) {
  const { t, fmt } = useI18n();
  return (
    <Card
      title={t('dashboard.card.software')}
      icon={<Package />}
      footer={
        <a className="card-link" href={hrefFor('software')}>
          {t('dashboard.card.softwareLink')} <ArrowRight aria-hidden="true" />
        </a>
      }
    >
      {dashboard.system.packageBackendReady ? (
        <>
          <p className="big-number">
            <span className="big-number__value">{fmt.number(dashboard.installedCount)}</span>
            <span className="big-number__label">{t('dashboard.card.installed')}</span>
          </p>
          <p className="muted">{t('dashboard.card.foreign', { count: dashboard.foreignCount })}</p>
        </>
      ) : (
        <p className="big-number big-number--unknown">
          <span className="big-number__label">{t('common.notAvailable')}</span>
        </p>
      )}
    </Card>
  );
}

function HealthCard({ dashboard }: { dashboard: Dashboard }) {
  const { t } = useI18n();
  const worst = dashboard.healthWorst;
  const top = dashboard.healthItems.slice(0, 3);
  const more = dashboard.healthItems.length - top.length;
  return (
    <Card
      title={t('dashboard.card.health')}
      icon={<HeartPulse />}
      footer={
        <a className="card-link" href={hrefFor('system', 'health')}>
          {t('dashboard.card.healthLink')} <ArrowRight aria-hidden="true" />
        </a>
      }
    >
      <p className={`status-line ${worst ? `status-line--${worst === 'critical' ? 'danger' : worst}` : 'status-line--success'}`}>
        {worst ? <SeverityIcon severity={worst} /> : <CircleCheck aria-hidden="true" />}
        <strong>{worst ? t(`dashboard.card.healthWorst.${worst}`) : t('dashboard.card.healthNone')}</strong>
      </p>
      {top.length > 0 ? (
        <ul className="icon-list">
          {top.map((item) => (
            <li key={item.kind} className={`icon-list__item icon-list__item--${item.severity === 'critical' ? 'danger' : item.severity}`}>
              <SeverityIcon severity={item.severity} />
              <span>
                <span className="sr-only">{t(`severity.${item.severity}`)}: </span>
                {t(`health.kind.${item.kind}`)}
                {item.count !== null ? ` (${item.count})` : ''}
              </span>
            </li>
          ))}
        </ul>
      ) : null}
      {more > 0 ? <p className="muted">{t('common.more', { count: more })}</p> : null}
    </Card>
  );
}

export function DashboardPage() {
  const i18n = useI18n();
  const { t } = i18n;
  const status = useStatus();
  const operations = useOperations();
  const { settings } = useAppState();
  const now = useNow();
  const { dashboard } = status;

  if (!dashboard) {
    return (
      <div className="page">
        <PageHeader title={t('dashboard.title')} />
        {status.dashboardError ? <ErrorState error={status.dashboardError} onRetry={() => void status.refresh()} /> : <LoadingState />}
      </div>
    );
  }

  const updates = status.updates ?? dashboard.updates;
  const headline = dashboardHeadline({ updates, lock: dashboard.lock, operationActive: operations.active });
  const operationKind = operations.tracked?.operation ? t(`operation.kind.${operations.tracked.operation.kind}`) : t('operation.kind.systemUpgrade');
  const canCheck = updates.status !== 'unsupported';

  const checkButton = (variant: 'primary' | 'secondary') => (
    <Button variant={variant} icon={<RefreshCw />} busy={status.checking} onClick={() => void status.checkUpdates()}>
      {status.checking ? t('updates.checking') : t('dashboard.action.check')}
    </Button>
  );

  let primary: ReactNode = null;
  if (headline.kind === 'operation') {
    primary = (
      <Button
        variant="primary"
        onClick={() => {
          operations.setExpanded(true);
          document.getElementById('operation-panel')?.scrollIntoView?.({ block: 'start' });
        }}
      >
        {t('dashboard.action.showOperation')}
      </Button>
    );
  } else if (headline.kind === 'updates') {
    primary = (
      <a className="btn btn--primary" href={hrefFor('updates')}>
        <span className="btn__icon" aria-hidden="true">
          <CircleArrowUp />
        </span>
        <span>{t('dashboard.action.viewUpdates')}</span>
      </a>
    );
  } else if (canCheck && (headline.kind === 'neverChecked' || headline.kind === 'failed' || headline.kind === 'stale')) {
    primary = checkButton('primary');
  } else if (canCheck && headline.kind !== 'prerequisiteMissing') {
    primary = checkButton('secondary');
  }

  // Due or overdue checks run within about a minute (background loop of the app).
  const nextCheckSoon = dashboard.nextCheckAt !== null && dashboard.nextCheckAt * 1000 <= now + 90_000;
  const nextCheck =
    settings.checkIntervalHours === 0
      ? t('dashboard.nextCheckOff')
      : dashboard.nextCheckAt === null
        ? t('dashboard.nextCheckUnknown')
        : nextCheckSoon
          ? t('dashboard.nextCheckSoon')
          : t('dashboard.nextCheck', { date: i18n.fmt.dateTime(dashboard.nextCheckAt) });

  const last = dashboard.lastActivity;
  const lastOutcome = last ? activityOutcome(last, i18n) : null;

  return (
    <div className="page">
      <PageHeader title={t('dashboard.title')} actions={primary} />

      <section className={`hero hero--${headline.tone}`} aria-labelledby="dashboard-status">
        <span className="hero__icon" aria-hidden="true">
          {HEADLINE_ICON[headline.kind]}
        </span>
        <div className="hero__text">
          <h2 className="hero__title" id="dashboard-status">
            {headlineTitle(headline, updates, operationKind, i18n)}
          </h2>
          <p className="hero__detail">{headlineDetail(headline, updates, now, i18n)}</p>
          {headline.kind === 'failed' && updates.error ? <p className="hero__detail">{t(`error.${updates.error.code}.title`)}</p> : null}
          <ul className="hero__facts">
            <li>{lastUpdateLine(dashboard, i18n)}</li>
            <li>{nextCheck}</li>
          </ul>
        </div>
      </section>

      {status.checkError ? (
        <Notice tone="danger" role="alert" title={`${t('dashboard.status.checkFailed')}: ${t(`error.${status.checkError.code}.title`)}`}>
          <p>{t(`error.${status.checkError.code}.text`)}</p>
        </Notice>
      ) : null}
      {dashboard.offlineUpdatePrepared ? (
        <Notice tone="info" title={t('dashboard.offlinePrepared')}>
          <p>{t('state.offlinePreparedText')}</p>
        </Notice>
      ) : null}
      {dashboard.rebootRecommended ? (
        <Notice tone="info" title={t('dashboard.reboot')}>
          <p className="status-line">
            <RotateCw aria-hidden="true" />
            {t('dashboard.rebootText')}
          </p>
        </Notice>
      ) : null}

      <div className="card-grid">
        <UpdatesCard updates={updates} headline={headline} />
        <SoftwareCard dashboard={dashboard} />
        <HealthCard dashboard={dashboard} />
        <SystemCard dashboard={dashboard} />
      </div>

      <Card
        title={t('dashboard.lastActivity')}
        icon={<Info />}
        footer={
          <a className="card-link" href={hrefFor('activity')}>
            {t('dashboard.lastActivityLink')} <ArrowRight aria-hidden="true" />
          </a>
        }
      >
        {last && lastOutcome ? (
          <p className="last-activity">
            <strong>{last.kind ? t(`operation.kind.${last.kind}`) : t('activity.kind.external')}</strong>
            <Badge tone={lastOutcome.tone}>{lastOutcome.text}</Badge>
            <TimeText seconds={last.endedAt ?? last.startedAt} />
          </p>
        ) : (
          <p className="muted">{t('dashboard.lastActivityNone')}</p>
        )}
      </Card>
    </div>
  );
}
