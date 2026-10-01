import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from './api/client';
import type { Operation } from './bindings/Operation';
import { App } from './App';
import { OperationPanel } from './components/operation/OperationPanel';
import { SettingsPage } from './pages/SettingsPage';
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
      progress: { currentPackage: 'systemd', packagesDone: 4, packagesTotal: 12, phaseDetail: 'installing packages' },
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
