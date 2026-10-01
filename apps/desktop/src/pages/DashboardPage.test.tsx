import { screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { ScenarioId } from '../api/mock';
import { mockBackend, renderApp } from '../test/utils';
import { DashboardPage } from './DashboardPage';

describe('DashboardPage', () => {
  it.each<[ScenarioId, string]>([
    ['neverChecked', 'Noch nicht geprüft'],
    ['offline', 'Prüfung fehlgeschlagen'],
    ['unsupported', 'Paketfunktionen nicht verfügbar'],
    ['prerequisiteMissing', 'Updateprüfung nicht möglich'],
  ])('never shows "0 Updates" for the uncertain state of scenario %s', async (scenario, title) => {
    mockBackend(scenario);
    renderApp(<DashboardPage />);
    expect(await screen.findByRole('heading', { name: title })).toBeInTheDocument();
    expect(document.body).not.toHaveTextContent(/\b0 Updates?\b/);
    expect(document.body).not.toHaveTextContent('System aktuell');
    expect(screen.getByText('Anzahl unbekannt')).toBeInTheDocument();
  });

  it('says that stale data is outdated', async () => {
    mockBackend('stale');
    renderApp(<DashboardPage />);
    expect(await screen.findByRole('heading', { name: 'Updateinformationen veraltet' })).toBeInTheDocument();
    expect(screen.getByText(/und sind veraltet\. Prüfe erneut/)).toBeInTheDocument();
    expect(screen.getByText('Anzahl unbekannt')).toBeInTheDocument();
  });

  it('shows the number of updates of a fresh check, the cards and the last update', async () => {
    mockBackend();
    renderApp(<DashboardPage />);
    expect(await screen.findByRole('heading', { name: '12 Updates verfügbar' })).toBeInTheDocument();
    for (const card of ['Updates', 'System', 'Software', 'Gesundheit']) {
      expect(screen.getByRole('heading', { name: card })).toBeInTheDocument();
    }
    expect(screen.getByText(/^Letztes Update: .*, erfolgreich$/)).toBeInTheDocument();
    expect(screen.getByText(/^Nächste automatische Prüfung: /)).toBeInTheDocument();
    expect(screen.getByRole('link', { name: 'Updates ansehen' })).toHaveAttribute('href', '#/updates');
  });

  it('shows the busy package manager and an offline update banner', async () => {
    mockBackend('locked');
    renderApp(<DashboardPage />);
    expect(await screen.findByRole('heading', { name: 'Paketverwaltung beschäftigt' })).toBeInTheDocument();
  });

  it('announces a prepared offline update', async () => {
    mockBackend('offlinePrepared');
    renderApp(<DashboardPage />);
    expect(await screen.findByText('Updates werden beim nächsten Neustart installiert')).toBeInTheDocument();
  });
});
