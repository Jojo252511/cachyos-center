import { Check, Circle, LoaderCircle } from 'lucide-react';

import type { OperationKind } from '../../bindings/OperationKind';
import type { OperationState } from '../../bindings/OperationState';
import { useI18n } from '../../i18n';

type Phase = 'authorize' | 'prepare' | 'download' | 'install' | 'remove' | 'done';

export function phasesFor(kind: OperationKind): Phase[] {
  return kind === 'remove' ? ['authorize', 'prepare', 'remove', 'done'] : ['authorize', 'prepare', 'download', 'install', 'done'];
}

function phaseIndex(kind: OperationKind, state: OperationState): number {
  const phases = phasesFor(kind);
  switch (state) {
    case 'awaitingAuthorization':
    case 'idle':
    case 'ready':
    case 'checking':
      return 0;
    case 'preparing':
      return 1;
    case 'downloading':
      return phases.indexOf('download');
    case 'installing':
      return phases.indexOf(kind === 'remove' ? 'remove' : 'install');
    default:
      return phases.length - 1;
  }
}

/** Phases of a running operation; the current one is marked with icon and text. */
export function PhaseStepper({ kind, state }: { kind: OperationKind; state: OperationState }) {
  const { t } = useI18n();
  const phases = phasesFor(kind);
  const current = phaseIndex(kind, state);
  return (
    <ol className="stepper" aria-label={t('operation.phases')}>
      {phases.map((phase, index) => {
        const status = index < current ? 'done' : index === current ? 'current' : 'pending';
        return (
          <li key={phase} className={`stepper__step stepper__step--${status}`} aria-current={status === 'current' ? 'step' : undefined}>
            <span className="stepper__icon" aria-hidden="true">
              {status === 'done' ? <Check /> : status === 'current' ? <LoaderCircle className="spin" /> : <Circle />}
            </span>
            <span className="stepper__label">{t(`operation.phase.${phase}`)}</span>
            <span className="sr-only"> ({t(`operation.phaseStatus.${status}`)})</span>
          </li>
        );
      })}
    </ol>
  );
}
