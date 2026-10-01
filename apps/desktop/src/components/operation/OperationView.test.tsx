import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import type { Operation } from '../../bindings/Operation';
import type { TrackedOperation } from '../../state/Operations';
import { renderI18n } from '../../test/utils';
import { OperationView, type OperationViewProps } from './OperationView';

function operation(patch: Partial<Operation>): Operation {
  return {
    id: '0b0e7d4c-1f6a-4c55-9a8e-2f3b4c5d6e7f',
    kind: 'systemUpgrade',
    origin: 'user',
    requestedAt: 1_790_000_000,
    state: 'downloading',
    packageTargets: [],
    startedAt: 1_790_000_001,
    endedAt: null,
    exitCode: null,
    summary: '',
    error: null,
    commitStarted: false,
    confirmedDigest: 'a'.repeat(64),
    actualPlan: null,
    progress: { currentPackage: 'mesa', packagesDone: 0, packagesTotal: 12, step: 'downloadingPackages' },
    snapshot: null,
    newPacnewFiles: 0,
    changes: { installed: 0, upgraded: 0, downgraded: 0, reinstalled: 0, removed: 0, packages: [] },
    outcomeUnknown: false,
    ...patch,
  };
}

function renderView(op: Operation, overrides: Partial<OperationViewProps> = {}) {
  const tracked: TrackedOperation = { id: op.id, operation: op, request: null, log: ['==> pacman -Syu requested'], logComplete: false, pollError: null };
  const props: OperationViewProps = {
    tracked,
    expanded: true,
    onToggleExpanded: vi.fn(),
    onCancel: vi.fn(),
    cancelBusy: false,
    cancelError: null,
    onDismiss: vi.fn(),
    onReviewPlan: vi.fn(),
    onRecheck: vi.fn(),
    recheckBusy: false,
    onViewLog: vi.fn(),
    logRequest: 0,
    confirmedPlan: null,
    rebootRecommended: false,
    resultPending: false,
    ...overrides,
  };
  renderI18n(<OperationView {...props} />);
  return props;
}

describe('OperationView', () => {
  it('offers cancelling while downloading and shows the package progress', async () => {
    const user = userEvent.setup();
    const props = renderView(operation({ state: 'downloading' }));
    expect(screen.getAllByText('Pakete werden heruntergeladen').length).toBeGreaterThan(0);
    expect(screen.getByText('0 von 12 Paketen')).toBeInTheDocument();
    expect(screen.getByText('Pakete werden heruntergeladen und ihre Signaturen geprüft …')).toBeInTheDocument();
    expect(screen.getByRole('progressbar', { name: 'Verarbeitete Pakete' })).toHaveAttribute('max', '12');
    await user.click(screen.getByRole('button', { name: 'Abbrechen' }));
    expect(props.onCancel).toHaveBeenCalledTimes(1);
  });

  it('shows no cancel button but the danger text while installing', () => {
    renderView(operation({ state: 'installing', commitStarted: true, progress: { currentPackage: 'systemd', packagesDone: 5, packagesTotal: 12, step: null } }));
    expect(screen.queryByRole('button', { name: 'Abbrechen' })).not.toBeInTheDocument();
    expect(screen.getByText('Installation läuft; Abbruch möglicherweise gefährlich')).toBeInTheDocument();
    expect(screen.getByText('5 von 12 Paketen')).toBeInTheDocument();
  });

  it('shows the progress text only when the total is known', () => {
    renderView(operation({ state: 'preparing', progress: { currentPackage: null, packagesDone: 0, packagesTotal: null, step: 'synchronizingDatabases' } }));
    expect(screen.queryByText(/von \d+ Paketen/)).not.toBeInTheDocument();
    expect(screen.queryByRole('progressbar')).not.toBeInTheDocument();
    expect(screen.getByText('Die Paketanzahl wird angezeigt, sobald pacman sie meldet.')).toBeInTheDocument();
  });

  it('asks to authenticate in the polkit dialog without a password field', () => {
    renderView(operation({ state: 'awaitingAuthorization', progress: { currentPackage: null, packagesDone: 0, packagesTotal: null, step: null } }));
    expect(screen.getAllByText('Warte auf Authentifizierung …').length).toBeGreaterThan(0);
    expect(screen.getByRole('button', { name: 'Abbrechen' })).toBeInTheDocument();
    expect(document.querySelector('input[type="password"]')).toBeNull();
  });

  it('reports a failure before the commit as unchanged package state', () => {
    renderView(
      operation({
        state: 'failed',
        endedAt: 1_790_000_100,
        error: { code: 'TRANSACTION_FAILED', message: 'invalid or corrupted package (PGP signature)', detail: null },
      }),
    );
    expect(screen.getByText('Fehlgeschlagen – Paketlage unverändert')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Log ansehen' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Erneut prüfen' })).toBeInTheDocument();
    // The databases may already be synchronized: warn against pacman -S alone.
    expect(screen.getByText(/nicht mit „pacman -S“ allein: Das wäre eine Teilaktualisierung/)).toBeInTheDocument();
  });

  it('gives no partial-upgrade hint for removals, which never synchronize', () => {
    renderView(operation({ kind: 'remove', state: 'cancelledBeforeCommit', endedAt: 1_790_000_100 }));
    expect(screen.getByText('Abgebrochen – Paketlage unverändert')).toBeInTheDocument();
    expect(screen.queryByText(/Teilaktualisierung/)).toBeNull();
  });

  it('reports an unclear package state after the commit with a terminal repair hint', () => {
    renderView(
      operation({
        state: 'needsAttention',
        commitStarted: true,
        endedAt: 1_790_000_100,
        error: { code: 'TRANSACTION_FAILED', message: 'failed to commit transaction', detail: null },
      }),
      { resultPending: true },
    );
    expect(screen.getByText('Paketlage unklar – bitte prüfen')).toBeInTheDocument();
    expect(screen.getByText('Im Terminal reparieren')).toBeInTheDocument();
    expect(screen.getByText('sudo pacman -Syu')).toBeInTheDocument();
    expect(screen.getByText('Weitere Paketaktionen sind gesperrt, bis du dieses Ergebnis geschlossen hast.')).toBeInTheDocument();
  });

  it('summarizes a successful operation with change counts, .pacnew and reboot hints', () => {
    renderView(
      operation({
        state: 'succeeded',
        commitStarted: true,
        endedAt: 1_790_000_100,
        newPacnewFiles: 1,
        changes: { installed: 2, upgraded: 12, downgraded: 0, reinstalled: 0, removed: 0, packages: ['linux-cachyos'] },
      }),
      { rebootRecommended: true },
    );
    expect(screen.getByText('Vorgang erfolgreich abgeschlossen')).toBeInTheDocument();
    expect(screen.getByText('12 Pakete aktualisiert')).toBeInTheDocument();
    expect(screen.getByText('2 Pakete installiert')).toBeInTheDocument();
    expect(screen.getByText(/1 neue .pacnew\/.pacsave-Datei angelegt/)).toBeInTheDocument();
    expect(screen.getByText(/Ein Neustart wird empfohlen/)).toBeInTheDocument();
  });
});
