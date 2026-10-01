import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';

import { api } from '../api/client';
import { normalizeError } from '../api/errors';
import type { AppError } from '../bindings/AppError';
import type { Operation } from '../bindings/Operation';
import type { OperationState } from '../bindings/OperationState';
import type { TransactionPlan } from '../bindings/TransactionPlan';
import { useStatus } from './Status';

/** Polling interval of the contract (`get_operation`). */
export const POLL_INTERVAL_MS = 750;
const MAX_LOG_LINES = 5000;
/** Extra polls after a terminal state when the log does not report `complete`. */
const MAX_TERMINAL_POLLS = 8;
/** Successful results older than this are not shown again after a GUI restart. */
const RESUME_SUCCESS_WINDOW_S = 24 * 60 * 60;
const ACK_STORAGE_KEY = 'cachyos-center.acknowledgedOperation';

const TERMINAL: readonly OperationState[] = ['succeeded', 'failed', 'needsAttention', 'cancelledBeforeCommit'];

export function isTerminalState(state: OperationState): boolean {
  return TERMINAL.includes(state);
}

/** Cancelling is only offered before the pacman commit phase. */
export function canCancel(state: OperationState): boolean {
  return state === 'awaitingAuthorization' || state === 'preparing' || state === 'downloading';
}

/** A finished operation whose result must be acknowledged before new package actions. */
export function requiresAcknowledgement(operation: Operation): boolean {
  if (!isTerminalState(operation.state)) return false;
  if (operation.outcomeUnknown) return true;
  if (operation.state === 'failed' || operation.state === 'needsAttention') return true;
  return operation.state === 'cancelledBeforeCommit' && operation.error !== null;
}

export function isPlanChanged(operation: Operation | null): operation is Operation & { actualPlan: TransactionPlan } {
  return operation?.error?.code === 'PLAN_CHANGED' && operation.actualPlan !== null;
}

export type StartRequest =
  | { kind: 'upgrade'; plan: TransactionPlan; createSnapshot: boolean }
  | { kind: 'install'; repository: string; name: string; plan: TransactionPlan }
  | { kind: 'remove'; name: string; recursive: boolean; plan: TransactionPlan };

export interface TrackedOperation {
  id: string;
  /** `null` until the first status poll returned. */
  operation: Operation | null;
  /** Request confirmed in this session; `null` after a GUI restart. */
  request: StartRequest | null;
  log: string[];
  logComplete: boolean;
  pollError: AppError | null;
}

export interface OperationsValue {
  tracked: TrackedOperation | null;
  /** An operation is running (not terminal yet). */
  active: boolean;
  /** A failed result is shown and blocks further package actions until dismissed. */
  resultPending: boolean;
  expanded: boolean;
  setExpanded(expanded: boolean): void;
  start(request: StartRequest): Promise<void>;
  /** Starts a new operation with the actual plan after a `PLAN_CHANGED` result. */
  restartWithPlan(plan: TransactionPlan, createSnapshot?: boolean): Promise<void>;
  cancel(): Promise<void>;
  dismiss(): void;
  /** Incremented to ask the live view to reveal and focus the log („Log ansehen“). */
  logRequest: number;
  showLog(): void;
}

const OperationsContext = createContext<OperationsValue | null>(null);

export function useOperations(): OperationsValue {
  const value = useContext(OperationsContext);
  if (!value) throw new Error('useOperations outside of OperationsProvider');
  return value;
}

function readAcknowledged(): string | null {
  try {
    return window.localStorage.getItem(ACK_STORAGE_KEY);
  } catch {
    return null;
  }
}

function storeAcknowledged(id: string): void {
  try {
    window.localStorage.setItem(ACK_STORAGE_KEY, id);
  } catch {
    // Storage is optional; the result is simply shown again after a restart.
  }
}

/** Decides whether an operation reported by `get_current_operation` is shown after a GUI restart. */
export function shouldResume(operation: Operation, acknowledgedId: string | null, nowSeconds: number): boolean {
  if (operation.kind === 'updateCheck') return false;
  if (!isTerminalState(operation.state)) return true;
  if (operation.id === acknowledgedId) return false;
  if (requiresAcknowledgement(operation)) return true;
  return operation.endedAt !== null && nowSeconds - operation.endedAt < RESUME_SUCCESS_WINDOW_S;
}

function startCommand(request: StartRequest): Promise<string> {
  switch (request.kind) {
    case 'upgrade':
      return api.startUpgrade(request.plan.digest, request.createSnapshot);
    case 'install':
      return api.startInstall(request.repository, request.name, request.plan.digest);
    case 'remove':
      return api.startRemove(request.name, request.recursive, request.plan.digest);
  }
}

/** Rebuilds the request for the actual plan of a `PLAN_CHANGED` result. */
export function requestForPlan(tracked: TrackedOperation, plan: TransactionPlan, createSnapshot?: boolean): StartRequest | null {
  const previous = tracked.request;
  if (previous?.kind === 'upgrade') return { ...previous, plan, createSnapshot: createSnapshot ?? previous.createSnapshot };
  if (previous) return { ...previous, plan };
  const operation = tracked.operation;
  if (!operation) return null;
  switch (operation.kind) {
    case 'systemUpgrade':
      return { kind: 'upgrade', plan, createSnapshot: createSnapshot ?? false };
    case 'install': {
      const target = plan.entries.find((entry) => entry.requested && entry.repository !== null);
      return target?.repository ? { kind: 'install', repository: target.repository, name: target.name, plan } : null;
    }
    case 'remove': {
      const name = operation.packageTargets[0] ?? plan.targets[0];
      return name ? { kind: 'remove', name, recursive: plan.recursive, plan } : null;
    }
    default:
      return null;
  }
}

