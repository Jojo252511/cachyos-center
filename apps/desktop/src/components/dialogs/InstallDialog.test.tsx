import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../../api/client';
import { mockBackend, renderApp } from '../../test/utils';
import { InstallDialog } from './InstallDialog';

describe('InstallDialog', () => {
  it('explains the full -Syu transaction and refreshes STALE repository data on request', async () => {
    const user = userEvent.setup();
    mockBackend('stale');
    const planInstall = vi.spyOn(api, 'planInstall');
    const checkUpdates = vi.spyOn(api, 'checkUpdates');
    renderApp(<InstallDialog target={{ repository: 'cachyos-extra-v3', name: 'gimp' }} onClose={vi.fn()} />);

    expect(await screen.findByText('Repository-Daten sind nicht aktuell')).toBeInTheDocument();
    const dialog = screen.getByRole('dialog', { name: 'gimp installieren' });
    expect(within(dialog).getByRole('button', { name: 'Installieren' })).toBeDisabled();
    expect(dialog).toHaveTextContent('pacman -Syu cachyos-extra-v3/gimp');

    await user.click(within(dialog).getByRole('button', { name: 'Jetzt aktualisieren' }));
    await waitFor(() => expect(planInstall).toHaveBeenCalledTimes(2));
    expect(checkUpdates).toHaveBeenCalledTimes(1);
    expect(await within(dialog).findByText(/Dabei werden zusätzlich 12 andere Systempakete aktualisiert\./)).toBeInTheDocument();
    expect(dialog).toHaveTextContent('Die Installation läuft als vollständige, konsistente Transaktion');
    expect(within(dialog).queryByText('Repository-Daten sind nicht aktuell')).not.toBeInTheDocument();
    await waitFor(() => expect(within(dialog).getByRole('button', { name: 'Installieren' })).toBeEnabled());
  });

  it('blocks the installation when the refresh fails', async () => {
    const user = userEvent.setup();
    mockBackend('offline');
    renderApp(<InstallDialog target={{ repository: 'extra', name: 'kdenlive' }} onClose={vi.fn()} />);
    await user.click(await screen.findByRole('button', { name: 'Jetzt aktualisieren' }));
    expect(await screen.findByText('Die Repository-Daten konnten nicht aktualisiert werden. Die Installation ist deshalb blockiert.')).toBeInTheDocument();
    expect(screen.getByText('Die Paketquellen sind nicht erreichbar. Prüfe die Netzwerkverbindung.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Installieren' })).toBeDisabled();
  });

  it('starts the installation with the digest of the shown plan', async () => {
    const user = userEvent.setup();
    mockBackend();
    const startInstall = vi.spyOn(api, 'startInstall').mockResolvedValue('33333333-3333-4333-8333-333333333333');
    const onClose = vi.fn();
    renderApp(<InstallDialog target={{ repository: 'cachyos-extra-v3', name: 'btop' }} onClose={onClose} />);
    const confirm = await screen.findByRole('button', { name: 'Installieren' });
    await waitFor(() => expect(confirm).toBeEnabled());
    const plan = await api.planInstall('cachyos-extra-v3', 'btop');
    await user.click(confirm);
    await waitFor(() => expect(startInstall).toHaveBeenCalledWith('cachyos-extra-v3', 'btop', plan.digest));
    expect(onClose).toHaveBeenCalled();
  });
});
