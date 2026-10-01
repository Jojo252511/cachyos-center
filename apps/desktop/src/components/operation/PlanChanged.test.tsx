import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../../api/client';
import type { Operation } from '../../bindings/Operation';
import type { TransactionPlan } from '../../bindings/TransactionPlan';
import type { UpdateCheckResult } from '../../bindings/UpdateCheckResult';
import { UpdatesPage } from '../../pages/UpdatesPage';
import { mockBackend, renderApp } from '../../test/utils';
import { OperationPanel } from './OperationPanel';

const FIRST = '11111111-1111-4111-8111-111111111111';
const SECOND = '22222222-2222-4222-8222-222222222222';

function operation(id: string, patch: Partial<Operation>): Operation {
  return {
    id,
    kind: 'systemUpgrade',
    origin: 'user',
    requestedAt: 1_790_000_000,
    state: 'preparing',
    packageTargets: [],
    startedAt: 1_790_000_000,
    endedAt: null,
    exitCode: null,
    summary: '',
    error: null,
    commitStarted: false,
    confirmedDigest: null,
    actualPlan: null,
    progress: { currentPackage: null, packagesDone: 0, packagesTotal: null, phaseDetail: null },
    snapshot: null,
    newPacnewFiles: 0,
    changes: { installed: 0, upgraded: 0, downgraded: 0, reinstalled: 0, removed: 0, packages: [] },
    outcomeUnknown: false,
    ...patch,
  };
}

describe('PLAN_CHANGED', () => {
  it('shows the difference and starts a new operation with the digest of the actual plan', async () => {
    const user = userEvent.setup();
    const backend = mockBackend();
    const confirmed = (await backend.invoke<UpdateCheckResult>('get_updates')).plan as TransactionPlan;
    const actual: TransactionPlan = {
      ...confirmed,
      digest: 'b'.repeat(64),
      entries: [
        ...confirmed.entries
          .filter((entry) => entry.name !== 'cachyos-keyring')
          .map((entry) => (entry.name === 'firefox' ? { ...entry, newVersion: '157.0.2-1.1' } : entry)),
        { action: 'upgrade', name: 'libdrm', repository: 'cachyos-extra-v3', oldVersion: '2.4.127-1.1', newVersion: '2.4.128-1.1', downloadSize: 400_000, requested: false, flags: ['driver'] },
      ],
    };

    const startUpgrade = vi.spyOn(api, 'startUpgrade').mockResolvedValueOnce(FIRST).mockResolvedValueOnce(SECOND);
    vi.spyOn(api, 'getOperation').mockImplementation(async (id) =>
      id === FIRST
        ? operation(FIRST, {
            state: 'cancelledBeforeCommit',
            endedAt: 1_790_000_010,
            confirmedDigest: confirmed.digest,
            actualPlan: actual,
            error: { code: 'PLAN_CHANGED', message: 'the actual plan differs from the confirmed plan', detail: null },
          })
        : operation(SECOND, { state: 'awaitingAuthorization', confirmedDigest: actual.digest }),
    );
    vi.spyOn(api, 'getOperationLog').mockResolvedValue({ lines: ['plan digest mismatch'], nextOffset: 21, complete: true });

    renderApp(
      <>
        <OperationPanel />
        <UpdatesPage />
      </>,
    );

    const install = await screen.findByRole('button', { name: 'Installieren' });
    await waitFor(() => expect(install).toBeEnabled());
    await user.click(install);
    const confirm = await screen.findByRole('button', { name: 'Upgrade starten' });
    await waitFor(() => expect(confirm).toBeEnabled());
    await user.click(confirm);
    expect(startUpgrade).toHaveBeenNthCalledWith(1, confirmed.digest, false);

    const panel = await screen.findByRole('region', { name: 'Paketvorgang' });
    expect(await within(panel).findByText('Der Paketplan hat sich geändert')).toBeInTheDocument();
    expect(within(panel).getByText('Hinzugekommen (1)')).toBeInTheDocument();
    expect(within(panel).getByText('libdrm')).toBeInTheDocument();
    expect(within(panel).getByText('Geändert (1)')).toBeInTheDocument();
    expect(within(panel).getByText('firefox')).toBeInTheDocument();
    expect(within(panel).getByText('157.0.1-1.1 → 157.0.2-1.1')).toBeInTheDocument();
    expect(within(panel).getByText('Weggefallen (1)')).toBeInTheDocument();
    expect(within(panel).getByText('cachyos-keyring')).toBeInTheDocument();
    // Further package actions are blocked until the result is handled.
    expect(screen.getByRole('button', { name: 'Installieren' })).toBeDisabled();

    await user.click(within(panel).getByRole('button', { name: 'Neuen Plan prüfen' }));
    const dialog = await screen.findByRole('dialog', { name: 'Geänderten Paketplan bestätigen' });
    expect(within(dialog).getAllByText('libdrm').length).toBeGreaterThan(0);
    const restart = within(dialog).getByRole('button', { name: 'Neuen Plan bestätigen und starten' });
    await waitFor(() => expect(restart).toBeEnabled());
    await user.click(restart);

    await waitFor(() => expect(startUpgrade).toHaveBeenCalledTimes(2));
    expect(startUpgrade).toHaveBeenLastCalledWith(actual.digest, false);
    expect(await within(panel).findAllByText('Warte auf Authentifizierung …')).not.toHaveLength(0);
  });
});
