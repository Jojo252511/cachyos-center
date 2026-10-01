import { Minus, Plus, RefreshCw } from 'lucide-react';
import type { ReactNode } from 'react';

import type { PlanDiff } from '../../bindings/PlanDiff';
import type { PlanEntry } from '../../bindings/PlanEntry';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import { useI18n } from '../../i18n';
import { findEntry } from '../../lib/plan';
import { versionText } from './PlanTable';

function DiffGroup({ title, icon, tone, entries, describe }: { title: string; icon: ReactNode; tone: string; entries: readonly PlanEntry[]; describe(entry: PlanEntry): string }) {
  if (entries.length === 0) return null;
  return (
    <div className={`diff-group diff-group--${tone}`}>
      <h4 className="diff-group__title">
        <span aria-hidden="true">{icon}</span>
        {title} ({entries.length})
      </h4>
      <ul className="diff-group__list">
        {entries.map((entry) => (
          <li key={`${entry.action}-${entry.name}`}>
            <span className="mono">{entry.name}</span> <span className="muted">{describe(entry)}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Difference between the confirmed plan and the actual plan (`PLAN_CHANGED`). */
export function PlanDiffView({ diff, confirmed }: { diff: PlanDiff; confirmed: TransactionPlan | null }) {
  const { t } = useI18n();
  const action = (entry: PlanEntry) => t(`plan.action.${entry.action}`);
  return (
    <div className="plan-diff">
      <DiffGroup title={t('planChanged.added')} icon={<Plus />} tone="added" entries={diff.added} describe={(e) => `${action(e)} · ${versionText(e)}`} />
      <DiffGroup
        title={t('planChanged.changed')}
        icon={<RefreshCw />}
        tone="changed"
        entries={diff.changed}
        describe={(e) => {
          const before = findEntry(confirmed, e.name);
          if (!before) return versionText(e);
          if (before.newVersion !== e.newVersion) return t('planChanged.versionChange', { from: before.newVersion ?? '–', to: e.newVersion ?? '–' });
          if (before.repository !== e.repository) return t('planChanged.versionChange', { from: before.repository ?? '–', to: e.repository ?? '–' });
          return t('planChanged.versionChange', { from: before.oldVersion ?? '–', to: e.oldVersion ?? '–' });
        }}
      />
      <DiffGroup title={t('planChanged.removed')} icon={<Minus />} tone="removed" entries={diff.removed} describe={(e) => `${action(e)} · ${versionText(e)}`} />
    </div>
  );
}
