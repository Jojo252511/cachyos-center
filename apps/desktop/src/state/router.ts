import { useMemo, useSyncExternalStore } from 'react';

export const PAGES = ['overview', 'updates', 'software', 'system', 'activity', 'settings'] as const;
export type PageId = (typeof PAGES)[number];

export interface Route {
  page: PageId;
  /** Optional anchor inside the page, e.g. `#/system/health`. */
  section: string | null;
}

export function parseHash(hash: string): Route {
  const match = /^#\/?([a-z]+)?(?:\/([a-z-]+))?/.exec(hash);
  const page = match?.[1] ?? '';
  return {
    page: (PAGES as readonly string[]).includes(page) ? (page as PageId) : 'overview',
    section: match?.[2] ?? null,
  };
}

export function hrefFor(page: PageId, section?: string): string {
  return `#/${page}${section ? `/${section}` : ''}`;
}

export function navigate(page: PageId, section?: string): void {
  window.location.hash = hrefFor(page, section);
}

function subscribe(callback: () => void): () => void {
  window.addEventListener('hashchange', callback);
  return () => window.removeEventListener('hashchange', callback);
}

export function useRoute(): Route {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash, () => '');
  return useMemo(() => parseHash(hash), [hash]);
}
