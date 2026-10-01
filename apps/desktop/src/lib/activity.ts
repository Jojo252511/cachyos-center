import type { HistoryEntry } from '../bindings/HistoryEntry';
import type { I18n } from '../i18n';
import { isTerminalState } from '../state/Operations';
import type { Tone } from './headline';

/** Localized outcome of a history entry; unknown outcomes are said explicitly. */
export function activityOutcome(entry: HistoryEntry, { t }: I18n): { text: string; tone: Tone } {
  if (entry.outcomeUnknown) return { text: t('activity.outcome.unknown'), tone: 'warning' };
  if (entry.state) {
    const tone: Tone =
      entry.state === 'succeeded'
        ? 'success'
        : entry.state === 'failed' || entry.state === 'needsAttention'
          ? 'danger'
          : isTerminalState(entry.state)
            ? 'neutral'
            : 'info';
    return { text: t(`operation.state.${entry.state}`), tone };
  }
  switch (entry.logOutcome) {
    case 'completed':
      return { text: t('activity.outcome.completed'), tone: 'success' };
    case 'failed':
      return { text: t('activity.outcome.failed'), tone: 'danger' };
    case 'interrupted':
      return { text: t('activity.outcome.interrupted'), tone: 'warning' };
    default:
      return { text: t('activity.outcome.unknown'), tone: 'warning' };
  }
}
