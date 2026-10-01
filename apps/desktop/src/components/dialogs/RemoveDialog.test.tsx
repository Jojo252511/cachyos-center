import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../../api/client';
import { mockBackend, renderApp } from '../../test/utils';
import { RemoveDialog } from './RemoveDialog';

describe('RemoveDialog', () => {
  it('blocks on DEPENDENCY_PROBLEM and lists the dependent packages', async () => {
    mockBackend();
    renderApp(<RemoveDialog name="gtk3" onClose={vi.fn()} />);
    const notice = (await screen.findByText('Entfernen nicht möglich')).closest('[role="alert"]') as HTMLElement;
    expect(notice).toHaveTextContent('Andere installierte Pakete hängen von gtk3 ab.');
    const items = within(notice).getAllByRole('listitem').map((item) => item.textContent);
    expect(items).toEqual(['cachyos-hello', 'firefox', 'visual-studio-code-bin', 'waybar']);
    expect(screen.getByRole('button', { name: 'Entfernen' })).toBeDisabled();
  });

  it('shows a strong warning and requires an extra confirmation for critical packages', async () => {
    const user = userEvent.setup();
    mockBackend();
    renderApp(<RemoveDialog name="sudo" critical onClose={vi.fn()} />);
    expect(await screen.findByText('Systemkritisches Paket')).toBeInTheDocument();
    expect(screen.getByText('Das Entfernen von sudo kann das System unbenutzbar oder nicht mehr startfähig machen.')).toBeInTheDocument();
    const confirm = screen.getByRole('button', { name: 'Entfernen' });
    expect(confirm).toBeDisabled();
    await user.click(screen.getByRole('checkbox', { name: 'Ich verstehe das Risiko und möchte diese Pakete trotzdem entfernen' }));
    await waitFor(() => expect(confirm).toBeEnabled());
  });

  it('re-plans when unneeded dependencies should be removed as well', async () => {
    const user = userEvent.setup();
    mockBackend();
    const planRemove = vi.spyOn(api, 'planRemove');
    renderApp(<RemoveDialog name="hyprpolkitagent" onClose={vi.fn()} />);
    expect(await screen.findByText('Dieses Paket wird entfernt')).toBeInTheDocument();
    expect(planRemove).toHaveBeenLastCalledWith('hyprpolkitagent', false);
    const recursive = screen.getByRole('checkbox', { name: 'Nicht mehr benötigte Abhängigkeiten mitentfernen' });
    expect(recursive).not.toBeChecked();
    await user.click(recursive);
    expect(await screen.findByText('Diese 2 Pakete werden entfernt')).toBeInTheDocument();
    expect(planRemove).toHaveBeenLastCalledWith('hyprpolkitagent', true);
    expect(screen.getAllByText('polkit').length).toBeGreaterThan(0);
    expect(screen.getByText(/Persönliche Daten in deinem Home-Verzeichnis werden nicht gelöscht/)).toBeInTheDocument();
  });

  it('offers no option to skip dependency checks', async () => {
    mockBackend();
    renderApp(<RemoveDialog name="hyprpolkitagent" onClose={vi.fn()} />);
    await screen.findByText('Dieses Paket wird entfernt');
    expect(screen.getAllByRole('checkbox')).toHaveLength(1);
    expect(document.body).not.toHaveTextContent(/nodeps|-Rdd|cascade/i);
  });
});
