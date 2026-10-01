import { useState } from 'react';

import { normalizeError } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import { focusSoon } from '../../lib/focus';
import { warningPackages } from '../../lib/plan';
import { isPlanChanged, useOperations, type TrackedOperation } from '../../state/Operations';
import { useStatus } from '../../state/Status';
import { PlanChangedDialog } from '../dialogs/PlanChangedDialog';
import { OperationView } from './OperationView';

/** Plan the user confirmed; after a GUI restart the current upgrade plan when its digest matches. */
function confirmedPlanOf(tracked: TrackedOperation, currentUpgradePlan: TransactionPlan | null): TransactionPlan | null {
  if (tracked.request) return tracked.request.plan;
  const digest = tracked.operation?.confirmedDigest;
  return digest && currentUpgradePlan?.digest === digest ? currentUpgradePlan : null;
}

/** Global live view of the tracked package operation (shown above every page). */
export function OperationPanel() {
  const operations = useOperations();
  const status = useStatus();
  const [cancelBusy, setCancelBusy] = useState(false);
  const [cancelError, setCancelError] = useState<AppError | null>(null);
  const [reviewOpen, setReviewOpen] = useState(false);
  const { tracked } = operations;
  if (!tracked) return null;

  const confirmedPlan = confirmedPlanOf(tracked, status.updates?.plan ?? null);
  const operation = tracked.operation;
  const planReboot = confirmedPlan ? warningPackages(confirmedPlan, 'rebootRecommended').length > 0 : false;

  const cancel = async () => {
    setCancelBusy(true);
    setCancelError(null);
    try {
      await operations.cancel();
      // The cancel button disappears with the next state: keep the focus in the panel.
      focusSoon(() => document.getElementById('operation-panel-title'));
    } catch (error) {
      setCancelError(normalizeError(error));
    } finally {
      setCancelBusy(false);
    }
  };

  return (
    <>
      <OperationView
        tracked={tracked}
        expanded={operations.expanded}
        onToggleExpanded={() => operations.setExpanded(!operations.expanded)}
        onCancel={() => void cancel()}
        cancelBusy={cancelBusy}
        cancelError={cancelError}
        onDismiss={() => {
          operations.dismiss();
          focusSoon();
        }}
        onReviewPlan={() => setReviewOpen(true)}
        onRecheck={() => void status.checkUpdates()}
        recheckBusy={status.checking}
        onViewLog={operations.showLog}
        logRequest={operations.logRequest}
        confirmedPlan={confirmedPlan}
        rebootRecommended={planReboot || (status.dashboard?.rebootRecommended ?? false)}
        resultPending={operations.resultPending}
      />
      {reviewOpen && operation && isPlanChanged(operation) ? (
        <PlanChangedDialog
          kind={operation.kind}
          confirmed={confirmedPlan}
          actual={operation.actualPlan}
          onClose={() => setReviewOpen(false)}
          onConfirm={() => operations.restartWithPlan(operation.actualPlan)}
        />
      ) : null}
    </>
  );
}
