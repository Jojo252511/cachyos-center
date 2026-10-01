import { ChevronDown, ChevronUp, CircleCheck, CircleX, ListChecks, LoaderCircle, OctagonAlert, RefreshCw, TriangleAlert, X } from 'lucide-react';
import type { ReactNode } from 'react';

import { FULL_UPGRADE_COMMAND } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import type { Operation } from '../../bindings/Operation';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import { useI18n, type I18n } from '../../i18n';
import { diffPlans, isDiffEmpty } from '../../lib/plan';
import { hrefFor } from '../../state/router';
import { canCancel, isPlanChanged, isTerminalState, type TrackedOperation } from '../../state/Operations';
import { Badge, type BadgeTone } from '../Badge';
import { Button } from '../Button';
import { ErrorPanel } from '../ErrorPanel';
import { Notice } from '../Notice';
import { PlanDiffView } from '../plan/PlanDiffView';
import { TerminalCommand } from '../TerminalCommand';
import { OperationLog } from './OperationLog';
import { PhaseStepper } from './PhaseStepper';

export interface OperationViewProps {
  tracked: TrackedOperation;
  expanded: boolean;
  onToggleExpanded(): void;
  onCancel(): void;
  cancelBusy: boolean;
  cancelError: AppError | null;
  onDismiss(): void;
  onReviewPlan(): void;
  onRecheck(): void;
  recheckBusy: boolean;
  onViewLog(): void;
  logRequest: number;
  /** Plan the user confirmed (for the `PLAN_CHANGED` difference). */
  confirmedPlan: TransactionPlan | null;
  rebootRecommended: boolean;
  resultPending: boolean;
}

export function operationTitle(operation: Operation | null, tracked: TrackedOperation, { t }: I18n): string {
  const kind = operation?.kind ?? (tracked.request?.kind === 'upgrade' ? 'systemUpgrade' : tracked.request?.kind);
  const name = operation?.packageTargets[0] ?? (tracked.request && tracked.request.kind !== 'upgrade' ? tracked.request.name : '');
  switch (kind) {
    case 'install':
      return t('operation.title.install', { name });
    case 'remove':
      return t('operation.title.remove', { name });
    case 'autoUpdatePrepare':
      return t('operation.title.autoUpdatePrepare');
    case 'updateCheck':
      return t('operation.title.updateCheck');
    default:
      return t('operation.title.systemUpgrade');
  }
}

export function stateLabel(operation: Operation, { t }: I18n): string {
  if (operation.kind === 'remove' && operation.state === 'installing') return t('operation.state.removing');
  return t(`operation.state.${operation.state}`);
}

function stateTone(operation: Operation): BadgeTone {
  if (operation.outcomeUnknown) return 'warning';
  switch (operation.state) {
    case 'succeeded':
      return 'success';
    case 'failed':
    case 'needsAttention':
      return 'danger';
    case 'cancelledBeforeCommit':
      return operation.error ? 'warning' : 'neutral';
    default:
      return 'info';
  }
}

function StateIcon({ operation }: { operation: Operation | null }) {
  if (!operation || !isTerminalState(operation.state)) return <LoaderCircle className="spin" aria-hidden="true" />;
  if (operation.outcomeUnknown) return <TriangleAlert aria-hidden="true" />;
  if (operation.state === 'succeeded') return <CircleCheck aria-hidden="true" />;
  if (operation.state === 'cancelledBeforeCommit') return <CircleX aria-hidden="true" />;
  return <OctagonAlert aria-hidden="true" />;
}

/** Progress of a running operation: only values reported by pacman, no estimated percentage. */
function ProgressInfo({ operation }: { operation: Operation }) {
  const { t } = useI18n();
  const { progress } = operation;
  const total = progress.packagesTotal;
  return (
    <div className="operation-progress">
      {progress.currentPackage ? <p>{t('operation.currentPackage', { name: progress.currentPackage })}</p> : null}
      {progress.step ? <p className="muted">{t(`operation.step.${progress.step}`)}</p> : null}
      {total !== null && total > 0 ? (
        <div className="operation-progress__count">
          <progress className="progress" value={progress.packagesDone} max={total} aria-label={t('operation.progressLabel')} />
          <span>{t('operation.progress', { done: progress.packagesDone, total })}</span>
        </div>
      ) : (
        <p className="muted">{t('operation.progressUnknown')}</p>
      )}
    </div>
  );
}