export function OperationsProvider({ children }: { children: ReactNode }) {
  const status = useStatus();
  const [tracked, setTracked] = useState<TrackedOperation | null>(null);
  const [expanded, setExpanded] = useState(true);
  const [logRequest, setLogRequest] = useState(0);
  const offsetRef = useRef(0);
  const trackedRef = useRef<TrackedOperation | null>(null);
  const refreshedFor = useRef<string | null>(null);
  const { refresh } = status;

  useEffect(() => {
    trackedRef.current = tracked;
  });

  // Resume a running operation (or show an unacknowledged result) after a GUI restart.
  useEffect(() => {
    let active = true;
    api.getCurrentOperation().then(
      (operation) => {
        if (!active || !operation || trackedRef.current !== null) return;
        if (!shouldResume(operation, readAcknowledged(), Math.floor(Date.now() / 1000))) return;
        offsetRef.current = 0;
        setTracked({ id: operation.id, operation, request: null, log: [], logComplete: false, pollError: null });
      },
      () => undefined,
    );
    return () => {
      active = false;
    };
  }, []);

  const trackedId = tracked?.id ?? null;
  const operationState = tracked?.operation?.state ?? null;
  const terminal = operationState !== null && isTerminalState(operationState);
  const needsPolling = tracked !== null && (!terminal || !tracked.logComplete);

  useEffect(() => {
    if (trackedId === null || !needsPolling) return;
    let cancelled = false;
    let timer: number | undefined;
    let terminalPolls = 0;

    const tick = async () => {
      try {
        const operation = await api.getOperation(trackedId);
        if (cancelled) return;
        const isFinal = isTerminalState(operation.state);
        if (isFinal) terminalPolls += 1;
        let lines: string[] = [];
        let logComplete = false;
        try {
          const chunk = await api.getOperationLog(trackedId, offsetRef.current);
          if (cancelled) return;
          offsetRef.current = chunk.nextOffset;
          lines = chunk.lines;
          logComplete = chunk.complete;
        } catch {
          // A missing log of a finished operation must not keep the poll alive.
          logComplete = isFinal;
        }
        const done = isFinal && (logComplete || terminalPolls >= MAX_TERMINAL_POLLS);
        setTracked((previous) =>
          previous && previous.id === trackedId
            ? {
                ...previous,
                operation,
                log: lines.length > 0 ? [...previous.log, ...lines].slice(-MAX_LOG_LINES) : previous.log,
                logComplete: done,
                pollError: null,
              }
            : previous,
        );
        if (done) return;
      } catch (error) {
        if (cancelled) return;
        const pollError = normalizeError(error);
        setTracked((previous) => (previous && previous.id === trackedId ? { ...previous, pollError } : previous));
      }
      if (!cancelled) timer = window.setTimeout(tick, POLL_INTERVAL_MS);
    };

    void tick();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [trackedId, needsPolling]);

  // Re-read package state once per finished operation.
  useEffect(() => {
    if (trackedId !== null && terminal && refreshedFor.current !== trackedId) {
      refreshedFor.current = trackedId;
      void refresh();
    }
  }, [trackedId, terminal, refresh]);

  // The backend reports finished operations as an event as well.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    api
      .onOperationFinished((operation) => {
        setTracked((previous) =>
          previous && previous.id === operation.id && !(previous.operation && isTerminalState(previous.operation.state))
            ? { ...previous, operation }
            : previous,
        );
      })
      .then(
        (fn) => (disposed ? fn() : (unlisten = fn)),
        () => undefined,
      );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const start = useCallback(async (request: StartRequest) => {
    const id = await startCommand(request);
    const previous = trackedRef.current;
    if (previous?.operation && isTerminalState(previous.operation.state)) storeAcknowledged(previous.id);
    offsetRef.current = 0;
    setTracked({ id, operation: null, request, log: [], logComplete: false, pollError: null });
    setExpanded(true);
  }, []);

  const restartWithPlan = useCallback(
    async (plan: TransactionPlan, createSnapshot?: boolean) => {
      const current = trackedRef.current;
      const request = current ? requestForPlan(current, plan, createSnapshot) : null;
      if (!request) throw normalizeError({ code: 'INVALID_INPUT', message: 'operation cannot be restarted', detail: null });
      await start(request);
    },
    [start],
  );

  const cancel = useCallback(async () => {
    const current = trackedRef.current;
    if (!current) return;
    const operation = await api.cancelOperation(current.id);
    setTracked((previous) => (previous && previous.id === operation.id ? { ...previous, operation } : previous));
  }, []);

  const dismiss = useCallback(() => {
    const current = trackedRef.current;
    if (!current?.operation || !isTerminalState(current.operation.state)) return;
    storeAcknowledged(current.id);
    setTracked(null);
  }, []);

  const showLog = useCallback(() => {
    setExpanded(true);
    setLogRequest((value) => value + 1);
  }, []);

  const active = tracked !== null && !terminal;
  const resultPending = tracked?.operation ? requiresAcknowledgement(tracked.operation) : false;

  const value = useMemo<OperationsValue>(
    () => ({ tracked, active, resultPending, expanded, setExpanded, start, restartWithPlan, cancel, dismiss, logRequest, showLog }),
    [tracked, active, resultPending, expanded, start, restartWithPlan, cancel, dismiss, logRequest, showLog],
  );
  return <OperationsContext.Provider value={value}>{children}</OperationsContext.Provider>;
}
