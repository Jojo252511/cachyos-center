import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from './api/client';
import { createMockTransport } from './api/mock';
import type { Operation } from './bindings/Operation';
import type { UpdateCheckResult } from './bindings/UpdateCheckResult';
import { App } from './App';
import { OperationPanel } from './components/operation/OperationPanel';
import { SettingsPage } from './pages/SettingsPage';
import { SoftwarePage } from './pages/SoftwarePage';
import { UpdatesPage } from './pages/UpdatesPage';
import { mockBackend, renderApp } from './test/utils';
import { render } from '@testing-library/react';

describe('application shell', () => {
  it('offers exactly the six navigation entries and marks the current page', async () => {
    mockBackend();
    render(<App />);
    const nav = await screen.findByRole('navigation', { name: 'Hauptnavigation' });
    const links = within(nav).getAllByRole('link');
    expect(links.map((link) => link.textContent?.replace(/, .*$/, '').replace(/\d+$/, ''))).toEqual([
      'Übersicht',
      'Updates',
      'Software',
      'System',
      'Aktivität',
      'Einstellungen',
    ]);
    expect(within(nav).getByRole('link', { name: 'Übersicht' })).toHaveAttribute('aria-current', 'page');
    expect(await screen.findByRole('heading', { level: 1, name: 'Übersicht' })).toBeInTheDocument();
    expect(screen.getByText('Demodaten: Browser-Vorschau ohne echte Systemdaten')).toBeInTheDocument();
  });
});

