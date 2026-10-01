import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';

import { normalizeError } from '../api/errors';
import type { AppError } from '../bindings/AppError';

export interface Resource<T> {
  /** Data of the current key (kept while reloading). */
  data: T | undefined;
  error: AppError | null;
  /** A request for the current key is in flight. */
  loading: boolean;
  reload(): void;
  /** Replaces the data of the current key (e.g. with a command result). */
  mutate(data: T): void;
}

interface Entry<T> {
  key: string;
  token: number;
  version: number;
  data: T | undefined;
  error: AppError | null;
}

/**
 * Loads data for `key` and reloads when the key changes, `version` changes
 * (e.g. after an event) or `reload()` is called. `key === null` disables
 * loading. Rejections are normalized into `AppError`; data of the same key
 * stays visible while reloading and on errors.
 */
export function useResource<T>(key: string | null, loader: () => Promise<T>, version = 0): Resource<T> {
  const loaderRef = useRef(loader);
  useLayoutEffect(() => {
    loaderRef.current = loader;
  });
  const [token, setToken] = useState(0);
  const [entry, setEntry] = useState<Entry<T> | null>(null);

  useEffect(() => {
    if (key === null) return;
    let active = true;
    loaderRef.current().then(
      (data) => {
        if (active) setEntry({ key, token, version, data, error: null });
      },
      (error: unknown) => {
        if (!active) return;
        setEntry((previous) => ({
          key,
          token,
          version,
          data: previous?.key === key ? previous.data : undefined,
          error: normalizeError(error),
        }));
      },
    );
    return () => {
      active = false;
    };
  }, [key, token, version]);

  const reload = useCallback(() => setToken((value) => value + 1), []);
  const mutate = useCallback(
    (data: T) => {
      if (key !== null) setEntry({ key, token, version, data, error: null });
    },
    [key, token, version],
  );

  const current = entry !== null && entry.key === key;
  return {
    data: current ? entry.data : undefined,
    error: current ? entry.error : null,
    loading: key !== null && !(current && entry.token === token && entry.version === version),
    reload,
    mutate,
  };
}
