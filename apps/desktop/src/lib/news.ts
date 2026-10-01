import type { AppError } from '../bindings/AppError';
import type { NewsStatus } from '../bindings/NewsStatus';

export type NewsGateState = 'loading' | 'clear' | 'unread' | 'unavailable';

export interface NewsGate {
  state: NewsGateState;
  unreadCount: number;
  /** Some feeds could not be fetched (or fetching is disabled). */
  incomplete: boolean;
}

/**
 * News check before a manual upgrade (concept §5.2): unread or unavailable
 * news require an explicit acknowledgement; uncertainty is never "passed".
 */
export function newsGate(news: NewsStatus | undefined, error: AppError | null, loading: boolean): NewsGate {
  if (!news) {
    if (loading && !error) return { state: 'loading', unreadCount: 0, incomplete: true };
    return { state: 'unavailable', unreadCount: 0, incomplete: true };
  }
  const incomplete = news.disabled || news.errors.length > 0 || news.fetchedAt === null || error !== null;
  if (news.unreadCount > 0) return { state: 'unread', unreadCount: news.unreadCount, incomplete };
  if (incomplete) return { state: 'unavailable', unreadCount: 0, incomplete };
  return { state: 'clear', unreadCount: 0, incomplete: false };
}

/** The confirm button of an upgrade needs the acknowledgement checkbox. */
export function requiresNewsAcknowledgement(gate: NewsGate): boolean {
  return gate.state === 'unread' || gate.state === 'unavailable';
}

/** Latest publication time of unread items (for `acknowledge_news`). */
export function latestUnread(news: NewsStatus): number | null {
  const unread = news.items.filter((item) => item.unread).map((item) => item.publishedAt);
  return unread.length > 0 ? Math.max(...unread) : null;
}
