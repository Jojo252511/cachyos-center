import type { LockStatus } from '../bindings/LockStatus';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';

export type HeadlineKind =
  | 'operation'
  | 'unsupported'
  | 'locked'
  | 'neverChecked'
  | 'failed'
  | 'prerequisiteMissing'
  | 'stale'
  | 'updates'
  | 'currentHeldBack'
  | 'current';

export type Tone = 'success' | 'accent' | 'warning' | 'danger' | 'info' | 'neutral';

export interface Headline {
  kind: HeadlineKind;
  tone: Tone;
  /** Number of updates; only set when the count is reliable (fresh check). */
  count: number | null;
}

/**
 * Top status line of the dashboard. A count is only reported for a fresh
 * check: `neverChecked`, `failed`, `unsupported` and `prerequisiteMissing`
 * never turn into "0 updates", `stale` says that the data is outdated.
 */
export function dashboardHeadline(input: { updates: UpdateCheckResult; lock: LockStatus; operationActive: boolean }): Headline {
  const { updates, lock, operationActive } = input;
  if (operationActive) return { kind: 'operation', tone: 'info', count: null };
  if (updates.status === 'unsupported') return { kind: 'unsupported', tone: 'danger', count: null };
  if (lock.state === 'locked') return { kind: 'locked', tone: 'warning', count: null };
  switch (updates.status) {
    case 'neverChecked':
      return { kind: 'neverChecked', tone: 'neutral', count: null };
    case 'failed':
      return { kind: 'failed', tone: 'danger', count: null };
    case 'prerequisiteMissing':
      return { kind: 'prerequisiteMissing', tone: 'warning', count: null };
    case 'stale':
      return { kind: 'stale', tone: 'warning', count: null };
    case 'fresh':
      if (updates.updates.length > 0) return { kind: 'updates', tone: 'accent', count: updates.updates.length };
      if (updates.heldBack.length > 0) return { kind: 'currentHeldBack', tone: 'warning', count: 0 };
      return { kind: 'current', tone: 'success', count: 0 };
  }
}