describe('package action guards', () => {
  it('starts nothing while another package manager holds the lock', async () => {
    mockBackend('locked');
    renderApp(<UpdatesPage />);
    expect(await screen.findByText('Paketverwaltung beschäftigt')).toBeInTheDocument();
    expect(screen.getByText(/entfernt die Sperre nicht/)).toBeInTheDocument();
    const install = screen.getByRole('button', { name: 'Installieren' });
    await waitFor(() => expect(screen.getByText(/Installation nicht möglich: Paketverwaltung beschäftigt/)).toBeInTheDocument());
    expect(install).toBeDisabled();
  });

  it('explains an incompatible libalpm and blocks package changes', async () => {
    mockBackend('unsupported');
    renderApp(<UpdatesPage />);
    expect(await screen.findByText(/Die installierte libalpm passt nicht zu diesem Build/)).toBeInTheDocument();
    expect(screen.getByText(/Technischer Grund: .*libalpm\.so\.17/)).toBeInTheDocument();
    expect(await screen.findByRole('button', { name: 'Installieren' })).toBeDisabled();
  });

  it('does not call an online upgrade complete while packages are held back', async () => {
    mockBackend('offlineConfHeld');
    renderApp(<UpdatesPage />);
    expect(await screen.findByText(/ohne die von der pacman-Konfiguration zurückgehaltenen Pakete/)).toBeInTheDocument();
    expect(screen.queryByText('Vollständiges Systemupgrade mit pacman -Syu')).toBeNull();
  });

  it('blocks package changes without the helper service', async () => {
    mockBackend('helperMissing');
    renderApp(<UpdatesPage />);
    await waitFor(() => expect(screen.getByText(/Installation nicht möglich: Der Hilfsdienst/)).toBeInTheDocument());
    expect(screen.getByText('Hilfsdienst nicht verfügbar')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Installieren' })).toBeDisabled();
  });
});

describe('GUI restart during an operation', () => {
  it('resumes the live view of a running operation', async () => {
    mockBackend();
    const running: Operation = {
      id: '44444444-4444-4444-8444-444444444444',
      kind: 'systemUpgrade',
      origin: 'user',
      requestedAt: 1_790_000_000,
      state: 'installing',
      packageTargets: [],
      startedAt: 1_790_000_001,
      endedAt: null,
      exitCode: null,
      summary: '',
      error: null,
      commitStarted: true,
      confirmedDigest: 'c'.repeat(64),
      actualPlan: null,
      progress: { currentPackage: 'systemd', packagesDone: 4, packagesTotal: 12, step: 'applyingChanges' },
      snapshot: null,
      newPacnewFiles: 0,
      changes: { installed: 0, upgraded: 0, downgraded: 0, reinstalled: 0, removed: 0, packages: [] },
      outcomeUnknown: false,
    };
    vi.spyOn(api, 'getCurrentOperation').mockResolvedValue(running);
    vi.spyOn(api, 'getOperation').mockResolvedValue(running);
    vi.spyOn(api, 'getOperationLog').mockResolvedValue({ lines: ['(4/12) upgrading mesa'], nextOffset: 22, complete: false });
    renderApp(<OperationPanel />);
    expect(await screen.findByText('Installation läuft; Abbruch möglicherweise gefährlich')).toBeInTheDocument();
    expect(screen.getByText('4 von 12 Paketen')).toBeInTheDocument();
    expect(await screen.findByText(/upgrading mesa/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Abbrechen' })).not.toBeInTheDocument();
  });
});

describe('settings', () => {
  it('saves preferences immediately and switches the language', async () => {
    const user = userEvent.setup();
    mockBackend();
    const saveSettings = vi.spyOn(api, 'saveSettings');
    renderApp(<SettingsPage />);
    await user.click(await screen.findByRole('radio', { name: 'English' }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ language: 'en' })));
    expect(await screen.findByRole('heading', { level: 1, name: 'Settings' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'English' })).toBeChecked();
  });

  it('offers the MCP host configuration only for copying', async () => {
    const user = userEvent.setup();
    mockBackend();
    const writeClipboard = vi.spyOn(api, 'writeClipboard').mockResolvedValue(undefined);
    renderApp(<SettingsPage />);
    expect(await screen.findByText(/Paketnamen und Hardware können private Details verraten/)).toBeInTheDocument();
    const config = await screen.findByLabelText('Host-Konfiguration');
    expect(config).toHaveTextContent('/usr/bin/cachyos-center-mcp');
    expect(writeClipboard).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Konfiguration kopieren' }));
    expect(writeClipboard).toHaveBeenCalledWith(config.textContent);
  });
});

/** Holds `check_updates` back until the returned function runs: busy state first, result later. */
function holdCheck(): () => void {
  let release: () => void = () => undefined;
  const original = api.checkUpdates.bind(api);
  vi.spyOn(api, 'checkUpdates').mockImplementation(
    () =>
      new Promise((resolve, reject) => {
        release = () => void original().then(resolve, reject);
      }),
  );
  return () => release();
}

describe('focus after closing non-modal parts', () => {
  it('gives the focus back to the package row when the details close', async () => {
    const user = userEvent.setup();
    mockBackend();
    renderApp(<SoftwarePage />);
    const row = (await screen.findAllByRole('button', { name: /^Details zu / }))[0];
    if (!row) throw new Error('no package row');
    await user.click(row);
    await user.click(await screen.findByRole('button', { name: 'Details schließen' }));
    await waitFor(() => expect(row).toHaveFocus());
  });

  it('moves the focus to the page title when a result is dismissed', async () => {
    const user = userEvent.setup();
    mockBackend();
    const failed: Operation = {
      id: '77777777-7777-4777-8777-777777777777',
      kind: 'systemUpgrade',
      origin: 'user',
      requestedAt: 1_790_000_000,
      state: 'failed',
      packageTargets: [],
      startedAt: 1_790_000_001,
      endedAt: 1_790_000_050,
      exitCode: 1,
      summary: 'downloading failed',
      error: { code: 'TRANSACTION_FAILED', message: 'failed retrieving file', detail: null },
      commitStarted: false,
      confirmedDigest: 'd'.repeat(64),
      actualPlan: null,
      progress: { currentPackage: null, packagesDone: 0, packagesTotal: 12, step: null },
      snapshot: null,
      newPacnewFiles: 0,
      changes: { installed: 0, upgraded: 0, downgraded: 0, reinstalled: 0, removed: 0, packages: [] },
      outcomeUnknown: false,
    };
    vi.spyOn(api, 'getCurrentOperation').mockResolvedValue(failed);
    vi.spyOn(api, 'getOperation').mockResolvedValue(failed);
    vi.spyOn(api, 'getOperationLog').mockResolvedValue({ lines: [], nextOffset: 0, complete: true });
    render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Ergebnis schließen' }));
    await waitFor(() => expect(screen.getByRole('heading', { level: 1 })).toHaveFocus());
  });

  it('keeps the focus in the operation panel after cancelling', async () => {
    const user = userEvent.setup();
    mockBackend();
    const downloading: Operation = {
      id: '88888888-8888-4888-8888-888888888888',
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
      confirmedDigest: 'e'.repeat(64),
      actualPlan: null,
      progress: { currentPackage: 'mesa', packagesDone: 0, packagesTotal: 12, step: 'downloadingPackages' },
      snapshot: null,
      newPacnewFiles: 0,
      changes: { installed: 0, upgraded: 0, downgraded: 0, reinstalled: 0, removed: 0, packages: [] },
      outcomeUnknown: false,
    };
    const cancelled: Operation = { ...downloading, state: 'cancelledBeforeCommit', endedAt: 1_790_000_030 };
    vi.spyOn(api, 'getCurrentOperation').mockResolvedValue(downloading);
    const poll = vi.spyOn(api, 'getOperation').mockResolvedValue(downloading);
    vi.spyOn(api, 'getOperationLog').mockResolvedValue({ lines: [], nextOffset: 0, complete: false });
    vi.spyOn(api, 'cancelOperation').mockImplementation(async () => {
      poll.mockResolvedValue(cancelled);
      return cancelled;
    });
    render(<App />);
    await user.click(await screen.findByRole('button', { name: 'Abbrechen' }));
    await waitFor(() => expect(document.getElementById('operation-panel-title')).toHaveFocus());
    expect(await screen.findByRole('button', { name: 'Ergebnis schließen' })).toBeInTheDocument();
  });

  it('keeps the focus in the news row after marking the news as read', async () => {
    const user = userEvent.setup();
    mockBackend('newsUnread');
    // Hold the answer back to reproduce the real order: busy first, state change later.
    let release: () => void = () => undefined;
    const original = api.acknowledgeNews.bind(api);
    vi.spyOn(api, 'acknowledgeNews').mockImplementation(
      (until) =>
        new Promise((resolve, reject) => {
          release = () => void original(until).then(resolve, reject);
        }),
    );
    renderApp(<UpdatesPage />);
    const button = await screen.findByRole('button', { name: 'Als gelesen markieren' });
    const status = screen.getByText(/ungelesene Meldung/);
    button.focus();
    await user.keyboard('{Enter}');
    // While the request runs the busy button stays focusable: aria-disabled, never
    // `disabled` (browsers move the focus of a disabled element to <body>).
    expect(button).toHaveAttribute('aria-busy', 'true');
    expect(button).not.toBeDisabled();
    expect(button).toHaveFocus();
    release();
    await waitFor(() => expect(screen.queryByRole('button', { name: 'Als gelesen markieren' })).toBeNull());
    // The same status element (not a replaced one) now says that nothing is unread and has the focus.
    await waitFor(() => expect(status).toHaveTextContent(/Keine ungelesenen Meldungen/));
    await waitFor(() => expect(status).toHaveFocus());
    expect(status.isConnected).toBe(true);
  });

  it('keeps the focus on a busy button while its action runs', async () => {
    const user = userEvent.setup();
    mockBackend();
    let release: () => void = () => undefined;
    const original = api.checkUpdates.bind(api);
    vi.spyOn(api, 'checkUpdates').mockImplementation(
      () =>
        new Promise((resolve, reject) => {
          release = () => void original().then(resolve, reject);
        }),
    );
    renderApp(<UpdatesPage />);
    const check = await screen.findByRole('button', { name: 'Jetzt prüfen' });
    check.focus();
    await user.keyboard('{Enter}');
    const busy = await screen.findByRole('button', { name: 'Prüfung läuft …' });
    expect(busy).toHaveAttribute('aria-disabled', 'true');
    expect(busy).not.toBeDisabled();
    expect(busy).toHaveFocus();
    // Activating it again while busy does nothing.
    await user.keyboard('{Enter}');
    expect(api.checkUpdates).toHaveBeenCalledTimes(1);
    release();
    const done = await screen.findByRole('button', { name: 'Jetzt prüfen' });
    expect(done).toHaveFocus();
  });
});

describe('focus when a re-render removes or disables the focused control', () => {
  it('hands the focus to the header check button when the first check replaces the empty state', async () => {
    const user = userEvent.setup();
    mockBackend('neverChecked');
    const release = holdCheck();
    window.location.hash = '#/updates';
    render(<App />);
    await screen.findByText('Noch keine Updateprüfung');
    const header = document.getElementById('updates-check');
    const empty = screen.getAllByRole('button', { name: 'Jetzt prüfen' }).find((button) => button !== header);
    if (!header || !empty) throw new Error('check buttons missing');
    empty.focus();
    await user.keyboard('{Enter}');
    // The empty state is gone while the check runs; the header button shows the check and has the focus.
    await waitFor(() => expect(header).toHaveFocus());
    expect(empty.isConnected).toBe(false);
    expect(header).toHaveAccessibleName('Prüfung läuft …');
    release();
    await waitFor(() => expect(header).toHaveAccessibleName('Jetzt prüfen'));
    expect(header).toHaveFocus();
  });

  it('hands the focus to „Updates ansehen“ when the first check on the overview finds updates', async () => {
    const user = userEvent.setup();
    mockBackend('neverChecked');
    const release = holdCheck();
    render(<App />);
    const check = await screen.findByRole('button', { name: 'Jetzt prüfen' });
    check.focus();
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('button', { name: 'Prüfung läuft …' })).toHaveFocus();
    release();
    const view = await screen.findByRole('link', { name: 'Updates ansehen' });
    // The button was replaced by a link, a different element: the focus follows it.
    expect(check.isConnected).toBe(false);
    await waitFor(() => expect(view).toHaveFocus());
  });

  it('keeps the focus when „Erneut prüfen“ removes the error of a rejected check', async () => {
    const user = userEvent.setup();
    mockBackend();
    const original = api.checkUpdates.bind(api);
    let release: () => void = () => undefined;
    vi.spyOn(api, 'checkUpdates')
      .mockRejectedValueOnce({ code: 'BUSY', message: 'another update check is running', detail: null })
      .mockImplementation(
        () =>
          new Promise((resolve, reject) => {
            release = () => void original().then(resolve, reject);
          }),
      );
    window.location.hash = '#/updates';
    render(<App />);
    const header = await screen.findByRole('button', { name: 'Jetzt prüfen' });
    await user.click(header);
    const retry = await screen.findByRole('button', { name: 'Erneut prüfen' });
    retry.focus();
    await user.keyboard('{Enter}');
    await waitFor(() => expect(header).toHaveFocus());
    expect(retry.isConnected).toBe(false);
    release();
    await waitFor(() => expect(header).toHaveAccessibleName('Jetzt prüfen'));
    expect(header).toHaveFocus();
  });

  it('keeps the focus when a successful retry removes the error panel of a failed check', async () => {
    const user = userEvent.setup();
    mockBackend('offline');
    const online = createMockTransport('?scenario=default', { latency: false });
    vi.spyOn(api, 'checkUpdates').mockImplementation(() => online.invoke<UpdateCheckResult>('check_updates'));
    window.location.hash = '#/updates';
    render(<App />);
    const retry = await screen.findByRole('button', { name: 'Erneut prüfen' });
    retry.focus();
    await user.keyboard('{Enter}');
    await waitFor(() => expect(retry.isConnected).toBe(false));
    await waitFor(() => expect(document.getElementById('updates-check')).toHaveFocus());
  });

  it('moves the focus to the result line when saving disables „Übernehmen“', async () => {
    const user = userEvent.setup();
    mockBackend();
    window.location.hash = '#/settings';
    render(<App />);
    await user.click(await screen.findByRole('radio', { name: /Nur benachrichtigen/ }));
    const apply = screen.getByRole('button', { name: 'Übernehmen' });
    apply.focus();
    await user.keyboard('{Enter}');
    const applied = await screen.findByText('Richtlinie übernommen.');
    await waitFor(() => expect(apply).toBeDisabled());
    // jsdom keeps the focus on a disabled button; WebKitGTK and Chromium blur it shortly after.
    act(() => {
      apply.removeAttribute('disabled');
      apply.blur();
      apply.setAttribute('disabled', '');
    });
    await waitFor(() => expect(applied).toHaveFocus());
  });
});
