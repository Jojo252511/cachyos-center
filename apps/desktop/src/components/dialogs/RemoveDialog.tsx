import { Trash } from 'lucide-react';
import { useState } from 'react';

import { api } from '../../api/client';
import { normalizeError, parseDependentPackages } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import { useI18n } from '../../i18n';
import { warningPackages } from '../../lib/plan';
import { usePackageActionGuard } from '../../state/guards';
import { useOperations } from '../../state/Operations';
import { useResource } from '../../state/useResource';
import { Button } from '../Button';
import { AppDialog } from '../Dialog';
import { ErrorPanel } from '../ErrorPanel';
import { Checkbox } from '../Form';
import { Notice } from '../Notice';
import { PlanSummary } from '../plan/PlanSummary';
import { GuardNotice } from '../StatusNotices';
import { LoadingState } from '../StateViews';

/**
 * Conservative removal of a repository package (`pacman -R`, optionally
 * `-Rs`). No `-Rdd`, `--nodeps` or cascade options exist.
 */
export function RemoveDialog({ name, critical = false, onClose }: { name: string; critical?: boolean; onClose(): void }) {
  const { t, fmt } = useI18n();
  const operations = useOperations();
  const guard = usePackageActionGuard();
  const [recursive, setRecursive] = useState(false);
  const [criticalAck, setCriticalAck] = useState(false);
  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState<AppError | null>(null);
  const plan = useResource(`remove:${name}:${recursive ? 'recursive' : 'plain'}`, () => api.planRemove(name, recursive));

  const dependencyProblem = plan.error?.code === 'DEPENDENCY_PROBLEM' ? plan.error : null;
  const dependents = dependencyProblem ? parseDependentPackages(dependencyProblem.detail) : [];
  const criticalPackages = plan.data ? warningPackages(plan.data, 'criticalPackages') : [];
  const criticalList = criticalPackages.length > 0 ? criticalPackages : critical ? [name] : [];
  const needsCriticalAck = criticalList.length > 0;
  const ready = plan.data !== undefined && !plan.loading && !plan.error;

  const toggleRecursive = (value: boolean) => {
    setRecursive(value);
    setCriticalAck(false);
  };

  const confirm = async () => {
    if (!plan.data) return;
    setStarting(true);
    setStartError(null);
    try {
      await operations.start({ kind: 'remove', name, recursive, plan: plan.data });
      onClose();
    } catch (reason) {
      setStartError(normalizeError(reason));
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
      title={t('remove.dialog.title', { name })}
      description={t('remove.dialog.description')}
      footer={
        <>
          <Button onClick={onClose} disabled={starting}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="danger"
            icon={<Trash />}
            busy={starting}
            disabled={!ready || guard.reason !== null || (needsCriticalAck && !criticalAck)}
            onClick={() => void confirm()}
          >
            {t('remove.dialog.confirm')}
          </Button>
        </>
      }
    >
      <Checkbox label={t('remove.dialog.recursive')} hint={t('remove.dialog.recursiveHint')} checked={recursive} onChange={toggleRecursive} disabled={starting} />

      {dependencyProblem ? (
        <Notice tone="danger" title={t('remove.dialog.dependencyTitle')} role="alert">
          <p>{t('remove.dialog.dependencyText', { name })}</p>
          {dependents.length > 0 ? (
            <>
              <p className="notice__subtitle">{t('remove.dialog.dependents')}</p>
              <ul className="package-chips">
                {dependents.map((dependent) => (
                  <li key={dependent} className="mono">
                    {dependent}
                  </li>
                ))}
              </ul>
            </>
          ) : null}
          <details className="details">
            <summary>{t('common.technicalDetails')}</summary>
            <pre className="mono-block">{[dependencyProblem.message, dependencyProblem.detail].filter(Boolean).join('\n')}</pre>
          </details>
        </Notice>
      ) : null}
      {plan.error && !dependencyProblem ? <ErrorPanel error={plan.error} onRetry={plan.reload} /> : null}
      {plan.loading && !plan.error ? <LoadingState label={t('remove.dialog.planning')} /> : null}

      {ready && plan.data ? (
        <>
          <h3 className="dialog__section-title">{t('remove.dialog.packagesTitle', { count: plan.data.entries.length })}</h3>
          <PlanSummary plan={plan.data} packagesOpen hideWarnings={['criticalPackages']} />
          {needsCriticalAck ? (
            <Notice tone="danger" title={t('remove.dialog.criticalTitle')}>
              <p>{t('remove.dialog.criticalText', { packages: fmt.list(criticalList) })}</p>
              <Checkbox label={t('remove.dialog.criticalAck')} checked={criticalAck} onChange={setCriticalAck} />
            </Notice>
          ) : null}
          <Notice tone="info">
            <p>{t('remove.dialog.userData')}</p>
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
