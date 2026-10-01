import { Play } from 'lucide-react';
import { useState } from 'react';

import { normalizeError } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import type { SnapshotSupport } from '../../bindings/SnapshotSupport';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import type { UpdateCandidate } from '../../bindings/UpdateCandidate';
import { useI18n } from '../../i18n';
import { requiresNewsAcknowledgement, type NewsGate } from '../../lib/news';
import { Button } from '../Button';
import { AppDialog } from '../Dialog';
import { ErrorPanel } from '../ErrorPanel';
import { Checkbox } from '../Form';
import { Notice } from '../Notice';
import { PlanSummary } from '../plan/PlanSummary';
import { GuardNotice } from '../StatusNotices';
import { Spinner } from '../StateViews';

export interface UpgradeDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  plan: TransactionPlan;
  heldBack: readonly UpdateCandidate[];
  news: NewsGate;
  snapshot: SnapshotSupport | null;
  /** Reason why the upgrade cannot start (lock, running operation, …). */
  blockedReason?: string | null;
  /** Starts the operation; rejects with `AppError`. */
  onConfirm(createSnapshot: boolean): Promise<void>;
}

/**
 * Confirmation of a full system upgrade: counts per action, download,
 * repositories, warnings and the news acknowledgement gate.
 */
export function UpgradeDialog({ open, onOpenChange, plan, heldBack, news, snapshot, blockedReason, onConfirm }: UpgradeDialogProps) {
  const { t } = useI18n();
  const [createSnapshot, setCreateSnapshot] = useState(false);
  const [acknowledged, setAcknowledged] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<AppError | null>(null);

  const needsAck = requiresNewsAcknowledgement(news);
  const canConfirm = !starting && !blockedReason && news.state !== 'loading' && (!needsAck || acknowledged) && plan.entries.length > 0;

  const confirm = async () => {
    setStarting(true);
    setError(null);
    try {
      await onConfirm(createSnapshot);
      onOpenChange(false);
    } catch (reason) {
      setError(normalizeError(reason));
    } finally {
      setStarting(false);
    }
  };

  return (
    <AppDialog
      open={open}
      onOpenChange={onOpenChange}
      variant="transaction"
      locked={starting}
      title={t('upgrade.dialog.title')}
      description={heldBack.length > 0 ? t('upgrade.dialog.descriptionHeldBack') : t('upgrade.dialog.description')}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)} disabled={starting}>
            {t('common.cancel')}
          </Button>
          <Button variant="primary" icon={<Play />} busy={starting} disabled={!canConfirm} onClick={() => void confirm()}>
            {t('upgrade.dialog.confirm')}
          </Button>
        </>
      }
    >
      <PlanSummary plan={plan} />
      <Notice tone="info">
        <p>{plan.source === 'isolatedCheckDb' ? t('plan.isolatedNote') : t('plan.systemDbNote')}</p>
      </Notice>

      {snapshot?.canRequestSnapshot ? (
        <Checkbox
          label={t('upgrade.dialog.snapshot')}
          hint={t('upgrade.dialog.snapshotHint', { config: snapshot.rootConfig ?? 'root' })}
          checked={createSnapshot}
          onChange={setCreateSnapshot}
        />
      ) : null}

      {news.state === 'loading' ? <Spinner label={t('upgrade.dialog.newsLoading')} /> : null}
      {news.state === 'unread' ? (
        <Notice tone="warning" title={t('upgrade.dialog.newsUnread', { count: news.unreadCount })}>
          {news.incomplete ? <p>{t('upgrade.dialog.newsUnavailable')}</p> : null}
        </Notice>
      ) : null}
      {news.state === 'unavailable' ? <Notice tone="warning" title={t('upgrade.dialog.newsUnavailable')} /> : null}
      {needsAck ? (
        <Checkbox label={t('upgrade.dialog.newsAck')} checked={acknowledged} onChange={setAcknowledged} />
      ) : null}

      <p className="muted">{t('plan.polkitNote')}</p>
      <GuardNotice message={blockedReason ?? null} />
      {error ? <ErrorPanel error={error} announce title={`${t('operation.startFailed')}: ${t(`error.${error.code}.title`)}`} /> : null}
    </AppDialog>
  );
}
