import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { buildUpgradePlan, UPDATE_DEFS } from '../../api/mockData';
import { createFormatters } from '../../i18n/format';
import type { NewsGate } from '../../lib/news';
import { renderI18n } from '../../test/utils';
import { UpgradeDialog } from './UpgradeDialog';

const plan = buildUpgradePlan(UPDATE_DEFS, 1_790_000_000);

function renderDialog(news: NewsGate, onConfirm = vi.fn().mockResolvedValue(undefined)) {
  renderI18n(<UpgradeDialog open onOpenChange={() => undefined} plan={plan} heldBack={[]} news={news} snapshot={null} onConfirm={onConfirm} />);
  return onConfirm;
}

describe('UpgradeDialog', () => {
  it('shows counts per action, download size, repositories and warnings', () => {
    renderDialog({ state: 'clear', unreadCount: 0, incomplete: false });
    const dialog = screen.getByRole('dialog', { name: 'Systemupgrade bestätigen' });
    expect(within(dialog).getByText('Aktualisieren').parentElement).toHaveTextContent('Aktualisieren12');
    expect(within(dialog).getByText('Pakete insgesamt').parentElement).toHaveTextContent('12');
    expect(within(dialog).getByText(createFormatters('de-DE').bytes(plan.downloadSize))).toBeInTheDocument();
    expect(dialog).toHaveTextContent('Repositories: cachyos-core-v3, core, cachyos-extra-v3, multilib, cachyos');
    expect(dialog).toHaveTextContent('Nach dem Vorgang wird ein Neustart empfohlen');
    expect(dialog).toHaveTextContent('Die Vorschau stammt aus der isolierten Prüfdatenbank. Maßgeblich ist die tatsächliche pacman-Transaktion');
  });

  it('requires the acknowledgement checkbox when news are unread', async () => {
    const user = userEvent.setup();
    const onConfirm = renderDialog({ state: 'unread', unreadCount: 2, incomplete: false });
    const confirm = screen.getByRole('button', { name: 'Upgrade starten' });
    expect(screen.getByText('Es gibt 2 ungelesene Arch-/CachyOS-Meldungen. Lies sie vor dem Upgrade.')).toBeInTheDocument();
    expect(confirm).toBeDisabled();
    await user.click(screen.getByRole('checkbox', { name: 'Ich habe die Hinweise gelesen und möchte fortfahren' }));
    expect(confirm).toBeEnabled();
    await user.click(confirm);
    expect(onConfirm).toHaveBeenCalledWith(false);
  });

  it('requires the acknowledgement checkbox when the news check is not possible', async () => {
    const user = userEvent.setup();
    renderDialog({ state: 'unavailable', unreadCount: 0, incomplete: true });
    expect(screen.getByText('News-Check nicht möglich. Prüfe die offiziellen Meldungen selbst, bevor du fortfährst.')).toBeInTheDocument();
    const confirm = screen.getByRole('button', { name: 'Upgrade starten' });
    expect(confirm).toBeDisabled();
    await user.click(screen.getByRole('checkbox', { name: 'Ich habe die Hinweise gelesen und möchte fortfahren' }));
    expect(confirm).toBeEnabled();
  });

  it('needs no acknowledgement when all news are read', () => {
    renderDialog({ state: 'clear', unreadCount: 0, incomplete: false });
    expect(screen.queryByRole('checkbox', { name: /Hinweise gelesen/ })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Upgrade starten' })).toBeEnabled();
  });

  it('keeps the confirm button disabled while news are being checked', () => {
    renderDialog({ state: 'loading', unreadCount: 0, incomplete: true });
    expect(screen.getByRole('button', { name: 'Upgrade starten' })).toBeDisabled();
  });

  it('shows why the upgrade cannot start', () => {
    renderI18n(
      <UpgradeDialog
        open
        onOpenChange={() => undefined}
        plan={plan}
        heldBack={[]}
        news={{ state: 'clear', unreadCount: 0, incomplete: false }}
        snapshot={null}
        blockedReason="Paketverwaltung beschäftigt: Ein anderer Paketmanager hält die Sperre."
        onConfirm={vi.fn()}
      />,
    );
    expect(screen.getByText('Paketverwaltung beschäftigt: Ein anderer Paketmanager hält die Sperre.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Upgrade starten' })).toBeDisabled();
  });
});
