import type { TransactionPlan } from '../../bindings/TransactionPlan';
import type { PlanWarning } from '../../bindings/PlanWarning';
import { useI18n } from '../../i18n';
import { countByAction, PLAN_ACTIONS, planRepositories } from '../../lib/plan';
import { PlanTable } from './PlanTable';
import { PlanWarnings } from './PlanWarnings';

export interface PlanSummaryProps {
  plan: TransactionPlan;
  /** Warnings rendered elsewhere in the dialog (e.g. as a dedicated notice). */
  hideWarnings?: readonly PlanWarning['kind'][];
  /** Show the full package list expanded (removals must list every package). */
  packagesOpen?: boolean;
}

/**
 * Package count per action, total download, disk space, source repositories,
 * warnings and the full package list of a plan.
 */
export function PlanSummary({ plan, hideWarnings = [], packagesOpen = false }: PlanSummaryProps) {
  const { t, fmt } = useI18n();
  const counts = countByAction(plan);
  const repositories = planRepositories(plan);
  const warnings = plan.warnings.filter((warning) => !hideWarnings.includes(warning.kind));
  return (
    <div className="plan-summary">
      <h3 className="sr-only">{t('plan.summary')}</h3>
      <dl className="stat-grid">
        {PLAN_ACTIONS.filter((action) => counts[action] > 0).map((action) => (
          <div className="stat" key={action}>
            <dt className="stat__label">{t(`plan.count.${action}`)}</dt>
            <dd className="stat__value">{fmt.number(counts[action])}</dd>
          </div>
        ))}
        <div className="stat">
          <dt className="stat__label">{t('plan.total')}</dt>
          <dd className="stat__value">{fmt.number(plan.entries.length)}</dd>
        </div>
        <div className="stat">
          <dt className="stat__label">{t('plan.download')}</dt>
          <dd className="stat__value">{plan.downloadSize > 0 ? fmt.bytes(plan.downloadSize) : t('plan.downloadNone')}</dd>
        </div>
        <div className="stat">
          <dt className="stat__label">{t('plan.sizeDelta')}</dt>
          <dd className="stat__value">
            {plan.installSizeDelta < 0
              ? t('plan.sizeFrees', { size: fmt.bytes(-plan.installSizeDelta) })
              : t('plan.sizeNeeds', { size: fmt.bytes(plan.installSizeDelta) })}
          </dd>
        </div>
      </dl>
      <p className="plan-summary__sources">
        <span className="plan-summary__label">{t('plan.sources')}:</span>{' '}
        {repositories.length > 0 ? t('plan.sourcesValue', { list: repositories.join(', ') }) : t('plan.sourcesLocal')}
      </p>
      <PlanWarnings warnings={warnings} />
      <details className="details" open={packagesOpen}>
        <summary>{t('plan.packagesToggle', { count: plan.entries.length })}</summary>
        <PlanTable entries={plan.entries} />
      </details>
    </div>
  );
}
