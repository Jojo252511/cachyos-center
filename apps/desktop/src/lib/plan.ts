import type { PlanAction } from '../bindings/PlanAction';
import type { PlanDiff } from '../bindings/PlanDiff';
import type { PlanEntry } from '../bindings/PlanEntry';
import type { TransactionPlan } from '../bindings/TransactionPlan';

/** Display order of plan actions. */
export const PLAN_ACTIONS: readonly PlanAction[] = ['upgrade', 'install', 'downgrade', 'reinstall', 'remove'];

export function countByAction(plan: TransactionPlan): Record<PlanAction, number> {
  const counts: Record<PlanAction, number> = { install: 0, upgrade: 0, downgrade: 0, reinstall: 0, remove: 0 };
  for (const entry of plan.entries) counts[entry.action] += 1;
  return counts;
}

/** Sync repositories of the plan in order of first appearance. */
export function planRepositories(plan: TransactionPlan): string[] {
  const seen: string[] = [];
  for (const entry of plan.entries) {
    if (entry.repository && !seen.includes(entry.repository)) seen.push(entry.repository);
  }
  return seen;
}

const entryKey = (entry: PlanEntry) => `${entry.name}\u0000${entry.action}`;

/**
 * Difference between the confirmed and the actual plan, computed like
 * `TransactionPlan::diff` of the Rust core (key: name + action).
 */
export function diffPlans(confirmed: TransactionPlan, actual: TransactionPlan): PlanDiff {
  const before = new Map(confirmed.entries.map((entry) => [entryKey(entry), entry]));
  const after = new Map(actual.entries.map((entry) => [entryKey(entry), entry]));
  const diff: PlanDiff = { added: [], removed: [], changed: [] };
  for (const [key, entry] of after) {
    const old = before.get(key);
    if (!old) {
      diff.added.push(entry);
    } else if (old.repository !== entry.repository || old.oldVersion !== entry.oldVersion || old.newVersion !== entry.newVersion) {
      diff.changed.push(entry);
    }
  }
  for (const [key, entry] of before) {
    if (!after.has(key)) diff.removed.push(entry);
  }
  return diff;
}

export function isDiffEmpty(diff: PlanDiff): boolean {
  return diff.added.length === 0 && diff.removed.length === 0 && diff.changed.length === 0;
}

export function warningPackages(plan: TransactionPlan, kind: 'criticalPackages' | 'heldBackPackages' | 'rebootRecommended' | 'replacements'): string[] {
  return plan.warnings.flatMap((warning) => (warning.kind === kind ? warning.packages : []));
}

/** Number of additional upgrades pulled in by an install (`-Syu`). */
export function includedUpgrades(plan: TransactionPlan): number {
  const warning = plan.warnings.find((w) => w.kind === 'includesSystemUpgrade');
  if (warning?.kind === 'includesSystemUpgrade') return warning.upgradeCount;
  return plan.entries.filter((entry) => entry.action === 'upgrade' && !entry.requested).length;
}

/** Entry of a confirmed plan matched by name (for "old → new" display in diffs). */
export function findEntry(plan: TransactionPlan | null, name: string): PlanEntry | undefined {
  return plan?.entries.find((entry) => entry.name === name);
}