function ChangeSummary({ operation }: { operation: Operation }) {
  const { t } = useI18n();
  const { changes } = operation;
  const rows = (['installed', 'upgraded', 'downgraded', 'reinstalled', 'removed'] as const).filter((key) => changes[key] > 0);
  return (
    <div className="operation-changes">
      <h3 className="operation-changes__title">{t('operation.changes.title')}</h3>
      {rows.length > 0 ? (
        <ul className="operation-changes__list">
          {rows.map((key) => (
            <li key={key}>{t(`operation.changes.${key}`, { count: changes[key] })}</li>
          ))}
        </ul>
      ) : (
        <p className="muted">{t('operation.changes.none')}</p>
      )}
      {changes.packages.length > 0 ? (
        <details className="details">
          <summary>{t('operation.changes.packages')}</summary>
          <p className="mono">{changes.packages.join(', ')}</p>
        </details>
      ) : null}
    </div>
  );
}

function ResultSummary(props: OperationViewProps & { operation: Operation }) {
  const { t } = useI18n();
  const { operation, confirmedPlan, onReviewPlan, onRecheck, recheckBusy, onViewLog, rebootRecommended, resultPending } = props;
  const blocks: ReactNode[] = [];
  // Upgrades and installs start with pacman -Sy; removals never synchronize.
  const mayHaveSynced = !operation.commitStarted && (operation.kind === 'systemUpgrade' || operation.kind === 'install');

  if (isPlanChanged(operation)) {
    const diff = confirmedPlan ? diffPlans(confirmedPlan, operation.actualPlan) : null;
    blocks.push(
      <Notice
        key="plan-changed"
        tone="warning"
        title={t('operation.planChanged.title')}
        actions={
          <Button variant="primary" size="sm" icon={<ListChecks />} onClick={onReviewPlan}>
            {t('operation.planChanged.review')}
          </Button>
        }
      >
        <p>{t('operation.planChanged.text')}</p>
        {diff && !isDiffEmpty(diff) ? <PlanDiffView diff={diff} confirmed={confirmedPlan} /> : <p className="muted">{t('planChanged.noDiff')}</p>}
      </Notice>,
    );
  } else if (operation.outcomeUnknown) {
    blocks.push(
      <Notice key="unknown" tone="warning" title={t('operation.result.outcomeUnknown')}>
        <p>{t('operation.result.outcomeUnknownText')}</p>
        <TerminalCommand command={FULL_UPGRADE_COMMAND} hint={t('errorAction.terminalHint')} />
      </Notice>,
    );
  } else if (operation.state === 'succeeded') {
    blocks.push(
      <Notice key="success" tone="success" title={t('operation.result.succeeded')}>
        <ChangeSummary operation={operation} />
      </Notice>,
    );
  } else if (operation.state === 'cancelledBeforeCommit' && !operation.error) {
    blocks.push(
      <Notice key="cancelled" tone="info" title={t('operation.result.cancelled')}>
        <p>{t('operation.result.cancelledText')}</p>
        {mayHaveSynced ? <p>{t('operation.result.syncedHint')}</p> : null}
      </Notice>,
    );
  } else {
    const unclear = operation.state === 'needsAttention' || operation.commitStarted;
    blocks.push(
      <Notice
        key="failed"
        tone="danger"
        title={unclear ? t('operation.result.unclear') : operation.state === 'cancelledBeforeCommit' ? t('operation.result.cancelled') : t('operation.result.failedBeforeCommit')}
      >
        <p>{unclear ? t('operation.result.unclearText') : t('operation.result.failedBeforeCommitText')}</p>
        {!unclear && mayHaveSynced ? <p>{t('operation.result.syncedHint')}</p> : null}
        {unclear && operation.changes.packages.length > 0 ? <ChangeSummary operation={operation} /> : null}
      </Notice>,
    );
    if (operation.error) {
      blocks.push(
        <ErrorPanel
          key="error"
          error={operation.error}
          onRetry={onRecheck}
          retryBusy={recheckBusy}
          onViewLog={onViewLog}
          command={unclear ? FULL_UPGRADE_COMMAND : undefined}
        />,
      );
    } else if (unclear) {
      blocks.push(<TerminalCommand key="terminal" command={FULL_UPGRADE_COMMAND} hint={t('errorAction.terminalHint')} />);
    }
  }

  if (operation.snapshot) {
    blocks.push(
      operation.snapshot.created && operation.snapshot.number !== null ? (
        <p key="snapshot" className="status-line">
          <CircleCheck aria-hidden="true" />
          {t('operation.snapshotCreated', { number: operation.snapshot.number, config: operation.snapshot.snapperConfig })}
        </p>
      ) : (
        <p key="snapshot" className="status-line status-line--warning">
          <TriangleAlert aria-hidden="true" />
          {t('operation.snapshotFailed', { error: operation.snapshot.error ?? t('common.unknown') })}
        </p>
      ),
    );
  }
  if (operation.newPacnewFiles > 0) {
    blocks.push(
      <p key="pacnew" className="status-line status-line--warning">
        <TriangleAlert aria-hidden="true" />
        <span>
          {t('operation.pacnew', { count: operation.newPacnewFiles })} <a href={hrefFor('system', 'health')}>{t('operation.pacnewLink')}</a>
        </span>
      </p>,
    );
  }
  if (operation.state === 'succeeded' && rebootRecommended) {
    blocks.push(
      <p key="reboot" className="status-line status-line--info">
        <RefreshCw aria-hidden="true" />
        {t('operation.reboot')}
      </p>,
    );
  }
  if (resultPending) {
    blocks.push(
      <p key="blocked" className="muted">
        {t('operation.blockedUntilDismissed')}
      </p>,
    );
  }
  return <div className="operation-result">{blocks}</div>;
}

