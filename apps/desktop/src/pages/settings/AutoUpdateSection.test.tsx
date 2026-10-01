import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../../api/client';
import type { AutoUpdateStatus } from '../../bindings/AutoUpdateStatus';
import { mockBackend, renderApp } from '../../test/utils';
import { AutoUpdateSection } from './AutoUpdateSection';

describe('AutoUpdateSection', () => {
  it('disables „Automatisch beim nächsten Neustart installieren“ with blockers and explains it directly below', async () => {
    mockBackend();
    renderApp(<AutoUpdateSection />);
    const option = await screen.findByRole('radio', { name: /Automatisch beim nächsten Neustart installieren/ });
    expect(option).toBeDisabled();
    expect(screen.getByRole('radio', { name: 'Aus' })).toBeChecked();
    expect(screen.getByText('in Entwicklung')).toBeInTheDocument();
    expect(
      screen.getByText(/ist ein echtes Opt-in für die automatische Installation über den vorhandenen CachyOS-Offline-Mechanismus/),
    ).toHaveTextContent('cachyos-center startet den Rechner nie automatisch neu.');
    expect(screen.getByText('pacman-offline ist nicht installiert.')).toBeInTheDocument();
    expect(screen.getByText(/muss von der Administration freigeschaltet werden/)).toBeInTheDocument();
  });

  it('applies the policy only with the separate „Übernehmen“ button', async () => {
    const user = userEvent.setup();
    mockBackend();
    const setPolicy = vi.spyOn(api, 'setAutoUpdatePolicy');
    renderApp(<AutoUpdateSection />);
    const apply = await screen.findByRole('button', { name: 'Übernehmen' });
    expect(apply).toBeDisabled();
    await user.click(screen.getByRole('radio', { name: 'Nur benachrichtigen' }));
    expect(setPolicy).not.toHaveBeenCalled();
    expect(apply).toBeEnabled();
    await user.click(apply);
    await waitFor(() => expect(setPolicy).toHaveBeenCalledTimes(1));
    expect(setPolicy.mock.calls[0]?.[0]).toMatchObject({ policy: 'notifyOnly', window: { time: '12:00' } });
    expect(await screen.findByText('Richtlinie übernommen.')).toBeInTheDocument();
  });

  it('acknowledges the current news for the scheduled preparation via the policy', async () => {
    const user = userEvent.setup();
    const backend = mockBackend();
    const status = await backend.invoke<AutoUpdateStatus>('get_auto_update_status');
    vi.spyOn(api, 'getAutoUpdateStatus').mockResolvedValue({ ...status, config: { ...status.config, policy: 'notifyOnly' } });
    const setPolicy = vi.spyOn(api, 'setAutoUpdatePolicy').mockImplementation(async (config) => ({ ...status, config }));
    renderApp(<AutoUpdateSection />);
    expect(await screen.findByText('noch nie')).toBeInTheDocument();
    const before = Math.floor(Date.now() / 1000);
    await user.click(screen.getByRole('button', { name: 'Aktuelle News als gelesen bestätigen' }));
    await waitFor(() => expect(setPolicy).toHaveBeenCalledTimes(1));
    const config = setPolicy.mock.calls[0]?.[0];
    expect(config?.policy).toBe('notifyOnly');
    expect(config?.newsAcknowledgedUntil).toBeGreaterThanOrEqual(before);
    expect(await screen.findByText(/^News bis .+ bestätigt\.$/)).toBeInTheDocument();
  });

  it('hides the news acknowledgement while the policy is off', async () => {
    mockBackend();
    renderApp(<AutoUpdateSection />);
    await screen.findByRole('button', { name: 'Übernehmen' });
    expect(screen.queryByRole('button', { name: 'Aktuelle News als gelesen bestätigen' })).not.toBeInTheDocument();
  });
});
