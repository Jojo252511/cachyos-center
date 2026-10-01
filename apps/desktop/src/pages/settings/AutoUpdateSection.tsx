import { CalendarClock, Check, Newspaper } from 'lucide-react';
import { useId, useState } from 'react';

import { api } from '../../api/client';
import { DOC_URLS, normalizeError } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import type { AutoUpdateConfig } from '../../bindings/AutoUpdateConfig';
import type { AutoUpdatePolicy } from '../../bindings/AutoUpdatePolicy';
import type { AutoUpdateStatus } from '../../bindings/AutoUpdateStatus';
import type { Operation } from '../../bindings/Operation';
import type { Weekday } from '../../bindings/Weekday';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { Card } from '../../components/Card';
import { ErrorPanel } from '../../components/ErrorPanel';
import { ExternalLink } from '../../components/ExternalLink';
import { Checkbox, RadioCards } from '../../components/Form';
import { KeyValue, KeyValueList } from '../../components/KeyValue';
import { Notice } from '../../components/Notice';
import { ErrorState, LoadingState } from '../../components/StateViews';
import { useI18n, type MessageKey, type Translate } from '../../i18n';
import { useResource } from '../../state/useResource';

export const WEEKDAYS: readonly Weekday[] = ['mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun'];
const TIME_PATTERN = /^([01]\d|2[0-3]):[0-5]\d$/;

const BLOCKER_KEYS: Record<string, MessageKey> = {
  helperMissing: 'settings.auto.blocker.helperMissing',
  experimentalLocked: 'settings.auto.blocker.experimentalLocked',
  pacmanOfflineMissing: 'settings.auto.blocker.pacmanOfflineMissing',
  offlineConfigUnverifiable: 'settings.auto.blocker.offlineConfigUnverifiable',
  externalPrepareTimer: 'settings.auto.blocker.externalPrepareTimer',
  offlineConfHoldsPackages: 'settings.auto.blocker.offlineConfHoldsPackages',
};

/**
 * Localized result of the last timer run. The helper's summary is English and
 * only shown as technical detail.
 */
export function timerResultText(result: Operation, t: Translate): string {
  if (result.state === 'succeeded') {
    const count = result.progress.packagesTotal;
    if (count === 0) return t('settings.auto.result.upToDate');
    if (count !== null) {
      return result.kind === 'autoUpdatePrepare'
        ? t('settings.auto.result.prepared', { count })
        : t('settings.auto.result.updates', { count });
    }
  }
  const state = t(`operation.state.${result.state}`);
  return result.error ? `${state} – ${t(`error.${result.error.code}.title`)}` : state;
}

interface Draft {
  policy: AutoUpdatePolicy;
  weekdays: Weekday[];
  time: string;
  requireSnapshot: boolean;
}

function toDraft(config: AutoUpdateConfig): Draft {
  return { policy: config.policy, weekdays: [...config.window.weekdays], time: config.window.time, requireSnapshot: config.requireSnapshot };
}

function sameDraft(a: Draft, b: Draft): boolean {
  const days = (d: Draft) => [...d.weekdays].sort().join(',');
  return a.policy === b.policy && a.time === b.time && a.requireSnapshot === b.requireSnapshot && days(a) === days(b);
}

