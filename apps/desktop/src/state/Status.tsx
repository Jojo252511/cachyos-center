import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { api } from '../api/client';
import { normalizeError } from '../api/errors';
import type { AppError } from '../bindings/AppError';
import type { Dashboard } from '../bindings/Dashboard';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';

/** Background refresh of the dashboard (no network access, cheap). */
export const DASHBOARD_REFRESH_MS = 60_000;

export interface StatusValue {
  dashboard: Dashboard | null;
  dashboardError: AppError | null;
  /** Latest known update check result (dashboard, `get_updates`, `check_updates` or event). */
  updates: UpdateCheckResult | null;
  checking: boolean;
  /** Rejection of the last `check_updates` call. */
  checkError: AppError | null;
  refresh(): Promise<void>;
  refreshUpdates(): Promise<void>;
  /** Runs the isolated update check (deduplicated while running). */
  checkUpdates(): Promise<CheckOutcome>;
  /** Increments whenever history may have changed (operation finished). */
  activityVersion: number;
}

/** Result of `check_updates`: either a result (which may report `failed`) or the rejection. */
export interface CheckOutcome {
  result: UpdateCheckResult | null;
  error: AppError | null;
}

const StatusContext = createContext<StatusValue | null>(null);

export function useStatus(): StatusValue {
  const value = useContext(StatusContext);
  if (!value) throw new Error('useStatus outside of StatusProvider');
  return value;
}

const stamp = (result: UpdateCheckResult) => result.attemptedAt ?? result.checkedAt ?? 0;

/** Keeps the newer of two results; equal timestamps take the incoming result. */
export function newerUpdates(current: UpdateCheckResult | null, incoming: UpdateCheckResult): UpdateCheckResult {
  if (current === null) return incoming;
  return stamp(incoming) >= stamp(current) ? incoming : current;
}

export function StatusProvider({ children }: { children: ReactNode }) {
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [dashboardError, setDashboardError] = useState<AppError | null>(null);
  const [updates, setUpdates] = useState<UpdateCheckResult | null>(null);
  const [checking, setChecking] = useState(false);
  const [checkError, setCheckError] = useState<AppError | null>(null);
  const [activityVersion, setActivityVersion] = useState(0);
  const sequence = useRef(0);
  const checkPromise = useRef<Promise<CheckOutcome> | null>(null);

  const acceptUpdates = useCallback((result: UpdateCheckResult) => {
    setUpdates((current) => newerUpdates(current, result));
  }, []);

  const refresh = useCallback((): Promise<void> => {
    const id = ++sequence.current;
    return api.getDashboard().then(
      (next) => {
        if (id !== sequence.current) return;
        setDashboard(next);
        setDashboardError(null);
        acceptUpdates(next.updates);
      },
      (error: unknown) => {
        if (id === sequence.current) setDashboardError(normalizeError(error));
      },
    );
  }, [acceptUpdates]);

  const refreshUpdates = useCallback(
    (): Promise<void> =>
      api.getUpdates().then(acceptUpdates, () => {
        // The dashboard refresh reports backend problems; nothing to add here.
      }),
    [acceptUpdates],
  );

  const checkUpdates = useCallback(() => {
    if (checkPromise.current) return checkPromise.current;
    setChecking(true);
    setCheckError(null);
    const run = (async (): Promise<CheckOutcome> => {
      try {
        const result = await api.checkUpdates();
        setUpdates(result);
        void refresh();
        return { result, error: null };
      } catch (reason) {
        const error = normalizeError(reason);
        setCheckError(error);
        void refreshUpdates();
        return { result: null, error };
      } finally {
        checkPromise.current = null;
        setChecking(false);
      }
    })();
    checkPromise.current = run;
    return run;
  }, [refresh, refreshUpdates]);

  useEffect(() => {
    void refresh();
    const id = window.setInterval(() => {
      if (document.visibilityState !== 'hidden') void refresh();
    }, DASHBOARD_REFRESH_MS);
    return () => window.clearInterval(id);
  }, [refresh]);

  useEffect(() => {
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const keep = (promise: Promise<() => void>) =>
      promise.then(
        (fn) => (disposed ? fn() : unlisten.push(fn)),
        () => undefined,
      );
    void keep(
      api.onUpdatesChecked((result) => {
        acceptUpdates(result);
        void refresh();
      }),
    );
    void keep(
      api.onOperationFinished(() => {
        setActivityVersion((value) => value + 1);
        void refresh();
      }),
    );
    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
    };
  }, [acceptUpdates, refresh]);

  const value = useMemo<StatusValue>(
    () => ({ dashboard, dashboardError, updates, checking, checkError, refresh, refreshUpdates, checkUpdates, activityVersion }),
    [dashboard, dashboardError, updates, checking, checkError, refresh, refreshUpdates, checkUpdates, activityVersion],
  );
  return <StatusContext.Provider value={value}>{children}</StatusContext.Provider>;
}
