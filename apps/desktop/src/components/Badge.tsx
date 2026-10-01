import { CircleAlert, CircleCheck, CircleQuestionMark, Info, OctagonAlert, TriangleAlert } from 'lucide-react';
import type { ReactNode } from 'react';

import type { Severity } from '../bindings/Severity';
import { useI18n } from '../i18n';
import type { Tone } from '../lib/headline';

export type BadgeTone = Tone;

export function Badge({ tone = 'neutral', icon, children, title }: { tone?: BadgeTone; icon?: ReactNode; children: ReactNode; title?: string }) {
  return (
    <span className={`badge badge--${tone}`} title={title}>
      {icon ? <span className="badge__icon" aria-hidden="true">{icon}</span> : null}
      {children}
    </span>
  );
}

const SEVERITY_TONE: Record<Severity, BadgeTone> = { info: 'info', warning: 'warning', critical: 'danger' };

export function SeverityIcon({ severity }: { severity: Severity }) {
  if (severity === 'critical') return <OctagonAlert aria-hidden="true" />;
  if (severity === 'warning') return <TriangleAlert aria-hidden="true" />;
  return <Info aria-hidden="true" />;
}

/** Severity as icon plus text (never color only). */
export function SeverityBadge({ severity }: { severity: Severity }) {
  const { t } = useI18n();
  return (
    <Badge tone={SEVERITY_TONE[severity]} icon={<SeverityIcon severity={severity} />}>
      {t(`severity.${severity}`)}
    </Badge>
  );
}

export type CheckState = 'ok' | 'warning' | 'blocked' | 'unknown' | 'info';

const CHECK_TONE: Record<CheckState, BadgeTone> = { ok: 'success', warning: 'warning', blocked: 'danger', unknown: 'neutral', info: 'info' };

export function CheckStateIcon({ state }: { state: CheckState }) {
  switch (state) {
    case 'ok':
      return <CircleCheck aria-hidden="true" />;
    case 'warning':
      return <TriangleAlert aria-hidden="true" />;
    case 'blocked':
      return <CircleAlert aria-hidden="true" />;
    case 'info':
      return <Info aria-hidden="true" />;
    default:
      return <CircleQuestionMark aria-hidden="true" />;
  }
}

export function CheckStateBadge({ state }: { state: CheckState }) {
  const { t } = useI18n();
  return (
    <Badge tone={CHECK_TONE[state]} icon={<CheckStateIcon state={state} />}>
      {t(`checkState.${state}`)}
    </Badge>
  );
}
