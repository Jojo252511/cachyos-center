import { CircleAlert, Pause, Replace, RotateCw, ShieldAlert } from 'lucide-react';

import type { PlanWarning } from '../../bindings/PlanWarning';
import { useI18n, type I18n } from '../../i18n';

export function warningText(warning: PlanWarning, i18n: I18n): string {
  const { t, fmt } = i18n;
  switch (warning.kind) {
    case 'includesSystemUpgrade':
      return t('warning.includesSystemUpgrade', { count: warning.upgradeCount });
    case 'criticalPackages':
      return t('warning.criticalPackages', { packages: fmt.list(warning.packages) });
    case 'heldBackPackages':
      return t('warning.heldBackPackages', { packages: fmt.list(warning.packages) });
    case 'rebootRecommended':
      return t('warning.rebootRecommended', { packages: fmt.list(warning.packages) });
    case 'replacements':
      return t('warning.replacements', { packages: fmt.list(warning.packages) });
  }
}

function WarningIcon({ warning }: { warning: PlanWarning }) {
  switch (warning.kind) {
    case 'criticalPackages':
      return <ShieldAlert aria-hidden="true" />;
    case 'heldBackPackages':
      return <Pause aria-hidden="true" />;
    case 'rebootRecommended':
      return <RotateCw aria-hidden="true" />;
    case 'replacements':
      return <Replace aria-hidden="true" />;
    default:
      return <CircleAlert aria-hidden="true" />;
  }
}

/** Localized list of plan warnings. */
export function PlanWarnings({ warnings }: { warnings: readonly PlanWarning[] }) {
  const i18n = useI18n();
  if (warnings.length === 0) return null;
  return (
    <section className="plan-warnings" aria-label={i18n.t('plan.warnings')}>
      <ul className="icon-list">
        {warnings.map((warning, index) => (
          <li key={`${warning.kind}-${index}`} className={`icon-list__item icon-list__item--${warning.kind === 'criticalPackages' ? 'danger' : warning.kind === 'heldBackPackages' ? 'warning' : 'info'}`}>
            <WarningIcon warning={warning} />
            <span>{warningText(warning, i18n)}</span>
          </li>
        ))}
      </ul>
    </section>
  );
}