/** Policy switch, update window and status of the automatic updates. */
export function AutoUpdateSection() {
  const { t, fmt } = useI18n();
  const status = useResource('auto-update', () => api.getAutoUpdateStatus());
  const [draft, setDraft] = useState<Draft | null>(null);
  const [applying, setApplying] = useState(false);
  const [applied, setApplied] = useState(false);
  const [applyError, setApplyError] = useState<AppError | null>(null);
  const [ackBusy, setAckBusy] = useState(false);
  const [ackDone, setAckDone] = useState<number | null>(null);
  const [ackError, setAckError] = useState<AppError | null>(null);
  const timeId = useId();
  const data = status.data;

  if (status.error && !data) return <Card title={t('settings.auto')} icon={<CalendarClock />}><ErrorState error={status.error} onRetry={status.reload} /></Card>;
  if (!data) return <Card title={t('settings.auto')} icon={<CalendarClock />}><LoadingState /></Card>;

  const saved = toDraft(data.config);
  const current = draft ?? saved;
  const dirty = !sameDraft(current, saved);
  const timeValid = TIME_PATTERN.test(current.time);
  const weekdaysValid = current.weekdays.length > 0;
  const blockers = data.prepareModeBlockers;
  const update = (patch: Partial<Draft>) => {
    setApplied(false);
    setDraft({ ...current, ...patch });
  };

  const applyConfig = async (config: AutoUpdateConfig): Promise<AutoUpdateStatus> => {
    const result = await api.setAutoUpdatePolicy(config);
    status.mutate(result);
    return result;
  };

  const apply = async () => {
    setApplying(true);
    setApplyError(null);
    setApplied(false);
    try {
      await applyConfig({
        policy: current.policy,
        window: { weekdays: WEEKDAYS.filter((day) => current.weekdays.includes(day)), time: current.time },
        requireSnapshot: current.requireSnapshot,
        newsAcknowledgedUntil: data.config.newsAcknowledgedUntil,
      });
      setDraft(null);
      setApplied(true);
    } catch (error) {
      setApplyError(normalizeError(error));
    } finally {
      setApplying(false);
    }
  };

  // The scheduled preflight only knows the system-wide acknowledgement of the policy file.
  const acknowledgeNews = async () => {
    setAckBusy(true);
    setAckError(null);
    setAckDone(null);
    const until = Math.floor(Date.now() / 1000);
    try {
      await applyConfig({ ...data.config, newsAcknowledgedUntil: until });
      setAckDone(until);
    } catch (error) {
      setAckError(normalizeError(error));
    } finally {
      setAckBusy(false);
    }
  };

  return (
    <Card title={t('settings.auto')} icon={<CalendarClock />}>
      {data.configError ? (
        <Notice tone="warning" title={t('settings.auto.configError')}>
          <p className="mono">{data.configError}</p>
        </Notice>
      ) : null}
      {!data.helperAvailable ? <Notice tone="warning" title={t('state.helperMissingTitle')}>{t('settings.auto.helperMissing')}</Notice> : null}

      <RadioCards<AutoUpdatePolicy>
        label={t('settings.auto.policy')}
        value={current.policy}
        onValueChange={(policy) => update({ policy })}
        disabled={!data.helperAvailable}
        options={[
          { value: 'off', label: t('settings.auto.off'), hint: t('settings.auto.offHint') },
          { value: 'notifyOnly', label: t('settings.auto.notify'), hint: t('settings.auto.notifyHint') },
          {
            value: 'prepareForNextReboot',
            label: t('settings.auto.prepare'),
            badge: <Badge tone="warning">{t('settings.auto.inDevelopment')}</Badge>,
            disabled: blockers.length > 0,
          },
        ]}
        after={{
          prepareForNextReboot: (
            <div className="prepare-explanation">
              <p>{t('settings.auto.prepareExplanation')}</p>
              {blockers.length > 0 ? (
                <div className="blocker-list">
                  <p className="blocker-list__title">{t('settings.auto.blockersTitle')}</p>
                  <ul>
                    {blockers.map((blocker) => (
                      <li key={blocker}>{BLOCKER_KEYS[blocker] ? t(BLOCKER_KEYS[blocker]) : t('settings.auto.blocker.unknown', { id: blocker })}</li>
                    ))}
                  </ul>
                  <p>{t('settings.auto.cachyosWorkflow')}</p>
                  <p>
                    <ExternalLink url={DOC_URLS.cachyosPostInstall}>{t('settings.auto.cachyosWorkflowLink')}</ExternalLink>
                  </p>
                </div>
              ) : null}
            </div>
          ),
        }}
      />

      <fieldset className="fieldset" disabled={current.policy === 'off' || !data.helperAvailable}>
        <legend className="field__label">{t('settings.auto.window')}</legend>
        <div className="weekday-picker" role="group" aria-label={t('settings.auto.weekdays')}>
          {WEEKDAYS.map((day) => (
            <label key={day} className="weekday" title={t(`weekday.long.${day}`)}>
              <input
                type="checkbox"
                checked={current.weekdays.includes(day)}
                onChange={(event) =>
                  update({ weekdays: event.target.checked ? [...current.weekdays, day] : current.weekdays.filter((d) => d !== day) })
                }
                aria-label={t(`weekday.long.${day}`)}
              />
              <span aria-hidden="true">{t(`weekday.short.${day}`)}</span>
            </label>
          ))}
        </div>
        {!weekdaysValid ? <p className="field__error">{t('settings.auto.weekdayRequired')}</p> : null}
        <div className="field field--inline">
          <label className="field__label" htmlFor={timeId}>
            {t('settings.auto.time')}
          </label>
          <input
            id={timeId}
            type="time"
            className="input input--time"
            value={current.time}
            onChange={(event) => update({ time: event.target.value })}
            aria-invalid={!timeValid}
          />
        </div>
        {!timeValid ? <p className="field__error">{t('settings.auto.timeInvalid')}</p> : null}
        <Checkbox
          label={t('settings.auto.requireSnapshot')}
          hint={t('settings.auto.requireSnapshotHint')}
          checked={current.requireSnapshot}
          onChange={(requireSnapshot) => update({ requireSnapshot })}
        />
      </fieldset>

      <div className="apply-row">
        <Button variant="primary" icon={<Check />} busy={applying} disabled={!dirty || !timeValid || !weekdaysValid || !data.helperAvailable} onClick={() => void apply()}>
          {applying ? t('settings.auto.applying') : t('settings.auto.apply')}
        </Button>
        <span role="status" className="apply-row__status">
          {applied ? t('settings.auto.applied') : dirty ? t('settings.auto.unsaved') : ''}
        </span>
      </div>
      <p className="field__hint">{t('settings.auto.applyHint')}</p>
      {applyError ? <ErrorPanel error={applyError} announce onRetry={() => void apply()} /> : null}

      <h3 className="card__subtitle">{t('settings.auto.status')}</h3>
      <KeyValueList>
        <KeyValue label={t('settings.auto.timer')}>{data.timerEnabled ? t('common.active') : t('common.inactive')}</KeyValue>
        <KeyValue label={t('settings.auto.nextRun')}>{data.nextRun !== null ? fmt.dateTime(data.nextRun) : '–'}</KeyValue>
        <KeyValue label={t('settings.auto.lastRun')}>{data.lastRun !== null ? fmt.dateTime(data.lastRun) : t('common.never')}</KeyValue>
        <KeyValue label={t('settings.auto.lastResult')}>
          {data.lastResult ? (
            <>
              {timerResultText(data.lastResult, t)}
              {data.lastResult.summary ? (
                <span className="muted block break">{t('common.technicalDetail', { detail: data.lastResult.summary })}</span>
              ) : null}
            </>
          ) : (
            '–'
          )}
        </KeyValue>
        <KeyValue label={t('settings.auto.prepared')}>{data.preparedForNextReboot ? t('common.yes') : t('common.no')}</KeyValue>
        <KeyValue label={t('settings.auto.external')}>
          {data.externalUpdaters.length === 0 ? (
            t('settings.auto.noExternal')
          ) : (
            <ul className="plain-list">
              {data.externalUpdaters.map((updater) => (
                <li key={`${updater.scope}-${updater.name}`}>
                  <code>{updater.name}</code> <Badge tone="info">{t('settings.auto.externalManaged')}</Badge>{' '}
                  <span className="muted">{updater.active ? t('common.active') : t('common.inactive')}</span>
                </li>
              ))}
            </ul>
          )}
        </KeyValue>
        {data.config.policy !== 'off' ? (
          <KeyValue label={t('settings.auto.newsAck')}>
            {data.config.newsAcknowledgedUntil !== null ? fmt.dateTime(data.config.newsAcknowledgedUntil) : t('settings.auto.newsAckNever')}
          </KeyValue>
        ) : null}
      </KeyValueList>

      {data.config.policy !== 'off' ? (
        <div className="news-ack">
          <Button icon={<Newspaper />} busy={ackBusy} disabled={!data.helperAvailable} onClick={() => void acknowledgeNews()}>
            {t('settings.auto.newsAckButton')}
          </Button>
          <p className="field__hint">{t('settings.auto.newsAckHint')}</p>
          <p role="status" className="apply-row__status">
            {ackDone !== null ? t('settings.auto.newsAckDone', { date: fmt.dateTime(ackDone) }) : ''}
          </p>
          {ackError ? <ErrorPanel error={ackError} announce /> : null}
        </div>
      ) : null}
    </Card>
  );
}
