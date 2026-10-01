import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { Operation } from '../bindings/Operation';
import type { OperationLogChunk } from '../bindings/OperationLogChunk';
import type { UpdateCheckResult } from '../bindings/UpdateCheckResult';
import { createMockTransport, parseScenario } from './mock';

describe('browser mock', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('selects scenarios from the URL query', () => {
    expect(parseScenario('?scenario=offline')).toBe('offline');
    expect(parseScenario('?scenario=nonsense')).toBe('default');
    expect(parseScenario('')).toBe('default');
  });

  it('simulates the operation lifecycle up to success', async () => {
    const mock = createMockTransport('', { latency: false });
    const finished = vi.fn();
    await mock.listen('operation-finished', finished);
    const updates = await mock.invoke<UpdateCheckResult>('get_updates');
    const id = await mock.invoke<string>('start_upgrade', { planDigest: updates.plan?.digest, createSnapshot: true });
    const state = async () => (await mock.invoke<Operation>('get_operation', { id })).state;
    const seen = new Set<string>([await state()]);
    for (let i = 0; i < 60; i += 1) {
      await vi.advanceTimersByTimeAsync(100);
      seen.add(await state());
    }
    expect([...seen]).toEqual(['awaitingAuthorization', 'preparing', 'downloading', 'installing', 'succeeded']);
    const operation = await mock.invoke<Operation>('get_operation', { id });
    expect(operation.changes.upgraded).toBe(12);
    expect(operation.snapshot?.created).toBe(true);
    expect(finished).toHaveBeenCalledTimes(1);
    const log = await mock.invoke<OperationLogChunk>('get_operation_log', { id, offset: 0 });
    expect(log.complete).toBe(true);
    expect(log.lines.some((line) => line.includes('upgrading linux-cachyos'))).toBe(true);
    const next = await mock.invoke<OperationLogChunk>('get_operation_log', { id, offset: log.nextOffset });
    expect(next.lines).toEqual([]);
    const after = await mock.invoke<UpdateCheckResult>('get_updates');
    expect(after.updates).toHaveLength(0);
  });

  it('allows cancelling only before the commit', async () => {
    const mock = createMockTransport('', { latency: false });
    const updates = await mock.invoke<UpdateCheckResult>('get_updates');
    const id = await mock.invoke<string>('start_upgrade', { planDigest: updates.plan?.digest, createSnapshot: false });
    await vi.advanceTimersByTimeAsync(1200);
    const cancelled = await mock.invoke<Operation>('cancel_operation', { id });
    expect(cancelled.state).toBe('cancelledBeforeCommit');

    const second = await mock.invoke<string>('start_upgrade', { planDigest: updates.plan?.digest, createSnapshot: false });
    for (let i = 0; i < 40 && (await mock.invoke<Operation>('get_operation', { id: second })).state !== 'installing'; i += 1) {
      await vi.advanceTimersByTimeAsync(100);
    }
    expect((await mock.invoke<Operation>('get_operation', { id: second })).state).toBe('installing');
    await expect(mock.invoke('cancel_operation', { id: second })).rejects.toMatchObject({ code: 'INVALID_INPUT' });
  });

  it('ends with PLAN_CHANGED and an actual plan in the planChanged scenario', async () => {
    const mock = createMockTransport('?scenario=planChanged', { latency: false });
    const updates = await mock.invoke<UpdateCheckResult>('get_updates');
    const id = await mock.invoke<string>('start_upgrade', { planDigest: updates.plan?.digest, createSnapshot: false });
    await vi.advanceTimersByTimeAsync(3000);
    const operation = await mock.invoke<Operation>('get_operation', { id });
    expect(operation.state).toBe('cancelledBeforeCommit');
    expect(operation.error?.code).toBe('PLAN_CHANGED');
    expect(operation.actualPlan?.digest).not.toBe(updates.plan?.digest);
    const retry = await mock.invoke<string>('start_upgrade', { planDigest: operation.actualPlan?.digest, createSnapshot: false });
    await vi.advanceTimersByTimeAsync(10_000);
    expect((await mock.invoke<Operation>('get_operation', { id: retry })).state).toBe('succeeded');
  });

  it('reports a foreign lock and refuses to start in parallel', async () => {
    const mock = createMockTransport('?scenario=locked', { latency: false });
    const updates = await mock.invoke<UpdateCheckResult>('get_updates');
    await expect(mock.invoke('start_upgrade', { planDigest: updates.plan?.digest, createSnapshot: false })).rejects.toMatchObject({ code: 'BUSY' });
  });
});
