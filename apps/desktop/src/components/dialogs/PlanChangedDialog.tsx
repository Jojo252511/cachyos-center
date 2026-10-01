import { Play } from 'lucide-react';
import { useState } from 'react';

import { api } from '../../api/client';
import { normalizeError } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import type { OperationKind } from '../../bindings/OperationKind';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import { useI18n } from '../../i18n';
import { newsGate, requiresNewsAcknowledgement } from '../../lib/news';
import { diffPlans, isDiffEmpty } from '../../lib/plan';
import { useResource } from '../../state/useResource';
import { Button } from '../Button';
import { AppDialog } from '../Dialog';
import { ErrorPanel } from '../ErrorPanel';
import { Checkbox } from '../Form';
import { Notice } from '../Notice';
import { PlanDiffView } from '../plan/PlanDiffView';
import { PlanSummary } from '../plan/PlanSummary';
import { Spinner } from '../StateViews';

export interface PlanChangedDialogProps {
  kind: OperationKind;
  /** Plan the user confirmed (unknown after a GUI restart). */
  confirmed: TransactionPlan | null;
  actual: TransactionPlan;
  onClose(): void;
  /** Starts a new operation with `actual.digest`; rejects with `AppError`. */
  onConfirm(): Promise<void>;
}

/** Re-confirmation after `PLAN_CHANGED`: shows the difference and the new plan. */
export function PlanChangedDialog({ kind, confirmed, actual, onClose, onConfirm }: PlanChangedDialogProps) {
  const { t } = useI18n();
  const isUpgrade = kind === 'systemUpgrade';
  const news = useResource(isUpgrade ? 'news' : null, () => api.getNews(true, false));
  const gate = isUpgrade ? newsGate(news.data, news.error, news.loading) : null;
  const needsAck = gate !== null && requiresNewsAcknowledgement(gate);
  const [acknowledged, setAcknowledged] = useState(false);
  const [starting, setStarting] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const diff = confirmed ? diffPlans(confirmed, actual) : null;

  const confirm = async () => {
    setStarting(true);
    setError(null);
    try {
      await onConfirm();
      onClose();
    } catch (reason) {
      setError(normalizeError(reason));
    } finally {
      setStarting(false);
    }
  };

  return (
    <AppDialog
      open
      onOpenChange={(open) => (open ? undefined : onClose())}
      variant="transaction"
      locked={starting}
      title={t('planChanged.dialog.title')}
      description={t('planChanged.dialog.description')}
      footer={
        <>
          <Button onClick={onClose} disabled={starting}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="primary"
            icon={<Play />}
            busy={starting}
            disabled={starting || gate?.state === 'loading' || (needsAck && !acknowledged)}
            onClick={() => void confirm()}
          >
            {t('planChanged.confirm')}
          </Button>
        </>
      }
    >
      {diff && !isDiffEmpty(diff) ? <PlanDiffView diff={diff} confirmed={confirmed} /> : <Notice tone="info">{t('planChanged.noDiff')}</Notice>}
      <h3 className="dialog__section-title">{t('planChanged.newPlan')}</h3>
      <PlanSummary plan={actual} />
      <Notice tone="info">
        <p>{actual.source === 'isolatedCheckDb' ? t('plan.isolatedNote') : t('plan.systemDbNote')}</p>
      </Notice>
      {gate?.state === 'loading' ? <Spinner label={t('upgrade.dialog.newsLoading')} /> : null}
      {gate?.state === 'unread' ? <Notice tone="warning" title={t('upgrade.dialog.newsUnread', { count: gate.unreadCount })} /> : null}
      {gate?.state === 'unavailable' ? <Notice tone="warning" title={t('upgrade.dialog.newsUnavailable')} /> : null}
      {needsAck ? <Checkbox label={t('upgrade.dialog.newsAck')} checked={acknowledged} onChange={setAcknowledged} /> : null}
      <p className="muted">{t('plan.polkitNote')}</p>
      {error ? <ErrorPanel error={error} announce title={`${t('operation.startFailed')}: ${t(`error.${error.code}.title`)}`} /> : null}
    </AppDialog>
  );
}