/** Live view of a package operation; used for upgrade, install and remove. */
export function OperationView(props: OperationViewProps) {
  const i18n = useI18n();
  const { t } = i18n;
  const { tracked, expanded, onToggleExpanded, onCancel, cancelBusy, cancelError, onDismiss, logRequest } = props;
  const operation = tracked.operation;
  const terminal = operation !== null && isTerminalState(operation.state);
  const title = operationTitle(operation, tracked, i18n);
  const label = operation ? stateLabel(operation, i18n) : t('operation.starting');

  return (
    <section className={`operation-panel ${terminal ? 'operation-panel--done' : ''}`} aria-label={t('operation.panelLabel')} id="operation-panel">
      <header className="operation-panel__header">
        <span className="operation-panel__icon">
          <StateIcon operation={operation} />
        </span>
        <h2 className="operation-panel__title" id="operation-panel-title" tabIndex={-1}>
          {title}
        </h2>
        <Badge tone={operation ? stateTone(operation) : 'info'}>{label}</Badge>
        <div className="operation-panel__actions">
          <Button size="sm" variant="ghost" icon={expanded ? <ChevronUp /> : <ChevronDown />} onClick={onToggleExpanded} aria-expanded={expanded} aria-controls="operation-panel-body">
            {expanded ? t('operation.collapse') : t('operation.expand')}
          </Button>
          {terminal ? (
            <Button size="sm" icon={<X />} onClick={onDismiss}>
              {t('operation.dismiss')}
            </Button>
          ) : null}
        </div>
      </header>
      <p className="sr-only" aria-live="polite">
        {`${title}: ${label}`}
      </p>
      {expanded ? (
        <div className="operation-panel__body" id="operation-panel-body">
          {operation && !terminal ? (
            <>
              <PhaseStepper kind={operation.kind} state={operation.state} />
              {operation.state === 'awaitingAuthorization' ? (
                <Notice tone="info" title={t('operation.state.awaitingAuthorization')}>
                  <p>{t('operation.awaitingAuthorizationHint')}</p>
                </Notice>
              ) : null}
              {operation.state === 'installing' ? (
                <Notice tone="warning" title={t('operation.noCancelDanger')}>
                  <p>{t('operation.noCancelDangerText')}</p>
                </Notice>
              ) : null}
              <ProgressInfo operation={operation} />
              {canCancel(operation.state) ? (
                <div className="operation-panel__cancel">
                  <Button variant="danger" size="sm" icon={<X />} busy={cancelBusy} onClick={onCancel}>
                    {cancelBusy ? t('operation.cancelling') : t('operation.cancel')}
                  </Button>
                  <span className="muted">{t('operation.cancelHint')}</span>
                </div>
              ) : null}
              {cancelError ? <ErrorPanel error={cancelError} announce title={t('operation.cancelFailed')} /> : null}
            </>
          ) : null}
          {!operation ? <p className="muted">{t('operation.starting')}</p> : null}
          {operation && terminal ? <ResultSummary {...props} operation={operation} /> : null}
          {tracked.pollError && !terminal ? (
            <p className="status-line status-line--warning" role="status">
              <TriangleAlert aria-hidden="true" />
              {t('operation.pollError')}
            </p>
          ) : null}
          <OperationLog lines={tracked.log} focusRequest={logRequest} />
        </div>
      ) : null}
    </section>
  );
}
